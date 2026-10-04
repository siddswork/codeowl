# exp-05 — What a file summary should be written from

**Status:** measured on Rust (rounds 1 and 2), on one TypeScript and one Java file (round 3), and on paged reading (round 4), 2026-10-03 and 2026-10-04. Nothing is built. The last open piece of the design is the crossover size between paged raw and summaries.
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

## Round 3 — does it carry over to TypeScript and Java (mean of 2 judges; range over 3 runs, 2 for DP)

Same blind method. Controls and key points were written first and frozen (Appendix B). One file per stack, both over the cap and both logic-rich:

| File | Size | Symbols | Where the 8,000-byte cap cuts |
|---|---|---|---|
| TypeScript: `talentTrail/lib/analytics-dashboard.ts` (private repo: judged locally, never published) | 18,574 bytes, 535 lines | 46 | line 261, inside `buildTrend` |
| Java: `quarkus-super-heroes/rest-fights/.../FightService.java` | 15,126 bytes, 387 lines | 1 class, 36 members | line 198, inside a fallback method |

Arms: A (today), AP (today's capped input with the new prompt), DP (whole file, new prompt), and a stack-specific bounded input, all with the new prompt:

| Stack | Bounded input |
|---|---|
| TypeScript (BP) | signature and `### Summary` for each of the 46 symbols (6,251 bytes); summaries written by cold agents from each symbol's own source, because the MCP server is attached to this repo only |
| Java (OP) | deterministic outline: class doc, annotations, and every member's annotations, signature and first comment sentence, no bodies (4,546 bytes; no LLM involved) |
| Java (SP) | the stored class spec's Summary and Behavior (a Java class is one symbol, so this is what the symbol-summary rung would hand over) |

| Arm | TypeScript | Java | Wrong claims per run (TypeScript, Java) |
|---|---|---|---|
| A: capped raw, original prompt (today) | 48 (48–50) | 47 (46–47) | 1, 0 |
| AP: capped raw, new prompt | 48 (48–50) | 47 (46–47) | 1, 0 |
| BP: symbol summaries (TypeScript) | **53** (53–54) | n/a | 0 |
| OP: member outline (Java) | n/a | **66** (65–67) | 0 |
| SP: stored class spec (Java) | n/a | 46 (45–46) | 2 |
| DP: whole file | 78 (78–79) | 78 (72–83) | 0, 0 |
| Stored spec | none | 34 | 1 |
| Control / poor | 90 / 3 | 78 / 5 | 0 |

Per key point, the shape of each arm:
- **Capped raw (A, AP):** exactly 1.00 on points before the cut and exactly 0.00 on every must-have point after it. TypeScript: 0 on four of its eight must-have points, all in the second half of the file (the function that loads the data, and three of the transform functions). Java: 0 on the winner logic, saving a fight, narration with circuit breaker and retries.
- **TypeScript summaries (BP):** no zeros on must-have points, but almost everything is 0.50: the topic without the specifics (numeric thresholds, lookup-table contents, the reason the data is fetched pre-aggregated). That last reason sits in a comment attached to no symbol, so no summary carries it.
- **Java outline (OP):** no zeros on must-have points; weakest on the winner logic and saving a fight (0.50), because an outline shows names, not logic.
- **Java stored class spec (SP):** misses the queries entirely, and inherits two false claims from the stored spec: every public method carries `@WithSpan` (`findRandomLocation` at line 132 does not), and `determineWinner` and `shouldHeroWin` are private (they are package-private, lines 309 and 333). The stored spec itself also had one wrong claim.

Findings:
1. **Today's capped input is blind to the second half of any big file**, on every stack: 23 to 48 % recall against 78 to 92 % for the control.
2. **The new prompt does nothing on capped raw text** (AP equals A on both files). In Rust the prompt's +16 and +20 came with a summaries input; it helps when the input spans the whole file, not when the content is simply absent. The "stack-neutral prompt lifts recall" claim is not established.
3. **TypeScript: symbol summaries beat today by about 5 points, within the noise; the whole file is 25 points higher.** This file is only 2.3 times the cap. The Rust result (summaries beat whole-file reading) held for files 20 to 30 times the cap, so the crossover between paging the raw file and summarising lies somewhere in between. Untested.
4. **Java: the deterministic outline gains 19 points** with no wrong claims, from an input that costs no LLM tokens to build.
5. **A Java class is one symbol, so the symbol-spec rung degenerates** into the class spec, which was no better than today and carried the stored spec's errors forward.
6. **All 6 capped TypeScript runs said the file was cut off** ("cut off partway through `buildTrend`"), counted as a wrong claim because the file is complete. It is the writer reporting its input, but it ends up in the summary. The Java capped runs did not.
7. **Existing prose is a risky input.** The stored `FightService` spec carries false claims that a summary built on it repeated in every run.

## Round 4 — does paging hurt (one recall judge; 2 runs per arm)

Minimal-cost design (about 0.28M tokens): reuse the frozen key points and the earlier single-read results, two runs per arm, one blind recall judge per file, and three already-scored anchors in each pool to check the judge against its earlier scores. The decision rule was set before running: paging is fine if the paged score is within 8 points of the single-read score.

Pages were cut between whole blocks (functions and their comments, or symbol entries), at most about 8,000 bytes each, with a "continues on page N of M" marker. Each run was one agent reading the pages in order, as a real session calling the task tool repeatedly would, then writing the paragraph.

| Experiment | Pages (bytes) | Paged score | Single-read score | Gap |
|---|---|---|---|---|
| TypeScript, raw file | 3 (7,466; 7,730; 3,553) | 77.5, 80.0 (mean 78.8) | whole file in one read: 78 | +1 |
| `spec.rs`, symbol summaries | 4 (7,779; 7,923; 8,089; 4,699) | 71.4, 69.0 (mean 70.2) | summaries in one read: 68 | +2 |
| TypeScript, today (capped), reference | n/a | n/a | 48 | paged raw is +31 over today |

Judge check against the anchors (earlier scores were the mean of two judges; this was one judge):

| Anchor | Earlier | This judge |
|---|---|---|
| TypeScript control | 90.0 | 87.5 |
| TypeScript whole file | 77.5 | 75.0 |
| TypeScript capped | 50.0 | 50.0 |
| `spec.rs` control | 91.7 | 90.5 |
| `spec.rs` summaries | 67.9 | 64.3 |
| `spec.rs` capped | 28.6 | 33.3 |

The judge scored on average about 2 points below the earlier means and moved at most 4.7 points on any anchor, so gaps of +1 and +2 are within its noise: the result is "no measurable difference", not "paging helps".

Findings:
1. **Paging did not hurt**, on raw text (TypeScript) or on symbol summaries (`spec.rs`), against the 8-point bar.
2. **Paged raw reaches the whole-file score** (about 79 against 48 today), at the same cost per summary as a single read (about 47k tokens).
3. **The crossover size is still open.** TypeScript at 2.3 times the cap favours paged raw (79 against 53 for summaries); `spec.rs` at 31 times favours summaries (68 and 70 against 61 for one read of the whole file). Somewhere between 2 and 31 times the cap, paged raw stops beating summaries. A starting rule of up to 5 to 8 pages of raw text, summaries beyond, is a guess to confirm on one mid-size file.

## Conclusions

What the four rounds support:

1. **The cap is the real problem.** A capped summary reads as complete and describes a prefix: perfect before the cut, zero after it, on Rust, TypeScript and Java.
2. **No single replacement input fits every file.** What was measured, by file shape:

   | File shape | Best input tested | Result against today |
   |---|---|---|
   | Fits the cap (Rust, 5.8 KB) | raw file | 92 against 54 for summaries |
   | Slightly over (TypeScript, 2.3×) | whole file | 78 against 48; summaries only 53 |
   | Many symbols, far over (Rust, 20 to 30×) | symbol summaries + coverage prompt | 68 and 55 against 26 and 23; beats the whole file read (61, 49) |
   | One big class (Java) | deterministic member outline | 66 against 47 |

3. **Never summaries-only for a file that fits** (92 against 54).
4. **The coverage prompt helps only when the input spans the whole file.** Keep it for summaries and outline inputs; it does nothing for capped raw text.
5. **Symbol summaries keep topics and lose specifics.** Detail lives in bodies and in comments attached to no symbol.
6. **Do not feed earlier LLM prose back in as the only input** (the stored class spec carried false claims into every Java run); prefer deterministic inputs where they exist.
7. **Keep the 8,000-byte cap.** The input shape moves instead.
8. **Design direction (not built):** an input ladder chosen by size and shape: raw when it fits; the raw file in pages when modestly over (paging measured to cost nothing in quality, round 4); symbol summaries, also in pages, when there are many symbols and the file is far over; the member outline for a single large class. Where paged raw gives way to summaries is not yet measured. The fingerprint hashes exactly what the writer saw for the rung used.
9. The "first sentence of each summary" and "drop the tests" rungs of the original ladder are dropped: test-dropping rescues 1 of 23 over-cap files, and the summaries input already fits 19 of them.
10. `### Behavior` text added to the summaries input was not worth its extra cost on Rust (+2 and +8 alone, no reliable gain over the prompt). It was not tried on TypeScript, where it may matter more.
11. **Paging does not hurt** (round 4): paged raw (TypeScript, 3 pages) and paged summaries (`spec.rs`, 4 pages) each scored within 2 points of their single-read counterparts, against a bar of 8, in the lenient case where one session sees every page.

## Limits of this evidence

- **One file per non-Rust stack, none for Python.** Rounds 1 and 2 are CodeOwl's own Rust; round 3 adds one TypeScript and one Java file. The prompt wording is stack-neutral, but its gain did not carry over to capped raw text, and nothing was measured on a Python module, a React component or Next.js route, or a Java file with several small classes. The TypeScript pack also still lacks some symbol kinds (`DECISIONS.md#q21`).
- **The TypeScript symbol summaries were not from the real pipeline.** The MCP server is attached to this repo only, so each symbol's `### Summary` came from a cold agent given that symbol's source, 5 agents handling about 5 symbols each (some context shared within a group). 36 of the 46 symbols are types and constants.
- **Round 3 had no TypeScript arm with behaviour text and no old-prompt summaries arm**, so the prompt's share of the TypeScript summaries score is not separable.
- **The "cut off" statement counted as wrong** in all 6 capped TypeScript runs is a judging choice: it is the writer reporting its input, but it ends up in the summary text.
- **The control note says the cap cuts TypeScript at line ~290;** it cuts at line 261. The frozen control file keeps the wrong figure; the key points are unaffected.
- **Paging was tested only in the lenient case.** In rounds 1 to 3 the writer read all of its input at once. Round 4 paged it, but one agent session saw every page, so every page stayed in its context. A writer that must forget earlier pages and carry notes was not tested (it would need one agent per page, roughly 2 to 3 times the cost); worth running only if the real loop is found to shed earlier pages.
- **Round 4 is small.** Two runs per arm, one judge (checked against six anchors: at most 4.7 points off, about 2 points stricter on average), no precision check on wrong claims, and one file per experiment.
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
| Round 3 TypeScript symbol summaries | 5 | about 0.2M |
| Round 3 generation | 25 | about 1.04M |
| Round 3 judging | 6 | about 0.31M |
| Round 4 generation | 4 | about 0.19M |
| Round 4 judging | 2 | about 0.09M |
| **Total** | **97** | **about 4.6M** |

## Plan

Nothing below is started. Each build step needs its own branch, plan and approval (`CLAUDE.md`), and tests first.

| # | Step | Gate |
|---|---|---|
| 1 | **Non-Rust check. Done (round 3).** One TypeScript and one Java file, controls and key points frozen first. | Result: the Rust finding does not generalise as one rule; see Conclusions. |
| 2 | **Paged-read check. Done (round 4), lenient case.** Paged raw (TypeScript, 3 pages) 78.8 against 78 single-read; paged summaries (`spec.rs`, 4 pages) 70.2 against 68. | Passed the pre-set bar (within 8 points). Not tested: a writer that must forget earlier pages; run only if the real loop sheds them. |
| 3 | **Find the crossover (next).** Decide the file size, relative to the cap, at which paged raw stops beating summaries. One mid-size file (about 8 to 15 times the cap) per stack, raw paged against summaries paged, same blind method with a precision pass. | A stated rule: raw up to N times the cap, summaries beyond. The starting guess is 5 to 8 pages. |
| 4 | **Design and tests.** File task shape per the ladder in Conclusions, paged with a cursor (stateless, like `pending`); a Java member outline built deterministically from the extractor's own member data. Failing tests first, including a rewrite of the `mcp.rs` lifecycle assertion `leaf_body_edit_stales_exactly_its_file_and_containing_rollup`. | Approved plan, own branch. |
| 5 | **Fingerprint.** Hash exactly what the writer saw for the rung used: raw-file tasks keep the whole-file hash; summary tasks hash module doc + each symbol's signature + its `spec_hash` (the containment rule rollups already use); outline tasks hash the outline. This replaces the parked `shape_hash` plan. `FORMAT_VERSION` bump with its doc-comment entry; guarded migration of stored file hashes or regeneration. | Measured on the comment-only commit scenario (corpus 100 % to 40.2 % current today). |
| 6 | **Prompt.** Add the coverage and specifics instruction for the summaries and outline inputs only, to `setup/codeowl-generate.md` and `setup/codeowl-generate.prompt.md` together, deferring to `STYLE.md`. Tell the writer not to describe its own input as cut off. | Both copies in step. |
| 7 | **Housekeeping.** Fix the stored `src/spec.rs` summary ("five kinds" lists four) and the false claims in the stored `FightService` spec (`quarkus-super-heroes`, local only). Decide whether a file-summary recall KPI belongs in `NORTHSTAR.md` (it would sit beside K1b; not added yet). | Owner decision. |

---

## Appendix A — what each arm's input looked like

- **B input:** `FILE:`, `MODULE DOC:` (the file's top comment, as written), then `SYMBOLS (N), in file order, each as: signature, then its written summary:` followed by, per symbol, the signature line and its `### Summary` sentence(s) indented under it.
- **X input:** the same, with each symbol's `### Behavior` paragraph added.
- **A input:** the raw file, cut by the same rule as `cap_generation_text` and ending with the marker `[... truncated: this content exceeded the generation-task size limit. Use get_symbol / search_code / get_callers for the parts not shown here.]`.
- The stored top-level `## Summary` was always removed from B and X so the writer could not copy it.
- **TypeScript BP input (round 3):** `FILE:`, `MODULE DOC: (none)`, then 46 symbols in file order, each as its declaration head (for example `export function buildTrend`) and a one-or-two-sentence summary. Each symbol's source began at its leading comment and ended at its closing brace; a section-divider comment that belongs to no symbol was dropped, as the real extractor would.
- **Java OP input (round 3):** `FILE:`, class doc, `@ApplicationScoped public class FightService`, then every member in file order: its annotations, its signature, and the first sentence of its comment where one exists (only one member had a comment). No bodies.
- **Java SP input (round 3):** the stored class spec's `### Summary` and `### Behavior`, with a one-line header saying the file holds a single class.

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

### `lib/analytics-dashboard.ts` (TypeScript, `talentTrail`; round 3)

Withheld: the source repo is private, and the control summary and 12 key points restate its business logic. The structure was the same as the others (8 must-have, 4 nice-to-have points, a hand-written control, a "must NOT claim" list of 4); the scores are in Round 3. To rerun on another TypeScript file, write the control and key points first and freeze them.

### `FightService.java` (Java, `quarkus-super-heroes/rest-fights`; round 3)

Control summary (hand-written):

> This is the business logic of the Fight microservice in the Quarkus Super Heroes sample: one application-scoped class that lists fights, picks fighters, runs a fight, asks another service to narrate it, and saves it. Almost every call to another service (heroes, villains, locations, narration) is wrapped in a timeout, 2 seconds for a single lookup and 4 for the combined random-fighters call, 5 for the "hello" pings, with a fallback method that returns a stand-in built from configuration (a default hero, villain, location, narration or image, or a "Could not invoke the X microservice" message), so a fight can still go ahead when a dependency is down. Random fighters are fetched together, each falling back on its own if empty, with an optional configured delay added to show how delays behave. Running a fight works out the winner and then persists it: the hero wins if its level plus a random adjustment (bounded by configuration) beats the villain's; otherwise the villain wins if its level is strictly higher; otherwise a coin flip decides; the date is stamped, the fight is saved to the reactive MongoDB store and an event is emitted on the "fights" channel. The two calls to the narration service are heavier: a circuit breaker that opens after failures reach half of 8 requests, three retries 200 ms apart, a 30-second timeout for text and 2 minutes for image generation, and configured fallback text or image. Paged queries sort by fight date, newest first, and public operations are traced with OpenTelemetry spans.

| # | W | Point |
|---|---|---|
| 1 | M | Purpose: the business logic of the Fight microservice, a single application-scoped Quarkus class that lists fights, picks random fighters and a location, runs a fight, has it narrated (text and image), saves it, and pings the other services. |
| 2 | M | Resilience on calls to other services: each has a timeout (about 2 s for single lookups, 4 s for the combined random-fighters call, 5 s for the hello pings) and a fallback method returning a stand-in built from configuration (default hero, villain, location, narration, image, or a "Could not invoke the X microservice" message), so a fight can proceed when a dependency is down. |
| 3 | M | Random fighters: a random hero and villain are fetched together, each falling back to the configured default when it comes back empty; an optional configured delay can be added to the response (a demo of delays and fault tolerance). |
| 4 | M | Winner logic: the hero wins if its level plus a random adjustment (bounded by configuration) beats the villain's with its own adjustment; otherwise the villain wins if its level is strictly higher; otherwise a coin flip; the fight is stamped with the current date. |
| 5 | M | Saving a fight: persisted to the reactive MongoDB store and a mapped event is emitted on the "fights" messaging channel (fire and forget) before the fight is returned. |
| 6 | M | Narration and image generation are the heavy calls: circuit breaker (opens at a 50% failure ratio over 8 requests, 2 s delay), 3 retries 200 ms apart, long timeouts (30 s for narration text, 2 minutes for image), and configured fallback narration text or image. |
| 7 | M | Queries: all fights, a page of fights sorted by fight date newest first, and one fight by its MongoDB id. |
| 8 | N | Four hello methods ping the heroes, villains, locations and narration services, each with a 5 s timeout and a fallback message. |
| 9 | N | Public operations are traced with OpenTelemetry spans, with arguments recorded as span attributes. |
| 10 | N | Clients, the event emitter, configuration and mappers are injected through the constructor; fallbacks, delay, adjustment bounds and team names all come from the configuration object. |
| 11 | N | A fight record is built from the winner and loser: name, picture, level, powers, each side's configured team name, and the location. |
| 12 | N | Fallback heroes, villains and locations are created from configured name, level or description, picture and powers. |

Must NOT claim: that the winner is decided only by comparing levels (there is a random adjustment and a coin flip); that it uses a relational/JDBC database or blocking calls; that the service itself generates the narration or image (it calls the narration service); that results are cached.

## Appendix C — how to rerun

The inputs, candidates, judge outputs and the label-to-arm key lived in a session scratchpad and are not in the repo. To rerun: rebuild the B input from `docs/specs/<file>.md` (module doc from the source file, then each symbol section's signature and `### Summary`, dropping the top-level `## Summary`), build A with the cap rule in `cap_generation_text`, write one cold agent per run with the prompt wording in "Design", shuffle candidates under fresh labels, and judge with the key points above. For a new file, write the control and key points first and freeze them before generating anything.
