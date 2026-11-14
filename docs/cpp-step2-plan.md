# Plan: C++ step 2, includes and edges

> **Status: a plan awaiting the owner's approval (2026-11-14; revised the same day after a review, see section 10). No code is written.** Step 2 of `docs/cpp-pack-plan.md`, written in detail because the one-line version there left eight questions open. Grounded in a read of `index.rs` (`rebuild`, `apply_changes`), `graph.rs` (`ResolvedImport`), `spec/staleness.rs`, `spec/source_text.rs::scoped_symbol_deps`, `mcp.rs` (`get_callers`, `get_callees`) and `java/mod.rs` (`same_package_edges`, the nearest precedent). Everything marked *proposed* is the owner's to decide.

## 1. What this step delivers

After it, a C++ graph has dependency edges. Today it has symbols and none between files, so a header change makes nothing stale and `get_callers` finds nothing. With edges: editing a class's public interface in a header makes the spec of every symbol that names it stale, and `get_callers` on that class lists the files that use it, including the source file that implements it.

It does **not** make C++ safe to generate specs for: pairing and shared ids (step 2b) come first.

## 2. How the rest of CodeOwl consumes an edge (what the design must fit)

All four facts are from the code, not assumed:

| Fact | Where | Consequence for C++ |
|---|---|---|
| An edge is `ResolvedImport { from_file, specifier, imported_name, target: Option<SymbolId> }` | `graph.rs` | The unit is file to **symbol**. Narrowing fits it exactly; a file-to-file include does not |
| A symbol's `deps_hash` covers the targets of the edges **from its file** whose `imported_name` appears as an identifier in **that symbol's own text** (`try_dependency_pairs`: `contains_identifier(symbol_text, imp.imported_name)`) | `spec/staleness.rs` | `imported_name` must be the word as written in the code. Per-symbol precision comes free |
| A file spec's `deps_hash` covers **every** edge from the file (`try_file_dependency_pairs`) | `spec/staleness.rs` | A file goes stale when any name it uses moves. Narrowing keeps that to the names it actually uses; an edge per include would make it stale on any change to a header |
| `get_callers` is `imp.target == Some(id)`; `get_callees` lists every edge of the file | `mcp.rs` | One edge per (file, target symbol) gives correct callers. A `None` target is shown as unresolved |
| `RepoIndex::rebuild` re-runs the pack's `resolve_imports` over **all** files on every change | `index.rs` | No special watcher rule: adding, removing or editing a header re-resolves every include. Resolution cost is paid per edit, so it must be fast (section 8) |

**Transitive includes need no cascade logic.** An edge goes straight from the using file to the symbol that defines the name, wherever in the include chain that is. A changes only when the defining symbol's `interface_hash` moves; B, the header in between, plays no part. This settles the question the earlier plan left open ("does editing C stale A through B?"): it does, if and only if A names a symbol of C.

## 3. Design

### 3.0 First: one symbol per id within a file

An edge points at "the symbol with this id". C++ gives several declarations one id (overloads, a prototype and its definition, both branches of an `#if`, a template and its specialisation), and `Graph::build` keeps only the last of them, so an edge would point at that one alone: a signature change to any earlier declaration would make no user stale, and the spec would look current. Measured on 2026-11-14, top-level names declared more than once in a file: `leveldb` 6 of 987 (0.6 %), `abseil` 1,257 of 12,604 (10 %), `fmt` 532 of 3,744 (14 %), `json` 1,112 of 4,860 (23 %).

So the same-file half of `docs/cpp-pack-plan.md` section 4.7 moves here from step 2b and is built before any edge: `extract_file` ends with a fold, the move `typescript::fold_same_id` already makes. The merged symbol takes its kind, signature and main span from the strongest declaration, keeps the other line ranges as `extra_spans`, and hashes every declaration into both its hashes, so an edit to any one of them moves the interface hash the edge reads. No core change is needed (the span list exists). The cross-file half (a prototype in a header, its body in a source file) stays in step 2b.

### 3.1 Extraction: `extract_imports`

`CppStack::extract_imports` (today: empty) returns, per file:

- **Includes.** One `ImportRef { specifier: <path as written, with its quotes or angle brackets>, imported_name: "*" }` for each `#include "x"` or `#include <x>`, read from the original text (preprocessor lines are tokens already, `macros.rs::lex`). Found anywhere in the file, including inside `#if`: every configuration is read. Not read: a computed include (`#include MACRO_NAME`), and `__has_include`.
- **The words the file uses.** A new field `identifiers: Vec<String>` on `FileImports` (shared struct, `#[serde(default)]`, empty for every other pack): the distinct identifier-like words of the file outside comments and string literals, **including those in preprocessor lines**, sorted. This is what the accuracy check's stand-in used (a regex over the text after stripping comments and strings), so the numbers stay comparable.

`imported_name: "*"` marks an include (a file, not a name), the convention the Java pack already uses for `import a.b.*;`. The specifier keeps its delimiters because the two forms resolve differently (section 3.2). No edge is ever emitted with `*` as its name.

### 3.2 Resolution: `resolve_imports`

For every file `f`, in sorted order (deterministic output):

1. **Resolve each include to a repo file.** Zero or several suffix matches resolve to nothing.

   | Form | Tried in order |
   |---|---|
   | `"path"` | The including file's directory; the repository root; `include/`; `src/`; a **unique** path-suffix match (`ends_with("/" + path)`) among all files |
   | `<path>` | The repository root; `include/`; `src/`; a unique path-suffix match **only when the path has a directory in it** |

   A library may include its own headers with angle brackets (nlohmann/json does, all 343 times), so `<…>` is resolved, but more strictly: a compiler does not look beside the including file for it, and a bare name is almost always a system header. Measured: with one rule for both forms, `#include <time.h>` in `abseil` resolved to `absl/time/time.h` four times.
2. **Close over includes.** `reach(f)` is every file reachable through the resolved includes, excluding `f`. Computed once per file with a memo; cycles are normal and terminate.
3. **Narrow.** For each file `g` in `reach(f)` and each **top-level symbol** `s` of `g`, emit an edge if `s`'s name is in `f.identifiers`, or (for an enum) one of its enumerators is. Edge:

   | Field | Value |
   |---|---|
   | `from_file` | `f` |
   | `specifier` | The repo-relative path of `g`, not the include as written (a transitive edge has no include in `f` to quote) |
   | `imported_name` | The word as written in `f`: the symbol's name, or the enumerator |
   | `target` | `s` |

   "Top-level symbol" means a symbol with no parent, **except** an out-of-class method definition (`DB::Open`), which belongs to its class and is not a target. Macros (D5), prototypes (D4) and typedefs are targets.
4. **No self edges**, and one edge per (file, target, word). One edge per *target* would be tidier but wrong: a symbol's dependencies are the edges whose word appears in its own text, so a symbol that uses only `kWrite` would lose its link to the enum if the file's single edge carried `kRead`.

No edge is emitted for an unresolved include: the graph holds only what resolved. How many did not is reported by the accuracy check (section 6) and, proposed, by `codeowl extract` (open question 5).

### 3.3 What is deliberately left for later

| Left out | Why | Where it goes |
|---|---|---|
| Precision rules (nearest header wins, skip names the file declares, qualify common names) | Measured, one at a time, against the baseline; each kept only if precision rises without recall falling | Step 4 |
| C++20 `import m;` edges | Needs a map from module name to the file holding `export module m;`, and that line is blanked today. No measured repository uses modules | A later step, when one does |
| `compile_commands.json` | None of the four measurement repositories has one (it comes from a configured build), so it could only be tested on a fixture. Not building for a repository that does not exist yet | When a repository needs it |
| Linking each include of a name to the "right" header when several define it | Linking to all candidates is the measured method; narrowing it is precision | Step 4 |

## 4. What changes in the code

| File | Change |
|---|---|
| `src/imports.rs` | `FileImports.identifiers: Vec<String>` (`#[serde(default)]`) |
| `src/cpp/mod.rs` | The same-file fold at the end of `extract_file` (section 3.0), with its tests |
| `src/cpp/includes.rs` (new) | Include extraction, the identifier scan, path resolution, closure, narrowing, with their unit tests |
| `src/cpp/macros.rs` | Expose the existing lexer to `includes.rs` (it already skips comments and strings and yields directives whole) |
| `src/stack.rs` | `CppStack::extract_imports` and `resolve_imports` call into `cpp::includes`; the doc comment that says "includes are not read" is rewritten |
| `src/graph.rs` | `FORMAT_VERSION` 24 to 25, with its note (the shape of `FileImports` and what the C++ pack produces both change) |
| `examples/cpp_probe.rs`, `utility/cpp_accuracy/compare.py` | Compare the pack's own edges (section 6) |
| Docs | `ARCHITECTURE.md` (the C++ pack), `GLOSSARY.md` (include closure), `setup/USAGE.md` if a user-visible note is added |

Every other pack is untouched: `identifiers` is empty for them, and `graph_stats` on the nine repositories of the other four packs must stay identical (the control used before).

## 5. Decisions the owner needs to make

| # | Question | Proposed | Alternative |
|---|---|---|---|
| S2-1 | Where is the "words the file uses" set computed? | At extraction, cached in the index (`FileImports.identifiers`) | At resolution, reading each file from disk every rebuild, as `java/mod.rs::same_package_edges` does. No new field and no cache growth, but every edit re-reads and re-scans the whole repository |
| S2-2 | Should an edge's `specifier` be the header's path or the include as written? | The header's path (a transitive edge has no include to quote) | The include text where there is one |
| S2-3 | `compile_commands.json` | Not in this step (section 3.3) | Read it when present, with a fixture-only test |
| S2-4 | C++20 `import` | Not in this step | Add it, with a module-name map |
| S2-5 | Should users see how many includes did not resolve? | Yes, as a line in `codeowl extract`'s stderr note and in `get_spec_coverage`, built in step 8 with the docs | Only in the accuracy check |
| S2-6 | Is the wider cache acceptable? | Yes: about 150 words per file, measured on the `abseil` sample before it is built (section 8) | Option S2-1's alternative |

S2-1 is the one with a real trade-off. My reasoning for the proposal: `rebuild` runs on every watcher event, so the per-edit cost matters, and tokenizing 900 files each time is the cost the cache avoids. Against it: a larger `.codeowl/index`, and a shape change that every pack's cache sees (one new empty field).

## 6. Build sequence

Each part: tests first, a negative control where it applies, `utility/check.sh`, `/code-review`.

| Part | Content | Done when |
|---|---|---|
| 0 | **The same-file fold** (section 3.0) | Tests for each kind of shared id (overloads, prototype plus definition, `#if` branches, a specialisation, a class beside a function of the same name): one symbol, the other ranges in `extra_spans`, and an edit to *any* declaration moves the interface hash. On the four repositories no two symbols of one file share an id (the probe counts them). The accuracy check's class and function numbers do not fall |
| A | `includes.rs`: include extraction and the identifier scan, with `FileImports.identifiers` | Tests: both forms, `# include`, inside `#if`, a computed include ignored, comments and strings not scanned, preprocessor words scanned |
| B | Path resolution and closure | Tests for each lookup rule in order and for each form; a unique and an ambiguous suffix match; a missing file; `<time.h>` beside a repo file `x/time.h` stays unresolved while `"time.h"` and `<x/time.h>` resolve; a diamond; a cycle |
| C | Narrowing and the edges; `CppStack` wired; `FORMAT_VERSION` bump | Tests on a small repository built through `RepoIndex`: the edge fields of section 3.2; a name in a comment is no edge; an enumerator links its enum; a transitive edge reaches the defining header; no self edge; an out-of-class definition is not a target. Negative control: with the narrowing step removed the edge tests fail |
| D | The staleness and callers behaviour end to end | A test in the pattern of `tests/java_spec.rs`: editing a public member's signature in a header changes the `deps_hash` of a user symbol that names the class and of its file; editing an unrelated symbol of the header changes neither; `get_callers` on the class lists the including file and the implementing source file |
| E | The measurement uses the pack's own edges | `cpp_probe` prints the graph's edges; `compare.py` reports them beside the stand-in |

## 7. Acceptance

| Check | Required |
|---|---|
| `utility/check.sh` | Passes |
| `graph_stats` on nine repositories of the other four packs, before and after | Identical |
| `utility/cpp_accuracy/run.sh`: the pack's own edges against the compiler | Within one percentage point of the stand-in's recall and precision on all four repositories (K11: 97.1 / 89.8 / 92.7 / 89.9 % recall, 98.8 / 69.0 / 32.3 / 53.1 % precision). A larger difference is investigated and explained, not accepted |
| The end-to-end staleness and callers tests (part D) | Pass |
| Symbols of one file sharing an id, on the four repositories | None |
| Rebuild time | Section 8 |

Step 2 is **not** judged by the section 7 targets of the main plan: the hard repositories miss the precision target by a wide margin, and raising it is step 4.

## 8. Cost, measured before and after

| Measure | Method | Kill criterion |
|---|---|---|
| Resolution time per rebuild | Time `RepoIndex::rebuild` on `abseil` (880 files) and `json` (442) with a build of the example, before and after | If resolution adds more than one second on `abseil`, memoise `reach` across files and re-measure before going on |
| Cache growth | Size of `.codeowl/index` on `abseil`, before and after | If it more than doubles, do not cache the words: scan each file at resolution instead, as `java/mod.rs::same_package_edges` does (decision S2-1's alternative). Extraction sees one file at a time, so it cannot filter the words by other files' names |
| Coverage time | Time `get_spec_coverage`'s computation on `abseil` before and after. It scans the whole edge list once per symbol, and C++ will have many more edges per file than the other packs | If it grows past a second, index the edges by file once per call (a change in `spec/`, measured on the other packs too) |
| The words per file | Measured 2026-11-14: median 30 to 105 per file, 90th percentile 190 to 570, a few files in the thousands (largest 5,671) | Informs S2-6; the cache-growth row decides |

## 9. Risks

- **Linking to every header that defines a name.** A common name defined in many headers links a file to all of them; that is the measured precision gap, left to step 4. Staleness over-reports (never under-reports), so a false edge costs a regeneration, not a missed change.
- **Header-only libraries.** The closure of `fmt/format.h` is nearly the whole library, so most files link to most names they use. This is what the 32 to 53 % precision measures.
- **Names read from text, not from the grammar.** A local variable that happens to share a name with a header symbol links to it. Step 4's rules address it; step 2 accepts it.
- **A missed edge is the dangerous kind.** A name reached only through a macro expansion, or an include path the rules cannot resolve, produces no edge, and a change to its target does not make the user stale. Measured as recall (89.8 to 97.1 %); the compiler layer (steps 6 and 7) is the answer for what remains.
- **A macro's body is not its interface.** A macro's interface hash covers `#define NAME(params)` only, so changing what it expands to leaves its users current, although the expansion is compiled into them. The other packs strip values the same way, but for a macro the hole is larger. Left as it is here; decided with the macro table (step 3).
- **Fan-in is inflated.** `file_fan_in` counts edges, and this step emits one per word, so a file that uses ten enumerators of one enum adds ten to that header's count, which feeds generation priority. Not changed here (it is shared code and the other packs' order would move); see section 10.
- **The cache shape changes.** Every repository rebuilds its cache once after the `FORMAT_VERSION` bump.

## 10. To look at again later

Recorded so they are not rediscovered. None blocks this step.

| What | Why it is open | When |
|---|---|---|
| Edge precision on template-heavy code (32 to 69 %) | This step builds the measured method as it is; the rules that should raise it are untested | Step 4. If rules cannot reach the 80 % target, the compiler layer becomes the recommended setup for such repositories |
| Whether a macro's body belongs in its interface hash | See risks | Step 3, with the macro table |
| Fan-in counted per edge, not per file | See risks. Fixing it changes priority order for every pack | Step 8, when C++ generation order is first looked at |
| Real repositories other than `leveldb` cannot be indexed by `codeowl` itself | `detect` refuses a repository with two stacks; the accuracy check reads files directly, so this step can be measured, but its end-to-end behaviour can be tried only on `leveldb` | Step 2c (`docs/multi-language-plan.md`) |
| The other half of shared ids and pairing (a prototype in a header, its body in a source file) | Needs a span in a second file, a core change | Step 2b |
| `compile_commands.json`, C++20 `import` edges | No measured repository needs either | When one does |
| Unresolved includes are visible only in the accuracy check | A missed edge is the dangerous kind, and a user cannot see it yet | Step 8 at the latest; earlier if the count on a real repository is not near zero |
| `abseil` edge precision differs from experiment 8 (58.7 to 69.0 %) | Cause not isolated; the first scripts are gone | When step 4 studies precision |
| All four measurement repositories are libraries | An application, generated code (`moc`, protobuf) and heavy conditional compilation are unmeasured | Before C++ is called supported |

## 11. Review record (2026-11-14)

A review of the first version against the code and three measurements changed it in these places: the same-file fold moved in from step 2b (section 3.0; shared ids affect 10 to 23 % of top-level names on the hard repositories, and edges to them would have been silently incomplete); angle-bracket includes resolve by stricter rules (a system header was taken for a repository file); the cache-growth fallback was one that extraction cannot perform, and is now the scan-at-resolution alternative; coverage time is measured as well as rebuild time; the include marker is `*`, as in the Java pack; one edge per target was considered for enumerators and rejected (section 3.2). Unchanged: the edge model, no cascade for transitive includes, no watcher rule.

## 12. Progress

**Part 0** (the same-file fold): merged as #133. The accuracy check equals the K11 baseline exactly.

**Part A** (`src/cpp/includes.rs`; 2026-11-14): `extract(source)` returns the includes (path with delimiters, `imported_name: "*"`, each once, any branch of an `#if`, `include_next` too) and `FileImports.identifiers` (distinct, sorted, comments and string literals excluded, preprocessor lines included). It is not wired into `CppStack` yet: the comment there is right that reporting includes before they resolve would inflate the unresolved-import count, so the wiring and the `FORMAT_VERSION` bump for it come with part C. Measured with a throwaway probe on the four repositories:

| Repository | Files | Words per file: median / 90th percentile / max | Serialised words next to serialised symbols |
|---|---|---|---|
| `abseil` | 880 | 67 / 202 / 1,063 | 1.10 MB next to 11.9 MB |
| `json` | 628 | 26 / 179 / 3,871 | 0.59 MB next to 7.8 MB |
| `fmt` | 73 | 105 / 428 / 2,428 | 0.23 MB next to 4.2 MB |
| `leveldb` | 132 | 59 / 180 / 496 | 0.13 MB next to 1.5 MB |

The words are about 9 to 15 % of the symbols' size, so the cache-growth kill criterion (more than double) is far away (S2-6 holds). Against the accuracy check's regex stand-in the new scan finds no word the stand-in lacks, and lacks only fragments of numbers and literals (`x80`, `ull`, `e20f`, the `L` of `L'x'`): never a name. Part E will show whether the edge numbers move.

**Part A, after review.** `/code-review` found real defects, all fixed: the words of an `#include` path (`vector`, `h`) and the directive keyword (`include`, `define`, `if`) were counted as words, so a repository symbol with such a name would have had an edge from every file; a digit separator (`1'000`) or an apostrophe in a directive hid the rest of it; and a block comment running on from a directive line leaked its words, which was a lexer defect in `macros.rs` (the directive now ends where the comment ends; `FORMAT_VERSION` 26; the accuracy check is unchanged). Declined: `#import` (Objective-C; no measured repository uses it) and a `//` comment continued by a backslash, which swallows the next line in C++ too. Cross-check: the includes `extract` finds equal those of the accuracy probe's independent line scanner on all four repositories (8,794 / 3,063 / 618 / 772 distinct file and path pairs; no difference either way). The word counts above were taken before these fixes and are a little high.

