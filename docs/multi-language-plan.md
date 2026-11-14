# Plan: several languages in one repository (a pack per file)

> **Status: a plan awaiting the owner's approval (2026-11-11). No code is written.** Step 2c of `docs/cpp-pack-plan.md`, split out because it changes the core for every language. The owner chose the direction on 2026-11-11 ("option 3"): route each file to the pack that reads it. Everything marked *proposed* is the owner's to decide; everything marked *not traced* was not read in the code.

## 1. Why

`stack::detect` picks exactly one pack for a repository and refuses the repository when two packs each find a source file (ROADMAP design decision 6, written when the case was "hypothetical, don't over-build for it"). Measured on 2026-11-10:

| Repository | What `detect` says |
|---|---|
| `abseil-cpp` | Refused: Python (6 files), C++ (880 files) |
| `fmt` | Refused: Python (4), C++ (20) |
| `nlohmann/json` | Refused: Python (20), C++ (442) |
| `full-stack-fastapi-template` | Needs `serve …/backend`: about 82 % TypeScript, 18 % Python |
| a Rust crate with a bindgen `wrapper.h` | Now refused, because C++ reads every `.h` |

Three of the four C++ measurement repositories cannot be indexed at all. `DECISIONS.md` question 3 already named the problem ("secondary-language subtrees") and sketched a "primary pack plus reduced secondaries" model that was never built. This plan replaces the sketch.

## 2. The idea

**Ask each file which pack reads it, not the repository which pack it is.** A pack already claims files by extension (`StackPack::source_kind`), and no two registered packs claim the same extension, so the pack for a file follows from its path. Nothing new is stored per file. No "reduced mode" is needed: each pack runs in full on the files it claims.

A repository has a **set of active packs**: those that claim at least one file outside the non-identity trees (`tests`, `test`, `benches`, `src/test/`, `utility`, `scripts`, `tools`, `hack`). That keeps today's answer for a Rust crate with C++ fixtures under `tests/`.

## 3. What changes

The call sites below were found by search, not traced; each is a place that today assumes one pack per repository.

| Place | Today | After |
|---|---|---|
| `stack::detect` | Returns one pack, or an error | Returns the active packs, in registry order; an error only when there are none |
| `RepoIndex` (`index.rs`) | Holds one `Box<dyn StackPack>` and a `pack_name` | Holds the active packs; a file is extracted by `pack_for(path)` |
| Cache check (`RepoIndex::load`) | Rejects a cache built by a different pack name | Rejects a cache whose set of pack names differs |
| `Graph::pack_name` | One string | A set of names, persisted; a `FORMAT_VERSION` bump |
| `stack::for_graph`, `stack::file_role` | Recover the one pack from the graph | `for_file(graph, path)` picks the pack by the file's extension; the graph's set only has to contain it |
| `resolve_imports` | Called once with every file's imports | Called once per active pack, with only that pack's files |
| `resolve_flow_edge` | Called per edge on the one pack | Called on the pack that produced the edge |
| `strip_test_code`, `file_fingerprint`, `is_schema_symbol`, `generated_source_dirs` | The one pack | The pack of the file in question; generated directories are the union |
| Feature layer (`features.rs`: `for_graph(graph).feature_model()`) | One feature model per repository | **Open: see D-ML3** |

Symbol ids start with the file path, so ids from two packs cannot collide.

## 4. What this gives, and what it does not

**Gives:** `abseil`, `fmt` and `nlohmann/json` index as C++ with their scripts as Python; the FastAPI template indexes from its root with both halves in one graph; a Rust crate with a header documents the header; coverage, staleness and rollups work across the mixed tree because they already work on files.

**Does not give:** edges *between* languages (a React `fetch` reaching a FastAPI route, Python calling a C++ binary, a generated OpenAPI client). Those are flow edges, added one real case at a time later (`DECISIONS.md` question 3, first and fourth bullets). A mixed repository has one connected graph per language until then, and the report must say so.

## 5. Decisions the owner needs to make

| # | Question | Proposed | Alternative |
|---|---|---|---|
| D-ML1 | When is a pack "active"? | One claimed file outside the non-identity trees | A minimum share (for example 2 % of code files), so one stray `.h` in a Rust crate does not switch C++ on. Cleaner output; needs a threshold with no data behind it |
| D-ML2 | What does a mixed repository's output say? | `extract`, `serve` and coverage list the active packs and the file count of each, and say that links between languages are not resolved | Say nothing |
| D-ML3 | How does the feature layer work with two feature models (for example Next.js pages and FastAPI routes)? | **Not decided; needs a read of `features.rs` first.** Starting point: each pack's model enumerates its own entry points, features are listed together, and a feature never spans two packs until cross-language edges exist | Keep one feature model per repository: the first active pack that has one. Simple, and it silently drops the other half |
| D-ML4 | Where do mixed-repo specs live and what are rollups called? | Unchanged: specs mirror the tree. A directory with files of two languages is one rollup | A rollup per language per directory |
| D-ML5 | Do we keep the "refuse" behaviour for any case? | No, except zero active packs | Refuse when more than N packs are active |

D-ML3 is the real design question. The others are small.

## 6. Build sequence

Each step: its own branch and pull request, tests first, `utility/check.sh`, `/code-review` before asking for a merge.

| Step | Content | Done when |
|---|---|---|
| 0 | Read `features.rs`, `mcp.rs`, `index.rs` and the spec layer for every one-pack assumption; write the findings into this plan; settle D-ML3 | The table in §3 is complete and every row says "traced" |
| 1 | **Detection and the graph.** `detect` returns the active set; the graph stores the set; the cache check compares sets; `for_file`. Behaviour for a single-pack repository is unchanged | `graph_stats` identical on the nine single-language repositories (the control); new tests: two packs active, one active, none, tests-tree fixtures ignored; `FORMAT_VERSION` bumped |
| 2 | **Extraction and resolution per file.** Each file goes to its pack; `resolve_imports` and flow edges run per pack on that pack's files | A fixture repository with Python and C++ yields both packs' symbols and each pack's own edges; no edge crosses packs |
| 3 | **The rest of the pack hooks** (`strip_test_code`, `file_fingerprint`, `is_schema_symbol`, `generated_source_dirs`) by file | A test per hook with two packs |
| 4 | **The feature layer**, as D-ML3 decides | The FastAPI template indexed from its root lists the FastAPI features and does not drop the TypeScript half without saying so |
| 5 | **Report what was found.** The active packs and the cross-language caveat in `extract`, `serve` and `get_spec_coverage` (D-ML2) | The output names every active pack |
| 6 | **Real repositories.** `abseil`, `fmt`, `json`, the FastAPI template from its root, a Rust crate with a header | Each indexes; the C++ numbers of experiment 8 are reproduced on `abseil`, `fmt` and `json` |
| 7 | **Docs.** `ARCHITECTURE.md` (the layering section and the pack registry), `DECISIONS.md` (question 3, third bullet resolved; a new decision), `ROADMAP.md` design decision 6 marked superseded, `GLOSSARY.md`, `setup/USAGE.md`, the README | The docs say what the code does |

## 7. Risks

- **It is a core change.** Every language's graph passes through the new set logic. The control is the same as for the other core changes: `graph_stats` identical on the nine single-language repositories before and after, and a fresh `.codeowl/` on each.
- **Two packs, one feature layer.** D-ML3 is not understood yet. If it turns out to be large it becomes its own step and the first release can ship with "the first active pack's feature model" and a warning.
- **A new kind of noise.** A repository with a few vendored scripts gets specs for them. The existing `FileRole` rules (test, generated) are the filter; vendored code is classed per pack (`CppStack::classify` already treats `third_party/` as generated).
- **The cache.** A repository whose set of packs changes (a first `.py` file is added) rebuilds; a full rebuild takes under a second on the largest repository measured, so this is accepted.
- **Plain C.** Still unread. A C-only repository stays "no source files", and a C repository with Python would index only its headers as C++. Documented, not fixed here.

## 8. Out of scope

Edges between languages; a pack for every language in a repository (only the registered ones are read); per-directory pack overrides; reading build files (`CMakeLists.txt`, `pom.xml`) to decide anything.
