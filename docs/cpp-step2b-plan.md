# C++ step 2b: one spec per class, across files

Step 2b of `docs/cpp-pack-plan.md` (§4.6 pairing, §4.7 shared ids across files). Written 2026-11-15 after tracing the core, before any code. **N1 (namespaces in ids) was decided by the owner on 2026-11-15 and reverses D3 of the pack plan; it is built first (part 0).** P1 to P8 (section 5) are proposals the owner has not yet settled; nothing but part 0 is to be built until they are.

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
| Namespaces are not in ids (D3, 2026-11-10), and `edges.rs` reads "a `::` in the name after the file prefix" as "an out-of-class definition, never a target" | `cpp/mod.rs` module doc and `fn id`, `cpp/edges.rs` | Two declarations of one name in different namespaces share an id and are merged into one symbol (section 3). **Reversed by N1.** Once a namespace chain is in the id, a `::` no longer means "out-of-class definition"; `edges.rs`, `cpp_probe` and `compare.py` must read a marker instead (part 0) |

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

**Namespaces** (a second throwaway probe, deleted, 2026-11-15; it parsed each file for its `namespace` blocks, treated an `inline` namespace as transparent and detected macro-opened namespaces by a naming heuristic, so the collision counts are the reliable figures and the macro shares approximate):

| | `leveldb` | `abseil` | `fmt` | `json` |
|---|---|---|---|---|
| Top-level declarations | 1,301 | 16,028 | 5,106 | 6,910 |
| Inside a named namespace as written | 80.2 % | 87.7 % | 54.4 % | 42.4 % |
| Only inside a namespace opened by a macro | 0 | 1.5 % | 9.4 % | 7.8 % |
| **Ids shared by declarations in different namespaces** (wrongly merged into one symbol today) | **0** | **62** | **24** | **45** |
| Ambiguous qualified definitions (several reachable classes of the name) | 0 | 16 | 1 | 0 |
| ...settled by the namespace the definition sits in | | 16 of 16 | 0 of 1 | |

Examples of the wrong merges: `absl::big_endian::FromHost` and `absl::little_endian::FromHost` are one `FromHost`; `count` and `detail::count`, `is_contiguous` and `detail::is_contiguous` on `fmt`; `hash` in `detail` and in `std` on `json`. The `fmt` case left ambiguous is `format_facet`, defined in a test file, with two candidates in the same namespace. For `abseil` the namespace macro (`ABSL_NAMESPACE_BEGIN`) opens only an inline ABI namespace, which is transparent; for `fmt` (`FMT_BEGIN_NAMESPACE`) and `json` (`NLOHMANN_JSON_NAMESPACE_BEGIN`) the macro hides the library's own `namespace fmt {` and `namespace nlohmann {`, so a chain read from the text lacks its outermost name until step 3 (section 4.0).

What the unpaired are, from the examples: `json`'s 12 are `std::` specialisations (`std::is_trivial<…>`), which are not member definitions and must stay unpaired; `abseil`'s 63 and part of `fmt`'s 19 are **namespace-qualified free functions** (`absl::base_internal::SpinLockWake`, `detail::format_windows_error`), which a fallback can pair to their prototype (decision P3); `abseil`'s 16 ambiguous are classes such as `Storage` declared in two namespaces.

## 4. Design

### 4.0 Namespaces in ids (N1, decided 2026-11-15; reverses D3)

**The decision.** The id of a symbol is `<file>::<chain>::<name>`, where `<chain>` is the enclosing named namespaces, outermost first (`db/db.h::leveldb::DB`, `db/db.h::leveldb::DB::Open`). It replaces `<file>::<name>`. Why: on the three repositories that have them, 62, 24 and 45 ids covered declarations of different namespaces and were merged into one symbol with one spec (section 3), and the 16 ambiguous pairings on `abseil` are settled by the namespace. D3 recorded "no" without a reason; the owner reversed it on seeing these figures. It is cheapest now: no C++ spec exists, and after step 2b generation is allowed, when every id change orphans specs.

**The chain rules.**

| Case | In the id |
|---|---|
| `namespace a { namespace b { … } }` and `namespace a::b { … }` | `a::b`, the same either way |
| `inline namespace v11 { … }` (an ABI version) | Omitted: transparent, as for name lookup |
| An anonymous namespace | Omitted. It keeps making what is inside it file-local (as today). A name in an anonymous namespace and the same name at global scope of one file are ambiguous to use anyway; part 0 counts how many ids this would still merge, and records it |
| A class | A scope like a namespace: members are `<file>::<chain>::<Class>::<member>`, and a member's `parent` is the class id |
| An out-of-class definition `void DBImpl::Get()` inside `namespace leveldb { … }` | `<file>::leveldb::DBImpl::Get`: the chain of where it is written, then the name as written. Written outside, as `void leveldb::DBImpl::Get()`, it is `<file>::leveldb::DBImpl::Get` too, by construction (the qualification is the chain) |
| `extern "C" { … }` | Not a namespace; unchanged |
| A namespace opened by a macro (`FMT_BEGIN_NAMESPACE`) | Not seen, until step 3's macro table. The chain then lacks the outer name (`detail::f` for `fmt::detail::f`) |

**What stops working and what replaces it.** A `::` in the part after the file prefix used to mean "an out-of-class definition". With a chain in the id it does not. So:

- The pack marks a qualified definition at extraction (its `raw` is already `qualified_function` for a function; a qualified static data member definition, `int C::n = 0;`, gets `qualified_variable`), and `edges.rs`, `cpp_probe` and `compare.py` read that instead of looking for `::`. The edge word of a symbol is the last `::` segment of its name.
- `compare.py` skips "a qualified function" by looking for `::` in the name today; it must use the probe's marker, or every namespaced function would be skipped and the accuracy figures would collapse.
- A section heading and an anchor in a spec carry the chain (`## `leveldb::DB``); the Rust pack already has `::` in its names, so `section_anchor` is known to cope, but part 0 checks it on a C++ id.
- About 158 assertions in the C++ tests name an id (131 in `src/cpp/mod.rs`); they change with the rule, mostly by adding the chain.

**The ordering constraint.** On `fmt` (9.4 % of declarations) and `json` (7.8 %), ids lack the outermost namespace until step 3 teaches the pack that `FMT_BEGIN_NAMESPACE` opens `namespace fmt {`. That rule will change those ids again. So **no C++ spec may be generated on a repository whose namespaces come from macros until step 3's namespace-macro rule is built**; `abseil` and `leveldb` are not affected (the first opens an inline namespace, the second uses plain `namespace leveldb {`), and every measurement here is unaffected (the accuracy check keys on the last segment). `docs/cpp-pack-plan.md` §5 is updated to say so.

### 4.1 Where pairing runs

A new `StackPack` hook with a default that does nothing, called by `RepoIndex::rebuild` before `Graph::build`:

```rust
fn pair_definitions(&self, files: &mut [FileExtraction], file_imports: &HashMap<String, FileImports>) {}
```

The C++ pack implements it in a new `src/cpp/pairing.rs`. It needs the include graph (`cpp::resolve::IncludeGraph`, built from `file_imports`) and every file's symbols, which is why it cannot run at extraction (one file at a time) or in `resolve_imports` (it gets the graph read-only, after it is built). `graph.rs` does not change its logic and names no pack (layering kept); the index already calls pack hooks. The per-file cache is untouched: pairing works on the clones `rebuild` makes, so a re-extraction of one file simply re-pairs.

### 4.2 The pairing rule

For each file `f` in path order, each top-level symbol `d` of `f` with a body:

1. **A qualified name `A::b`** (or `A::B::c`), written inside a chain `N` (the namespaces around the definition): C++ looks `A` up from the innermost enclosing namespace outward, so try `N + A`, then `N` minus its last segment `+ A`, and so on down to `A` alone. At the first level where a container with that id (`<g>::<level>::A`) exists among the files `g` in `f` and `reach(f)`, take it: exactly one, pair `d` with it; several at that level, leave `d` where it is and count it. No level has one: go to rule 3. (A definition written `void leveldb::DBImpl::Get()` outside the namespace is found at the last level, by the full qualification.)
2. **An unqualified function** (raw `function`, no `declaration` marker) in a file that is not the prototype's: the candidates are the symbols named `b` with the `declaration` marker in `reach(f)`, looked up in `f`'s own chain `N` and outward exactly as in rule 1 (`<g>::<level>::b`). Exactly one at the first level that has any: pair. Otherwise leave and count.
3. **A qualified name with no class** (`ns::f`): apply rule 2 with the qualification read as a namespace path (`absl::base_internal::SpinLockWake` looks up `<g>::absl::base_internal::SpinLockWake`; decision P3).
4. Never paired: an `operator` at namespace scope (rule 2 handles a free `operator==` like any function only if it has a prototype), a definition whose qualifier names a `std::` entity (rule 1 finds no class, rule 3 finds no prototype: it stays), a test (`raw == "test"`).

Overloads need no parameter matching: the same-id fold of step 2 already made one symbol of every overload in a file, so a pairing moves the whole group.

### 4.3 What a pairing does

| | Value |
|---|---|
| The definition `d` | Removed from `f`'s symbols (its children, if any, with it; a definition has none unless it is an out-of-class nested class, which is left unpaired and counted) |
| The target | The member of the class that `d` defines, found by name among the class's children (`db_impl.h::leveldb::DBImpl::Get`); the class itself when no member matches (decision P2). For rule 2 and 3, the prototype |
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
| **N1** | **Do namespaces appear in ids?** | **Decided by the owner, 2026-11-15: yes**, the chain of enclosing named namespaces (section 4.0). Reverses D3 | The measurement of section 3: 62, 24 and 45 ids merged declarations of different namespaces; 16 of 17 ambiguous pairings settled; cheapest before any C++ spec exists. Alternatives not taken: a chain only when ids collide (an id would change when another declaration is added, orphaning its spec); keeping D3 and splitting on collision (the same instability, and 16 pairings stay ambiguous) |
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
| 0 | **Namespaces in ids** (section 4.0, N1). The extractor tracks the chain of named namespaces (inline ones transparent, anonymous omitted) and puts it in every top-level id and in its members' ids; a qualified definition is marked (`qualified_function`, new `qualified_variable`); `edges.rs`, `cpp_probe` and `compare.py` read the marker and the last segment instead of looking for `::`. `FORMAT_VERSION` bump. The existing id assertions change with the rule | Tests first: each row of the chain table in section 4.0 (nested and `a::b` forms, inline, anonymous, a class member, an out-of-class definition written inside and outside its namespace, two same-named declarations in different namespaces stay two symbols, the same name in `#if` branches of one namespace still folds); `edges.rs` never targets a qualified definition and does target a namespaced class by its last segment; a heading and an anchor for `leveldb::DB`. On the four repositories: **no two symbols of one file share an id across different namespaces**, the count of ids still shared only by an anonymous-namespace difference is recorded, and `utility/cpp_accuracy/run.sh` equals the figures of 2026-11-15 exactly (classes, functions, recall, precision). `graph_stats` identical on the seven other-pack repositories |
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
| No two symbols of one file share an id, on the four repositories | None, including across namespaces (part 0: the 62, 24 and 45 groups of section 3 become separate symbols) |
| Ambiguous pairings, on the four repositories | 16 of 17 settled by the namespace (measured in section 3); the remainder listed |
| The end-to-end tests of part C | Pass |

## 8. Cost

| Measure | Method | Kill criterion |
|---|---|---|
| Pairing time per rebuild | Time it on `abseil` | Over one second: memoise per file and re-measure |
| Graph size | `.codeowl/graph` on `abseil`, before and after | Should shrink slightly (definitions fold into their classes); a growth over 10 % is investigated |
| Largest paired class text | `DBImpl` on `leveldb`, `Cord` or `Status` on `abseil` | Informs P5 |

## 9. Risks

- **A wrong pairing puts a body in the wrong class's spec.** With the chain in the id (N1) the lookup follows C++'s own order (innermost namespace outward), and C++ requires the class to be declared before an out-of-class definition, so the real class is reachable from the defining file; a unique match is wrong only when an include failed to resolve (the `.inc` gap) or a `using` declaration brings the class in under another namespace. (An earlier draft of this section blamed the missing namespace; the measurement of section 3 showed the cost of no namespace was missed pairings and merged symbols, not wrong pairings.) Mitigation: two candidates at one level stay unpaired; check by hand a sample of 30 cross-file pairings on `abseil` and record the result.
- **Ids of `fmt` and `json` change again at step 3.** Their outer namespace is opened by a macro the pack cannot yet read (section 4.0). Hence the rule: no C++ spec is generated on such a repository before step 3's namespace-macro rule. Anyone who generates earlier gets specs keyed by ids that will change; the coverage report should say so for a C++ repository (to build with step 8's user-facing counts).
- **A qualified name no longer shows as `::` alone.** Every consumer that read `::` as "qualified" must read the marker (part 0). A missed consumer fails visibly in the accuracy check (counts collapse) or in `edges.rs` (a qualified definition becomes a target), which is why part 0's acceptance includes the accuracy figures.
- **More dependencies for a class.** Scoping now covers the bodies, so a class spec goes stale when something a body uses changes. Correct, and the price of describing behaviour, but it raises regeneration volume on implementation-heavy classes.
- **A tool contract change.** `get_source` gains a field and labelled blocks; the tool description must say so.
- **`get_symbol` on the old `.cc` id fails after pairing.** No C++ spec exists yet, so nothing is orphaned; agents reach the body through the class or `search_code`.
- **The `.inc` gap from step 2 stays**: a body defined in an `.inc` file is not read, so it cannot pair.

## 10. To look at later

- A symbol-level ladder for a class whose outline proves too thin (P5).
- A pointer from a bodies-only `.cc` to the header's spec (P6).
- How many ids an anonymous-namespace difference alone would still merge (part 0 counts it); give the anonymous namespace a segment in the id only if the count is not negligible.
- A `using` declaration or `using namespace` that makes a class visible under another namespace: pairing does not read them.

## 11. Progress

2026-11-15: the plan, with the core traced and the pairing rule and the namespace question measured on the four repositories. N1 decided by the owner. Nothing built.
