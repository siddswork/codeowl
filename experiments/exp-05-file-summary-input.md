# exp-05 — What a file summary should be written from

**Status:** measured on Rust only (2026-10-03). Nothing is built. The non-Rust check and the paged-read check below are still to run.
**Feeds:** `DECISIONS.md#q22` (the decision this evidence supports), the file-task shape in `spec.rs` (`spec_task_to_response`, the `File` arm), the file-summary instructions in `setup/codeowl-generate.md` and its `.prompt.md` port, and the finer file-level fingerprint plan on branch `file-fingerprint`.
**Not a decision doc:** conclusions fold into `DECISIONS.md#q22` and `ARCHITECTURE.md` when built. Where this file and those disagree, they win.

---

## Why this experiment exists

A file's `## Summary` is written by the calling agent from whatever the file task hands it. Today that is the file's raw text, cut at `max_spec_task_bytes` (8,000 bytes, `MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT`) with a truncation marker. Measured on CodeOwl's own `src`: 23 of 35 files are over the cap, so most file summaries are written from the first 8 KB of a file that may be 30 times that size. The summary then claims to describe the whole file.

The same question blocks the finer file-level fingerprint (so that a comment-only edit stops staling hub file summaries): the fingerprint should cover exactly what the summary was written from, so the input has to be decided first.

The owner rejected raising the cap (it is a client-side limit CodeOwl cannot see past), and proposed a ladder: raw file if it fits, else the symbol specs already written, paged if still too big. This experiment tests that, and what the writer is told.

## Design

**Files** (chosen as the worst cases, plus one that fits):

| File | Size | Symbols | Role |
|---|---|---|---|
| `src/spec.rs` | 244,535 bytes (131,153 without tests) | 105 | worst: summaries alone are 28 KB |
| `src/mcp.rs` | 174,497 bytes (76,807 without tests) | 34 | second worst; its tools are methods folded into one `impl`, so symbol summaries are thin |
| `src/rust_crates.rs` | 5,768 bytes | 3 | fits the 8,000-byte window, with real logic |

**Arms.** One cold sub-agent per run, told to read exactly one input file and write one paragraph (150 to 250 words, jargon-free per `docs/specs/STYLE.md`, no milestone numbers):

| Arm | Input | Prompt |
|---|---|---|
| A | the file as it is today: raw text cut at 8,000 bytes plus the truncation marker | original |
| B | module doc, then each symbol's signature and `### Summary` (no raw code, no `### Behavior`) | original |
| D | the whole raw file (not shippable above the cap; an upper bound) | original |
| BP | the same input as B | new: adds the coverage instruction below |
| X | B plus each symbol's `### Behavior` text | original |
| XP | X input | new |

The new instruction, added to the task line: *"The reader will use it to decide where to look and what to trust, so: cover EVERY major part of the file, not only the first ones; state concrete specifics (named rules, limits, thresholds, flags, ordering) instead of general descriptions; and mention behaviours that would surprise a reader."* Length stayed at 150 to 250 words in every arm.

**Anchors, scored in the same blind pools:** the spec text already stored in `docs/specs/` for each file (written mid-run with the symbol specs in the writer's memory); a hand-written control (written by reading the code, one per file); a "poor" candidate (module doc only).

**Scoring.**
- Key points were written from the code, weighted must-have 2 or nice-to-have 1, and frozen before any run (Appendix B). A judge marks each present (1), partial (0.5) or absent (0) by meaning, not keyword. Score = weighted share.
- Two independent blind judges for recall (one reading candidates in reverse order), candidates shuffled and relabelled; mapping kept in a file no judge saw.
- One precision judge per file checks specific claims against the real code and a "must NOT claim" list per file.
- Calibration: control should score near the top, poor near the bottom.
- Rule fixed before the second round: a richer input is worth building only if it beats baseline B by at least 8 points on both files, adds no wrong claims, and costs less per summary than D.

## Round 1 — the three owner-proposed shapes (mean of 2 judges; range over 3 runs)

| File | A: today (cap) | B: symbol summaries | D: whole file | Stored | Control | Poor |
|---|---|---|---|---|---|---|
| `spec.rs` | 26 (24–29) | **46** (44–48) | 57 (55–60) | 45 | 92 | 12 |
| `mcp.rs` | 23 (21–26) | **34** (30–41) | 52 (46–57) | 62 | 74 | 5 |
| `rust_crates.rs` (fits) | **92** (92–92) | 54 (54–54) | n/a | 81 | 92 | 35 |

Wrong claims: none anywhere except `spec.rs`, where D (both runs), the stored summary and one B run had one each. Three were the same slip, "five kinds of document" while listing four; the fourth misordered generation.

Findings:
1. **A (today) is the worst on large files**, 11 to 19 points under B at no more wrong claims.
2. **Always-summaries is rejected.** Where the raw file fits, raw scored 92 and summaries 54, on every run. Raw wins whenever it fits.
3. **B did not reach D** (11 and 18 points behind, against a bar of "within about 10"). D costs about twice the tokens per summary and cannot go through the task limit on a 244 KB file.
4. The stored summary beat every cold arm on `mcp.rs` (62 against B's 34 and D's 52). It was written mid-run, with the symbol specs and code already in the writer's memory, so part of the gap was probably the instructions or the input, which prompted round 2.

## Round 2 — is richer input, or a better prompt, what helps (mean of 2 judges; range over 3 runs)

Every arm was re-judged in one pool with old B and D, so the scale is shared.

| Arm | `spec.rs` | `mcp.rs` | Tokens per summary (`spec.rs`, `mcp.rs`) | Wrong claims per run |
|---|---|---|---|---|
| B | 52 (48–57) | 35 (30–39) | about 49k, 40k | 0, 0 |
| X (behaviour text, old prompt) | 53 (49–58) | 43 (39–46) | about 72k, 49k | 0, 0 |
| **BP (same input as B, new prompt)** | **68** (64–71) | **55** (50–60) | about 49k, 40k | 0, 0 |
| XP (behaviour text, new prompt) | 68 (61–75) | 65 (46–74) | about 72k, 49k | 0, 0.33 |
| D (whole file) | 61 (57–66) | 49 (48–50) | 94–104k, 71–74k | 1, 0 |
| Stored | 50 | 60 | n/a | 1, 0 |
| Control / poor | 92 / 14 | 74 / 10 | n/a | 0 |

Findings:
1. **The prompt, not the input, does the work.** BP gained 16 and 20 points over B at no extra cost. Adding behaviour text alone (X) gained 2 and 8.
2. **The pre-set rule fails for richer input.** X is under the 8-point bar on `spec.rs`.
3. **XP did not reliably beat BP.** Equal on `spec.rs` (68 and 68). On `mcp.rs` it was +10 on average but split (46 against 74 across runs), and one run made a wrong claim: `get_spec` "always" returns the last good text, which is false for a missing spec.
4. **BP beats whole-file D on both files** (+7, +6), at about half the cost.
5. **The "five kinds" slip reappeared** in D and in the stored summary but not in any B/BP/X/XP run, so it comes from reading the whole file, not from the summaries.

## Conclusions

1. **Raw file when it fits the task limit; otherwise module doc, signatures and each symbol's `### Summary`.** No `### Behavior` text.
2. **Change the file-summary instruction** to ask for coverage of every major part, concrete specifics, and surprising behaviours. That is worth more than any input change tested.
3. **Never summaries-only for a file that fits.** (92 against 54.)
4. **Keep the 8,000-byte cap.** The input shape moves instead.
5. The "first sentence of each summary" rung and the "drop the tests" rung of the original ladder are dropped: measurement showed test-dropping rescues 1 of 23 over-cap files, and the summaries input already fits 19 of them.

## Limits of this evidence

- **Rust only.** Both large files and the small one are CodeOwl's own Rust. The prompt wording is stack-neutral, but the gain is not shown to carry over (TypeScript routes and components, Java classes of many small methods, Python modules). The TypeScript pack also still lacks some symbol kinds (`DECISIONS.md#q21`), which would thin the summaries input there.
- **Not paged.** In every test the writer read all 28 KB of `spec.rs` summaries at once. The real task limit means paging with a cursor, and a writer carrying notes across pages may lose information. Untested.
- **Small samples.** Three runs per arm; judge gaps of 5 to 10 points on large files. Treat differences under about 8 points as noise.
- **Biased control.** The control and key points were written by the same author, so the control's score is biased upward; it is a ceiling check, not an absolute grade.
- **One judge model family** scored everything.
- **The new prompt was not tried on files that fit the cap** (arm A already scored 92 there).
- **Style line.** "Has never taken a compilers course" comes from this repo's `docs/specs/STYLE.md`; the shipped prompt should defer to each repo's own `STYLE.md`, not hard-code it.

## Cost

| Stage | Agents | Tokens |
|---|---|---|
| Round 1 generation | 22 | about 1.09M |
| Round 1 judging | 9 | about 0.42M |
| Round 2 generation | 18 | about 1.0M |
| Round 2 judging | 6 | about 0.33M |
| **Total** | **55** | **about 2.8M** |

## Plan

Nothing below is started. Each build step needs its own branch, plan and approval (`CLAUDE.md`), and tests first.

| # | Step | Gate |
|---|---|---|
| 1 | **Non-Rust check.** Repeat the BP-versus-A comparison (and D as the ceiling) on one TypeScript file (`talentTrail`, private: judged locally, never published) and one Java file (`commons-lang` or `quarkus-super-heroes`) that exceed the cap. Write controls and key points first and freeze them; same blind method. | If BP clearly beats A on both: ship the prompt as stack-neutral. If not: tune the wording per stack before shipping. |
| 2 | **Paged-read check.** On `spec.rs`, compare the single-read BP (68) against a stateless paged read of 4 to 5 chunks with the writer carrying notes. | If paged is within noise of single-read: build paging as designed. If it loses clearly: look at the alternative (summarise chunks, then summarise the chunk summaries). |
| 3 | **Design and tests.** File task shape: raw if the file fits, else module doc + signatures + `### Summary`, paged with a cursor (stateless, like `pending`). Failing tests first, including a rewrite of the `mcp.rs` lifecycle assertion `leaf_body_edit_stales_exactly_its_file_and_containing_rollup`. | Approved plan. |
| 4 | **Fingerprint.** For summary-based tasks, hash module doc + each symbol's signature + its `spec_hash` (the containment rule rollups already use); raw-file tasks keep the whole-file hash. This replaces the parked `shape_hash` plan. `FORMAT_VERSION` bump with its doc-comment entry; guarded migration of stored file hashes or regeneration. | Measured on the comment-only commit scenario (corpus 100 % to 40.2 % current today). |
| 5 | **Prompt.** Add the coverage instruction to `setup/codeowl-generate.md` and `setup/codeowl-generate.prompt.md` together, deferring to `STYLE.md`. | Both copies in step. |
| 6 | **Housekeeping.** Fix the stored `src/spec.rs` summary ("five kinds" lists four). Decide whether a file-summary recall KPI belongs in `NORTHSTAR.md` (it would sit beside K1b; not added yet). | Owner decision. |

---

## Appendix A — what each arm's input looked like

- **B input:** `FILE:`, `MODULE DOC:` (the file's top comment, as written), then `SYMBOLS (N), in file order, each as: signature, then its written summary:` followed by, per symbol, the signature line and its `### Summary` sentence(s) indented under it.
- **X input:** the same, with each symbol's `### Behavior` paragraph added.
- **A input:** the raw file, cut by the same rule as `cap_generation_text` and ending with the marker `[... truncated: this content exceeded the generation-task size limit. Use get_symbol / search_code / get_callers for the parts not shown here.]`.
- The stored top-level `## Summary` was always removed from B and X so the writer could not copy it.

## Appendix B — frozen key points and "must NOT" lists

Weights: M = must-have (2), N = nice-to-have (1). Built from the code, not from stored specs, and frozen before any run.

### `src/spec.rs`

Control summary (hand-written):

> This file owns the spec documents: how they are laid out under `docs/specs/`, how CodeOwl decides whether each one is still accurate, what work remains, and how the agent's writing is checked and saved. It defines four kinds of document: a per-file spec (a section per top-level function or type, with hashes in a hand-parsed frontmatter block), a feature spec, a folder rollup, and one system spec. Which documents exist follows granularity rules: a file qualifies if it declares a top-level function or type (exported or not, generated code excluded), and a folder if at least two files directly in it qualify. Staleness is hash-based and never compares prose: a file or symbol records its own source hash and a hash of the public shape of what it depends on, a feature records its participants' hashes, and rollups and the system spec record their children's spec hashes. `next_task` and its siblings hand the generate loop the next unit of work bottom-up and shape the context: a symbol's full span including folded impl blocks, dependencies scoped to what that text actually names, large classes reduced to an outline, and everything capped at 8,000 bytes with a visible truncation marker. The `submit*` functions validate and save the agent's prose, rejecting empty text and cop-outs such as "see the source", and a hand edit is detected by prose-hash drift and either adopted or preserved. Finally it builds the coverage report: current, stale and missing counts, quality smells, orphaned specs, coverage and freshness as separate axes, fan-in-weighted freshness, and a prioritized, budgeted list with shared code first and the system summary last.

| # | W | Point |
|---|---|---|
| 1 | M | Defines the persisted spec documents and their on-disk format: per-file spec, feature spec, folder rollup, one system spec, saved under a mirrored `docs/specs/` tree with hash frontmatter that a hand-written parser reads back. |
| 2 | M | Staleness is decided by comparing stored hashes with recomputed ones, never by comparing prose. Files and symbols key on their own source hash plus the public-shape hash of their dependencies; features key on their participants' hashes; rollups and the system spec key on their children's spec hashes. |
| 3 | M | Granularity rules decide which documents exist: a file qualifies if it has a top-level callable or container (exported or not; generated code excluded); a folder gets a rollup if at least two of its direct files qualify; one system spec. |
| 4 | M | Builds the "what to write next" tasks for the generate loop bottom-up (symbols, then file, then feature; folder files then the folder; modules and features then system), stateless, returning nothing when everything is current. |
| 5 | M | Shapes the context given to the agent: a symbol's span including folded impl blocks, dependencies scoped to what that text names with externals collapsed, large containers reduced to an outline, and a hard byte cap (8,000) with a visible truncation marker. |
| 6 | M | Validates and saves what the agent submits (required headings or title, rejecting empty text and cop-out or too-short prose), writing nothing on rejection, and returns the recorded hashes. |
| 7 | M | Human edits are detected by drift between the prose and its stored spec hash: edited with unchanged code is quietly adopted; edited with changed code is regenerated with the human text passed along to preserve. |
| 8 | M | Produces the coverage report: every document's status (current, stale, missing) with quality smells, summary counts, coverage and freshness as separate axes, fan-in-weighted freshness, per-kind and per-folder breakdowns, top stale files by impact, and the cycles remaining for a full run. |
| 9 | M | Orders pending work for a budgeted run: widely imported shared code first, then features, other files, folder summaries, tests, and the system summary last. |
| 10 | N | Finds orphaned specs whose subject no longer exists (file, folder, feature, or a symbol section inside a live file), without flagging hand-written documents. |
| 11 | N | Deterministic quality checks ("smells"): cop-out phrases, fewer than four words, and identical dependency lists across all symbols of a file. |
| 12 | N | Reports build-generated source folders for languages that have them, and classifies test paths through the language pack. |

Must NOT claim: that `spec.rs` calls a language model or writes the prose itself; that staleness is decided by comparing the spec text; that only exported/public symbols get a spec section; that the file contains the MCP tools or the file watcher.

### `src/mcp.rs`

Control summary (hand-written):

> This file is CodeOwl's MCP server: every tool an AI coding session can call, with the request and response shapes. There are nine tools in two groups. The read tools never write and never call a language model: `get_symbol`, `get_source` (the real code, with context lines, a byte cap, a `truncated` flag, and a `graph_in_sync` flag that says whether the file changed since the index last saw it), `get_callers` and `get_callees` (file-level import links, not a call graph), `get_spec` (a pure read returning missing, current or stale with what moved, and quality smells), `search_code` (regex search with match, line, context and total-size caps) and `get_spec_coverage` (counts, ratios, breakdowns, orphans and a paged pending list). The write-driving pair, `get_next_spec_task` and `submit_spec`, is how the client's own model fetches a piece of writing, writes it, and sends it back. The task dispatcher is bottom-up: a file's symbols, then the file, then any feature it starts, with `system` sweeping every folder and feature, `feature:<slug>` resolving to that exact entry and not a sibling, and an explicit done object when nothing is left. The server keeps its graph in a swappable cell that the file watcher replaces, so each request works from one consistent snapshot, and the response size limits are configurable when the server starts.

| # | W | Point |
|---|---|---|
| 1 | M | It is the MCP server surface: the nine tools and their request/response shapes, in two groups, read-only tools and the write-driving pair; CodeOwl never calls a language model, the connected agent writes the prose. |
| 2 | M | The graph lives in a swappable cell the file watcher replaces; each request takes one consistent snapshot and never straddles a swap. |
| 3 | M | `get_source` returns real code for a symbol or file, with extra context lines (capped at 20), a byte cap with a `truncated` flag, and `graph_in_sync` saying whether the file changed since the index last saw it. |
| 4 | M | `get_callers` lists files importing a symbol (for a table, files whose query resolves to it), and `get_callees` lists what a file imports with resolved or null targets; both are file-level import links, not a call graph. |
| 5 | M | `get_spec` is a pure read that never triggers generation; it routes by id shape (system, rollup, feature, file or symbol) and reports missing, current or stale with the last good text, what moved, and quality smells. |
| 6 | M | `get_next_spec_task` dispatches bottom-up (symbols, then file, then feature; folders; `system` sweeps all) and returns an explicit done object when nothing is left; `feature:<slug>` resolves to that exact entry. |
| 7 | M | Task payloads are shaped for the agent: scoped dependencies, large containers reduced to an outline, text capped at a configurable size; feature tasks carry core file sources, dependency summaries and table columns; folder and system tasks carry already-written summaries. |
| 8 | M | `submit_spec` routes by id shape to the matching save function and returns the recorded hashes (no source hash for feature, folder and system specs). |
| 9 | M | `get_spec_coverage` returns counts, coverage and freshness, per-kind and per-folder breakdowns, top stale files, orphans, generated-source info, and a prioritized `pending` list paged 50 at a time with a cursor. |
| 10 | N | `search_code` wraps regex search with caps: match count, line width, context lines and total response size. |
| 11 | N | Response conventions: results wrapped in objects because MCP requires them, wire types kept separate from internal ones, and size limits configurable when the server starts. |
| 12 | N | The server's instructions text tells connecting clients when to use each tool. |

Must NOT claim: that `get_spec` or any read tool triggers spec generation; that `get_callers` / `get_callees` give a real call graph; that `search_code` uses a prebuilt index or semantic search; that CodeOwl itself writes the spec prose.

### `src/rust_crates.rs`

Control summary (hand-written):

> This file works out which Cargo crate a Rust file belongs to and which crate names a `use` statement can begin with, both of which import resolution needs. In `crate::x`, `crate` means the root of the file's own crate, not the whole repo; in `use some_crate::x`, the name may be a workspace sibling or the library of the very package a binary, test or example lives in. It reads the `Cargo.toml` files in and above the folders of the indexed files, skipping any that are missing or do not parse. A package is importable only if its library root file is among the indexed files, since a binary-only package cannot be imported, and its name is `[lib] name` if set, otherwise the package name with hyphens turned into underscores; `[lib] path` is honoured and defaults to `src/lib.rs`. `lib_root(name)` answers "which folder is this repo-local library", and gives nothing for outside dependencies such as `std` or `serde`. `crate_root(file)` walks upward for the nearest folder holding `lib.rs`, `main.rs` or a declared library root; a file under no such folder, like one in `tests/` or `examples/`, falls back to the `src` of its owning package, never another package's, and to a top-level `src` when no manifest is above it. Everything is visited in sorted order, so two crates claiming one name always resolve the same way.

| # | W | Point |
|---|---|---|
| 1 | M | Purpose: decide which Cargo crate a Rust file belongs to so that `crate::` and `use some_crate::...` resolve correctly, including multi-crate repos. |
| 2 | M | `crate::` means the root of the file's own crate, found by walking up to the nearest folder holding `lib.rs` or `main.rs` or a manifest-declared library root; not one root for the whole repo. |
| 3 | M | Fallback for files under no such folder (tests, examples): the `src` of the package that owns the file, never another package's, else a top-level `src`. |
| 4 | M | Library discovery from the `Cargo.toml` files in or above indexed files: a package is importable only if its library root is among the indexed files; name is `[lib] name` else the package name with hyphens turned to underscores; `[lib] path` honoured, default `src/lib.rs`. |
| 5 | M | `lib_root(name)` returns the folder of a repo-local library crate and nothing for outside dependencies (`std`, `serde`). |
| 6 | N | Robustness: a missing or unparseable manifest is skipped, never an error; a workspace-only manifest has no package. |
| 7 | N | Deterministic: folders are visited in sorted order, so two crates claiming one name always resolve the same way (first wins). |
| 8 | N | The two small helpers (`parent_dir`, `join`) only do forward-slash path text operations. |

Must NOT claim: that it reads `Cargo.lock`, downloads crates, or resolves external dependencies; that `crate::` means one root for the whole repo; that a binary-only package can be imported by name.

## Appendix C — how to rerun

The inputs, candidates, judge outputs and the label-to-arm key lived in a session scratchpad and are not in the repo. To rerun: rebuild the B input from `docs/specs/<file>.md` (module doc from the source file, then each symbol section's signature and `### Summary`, dropping the top-level `## Summary`), build A with the cap rule in `cap_generation_text`, write one cold agent per run with the prompt wording in "Design", shuffle candidates under fresh labels, and judge with the key points above. For a new file, write the control and key points first and freeze them before generating anything.
