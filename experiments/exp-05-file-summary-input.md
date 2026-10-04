# exp-05 — What a file summary should be written from

**Status (2026-10-04, seven rounds; Round 7 is a no-token replay of the Rust history):** measured on Rust (rounds 1 and 2), on one TypeScript and one Java file (round 3), on paged reading (round 4), on the crossover between paged raw and summaries (round 5), and on dropping test code before paging (round 6), 2026-10-03 to 2026-10-04. Nothing is built. The ladder is specified except for the exact cutoff between 12 and 22 pages (interpolated) and a writer that must forget earlier pages (untested); the next step is a build plan on its own branch.
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

## Round 5 — where raw gives way to summaries (`src/rust.rs`, 12.4× the cap; 2 recall judges, 1 precision judge)

One Rust file in the unmeasured middle: 99,585 bytes (12.4 times the cap), 2,455 lines, 35 symbols, about 47 % of the bytes unit tests. Controls and key points were written first and frozen (Appendix B). Arms: raw text in 14 pages (about 8,000 bytes each, cut at blank lines, tests included as a real task would send them) against the module doc plus 35 symbol summaries in 2 pages, both with the new prompt, 2 runs each; one capped reference run with the original prompt; a hand-written control and a one-line poor candidate as anchors.

| Arm | Run 1 | Run 2 | Mean | Tokens per run | Wrong claims |
|---|---|---|---|---|---|
| **Paged raw** (14 pages) | 73.8 | 71.4 | **72.6** | about 84k | 0 |
| **Paged summaries** (2 pages) | 64.3 | 52.4 | **58.4** | about 43k | 0 |
| Capped today (reference, 1 run) | 35.7 | n/a | 35.7 | about 41k | 0 |
| Control / poor | 81.0 / 4.8 | n/a | n/a | n/a | n/a |

The two judges agreed exactly on every candidate except the capped one (33.3 and 38.1).

Per key point (mean over runs and judges):

| # | W | Key point | Paged raw | Paged summaries | Capped | Control |
|---|---|---|---|---|---|---|
| 1 | M | Purpose: two jobs (items and imports) | 1.00 | 1.00 | 0.50 | 1.00 |
| 2 | M | Kind mapping | 0.50 | 0.75 | 1.00 | 1.00 |
| 3 | M | Shallow; what it skips | 1.00 | 0.75 | 0.75 | 1.00 |
| 4 | M | Inherent impl folding | 0.50 | 0.50 | 0.50 | 1.00 |
| 5 | M | Trait impl folds in one narrow case | 1.00 | 0.50 | 1.00 | 1.00 |
| 6 | M | Public surface feeding the interface hash | 1.00 | 0.50 | 0.00 | 1.00 |
| 7 | M | Doc, attribute and `pub` details | 0.00 | 0.50 | 0.00 | 0.00 |
| 8 | M | Import extraction | 1.00 | 0.50 | 0.00 | 1.00 |
| 9 | M | Import resolution | 1.00 | 0.50 | 0.00 | 1.00 |
| 10 | N | Inline `crate::` edge rule | 0 | 0 | 0 | 0 |
| 11 | N | Same-file trait edge | 1.00 | 1.00 | 0.00 | 1.00 |
| 12 | N | Deterministic; externals unresolved | 0.25 | 0.25 | 0 | 0 |

- Capped input loses the whole imports half of the file (points 8, 9 and 11 sit after the cut).
- Paged summaries keep every topic but mostly at 0.50: right subject, missing specifics.
- Points 7 and 10 were missed even by the control: the edge of what one 250-word paragraph holds.

Putting the five points together (different files and judging sessions, so a rough picture only):

| File | Size vs cap | Best raw | Summaries | Raw advantage |
|---|---|---|---|---|
| `analytics-dashboard.ts` | 2.3× | 79 (paged, 3 pages) | 53 | +26 |
| `rust.rs` | 12.4× | 73 (paged, 14 pages) | 58 | +14 |
| `mcp.rs` | 22× | 49 (one read of the whole file) | 55 | −6 |
| `spec.rs` | 31× | 61 (one read of the whole file) | 68 | −7 |

Findings:
1. **At 12.4 times the cap, paged raw still beats summaries by 14 points**, with no wrong claims in either. Both far outscore today's capped input (36).
2. **The raw advantage shrinks steadily and turns negative somewhere between about 12 and 22 times the cap**, roughly 15 to 20 pages. A safe starting rule is paged raw up to about 12 pages (the largest size actually verified), summaries beyond.
3. **Paged raw costs about twice the tokens of paged summaries** (84k against 43k). Half of this file's pages were unit tests; dropping `#[cfg(test)]` blocks before paging would roughly halve that cost and push the raw range further out. Untested.
4. **The 22× and 31× comparisons used one read of the whole file, not pages**, and were different files and judging sessions; the cutoff is an interpolation, not a measurement.

## Round 6 — does dropping test code before paging hurt (`src/rust.rs`; 2 recall judges, 1 precision judge)

Same file and frozen key points as round 5. The test module (`#[cfg(test)] mod tests`, about 47 % of the bytes) was cut off before paging: 52,950 bytes of code in 8 pages (the last page small) against 14 pages with tests. Two runs, new prompt. The earlier paged-raw runs, the paged-summaries run and the control were re-judged in the same blind pool so every score is on one scale.

| Arm | Run 1 | Run 2 | Mean | Pages | Tokens per run | Wrong claims |
|---|---|---|---|---|---|---|
| **Paged raw, tests dropped** | 71.4 | 81.0 | **76.2** | 8 | about 62k | 0 |
| Paged raw, tests included | 76.2 | 73.8 | 75.0 | 14 | about 84k | 0 |
| Paged summaries | 59.5 | n/a | 59.5 | 2 | about 43k | 0 |
| Control | 81.0 | n/a | 81.0 | n/a | n/a | 0 |

Per key point, dropping tests against keeping them: inherent impl folding 1.00 against 0.75, doc, attribute and `pub` details 0.75 against 0, public surface feeding the interface hash 0.50 against 1.00, import extraction 0.75 against 1.00; everything else equal. Both miss the inline `crate::` rule. The two re-judged anchors moved by about 2 and 5 points from round 5, so only same-session comparisons count.

Findings:
1. **Dropping test code costs no measurable quality** (76.2 against 75.0, noise). One stripped run reached the control's score.
2. **It saves 43 % of the pages and 27 % of the tokens per summary.** Tokens fall less than pages because the prompt and the first reads cost the same either way.
3. **For Rust the page-count cutoff applies after stripping**: paged raw up to about 12 pages of non-test code. `rust.rs` is 8 pages stripped. Other stacks keep their tests in separate files, so nothing changes for them.
4. **The fingerprint can then exclude test code too**: hashing what the writer saw means a test-only edit no longer stales the file summary.

## Round 7 — what the fingerprint rule spares, replayed on CodeOwl's own history (Rust only, no tokens)

Rounds 1 to 6 decided what the writer is shown. This round asks what should make a file summary stale. The probe replays the 148 commits that touched `src/*.rs`: 342 edits to existing files. For each edit it asks whether the file summary's fingerprint would have changed under a candidate rule, using the real extractor (symbols, signatures, export flags, imports, doc comments) and tree-sitter for literals. The probe was throwaway and is not in the repo.

The history, by kind of edit: shape change (a signature, constant, import, or an added or removed item) 223 (65.2 %); function body only 67 (19.6 %); other comment or whitespace 20 (5.8 %); test-only 18 (5.3 %); doc-comment only 14 (4.1 %).

**Part A — the rules**

| Rule | Edits that would NOT stale the file summary | Excluding the 18 test-only edits |
|---|---|---|
| 0. Today: whole file text | 0 (0 %) | 0 |
| 1. Shape, ignore doc comments | 116 (33.9 %) | 101 of 324 (31.2 %) |
| 2. Shape, include doc comments | 91 (26.6 %) | 76 of 324 (23.5 %) |
| 3. **The ladder rule as recorded in Round 5/6**: raw files (up to 12 pages) hash their text without tests; beyond that, shape + docs + symbol bodies | **18 (5.3 %)** | **0 of 324** |
| 4. As 3, but the raw rung hashes shape + docs instead of text | 88 (25.7 %) | 73 of 324 (22.5 %) |

"Shape" is symbol ids, kinds, signatures, export flags, top-level constant values and imports; "doc comments" is each symbol's doc comment plus the module doc. 312 of the 342 edits (91 %) are in files that fit the raw rung.

**Part B — literals** (a number or string written inside a function body, which the shape does not read). Each rule below is shape + docs + the literal rule. Literals are compared per file, outside the test module.

| Literal rule | Spared | Share | Body-only edits flagged (of 56) |
|---|---|---|---|
| none | 91 | 26.6 % | 0 |
| L1: every literal | 67 | 19.6 % | 19 |
| L2: numbers except 0, 1, 2, plus strings of 8+ characters | 73 | 21.3 % | 13 |
| L3: numbers except 0, 1, 2, plus every string | 68 | 19.9 % | 18 |
| L4: numbers except 0, 1, 2 only | 90 | 26.3 % | 1 |
| L5: strings of 8+ characters only | 73 | 21.3 % | 13 |

The body-only count is 56 here (the earlier 67 also counted edits that changed a doc comment, which every doc-including rule already flags).

Hand-check of the 19 edits L1 flags (author's judgment, not a judge model): 3 clearly relevant to a summary (the `mcp.rs` server instructions rewritten twice, and the tool descriptions rewritten); about 6 plausibly relevant (a new error message, new symbol kind names, the text `"same file"` shown in dependency lines); about 10 noise (single characters like `'\n'`, the `2` in `[usize; 2]`, a reworded log message).

Findings:
1. **The ladder rule as recorded spares nothing outside test edits** (0 of 324): any non-test text change counts for a raw-rung file, and nearly every file is in the raw rung. "Hash exactly what the writer saw" therefore does not deliver the goal of this work for most files.
2. **A shape + docs fingerprint on every file spares about a quarter of edits** (26.6 %), mostly the 67 body-only edits.
3. **Numbers almost never change on their own in body-only edits** (1 of 56), so a numbers-only rule is nearly free (26.3 %) but catches few cases; **strings carry the flags**, and the clearly relevant ones are agent-facing text.
4. **Adding the L2 literal rule costs about 5 points** (26.6 % to 21.3 %) and catches the number changes and long-string rewrites, the cases a false "current" would hurt most. The case it closes is the one where a body edit changes a limit or message the summary quotes, such as a word-count floor going from 4 to 6.
5. **Including doc comments costs 7 to 8 points against ignoring them** (26.6 % against 33.9 %) and keeps a doc fix, which a summary paraphrases, from leaving wrong text marked current (the failure PR #87 exposed).

## Conclusions

What the seven rounds support:

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
8. **Design direction (not built):** an input ladder chosen by size and shape: raw when it fits; the raw file in pages when modestly over (paging measured to cost nothing in quality, round 4); symbol summaries, also in pages, when there are many symbols and the file is far over; the member outline for a single large class. Paged raw beat summaries by 14 points at 12.4 times the cap and lost by about 6 to 7 at 22 and 31 times, so the cutoff lies between about 12 and 22 pages; start at 12. What the fingerprint hashes is a separate question, answered in Conclusion 14 (round 7).
9. The "first sentence of each summary" and "drop the tests" rungs of the original ladder are dropped: test-dropping rescues 1 of 23 over-cap files, and the summaries input already fits 19 of them.
10. `### Behavior` text added to the summaries input was not worth its extra cost on Rust (+2 and +8 alone, no reliable gain over the prompt). It was not tried on TypeScript, where it may matter more.
11. **Paging does not hurt** (round 4): paged raw (TypeScript, 3 pages) and paged summaries (`spec.rs`, 4 pages) each scored within 2 points of their single-read counterparts, against a bar of 8, in the lenient case where one session sees every page.
12. **The cutoff between paged raw and summaries lies between about 12 and 22 times the cap** (round 5): raw won by 14 at 12.4× (73 against 58), summaries won by 6 to 7 at 22× and 31×. Starting rule: paged raw up to about 12 pages, summaries beyond. Paged raw costs about twice the tokens of summaries.
13. **For Rust, drop test code before paging** (round 6): quality unchanged (76 against 75), 43 % fewer pages and 27 % fewer tokens. The 12-page cutoff applies to the pages sent after stripping.
14. **The fingerprint should not be "the text the writer saw"** (round 7): that rule spares 0 of 324 non-test edits in CodeOwl's history. The proposed rule is a **shape + doc comments + literals fingerprint** on every file: symbol ids, kinds, signatures, export flags, top-level constant values, imports, each symbol's doc comment and the module doc, plus the numbers (other than 0, 1, 2) and strings of 8 or more characters in function bodies (rule L2). It spares about 21 % of edits (about 27 % without the literal part). Body edits still stale the symbol's own spec. This is a recommendation, pending the owner's choice between this, shape + docs without literals, and shape without docs.

## Limits of this evidence

- **One file per non-Rust stack, none for Python.** Rounds 1 and 2 are CodeOwl's own Rust; round 3 adds one TypeScript and one Java file. The prompt wording is stack-neutral, but its gain did not carry over to capped raw text, and nothing was measured on a Python module, a React component or Next.js route, or a Java file with several small classes. The TypeScript pack also still lacks some symbol kinds (`DECISIONS.md#q21`).
- **The TypeScript symbol summaries were not from the real pipeline.** The MCP server is attached to this repo only, so each symbol's `### Summary` came from a cold agent given that symbol's source, 5 agents handling about 5 symbols each (some context shared within a group). 36 of the 46 symbols are types and constants.
- **Round 3 had no TypeScript arm with behaviour text and no old-prompt summaries arm**, so the prompt's share of the TypeScript summaries score is not separable.
- **The "cut off" statement counted as wrong** in all 6 capped TypeScript runs is a judging choice: it is the writer reporting its input, but it ends up in the summary text.
- **The control note says the cap cuts TypeScript at line ~290;** it cuts at line 261. The frozen control file keeps the wrong figure; the key points are unaffected.
- **Paging was tested only in the lenient case.** In rounds 1 to 3 the writer read all of its input at once. Round 4 paged it, but one agent session saw every page, so every page stayed in its context. A writer that must forget earlier pages and carry notes was not tested (it would need one agent per page, roughly 2 to 3 times the cost); worth running only if the real loop is found to shed earlier pages.
- **Round 6 is the same single file with two runs per arm.** Stripping was tested on Rust only, where the test module is part of the file; it does not apply to stacks whose tests live in other files. Run 1 and run 2 of the stripped arm differ by 10 points (71 and 81), so the 1-point gap to the unstripped arm is well inside the noise.
- **Round 5 is one Rust file with two runs per arm.** The 22× and 31× comparisons come from other files, other judging sessions and one read of the whole file rather than pages, so the cutoff between 12 and 22 pages is an interpolation. The paged-raw input included the file's unit tests (about half the bytes), as a real task would.
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
| Round 5 generation | 5 | about 0.30M |
| Round 5 judging | 3 | about 0.15M |
| Round 6 generation | 2 | about 0.12M |
| Round 6 judging | 3 | about 0.16M |
| Round 7 history replay | 0 (scripts only) | 0 |
| **Total** | **110** | **about 5.4M** |

## Plan

Nothing below is started. Each build step needs its own branch, plan and approval (`CLAUDE.md`), and tests first.

| # | Step | Gate |
|---|---|---|
| 0 | **First: a read-only "explain stale" MCP tool** (owner's call, 2026-10-04). A new tool, so no existing interface changes: with an id it returns the cause chain for one stale document (a symbol's `source` or a named `dependency` that moved; a file's symbols or imports; a folder or system spec's stale child); without an id it counts causes across the whole corpus (the table built by hand earlier: dependencies only, source only, both). It needs one additive piece of data: each target's hash kept in the spec frontmatter beside the existing combined `deps_hash`, written on submit. No hash definition changes, so **no migration**; specs written before it report "cause unknown" until regenerated. It stays a pure read and never triggers generation. The writer's input is unchanged (rewriting from scratch is kept on purpose: old text carries old mistakes). Its value is debugging, measuring, and letting a reader of a stale spec see what moved; it makes steps 0c and 5 measurable before and after. MCP-only at first. **Agreed details (owner, 2026-10-04):** tool name `explain_stale { id? }`; the record is an extra frontmatter block of lines `  <owner id> -> <target id>: <hash>` (owner `file` for the file itself), hashes shortened to 12 hex characters; nested causes limited to 3 levels (file, folder, system); the graph cache and `FORMAT_VERSION` do not change. Built as five commits: (1) refactor `dependency_hash` so the sorted `(target id, hash)` pairs are available, with the existing `deps_hash` byte-identical; (2) record the pairs on submit and parse them back, tolerating specs without them; (3) the explanation core; (4) the tool, including the whole-corpus cause counts and the count of dependants resting on a coarse source-fallback target; (5) docs (`ARCHITECTURE.md` tool reference and "Caching and invalidation", `GLOSSARY.md`, `setup/USAGE.md`, `CLAUDE.md` tool count, a line in the server instructions; no milestone numbers in anything an agent reads). Tests first, among them: identical `deps_hash`; frontmatter round trip and a legacy file; submit records pairs equal to the current pairs; a signature edit of an imported symbol is named by the tool with old and new hash while a body edit of it leaves the dependant current; a spec without the record reports "unknown"; a stale file makes its folder stale and the chain nests the file's cause; corpus counts on a fixture; the tool writes nothing and every existing tool's tests pass untouched. | Own branch (`explain-stale`), a plan grounded in the code (approved in conversation), tests first, owner approval. Adds the tenth tool to the surface listed in `CLAUDE.md`. |
| 0b | **Backfill the per-target record for CodeOwl's own specs, as its own commit** (owner's call, 2026-10-04), after step 0 is merged. A small deterministic command (no LLM, no tokens): for each spec, recompute the combined `deps_hash` from the current graph; only if it equals the stored value, write the current per-target pairs (they are then provably what the spec was written from); skip every other spec, which stays "unknown" until regenerated. Gives a clean "before" baseline for steps 0c and 5. | The command's own tests (matching spec backfilled, stale spec skipped); the commit touches only spec frontmatter. |
| 0c | **The import-hash fix, before the file-summary work** (owner's argument, 2026-10-04: it is the original trigger, smaller, and independent of the input ladder; the file-summary problem was found while checking it). The owner's and the author's shared reading, **to be confirmed against the original wording**: importers of a file target hash that file's whole raw text (`interface_or_source_hash` falls back to the file's `source_hash`, `spec.rs`), so a comment-only edit stales every importer; give a file target a hash of its exported interface instead. It changes stored `deps_hash` values, so it needs `FORMAT_VERSION` handling and a guarded migration (below). Measured with step 0's cause counts, before and after. | Own branch and plan after step 0; the definition of the file-level interface hash is confirmed by the owner first. |
| 0d | **One migration tool for hash-definition changes** (serves 0c and 5, so the corpus is migrated once, not twice, per the owner's point): rewrites stored hashes only for specs that were current under the old definition (verified by recomputing under the old one), no tokens, then a coverage check. Alternative: regenerate (about 540k tokens last time); the script is cheaper and CodeOwl has no external corpora to migrate. | Built with 0c; reused by 5. |
| 1 | **Non-Rust check. Done (round 3).** One TypeScript and one Java file, controls and key points frozen first. | Result: the Rust finding does not generalise as one rule; see Conclusions. |
| 2 | **Paged-read check. Done (round 4), lenient case.** Paged raw (TypeScript, 3 pages) 78.8 against 78 single-read; paged summaries (`spec.rs`, 4 pages) 70.2 against 68. | Passed the pre-set bar (within 8 points). Not tested: a writer that must forget earlier pages; run only if the real loop sheds them. |
| 3 | **Find the crossover. Done for Rust (round 5).** `rust.rs` at 12.4 times the cap: paged raw 72.6 against paged summaries 58.4. | Starting rule: paged raw up to about 12 pages, summaries beyond; the true cutoff lies between about 12 and 22. A mid-size file (about 16 to 20 times the cap) in another stack would tighten it; optional. |
| 3b | **Drop test code before paging (Rust). Done (round 6).** Paged raw without the `#[cfg(test)]` module: 76.2 against 75.0 with it; 8 pages against 14; about 62k tokens against about 84k. | Quality held, and pages fell 43 %: build it for Rust. The pack knows its own test-module convention. |
| 4 | **Design and tests.** File task shape per the ladder in Conclusions, paged with a cursor (stateless, like `pending`); a Java member outline built deterministically from the extractor's own member data. Failing tests first, including a rewrite of the `mcp.rs` lifecycle assertion `leaf_body_edit_stales_exactly_its_file_and_containing_rollup`. | Approved plan, own branch. |
| 5 | **Fingerprint (rule revised after round 7).** One shape + doc comments + literals fingerprint for every file, independent of which input rung the writer used (see Conclusion 14). New hash field, so `FORMAT_VERSION` bump with its doc-comment entry; guarded migration of stored file hashes or regeneration; a per-pack literal rule (Rust first, then the other packs' literal node kinds). Comes after the import-hash fix (0c) and reuses its migration tool (0d). The choice among the three candidate rules in Conclusion 14 is still the owner's. | Measured on the comment-only commit scenario (corpus 100 % to 40.2 % current today) and on the history probe (about 21 % of edits spared). |
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

### `src/rust.rs` (Rust, this repo; round 5)

Control summary (hand-written):

> This file is the Rust pack's reader for one Rust source file, doing two jobs. First it uses a parser (tree-sitter) to list the file's top-level items, plus one level into impl, trait, inline module and struct bodies: functions and macros become callables, structs, enums, unions, traits, impls and modules become containers, consts, statics, type aliases and named struct fields become values, with the real keyword kept. It is deliberately shallow: function bodies, enum variants and nested functions are not declarations, a tuple struct's positional fields are not extracted, and a `#[cfg(test)]` module is skipped whole. Each `impl Foo` block is folded into the `Foo` it belongs to, so a type reads as one unit: methods move under the type, hashes are chained so an edit to a method counts as an edit to the type, and several impl blocks collapse in order. A trait impl folds in only one narrow case: a type with no fields, no inherent methods and exactly one trait impl in the file; otherwise it stays separate. For exported types the public shape, meaning public fields, trait method signatures and public methods, feeds the interface hash that decides when dependants go stale. Second, it reads `use` statements (a `pub use` is a re-export, an alias keeps the source name, braced lists are flattened one level, globs and nested lists are skipped), also finds `crate::` paths written inline without a `use`, and resolves each module path to a file by filesystem convention (`a::b` is `a/b.rs` or `a/b/mod.rs`), handling `crate`, `self`, `super` and library names in the repo, then follows one `pub use` hop. It adds an edge from a trait impl to a type defined in the same file, since no `use` would.

| # | W | Point |
|---|---|---|
| 1 | M | Purpose: the Rust language pack's reader for one source file, doing two jobs: extracting the file's items (symbols) with a tree-sitter parse, and extracting and resolving its `use` imports. |
| 2 | M | Kind mapping: fn and macro_rules become callables; struct, enum, union, trait, impl and mod become containers; const, static, type alias and named struct fields become values; the concrete keyword is kept alongside the kind. |
| 3 | M | Deliberately shallow: top-level items plus one level into impl, trait, inline mod and struct bodies; function bodies, enum variants and nested fns are not declarations; a tuple struct's positional fields are not extracted; a `#[cfg(test)]` module is skipped entirely. |
| 4 | M | Inherent impl folding: each `impl Foo` block is folded into the `Foo` this file defines (methods reparented onto it with rewritten ids, source hashes chained so any member edit changes the type's hash, attribute markers merged, the block dropped); several impls collapse in source order; impls on a type the file does not define are left alone; a method colliding with a field name gets a disambiguated id. |
| 5 | M | A trait impl folds only in one narrow shape: the type has no fields, no inherent-impl methods, and exactly one trait impl in the file; any other shape leaves trait impls as their own symbols; the check is per file only (a known, logged limitation). |
| 6 | M | Public surface feeding `interface_hash`: for an exported container, public fields (value stripped for const/static, kept for a type alias), a trait's method signatures, and the public methods of folded inherent impls (every method of a folded trait impl); an unexported type has no interface hash. |
| 7 | M | Per-symbol details: the signature is the text before the body; leading doc comments (`///`, `//!`, `/** */`) are collected only if adjacent, with plain comments and attributes stepped over; attributes are kept verbatim; `pub`, `pub(crate)` and `pub(super)` count as exported, and a member inside an impl or trait never does. |
| 8 | M | Import extraction: a `use` is an import, a `pub use` a re-export, an alias tracks the source name, braced lists flatten one level, globs, `self` and nested lists are skipped; `crate::`-rooted paths written inline with no `use` are also found by walking the whole tree. |
| 9 | M | Import resolution: a module path becomes a file by filesystem convention (`crate::a::b` is `a/b.rs`, `a/b/mod.rs`, then `lib.rs`/`main.rs` in it), not by reading `mod` declarations; handles `crate` (the file's own crate root), `self`, `super` and names of library crates in the repo; the item is then looked up as a top-level symbol, following one hop through a `pub use`. |
| 10 | N | Inline `crate::` references only add an edge when they resolve to a real, new target: not a same-file target, and not one an explicit `use` already covers. |
| 11 | N | A same-file trait impl gets a synthesized impl-to-type edge (labelled "same file"), because no `use` statement would create one. |
| 12 | N | Deterministic and bounded: files are processed in sorted order; external crates (`std`, `anyhow`) stay unresolved; an unresolvable module path gives no edge. |

Must NOT claim: that it calls a language model or writes prose; that it parses function bodies, or extracts enum variants or nested functions as symbols; that it reads `mod` declarations to find module files; that it resolves external crates (`std`, `serde`) to files; that every trait impl is folded into its type.

## Appendix C — how to rerun

The inputs, candidates, judge outputs and the label-to-arm key lived in a session scratchpad and are not in the repo. To rerun: rebuild the B input from `docs/specs/<file>.md` (module doc from the source file, then each symbol section's signature and `### Summary`, dropping the top-level `## Summary`), build A with the cap rule in `cap_generation_text`, write one cold agent per run with the prompt wording in "Design", shuffle candidates under fresh labels, and judge with the key points above. For a new file, write the control and key points first and freeze them before generating anything.
