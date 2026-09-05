# Experiment 01 — Ground Truth (fill in BEFORE any run)

> You wrote this feature, so you are the oracle. **Seal this file before running anything.**
> If you fill it in after seeing a run's output, you will grade leniently without noticing — that is
> the single most common way an experiment like this produces a false positive.
>
> Commit this file (or just note the timestamp) before Run A starts.

**Filled in on:** `2026-09-05`  **By:** you, via a dedicated code-reading research pass (grep + file reads +
schema/RLS inspection) — a separate pass from Run A/Run B, not one of the scored sessions itself.

---

## A. What the feature actually does, end to end

Write it as if explaining to a new developer. This doubles as the "correct answer" for prompt item 1,
and — importantly — it is a first draft of what a **spec** for this feature would have to contain.

- **Trigger — when does discrepancy detection run?**
  Not a cron job — synchronous, fire-and-forget, inside two judge-write API routes, via
  `lib/artwork-result-service.ts`:
  - `POST /api/judge/evaluations/submit` → `upsertArtworkResultIfBothSubmitted()` — runs after every
    submission, no-ops unless both judges have `status = 'Submitted'`.
  - `PUT /api/judge/evaluations/[evaluationId]` (editing an already-submitted evaluation) →
    `recalculateArtworkResultIfResolvable()` — bails immediately if `artwork_results.resolution_status
    !== 'none'`, so it can never disturb a row already mid-reeval/resolution.
  Errors are only `console.error`'d, never thrown — an evaluation save is never rolled back because of a
  discrepancy-calc failure.

- **The rule — what counts as a discrepancy?** (thresholds, per-parameter vs. total, `low`/`medium`/`high` bands)
  `lib/scoring.ts`: `DISCREPANCY_LOW_MAX = 20`, `DISCREPANCY_MEDIUM_MAX = 40` → low = [0,20), medium =
  [20,40], high = (40,∞). **Per-criterion, not on total score** — `perCriterionDiscrepancy()` computes
  `avg(|j1[i] - j2[i]|)` across matched criteria, specifically so two judges scoring in opposite
  directions per-criterion but landing on the same average still get flagged.
  **Undocumented inconsistency:** the re-evaluation round uses a *different* formula — a plain
  `|judge1_avg - judge2_avg|` on overall scores, reusing the same low/medium/high bands. Initial
  detection = per-criterion; reeval discrepancy = straight score diff. Real, shipped, not documented
  anywhere.
  Score handling by level (`buildResult()`): low/medium → `resolution_status: 'none'`, final score =
  `(j1+j2)/2` immediately. high → `resolution_status: 'pending_reeval'`, `final_score = null` (held).

- **What happens when one is flagged?** (`resolution_status` transitions: `pending_reeval` → ? → ?)
  No separate flag table — `artwork_results` *is* the queue. On upsert it writes judge1/2_id,
  judge1/2_score, raw_final_score, final_score, discrepancy_pct, discrepancy_level, scoring_method,
  resolution_status, and stamps `reeval_requested_at` only when status becomes `pending_reeval`.
  Judge-facing pages read `resolution_status = 'pending_reeval'` (via `needsReevalForJudge()` in
  `lib/judge-utils.ts`) to show that judge a re-evaluation prompt.

- **Who resolves it, and how?** (judge re-eval path vs. admin resolve path — when does each apply?)
  Two stages.
  **Stage A — judge re-eval** (`POST /api/judge/evaluations/[evaluationId]/reeval`): each of the two
  original judges independently resubmits scores. `applyJudgeReeval()`:
  - one judge done → records their score, stays `pending_reeval`;
  - both done, new diff ≤40 → `resolution_status: 'resolved'`, `scoring_method: 'reeval_average'`;
  - both done, new diff >40 → `resolution_status: 'pending_admin'`, score stays null.
  **Stage B — admin resolution** (`POST /api/admin/.../discrepancies/[registrationId]/resolve`,
  admin-only, only valid from `pending_admin`): two methods — `accept_average` (average of post-reeval
  judge scores), `manual_override` (requires a 0–100 `overrideScore` + non-empty notes).
  Note: the planning docs describe a third option, "assign a third judge" (Story 9.2.2) — never built.
  Only these two exist in code.

- **What does resolution change?** (final score recomputed? original scores kept? audit trail?)
  Reeval route writes judge1/2_score, raw_final_score, final_score, discrepancy_pct, discrepancy_level,
  scoring_method, resolution_status, reeval_judge1/2_completed. Admin resolve writes raw_final_score,
  final_score, scoring_method, resolution_status, resolution_method, resolution_override_score,
  resolution_notes, resolved_by, resolved_at — guarded by `.eq("resolution_status","pending_admin")` so a
  stale double-submit can't clobber it. Final-score computation is pure application code — no DB
  trigger/function derives it.

- **What still needs to be true after removal?** (the two-judge rule itself presumably stays — what else?)
  **Not directly addressed by the research pass — inferred, needs confirmation before sealing:** the
  two-judge submission gate and the plain `(j1+j2)/2` averaging for the non-discrepant case (§A rule,
  low/medium) presumably survive; only the detection/flagging/reeval/admin-resolution machinery goes.
  Confirm this before treating it as ground truth.

---

## B. Files that must change

A naive `grep -ril "discrepan|variance|deviation|flag"` returns the list below. **Mark each one**, and
add anything grep missed. `MUST` = must change to remove the feature. `NO` = matched but not actually
part of this feature (false positive). `?` = partially involved / judgment call.

| File | MUST / NO / ? | What has to happen to it |
|---|---|---|
| `lib/reeval.ts` | MUST (whole) | Delete — `applyJudgeReeval()` |
| `lib/scoring.ts` | MUST (whole) | Delete — thresholds, `perCriterionDiscrepancy()`, `buildResult()` |
| `lib/artwork-result-service.ts` | MUST (whole) | Delete — `upsertArtworkResultIfBothSubmitted()`, `recalculateArtworkResultIfResolvable()`; table exists only for this feature |
| `lib/reeval.test.ts` | MUST (whole) | Delete with `reeval.ts` |
| `lib/scoring.test.ts` | MUST (whole) | Delete with `scoring.ts` |
| `app/api/admin/competitions/[id]/discrepancies/route.ts` | MUST (whole) | Delete |
| `app/api/admin/competitions/[id]/discrepancies/[registrationId]/resolve/route.ts` | MUST (whole) | Delete — `accept_average` / `manual_override` |
| `app/api/judge/evaluations/[evaluationId]/reeval/route.ts` | MUST (whole) | Delete |
| `app/api/test/advance-discrepancy/route.ts` | MUST (whole) | Delete (dev-only test helper, 404s outside dev mode already) |
| `app/api/test/update-evaluation-scores/route.ts` | MUST (whole) | Delete (dev-only test helper) |
| `app/portal/admin/competitions/[id]/discrepancies/page.tsx` | MUST (whole) | Delete |
| `app/portal/admin/competitions/[id]/discrepancies/discrepancy-table.tsx` | MUST (whole) | Delete |
| `app/portal/admin/competitions/[id]/discrepancies/[registrationId]/page.tsx` | MUST (whole) | Delete |
| `app/portal/admin/competitions/[id]/discrepancies/[registrationId]/resolve-form.tsx` | MUST (whole) | Delete |
| `app/portal/admin/competitions/[id]/discrepancy-summary.tsx` | MUST (whole) | Delete |
| `e2e/lifecycle/07-admin-review.spec.ts` | MUST (whole) | Delete |
| `e2e/lifecycle/08-results-teardown.spec.ts` | MUST (whole) | Delete — hard-asserts `finalScore` is never null; the assertion most likely to surface stuck rows |
| `app/portal/admin/competitions/[id]/page.tsx` | PARTIAL | Remove the `<DiscrepancySummary>` embed only |
| `components/judge/evaluation-form.tsx` | PARTIAL | Remove reeval submit path, banner, button label/target, deadline bypass |
| `components/judge/artwork-grid.tsx` | PARTIAL | Remove reeval badge/banner/tab (tab already disappears when `reevalCount === 0`) |
| `e2e/helpers/api-client.ts` | PARTIAL | Remove only `getDiscrepancies`/`resolveDiscrepancy` helpers — file has other, unrelated helpers |
| `supabase/schema.sql` | MUST | Drop the `artwork_results` table entirely — not a column-level edit (see §C) |
| `lib/analytics-dashboard.ts` | **? — not yet confirmed** | Matched original grep; not addressed in the research pass above — check before sealing |
| `lib/analytics-dashboard.test.ts` | **? — not yet confirmed** | Same as above |
| `lib/data-tools.test.ts` | **? — not yet confirmed** | Same as above |
| `lib/types/database.ts` | **? — likely MUST** | Presumably Supabase-generated types incl. `artwork_results`; needs regen after table drop — not explicitly confirmed |
| `components/ui/{label,button,alert,badge}.tsx` | NO | Confirmed false positive — matched only on the word "flag" |
| **— surfaced only by reading code, missed by the original grep pattern —** | | |
| `app/api/judge/evaluations/submit/route.ts` | PARTIAL | One call to `upsertArtworkResultIfBothSubmitted()` |
| `app/api/judge/evaluations/[evaluationId]/route.ts` | PARTIAL | One call to `recalculateArtworkResultIfResolvable()` |
| `app/portal/judge/competitions/[id]/page.tsx` | PARTIAL | `reevalMap`/`reevalCount`/`filter === "reeval"` logic |
| `app/portal/judge/competitions/[id]/evaluate/[registrationId]/page.tsx` | PARTIAL | `isReeval` computation + `displayScores` branch — **queried on every page load**, not just reeval cases |
| `components/judge/evaluation-form-client.tsx` | PARTIAL | Reeval submit path, banner, button label/target, deadline bypass |
| `components/judge/artwork-card.tsx` | PARTIAL | Badge/banner |
| `components/judge/artwork-status-tabs.tsx` | PARTIAL | Tab |
| `lib/judge-utils.ts` | PARTIAL | Just `needsReevalForJudge()` |
| `lib/test-cleanup-utils.ts` | PARTIAL | `artwork_results` entry in cleanup ordering |
| `app/api/test/cleanup-competitions/route.ts` | PARTIAL | `artwork_results` entry in cleanup ordering |
| **+ anything still missed →** | | |

> **Count check, unresolved:** the research pass said "19 files total," but the full breakdown above
> (whole + partial + code-surfaced) lists 31 distinct paths. §G's recall/precision need one agreed
> denominator — reconcile this before sealing the file.

---

## C. Database / schema changes required

Known coupling in `supabase/schema.sql`:
`artwork_results.discrepancy_pct` (NOT NULL), `artwork_results.discrepancy_level` (NOT NULL),
`artwork_results_discrepancy_level_check`, `idx_artwork_results_discrepancy`, `idx_artwork_results_pending`,
and `resolution_status`.

- **Drop the columns, or leave them and stop writing?**
  Drop the **whole table** — nothing else reads from `artwork_results`. Every column on it is either
  feature-only (`scoring_method`, `discrepancy_pct`, `discrepancy_level`, `reeval_requested_at`,
  `reeval_judge1/2_completed`, `resolution_status`, `resolution_method`, `resolution_override_score`,
  `resolution_notes`, `resolved_by`, `resolved_at`), dead (`reeval_original_judge1/2_score` — present in
  schema, no read/write anywhere in code), or general-shaped but only used by this feature in practice
  (`judge1/2_id`, `judge1/2_score`, `raw_final_score`, `final_score`, `created_at`, `updated_at`).
  Column-level surgery isn't worth it when the table has no other purpose.
- **Does `resolution_status` survive removal, or does it only exist for this feature?**
  Only exists for this feature — goes with the table.
- **Does a migration need to run before the code deploys, or after?**
  **After.** No views/triggers depend on the table, but judge pages query it unconditionally on every
  page load (see §E). Dropping first would 500 every judge page load until the code deploy landed.
- **Anything in RLS policies / views / triggers / functions that references these?**
  - RLS: "Admins have full access to artwork_results" (full CRUD); "Judges can view results for their
    artworks" (SELECT only, where they're judge1/2). No judge *write* policy — judge writes during reeval
    go through the service-role client, bypassing RLS entirely.
  - **No triggers** on this table at all (unlike almost every other table) — `updated_at` is set
    manually everywhere.
  - No views select from it.
  - **One SQL function does:** `cleanup_e2e_participants()` has a
    `DELETE FROM artwork_results WHERE registration_id = ANY(...)` branch, independent of the JS-side
    cleanup ordering — easy to miss since it lives in SQL, not TypeScript.
  - 3 indexes, including a partial index `idx_artwork_results_pending` keyed to the literal values
    `'high'`/`'pending_reeval'`/`'pending_admin'`.
  - 5 FK constraints (to `competitions`, `portal_users` ×3, `registrations`), none cascading — deletion
    order matters.

---

## D. In-flight data & state decisions

- **Rows currently sitting in `pending_reeval` / `pending_admin` — what happens to them?**
  Stuck permanently with `final_score = null` — no timeout or fallback exists. If the judge-reeval stage
  is removed but admin-resolve is kept, no row can ever reach `pending_admin` again through production
  code (only the dev-only `advance-discrepancy` route can force it, and that 404s outside dev mode).
  Practically: **resolve or manually override every non-`none`/`resolved` row before removing the code**,
  or run a one-time migration that force-resolves whatever's left (e.g.
  `resolution_status = 'resolved'`, `final_score = (j1+j2)/2`).
- **Do any artworks end up with no final result if flagging disappears mid-flight?**
  Yes — the sharpest risk. `final_score` is null for any row still `pending_reeval` or `pending_admin`.
  `08-results-teardown.spec.ts` hard-asserts `finalScore` is never null for a "final results"
  competition — any results/ranking feature reading `final_score` would silently produce incomplete
  rankings for artworks stuck mid-flight. No automatic fallback average on timeout.
- **Is there a live competition where this matters right now?**
  Can't be determined from the repo alone — needs a live DB query, not static code. The repo has no
  production data, only E2E fixtures. `feedback-proofread/DESIGN.md` shows a `psql` check against the
  live DB on 2026-09-03 (3 evaluations, 1 competition, 1 judge, all `Submitted`, none flagged yet since
  scores weren't computed against another judge) — that snapshot had only one judge's evaluations for the
  competition, so `artwork_results` likely has 0 rows currently (it only populates once both judges
  submit). **Open action:** confirm with a fresh
  `SELECT * FROM artwork_results WHERE resolution_status != 'resolved'` before touching this, rather than
  trusting a two-day-old snapshot.

---

## E. Non-obvious couplings — *the most important section*

Things a **name-based search cannot find**. This section is the real payload of the experiment: it is
the list of facts a spec would have to carry in order to beat grep. Be specific.

- **String-literal / ORM references** (e.g. `.select('discrepancy_level')`, a status string, a column
  name built dynamically) that don't look like a symbol reference:
  The partial index `idx_artwork_results_pending` is keyed to the literal values `'high'`,
  `'pending_reeval'`, `'pending_admin'` — invisible to a symbol-based search. Separately,
  `reeval_original_judge1_score`/`reeval_original_judge2_score` exist in the schema but have **zero**
  code references anywhere — a schema/code mismatch only visible by reading both sides, not something
  grep on the codebase alone would ever surface.
- **Semantic coupling** — code that has no matching identifier but breaks anyway (e.g. score aggregation
  that silently assumes a resolution step ran; a UI count that would go to zero; an analytics number that changes):
  - Once a row leaves `resolution_status = 'none'`, ordinary evaluation edits (PUT) can never update the
    score again — only reeval/admin-resolve routes can write to it. Removing those routes while leaving
    the recalculate-gate in place would silently **freeze those rows forever**.
  - `discrepancy-summary.tsx`, embedded directly on the main admin competition page, would start
    rendering a permanent "No scored artworks yet" zero-state even for a fully-judged competition if the
    backend is removed but the widget isn't — a misleading regression, not a crash.
  - The judge evaluate page and the judge grid page both **query `artwork_results` on every single page
    load**, not just for reeval cases. Drop the table/columns without updating these two pages and every
    judge evaluation page load 500s.
- **Code that exists *only* because of this feature** but isn't named after it (helpers, fixtures,
  seed data, the `app/api/test/*` routes):
  `lib/test-cleanup-utils.ts` and `app/api/test/cleanup-competitions/route.ts` both carry an
  `artwork_results` entry in cleanup ordering; `cleanup_e2e_participants()` (SQL function) independently
  deletes from the same table. None of these are named after "discrepancy" or "reeval" — they're
  infrastructure that exists because this feature's table exists.
- **Ordering constraints** — anything that must be removed in a specific order to avoid a broken state:
  1. Resolve or force-resolve all outstanding `pending_reeval`/`pending_admin` rows first (data step,
     before any code/schema change).
  2. Removing the reeval API route while the evaluate page still computes `isReeval: true` from a stale
     query → judges hit 404 on submit. Removing page-side detection first (making `isReeval` always
     false) while stuck rows remain in `pending_reeval` → those rows become permanently unreachable, no
     UI path can ever resolve them.
  Cleanest order: (a) force-resolve pending rows via migration/manual script → (b) remove UI branches
  (evaluate page, grid page, form, card/tabs) → (c) remove API routes → (d) remove
  `lib/reeval.ts`/`scoring.ts` discrepancy logic → (e) drop the table, RLS policies, indexes, and the
  `cleanup_e2e_participants()` SQL branch **together** (they must move in lockstep or E2E cleanup starts
  throwing on a dropped table) → (f) remove the `test-cleanup-utils.ts` entry and the two dev-only test
  routes → (g) delete the two lifecycle E2E specs (07, 08) or gut their discrepancy assertions.
  Migration timing: the table drop must happen **after** the code deploy that stops querying it, not
  before (see §C).
- **The thing you'd most expect a competent stranger to get wrong here:**
  Two candidates, both plausible: (1) the undocumented formula inconsistency between initial detection
  (per-criterion average) and reeval (plain overall-score diff) — nothing in the code flags this as
  intentional vs. a bug, so a remover might "fix" it instead of just deleting it; (2) dropping the table
  before the code deploy lands, because nothing else in the schema (no views/triggers) signals that the
  app still queries it on every judge page load.

---

## F. Pre-registered prediction

Committing this before the run keeps you honest about what counts as success.

- The prompt describes the feature *by behavior*, never using the codebase's own word ("discrepancy"),
  so the agent must bridge concept → vocabulary. I expect the specs to help with *locating* code:
  **not at all / a little / a lot**: `a lot`
- Caveat to hold in mind: "re-evaluation" in the prompt is a near-match for `lib/reeval.ts`, so a
  baseline agent still has a grep foothold. Predict how many tool calls Run A needs before it first
  hits the word "discrepancy": `2 to 4`
- I expect the specs to help most with: `understanding the feature, finiding the files where the code is`
- I will consider the hypothesis **supported** if: `Run B's reads touching docs/specs > 0 (H1 — it used them at all), and Run B's factual-error count is ≤ Run A's, and Run B matches or beats Run A on §E couplings surfaced (formula inconsistency, dead columns, per-page-query risk, ordering constraints) — regardless of whether Run B used fewer tokens.`
- I will consider it **falsified** if: ` docs/specs reads are 0 (agent ignored them — H1 fails outright, and the rest doesn't matter), or Run B is cheaper/faster but introduces even one factual error Run A didn't make, or Run B misses a §E coupling that Run A found unaided.`

---

## G. Scoring rubric (apply identically to every run)

| Metric | How to compute |
|---|---|
| **Recall** | (§B `MUST` items the run identified) / (total §B `MUST` items) |
| **Precision** | (correctly identified items) / (all items the run claimed need changing) — punishes shotgun answers |
| **Couplings found** | count of §E items surfaced unprompted, out of total §E items |
| **Decisions surfaced** | count of §C + §D questions raised unprompted, out of total |
| **Factual errors** | count of statements about behavior that are **wrong**. Weight these heavily — a confidently wrong claim is worse than a gap, and is the main risk of a stale spec |
| **Exploration cost** | file-read tool calls, and total bytes of source pulled into context (mined from the transcript) |
| **Token cost** | `input_tokens + cache_creation_input_tokens` summed over the session |
| **Turns** | assistant turns to reach the final plan |

**Cost metrics are secondary.** If Run B is cheaper but has more factual errors or misses §E couplings,
that is a *failure*, not a win.
