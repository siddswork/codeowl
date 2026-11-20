# C++ step 2b: one spec per class, across files

Step 2b of `docs/cpp-pack-plan.md` (§4.6 pairing, §4.7 shared ids across files). Written 2026-11-15 after tracing the core, before any code. **N1 (namespaces in ids) was decided by the owner on 2026-11-15 and reverses D3 of the pack plan; it is built first (part 0).** P1 to P8 (section 5) were approved as recommended by the owner on 2026-11-15.

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

**As built (part B), which is simpler than the rules above.** C++ requires an out-of-class definition to be written in a namespace that *encloses* its declaration, with the qualification supplying the rest, so the definition's id after its file (`leveldb::DBImpl::Get`, whether written inside `namespace leveldb` or out in full) is exactly its declaration's id after its file. The levels lookup of rules 1 and 2 is therefore not needed: a definition pairs with the one symbol of that id, in a file its own file reaches, of the right kind. For a qualified definition that is a member or a namespace function (a member may already have an inline body in the header for one configuration, as abseil's `CycleClock::Now` does) or, for a static data member, a field; for a plain free function with a body it is a bare prototype only (a second body is an overload, not its prototype). Several candidates: left and counted `ambiguous`. None: a qualified definition goes to the class that declares no such member (`to_class`, P2) or stays and is counted `unmatched`. A definition that is not exported (`static`, an anonymous namespace) is never paired. The one case this misses is a redundant qualification (`absl::Status::X` written inside `namespace absl`, id `absl::absl::Status::X`); see section 10.

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
| A | **The core.** The new field on `ExtractedSymbol` and `Symbol`, carried by `Graph::build`; per-file spans and text in `source_text.rs`; the per-file dependency scope (one helper for staleness, `Depends on` and the task); `get_source` per P4. No `FORMAT_VERSION` bump (part B's) | Tests on a hand-built graph with a symbol whose span is in a second file (no C++ needed): its text has both blocks with the file line; its dependency scope takes the second file's edges against the second file's text only; `get_source` returns both and flags a changed second file. `graph_stats` identical on the seven other-pack repositories |
| B | **The pairing pass.** The `pair_definitions` hook, called in `rebuild`; `cpp/pairing.rs` with rules 1 to 4 and section 4.3; `FORMAT_VERSION` 30 (with the note part A's section 11 gives) | A test for each row of the pack plan's §4.6 table that pairing touches (bodies in the header: no pairing; some bodies in a `.cc`; bodies below the class in one file; a `.cc` with private classes and functions; a free function with a prototype; a `static` helper; a class implemented across two `.cc` files), plus: two reachable classes of one name stay unpaired; a namespace-qualified function pairs to its prototype; a `std::` specialisation stays; a static data member pairs to the class |
| C | **End to end and measurement.** Rewritten 2026-11-20 after checking what parts A and B already built: section 12 | Section 12's tests pass; its measurements recorded in section 11 |

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
- **A free function's overload with no prototype is paired with the same-named symbol** (found by the part B review, not measured). Ids are by name, not by signature, so `int f(double) { … }` defined in a `.cc` whose header declares only `int f(int);` has the same id as that prototype and its body is moved under it, where the spec for `f` then describes an overload the header never declared. Within one file the same-id fold already treats overloads as one symbol (section 4.7: one spec describes them together), so this extends that stance across files rather than breaking it; and a free function that has *some* prototype in a reachable header is mostly an implementation of it. Checking the parameter text would need a normalisation that default arguments and parameter names defeat. Revisit if a repository shows a spec describing the wrong overload.
- **Redundant qualification: built** (section 11, "After the qualification fix"). What remains of the pairing gaps are extraction's: a parse error that loses a header's namespaces, an annotation macro glued into a name.
- **Known wrong output, kept on cost:** an anonymous-namespace symbol and a global one of the same name in one file share an id and fold (5 groups in about 16,000 top-level symbols: `AbslParseFlag`, `AbslUnparseFlag`, `VLogLevel`, `Vector128`, a `json` test `operator==`). The merged symbol is then marked exported although one half is file-local. Giving the anonymous namespace a segment in the id would fix it, and would lengthen the id of every symbol in an anonymous namespace (most of a typical `.cc`) for these five. Decided against at 5 groups; revisit if a repository measured later has more.
- `adopt-moved-specs` matches a moved symbol by the last `::` segment of its id (its own `short_name`), unlike the heading. For C++ that tolerates a class moving between namespaces, which is probably what is wanted, and it groups `big::FromHost` and `little::FromHost` as one name. Look at it when C++ generation starts and a restructuring needs adopting.

## 11. Progress

2026-11-15: the plan, with the core traced and the pairing rule and the namespace question measured on the four repositories. N1 decided by the owner; P1 to P8 approved as recommended.

**Part 0, namespaces in ids (built 2026-11-15, `FORMAT_VERSION` 29).** The chain rules of section 4.0 as written, in `cpp/mod.rs` (`Scope.chain`, set by `namespace_definition`, used by `id()`), `qualified_variable` beside `qualified_function`, `edges.rs` reading the marker and the last segment, `cpp_probe` and `compare.py` reading the marker. 13 new tests in `cpp/mod.rs` (red first), 4 in `tests/cpp_edges.rs`, one in `tests/cpp_spec_sections.rs`, one in `spec/layout.rs`; the old id assertions changed with the rule (a `standards.rs` expectation of two symbols for a namespaced prototype and its qualified definition is now one, which is the right reading).

Three things the work found that the plan had not:

- **A spec section is found by its heading, and the heading was the last `::` segment.** With namespaces in ids, `big::FromHost` and `little::FromHost` in one file would both read `FromHost`, and the second section's prose would have been read from the first's block, silently. The heading now comes from a pack hook, `StackPack::section_name`, whose default is the last `::` segment (what every other pack's stored specs already have) and which the C++ pack overrides to keep the namespaces; `stack::section_name(id)` finds the pack by the id's file extension, because a stored spec is parsed without a graph. A test through the real `submit` loop failed before the change and passes after. Anchors drop the colons (`bigfromhost`), checked. **The first version changed `short_name` for every pack, and the review caught it:** a Rust trait impl is a top-level symbol whose id has `::` in its type path (9 in CodeOwl, 6 in `bat`, 68 in `ripgrep`), and CodeOwl's own `docs/specs/src/mcp.rs.md` has sections headed by the last segment; the change would have stopped them being found. Pinned now by a test, and `migrate-to-okf .` on CodeOwl reports 0 specs to change.
- **The same-id fold made a member that is defined below its class a second top-level symbol** (the survivor was the definition, with no parent, while the class still listed it as a child). The defect was already on `master` for un-namespaced code (`class A { int f(); }; int A::f() {…}`); the chain makes the case more common (a definition written `shapes::Circle::area` outside `namespace shapes` now shares the member's id). The fold keeps the member's parent and kind.
- **A prototype in `namespace absl { }` and a definition written `absl::f()` fold into one symbol, which the fold left marked `qualified_function`,** so no edge could ever point at the real function. When one declaration is qualified and the other is not, the plain one shows it is the namespace's own function, and the marker is dropped.

Measured, `leveldb` / `abseil` / `fmt` / `json`:

| | Before | After |
|---|---|---|
| Top-level ids spanning different namespaces (same-id groups) | 0 / 62 / 24 / 45 | **0 / 7 / 0 / 1** |
| `utility/cpp_accuracy/run.sh`: the pack's classes, functions, edges, recall, precision | the figures of 2026-11-15 | **identical** (753 / 814 / 1,046 / 1,403 edges; recall 97.1 / 89.8 / 92.7 / 97.9 %; precision 98.8 / 73.2 / 49.5 / 69.6 %) |
| `graph_stats` on the seven other-pack repositories | | byte-identical to `master` |

The 8 groups left are not collisions of unrelated entities. Three are a prototype in a namespace folded with its qualified definition (`absl::SleepFor`, `SpinLockDelay`, `SpinLockWake`: one entity, correctly one symbol). Five differ only by an **anonymous namespace** (`AbslParseFlag`, `AbslUnparseFlag`, `Vector128` and `VLogLevel` on `abseil`, `operator==` in a `json` test): about 5 of 16,000 top-level declarations, so the anonymous namespace stays omitted from the id, as section 4.0 said to decide on this count. The stand-in's figures for `fmt` and `json` differ from the 2026-11-12 baseline because of the earlier directive-comment fix, not this part; the figures the targets judge are the pack's own, and they are unchanged.

**Part A, the core (built 2026-11-15, no `FORMAT_VERSION` bump: see below).** `ExtractedSymbol` and `Symbol` gain `other_file_spans: Vec<(String, [usize; 2])>` (P1), empty by default and left out of the serialised cache when empty; `Graph::build` carries it. Everything that reads a symbol's text now reads it file by file:

| Reader | Change |
|---|---|
| `source_text.rs` | `other_file_blocks` (the symbol's and its members' spans in other files, one entry per file in path order, merged by the rule the home spans use), `symbol_file_texts` (the text file by file, home first, unlabelled), `symbol_span_text` (the home text, then each block of another file under `// from <file>, lines a-b`), `symbol_scoped_deps` (one helper) |
| Staleness (`try_dependency_pairs`), the rendered `Depends on` list, the generation task's dependency list | All three take the edges **of each file against that file's part of the symbol**; one helper, so they cannot disagree |
| `get_source` (P4) | The labelled blocks follow the home text; a new `elsewhere: [{file, lines}]`, absent when empty; `graph_in_sync` is false when any of the files changed; `file` and `lines` stay the home file's |
| The task's size check, the container outline | Read `symbol_span_text`, so they include the other files' text with no change |

Tests (red first): a hand-built fixture with no C++ in it (`spec/test_support.rs::paired_graph`: a class in `a.h` whose members have bodies in `a.cc`, and two **decoy edges that resolve to other symbols**, so counting one is visible; my first version had the decoys resolve to the same targets as the real edges and passed against the old code, which is how I knew it was too weak). 11 new tests: the text and its label, a container gathering its members' spans, the dependency scope, the staleness pairs, the `Depends on` lines, the generation task, `get_source` with `elsewhere` and the `graph_in_sync` flip after an edit to the other file, an ordinary symbol with no `elsewhere` key in the JSON, and a cache round trip with a mutation check (without `skip_serializing_if` it fails).

Controls: `graph_stats` byte-identical to `master` on the seven other-pack repositories; **the status of all 2,401 specs on `commons-lang`, `quarkus-quickstarts`, `quarkus-super-heroes` and `talentTrail` identical to `master`** (so the per-file scope changes nothing for a one-file symbol, including the dependency hashes already stored in those specs); the accuracy check unchanged. Nothing fills the field yet: part B does.

**No bump in part A, one in part B.** The field is additive (`#[serde(default)]`), left out of the serialised cache when empty, and nothing fills it, so a cache written before it gives exactly the same results; the bump rule exists to stop a stale cache serving results a change would alter, and this change alters none. Part B bumps to 30, with this note in the constant's comment: *`ExtractedSymbol` and `Symbol` gain `other_file_spans`, the parts of a symbol's source in another file (read by the generation task, `get_source`, the size check and the dependency scope), and the C++ pack pairs an out-of-class definition with its class.*

**After the review of part A** (fixed, each with a test that failed first): a container's share of its file counts only the text in that file (the other file's text was added to the numerator and not the denominator, so a class with a large `.cc` would have read as bigger than its header); a span the pairing step records in the symbol's own file is read once as the symbol's own text, not twice; `get_source` answers when one of the other files is gone (a flagged gap and `graph_in_sync: false`, not a failed call) and applies `context_lines` to the other files' parts, clamped to each file's length; `try_dependency_pairs` gives no answer for a file id that is not the symbol's file. Declined: re-reading files per symbol. Only spec-bearing symbols (a file's top-level classes and functions) are asked, never their members, so a class costs one read of each of its files, as a symbol cost one read of its file before; the coverage-time row of section 8 measures it in part C.

**Part B, the pairing pass (built 2026-11-15, `FORMAT_VERSION` 30).** `StackPack::pair_definitions` (a no-op default; the C++ pack's is `cpp::pairing::pair`), called by `RepoIndex::rebuild` on its copies of the per-file extractions before `Graph::build`, so a pairing is redone from the cache on every rebuild and an edit to either file re-pairs. A pairing removes the definition from its file, gives the target its lines in `other_file_spans` (with the file), drops the target's `declaration` marker and turns a `prototype` into a `function`, takes the definition's comment when the target has none, and folds the definition's hash into the target's and into every class around it (sorted, so the order bodies are written in moves nothing); interface hashes are untouched, which is what keeps a body edit from staling a user of the class. 24 unit tests in `cpp/pairing.rs` (one for each row of the pack plan's section 4.6 that pairing touches, and the cases below), 2 through the real index (`tests/cpp_pairing.rs`) and one through `get_source`. Mutation checks: dropping the exported guard fails the static-helper test, dropping the ancestor hash fold fails three hash tests, removing the call from `rebuild` fails all three index-level tests. The part D test that pinned the known limit (an implementing file does not learn a private member changed) is flipped: the class now goes stale.

What the first measurement taught, each a test that failed first:

| Finding | Fix |
|---|---|
| A member defined inline in the header for one configuration and in a `.cc` for another has a body in each file; my first candidate rule wanted a bare declaration, so 87 definitions on `abseil` went to their *class* | A qualified definition's candidates are any member or namespace function of that id |
| Loosening that for plain free functions made another body a "prototype" (22 ambiguous on `fmt`) | A plain function with a body pairs with a bare prototype only |
| `btree<P>::operator=` kept `<P>` in its id, so it never matched `btree::operator=` | `plain_name` strips the class's template arguments before an `operator` |
| My fix for that cut `is_contiguous<…operator[]…>` at an `operator` *inside* the template arguments and made the garbage `is_contiguous[](size_t()))>>` (caught as a class lost on `fmt`, 332 to 331, by diffing the probe against `master`) | The `operator` keyword is looked for only outside template arguments and on identifier boundaries (which also fixes any name that merely contains the letters) |

Measured, `leveldb` / `abseil` / `fmt` / `json`, by `cpp_probe`'s `P` line:

| Definitions in a file other than their declaration's | `leveldb` | `abseil` | `fmt` | `json` |
|---|---|---|---|---|
| Paired, qualified (a member or a namespace function) | 221 | 594 | 276 | 0 |
| Paired, a free function with its prototype in a header | 123 | 456 | 111 | 11 |
| Moved into their class (no member of that name declared) | 0 | 8 | 0 | 0 |
| Ambiguous (several candidates), left | 0 | 5 | 0 | 0 |
| Unmatched (no declaration reachable), left | 0 | 43 | 2 | 8 |
| **Qualified definitions paired** | **100 %** | **92.6 %** | **99.3 %** | **0 of 8** |

Why the rest are left, from reading them: `json`'s 8 and a few on `abseil` are definitions of `std::` entities, which are not in the repository (correct); about 20 on `abseil` are an annotation macro glued into the name (`absl_nonnull`, `absl_nullable`), and 8 are classes the extractor misses (`LogMessageFatal` and friends): both are extraction gaps that step 3 (the macro table) and the K11 class list already own; 4 are the redundant qualification above; the rest are odd names (a conversion operator named `operator `, `friend struct` read as a function). **Checked, not assumed:** the pack's accuracy figures. `leveldb`, `abseil` and `json` are identical to the last run; `fmt` edge precision rises 49.5 to 51.7 % with recall unchanged (1,046 to 1,001 edges), and diffing the probe's edges against `master` shows why: 57 removed edges are duplicates (a file reached both a prototype and its definition under one word, which are one symbol now) and 18 are self edges (`core.h` declares `assert_fail`, a definition in `format-inl.h` is paired into it, and `core.h` does not depend on its own symbol); none is added and none of the reference's edges is lost. Classes and functions equal the baseline. `graph_stats` on the seven other-pack repositories and the status of all 2,401 stored specs on four are identical to `master`.

**After the review of part B** (each fixed with a test that failed first unless stated): `apply` now reports what it did and not what was planned, never removes a definition whose target is missing, and never moves a body into a symbol that is itself being moved (the body would have gone with it); the counts in the `P` line and the tables above are of moves made. `apply` builds one id map per file and removes definitions at the end, and the include closure is worked out only for a file that has a definition to pair (`pair` takes 12 ms on `leveldb`, 100 ms on `abseil`, 82 ms on `fmt`, 32 ms on `json`, against a kill criterion of one second, measured before this change and the counts unchanged after). `plain_name` and the `operator` search ignore an arrow and anything inside parentheses when matching `<` and `>`, so `X<decltype(a->b), decltype(c.operator[](0))>` is `X` (the first version of the search cut such a name into garbage, the same defect as `is_contiguous` found earlier). `pair` documents that it is given fresh copies. **Refuted by a test:** the review said the class would not depend on what a body in the `.cc` names (`db.cc` calls `Checksum`, which only `db.cc` includes); part A's per-file dependency scope already gives the class that dependency, pinned now by a task test. **Recorded, not built:** an overload with no prototype is paired with the same-named symbol (section 10).

**After the qualification fix (2026-11-15, `FORMAT_VERSION` 31).** A qualified definition now carries a marker `chain:N` (how many leading segments of its id are the namespaces around it), and `pair` tries the ids innermost first: the written name behind the first `N`, `N - 1`, … down to 0 of the enclosing namespaces, the first with a declaration deciding, as C++ lookup does (`absl::Status::X` written inside `namespace absl` is `absl::Status::X`, not `absl::absl::Status::X`). Three tests written red first (the marker; the repeated namespace; an inner one repeated, `b::C::f` inside `a::b`), one that already passed and pins the rule that the innermost namespace holding the name wins and is not ambiguous, and a mutation check (trying only the full id fails exactly the two new pairing tests). The pack's accuracy figures are unchanged.

| | `leveldb` | `abseil` | `fmt` | `json` |
|---|---|---|---|---|
| Paired, qualified | 221 | 606 (594) | 277 (276) | 0 |
| Moved into their class | 0 | 8 | 0 | 0 |
| Ambiguous | 0 | 5 | 0 | 0 |
| Unmatched | 0 | 31 (43) | 1 (2) | 8 |
| **Qualified definitions paired** | **100 %** | **94.5 %** (92.6) | **99.6 %** (99.3) | 0 of 8 |

(The figures in brackets are before this fix.) It gained 12 on `abseil` and 1 on `fmt`, more than the 4 I had counted, so I listed every new pairing instead of trusting the count. On `abseil`, **3 are the redundancy** (`Status::MakeNonOkStatusWithOkCode`, `Cord::FindImpl`, `Cord::Find`; of the 4 recorded above, the fourth, `absl::strings_internal::absl::chars_format`, was never a definition: it is a symbol of the *header* `charconv_parse.h`, a parameter type in `ParseFloat(…, absl::chars_format format_flags)` that the extractor misread as a function, an extraction misparse that stays unmatched) and **9 are not**: the 8 `log_internal::LogMessageFatal::…` constructors and destructors, and `Streamable::Print`. Their headers lost their namespaces to a parse error (`log_message.h` has 94 parse-error places: `LogMessage` is `absl::log_internal::LogMessage` but the classes after the break are `absl::LogMessageFatal`, because the recovery visits what is left of a broken namespace block with the outer scope; `bind.h` has 5 and its `Streamable` has no namespace at all). The definition names `LogMessageFatal` from inside `absl::log_internal`, lookup goes outward, and the shorter id matches the class whose id is wrong. The pairing is right, by the coincidence of one wrong id meeting the shorter prefix; it could as well be wrong where two classes of one name exist at different levels and a header is broken. The one on `fmt` (`detail::native_formatter::format` into `core.h`'s `native_formatter::format`; `core.h` has 1 parse-error place) I did not examine. The defect is the parse-error recovery (K11's "files partly read": 43 of 160 in scope on `abseil`), not the pairing; recorded here because it makes the 94.5 % a little less solid than the exact 92.6 % was.

**After the review of the qualification fix** (each with a test that failed first unless stated). The lookup decided across all levels for a declaration before it looked at any class, so a declaration in an *outer* namespace beat an *inner* class that lacked the member, which C++ lookup never does (it stops at the innermost name): an unmatched definition could become a wrong pairing. `pair` now decides level by level, innermost first: a declaration of the member at that level, else the class at that level, and an outer level is never consulted once an inner one has the name. `PairingReport` gains `shortened`, the pairings found by a name shorter than the definition's own id, which is where correctness rests on lookup and not on an exact id; the probe's `P` line prints it: **`abseil` 12, `fmt` 1, `leveldb` 0, `json` 0**, which is the 3 + 9 above and the one unexamined, now a number that can be re-checked. The `chain:N` marker is stripped from every symbol at the end of the pass, so it never reaches the graph, where markers are labels people read. Pins that already passed: a definition written from the global namespace (`::absl::Status::Make`), a template operator in a namespace (`btree<P>::operator=`), and two declarations at the innermost level being ambiguous and not falling outward. Declined: storing the split as a typed field (a marker is internal, now stripped, and avoids a field on every symbol of every pack for one pack's pass); the two cases the review raised about a leading `::` and about segment counts after `plain_name`, which the pins show are handled.

**The unexamined pairing, investigated and closed (2026-11-15, `FORMAT_VERSION` 32).** The one new `fmt` pairing (`native_formatter::format` into `core.h`) was correct and rested on a wrong id: `core.h` has `native_formatter` inside `namespace detail`, and the extractor gave it no `detail`. Measured against the namespaces the raw braces give, **every wrong id in all three repositories is in a file with parse errors, and none in any file without**: `leveldb` 0 of 1,269 top-level symbols, `abseil` 311 of 14,596 (2.1 %), `fmt` 50 of 4,051 (1.2 %), `json` 571 of 4,536 (12.6 %). That is why the pairing, and the 94.5 %, were only the visible edge of it.

**What I first told the owner was wrong, and the data corrected it.** I said the cause was `decltype(…)` in template arguments and `unsigned(size) << shift`, constructs the grammar cannot read. A fixture built from those constructs kept `detail`. Reducing the real `core.h` (whole declarations at a time, with the braces as the oracle for "this is in `detail`", and a first attempt that reduced a line at a time and produced incoherent code that proved nothing) and then removing one line at a time found two causes in our own **blanking**, not in the grammar, and each was confirmed on the real file by editing it:

| Cause | What happened | Fix |
|---|---|---|
| **A macro before a constructor with `noexcept` before its initialiser list**: `FMT_CONSTEXPR buffer(grow_fun grow, size_t sz) noexcept : size_(sz) {…}` | The constructor rule looked for the `:` directly after the parameter list's `)`, found `noexcept`, and left the macro, which the grammar read as a return type; the initialiser list then could not parse, and the recovery dropped `namespace detail` from every declaration after | Skip `noexcept`, `noexcept(…)`, `throw(…)` and `[[…]]` before looking for the `:` |
| **Macros standing for `try` and `catch`**: `FMT_TRY { flush(); } FMT_CATCH(...) {}` in a destructor | Not blanked, so the grammar failed, and **the whole class was lost** (not only its namespace) along with the namespace of what followed | `statement_block_macro`: a macro-looking name that starts a statement and is directly followed by `{` is blanked; with arguments (`FMT_CATCH(...) {`, which looks like the head of a test, `TEST_F(A, B) {`) only after the `}` of a block that a macro or `try` opened. The idiom is in all three repositories (`ABSL_INTERNAL_TRY`, `FMT_TRY`, `JSON_TRY`) |

The `if (FMT_MSC_VERSION != 0)` line a first reduction pointed at was an artifact of the reduction: removing it from the real file changes nothing.

Effect, `leveldb` / `abseil` / `fmt` / `json`:

| | Before | After |
|---|---|---|
| Ids lacking their namespaces | 0 / 311 / 50 / 571 | **0 / 268 / 2 / 540** |
| Shortened pairings | 0 / 12 / 1 / 0 | 0 / 12 / **0** / 0 |
| Classes found | 133 / 399 / 332 / 355 | 133 / 399 / **334** / **356** |
| Functions found | 897 / 2,130 / 1,281 / 2,132 | 897 / 2,130 / **1,294** / **2,136** |
| Edge recall | 97.1 / 89.8 / 92.7 / 97.9 % | 97.1 / 89.8 / **93.7** / **98.0 %** |
| Edge precision | 98.8 / 73.2 / 51.7 / 69.6 % | 98.8 / 73.2 / **57.7** / **70.9 %** |
| Files partly read | 7 / 43 / 15 / 31 | 7 / 43 / **12** / 31 |

`fmt` now meets its recall (93 %) and function (97 %) targets; its classes (93.0 %) and precision (57.7 %) do not yet. Four tests written red first (the constructor shape in the rule's own tests and as a namespace-loss fixture; the try/catch macros, with gtest's `TEST_F(A, B) {` as the case that must stay; the namespace-loss fixture for it).

**Update, 2026-11-16.** The bisect found two more blanking gaps, both fixed: explicit instantiations (`extern template …;`, `template class X<…>;`: three or more followed by a directive leave the namespaces unclosed; `raw_hash_set.h`, `status.h`) and an annotation macro after a parameter name (`T&& value MACRO)`). `abseil` 268 to 78. The 78 are in `layout_benchmark.cc` (24), `bind.h` (18), `resize_and_overwrite.h` (11), `log_message.h` (9), `span.h` (8), others (8); `json`'s 540 are not yet reduced.

**What this does not close.** 78 ids on `abseil` (268 before the two fixes above) and 540 on `json` still lack their namespaces, all in files with parse errors, so there are more causes of the same kind. Of `json`'s, 249 are in a vendored old copy of the library and 142 in `doctest.h`. Whether to read the namespace from the braces (a token reader was built and tested and is saved, unused) or to find the remaining blanking gaps one by one with the same reduction is the next decision; the evidence so far favours finding the gaps, since each also recovers classes and functions that braces cannot.

**After the review of the blanking fixes.** Fixed, with tests that failed first: a macro-named namespace or type before a `{` (`namespace DETAIL_NS {`) no longer makes a following `TEST_F(A, B) {` read as a `catch`, because the opener's macro must itself start a statement; a braced initialiser of macro-named types (`= { ROW_A{1} }`) is no statement; the `[[` scan is bounded at `;` and `{`; constructors with a `requires` clause or a function-try-block lose their macro too. Corrections to the record above: **five** tests were added (four written red first, one negative), and the `if (FMT_MSC_VERSION != 0)` line **is** needed in the minimal fixture `LOST_NAMESPACE` (without it the fixture keeps its namespace) although removing it from the real `core.h` changes nothing, because other statements there play its part. Known limit: a *nested* braced initialiser (`{ { ROW_A{1} } }`) reads as a block in a block and its macro is blanked; it still parses and changes no symbol.

**The namespace bisect, closed (2026-11-20).** After #144 and #145 the ids lacking their namespaces are `leveldb` 0, `abseil` 54, `fmt` 2, `json` 128 (all in the vendored `doctest.h`, where reading further than before also exposes 161 ids written `doctest::doctest::…`, from a `namespace doctest {` opened in both branches of an `#ifdef`). The owner stopped the hunt: the last rules cleared about 250 `abseil` ids and moved none of its K11 figures (which match by the last `::` segment and so cannot see a namespace) and two unmatched pairings, and every rule found by reducing a file of these four repositories risks fitting them rather than C++. From here a blanking rule is added only when it moves a K11 figure or the paired share, or fixes a file the dogfood reaches. The tools stay for that: `examples/cpp_ids.rs` and `utility/cpp_accuracy/reduce/`.

## 12. Part C in detail (2026-11-20)

Parts A and B already built most of what section 6 listed for part C, so this section lists only what is missing. **Already there, not repeated:** the private-member limit flipped (`tests/cpp_staleness.rs`); a body edit moves the class's source hash and not its interface hash (`tests/cpp_pairing.rs`); `get_source` on a class shows the body with `elsewhere`, and the old `.cc` id is gone (`mcp.rs` tests); the generation task for a class split over two files has both parts and both files' dependencies; `get_callers` on a class lists the including and the implementing file; `cpp_probe` pairs before building its graph; the paired share and the unpaired, explained (section 11).

### 12.1 Tests (red first where the behaviour is new; where it already holds, the test pins it and a mutation check shows it can fail)

| # | Test | Where | Why it is not covered yet |
|---|---|---|---|
| C1 | **A body edit through stored specs.** Submit specs for the class, the user symbol, the user file and the header file; edit only the body in the `.cc`. The class reads `changed:source`; the user symbol, the user file and the header's file summary read current (P8) | `tests/cpp_staleness.rs` | Today the body edit is checked on hashes only, never through `submit` and `symbol_changes`/`file_changes`, which is what a user sees |
| C2 | **A dependency only a body uses.** `crc.h` included by the `.cc` alone; change `Checksum`'s signature. The class reads `changed:dependencies`; the class's users do not move | `tests/cpp_staleness.rs` | The scoped dependency of a body is tested on a task's list, not on staleness end to end |
| C3 | **The watcher's path.** Through `RepoIndex::apply_changes`, not `open`: a body edit re-pairs (the class's source hash moves, `other_file_spans` keep the right lines); removing the definition from the `.cc` leaves the member with no `other_file_spans` and the `.cc` id still absent; deleting the `.cc` does the same | `tests/cpp_pairing.rs` | Every C++ test builds the index from scratch; the incremental rebuild is where a cache-of-clones mistake would show |
| C4 | **A body edit that adds a call the header does not know.** The `.cc` body starts calling a function from a header the `.cc` already includes: the class goes `changed:dependencies` | `tests/cpp_staleness.rs` | The dependency scope is taken per file; this pins the "`.cc` edges against the `.cc` text" half end to end |

### 12.2 Measurements (section 8's rows, still open)

| Measure | How | Kill criterion / use |
|---|---|---|
| Pairing time | Recorded already (12 / 100 / 82 / 32 ms) | Done |
| Graph size with and without pairing on `abseil` | `cpp_probe` serialises both graphs (`abseil` cannot be indexed by `codeowl` before step 2c, so not `.codeowl/graph`); say so in the record | Growth over 10 % is investigated |
| Largest paired class | Text size of `DBImpl` on `leveldb`, and of the largest on `abseil`, against the 4,000-byte outline threshold and the 8,000-byte cap | Informs P5 before the dogfood: if `DBImpl`'s task is outline-only, the dogfood shows whether that is enough |
| Coverage time on `leveldb` | `get_spec_coverage` (or its function) timed on `leveldb`, `master` before part A against now | The part A review deferred it here; over twice the old time is investigated |
| A hand check of 30 cross-file pairings on `abseil` | Read 30 pairings at random (fixed seed) from the probe; each correct or not | Section 9's mitigation, never done. One wrong pairing is investigated before the dogfood |
| `leveldb` end to end | `codeowl extract` on `leveldb`, then `get_spec_coverage`: the spec-bearing count, no bodies-only `.cc` listed, nothing panics | A smoke test of what the dogfood will drive |

### 12.3 Done when

C1 to C4 pass (red first or with a mutation check); the measurements are in section 11; `utility/check.sh`; `graph_stats` identical on the seven other-pack repositories (no production code is expected to change; if one does, the bump rule applies); `/code-review`. Then step 2b is finished and the next step is the `leveldb` dogfood (`docs/cpp-pack-plan.md` section 5, order of 2026-11-20).
