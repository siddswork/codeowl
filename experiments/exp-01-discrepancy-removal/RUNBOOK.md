# Experiment 01 — Runbook

> **This session cannot run the experiment.** My context already contains the answer — I have grepped
> the feature, read the schema coupling, and listed the affected files. Any run from here measures a
> contaminated agent. Run A and Run B must each happen in a **fresh Claude Code session started from
> the talentTrail repo**, with no prior context. My role is before (design) and after (mining, scoring).

---

## Step 0 — Seal the oracle *(you, ~30 min, once)*

1. Fill in `GROUND-TRUTH.md` completely — including §E (non-obvious couplings) and §F (prediction).
2. Note the timestamp at the top. Do not edit it again after Run A starts.

Do not skip §A. It is the answer key, and it is also your first worked example of what a spec
looks like when written by someone who knows the code.

## Step 1 — Pre-flight *(2 min)*

```bash
cd /home/sidd/dev/startup/talentTrail
git status --short          # expect only: AM tools/feedback-proofread/DESIGN.md
git branch --show-current   # note it; must be identical for Run B
ls docs/specs 2>/dev/null   # must NOT exist for Run A
```

Do not commit anything between now and the end of Run B.

## Step 2 — Run A (baseline, no specs)

```bash
cd /home/sidd/dev/startup/talentTrail
claude
```

In that **new** session:

1. Paste the prompt block from `PROMPT.md` **verbatim**. Nothing before it, nothing after it.
2. If the agent stalls or asks to proceed, reply only `continue`. Never add information.
3. If it asks a clarifying question, answer only from the Background paragraph already in the prompt.
4. Let it finish. It must not edit files — if it tries, decline the permission.
5. Copy its final plan to the clipboard (you'll paste it into `runs/A/plan.md` — the script extracts
   the last text block automatically, but paste the full thing if it looks truncated).
6. Note the session id (`/status` in that session shows it).

Then, back in **this** session or any shell:

```bash
cd /home/sidd/dev/openSource/codeowl/experiments/exp-01-discrepancy-removal
python3 mine.py --list                    # confirm the newest transcript is your run
python3 mine.py --run A                   # or: --run A --session <uuid>
```

Writes `runs/A/metrics.md`, `runs/A/files-read.md`, `runs/A/plan.md`.

## Step 3 — Derive the spec template *(together)*

Open `runs/A/files-read.md` and fill the last column: for each probe, *what fact was the agent after?*
That list is the evidence-derived spec template. Cross-check it against `GROUND-TRUTH.md` §E, which asks
the same question from the other direction.

## Step 4 — Write the specs

Into `talentTrail/docs/specs/` as a mirrored tree, for the files Run A touched.

**Hard constraint:** write each file's spec from *that file's own source plus its children's specs only*
— no repo-wide view, no cross-file search. Otherwise you are validating a system you cannot build
(see `../../REVIEW.md` gaps #1–#2 for why the real pipeline cannot do better).

Add one line to `talentTrail/CLAUDE.md` pointing at `docs/specs/`. Keep it neutral — "Specs for this
codebase live in docs/specs/" — not "always read the specs first", which would rig H1.

## Step 5 — Run B

Identical to Step 2, fresh session, same verbatim prompt, specs now present.

```bash
python3 mine.py --run B
```

## Step 6 — Score *(together)*

Fill the manual rows in both `metrics.md` files from `GROUND-TRUTH.md` §G, then compare. Read the result
per `README.md` → "Reading the result". Remember: **cheaper but wronger is a failure.**

---

## What the script measures

| Metric | Source |
|---|---|
| Exploration tool calls | `Read`/`Grep`/`Glob` + `Bash` whose command matches `grep\|rg\|find\|cat\|head\|tail\|sed\|awk\|ls\|wc\|tree\|fd` |
| Source pulled into context | byte/line count of those tool results — the direct measure of what CodeOwl claims to reduce |
| New content into context | `input_tokens + cache_creation_input_tokens` (cache_read excluded — it is re-reading what is already resident, ~10x cheaper, and not evidence of waste) |
| Distinct paths / probes | one row per file, and per Bash command, in `files-read.md` |
| **Discovery probe** | the tool-call ordinal at which `discrepan*` first appears anywhere. Isolates the concept→vocabulary bridging component from the consequences component |
| **H1 probe** | count of reads touching `docs/specs`. Zero in Run B means the agent ignored the specs and the hypothesis fails at the first gate |
| Subagents | `Agent` calls and sidechain turns, so delegated exploration is not silently uncounted |

Validated against a real 2.3 MB transcript: 125 exploration calls, 156.6 KB pulled, 109 distinct probes,
vocabulary first appearing at call #24.
