# CodeOwl — Roadmap (Working Draft)

> Companion to `ARCHITECTURE.md` (how) and `REQUIREMENTS.md` (what/for whom). This document is the *when* and *in what order* — a walking-skeleton sequence where each milestone is independently runnable, has a concrete pass/fail test, and adds real MVP value rather than being scaffolding for its own sake. Pending trims from `CLAUDE.md` (tool surface, commit hygiene) get resolved as they're hit, not decided up front.

## Test repos

Fixed set of real repos the milestone validations below run against — chosen for language coverage and for actually stressing specific open questions, not just for being famous. Not part of CodeOwl's own repo; live as siblings on disk.

| Repo | Path | Language | Size | Why this one |
|---|---|---|---|---|
| talentTrail | `~/dev/startup/talentTrail` | TypeScript/TSX | 307 files | The Phase 1 pilot repo itself (Next.js 16, Supabase, Upstash — matches `REQUIREMENTS.md`'s pilot description exactly). Primary target for every Phase 1 milestone. |
| memolink | `~/dev/openSource/inspirations/memolink` | Java (+ a little Python) | 45 Java files, Maven multi-module | The project CodeOwl is architecturally inspired by. Multi-module Maven build exercises build-manifest parsing beyond a single-module case. |
| leveldb | `~/dev/openSource/test-repos/leveldb` | C++ | 132 files, CMake | Stresses open question 1 directly — tree-sitter's weak spot on overloads/virtual dispatch. Real interface hierarchies (`Comparator`, `Iterator`, `WriteBatch::Handler`), well-documented enough to check generated specs against. |
| commons-lang | `~/dev/openSource/test-repos/commons-lang` | Java | 627 files, Maven | Plain classic Java, no framework magic — a second Java data point distinct from memolink's Spring-adjacent style. |

Only talentTrail is load-bearing for the Phase 1 milestones as written (M1–M11 all target it). The other three exist to catch language-specific extraction bugs early rather than discovering them only once Phase 2's polyglot ambitions are actually being built — worth a quick M1/M2 smoke pass against each once the TS pipeline works, even though the detailed milestone validations above are TS-specific.

## Sequencing principle

Each milestone below satisfies three things or it isn't a milestone:
1. **Runs on its own** — a CLI invocation or an MCP query you can actually execute, not "code exists but nothing calls it."
2. **Has a concrete validation**, not "add tests" — specific inputs (mostly against the pilot repo) and the exact output that means it worked.
3. **Adds value that compounds** — later milestones consume earlier ones' output; nothing here is throwaway scaffolding.

Sizing (S/M/L) is relative effort, not a time estimate — useful for sequencing decisions, not for a calendar.

---

## Phase 1 milestones

### M1 — Extraction walking skeleton
**Size:** M · **Builds on:** nothing

**Scope:** A CLI binary that walks the pilot repo's `.ts`/`.tsx` files, parses each with tree-sitter, and emits the `Symbol` record for every function/class/const — `id, kind, file, lines, signature, docstring` plus `parent`/`children` (containment tree only). Dump as JSON to stdout.

**Explicitly not in scope:** no import/call resolution, no hashing, no MCP, no persistence. One file in, its symbols out — that's the whole loop.

**Validation:** run against 5–10 hand-picked pilot-repo files spanning your hardest real cases — a React component with hooks, a barrel file, a class, a file with generics — and manually verify the emitted symbol list and containment tree against what you know is actually there. This is the milestone where tree-sitter's cursor API and the "don't let `Node<'a>` escape the parse function" trap (`CLAUDE.md`) actually get learned.

**MVP value:** proves the parser layer works on your actual pilot repo, not a toy example — the highest-uncertainty piece resolved first.

---

### M2 — Module resolution + caching backbone
**Size:** L · **Builds on:** M1

**Scope:** Three things that land together because they share the same graph data structure:
- Wire in `oxc_resolver` to turn each `import` into a real target `SymbolId` (path aliases, barrels, `exports` maps) — populates `imports`/reference edges.
- Move the graph into a proper arena (`Vec<Symbol>`, `SymbolId` indices — per `CLAUDE.md`) and persist it to `.codeowl/graph` (pick a serialization format here — `serde` + bincode or JSON, doesn't matter yet).
- Compute `source_hash` per symbol and propagate it up the containment tree (the Merkle-style aggregation from "Caching and invalidation"), and compute `interfaceHash` per exported symbol (from "Recursive spec generation" / the `Symbol` record).

**Explicitly not in scope:** no spec generation, still no MCP. Call resolution (which function calls which) can wait — only *import*-level (file-to-file) reference edges are needed for the reference-edge invalidation model.

**Validation, three separate checks:**
- *Resolution*: pick 5 files with known import graphs (a `@/lib/...` alias, a barrel re-export, a relative import) — assert each resolves to the correct target `SymbolId`.
- *Hash propagation*: edit one leaf function's body, re-run — assert exactly the expected ancestor chain's `source_hash` changed and nothing else did.
- *interfaceHash*: craft a before/after pair with a body-only change (hash must **not** move) and a signature change (hash **must** move) — this is the fixture that directly tests gap 2's fix.

**MVP value:** this is "the brain's" actual skeleton — the graph the rest of the system reads and writes now genuinely exists and is provably correct on the invalidation logic that gap 1 and gap 2 were about.

---

### M3 — MCP read surface
**Size:** S · **Builds on:** M2

**Scope:** Stand up the `rmcp` stdio server. Implement the pure-read tools: `get_symbol(id)`, `get_callers(id)`/`get_callees(id)` (now resolvable via M2's import edges), and `get_spec(id)` — which, since nothing has been generated yet, always returns the deterministic stub flagged `missing` (signature + docstring, no LLM). Add `search_code` as embedded ripgrep (the `grep` crate) — zero dependency on the graph, cheap to include here since the server now exists to expose it.

**Explicitly not in scope:** no generation — `get_next_spec_task`/`submit_spec` don't exist yet.

**Validation:** connect from Claude Code (or the MCP inspector) and query a handful of known symbols from M1/M2's fixtures — confirm the response shapes match, confirm `get_spec` on anything returns `missing` with a sane stub, confirm `search_code` finds a known string.

**MVP value:** **the first point CodeOwl is actually usable from an agent.** Even with zero specs generated, an agent can now query real structural facts about the repo instead of grepping blind — this alone is a testable slice of the value proposition.

---

### M4 — Generation loop
**Size:** M · **Builds on:** M3

**Scope:** `get_next_spec_task()` and `submit_spec(id, content)`, the spec file writer implementing the decided document format (see `ARCHITECTURE.md` "Spec document format"): per-symbol `source_hash`/`spec_hash` pairs in frontmatter (not just per-file — section-level invalidation granularity), CodeOwl-written deterministic lines (signature from M1, dependency list from M2 — the LLM writes only the prose it uniquely can), `spec_hash` covering the LLM-written prose only, and the granularity rules (a file gets its own doc iff it has ≥1 exported function/class; a directory gets an `_index.md` iff it has ≥2 spec-bearing entries; barrel/boilerplate files appear as one-liners in their directory's rollup instead). Plus the `/codeowl generate <id>` command scoped to a single node (no `--all`/`--budget` yet). This is also where **File nodes enter the graph** (a file-level spec's frontmatter needs a file-level `source_hash`) — and with them, the containment `parent`/`children` conversion from `String` to `SymbolId`, done once across symbol+file rather than twice.

**Explicitly not in scope:** staleness/regeneration logic — this milestone only covers the *first-ever* generation of something that's currently `missing`. Feature specs are M5's, not M4's.

**Validation:** run `/codeowl generate` on one real file. Confirm: the spec lands at the correct mirrored path, frontmatter carries correct per-symbol and file-level hashes, the signature/dependency lines match the graph exactly (not LLM-paraphrased), `get_spec` on that id now returns the real spec instead of the stub, and re-running `generate` on the same id with source unchanged does **not** call the LLM again (source_hash match, per "Ordering" step 1). Also confirm the granularity rules: a barrel file produces no document, and a single-file directory produces no `_index.md`.

Done against the pilot repo (`lib/utils.ts`, real MCP stdio session, both symbols + the file itself), with one caveat: only the file-spec granularity rule got a write path. The directory-rollup rule (`directory_is_spec_bearing`) is implemented and tested, but nothing writes an `_index.md` at all yet, single-file or otherwise — its document shape was never templated in `ARCHITECTURE.md` (see its open question 5), so the single-file validation case currently passes vacuously (no rollup exists to *not* produce) rather than by the rule actually declining one. Pick the format and wire the write path before this milestone's directory-rollup half counts as done. Also worth noting: this milestone caught a real M2 bug in passing — `main.rs` never canonicalized its `path` argument, so `codeowl extract .` silently resolved 0 imports (relative root vs. `oxc_resolver`'s always-absolute output made every `strip_prefix` fail); fixed alongside M4 since it was a one-line, well-isolated correctness fix, not a scope expansion.

**Correction, found during M5's validation, not this milestone's own:** the `### Depends on` line this milestone shipped was wrong, not just imprecise — it listed every import the *file* had under every symbol in it, so `formatDate` claimed to depend on `clsx`/`twMerge` despite never touching either (M2 only resolves imports at file granularity, and M4's writer didn't narrow past that). Fixed in M5 with a whole-word text search scoped to each symbol's own source span — see `ARCHITECTURE.md`'s "File spec shape". Flagged here because the bug shipped as part of *this* milestone's deliverable, even though nothing in M4's own validation was positioned to catch it (the validation target, `lib/utils.ts`, happens to have a symbol — `formatDate` — that doesn't use the file's only imports, but nobody looked at that section closely until generating a second, unrelated file surfaced the pattern).

**MVP value:** the actual deliverable — a real, LLM-written, cached spec exists and is queryable. This is the milestone where "the brain" produces its first thought.

---

### M5 — Feature layer
**Size:** M · **Builds on:** M4

**Scope:** The BA-facing document kind and the machinery that makes it derivable (see `ARCHITECTURE.md` "Feature specs"):
- **Entry-point enumeration** from framework conventions: `app/**/page.tsx`, plus orphan API routes (`app/api/**/route.ts` no page references — webhooks/crons).
- **Route-literal resolver**: `fetch("/api/submit-artwork")` → `app/api/submit-artwork/route.ts` by Next.js path convention — deterministic, framework-aware, and deliberately *not* general call resolution (still deferred past Phase 1). This recovers the UI→API edges the import graph structurally cannot see.
- **Participant-set assembly**: from an entry point, follow import edges + route-literal edges to collect the files/symbols a feature touches.
- **Feature spec generation** via the same `get_next_spec_task`/`submit_spec` loop, writing `docs/specs/_features/<route-slug>.md` with the `participants` frontmatter map (each participant's hash as observed at generation time). Feature specs *consume* participants' summaries or deterministic stubs — never trigger their generation (the containment-only recursion invariant holds).

**Explicitly not in scope:** staleness (M7), manifest-based feature renaming/merging (later refinement — v1 names come from route paths), general call/invocation resolution (still deferred past Phase 1 per open question 3).

**Validation:** enumerate entry points on the pilot repo — count matches its actual page/route inventory. The route-literal resolver maps `fetch("/api/submit-artwork")` in `app/submit/page.tsx` to the right route file. Generate the artwork-submission feature spec — confirm its participant list includes the page, both API routes it calls, and the lib helpers they import, and confirm (human judgment, the point of the whole exercise) that the document answers "how does artwork submission work" without opening a single source file.

Done against the pilot repo, real MCP stdio session, both entry-point shapes: `app/submit/page.tsx` (a page) and `app/api/payments/webhook/route.ts` (an orphan route — nothing else in the repo ever fetches it, matching its real shape as a Razorpay-called webhook). Page enumeration matched exactly (31 on disk, 31 found). The route-literal resolver found *three* core files for the submit feature, not the two `ARCHITECTURE.md`'s own worked example named — it also caught `app/api/get-registration/route.ts`, fetched via a template literal with a query-string interpolation (`` `/api/get-registration?code=${...}` ``) the resolver correctly distinguished from a *path*-interpolated one (which it deliberately declines to match — see `static_path_from_literal`'s doc comment). One caveat on the "count matches inventory" bullet specifically: of 68 API routes, 45 came back as orphans — some of those are likely routes fetched via a genuinely dynamic path segment (`` `/api/foo/${id}` ``) that the resolver conservatively refuses to match rather than guess at, so "orphan" here means "not resolved," not strictly "never called." Tightening that would mean relaxing the deliberate no-guessing rule, which isn't obviously worth it yet — flagged, not fixed.

**MVP value:** the documents BAs (and agents doing feature-shaped work) actually read now exist — and M11's exit test stops being structurally rigged against feature-shaped questions.

---

### M6 — Directory rollups
**Size:** S · **Builds on:** M4

**Scope:** Closes the gap M4 left open: the directory-rollup granularity rule (`directory_is_spec_bearing`, ≥2 spec-bearing files) was implemented and tested in M4, but nothing writes an `_index.md` — its document shape was never templated in `ARCHITECTURE.md` (open question 5). This milestone decides that shape and wires the write path: whether a rollup's `## Summary` synthesizes its files' summaries or just lists them, per-file hashes in frontmatter keyed on each file's own `source_hash` (mirroring the file spec's per-symbol pattern one level up), and what makes a rollup a valid `/codeowl generate <id>` target (a directory path, resolved against the files under it — directories still aren't graph nodes, per M4's deliberate scope cut, so this needs its own lightweight lookup rather than `graph.find`). Once decided, record the shape in `ARCHITECTURE.md`'s "Spec document format" alongside the file/feature templates, and remove open question 5.

**Explicitly not in scope:** staleness (M7 needs a rollup to go stale, but propagating that correctly is its own milestone, not this one's) — this milestone only covers first-ever generation, same split M4 made for file specs.

**Validation:** generate rollups for two real pilot-repo directories: one with ≥2 spec-bearing files (confirm `_index.md` is written, lists all of them, frontmatter hashes match) and one single-file directory, e.g. an `app/api/<route>/` folder (confirm no `_index.md` is produced — this is the M4 validation bullet that only passed vacuously until now).

Done against the pilot repo, real MCP stdio session. The document shape (`## Summary` LLM-synthesized from the directory's files' own summaries, `## Contents` CodeOwl-written and recomputed on every render, frontmatter keyed on each file's `spec_hash`) is recorded in `ARCHITECTURE.md`'s "Spec document format", closing open question 5. `lib/email` (3 files, 4 exported symbols, picked over the much larger `lib/` itself — ~40 spec-bearing files there, more than a dogfooding pass needed to prove the mechanism) walked its full bottom-up ladder — each file's symbols, then each file, then the rollup only once all three files were current — to a correct `_index.md` exactly matching the decided shape. `app/about`, a genuine single-file directory, correctly produced no task at all and no `_index.md` on disk. `/codeowl generate <dir>`'s "not a graph node" lookup (grouping `Graph::files()` by parent path) works as scoped; the reserved-path collision with a future system spec at `docs/specs/_index.md` (dir_path `""`, the repo root) is guarded against with a clear error rather than silently mishandled, since no system-spec milestone exists yet to reconcile it against.

**MVP value:** the "module orientation" document kind actually exists, and M7's staleness validation (which assumes a directory `_index.md` to edit and re-check) becomes runnable as written instead of silently depending on unbuilt scaffolding.

---

### M7 — Staleness & invalidation end-to-end
**Size:** M · **Builds on:** M6

**Scope:** Wire up the parts of M2's hashing that M4/M5/M6 didn't yet exercise, across **all document kinds**: `get_spec` on a stale node returns the last-known-good spec flagged `stale` with what changed (not just `missing`), `/codeowl generate` on a stale node does a real cascade-bounded regeneration, and a feature spec goes stale when any entry in its `participants` map moves (or the participant set itself changes — a new route literal appears).

**Validation — this is the milestone that specifically proves gaps 1–3 work *together*, not just individually:**
- Generate specs for a small module tree (a file with 2–3 functions, contained in a directory with an `_index.md`).
- Edit one leaf function's body only → confirm exactly its file spec and the containing directory's `_index.md` go stale, nothing else.
- Edit a widely-imported utility's *signature* → confirm its direct importers' specs go stale (via `interfaceHash`) but nothing importing *those* importers does, and nothing containing the utility's unrelated siblings does.
- Edit the utility's implementation only (signature unchanged) → confirm **zero** importers go stale. This is the literal gap-2 regression test.
- Edit the body of a feature participant → confirm that feature's spec goes stale; edit a file no feature touches → confirm no feature spec does.

**MVP value:** the correctness properties the whole design exists to guarantee are now demonstrated on real code, not just argued in a doc.

Done. The real new mechanism was the reference-edge dimension: `source_hash` alone (already correct since M2) already made the leaf-edit/rollup-propagation bullet work for free — a file's whole-text hash already moves on any internal edit, and a rollup's currency check (keyed on each file's own recorded `spec_hash`) already treats a not-yet-current file's contribution as absent, so it goes stale in lockstep. What genuinely didn't exist was anything that noticed *"nothing in this file changed, but something it imports did"* — that's the new `deps_hash` field (per-symbol and per-file, alongside `source_hash`), populated from exactly the same resolved-import scoping `### Depends on` already uses. `get_spec` across all four document kinds now reports `"missing"` / `"current"` / `"stale"` plus a deterministic `changed: Vec<String>` naming what moved, and needed no new regeneration path — the identical hash-mismatch check that decides staleness already made `next_task`/`next_feature_task`/`next_rollup_task` return a task instead of `None`.

Validated three ways. Unit tests: `diff_hash_lists` (the shared participant/rollup-file differ) reports `added:`/`changed:`/`removed:` correctly. Real-MCP integration tests (fresh fixtures, no pilot-repo dependency): (1) a dependency's *signature* edit stales its importer (`changed:dependencies`) while an *implementation-only* edit doesn't touch it, and an unrelated third file is never affected; (2) a leaf function's body-only edit stales exactly its own file spec and the containing directory's rollup — confirmed as an immediate `get_spec` read, not something requiring a full regenerate pass first — while a sibling file in the same directory stays current; (3) editing a feature's core participant (the page itself) stales the feature, editing a file the feature never touches doesn't. Then live against the pilot repo's real `lib/email` (from M6): confirmed pre-M7 specs (no `deps_hash` in their frontmatter) read back as `stale` rather than crashing or silently trusting stale content, regenerating them once is enough to reach the new format, and a live edit to `lib/email/config.ts::getResendClient`'s signature correctly staled both files that import it (`send-artwork-submission-reminder.ts`, `send-registration-email.ts`) via `changed:dependencies`, while a body-only edit staled neither — the source file was restored byte-for-byte afterward.

---

### M8 — Completeness & correction mechanics
**Size:** M · **Builds on:** M7

**Scope:** `get_spec_coverage(scope?)` (reporting against the granularity rules' document *inventory*, not raw file count), `/codeowl generate --all` and `--all --budget=N`, and the full four-case human-correction reconciliation from "Human corrections." Budget spend order is the decided priority: **system spec → feature specs → high-fan-in files → long tail** — so on a brownfield repo, the first budgeted runs build exactly the documents a human would want first.

**Validation:**
- Run `get_spec_coverage` on the pilot repo pre-generation — confirm it reports the right missing/stale/current breakdown against the rule-derived inventory (barrel files absent from it entirely).
- Run `--all --budget=10` — confirm exactly ≤10 generations happen, in priority order: a crafted fixture with a feature spec, a high-fan-in file (imported by 5 others), and a leaf file (imported by none) gets them generated in exactly that order.
- Hand-edit a generated spec file with source unchanged → confirm a subsequent `generate` leaves it untouched and updates `spec_hash` to match (case 3). Then change the underlying source too → confirm a reconciliation regeneration that preserves the human correction where still accurate (case 4).

**MVP value:** the design's full feature set for Phase 1 is now built — this is functionally "Phase 1 complete" from a capabilities standpoint, before the incremental/live-session and validation milestones.

Done, with one prerequisite this milestone's own scope surfaced first: the priority order needs a **system spec** to put first in line, and nothing had ever decided that document's shape or built its generation path — the same kind of silent gap M4 left for directory rollups. Decided and built before the rest of M8: one document per repo at `docs/specs/_index.md`, composed from two flat lists (every rollup-bearing directory anywhere in the repo, and every enumerated feature entry point — never a nested module tree, since rollups themselves don't recursively aggregate), addressed by the fixed pseudo-id `"system"` (or `"."`, which triggers the identical whole-repo walk). See `ARCHITECTURE.md`'s "System spec shape."

`get_spec_coverage(scope?)` reports current/stale/missing against the granularity rules' actual document inventory (files, rollups, features, the system spec) and returns `pending` — every non-current item, already sorted system-spec-first, then features, then files by descending import fan-in, then rollups. `--all`/`--budget=N` needed no new MCP mechanism: they're the slash command walking (or budget-capping a walk of) exactly that list, reusing `get_next_spec_task`/`submit_spec` verbatim.

Human-correction reconciliation (cases 3 and 4) is implemented for file specs — both per-symbol and the file's own summary — by re-hashing the currently-parsed prose inside `next_task` itself and comparing it to the recorded `spec_hash`; a mismatch with source unchanged reconciles silently (frontmatter hash refreshed, no LLM call, prose untouched), a mismatch with source *also* changed surfaces the human's prior text on the task for the agent to reconcile against. **Not yet extended to feature/rollup/system specs** — flagged in `ARCHITECTURE.md`, not silently skipped; the ROADMAP validation bullet below was scoped to file specs specifically, and the other three kinds share the same detection shape if it turns out to matter later.

**Validated three ways.** Unit tests: the four-case classification (current / plain regen / silent reconcile / reconciliation-with-prior) for both a symbol and a file, `coverage`'s counts and priority ordering against a crafted fixture, and a regression test for a real bug caught while writing that fixture — `enumerate_modules` would have offered the repo root itself as a generatable module (it can have >=2 spec-bearing files directly in it) despite `next_rollup_task` refusing to ever generate that reserved path, now excluded up front. Real-MCP integration tests: a coverage-driven "budget=2" walk that stops exactly where it should, and a human edit surfacing correctly through the real `get_next_spec_task`/`get_spec` calls. Live against the pilot repo: `get_spec_coverage` on all 307 files reported 6 current / 3 stale / 318 missing across 327 real documents with no crash, `scope` correctly reached nested subdirectories (`lib/email/templates`) while *not* false-matching an unrelated same-prefix file (`lib/email.ts`) — a real bug the naive string-prefix version of `scope` had, caught and fixed by this same dogfooding pass — and a live hand-edit to `lib/email/config.ts.md`'s real prose reconciled silently and was confirmed byte-for-byte preserved.

**Follow-up, added after M8: quality smells.** A white-box audit of the real generated specs (prompted by a broader question about whether the tool's generated-docs output holds up on its own) found two real file specs — `app/submit/page.tsx.md`, `app/api/payments/webhook/route.ts.md` — that were template stubs (`"<name> does its job."` / `"See source."`) from an early mechanism-validation pass, never regenerated since, and no part of the design could tell that content apart from a genuinely good spec once its hashes stopped moving. Added `prose_smells` — a small, deterministic, non-LLM check (a cop-out-phrase denylist, a word-count floor, and for file specs the pre-M5 identical-dependency-list signature) — surfaced via `get_spec`'s new `smells` field, folded into `get_spec_coverage`'s `pending` list, and wired into `next_task`/`next_feature_task`/`next_rollup_task`/`next_system_task` themselves so targeting a smelly-but-current document directly actually produces a task instead of a `null` indistinguishable from genuinely-nothing-left. `status` itself stays purely hash-based throughout (see `ARCHITECTURE.md`'s "Quality smells") — a real bug caught mid-implementation had `get_spec_coverage`'s file-status check delegating to `next_task`, which meant a smelly-but-hash-current file started incorrectly reporting `status: "stale"`, conflating two signals that need to stay independent; fixed by giving file status its own hash-only check rather than reusing the now-smell-aware task generator. Fixing this also broke roughly a dozen pre-existing tests across the suite that used single-word/short placeholder content (`"S.", "B.", "Does one thing."`) as submit prose — all rewritten to realistic-length text, which is arguably a healthier baseline for the test suite regardless. Live-validated against the pilot repo: both known-bad specs were correctly flagged with `["cop_out_phrase", "suspiciously_short"]`, and every known-good spec checked (the `lib/email/*` files, `lib/utils.ts`, the `submit` feature) came back clean — no false positives.

---

### M9 — Incremental indexing (live sessions)
**Size:** S · **Builds on:** M8

**Scope:** `notify`-based file watcher for the remainder of an MCP server session, plus the fresh-spawn catch-up pass (hash-check everything against `.codeowl/` on startup).

**Validation:** start the server, edit a file while it's running, confirm a subsequent `get_spec` on an affected node reflects the change without restarting the server. Kill the server, edit more files, restart, confirm the catch-up pass reindexes exactly the changed files.

**MVP value:** turns CodeOwl from "a tool you re-run" into something that behaves like a language server across a real multi-hour coding session — the actual target interaction model from `REQUIREMENTS.md`.

Done. A new `RepoIndex` (`src/index.rs`) keeps the *per-file inputs* the graph is built from — extracted symbols, named imports/re-exports, route literals, raw-text hash — persisted alongside the graph at `.codeowl/index`, so a rebuild re-parses only the files that actually moved. Both moments use it: `RepoIndex::open` is the fresh-spawn catch-up (load the cache, hash-check every file, re-extract what changed while nothing was running, returning a `CatchUp { added, modified, removed }` that names exactly which), and `RepoIndex::apply_changes` is the watcher's incremental update (identical-content writes are a no-op, never a rebuild). The watcher itself (`src/watch.rs`) follows MemoLink's `GraphWatchService`: one background thread, a 300 ms debounce collapsing an editor's write burst into one rebuild, and per-directory (not recursive) `notify` watches over the gitignore-visible tree — a recursive watch on the root would also register `node_modules` and blow past the OS inotify limit on a real Next.js repo; newly-created directories are registered as they appear. The MCP server's graph moved from `Arc<Graph>` to `Arc<ArcSwap<Graph>>`: every request handler loads one consistent snapshot up front (a `SymbolId` is only valid for the graph that produced it, so a handler must not straddle a swap), and the watcher publishes a reindexed graph with a single lock-free `store`. Validated three ways: unit tests for the catch-up diff and the equivalence of an incrementally-rebuilt graph to a cold full build; an MCP-level test that `get_symbol` reflects a hot-swapped graph; and `tests/incremental.rs` driving the real `notify` watcher end-to-end (edit a file, poll until the served graph shows the new hash; add a file, poll until its import edge resolves). `codeowl extract` now also writes `.codeowl/index` as a side effect, warming the cache for a later `serve`.

---

### M10 — SQL/schema boundary resolution
**Size:** M · **Builds on:** M2 (graph), independent of M3–M9

**Scope:** The dedicated SQL DDL extractor (schema nodes for tables/columns/constraints) plus the fuzzy string/ORM-aware matcher on the application-code side, per open question 3's second bullet in `ARCHITECTURE.md`. Called out there as "directly relevant to the Phase 1 pilot… not a future concern" — the pilot repo's Supabase migrations are real, not hypothetical.

**Explicitly not in scope:** the other two polyglot sub-problems from that open question (general call/invocation boundaries, standalone "island" nodes) — genuinely lower-stakes for this pilot, can stay deferred past Phase 1. (M5's route-literal resolver is a deliberate, narrow exception: framework-convention mappings only, not call analysis.)

**Validation:** point it at the pilot repo's actual migration files, confirm schema nodes are created for known tables, and confirm at least one known ORM/string-literal reference in application code resolves to the right schema node (fuzzy match, so "resolves to a plausible candidate" is the bar, not exact precision).

**MVP value:** without this, any exit-criterion task touching schema-coupled code (exactly the kind of task the deleted exp-01 scaffolding picked, for good reason) gets an unfairly degraded test — schema nodes would just be invisible. This milestone is what makes M11 a fair test rather than an easy one.

---

### M11 — Exit criterion validation
**Size:** S (mechanically) · **Builds on:** M8 and M10

**Scope:** The thing Phase 1 actually exists to answer. Populate `docs/specs/` for a meaningful slice of the pilot repo (via M8's `--budget=N`, which now builds the system + feature layer first), add the `CLAUDE.md` line telling an agent to check specs first, then run several real tasks with and without CodeOwl available and compare — using `utility/mine.py`, already built, for the token/exploration-tool measurement.

**Validation:** this milestone *is* the validation — the deliverable is a verdict, not code. Concretely: for each task, does `mine.py`'s exploration-tool count and token estimate drop meaningfully with specs available, and does the agent actually use `get_spec` rather than defaulting to `Read`/`Grep` (the H1 gate from the deleted exp-01 scaffolding's framing — worth reusing that hypothesis table even though the manual pre-code version isn't being run). Plus the BA-side gate added when feature specs became a Phase 1 must-have: a feature-shaped question ("how does artwork submission work?") is answerable from the feature spec alone, judged by a human reading it without the code.

**MVP value:** this is the actual Phase 1 exit gate. A "no" here is a valid, useful outcome — it means stopping before any Phase 2 investment, which is the entire reason Phase 1 was scoped this way.

---

## Phase 2 — sketch only, revisit after M11

Deliberately coarse — detailed planning here is premature until M11 answers whether Phase 2 happens at all. Rough shape, in likely order:

1. **HTTP/SSE transport** — `rmcp` over HTTP instead of stdio, repo-scoped tool calls.
2. **`tantivy` + ONNX embeddings** — the real search index deferred out of Phase 1 (see `ARCHITECTURE.md` "Storage"), now justified by multi-user load.
3. **Multi-repo namespacing** — per-repo index/graph/spec-cache within one shared process (`REQUIREMENTS.md` "Hosting granularity").
4. **Stub nodes + cross-team delegation** — the cross-repo dependency model (`REQUIREMENTS.md` "Multi-repo & team ownership").
5. **Web viewer** — the Cytoscape.js-style graph browser for BAs/QA/SREs.
6. **Auth/roles** — reopens once multiple users share one hosted instance (`REQUIREMENTS.md` open question 2).

## Provisional decisions this ordering makes

Worth flagging since I made these calls rather than asking:
- **M10 (SQL) is sequenced before M11**, not folded into M4–M9's core TS pipeline. If you'd rather ship M11 sooner and pick a validation task that avoids schema-coupled code, M10 can move after M11 or drop out of Phase 1 entirely — it's the one milestone here that isn't strictly load-bearing for the others.
- **`CLAUDE.md`'s two pending items** (tool-surface trimming, spec-regen commit hygiene) aren't given their own milestones — the plan resolves the tool-surface one implicitly (M3–M8 only ever build the tools actually used: `get_spec`, `get_symbol`, `get_callers`/`get_callees`, `search_code`, the three generation tools, `get_spec_coverage` — `trace_path`/`get_tests_for`/`get_dependencies` simply never get built unless a later milestone needs them), and commit hygiene is a workflow habit to adopt once M8 exists, not something to build.

Revised 2026-09-06: the spec-format decision (see `ARCHITECTURE.md` "Spec document format") inserted the feature layer as its own M5 and renumbered everything after it — old M5–M9 are now M6–M10. Feature specs were promoted from a Phase 2 idea to a Phase 1 must-have after tracing a real pilot-repo feature and finding its two load-bearing hops (UI→API via `fetch("/api/…")`, API→DB via `.from("table")`) are string literals the import graph structurally cannot see — meaning directory-mirrored specs alone would have rigged M10's exit test against exactly the feature-shaped questions it needs to answer.

Revised again 2026-09-06, after M4's real-repo validation: M4 shipped the file-spec granularity rule but never wrote directory rollups (their document shape was never templated — see `ARCHITECTURE.md`'s open question 5), and that gap turned out to be silently load-bearing — the staleness milestone's own validation assumes a directory `_index.md` already exists to edit and re-check. Rather than leave that implicit, directory rollups became their own milestone, inserted as the new M6 right after the feature layer; everything from the old M6 (Staleness) onward shifted up by one, M6–M10 becoming M7–M11.
