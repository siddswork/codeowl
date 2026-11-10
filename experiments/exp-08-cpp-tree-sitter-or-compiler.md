# Experiment 8: for C++, tree-sitter or a compiler?

Status (2026-11-10): **measured on four repositories; the owner accepted the layered recommendation the same day** (`DECISIONS.md` open question 1, for C++ only). The measurement was made with throwaway scripts outside the repository, before any pack code exists. Unlike experiments 5 to 7, the protocol was not written before the first result: the first probe ran, and the comparison was then built around it. The limits of the method are in "What this does not show".

## Question

`DECISIONS.md` open question 1 has asked since the start whether tree-sitter is precise enough, "especially for C++", or whether a compiler-grade source (clangd, libclang, a SCIP index) is needed. The owner wants C++ to be CodeOwl's strong language, the decision backed by data, and every standard from C++98 onward supported.

Three sub-questions:

1. How much of a C++ repository does tree-sitter read correctly, against what a compiler reads?
2. How well can dependencies between files and symbols be recovered without a compiler?
3. Which C++ standards does the grammar handle?

## Method

- **Tree-sitter side.** `tree-sitter-cpp` 0.23.4 with `tree-sitter` 0.27 (the version in `Cargo.toml`), driven by a small Rust probe that lists definitions, includes and parse-error nodes per file. Run twice: on the source as written, and on a copy with macros **blanked** (replaced by the same number of spaces, so offsets and line numbers do not move) by four syntactic rules:
  - (a) an all-capitals word between `class`/`struct`/`union` and the name;
  - (b) an all-capitals word with an underscore, with or without arguments, after a declarator's `)` or after a member's name;
  - (c) such a word directly in front of another identifier;
  - (d) a line that is nothing but such a word, with or without arguments.

  Preprocessor lines are never touched.
- **Compiler side (the reference).** libclang 18.1.1 through its Python package, installed in a scratch folder, with the system headers of g++ 13. Every file in scope is parsed as its own translation unit. The reference lists every class, enum and function *definition* in the repository's files, and every *edge*: a file uses a declaration whose top-level owner lives in another repository file (a call to `DB::Get` is an edge to the class `DB`), plus the link from an out-of-class definition to the header that declares it.
- **Dependency recovery without a compiler.** For each file, follow its `#include`s to repository files; make an edge to each top-level symbol of an included file whose name (or, for an enum, one of its enumerators) is written in the including file, comments and strings removed. Measured over direct includes and over the transitive closure.
- **Standards.** One hand-written file per standard with its characteristic syntax, checked with `g++ -fsyntax-only` where g++ 13 knows the standard.

| Repository | Commit | Why | Files measured |
|---|---|---|---|
| `leveldb` | `7ee830d` | Small, clean, Google style | 93 (tests, benchmarks and the Windows port left out) |
| `abseil-cpp` | `fce9e66` | Large, Google style, attribute macros everywhere | 160, an even sample of 904 non-test files |
| `fmt` | `e7e6e3f` | Templates and macros, a modern library | 20 (`include/`, `src/`) |
| `nlohmann/json` | `eac1b43` | Header-only, heavy templates | 81 (`include/`) |

## Results

### 1. What tree-sitter reads

| | `leveldb` | `abseil` | `fmt` | `json` |
|---|---|---|---|---|
| Files with a parse error, as written | 25 of 93 | 155 of 160 | 19 of 20 | 77 of 81 |
| Files with a parse error, blanked | 8 of 93 | 50 of 160 | 16 of 20 | 33 of 81 |
| Class definitions found, as written | 107 of 133 (80.5 %) | 364 of 401 (90.8 %) | 282 of 349 (80.8 %) | 339 of 369 (91.9 %) |
| Class definitions found, blanked | 133 of 133 (100 %) | 384 of 401 (95.8 %) | 300 of 349 (86.0 %) | 350 of 369 (94.9 %) |
| Function definitions found, blanked | 892 of 897 (99.4 %) | 2,093 of 2,199 (95.2 %) | 1,286 of 1,315 (97.8 %) | 2,173 of 2,223 (97.8 %) |
| Classes and functions tree-sitter reports that the compiler does not (blanked) | 96 | 430 | 181 | 358 |

- **As written, tree-sitter is not good enough on any of the four.** `class LEVELDB_EXPORT DB { … }` parses as a function named `DB`; one in five classes is lost on the cleanest repository.
- **Blanking recovers most of it, and all of it on clean code.** The rules are purely textual and need no knowledge of the repository.
- **What blanking cannot recover** is code the preprocessor writes or reshapes: `FMT_TYPE_CONSTANT(int, int_type);` declares a specialisation only the compiler sees; `FMT_ENABLE_IF(…)` sits inside a template parameter list; `if FMT_CONSTEXPR20 (…)`. `fmt` stays at 86 % of classes.
- **The extra definitions are not all errors.** They include code the compiler skipped in this one configuration (other platforms, disabled features) and members of classes that were misparsed. They were not separated.

### 2. Dependencies without a compiler

| | `leveldb` | `abseil` | `fmt` | `json` |
|---|---|---|---|---|
| Reference edges (file to top-level symbol) | 766 | 664 | 559 | 998 |
| File-level dependencies reached by direct includes | 66.8 % | 92.3 % | 52.8 % | 64.5 % |
| File-level dependencies reached by transitive includes | 100 % | 100 % | 100 % | 100 % |
| Edges found, direct includes: recall / precision | 75.5 % / 98.6 % | 87.8 % / 71.2 % | 41.0 % / 53.9 % | 62.1 % / 62.5 % |
| Edges found, transitive includes: recall / precision | 97.1 % / 98.8 % | 88.7 % / 58.0 % | 94.1 % / 42.3 % | 90.1 % / 52.8 % |

- **Direct includes are not enough.** A third to a half of real file dependencies arrive through another header. The first version of the plan proposed direct includes only; that is withdrawn.
- **Recall with transitive includes is 89 % to 97 %.** What stays missed on `leveldb` (22 edges): 18 are a class used without its name being written (a method called on a value whose type is inferred), 4 are a use that resolves to a forward declaration. The first kind is the true limit of matching by name.
- **Precision is 99 % on clean code and 42 % to 58 % on template-heavy code.** A false edge makes a spec stale when it need not be; it never hides a change. Two causes: common names (`get`, `data`, `begin`) match everywhere, and a header-only library's transitive closure is nearly the whole library. **The figure is understated by an unknown amount**: inside a template that is never instantiated the compiler does not resolve dependent names, so the reference itself is missing real edges there.
- **A library may include its own headers with angle brackets** (`json`: all 343 of its internal includes). Resolving only quoted includes found nothing.
- **One line of macro handling moved `json` from 40 % to 90 % recall** (rule (d): `NLOHMANN_JSON_NAMESPACE_BEGIN` alone on a line had hidden every namespace body). Macro handling decides the result more than anything else.

### 3. What the compiler costs

| | `leveldb` | `abseil` | `fmt` | `json` |
|---|---|---|---|---|
| Translation units with compile errors | 1 of 93 | 10 of 160 | 0 of 20 | 34 of 81 |
| Time, compiler reference | 46 s | 165 s | 27 s | 112 s |
| Time, tree-sitter probe (whole repository) | 0.14 s | 1.95 s | 0.42 s | 1.77 s |

- **It needs the build's knowledge.** `leveldb` compiled only after a generated header (`port/port_config.h`) was written by hand and two platform macros were defined. 34 of `json`'s 81 headers do not compile standalone. Without `compile_commands.json` the flags are guesses.
- **It sees one configuration.** The Windows port of `leveldb`, and every disabled `#ifdef` branch, do not exist for the compiler. Tree-sitter reads every branch.
- **It is slow and static.** 60 to 330 times slower in this setup (the Python binding inflates this; a native reader would be faster, by an unmeasured factor), and an index is as old as its last run, where the watcher re-parses a file in about a second.
- **It is heavy to ship.** `libclang.so` is 64 MB, against the single self-contained binary of `ARCHITECTURE.md` §9.
- **It is exact where it applies**, and it is the only way to see macro-generated declarations, uses of a type that is never named, and which override a virtual call can reach.

### 4. Standards

| Standard | Sample parses cleanly? | What fails |
|---|---|---|
| C++98 / 03 | Almost | A `typedef` of a pointer to a `const` member function (the alias is still found) |
| C++11 | Yes | |
| C++14 | Yes | |
| C++17 | Almost | A pack expansion in a `using` declaration, `using Ts::operator()...;` (the class is still found) |
| C++20, without modules | Yes | Concepts, `requires`, coroutines, `<=>`, designated initialisers all parse |
| C++20 modules | **No** | `export module`, `export` in front of a declaration: an exported function and block were lost, an exported class was misplaced |
| C++23 | Partly | Explicit object parameters (`this Self self`), `if consteval`, a `static` lambda. The enclosing symbols are still found |
| C++26 | Partly | Pack indexing, `= delete("reason")` (the class holding it was lost), contracts (the function was read as a prototype) |

There is no "C++25": the standards are 98, 03, 11, 14, 17, 20, 23 and 26. The samples are small and hand-written; they show which syntax fails, not how often it occurs in real code. g++ 13 could confirm validity only up to most of C++20.

## Reading

1. **Neither alone is the answer.** Tree-sitter as written loses a fifth of the classes. A compiler needs a working build, sees one configuration, and cannot keep up with a live session.
2. **Tree-sitter with macro handling is a sound base**: all classes and 97 % of dependencies on clean code, 86 % to 96 % of classes and about 90 % of dependencies on hard code, at no setup cost, in every configuration, in under two seconds.
3. **The gap that remains is the preprocessor's and the type system's**, which no syntax rule closes: declarations written by macros, types used without being named, virtual dispatch, and the low precision of name matching in template-heavy headers.
4. **A language server is the wrong shape in any case.** CodeOwl needs a stored index, not a live query protocol. The compiler-grade candidates are libclang read directly and a SCIP file from `scip-clang`.

## Recommendation (accepted by the owner, 2026-11-10)

**Both, in layers.** Tree-sitter with macro handling always runs and is the base of the graph. A compiler-grade index is an optional second layer, used when the repository has a `compile_commands.json`, that corrects edges and adds what only a compiler sees. CodeOwl reports which layer answered, and its measured accuracy against the reference becomes a KPI.

Kill criteria, to be applied when the pack exists and the measurement is repeated with it:

- If the base layer cannot reach 95 % of classes on `abseil` and `json`, the repository's own `#define`s must be read (a macro table) before C++ is called supported. (The owner has since decided to plan the macro table as a step in any case, after the blanking rules: `DECISIONS.md` question 28.)
- If the base layer's edge precision on template-heavy code cannot be raised above 80 %, staleness on such repositories is too noisy without the compiler layer, and the compiler layer moves from optional to recommended for them.
- If the compiler layer changes fewer than one edge in twenty on `leveldb` and `abseil`, it is not worth its setup cost for Google-style code and is built only for the hard cases.

## What this does not show

- **The probe is not the pack.** Its notion of "top-level" is crude, and its blanking rules were written in an afternoon. The real pack should do better; these numbers are a floor for tree-sitter, not a forecast.
- **The reference is one configuration, and is itself incomplete inside uninstantiated templates.** Precision on `fmt` and `json` is therefore understated.
- **Member-level edges were not measured**: which method calls which, overload resolution, virtual dispatch. Only file-to-top-level-symbol edges, the ones staleness uses.
- **`abseil` is a sample** of 160 files, and tests were left out of every repository.
- **No codebase with heavy conditional compilation or code generation** (a kernel, a game engine, Qt with `moc`) was measured.
- **`scip-clang` and clangd were not run**; neither is installed here, and installing them needs the owner.
- **The scripts are not in the repository.** They are in the session's scratch folder and are to be rebuilt on the real pack as a repeatable check (`docs/cpp-pack-plan.md`, step 5).
