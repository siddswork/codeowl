# C++ step 2b: one spec per class, across files

Step 2b of `docs/cpp-pack-plan.md` (§4.6 pairing, §4.7 shared ids across files). Written 2026-11-15 after tracing the core, before any code. Decisions for the owner are section 5; nothing is built until they are settled.

## 1. What this step delivers

A C++ class is declared in a header and implemented in one or more source files. Today each out-of-class definition (`void DBImpl::Get(...) { … }` in `db_impl.cc`) is its own top-level symbol with its own spec, and the class's spec never sees a body. After this step:

| Today | After |
|---|---|
| `db_impl.cc::DBImpl::Get` is a top-level symbol and spec-bearing | Its body belongs to `db_impl.h::DBImpl` (to the member `DBImpl::Get`); the `.cc` symbol is gone |
| Editing a body in the `.cc` makes nothing of the class stale | The class's spec goes stale; users of the class do not (its interface hash covers the declaration only) |
| Editing a private member in the header leaves `Widget::size` (in `widget.cc`) current, pinned as a known limit in `tests/cpp_staleness.rs` | The class's spec goes stale, because the method is part of the class |
| A free function's prototype in `db.h` and its body in `db.cc` are two symbols | One symbol in the header, written from the body |
| A `.cc` holding only bodies has a spec section per method | It has no spec-bearing symbol, so no file spec (the existing rule for such a file) |

This is the last step before generation is allowed on a C++ repository (`docs/cpp-pack-plan.md` §5).

## 2. What the core does today (traced, with places)

| Fact | Where | Consequence |
|---|---|---|
| The graph is built in production in one place: `RepoIndex::rebuild`, from clones of the per-file extractions the index caches, on every change; the watcher's `apply_changes` re-extracts the changed files and calls `rebuild` | `index.rs` `rebuild`, `apply_changes` | Pairing can run in `rebuild`, before `Graph::build`, over the cached extractions. A body edit in a `.cc` re-pairs by itself: **no watcher change is needed** (the pack plan expected one) |
| Other `Graph::build` callers are tests, the Python pack's test, and `examples/cpp_probe.rs` | `grep Graph::build` | The probe must pair too, or its edges drift from the index's |
| `extra_spans: Vec<[usize; 2]>` exists on `ExtractedSymbol` and `Symbol` and carries **no file**: every span is in the symbol's own file. Written by the Rust impl fold and the TypeScript and C++ same-id folds | `symbol.rs`, `rust/mod.rs:346`, `typescript/mod.rs:300`, `cpp/mod.rs:209` | A span in another file needs a file beside it |
| One reader of spans: `merged_symbol_spans` (own span, children outside it, `extra_spans`, merged, sorted), and `symbol_span_text`, which reads **one file** | `spec/source_text.rs` | Both assume one file |
| Five callers read a symbol's text through those: the generation task (`mcp.rs` `get_next_spec_task`), `get_source` (`mcp.rs`), the task's largest-container share (`spec/task_input.rs`), the dependency scope for staleness (`spec/staleness.rs` `try_dependency_pairs`), and the rendered `Depends on` list (`spec/format.rs` `dependency_lines`) | as listed | All five must see the other file's text |
| The dependency scope keeps the edges **from the symbol's file** whose word appears in the symbol's text (`imp.from_file == file.id && contains_identifier(text, imp.imported_name)`) | `staleness.rs`, `source_text.rs` `scoped_symbol_deps` | A body's dependencies are edges *from the `.cc`*. The scope must be taken per file: the `.cc` edges against the `.cc` text |
| `get_source` returns one `file`, one `lines` envelope and one `graph_in_sync` flag (the file's current hash against the graph's) | `mcp.rs` `get_source` | Needs a shape for text in a second file (decision P4) |
| A large container's task shows an outline (member signatures and docstrings, no bodies) above 4,000 bytes; every task text is capped at 8,000 | `source_text.rs` `maybe_reduce_container_source`, `container_outline` | A class with dozens of bodies is already handled the way a Java god class is (decision P5) |
| A file is spec-bearing when it has a top-level callable or container; the sections are the file's top-level symbols | `spec/layout.rs` `file_is_spec_bearing`, `spec_bearing_children` | Removing a paired definition from its file's symbols removes its section, and a bodies-only `.cc` loses its file spec, with no layout change |
| An out-of-class definition is extracted as a top-level symbol, raw `qualified_function`, id `<file>::A::b`, template arguments stripped (`Box<T>::f` is `Box::f`); a static data member definition (`int C::n = 0;`) is a top-level `variable` with `::` in its name; a prototype carries the `declaration` marker | `cpp/mod.rs` `function_raw`, `declaration`, `plain_name` | The pairing rule reads these |
| Namespaces are not in ids | `cpp/mod.rs` module doc | Two classes of one name in different namespaces have the same name to the rule (risk, section 9) |

## 3. Measured before designing

A throwaway probe (deleted) applied the rule of section 4.2 to the four repositories:

| Qualified definitions (`A::b`) | `leveldb` | `abseil` | `fmt` | `json` |
|---|---|---|---|---|
| One class `A` in the same file | 89 | 365 | 125 | 9 |
| One class `A` in one reachable header | 228 | 604 | 262 | 0 |
| No class `A` reachable | 0 | 63 | 19 | 12 |
| Several classes `A` reachable | 0 | 16 | 1 | 0 |
| **Paired** | **100 %** | **92.5 %** | **95.1 %** | **42.9 %** |

| Free functions with a body | `leveldb` | `abseil` | `fmt` | `json` |
|---|---|---|---|---|
| One prototype in a reachable header | 126 | 468 | 116 | 13 |
| Several prototypes reachable | 0 | 27 | 0 | 1 |
| No prototype elsewhere (stays where it is) | 142 | 3,059 | 824 | 1,828 |

What the unpaired are, from the examples: `json`'s 12 are `std::` specialisations (`std::is_trivial<…>`), which are not member definitions and must stay unpaired; `abseil`'s 63 and part of `fmt`'s 19 are **namespace-qualified free functions** (`absl::base_internal::SpinLockWake`, `detail::format_windows_error`), which a fallback can pair to their prototype (decision P3); `abseil`'s 16 ambiguous are classes such as `Storage` declared in two namespaces.

## 4. Design

### 4.1 Where pairing runs

A new `StackPack` hook with a default that does nothing, called by `RepoIndex::rebuild` before `Graph::build`:

```rust
fn pair_definitions(&self, files: &mut [FileExtraction], file_imports: &HashMap<String, FileImports>) {}
```

The C++ pack implements it in a new `src/cpp/pairing.rs`. It needs the include graph (`cpp::resolve::IncludeGraph`, built from `file_imports`) and every file's symbols, which is why it cannot run at extraction (one file at a time) or in `resolve_imports` (it gets the graph read-only, after it is built). `graph.rs` does not change its logic and names no pack (layering kept); the index already calls pack hooks. The per-file cache is untouched: pairing works on the clones `rebuild` makes, so a re-extraction of one file simply re-pairs.

### 4.2 The pairing rule

For each file `f` in path order, each top-level symbol `d` of `f` with a body:

1. **A qualified name `A::b`** (or `A::B::c`): the candidates are the containers with id `<g>::A` (resp. `<g>::A::B`) for `g` in `f` and `reach(f)`. Exactly one: pair `d` with it. None: go to rule 3. Several: leave `d` where it is and count it.
2. **An unqualified function** (raw `function`, no `declaration` marker) in a file that is not the prototype's: the candidates are the symbols `<g>::b` with the `declaration` marker in `reach(f)`. Exactly one: pair. Otherwise leave and count.
3. **A qualified name with no class** (`ns::f`): treat the last segment as an unqualified function and apply rule 2 (decision P3).
4. Never paired: an `operator` at namespace scope (rule 2 handles a free `operator==` like any function only if it has a prototype), a definition whose qualifier names a `std::` entity (rule 1 finds no class, rule 3 finds no prototype: it stays), a test (`raw == "test"`).

Overloads need no parameter matching: the same-id fold of step 2 already made one symbol of every overload in a file, so a pairing moves the whole group.

### 4.3 What a pairing does

| | Value |
|---|---|
| The definition `d` | Removed from `f`'s symbols (its children, if any, with it; a definition has none unless it is an out-of-class nested class, which is left unpaired and counted) |
| The target | The member of the class that `d` defines, found by name among the class's children (`db_impl.h::DBImpl::Get`); the class itself when no member matches (decision P2). For rule 2 and 3, the prototype |
| The target's new spans | `d`'s main span and its `extra_spans`, each **with `f` as its file**, in a new field (decision P1) |
| The target's source hash | One hash over its own and `d`'s, so an edit to the body moves it |
| The class's source hash (when the target is a member) | Folded the same way: the class's hash is a fold of its members', and a member changed |
| Interface hashes | Unchanged. A body edit must not make users stale |
| `declaration` marker | Dropped from a prototype that received a body |
| Docstring | The target's; `d`'s when the target has none |

### 4.4 Readers in the core

- **Text.** `merged_symbol_spans` becomes per file: the symbol's own file first, then each other file in path order, each with its merged spans; a container gathers its members' spans in other files too. `symbol_span_text` (used for the generation task and the size check) joins them, each block in another file preceded by one line naming it (`// from db/db_impl.cc, lines 120-180`), so the writer knows where a body lives.
- **Dependency scope.** `try_dependency_pairs`, `dependency_lines` and the task's dependency list take the edges from each file against that file's text (the header's edges against the declaration, the `.cc`'s against the bodies). One helper serves all three, so they cannot disagree.
- **`get_source`.** Decision P4.

### 4.5 Spec layout and staleness

No change to `layout.rs`: the paired definition is no longer a top-level symbol of its file. A class's spec goes stale on an edit to its declaration or any body (source hash), and on a change to anything a body or the declaration names (dependency scope); its users go stale only when the declaration's interface moves. The header's **file** summary does not go stale on a body edit in the `.cc`: the header's fingerprint is unchanged (decision P8).

## 5. Decisions for the owner

| # | Question | Recommendation | Why |
|---|---|---|---|
| P1 | How a span in another file is stored | A **new field** `other_file_spans: Vec<(String, [usize; 2])>`, `#[serde(default, skip_serializing_if = "Vec::is_empty")]`, on `ExtractedSymbol` and `Symbol`; `extra_spans` unchanged | Changing `extra_spans`' type touches the Rust and TypeScript packs and every test that builds one; a separate, empty-by-default field leaves every other pack's output byte-identical, which is what makes `graph_stats` a clean control |
| P2 | Where a body attaches | To the **member it defines** (the class when none matches); the class's text and hash gather it | `get_source` on `DBImpl::Get` then shows the body, and the class still covers everything. Attaching only to the class is simpler but leaves the member's source a bare declaration |
| P3 | A namespace-qualified free function (`absl::base_internal::SpinLockWake`) | Fall back to the free-function rule on the last segment | 63 on `abseil` and some of `fmt`'s 19 are exactly this; without it they stay separate symbols |
| P4 | `get_source` for a symbol with spans in other files | The home file's text, then each other file's blocks under a line naming the file and lines; a new optional field `elsewhere: [{file, lines}]`; `graph_in_sync` false if any of the files changed; `lines` stays the home envelope | Keeps the response readable as one text and backward compatible for one-file symbols |
| P5 | A class with dozens of bodies in a task | Reuse the existing outline above 4,000 bytes (member signatures, no bodies); no new paging in this step; measure `DBImpl`'s size and record it | It is how a Java god class is handled; a symbol-level ladder is new machinery to build only if the outline proves too thin in a real generation |
| P6 | What a reader of a bodies-only `.cc` on GitHub sees | Nothing in this step (no file spec) | The header's spec covers it; a pointer file is new layout work with no measured need |
| P7 | Where the unpaired count is shown | In the accuracy probe and the step's measurements now; to users in step 8, with the unresolved-include count (S2-5) | One place for "what the pack could not read", built once |
| P8 | The header's file summary on a `.cc` body edit | Stays current | The summary describes the header; the class's own spec is what goes stale |

Out of scope: a public header naming the file that implements it (`DB` in `db.h` is implemented by `DBImpl` through inheritance, not pairing; pack plan §4.5, with the compiler layer).

## 6. Build sequence

Each part: tests first (red, then green), a negative control where it applies, `utility/check.sh`, `/code-review`.

| Part | Content | Done when |
|---|---|---|
| A | **The core.** The new field on `ExtractedSymbol` and `Symbol`, carried by `Graph::build`; per-file spans and text in `source_text.rs`; the per-file dependency scope (one helper for staleness, `Depends on` and the task); `get_source` per P4. `FORMAT_VERSION` bump | Tests on a hand-built graph with a symbol whose span is in a second file (no C++ needed): its text has both blocks with the file line; its dependency scope takes the second file's edges against the second file's text only; `get_source` returns both and flags a changed second file. `graph_stats` identical on the seven other-pack repositories |
| B | **The pairing pass.** The `pair_definitions` hook, called in `rebuild`; `cpp/pairing.rs` with rules 1 to 4 and section 4.3 | A test for each row of the pack plan's §4.6 table that pairing touches (bodies in the header: no pairing; some bodies in a `.cc`; bodies below the class in one file; a `.cc` with private classes and functions; a free function with a prototype; a `static` helper; a class implemented across two `.cc` files), plus: two reachable classes of one name stay unpaired; a namespace-qualified function pairs to its prototype; a `std::` specialisation stays; a static data member pairs to the class |
| C | **End to end and measurement.** Tests through `RepoIndex::open` and `submit` (pattern of `tests/cpp_staleness.rs`): a body edit in the `.cc` makes the class's spec stale and no user's; the part D known-limit assertion flips; `get_source` on the class and on the member shows the body. `cpp_probe` pairs before building its graph; the accuracy check equals its last figures; the paired share on the four repositories is recorded with the unpaired explained | All pass; numbers in section 11 |

## 7. Acceptance

| Check | Required |
|---|---|
| `utility/check.sh` | Passes |
| `graph_stats` on the other packs' seven repositories, before and after | Identical |
| `utility/cpp_accuracy/run.sh` | Equal to the figures of 2026-11-15 (pairing moves no edge target: a paired definition was never a target) |
| Paired share | Recorded per repository; every unpaired group explained by kind (ambiguous, `std::` specialisation, no prototype) |
| No two symbols of one file share an id, on the four repositories | None (kept from step 2) |
| The end-to-end tests of part C | Pass |

## 8. Cost

| Measure | Method | Kill criterion |
|---|---|---|
| Pairing time per rebuild | Time it on `abseil` | Over one second: memoise per file and re-measure |
| Graph size | `.codeowl/graph` on `abseil`, before and after | Should shrink slightly (definitions fold into their classes); a growth over 10 % is investigated |
| Largest paired class text | `DBImpl` on `leveldb`, `Cord` or `Status` on `abseil` | Informs P5 |

## 9. Risks

- **A wrong pairing puts a body in the wrong class's spec.** Namespaces are not in ids, so a definition inside `namespace a` can pair with a unique reachable `b::B` when `a::B` lives in a file the definition does not reach. Mitigation: two candidates stay unpaired; check by hand a sample of 30 cross-file pairings on `abseil` and record the result. If wrong pairings show up, record each symbol's namespace as a marker at extraction and require it to match.
- **More dependencies for a class.** Scoping now covers the bodies, so a class spec goes stale when something a body uses changes. Correct, and the price of describing behaviour, but it raises regeneration volume on implementation-heavy classes.
- **A tool contract change.** `get_source` gains a field and labelled blocks; the tool description must say so.
- **`get_symbol` on the old `.cc` id fails after pairing.** No C++ spec exists yet, so nothing is orphaned; agents reach the body through the class or `search_code`.
- **The `.inc` gap from step 2 stays**: a body defined in an `.inc` file is not read, so it cannot pair.

## 10. To look at later

- A symbol-level ladder for a class whose outline proves too thin (P5).
- A pointer from a bodies-only `.cc` to the header's spec (P6).
- Namespace markers, if the hand check of section 9 finds wrong pairings.

## 11. Progress

(Empty: nothing built.)
