# Experiment 01 — Discrepancy-feature removal

Tests the core CodeOwl hypothesis (`REVIEW.md` gap #4) on a real task, before any code is written.

**Target repo:** `/home/sidd/dev/startup/talentTrail` — Next.js 16, 307 TS/TSX files, 50,350 lines.
**Task:** understand and plan the removal of the two-judge scoring-discrepancy flagging feature.
**Why this task:** ~20 files across API / portal UI / lib / SQL schema / unit + e2e tests; removal is
impact-analysis shaped; a `NOT NULL` schema coupling and a `resolution_status` state machine mean the
hard part is *consequences*, not *location*; and the author is the ground-truth oracle.

## What is actually being tested

| | Claim | This experiment |
|---|---|---|
| **H1** | An agent will *use* specs rather than defaulting to Grep/Read | **Primary** — hard binary gate |
| **H2** | A spec beats source on tokens for files you must know but not edit | Measured, n=1 (directional only) |
| **H3** | Generated specs are accurate enough to act on | **Primary** — via factual-error count |
| **H4** | Specs stay fresh | Not tested — that's `REVIEW.md` gaps #1/#2 |

## Files

- `PROMPT.md` — the verbatim task prompt and run discipline. Identical across all runs.
- `GROUND-TRUTH.md` — **fill in and seal before Run A.** The oracle and the scoring rubric.
- `runs/<run-id>/` — per-run output: `plan.md` (agent's final answer), `metrics.md`, session id.

## Protocol

**Step 0 — seal the oracle.** Fill in `GROUND-TRUTH.md` from your own memory and code, including
section F (pre-registered prediction). Do this before anything else.

**Step 1 — Run A (baseline, no specs).** Fresh session in talentTrail, paste `PROMPT.md` verbatim.
Save the final plan. Then mine the transcript from
`~/.claude/projects/-home-sidd-dev-startup/*.jsonl` for: every `Read`/`Grep`/`Glob`/`Task` call, bytes of
source pulled into context, token usage, turn count.

**Step 2 — derive the spec template from evidence.** For each file the agent read, ask: *what fact was
it actually after?* That list — not intuition — defines what a spec must contain. Cross-check it against
`GROUND-TRUTH.md` §E, which is the same question asked from the other direction.

**Step 3 — write specs** for the files Run A touched, into `docs/specs/` in talentTrail (mirrored tree,
per `REQUIREMENTS.md`). **Constraint:** write each file's spec from *that file's source plus its children's
specs only* — no repo-wide view. Otherwise you are validating a system you cannot build. Add one line to
talentTrail's `CLAUDE.md` pointing at `docs/specs/`.

**Step 4 — Run B.** Fresh session, same prompt verbatim, specs present. Same measurements.

**Step 5 — score both** against `GROUND-TRUTH.md` §G and compare.

## Reading the result

- **Agent ignored the specs** → H1 fails. The problem is discoverability/trust, not generation. Fix that
  (CLAUDE.md phrasing, hooks, tool description) before building anything, or the project has no path.
- **Used them, no cost savings** → template problem, learned for ~$0. Iterate on spec content, re-run.
- **Used them, cheaper, but more factual errors or missed §E couplings** → **negative result.** A confidently
  wrong spec is worse than no spec. This is the outcome most likely to be misread as success.
- **Used them, cheaper, same or better correctness** → hypothesis live, and you now have an evidence-derived
  spec template to build the pipeline around. Then fix `REVIEW.md` gaps #1–#3 and start writing code.

## Known limits of this experiment

- **n=1.** The cost delta is directional, not a measurement. H1 and H3 are answerable at n=1; H2 is not.
- **The prompt withholds the codebase's vocabulary.** The feature is described by behavior; the word
  "discrepancy" — which `grep -ril discrepan` would resolve in a single call — never appears. This is
  realistic (you genuinely would describe it this way months later) but it does make the test *more*
  favourable to CodeOwl than a term-of-art prompt would be, since concept→vocabulary bridging is a gap
  specs close well. Score the discovery win and the consequences win separately, and weight consequences
  higher: "re-evaluation" is still a near-match for `lib/reeval.ts`, so discovery is slowed, not blocked,
  and a discovery-only win would not generalise to features whose names you *do* remember.
- **Author-written specs are an upper bound.** Even under the Step 3 constraint, you know things the
  pipeline's LLM will not. Treat Run B as a ceiling on what a real implementation would achieve.
