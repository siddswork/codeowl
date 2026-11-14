# Plan: a C++ pack, built to be CodeOwl's strongest language

> **Status: decisions D0 to D7 agreed with the owner on 2026-11-10 and recorded in `DECISIONS.md` questions 1 and 28; D8 waits for step 6. No pack code is written.** The first version planned a pack like the other four. The owner then set three requirements: C++ is to be the strong suit, because it is the hardest language and the one every competitor is weakest on; the choice between tree-sitter and a compiler must rest on data; and every standard from C++98 onward is to be supported. The data is `experiments/exp-08-cpp-tree-sitter-or-compiler.md`; this plan is built on it. Everything marked *proposed* is the owner's to decide.

## 1. What "strong suit" means here

Competitors generate C++ documentation with an LLM reading files, and CodeWikiBench found all of them weaker on C and C++ (`NORTHSTAR.md` §3). CodeOwl can be stronger in five ways, each of which this plan makes a deliverable:

| # | Strength | Why competitors lack it |
|---|---|---|
| S1 | **It reads macro-heavy C++ correctly with no build.** | A plain parser loses one class in five (measured); an LLM reading the file guesses |
| S2 | **It documents every configuration.** Every `#ifdef` branch and every platform file is read | A compiler-based tool sees the one configuration it was built for |
| S3 | **It is exact when a build is available.** A compiler-grade layer corrects the graph | Wiki generators have no compiler in the loop |
| S4 | **Its accuracy on C++ is a published, repeatable number**, measured against a compiler | No competitor publishes one for any language |
| S5 | **It covers C++98 to C++26**, with a test per standard and stated gaps | Unstated elsewhere |

## 2. What the data settled

From experiment 8, on `leveldb`, `abseil`, `fmt` and `nlohmann/json`, against libclang:

| Finding | Number | Consequence for the plan |
|---|---|---|
| Tree-sitter on source as written loses classes | 80 % to 92 % of classes found | Macro handling is the core of the pack, not a detail |
| Four textual blanking rules recover most | 100 % of classes on clean code, 86 % to 96 % on hard code | Blanking is built first and measured on all four repositories |
| What is left needs the repository's own macro definitions | `fmt` stays at 86 % | A macro table is planned as step 3 and expected to be needed (D1) |
| Direct includes miss real dependencies | Only 53 % to 92 % of file dependencies reached; transitive includes reach 100 % | Includes are followed transitively. The first version's "direct only" is withdrawn |
| Matching by name recovers most edges | 89 % to 97 % recall | Good enough for staleness on its own |
| Matching by name over-links template-heavy code | Precision 99 % on clean code, 42 % to 58 % on hard code (understated; see the experiment) | Precision work is its own step (step 4), and the compiler layer exists for this |
| A library may use angle brackets for its own headers | All 343 internal includes in `json` | Both include forms are resolved against the repository |
| A compiler needs the build, sees one configuration, and is slow | 34 of 81 `json` headers fail standalone; 46 s against 0.14 s on `leveldb` | It cannot be the base layer |
| Modules and some C++23 / C++26 syntax fail in the grammar | See §6 | A syntax shim, tested per standard |

**Tree-sitter or LSP, answered:** neither alone. Decided by the owner on 2026-11-10 (decision D0): tree-sitter with macro handling is the base layer and always runs; a compiler-grade index is an optional second layer. A language server as such is the wrong shape (CodeOwl needs a stored index, not a live protocol); the second layer is libclang read directly or a SCIP file from `scip-clang`, chosen by a spike in step 6.

## 3. Decisions the owner needs to make

| # | Question | Proposed | Alternative |
|---|---|---|---|
| **D0** | Tree-sitter or a compiler for C++? | **Decided (Sidd, 2026-11-10): both, layered.** Base layer first; compiler layer after the base is measured. Recorded in `DECISIONS.md` question 1 | Compiler only (exact, but needs a build, one configuration, 64 MB library); tree-sitter only (no fix for the measured precision gap) |
| D1 | How are macros handled in the base layer? | **Agreed (2026-11-10): blanking rules first, in step 1; the macro table after, in step 3.** The owner expects the table to be needed, and the probe's numbers agree (`fmt` 86 %, `json` 94.9 % of classes against a 95 % target). Step 1's measurement on the real pack decides what the table must cover, not whether to plan for it | A hand-written macro list per repository; the macro table from the start (considered and set aside the same day: the blanking is needed in either case, and the table brings a core change and a watcher rule into the first step) |
| **D2** | Where is a class's spec written when its declaration and its method bodies are in different files? | **Agreed with the owner (2026-11-10): one spec per class.** It lives in the file that holds the `class X { … }` declaration and is written from the bodies wherever they are. Full rule and cases in §4.6 | Rejected: a spec per file (the class in the header, every `DBImpl::Get` its own section in the `.cc`). Cheaper to build, but the header's spec would be written from declarations alone, the `.cc` would fragment into one section per method, and it contradicts the rule that a method is not spec-bearing |
| D3 | Do namespaces appear in symbol ids? | **Agreed (2026-11-10):** no | Yes |
| D4 | Is a header prototype with no body a symbol? | **Agreed (2026-11-10):** yes | Definitions only |
| D5 | Are macros symbols? | **Agreed (2026-11-10):** yes, a top-level `#define` (not an include guard). The experiment found macros are real dependencies: 80 of `json`'s missed edges were to macros | Ignore macros |
| D6 | File extensions | **Agreed (2026-11-10):** `.cc`, `.cpp`, `.cxx`, `.h`, `.hpp`, `.hh`, `.ipp`, `.inl`, and module units `.cppm`, `.ixx` | Add plain C (`.c`) |
| D7 | What must the numbers be before C++ is called supported? | **Agreed for now (2026-11-10):** the targets in §7, to be revisited when the real pack is measured | Other thresholds |
| D8 | May a compiler be installed on this machine for step 6 (`clang`, `scip-clang`)? | Yes, when step 6 starts | Stop after the base layer |

## 4. Design

### 4.1 Base layer: extraction

`src/cpp/` with `mod.rs` (extraction), `macros.rs` (blanking, later the macro table), `includes.rs` (resolution and edges). `CppStack` implements `StackPack` and is one entry in `stack::registered_packs`.

- **Blanking before parsing.** A macro that breaks the grammar is replaced by spaces of the same length in a copy of the text, so every offset and line number is unchanged; signatures, spans and hashes come from the original text. The four rules of the experiment are the starting set. Preprocessor lines are never blanked.
- **Symbols.**

  | Source | Symbol |
  |---|---|
  | `class`, `struct`, `union` with a body | `Container`; methods and fields are its children |
  | `enum`, `enum class` | `Container`; its enumerators count as names that refer to it |
  | Function definition or prototype at namespace scope | `Callable` |
  | `Class::method` defined outside the class | Extracted where it is written, then folded into the class by pairing (D2, §4.6); not spec-bearing once paired |
  | `template<…>`, `requires …`, a concept | Part of the signature; a `concept` is a `Value` |
  | `typedef`, `using X = …`, namespace-scope variable | `Value` |
  | `#define` (D5) | `Value` or `Callable`, `raw: "macro"` |
  | `TEST`, `TEST_F`, `TEST_P` | `Callable` `file::Suite.Name` |
  | `namespace` | Not a symbol |

- **Exported**: not in an anonymous namespace, not `static` at namespace scope; a member is exported when `public` or `protected`.
- **Overloads** share an id and both fold into the parent's interface hash (the Java rule).
- **Every configuration is kept** (S2): both branches of an `#if` are extracted. A symbol defined in two branches is two definitions sharing an id, like an overload.
- **Honesty about what was not read.** The pack counts parse-error nodes per file. A file above a threshold is reported (in `extract`'s output and in coverage) as partially read, not silently accepted.

### 4.2 Base layer: includes and edges

- Both `"…"` and `<…>` are resolved against the repository: the including file's directory, the root, `include/` and `src/`, then a unique path-suffix match. `compile_commands.json`, when present, supplies the real include directories.
- **Transitive.** A file's reachable headers are the closure of its includes.
- **Narrowing.** An edge is made to a top-level symbol of a reachable header when its name, or one of its enumerators, is written in the file (comments and strings excluded).
- **Precision rules** (step 4, each kept only if it measurably helps): prefer the nearest header when a name is defined in several; do not match a name that the file itself declares; require a qualifying context for very common names (`get`, `data`, `size`), for example `ns::name` or a call with the owning type in scope; treat a header-only library's umbrella header as transparent.

### 4.3 The macro table (step 3, expected to be needed)

Extraction is per file today, so a pack cannot know what another file's macro means. The table is a first pass over the repository's `#define`s that classifies each macro by its replacement text: expands to nothing or to attributes (blank it); expands to keywords (`FMT_CONSTEXPR` to `constexpr`; blank it or substitute); opens or closes a namespace or a block (`FMT_BEGIN_NAMESPACE`; blank it and keep the nesting balanced); declares something (`FMT_TYPE_CONSTANT`; record the invocation as a symbol named by its arguments, marked macro-generated). The blanking mechanism of step 1 is reused: the table decides which macros to blank and what else to do with them, and the positional rules remain as the fallback for a macro the repository does not define (one from a system or third-party header).

Two costs make it its own step. It needs a pre-pass the `StackPack` trait does not have (`index.rs` calls `extract_symbols` one file at a time), so it is a core change. And it breaks an assumption the watcher relies on: today an edit to one file re-parses that file only, but an edit to a `#define` changes how every file using that macro parses. The starting rule is the simple one: if an edit changes the macro table, re-extract the whole repository (under two seconds on the 300,000-line `abseil` probe); narrowing that to the files that use the changed macro is done only if it is measured to matter.

It is skipped only if step 1's real pack already meets the class targets of §7 on all four repositories, which the probe's numbers make unlikely.

### 4.4 Second layer: the compiler index (steps 6 and 7)

When `compile_commands.json` exists and an index has been produced, CodeOwl loads it and:

- replaces name-matched edges with the compiler's for every file the index covers, and keeps the base layer's edges for files it does not cover (other platforms, disabled branches, files changed since the index was made);
- adds declarations only a compiler sees (macro-generated, implicit);
- records, for a virtual method, the overrides a call can reach, so `get_callers` on an interface method is complete.

The index is optional and CodeOwl never runs a compiler itself. Each answer says which layer produced it. The choice between libclang and SCIP, the staleness rule for an old index, and the cost to the single-binary property are the subject of the step 6 spike; this is also the first build of roadmap idea 3 (SCIP import), on the language that needs it most.

### 4.5 What C++ specs should say that other packs' specs do not

Filled in by CodeOwl from the graph, never by the LLM: for a class, its bases and (with the compiler layer) the classes that derive from it; for a virtual method, what it overrides; for a header, which source files implement it; for a symbol under `#if`, the condition it exists under. These are what a reader of a C++ codebase most often needs and cannot see in one file.

### 4.6 One spec per class: pairing a declaration with its bodies (D2)

**The rule.** A class's spec lives in the file that holds its `class X { … }` declaration, and covers its methods wherever their bodies are. Pairing across files happens only when a body is in a different file from its declaration. A forward declaration (`class X;`) is never the home.

| Case | What happens |
|---|---|
| The header holds the method bodies (inline methods, templates, header-only libraries) | No pairing. One section in the header's spec, as for a Java class |
| The header holds some bodies, a `.cc` the rest | One section in the header's spec, written from both files |
| Bodies defined below the class in the same file (`void Foo::bar() { … }`) | Folded into the class within that file, as the Rust pack folds an `impl` block into its type |
| No header: a `.cc` with several classes | Each class is a top-level symbol with its own section in that `.cc`'s spec. A class in an anonymous namespace still gets one (a private top-level symbol is spec-bearing) |
| Classes and standalone functions in one `.cc` | A section per class and a section per function, in that file's spec |
| A standalone function with a prototype in a header and its body in a `.cc` | Paired the same way: one section, in the header's spec, written from the body |
| A standalone function with no prototype elsewhere (a `static` helper) | A section in the `.cc`'s spec |
| A class implemented across several `.cc` files (`Env` in `env_posix.cc` and `env_windows.cc`) | One section; every body is an input, labelled with its file |

So a `.cc` file's own spec holds what is declared only there (private classes, helper functions) and its summary. A `.cc` that holds nothing but bodies for a header's class has no spec-bearing symbol of its own, and so no file spec, by the existing rule for a file with no top-level symbol.

**Hashes and staleness**, the same semantics as a class in the other packs:

- The class's own source hash covers its declaration and every paired body. A body edit in the `.cc` makes the class's spec stale, which is correct: the behaviour it describes changed.
- The class's interface hash covers the declaration only. A body edit does not move it, so files that merely use the class stay current.
- Editing the declaration in the header moves the interface hash and makes the users stale, one hop, as today.

**How pairing is found.** After includes are resolved (§4.2): a definition whose name is qualified (`DBImpl::Get`) is matched to a class named `DBImpl` declared in the same file or in a header that file reaches; a free function's definition is matched to a prototype of the same name and parameter list in a reachable header. No match, or more than one candidate class, leaves the definition unpaired: it stays a top-level symbol in its own file and is counted in a report, never silently dropped.

**What it changes in the core** (to be traced fully before step 2b; only partly read so far):

- A symbol must be able to own a span in another file. `Symbol` has `extra_spans` today, used by the Rust pack for spans in the same file; it needs a file beside each span.
- The source hash fold, `get_source`, the generation task's input and the watcher (an edit to the `.cc` must re-hash a symbol whose home is the header) must follow those spans.
- The spec layout (`spec/layout.rs`) must not count a paired definition as spec-bearing.
- This is a change every pack's cache sees, so it is a `FORMAT_VERSION` bump, and `graph_stats` on the other repositories must be unchanged apart from the new field.

### 4.7 Declarations that share an id (decided 2026-11-11, built in step 2b)

C++ gives several different declarations one id: overloads (`write(int)`, `write(double)`), a prototype and its definition in one file, both branches of an `#if`, and a template with its specialisation (`Box` and `Box<bool>`). `Graph::build` keeps one node per id (`by_id.insert`, last wins), so today the earlier declarations cannot be looked up and a member can be attached to the wrong parent. The extractor reads every one of them on purpose (S2), so the pack must not hand the graph duplicates.

**Decision: merge them into one symbol with several spans** (option C of the discussion). The pack ends `extract_file` with a fold step, the same move the TypeScript pack already makes in `typescript::fold_same_id` and the Rust pack makes for an `impl` block:

| Field of the merged symbol | Value |
|---|---|
| id, kind, signature, main span | From the strongest declaration (container before callable before value); the first on a tie |
| `extra_spans` | The line range of every other declaration |
| source hash and interface hash | A hash over every declaration's hash, so an edit to any one moves it |
| exported | True if any declaration is |
| children | The union |
| docstring | The first that exists |

Consequences, accepted:

- A spec describes all the overloads together. Editing one overload makes the whole spec stale, which is correct and costs one extra regeneration.
- `get_source` shows every declaration, because it already reads the main span and every extra span.
- Nothing is lost for callers: C++ has no call-level resolution yet, and the merged symbol is the unit a dependency edge points at.

**Same file first, then across files.** The fold within one file needs no core change (the span list already exists); it is the cheap half and can be built before the rest of 2b. Across files (a prototype in `db.h`, its body in `db.cc`) a span needs a file beside it, which is the core change of §4.6; the cross-file case uses this same merged shape, so the two are one mechanism.

**Open points, decided when step 2b is planned in detail:** how a class with dozens of bodies is offered to the writer without exceeding the task size (the file ladder and paging exist for files, not yet for one symbol's bodies); whether the spec of a public header (`include/leveldb/db.h`) should name the internal file that implements it; what a reader of the `.cc` on GitHub sees when that file has no spec of its own.

## 5. Build sequence

Each step is its own branch and pull request, tests first, `utility/check.sh` before every commit, `/code-review` before asking for a merge. Every step ends by re-running the measurement and recording the numbers.

| Step | Content | Done when |
|---|---|---|
| 0 | This plan and experiment 8 | Done 2026-11-10: D0 to D7 answered |
| 1 | **Extraction with blanking.** `tree-sitter-cpp`, `src/cpp/mod.rs`, `macros.rs`, `CppStack`, registration, parse-error reporting, a fixture test per standard (§6) | Unit tests for every row of §4.1 and every blanking rule, each with a negative control; `detect` and `graph_stats` unchanged on the existing test repositories; classes and functions measured on the four repositories |
| 2 | **Includes and edges.** Both include forms, transitive closure, narrowing | Unit tests per rule; edge recall and precision measured on the four repositories; a signature edit in a header makes its users stale |
| 2b | **Pairing and shared ids: one spec per class** (§4.6) **and one symbol per id** (§4.7). The same-file fold first (no core change); then the core change for a span in another file, the pairing pass, the layout rule. Planned in detail, with the core traced, before any code. **No generation is run on a C++ repository before this step** | A test for every row of the table in §4.6 and §4.7; no two symbols of one file share an id (checked on the four repositories); a body edit in a `.cc` makes the class's spec stale and no user of the class; the share of out-of-class definitions paired, with the unpaired ones explained; `graph_stats` unchanged on the other packs' repositories |
| 2c | **A pack per file: several languages in one repository.** Its own plan, `docs/multi-language-plan.md`, approved before any code. Needed before `abseil`, `fmt` and `nlohmann/json` can be indexed at all: each carries Python scripts, and `detect` refuses a repository with two stacks | Those three repositories index as C++ with their scripts as Python; `graph_stats` identical on the nine single-language repositories; the plan's own tests |
| 3 | **The macro table** (§4.3). Expected to be needed; skipped only if step 1 already meets the class targets on all four repositories. The pre-pass, the classification, and the watcher rule for an edited `#define` | Classes found meet §7 on all four repositories; an edit to a `#define` re-extracts the files it affects, with a test; `graph_stats` unchanged on the other packs' repositories |
| 4 | **Precision.** The rules of §4.2, one at a time, each kept only if precision rises without recall falling | Edge precision on `fmt`, `json`, `abseil` reported before and after each rule |
| 5 | **Mostly done 2026-11-12 (brought forward): the measurement becomes a repeatable check.** Built: `utility/cpp_accuracy/`, K11 in `NORTHSTAR.md` with its baseline. Open: the dependency edges are the method step 2 will use, not the pack's own edges (point `compare.py` at the real ones when step 2 lands), and the `abseil` edge precision differs from experiment 8 by a cause not isolated. Original text: The reference script and the comparison move into `utility/`, run against the real pack's output; `NORTHSTAR.md` gains a C++ accuracy KPI with its baseline (S4) | Anyone can re-run it; the numbers of experiment 8 are reproduced or the difference explained |
| 6 | **Spike: the compiler layer.** With a compiler installed (D8): libclang against `scip-clang` on `leveldb` and `abseil`; how many edges it changes; what it costs to load, to keep fresh and to ship | A written recommendation and the kill criterion applied: if it changes fewer than one edge in twenty on Google-style code, it is built only for the hard cases |
| 7 | **The compiler layer**, as the spike decides | Edges for covered files equal the reference; an index older than a file falls back to the base layer for that file, with a test |
| 8 | **Dogfood and docs.** `/codeowl-generate` on `leveldb`, then on part of `abseil`; the owner reads about ten specs from each. `ARCHITECTURE.md` §9 and open question 1, `GLOSSARY.md`, `REQUIREMENTS.md`, the roadmap's test-repo table, `setup/USAGE.md`, the README, `NORTHSTAR.md` | The owner judges the specs; D2 is confirmed or reopened; open question 1 is closed for C++ with the numbers |

Steps 1, 2, 2b, 2c and 8 alone give a working C++ pack. (2c is not C++ work as such, but three of the four measurement repositories cannot be indexed without it, so steps 3 to 5 cannot be measured before it.) Steps 3 to 7 are what make it the strong suit, and each has a measurement that can stop it.

`FORMAT_VERSION`: step 1 adds a pack and needs no bump if `graph_stats` is identical on the other repositories; every later step that changes what the C++ pack produces bumps it.

## 6. Standards: C++98 to C++26

There is no "C++25"; the standards are 98, 03, 11, 14, 17, 20, 23 and 26. Measured on hand-written samples (experiment 8):

| Standard | Grammar today | Plan |
|---|---|---|
| 98 / 03, 11, 14, 17 | Parses, with two rare failures that lose no symbol | A fixture test each; the two failures recorded as known |
| 20, without modules | Parses (concepts, `requires`, coroutines, `<=>`) | A fixture test |
| 20 modules | Fails: `export` loses or misplaces symbols | A shim: blank `export` in front of a declaration; read `export module`, `module` and `import` lines as the file's module identity and its imports. Module imports become edges like includes |
| 23 | Three constructs fail; the enclosing symbols survive | The same shim where a blanking fixes it (`this` in a parameter list, `static` after a lambda's `[]`); the rest recorded |
| 26 | Pack indexing, `= delete("reason")`, contracts fail; one class and one function body were lost | The shim for `= delete("…")` and contract clauses; the rest recorded |

The fixture files are tests in the repository, one per standard, each asserting the symbols extracted. A construct the pack cannot read is listed in `setup/USAGE.md`, not hidden. Following the grammar's releases is the long-term fix; the shim is what keeps CodeOwl ahead of them.

## 7. Targets (D7, agreed for now on 2026-11-10)

Measured against the compiler reference on the four repositories, tests excluded.

| Measure | Clean code (`leveldb`) | Hard code (`abseil`, `fmt`, `json`) | Experiment 8, for comparison |
|---|---|---|---|
| Classes found, base layer | 100 % | 95 % or more | 100 % / 86 % to 96 % |
| Function definitions found, base layer | 99 % or more | 97 % or more | 99.4 % / 95 % to 98 % |
| Edge recall, base layer | 97 % or more | 93 % or more | 97 % / 89 % to 94 % |
| Edge precision, base layer | 98 % or more | 80 % or more | 99 % / 42 % to 58 % |
| Edges, compiler layer, covered files | Equal to the reference | Equal to the reference | Not built |

The hard-code precision target is the one most likely to be missed; if it is, the compiler layer becomes the recommended setup for such repositories and the docs say so.

## 8. Risks

- **A blanking rule may blank something real**, such as an all-capitals type name. Each rule gets tests with such names, and the per-repository measurement catches a rule that costs more than it gains.
- **The precision reference is imperfect.** A compiler does not resolve names inside templates that are never instantiated, so a "false" edge may be real. Step 4 must check a sample by hand before optimising against the number.
- **The four repositories are all libraries.** No application, no code generator (`moc`, protobuf), no kernel-style conditional compilation. A fifth repository of that kind should be added before C++ is called done; none is chosen.
- **The compiler layer cuts against two design properties**: the single self-contained binary and the index that is always live. That is why it is optional, loaded and never run by CodeOwl, and decided by a spike.
- **Scope.** This is several times the work of the Java or Python pack. Steps 1, 2, 2b and 8 are the minimum useful cut; the rest can be stopped at any measurement.
- **Pairing changes the core, not only the pack.** A symbol that owns a span in a second file touches hashing, `get_source`, the watcher and the spec layout for every language. It is the riskiest step in the minimum cut, which is why it gets its own detailed plan and why `graph_stats` on the other repositories is its control.
- **A wrong pairing puts a body in the wrong class's spec.** Matching is by qualified name over reachable headers; two classes of the same name in different namespaces are the case to test, and an ambiguous match is left unpaired and reported.
- **One stack per repository holds only until step 2c.** Until then a repository that mixes C++ with another supported language outside `tests/`, `scripts/` or `tools/` is refused by `detect` (measured: `abseil`, `fmt` and `nlohmann/json` all are). Step 2c replaces the rule; see `docs/multi-language-plan.md`.

## 9. Out of scope

Plain C; a feature model (entry points such as `main`, gRPC services); a fingerprint rule narrower than the whole text; running a build or a compiler from CodeOwl; Objective-C++; CUDA and other dialects.
