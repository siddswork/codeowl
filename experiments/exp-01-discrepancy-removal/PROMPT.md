# Experiment 01 — Task Prompt (verbatim, identical for all runs)

> Paste the block below **exactly as-is** into a fresh Claude Code session in `/home/sidd/dev/startup/talentTrail`.
> Do not add hints, do not mention CodeOwl or specs, do not answer follow-up questions with extra context beyond
> what a normal user would give. If the agent asks a clarifying question, answer only from the Background below.

---

```
I want to remove a feature from the judging flow.

Background: every artwork is evaluated by two judges. I added logic that detects when
the two judges give very different scores on the same parameter, flags those cases, and
triggers a re-evaluation / review process for them.

I want that whole thing gone. Before I change anything, I want to understand what's
actually there.

Produce a written removal plan covering:
1. What the feature currently does, end to end
2. Everything that has to change to remove it
3. Anything that makes removal risky, or that I'll have to decide

Do not edit any files. This is read-only — I want the plan first.
```

---

## Why the prompt is shaped this way

- **It's what you'd actually type — described by behavior, not by name.** The codebase's own term
  ("discrepancy") is deliberately absent. This is not artificial vagueness: it's how anyone describes a
  feature they built months ago without recalling its internal vocabulary. It forces the agent to bridge
  *concept → codebase vocabulary*, which is precisely the gap a spec layer is supposed to close.
- **Note the residual leak.** "re-evaluation" is a near-match for `lib/reeval.ts`, so an agent grepping
  `reeval` still gets a foothold, one hop from there to the discrepancy vocabulary. Discovery is harder
  than with the term-of-art name, not blocked. Leave it — pushing the wording further from the code
  ("when two judges disagree, something happens") would start to read as a puzzle rather than a request.
- **It does not leak the answer.** No mention of the database, `NOT NULL` columns, `resolution_status`,
  in-flight rows, tests, or any file path. Discovering those is the agent's job and is the thing being measured.
- **Item 3 is deliberately open-ended.** "Anything risky or that I'll have to decide" gives the agent
  a fair opening to surface the migration and in-flight-state problems without being told they exist.
- **Read-only** so the run is repeatable in a fresh session with an identical starting tree.

## Run discipline

| | |
|---|---|
| Repo | `/home/sidd/dev/startup/talentTrail` |
| Branch | same branch for every run; `git status` must be identical at the start of each |
| Session | **fresh session per run** (`/clear` is not enough if context was built — start a new session) |
| Edits | none, in any run |
| Follow-ups | if the agent stalls, reply `continue` only — never add information |
| End of run | save the agent's final plan to `runs/<run-id>/plan.md`, note the session id |

Do not commit anything between runs. The staged `tools/feedback-proofread/DESIGN.md` is unrelated —
leave it exactly as it is so both runs start from the same tree.
