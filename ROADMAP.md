# CodeOwl — Roadmap (Working Draft)

> Companion to `ARCHITECTURE.md` (how) and `REQUIREMENTS.md` (what/for whom). This document is the *when* and *in what order* — a walking-skeleton sequence where each milestone is independently runnable, has a concrete pass/fail test, and adds real MVP value rather than being scaffolding for its own sake. Pending trims from `CLAUDE.md` (tool surface, commit hygiene) get resolved as they're hit, not decided up front.

## Test repos

Fixed set of real repos the milestone validations below run against — chosen for language coverage and for actually stressing specific open questions, not just for being famous. The non-CodeOwl ones live as siblings on disk.

| Repo | Path | Language | Size | Why this one |
|---|---|---|---|---|
| the pilot repo (`talentTrail`) | `~/dev/startup/talentTrail` *(private)* | TypeScript/TSX | ~300 files | **Phase 1 + M13 (`TypeScriptNextStack`).** A real Next.js 16 (App Router) / React 19 / Supabase / Upstash application — matches `REQUIREMENTS.md`'s pilot description. Primary target for every Phase 1 milestone. Named here once; everywhere else it's just "the pilot repo". |
| codeowl (this repo) | `~/dev/openSource/codeowl` | Rust | 16 files, ~10k lines | **M14 (`RustStack`) — the seam validation.** The one repo the author knows line-for-line, so "read ~10 self-specs and judge" is a real check. Cheap: no repo setup, no build system to model, no schema layer. The eventual dogfood is CodeOwl describing itself. |
| commons-lang | `~/dev/openSource/test-repos/commons-lang` | Java | 627 files, single-module Maven | **M16 (`JavaStack`) — a third stack, and the `feature_model() -> None` path.** Plain classic Java, no framework, no runtime entry surface: proves the trait survives a stack the TS and Rust impls didn't shape, and that a library with no feature layer still gets a coherent corpus. Records/annotation-types stress `SymbolKind`; star imports stress resolution. |
| quarkus-super-heroes | `~/dev/openSource/test-repos/quarkus-super-heroes` | Java (Quarkus) | ~120 Java files, Maven reactor (7 services) | **M17 — the feature layer for a Java service.** The official Quarkus reference app: a parent-pom reactor of independently-deployable services (`rest-heroes`, `rest-villains`, `rest-fights`, `event-statistics`, `rest-narration`, `grpc-locations`, + a UI). Exercises multi-module resolution, **heterogeneous entry-point kinds** (JAX-RS `@Path`, `@Incoming`/`@Outgoing` Kafka, `@Scheduled`, `@GrpcService`), the JPA-vs-migrations schema question, and **cross-service edges carried by string** (`@RegisterRestClient`) — the same boundary-resolution shape as M10's `.from()`. Each service ships a README + architecture diagram, so specs are checkable against ground truth. |
| leveldb | `~/dev/openSource/test-repos/leveldb` | C++ | 132 files, CMake | **Not scheduled — a C++ datapoint for whenever a `CppStack` happens.** Stresses open question 1 directly: tree-sitter's weak spot on overloads/virtual dispatch, real interface hierarchies (`Comparator`, `Iterator`, `WriteBatch::Handler`), well-documented enough to check specs against. |

The pilot repo is load-bearing for Phase 1 (M1–M11) and is the M13 pack-extraction target. Phase 2's polyglot milestones each own one of the others: **M14 → codeowl, M16 → commons-lang, M17 → quarkus-super-heroes**. `leveldb` isn't on the schedule — it's the C++ case held in reserve. A quick `codeowl extract` smoke pass against each new repo is still worth doing the moment its stack's extractor compiles, ahead of that stack's full milestone.

## Sequencing principle

Each milestone below satisfies three things or it isn't a milestone:
1. **Runs on its own** — a CLI invocation or an MCP query you can actually execute, not "code exists but nothing calls it."
2. **Has a concrete validation**, not "add tests" — specific inputs (mostly against the pilot repo) and the exact output that means it worked.
3. **Adds value that compounds** — later milestones consume earlier ones' output; nothing here is throwaway scaffolding.

Sizing (S/M/L) is relative effort, not a time estimate — useful for sequencing decisions, not for a calendar.

---

## Phase 1 milestones

> **Status (2026-09-09): Phase 1 complete; Phase 2 through M16 done (branch `m16-java-stack`, not yet merged).** M1–M10 shipped and validated; M11 validated by sample (full corpus generation deliberately stopped — see M11's "Outcome"). Phase 2's polyglot core has landed **M12** (cache `format_version` + named seams), **M13-pre** (feature-layer spike), **M13** (the `StackPack` trait), **M14** (`RustStack` — `tree-sitter-rust` extraction + module-tree resolution, `detect()` branching, `feature_model() -> None`, validated on a ~35-spec self-corpus with no trait leaks), **M15** (M14's one finding folded in — an inherent `impl Foo` merges into `Foo`, still zero `spec.rs` change; symbol source spans follow a folded method; doc-neutralization first pass; `symbol.rs`/`graph.rs` self-specs re-rendered), and **M16** (`JavaStack` on commons-lang — `tree-sitter-java` extractor, path-suffix + same-package resolution, N-way `detect`, `feature_model() -> None`; validated by a commons-lang dogfood — internal resolution 100 %, spec quality high; headline M18 finding: `get_next_spec_task` returns whole-file source for a folded Java class, over the MCP token limit). `FORMAT_VERSION` 3 → 6 (M16 needs no bump). The exhaustive self-spec corpus (~240 specs + `rollup:src` + `system`) is a deferred follow-up. Two bugs found during M16, tracked separately: a `submit`/`next_task` infinite loop on smelly prose (**fixed, PR #17, merged**) and an oxc_resolver `strip_prefix` bug that breaks TS resolution under a symlinked repo root on macOS (fix after M16). See "Phase 2 — the polyglot core first".

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

Live against the pilot repo (~290 TS/TSX files): with `serve` running, an edit inside a symbol body surfaced through `get_symbol` after ~0.5 s with no restart, and the revert ~0.4 s later; an edit made while nothing was running was caught by the next spawn's catch-up pass (`catch-up: reindexed 1 file(s)`), and the file returned byte-identical with `git status` clean. Per-directory watch registration over the gitignore-visible tree handled the real repo without straining the inotify limit.

---

### M10 — SQL/schema boundary resolution
**Size:** M · **Builds on:** M2 (graph), independent of M3–M9 · **Status: done** (stack-modularization prep + SQL extractor + `.from()` matcher)

**Scope:** The dedicated SQL DDL extractor (schema nodes for tables/columns/constraints) plus the fuzzy string/ORM-aware matcher on the application-code side, per open question 3's second bullet in `ARCHITECTURE.md`. Called out there as "directly relevant to the Phase 1 pilot… not a future concern" — the pilot repo's Supabase migrations are real, not hypothetical.

**Explicitly not in scope:** the other two polyglot sub-problems from that open question (general call/invocation boundaries, standalone "island" nodes) — genuinely lower-stakes for this pilot, can stay deferred past Phase 1. (M5's route-literal resolver is a deliberate, narrow exception: framework-convention mappings only, not call analysis.)

**Stack-modularization prep (part of this milestone, done first):** before the SQL work, doc-mark the four TypeScript + Next.js–coupled files (`extract.rs`, `imports.rs`, `resolve.rs`, `features.rs`; plus the one line in `index.rs`) as "the TypeScript + Next pack — Phase 2 seam here", and dedup the `.tsx`/`.ts` grammar pick copy-pasted across three of them. Pure labeling and cleanup, no abstraction (~30 min). Then the SQL DDL extractor and Supabase `.from("table")` matcher land ad hoc against the current structure — a second framework-convention resolver alongside M5's route literals, and a second extractor kind. Centralizing all of it behind `src/lang.rs` is deferred to M11 (see its scope), and the `StackPack` trait to Phase 2.

**Validation:** point it at the pilot repo's actual migration files, confirm schema nodes are created for known tables, and confirm at least one known ORM/string-literal reference in application code resolves to the right schema node (fuzzy match, so "resolves to a plausible candidate" is the bar, not exact precision).

**Validation result (2026-09-07):** against the pilot's real `supabase/schema.sql` (4998-line pg_dump), 25/25 `CREATE TABLE`s become `SymbolKind::Table` nodes — `tree-sitter-sequel` parses 24, a `CREATE TABLE` header line-scan backstops the one it drops on a nested CHECK cast. 304 `.from("table")` call sites extracted across the app; 275 (90%) resolve to a table node. The 29 that don't are all database *views* (`registrations_detailed`, `judge_artworks_anonymized`, …) — `CREATE VIEW` is deliberately out of scope, so "unresolved" is correct there, not a miss. `get_callers` on a table id lists the files that `.from()` it. Column types, foreign-key edges, and views stay out of scope (candidates for a follow-up if M11's corpus wants them). `src/schema.rs`; `tests/schema.rs` + unit tests; `tree-sitter-sequel` 0.3.11 added.

**MVP value:** the pilot is a payments/registration app — its domain *is* the database. Without schema nodes a feature spec's "Data touched" section can't name the tables a flow reads and writes, and a file spec for `processWebhookEvent` can't say it marks `payments` and `registrations` — the specs stay vague exactly where the domain is richest. This directly determines whether the M11 corpus passes its BA cut on any payments/registration question, and it's the last framework-convention resolver Phase 1 needs (after M5's route literals).

---

### M11 — Spec corpus + dual-audience quality bar
**Size:** M · **Builds on:** M8 and M10 · **Status: validated by sample (2026-09-07)** — codeowl-side prep complete; feature-spec quality demonstrated on the pilot; full generation deliberately stopped. Details at the end of this section.

**Scope:** Produce the thing the project exists to produce. Generate specs across a substantial, representative slice of the pilot repo — every feature, all of `lib/`, `app/api/`, and the load-bearing `app/` pages — via `/codeowl-generate --all --budget=N` in repeated passes, and commit the corpus as its own PR (per the spec-regeneration commit-hygiene convention). Add the `CLAUDE.md` line pointing agents at `get_spec`/`get_spec_coverage` before they explore.

**Stack modularization (part of this milestone, separate work item):** land `src/lang.rs` — free functions, no trait — that centralizes the TypeScript + Next coupling flagged in M10: the grammar pick, `is_extractable`, the resolver extension list, and a `detect(root)` that fails fast on a repo it can't parse instead of silently serving an empty graph. Relocate M10's schema extractor / matcher into it. The interface is designed against two convention resolvers and two extractor kinds, not one. Promoting `lang.rs` to a `StackPack` trait with a real second-stack implementation stays in Phase 2 — the only step that actually validates the seams, which a same-stack extension like M10 can't. Independent of the corpus deliverable, but sequenced first (see below) so the corpus isn't generated against a moving codebase.

**Execution / sequencing** (decided 2026-09-07). Two work streams, split by repo:

*In the codeowl repo, before any generation:*
1. `src/lang.rs` + `detect(root)` — the stack-modularization item above. TDD, its own commit.
2. **Data-touched participant wiring** — the piece M10 deferred: wire resolved `.from("table")` refs into `assemble_participants` as a `data` participant, hashed in `current_participant_hashes`, and pass the table's column list to the feature task. This is what gives a feature spec's "Data touched" section graph backing and schema-change staleness — do it *before* generation so the corpus is written with it, not regenerated after. TDD, its own commit.
3. Rebuild the release binary.

*In the pilot repo:*
4. Point the pilot's `.mcp.json` at the fresh binary; restart the MCP client there.
5. `/codeowl-generate --all --budget=N` in repeated passes from a Claude Code session **inside the pilot repo** — the real command, not the manual stdio loop, since exercising `/codeowl-generate` end-to-end is part of what M11 validates. Review each batch for accuracy. Extractor/resolver gaps found mid-run get fixed back in the codeowl repo, rebuilt, resumed. Stop when `get_spec_coverage` reports nothing `missing`.
6. Commit the corpus as its own PR in the pilot repo; add the `CLAUDE.md` line.

*The four cuts:* dev cut and smell cut can be run by an agent from the pilot session; the **BA cut needs a human** answering feature questions from specs alone; the `mine.py` data point is optional.

**Validation:** the deliverable is the corpus plus a short quality writeup, assessed four ways:
- **BA cut** — a human answers 3–4 feature-shaped questions ("how does artwork submission work?", "how does payment reconciliation work?", "what happens when a judge is reassigned?") from the feature and system specs alone, without opening any source file. Pass = each spec was sufficient *and* accurate.
- **Dev cut** — spot-check ~10 file/symbol specs against their source: no invented behavior, no missing error/edge cases, dependency lists right.
- **Smell cut** — `get_spec_coverage` reports `smelly: 0` across the generated corpus.
- **Agent data point (optional, informational)** — one `utility/mine.py` run comparing 2–3 real tasks with and without specs available. Not a gate.

**MVP value:** this is Phase 1's actual output — a real, committed, dual-audience spec corpus over a real repo, plus evidence it holds up when a human reads it cold. Not a verdict on whether to continue: the brownfield-documentation value proposition is the premise, not the hypothesis under test.

**Outcome — validated by sample, full generation stopped (2026-09-07).**

*Done:*
- All codeowl-side prep (`f865e81`…`ba3b69b`): `src/lang.rs` + `detect()`, data-touched participants, `--all` prioritization (fan-in tiering, `{"kind":"done"}`, test-code tier, shared-code cap), rendered-component `core` expansion, default-import resolution. 134 unit + 10 integration tests.
- Partial corpus on the pilot: ~45 current specs — 7 feature specs, ~35 `lib/` file specs, 2 rollups (~15% of the granularity-rule inventory).
- The pilot's `CLAUDE.md` gained the "Structural specs (CodeOwl MCP)" section.

*Not done, deliberately:* the remaining ~85 feature specs, the `lib/`/`app/` file-spec tail, the rollups, the system spec, and the formal four-cut writeup. These are mechanical volume, not risk.

*Why stopped:* the feature-spec layer — the highest-value, highest-risk output — was reviewed by the project owner and judged to clearly meet the BA and dev bars. The clinching example: `_features/api-judge-evaluations-[evaluationId]-reeval.md` correctly leads with "this flow is dormant" — the re-eval feature was switched off by flipping one variable to `false` (commit `b6d8965f` in the pilot), exactly the kind of runtime-state fact a hand-maintained doc rots on and a regenerated spec catches. `get_spec` staleness, smell detection, `.from()`-table "Data touched", and rendered-component `core` all confirmed working end to end. The dual-audience value proposition (already the Phase 1 *premise*, not a hypothesis) is demonstrated; grinding out the full corpus adds coverage, not confidence.

*If resumed later:* `/codeowl-generate --all --budget=N` in the pilot picks up exactly where it left off — nothing is lost by stopping.

---

### Stack modularization — folded into M10, M11, and Phase 2

The coupling to TypeScript + Next.js App Router comes in two kinds:

- **Mechanical coupling** — the tree-sitter grammar pick, `is_extractable`, the resolver extension list, the `.sql`-vs-TS extract dispatch. Confined to `extract.rs` / `imports.rs` / `resolve.rs` / `schema.rs` / `features.rs`, and (M11) centralized behind `src/lang.rs` + `detect(root)`.
- **Model coupling** — the *shape* CodeOwl assumes a repo has, which has leaked upward as M10/M11 added features:
  - `graph.rs` carries four pack-specific derived-edge collections: `route_literals` (Next `fetch("/api/…")`), `table_refs` (Supabase `.from()`), `rendered_components` (React JSX), `resolved_default_imports` (ES modules). The generic `Graph` struct names four TS+Next concepts.
  - `spec.rs::prioritize` gained `is_test_path` (JS-ecosystem flavored) and `is_ui_primitive` (`components/ui/` — a shadcn/Next convention), in the module that's meant to be language-neutral.
  - the whole feature-layer *concept* is routing-shaped: "features come from `app/**/page.tsx` + orphan API routes." A Django app's features are `urls.py` + views, Rails' are `routes.rb` + controllers — same spirit, different mechanics. We've only seen one stack's answer.

Making this pluggable, by increment:

- **M10** — doc-mark the coupled files, dedup the grammar pick, land the SQL extractor / matcher ad hoc. *(done)*
- **M11** — `src/lang.rs` centralizes the mechanical coupling and adds `detect(root)`; free functions, no trait. *(done)*
- **Phase 2** — the full breakdown (M12 interim de-coupling → M13-pre entry-point spike → M13 `StackPack` trait → M14 `RustStack` → M15 two-stack checkpoint → M16 `JavaStack` on commons-lang → M17 Quarkus on quarkus-super-heroes → M18 iterate + ship) is in "Phase 2 — the polyglot core first" below, including the complete M1–M11 coupling inventory and the M13 design decisions. It leads Phase 2 because it's the only step that validates the seams, and every milestone since M10 keeps adding to what it has to unwind.

**Why this order:** M10 was on the critical path to M11's corpus; modularization is on none. Doing M10/M11 first also means the eventual trait is designed from a real sample (two extractor kinds, four convention resolvers, the feature model) rather than guessed at. Neither order buys the strong validation — that needs the second language.

---

## Phase 2 — the polyglot core first

Reprioritized 2026-09-07: the `StackPack` work leads. Everything after it (web viewer, headless generation, HTTP, search index, multi-repo) is deferred behind it and stays a sketch — committed `.md` specs are already browsable on GitHub, so the human-audience viewer isn't blocking anything, and every one of those later items is easier to build on a core that isn't secretly single-stack.

The polyglot core is **M12 → M13-pre spike → M13 → M14 → M15 → M16 → M17 → M18**, in order:
- **M12** (done) — cache-version fix + named seams; protects everything after it.
- **M13-pre** (done) — paper spike, before M13 freezes the trait: CodeOwl's own feature layer (for M14) + a forward sketch of the Java entry-point model (for M16/M17). → `experiments/exp-02-feature-layer.md`.
- **M13** (done) — the `StackPack` trait; the existing TS+Next+SQL+Supabase code is now behind `TypeScriptNextStack`. Still one pack. Validated inert on the pilot.
- **M14** (done) — `RustStack` on CodeOwl's own repo. Exercised the seams; no trait shape leaks. `FORMAT_VERSION` 3 → 5.
- **M15** (code + docs done) — inherent `impl Foo` folds into `Foo` (M14's one finding); symbol source spans follow a folded method; doc-neutralization first pass; `symbol.rs`/`graph.rs` self-specs re-rendered. `FORMAT_VERSION` 5 → 6. The exhaustive self-spec corpus is deferred to a follow-up (owner scoped M15 down).
- **M16** (done, branch not merged) — `JavaStack` on **commons-lang**: a third stack the TS/Rust impls didn't shape, and the first real corpus with `feature_model() -> None`. `tree-sitter-java` extractor; `record`/`@interface` → `Container` (decision 1 resolved); resolution keys on path-suffix FQN + same-package scan, not `pom.xml` (internal resolution came in at 100 %); N-way `detect`. Dogfood validated the zero-feature path and spec quality on 7 shared-code specs. Headline M18 finding: `get_next_spec_task` returns a folded Java class's whole-file source, over the MCP token limit.
- **M17** — **Quarkus** on **quarkus-super-heroes**: the feature layer for a Java *service* — heterogeneous entry-point kinds, multi-module Maven, the JPA-vs-migrations schema call, `@RegisterRestClient` cross-service edges.
- **M18** — fold M16/M17's findings; generalize the schema layer off its `.sql`-file assumption; finish stack-neutral docs; ship the polyglot core.

M12/M13 are refactors that must regenerate the Phase 1 corpus **identically** — the *spec files* and `get_spec_coverage` output, not the `.codeowl/` cache (its format is expected to change, see M12). Everything from M14 on is real new stacks. The Java track (M16–M17) was pulled into Phase 2 ahead of the deferred items because a JVM service corpus is the highest-value second target for this project's owner; Rust-on-self stays M14 because it validates the seams for near-zero cost.

**Named `StackPack`, not `LanguagePack`, on purpose.** The Phase 1 pack already spans two languages (TypeScript + SQL) and two frameworks (Next.js App Router + Supabase). The unit of pluggability is a *stack*, not a language — calling it a language pack invites someone to write a separate SQL pack and then wonder why it can't see the TS side. One `StackPack` per repo.

### The coupling being unwound

From building all of it in M1–M11, the TS+Next coupling is:

**Mechanical** (mostly already behind `lang.rs`): the grammar pick, `is_extractable` / `is_schema_file` extension lists, `RESOLVER_EXTENSIONS`, the `.sql`-vs-code extract dispatch, `detect(root)`.

**Extraction** (`extract.rs`, `schema.rs`): every `node.kind()` string is a tree-sitter-typescript / tree-sitter-sequel grammar name; `SymbolKind::{Function,Class,Method,Const,Table}`; signature rendering; `/** */` JSDoc docstrings; the class→method Merkle rollup.

**Reference resolution** (`imports.rs`, `resolve.rs`): ES `import`/`export`/default-import parsing; `oxc_resolver` with `tsconfig.json` alias discovery; named-re-export (barrel) chasing.

**Framework conventions** (`features.rs` — the deepest): `enumerate_entry_points` (Next.js `app/**/page.tsx` + orphan `app/api/**/route.ts` file-routing); `feature_slug`; `extract_route_literals` + `resolve_route_literal` (`fetch("/api/…")` → route file by path convention); `extract_table_refs` + `resolve_table_ref` (Supabase `.from("table")`); `extract_rendered_components` + `resolve_rendered_component` (React `<Tag/>`); the `is_colocated` / `file_does_data_work` `core`-admission heuristics; the whole `assemble_participants` participant model. **Every flow edge is unresolved at extraction time** — `RouteLiteral` holds a path string, `TableRef` a table name, `RenderedComponent` a JSX tag — and resolved against the graph later by a pack-specific matcher. Resolution and the `core`-admission policy are both pack judgment, not just extraction.

**Leaked into the generic core**: `graph.rs`'s four pack-specific derived-edge fields (`route_literals`, `table_refs`, `rendered_components`, `resolved_default_imports`); `spec.rs::prioritize`'s `is_test_path` (JS-ecosystem) and `is_ui_primitive` (`components/ui/`); `spec.rs` granularity keying on `SymbolKind::{Function,Class}` + `is_exported`.

Genuinely generic, untouched by all of this: the arena (`graph.rs`), `hash.rs`, `index.rs`'s `RepoIndex` / catch-up / watcher, `watch.rs`, `mcp.rs` (tools, task shapes, `{"kind":"done"}`), `spec.rs`'s document format + staleness diffing + the `prioritize` *tiering* logic itself, `search.rs`.

**Not in Phase 2:** grammars stay compile-time-linked crates (`tree-sitter-typescript`, `tree-sitter-sequel`, and M14's `tree-sitter-rust`). "Add a stack without recompiling" — the WASM-grammar story in `ARCHITECTURE.md`'s "Implementation stack" — is a later concern; a `StackPack` is a Rust `impl`, selected by `detect()`.

### Where the risk concentrates

Extraction, import resolution, `SymbolKind` mapping — tedious but well-understood; a bug there is caught by a failing test. **The feature layer is the hard part, because it's the only part of CodeOwl that models a *product*, not *code*.** Symbols/imports/containment/hashing are universal. *"A feature is an entry point plus what it reaches"* is a claim about how **web apps** are shaped — and it's exactly what makes the corpus valuable to a BA (the dormant-feature catch that validated M11 came out of a feature spec). Two nested unknowns:

- **Does the concept survive a non-web stack?** (M13-pre). Pick wrong and the second pack's corpus is much less useful — and you only find out when a human reads it.
- **The `core`-admission policy has no mechanical ground truth.** `is_colocated || does_data_work` was reverse-engineered from *one* repo over three iterations this session (the first two were wrong). Generalizing it is harder than generalizing extraction because "correct" means "the spec reads well," which needs a human per stack.

De-risking is built into the plan: M13-pre before the trait freezes, `feature_model()` optional, and M14's validation is a human dev cut, not a diff.

---

### M12 — Interim de-coupling (no trait yet)
**Size:** S · **Builds on:** M11

**Scope:** Make every leaked seam *visible and named*, and version the cache, with no behavior change — so M13 is a mechanical lift rather than archaeology.
- **Cache format version.** `RepoIndex` (and the persisted `Graph`) currently have no version field, so a `FileInputs`/`Graph` shape change is silently absorbed by `#[serde(default)]` and yields *wrong* data (empty `rendered_components`, etc.) until every file happens to change — M11 shipped this latently. Add a `format_version: u32` to the persisted index/graph; on read, a mismatch (or absence) triggers a full rebuild, not a partial reuse. **Do this first** — it's the smallest standalone fix and it protects everything M13+ does.
- Move `is_test_path` and `is_ui_primitive` out of `spec.rs`, behind a named seam (`lang.rs` or a `conventions` module); `spec.rs::prioritize` calls through it. While there, reshape them: `is_ui_primitive` generalizes to nothing (a Rust stack returns constant `false` — a trait method one impl no-ops is a smell). Replace both with a single `classify(path) -> FileRole` returning `Domain | Primitive | Test | Generated` — `Primitive` covers `components/ui/`, `Test` covers `is_test_path`, and `Generated` (protobuf output, codegen) is something every stack has and `prioritize` should already be sinking.
- Doc-mark `graph.rs`'s four derived-edge fields as "pack-contributed — empty on a repo the active pack doesn't cover," grouped and commented as a unit.
- Fold `resolved_default_imports` into the import-resolution output path (it's just resolution) rather than a parallel field, if it's clean to do.
- Introduce a `SourceKind` (`Code` | `Schema`) so the `.sql` dispatch in `lang::extract_symbols` is a named branch, not a string check.

**Validation:** `cargo test` pass count unchanged; `codeowl extract` on the pilot produces identical symbol JSON (cache format aside); a fresh `--all` on the pilot regenerates the existing spec files byte-for-byte; `git diff` touches only `spec.rs`, `graph.rs`, `lang.rs`, `index.rs`, `imports.rs`/`resolve.rs`.

---

### M13-pre — Entry-point spike (on paper, before M13 finalizes the trait)
**Size:** XS–S · **Builds on:** M12

**Scope:** Two written answers (not code), in one note:

1. **What would CodeOwl's own feature specs be?** (drives M14.) Candidate answers — entry points are `main.rs`'s CLI subcommands + the `#[tool]` MCP handlers; or the crate's `pub` API surface; or a library/CLI stack legitimately has **no** feature layer and the system spec composes over module rollups alone. Each is defensible; the wrong pick makes M14's self-corpus much less valuable, and it's not checkable mechanically — a human reads the resulting specs and judges. This is why M13 makes the feature layer an *optional* pack capability rather than a required trait method.

2. **What is a Java entry point?** (forward sketch for M16/M17, so M13's `trait FeatureModel` isn't shaped by Rust + Next alone.) commons-lang (M16): almost certainly `None` — a pure utility library with no runtime entry surface. A Quarkus service (M17): **several kinds** of entry point coexisting in one repo — JAX-RS `@Path` resources, `@Incoming`/`@Outgoing` Kafka listeners, `@Scheduled` jobs, `@GrpcService` methods, `main`. Implication for M13: `EntryPoint` carries a *kind*; a route slug is a Next-ism, not a universal name; `enumerate_entry_points` may need to return heterogeneous kinds.

**Deliverable:** `experiments/exp-02-feature-layer.md`. Conclusions fold into `ARCHITECTURE.md` open question 4 and the M13 / M14 / M16 / M17 plans as each starts.

**Validation:** the note is concrete enough that M14 *and* M16 can execute against it without re-litigating.

---

### M13 — The `StackPack` trait; extract the TS+Next pack behind it
**Size:** M–L (see "test churn" below) · **Builds on:** M12, M13-pre

> **Status: complete (2026-09-07, PR #7).** All three pieces landed across 8 commits (`3b6e34a`…`d0e4bbd`). `src/stack.rs` holds `trait StackPack` + `TypeScriptNextStack`; `graph.rs`'s four typed edge fields are one `flow_edges: Vec<FlowEdge>`; the feature layer is behind `trait FeatureModel` + `TypeScriptNextFeatureModel` with a generic `assemble_participants` walk. `FORMAT_VERSION` 1 → 3. Design decisions 1/2/4 decided here (see the list below); 8/9 came from M13-pre. **Validated inert on the pilot** two ways — `structural_sweep.py` byte-identical vs `origin/master` across `codeowl extract` / `get_spec_coverage` / `get_next_spec_task` (103 targets) / `get_callers` / `get_callees` / `get_symbol`, and a real generation run where the rebuilt v3 graph reconciles exactly (`flow_edges` 759 = 29 route-literal + 304 table-ref + 426 rendered-component, the old three collections) and specs written pre-M13 still read as current. Test churn came in well under the ~97 budgeted: 4 call sites (2 integration tests + 2 `spec.rs` tests) moved onto `default_feature_model()`; the rest ride free-function shims that are genuinely part of `TypeScriptNextStack`. Not done here (deferred, see "Known issues" in `CLAUDE.md`): routing `spec.rs`/`mcp.rs` through `pack.feature_model()` (M14/M15), and the `SymbolKind` enum reshape itself (M14).

#### In plain terms

Today CodeOwl only knows how to read TypeScript/Next.js, and that knowledge is *wired directly* into the code that builds the graph. M13 puts a **universal socket** on CodeOwl's core (`trait StackPack`) and builds one **adapter** that fits it (`struct TypeScriptNextStack`). The adapter for now just forwards every call to the code that already exists — **nothing behaves differently**. The point is the socket: once it's there, adding Rust (M14) or Java (M16) is writing a new adapter, not editing the core.

**Still one adapter.** M13 ships with exactly `TypeScriptNextStack`. `lang::detect(root)` looks at the repo, sees `.ts`/`.tsx`, and hands back that one pack; anything else is an error. M14 is where a second adapter first proves the socket is shaped right.

#### The three pieces of work

1. **The socket + moving TypeScript behind it.** Define `trait StackPack` — the list of questions the core needs answered about *any* codebase (what files do you read, what's in this file, where does this import point). `TypeScriptNextStack` answers each by calling today's functions in `extract.rs` / `imports.rs` / `resolve.rs` / `schema.rs` / `features.rs`, which stay `pub` as thin shims. The core (`index.rs`, later `spec.rs`) stops naming those functions and asks `pack.…` instead.

2. **Collapse four graph fields into one.** `graph.rs` currently carries four separate, TypeScript-shaped lists of "this file reaches that thing" — API-route calls (`route_literals`), DB-table calls (`table_refs`), rendered React components (`rendered_components`), resolved default-imports. Replace all four with **one generic list**, `flow_edges: Vec<FlowEdge>`. The pack produces the raw edges (`pack.extract_flow_edges`) and resolves each one against the built graph (`pack.resolve_flow_edge`). A stack with none of these conventions just produces an empty list.

3. **The feature layer becomes optional and pack-owned.** "A feature is a web page plus everything it reaches" is a Next.js idea. Move the Next-specific parts (`is_page`, `is_api_route`, route-path joining, the `is_colocated || does_data_work` rule for what counts as a feature's `core`) *inside* `TypeScriptNextStack`. What stays generic in `features.rs` is a plain graph walk with two holes the pack fills: `resolve_flow_edge` (where does this edge point) and `admits_to_core` (does that file belong in the feature, or stay a one-line dependency). A stack with no feature concept at all returns `feature_model() -> None`, and the system spec is composed from module rollups alone — M14 (CodeOwl-on-itself) and M16 (commons-lang) both do this; see `experiments/exp-02-feature-layer.md`.

#### Progress

- **Commit 1 (`3b6e34a`) — done.** `ExtractedSymbol`/`Symbol`/`SymbolView` gain `markers: Vec<String>` for annotations (M13 design decision 9 — the Java feature/schema model is entirely annotation-driven; empty for TS). `FORMAT_VERSION` 1 → 2.
- **Commit 2 (`fcac579`) — done.** `trait StackPack` (piece 1) with `name` / `source_kind` / `classify` / `extract_symbols` / `extract_imports` / `resolve_imports`; `TypeScriptNextStack` delegating to the free functions; `detect() -> Box<dyn StackPack>`; `RepoIndex` routed through `self.pack.*`. Zero test churn (build/open signatures unchanged — they call `detect()` internally). A test asserts each method returns exactly what the old free function does.
- **Commits 3–4 (`9e5c427`, `cb774df`) — done.** Drop the redundant `route_literals` parameter (read it off the graph); collapse `graph.rs`'s three typed edge collections (`route_literals` / `table_refs` / `rendered_components`) into one `flow_edges: Vec<FlowEdge { from_file, kind, raw, target: FlowTarget }>`, resolved at build time by `pack.extract_flow_edges` + `pack.resolve_flow_edge`. `FORMAT_VERSION` 2 → 3. Byte-identical on the pilot.
- **Commit 5 (`9f9d751`) — done.** The feature layer (piece 3). `trait FeatureModel { enumerate_entry_points, admits_to_core }` + `TypeScriptNextFeatureModel` (holds `is_page` / `is_api_route` / the route-path slug / the `is_colocated || does_data_work` rule); `StackPack::feature_model() -> Option<&dyn FeatureModel>` (default `None`; TS returns `Some`). `assemble_participants` is now a generic walk over `flow_edges` + one-hop imports — a file-node edge joins `core` iff `fm.admits_to_core`, a symbol-node edge is the `data` tier. `EntryPoint { file, slug }` → `{ kind, id, title, file }` with `id` == the old slug (spec filenames unchanged). No persisted-shape change, so `FORMAT_VERSION` stays 3. Test churn absorbed: 2 integration tests + 2 spec.rs tests move onto `default_feature_model()`; the rest ride the `enumerate_entry_points` free-fn shim.
- **Commit 6 (`d0e4bbd`) — done.** Design decision 1 (`SymbolKind`) decided → option (a); documented, implemented in M14.
- **Verification — done.** `structural_sweep.py` on the pilot came back byte-identical vs `origin/master` across all six categories; a real generation run confirmed the rebuilt v3 graph reconciles (see the Status box above). PR #7 merged.

#### The trait, current + planned shape

```
trait StackPack {
    fn name(&self) -> &str;                                        // "typescript-next" — DONE
    fn source_kind(&self, path: &Path) -> Option<SourceKind>;      // reads this file? code or schema? — DONE
    fn classify(&self, rel_path: &str) -> FileRole;                // Domain | Primitive | Test | Generated — DONE
    fn extract_symbols(&self, rel_path: &str, source: &str) -> Vec<ExtractedSymbol>;   // DONE
    fn extract_imports(&self, rel_path: &str, source: &str) -> FileImports;            // DONE
    fn resolve_imports(&self, root: &Path, file_imports: &HashMap<String, FileImports>, graph: &Graph)
        -> Vec<ResolvedImport>;                                    // pack owns the resolver — DONE

    fn extract_flow_edges(&self, rel_path: &str, source: &str) -> Vec<UnresolvedFlowEdge>;    // DONE
    fn resolve_flow_edge(&self, graph: &Graph, edge: &UnresolvedFlowEdge) -> FlowTarget;      // DONE

    // OPTIONAL — None means "this stack has no feature specs" (M13-pre). Default `None`;
    // TS overrides. M14/M16 take the default; M17 gives it a second impl.
    fn feature_model(&self) -> Option<&dyn FeatureModel>;                                     // DONE
}

trait FeatureModel {                                                                         // DONE
    fn enumerate_entry_points(&self, graph: &Graph) -> Vec<EntryPoint>;
    fn admits_to_core(&self, graph: &Graph, entry: &EntryPoint, candidate_file: &str) -> bool;
}

// `kind` is pack-owned free text — a Quarkus service has http-resource / kafka-consumer /
// scheduled-job / grpc entry points at once, so no closed Page|ApiRoute enum. `id` must be
// unique across kinds (it names the spec file). `file`, never SymbolId — a SymbolId is only
// valid for the graph that made it and must never reach a spec file. TS: kind is
// "page"/"api-route", id is the route slug (unchanged), title is the URL path.
struct EntryPoint { kind: String, id: String, title: String, file: String }                  // DONE
```

`graph.rs` ends up with **one** `flow_edges: Vec<FlowEdge { from_file, target: FlowTarget, kind: String }>`, resolved at build time by `pack.resolve_flow_edge` on each `UnresolvedFlowEdge`. `assemble_participants` becomes a generic walk over `flow_edges` + one-hop imports with those two pack hooks. The `SymbolKind` question (design decision 1) is **decided and written down in M13 but not implemented** — reshaping the enum with no second stack to check against is exactly what design decision 8 warns against; M14 does the remap when it adds Rust's kinds.

#### Test churn — real, not "zero"

~97 direct call sites of pack functions across `src/` (23 in `features.rs`, 19 in `extract.rs`, 13 in `imports.rs`, 10 in `spec.rs`, …) plus `tests/`. Plan: keep the free functions `pub` as shims through the refactor so existing tests compile untouched, then a dedicated commit moves tests onto the trait (or drops the shim and takes one reviewed churn commit). Commits 1–2 hit **zero** churn by not changing public signatures; the flow-edge / feature-layer commit won't be so lucky.

**How it actually went:** far under budget — 4 call sites total (2 integration tests + 2 `spec.rs` tests) moved onto `default_feature_model()`, because the pack shims (`extract_route_literals`, `enumerate_entry_points`, …) are genuinely part of `TypeScriptNextStack`'s implementation and testing them directly is correct, not debt. No dedicated churn commit was needed.

#### Validation

The pilot's **spec files** and **`get_spec_coverage` output** are byte-identical to `origin/master` — that's the whole test: a refactor that changes what CodeOwl *writes* is a bug. The `.codeowl/graph` JSON on disk **is** expected to change (new `flow_edges` shape, new `format_version`). Every existing test passes.

`utility/structural_sweep.py` is the standard check for this (and every Phase 2 refactor milestone) — it builds two `codeowl` binaries and diffs `codeowl extract`, `get_spec_coverage`, `get_next_spec_task` for every feature/rollup/system/sample-file, and `get_callers`/`get_callees`/`get_symbol`, against a target repo, restoring its `.codeowl/` afterwards:

```
python3 utility/structural_sweep.py --repo ~/dev/startup/talentTrail
```

**Result:** every commit came back fully byte-identical on the pilot — `codeowl extract`, `get_spec_coverage`, `get_next_spec_task` (103 targets), `get_callers` (25 tables), `get_callees`, `get_symbol`, all against `origin/master`. Cross-checked with a real generation run: the M13 binary rebuilt `talentTrail/.codeowl/` to `format_version: 3` with `nodes: 1122` (unchanged) and `flow_edges: 759` — exactly `29 + 304 + 426`, the totals of the old `route_literals` / `table_refs` / `rendered_components` — and feature specs written earlier (against a pre-M12 binary) reconciled as *current*, confirming the participant-hash path is inert through M12+M13.

> **Note (stale-binary gotcha, worth keeping):** the first pilot generation run silently used a months-old `target/release/codeowl` because `talentTrail/.mcp.json` pointed there and the dev loop only builds `target/debug/`. The pre-M12 binary has no `format_version` and happily rewrote the cache in the old format. Fix: point `.mcp.json` at `target/debug/codeowl` (kept current by `cargo build`/`cargo test`), or rebuild release before every generation run.

---

### M14 — `RustStack` — the second stack
**Size:** L · **Builds on:** M13, M13-pre

> **Status: code complete (2026-09-08, branch `m14-rust-stack`).** 8 commits (`86b591f`…): `SymbolKind` reshaped to `Container | Callable | Value | Schema` + pack-owned `raw` (design decision 1); `tree-sitter-rust` extractor in `src/rust.rs` (fn/struct/enum/trait/impl/mod/const/static/macro_rules, `#[cfg(test)] mod tests` skipped, `///`+`//!` docs, `#[derive]`/attrs → `markers`); `detect()` branches TS vs Rust and errors on ambiguity (design decision 6); module-tree import resolution (`use crate::`/`super::`/`self::` + one `pub use` hop, no `oxc_resolver`); `spec.rs`/`mcp.rs` routed through a persisted `Graph::pack_name` for `classify` + `feature_model` (`RustStack` takes the trait default `None`; the zero-feature system-spec path is verified); `.mcp.json` dogfood config + a `## Key flows` clause for a no-feature-layer system spec. `FORMAT_VERSION` 3 → 5. `structural_sweep.py` stayed byte-identical on the pilot through commit 6; commit 7 (`scoped_symbol_deps` — fold unresolved externals into one `externals: …` line) is a deliberate shared-render change Rust exposed. **Self-corpus validation:** ~35 specs generated across `symbol.rs` / `imports.rs` / `graph.rs` — extraction correct (private fns, generics, tuple-structs, `impl` blocks all handled), resolution working (every internal `use crate::` resolved), prose coherent and well-referenced, **no trait shape leaks**. Findings folded here: stale `build_graph_from_sources` / `extract_and_hash` comments (now `#[cfg(test)]`-gated) and the `FORMAT_VERSION` history. Deferred to M15: the exhaustive ~240-spec corpus + `rollup:src` + `system`, a deps re-render pass on pre-commit-7 specs, and whether to merge an inherent `impl` into its type's spec section.

**Scope:** A real `StackPack` for Rust, exercised on **CodeOwl's own repo** (the dogfood — see the test-repo list). This is the milestone that validates *most* of the trait; M12/M13 only rearrange TS+Next code. **Scope caveat from the M13-pre spike:** since M14 returns `feature_model() -> None`, it validates every part of `StackPack` *except* `FeatureModel` — that seam gets its second implementation only at M17. Keep `FeatureModel` minimal in M13 accordingly (see design decision 8).
- `tree-sitter-rust`. Extract `fn`, `struct`, `enum`, `trait`, `impl` blocks, `mod`, `const`/`static`, `macro_rules!`. Map its grammar kinds onto whatever `SymbolKind` shape M13 settled on.
- Module-tree reference resolution: `use crate::…` / `use super::…` / `pub use` re-exports resolved against the `mod` hierarchy — no `oxc_resolver`, no `tsconfig`, no barrel-chasing.
- `///` / `//!` doc comments.
- **`feature_model() -> None`** — the M13-pre spike's answer (`experiments/exp-02-feature-layer.md`). CodeOwl has workflows but no mechanical way to enumerate them; instead the self-corpus's **system spec** carries named sections for the four cross-cutting flows (extraction / spec generation / structural query / incremental reindex). Verify `spec.rs`'s system-spec composition tolerates an empty feature set (design decision 3).
- Flow edges: a Rust service's cross-file "reach" is plain function calls, not string literals — M14 decides how much call-graph to model (probably: `extract_flow_edges` returns nothing, matching M10's "call analysis stays deferred").

**Validation:** `codeowl extract` + `serve` on `~/dev/openSource/codeowl`; generate a spec corpus for CodeOwl describing itself; a dev cut on ~10 of those self-specs by the one person who knows the code line-for-line. Every place `TypeScriptNextStack` assumed something the trait shouldn't have is a finding for M15.

---

### M15 — Iterate the trait; ship the TS+Rust state
**Size:** M · **Builds on:** M14

> **Status: code + docs done (2026-09-08, branch `m15-trait-iterate-self-corpus`).** M14's one real finding — a Rust type rendering as two spec sections (`## Graph` + `## impl Graph`) — is fixed: `rust::merge_inherent_impls` folds an inherent `impl Foo` block into the `Foo` symbol after the tree-sitter walk (methods reparented, `source_hash` Merkle-folded, `markers` unioned), so `spec.rs` sees a Rust type exactly as it sees a TS class — **zero `spec.rs` change**, the socket held. Trait impls (`impl Trait for Foo`) stay their own symbols. Incidental fix: two `impl CodeOwlServer` blocks were colliding on one arena id. Follow-on: `spec::symbol_span_text` — a symbol's source span now also covers a folded method's body, wherever it sits in the file, so `### Depends on` and the generation task see method-only deps (`SpecTask::Symbol` loses its unused `lines` field; `read_lines`/`read_symbol_text` replaced). `FORMAT_VERSION` 5 → 6. Doc-neutralization first pass: `ARCHITECTURE.md` §1/§2 "per language" → "per stack" + a `StackPack` paragraph; "Feature specs / Making them derivable" now leads with "these are a pack's *optional* hooks" and names each generic seam; `setup/codeowl-generate.md` drops the hard-coded `app/submit/page.tsx`. Self-corpus: `symbol.rs` / `graph.rs` re-rendered for the folded shape (`imports.rs` unaffected). Also `get_spec_coverage` now reports `generations_remaining` (and a per-`pending` `generations` share) — the real `--budget=N` for a complete pass, counting uncovered symbols not just documents (CodeOwl's own remaining corpus ≈ 240). **Deferred to a follow-up:** the exhaustive self-corpus + `rollup:src` + `system` (the `## Key flows` one) — the owner scoped M15 to the trait iteration + docs + the 3-file re-render. 180 unit + integration green; `structural_sweep.py` byte-identical on the pilot (the additive `generations` fields folded away like `markers`).

**Scope:** Fold M14's findings back into `trait StackPack` — the leaks that only a genuinely different stack exposes. Commit CodeOwl's self-spec corpus. First pass at making the docs stack-neutral: rewrite `ARCHITECTURE.md`'s "Extractors" / "Feature specs" sections and `setup/codeowl-generate.md` (it currently says "a page like `app/submit/page.tsx`" — pack-specific examples move behind "your stack's entry points"). This is the **two-stack checkpoint, not the end of the polyglot core** — M16–M17 add Java and M18 does the final trait iteration once a non-Rust, non-web stack has had its say.

**Validation:** both the pilot (TS+Next) and CodeOwl (Rust) generate current corpora from the same binary with no stack-specific branches outside the two `StackPack` impls.

---

### M16 — `JavaStack` on commons-lang — a third stack, and the no-feature-layer path
**Size:** L · **Builds on:** M15

> **Status: code + commons-lang dogfood done (2026-09-09, branch `m16-java-stack`).** `src/java.rs` — `tree-sitter-java` over `class` / `interface` / `enum` / `@interface` / `record` + methods / constructors / annotation elements / fields, nested types recursed, `::` id separator, Javadoc `/** */` as docstring (leading-`*` stripped), `@Annotation`s → `markers`. **`SymbolKind` decision (design decision 1) resolved:** every named type kind — records and `@interface` included — maps to `Container`, no size test. A record is a restricted `final class` (JLS); modelling it as anything else would drop a record-per-file DTO layout out of the corpus (`file_is_spec_bearing` needs an exported `Container`/`Callable`) and mis-file a type declaration as `Value`. Supersedes the "record → `Value` unless it has non-accessor methods" lean — commons-lang has zero records (Java 8) so M17 (Quarkus) is where it gets real exercise. No `merge_inherent_impls` step: a Java class body already holds its members. **Resolution:** `import a.b.C` maps to the walked `.java` file whose path *ends* `a/b/C.java` — a path-suffix match, so no source-root discovery, no `pom.xml`, Gradle and (for M17) multi-module free; static / nested-type imports fall back to resolving the specifier as the type FQN; `import a.b.*` is recorded and resolves to nothing. Plus **same-package implicit edges**: each file's source is scanned for the simple name of every top-level container in its own directory (`spec::contains_identifier`-style whole-word match) — this is what keeps a Java dependency graph from being artificially sparse. `JavaStack` in `stack.rs` delegates to `crate::java::*`; `classify` → `Test` for `src/test/`, `Generated` for `target/generated-sources/` + Gradle `build/generated/`; `feature_model()` = trait default `None`; `extract_flow_edges` empty. `lang::detect` restructured from `match (ts, rs)` to an N-candidate count-and-match; `is_test_tree` gains a `src/test/` clause so commons-lang's 363 test files don't inflate the count. `FORMAT_VERSION` unchanged (`pack_name`, v5, already forces a rebuild for a new pack). 200 tests (187 unit + `tests/java_spec.rs` ×2).
>
> **commons-lang dogfood (the validation gate — `serve` + `/codeowl-generate --all --budget=15`, human read of the 7 generated specs):** graph builds clean — `pack_name: java`, `format_version: 6`, 14,725 nodes. **Internal import resolution 100 %** (1,373/1,373; external `java.*` correctly `None`); 1,244 same-package implicit edges, verified accurate (e.g. `CharUtils` → `ArrayUtils.setAll` / `StringUtils.isEmpty`, real calls with no import). `feature_model() -> None` composes fine — `get_spec_coverage` reports 0 features, the pending queue is files → rollups → system, no crash. Spec quality is high — the generated `StringUtils` / `CharUtils` / `Strings` specs name private internals (`ordinalIndexOf`, `splitWorker`, `CsStrings`/`CiStrings`), per-overload exception contracts, constants, the null-in-null-out convention. PR #17's smelly-prose rejection didn't interfere (all 15 real-prose submits accepted). The 7 specs are left uncommitted in the commons-lang tree; committing the Java corpora is M18 scope.
>
> **Findings → M18:** (1) **[headline] `get_next_spec_task` payload size.** A folded Container's `source` is the whole class span ≈ whole file — `StringUtils` 420 KB, `ArrayUtils` 416 KB, `ClassUtils` 77 KB — every call over the MCP token limit, forcing the client to dump to a file and grep it. The class-level `### Summary`/`### Behavior` doesn't need 9,000 lines of method bodies. Fix: for a large Container task, send member signatures + docstrings, not full bodies. Java God-classes make this common; TS/Rust files are small enough to have hidden it. (2) `get_spec_coverage`'s `pending` also over-limit (95 KB / 626 files) — cap or paginate. (3) same-package scan over-includes an ambiguous simple name already covered by an explicit import (commons-lang has both `lang3.Streams` and `lang3.stream.Streams`; a `Streams.of(` mention pulls in the wrong one too) — suppress a same-package hit when an import already binds that simple name. (4) cosmetic: a static-imported member (`import static …StringUtils.INDEX_NOT_FOUND`) resolves to the member id and lists alongside a full `StringUtils` dep — redundant; decide member-vs-class granularity for static imports. (5) not CodeOwl: the agent submitted symbol-format content to a file id once (self-corrected, cost ~1 generation) — `submit` could reject `### Summary`-prefixed content on a file id.
>
> **Found in passing — tracked outside M16:** (1) `spec::submit` stored prose that `prose_smells` would flag, and `next_task` re-offers a flagged spec indefinitely, so an unbounded generate loop hangs. Fixed on `fix/submit-rejects-smelly-prose` (PR #17): `submit`/`submit_feature`/`submit_rollup`/`submit_system` reject smelly content at write time. (2) `src/resolve.rs::specifier_to_rel_path` strips an **un-canonicalized** `repo_root` off oxc_resolver's symlink-resolved output — under a symlinked repo root (macOS `/var` → `/private/var`, `/tmp`) all TS import resolution returns `None` (13 `cargo test` failures on Mac vs. master). TS pack only — Rust and Java resolvers don't use oxc. **Fix after M16, own PR: canonicalize `repo_root` before the `strip_prefix`** (not `abs_from.parent()` — oxc's `resolve_file` already does `.parent()` internally).

**Scope:** A real `StackPack` for plain classic Java, exercised on **commons-lang** (627 files, single-module Maven — see the test-repo list). No framework, no schema, no runtime entry surface: the milestone that proves the trait survives a stack the TS and Rust impls didn't shape, *and* exercises `feature_model() -> None` end to end on a real corpus.
- `tree-sitter-java`. Extract `class`, `interface`, `enum`, `record`, `@interface` (annotation types), methods, nested types, `static`/instance fields. Map onto whatever `SymbolKind` shape M13 settled on (records and annotation types are the awkward cases).
- **Package → file reference resolution.** `import com.foo.Bar` → `com/foo/Bar.java` on the source root, plus same-package implicit imports and `import static`. No `oxc_resolver`, no alias config; the hard part is star imports (`import com.foo.*`) and resolving an unqualified name against the enclosing package + `java.lang`. **Key on the `src/main/java` layout convention, not on `pom.xml`** — that way a Gradle repo works for free; only multi-module *discovery* (M17) is build-tool-specific.
- Javadoc (`/** … */`) as the docstring, including the leading `*` strip.
- **`classify()` for Java.** `src/test/java` is the test root — today's `is_test_path` is JS-convention (`__tests__/`, `.test.`, `e2e/`) and matches nothing in a Maven layout. M12's pack-owned `classify(path) -> FileRole` is exactly the seam for this; `JavaStack` returns `Test` for the test source root. Generated: `target/generated-sources/**` (already gitignored, so mostly moot).
- **`feature_model()` returns `None`.** commons-lang is a library; its "surface" is its public API, already covered by symbol/file/rollup specs. Verify `spec.rs`'s system-spec composition tolerates zero features (M13 design decision 3).
- Flow edges: `extract_flow_edges` returns nothing (same call-analysis-deferred stance as Rust).

**Validation:** `codeowl extract` + `serve` on commons-lang; generate a partial corpus (shared-code tier + a sample of module rollups + the system spec); a human read of ~8–10 specs for correctness. Every place the trait still assumes a resolver config, a route-shaped feature, or a `.sql` schema file is an M18 finding. **Done** — `serve` + `/codeowl-generate --all --budget=15` produced 7 high-fan-in file specs, all read and judged accurate; internal resolution 100 %, `feature_model() -> None` composes; findings in the status box (headline: `get_next_spec_task` returns whole-file source for a folded Java class, over the MCP token limit).

---

### M17 — Quarkus: the feature layer for a Java service
**Size:** L · **Builds on:** M16

**Scope:** Extend `JavaStack` (or add a `QuarkusStack` that composes it) and exercise it on **quarkus-super-heroes** (7-module Maven reactor of independently-deployable services — see the test-repo list). This is where Java gets a feature layer and where the M13-pre "several kinds of entry point" sketch gets implemented.
- **Multi-module Maven resolution.** Each module has its own `src/main/java` root and its own `pom.xml`. One repo is still **one graph** — the walk covers every module's source root, so a cross-module import resolves by the same package→file rule M16 built, with no reactor modelling needed. What the reactor *does* buy is knowing which roots exist and which modules declare a dependency on which; only read the poms if a resolution ambiguity actually demands it.
- **Entry-point model — `feature_model() -> Some`.** `enumerate_entry_points` returns heterogeneous kinds: JAX-RS `@Path` + `@GET`/`@POST`/… resource methods, `@Incoming`/`@Outgoing` (or `@Channel`) Kafka listeners, `@Scheduled` jobs, `@GrpcService` methods. `EntryPoint.kind` distinguishes them; the human-facing title is derived per kind (HTTP verb + path, channel name, cron expr). **Slugs must be unique across kinds** — `GET /fights` and a Kafka consumer on channel `fights` would both slug to `fights` and collide on `docs/specs/_features/fights.md`. Kind-prefix the `id` (`http-fights`, `kafka-fights`) or otherwise disambiguate; the pilot never hit this because pages and API routes came from disjoint path spaces.
- **`admits_to_core`.** A Java service's `core` is CDI-shaped, not co-location-shaped. Note this is **type-reference classification, not dataflow**: an `@Inject`ed field or constructor param has a declared type, that type resolves through the ordinary import graph to a file, and the annotations on *that* file's class decide admission (`@ApplicationScoped`/`@Singleton` service or Panache repository/entity → admit; a framework primitive like an injected `Config` or `ObjectMapper` → stay a stub dependency). No call analysis needed. "Does data work" ⇒ "touches an `@Entity`/`PanacheRepository`, or calls a `@RegisterRestClient`". The M11 `is_colocated || does_data_work` policy is a Next-ism; expect iterations against the one repo, as M11 warns.
- **Schema decision (deferred from M10's `.sql`-only assumption).** Quarkus persistence is JPA `@Entity` / Panache active-record / Hibernate — annotations on Java classes, not a `.sql` dump — or Flyway/Liquibase migrations (`.sql` / `.xml` under `src/main/resources/db`). M17 picks one as the `SymbolKind::Table` source for Java and records why; the other is a follow-up.
- **`@RegisterRestClient` cross-service flow edges.** A `@RegisterRestClient` interface plus its `@Path` is a string-carried edge from one service to another's endpoint — the same shape as M10's `.from("table")` and the pilot's `fetch("/api/…")`. `extract_flow_edges` / `resolve_flow_edge` handle it.

**Validation:** `codeowl serve` on quarkus-super-heroes; generate feature specs for a sample of endpoints across ≥2 services plus the cross-service edges; a human read judging whether a BA-style "how does *fights* call *heroes*" question is answered. Findings feed M18.

---

### M18 — Iterate the trait; make the schema layer stack-neutral; ship the polyglot core
**Size:** M · **Builds on:** M17

**Scope:** Fold M16 + M17's findings back into `trait StackPack` / `trait FeatureModel` — the leaks a non-Rust, non-web stack exposed that M15 couldn't have seen. Generalize the schema layer so `SymbolKind::Table` isn't `.sql`-file-bound (M10's assumption; M17 broke it). Finish the doc neutralization M15 started. Commit the Java corpora (commons-lang partial + quarkus-super-heroes feature sample).
- **`get_next_spec_task` payload size (M16's headline finding).** A folded Container's `source` is its whole span — for a Java God-class (`StringUtils` ~9 k lines) that is 60–420 KB, over the MCP result token limit; the client has to persist and grep it. The class-level `### Summary`/`### Behavior` prompt doesn't need every method body. Fix: for a Container task over some size, send member signatures + docstrings (+ private-helper bodies) in place of the full span. Also cap / paginate `get_spec_coverage`'s `pending` (626 files ⇒ 95 KB on commons-lang).
- **Same-package scan precision (M16).** Suppress a same-package implicit edge for a simple name that an explicit `import` in the same file already binds (commons-lang's `lang3.Streams` vs `lang3.stream.Streams`). Decide member-vs-class granularity for a resolved `import static`.

**Validation:** the pilot (TS+Next), CodeOwl (Rust), commons-lang (Java library), and quarkus-super-heroes (Java service) all generate current corpora from one binary, with every stack-specific decision inside a `StackPack` impl and nothing leaking into `graph.rs` / `spec.rs` / `index.rs`.

---

### Design decisions to resolve during M13

These are the "how exactly" questions M13 has to answer as it builds the socket. Each one is a place where a wrong early guess is expensive to undo once a second stack depends on it. "Leaning X" = the current best answer, to be confirmed (or overturned) while implementing; items now marked **decided** were settled during M13 (M13-pre for 3/8/9, the feature-layer commit for 1/2/4).

1. **`SymbolKind`** — **decided in M13 (option a); implemented in M14.** The enum becomes a small stack-neutral set the generic core reasons about, plus a pack-owned `raw: String` (e.g. `"record"`, `"macro_rules"`, `"@interface"`) carried on the symbol for display and pack-internal logic:

   ```
   enum SymbolKind { Container, Callable, Value, Schema }
   ```

   - `Container` — has members that get their own specs: TS `class`, Rust `struct`/`enum`/`trait`/`impl`/`mod`, Java `class`/`interface`/`enum`/`record`/`@interface`.
   - `Callable` — TS `function`/`method`, Rust `fn`, Java method/constructor.
   - `Value` — a data-carrier with no spec of its own: TS `const`, Rust `const`/`static`. Rolls up into its file spec.
   - `Schema` — today's `Table`; M18 widens it off the `.sql` assumption.

   `spec.rs`'s granularity rule becomes "generate a symbol spec for `Container` and `Callable`, fold `Value`/`Schema` into the file" — no pack branch. The mapping from grammar node kind → `SymbolKind` is the pack's (`extract_symbols` already returns `ExtractedSymbol.kind`); M14 does the actual enum reshape + remap of the TS mapping when it has `tree-sitter-rust` output to check the generic set against. **Java `record` (M16, resolved):** every named type kind → `Container`, no size test. A record is a restricted `final class`; modelling it as `Value` would drop a DTO-per-file layout out of the spec corpus (`file_is_spec_bearing` needs an exported `Container`/`Callable`) and mis-file a type declaration under `Value` (which is for `const`/field/variable). The earlier "`Value` unless it declares non-accessor methods" lean is overturned — commons-lang has no records to have informed it (Java 8); M17's Quarkus corpus is the real test.
2. **`Graph` derived edges** — leaning: one generic `flow_edges: Vec<FlowEdge>` (see M13), resolved at build time via `pack.resolve_flow_edge`; not four typed fields, not `Box<dyn Any>` pack data.
3. **Feature layer optionality** — decided (M13-pre forces it): `StackPack::feature_model() -> Option<&dyn FeatureModel>`. A stack with no feature model still gets symbol/file/rollup/system specs. `spec.rs`'s system-spec composition already has to tolerate "zero features" — verify that path. M16 (commons-lang) is the milestone that exercises `None` on a real corpus; the M14 Rust spike may or may not conclude `None`, but a pure utility library certainly does.
4. **`core` admission** — the traversal is generic; `resolve_flow_edge` and `admits_to_core` are the two pack hooks it can't do without. Do not try to make admission a generic rule — it's a per-stack heuristic with no mechanical ground truth (the TS one took three iterations against one repo).
5. **Resolution ownership** — pack owns imports *and* flow edges end to end. Output shapes (`ResolvedImport`, `FlowEdge`) are generic. No shared resolver infrastructure.
6. **One pack per repo** — `detect()` picks exactly one `StackPack` and errors on ambiguity (a repo that's both a Next app and a Rust service). Multi-pack in one repo is deferred past Phase 2. quarkus-super-heroes is *not* the counter-example it looks like: `ui-super-heroes/` is a Maven module wrapping ~10 JS files and the whole repo has **zero** `.ts`/`.tsx`, so `detect()` is unambiguous there. The real ambiguity case is still hypothetical — don't over-build for it.
7. **Cache invalidation on pack change** — `format_version` (M12) covers shape changes; also key the cache on the active pack's `name()`, so switching packs (or upgrading one) forces a rebuild rather than reusing edges the new pack wouldn't produce.
8. **`FeatureModel` stays minimal** — M14 (Rust) and M16 (Java library) both return `None`, so `FeatureModel` gets its *second* implementation only at M17. Two methods, no speculative surface: an interface with one real impl for three milestones will accrete TS+Next assumptions nobody can see. Anything M17 turns out to need is an M18 addition, not an M13 guess.
9. **Symbol-level markers** — the entire Java feature and schema model keys on **annotations** (`@Path` → entry point, `@Entity` → schema symbol, `@ApplicationScoped` → admits to core, `@RegisterRestClient` → flow edge). `ExtractedSymbol` today is `{id, kind, file, lines, signature, docstring, is_exported, source_hash, interface_hash, parent, children}` — there is nowhere to put them, and `signature` string-matching is not a substitute. M13 must add a pack-owned marker field (`markers: Vec<String>`, or a typed equivalent) while the symbol shape is already being touched. Cheap now; reopening `ExtractedSymbol` at M17 is not. Rust decorates too (`#[tool]`, `#[derive]`), so M14 exercises the field rather than leaving it dead.

---

### Deferred behind the polyglot core — sketch only

- **Web viewer** — a graph + spec browser for BAs/QA/SREs. Lower priority now that committed specs render on GitHub; still wanted for cross-cutting navigation and non-git-native readers.
- **Headless / scheduled spec generation** — a non-interactive runner (Claude Code SDK or `--print`, CI/git-hook triggered) driving the `get_next_spec_task → submit_spec` loop. CodeOwl still never calls an LLM itself.
- **HTTP/SSE transport** — `rmcp` over HTTP instead of stdio.
- **`tantivy` + ONNX embeddings** — the real search index deferred out of Phase 1.
- **Multi-repo namespacing** — per-repo index/graph/spec-cache in one shared process.
- **Stub nodes + cross-team delegation** — the cross-repo dependency model.
- **Auth/roles** — once multiple users share one hosted instance.

## Provisional decisions this ordering makes

Worth flagging since I made these calls rather than asking:
- **M10 (SQL) is sequenced before M11** so the corpus M11 generates can describe the database layer. It was once the one optional Phase-1 milestone; the reframe below makes it load-bearing — the pilot's domain is largely its schema, and a corpus that can't name tables fails M11's BA cut on any payments/registration question.
- **`CLAUDE.md`'s two pending items** (tool-surface trimming, spec-regen commit hygiene) aren't given their own milestones — the plan resolves the tool-surface one implicitly (M3–M8 only ever build the tools actually used: `get_spec`, `get_symbol`, `get_callers`/`get_callees`, `search_code`, the three generation tools, `get_spec_coverage` — `trace_path`/`get_tests_for`/`get_dependencies` simply never get built unless a later milestone needs them), and commit hygiene is a workflow habit to adopt once M8 exists, not something to build.

Revised 2026-09-06: the spec-format decision (see `ARCHITECTURE.md` "Spec document format") inserted the feature layer as its own M5 and renumbered everything after it — old M5–M9 are now M6–M10. Feature specs were promoted from a Phase 2 idea to a Phase 1 must-have after tracing a real pilot-repo feature and finding its two load-bearing hops (UI→API via `fetch("/api/…")`, API→DB via `.from("table")`) are string literals the import graph structurally cannot see — meaning directory-mirrored specs alone would have rigged M10's exit test against exactly the feature-shaped questions it needs to answer.

Revised again 2026-09-06, after M4's real-repo validation: M4 shipped the file-spec granularity rule but never wrote directory rollups (their document shape was never templated — see `ARCHITECTURE.md`'s open question 5), and that gap turned out to be silently load-bearing — the staleness milestone's own validation assumes a directory `_index.md` already exists to edit and re-check. Rather than leave that implicit, directory rollups became their own milestone, inserted as the new M6 right after the feature layer; everything from the old M6 (Staleness) onward shifted up by one, M6–M10 becoming M7–M11.

Revised again 2026-09-06, after M9: Phase 1's framing shifted. The original exit criterion — does spec generation measurably cut an agent's token use, a go/no-go gate on whether to continue — is retired. The value proposition is taken as settled: accurate, current, dual-audience (human + LLM) documentation of a brownfield codebase. So **M11 changes from "run tasks and reach a verdict" to "generate a real spec corpus over the pilot and hold it to a dual-audience quality bar"**; **M10's rationale shifts** from "makes M11 a fair test" to "lets the corpus describe the database" (which also makes it load-bearing rather than optional); the agent-side token measurement survives only as an optional data point. `REQUIREMENTS.md`'s problem statement, goal, and success criterion were updated to match, and Phase 2 was reprioritized — web viewer and headless generation move to the front, since the human audience and corpus freshness are now first-class concerns.

Revised again 2026-09-06: added CodeOwl's own repo to the test-repo list as the Rust case (Phase 2 grammar target), and added the "Stack modularization" section above — the TS+Next coupling gets made pluggable in three increments bracketing M10 (label + dedup now, `lang.rs` + `detect()` after M10, the `LanguagePack` trait + a second language in Phase 2). Order settled as M10-before-modularization: M10 is critical-path for M11's corpus and gives the eventual `lang.rs` boundaries a two-example sample to design against, and building M10 through the seams would only weakly test them anyway (it's a same-stack extension, not a second language).

Revised 2026-09-07: folded the stack-modularization increments into the milestone bodies so every actionable step lives in an Mn plan — the label + dedup prep is now explicitly part of M10's scope, and `src/lang.rs` + `detect(root)` is a named work item in M11's scope (independent of the corpus deliverable). The "Stack modularization" section is now a short cross-milestone overview, not a plan of its own. Phase 2's `LanguagePack` trait step is unchanged.

Revised again 2026-09-07, after M10 shipped: added an "Execution / sequencing" block to M11 — `src/lang.rs` and the data-touched participant wiring (deferred from M10) both land in the codeowl repo *before* corpus generation; the passes then run via `/codeowl-generate` from a session inside the pilot repo (not the manual stdio loop); the BA cut needs a human, the dev/smell cuts don't.

Revised again 2026-09-07, mid-corpus generation: the "Stack modularization" section was rewritten to name **two** kinds of coupling, not one. M10/M11's feature work (rendered-component `core` expansion, default-import resolution, the `--all` prioritization heuristics) added TS+Next-specific *model* coupling on top of the mechanical coupling `lang.rs` covers: `graph.rs` now carries four pack-specific derived-edge collections, `spec.rs::prioritize` gained `is_test_path`/`is_ui_primitive`, and the feature-layer concept is routing-shaped. Added an **interim step** (move those heuristics behind a named seam + doc-mark the `graph.rs` fields — no behavior change, makes the Phase 2 extraction mechanical) and promoted the `StackPack` trait to Phase 2 item 2, explicitly scoped to abstract both kinds of coupling.

Revised again 2026-09-07: **M11 marked validated by sample; Phase 1 complete.** The project owner reviewed the pilot's feature specs and judged them to clearly meet the BA and dev bars — the clinching example being a spec that correctly reports a feature as dormant after it was disabled by a one-line variable flip (`b6d8965f` in the pilot). Full corpus generation was stopped at ~15% coverage: the highest-value, highest-risk layer is proven, and the rest is mechanical volume. See M11's "Outcome" for the done/not-done split. Phase 2 is next, led by the `LanguagePack` trait.

Revised again 2026-09-07: **Phase 2 reprioritized and the polyglot core planned in detail**, then reviewed. The stack-pluggability work now leads Phase 2 — the web viewer drops behind it since committed `.md` specs already render on GitHub. The opening is M12 (interim de-coupling, incl. a **cache `format_version`** — M11 shipped a latent bug where a `#[serde(default)]` field silently reads empty on an old cache) → M13-pre (a paper spike on what CodeOwl's own feature specs would be) → M13 (`StackPack` trait — renamed from `LanguagePack`, since the Phase 1 pack already spans TS + SQL + Next + Supabase; a *stack*, not a language) → M14 (`RustStack` on CodeOwl's own repo, the real validation) → M15 (iterate the trait). Review pass added: the feature layer is an **optional** `feature_model()` (a CLI/library may have none); flow edges need `resolve_flow_edge` + `admits_to_core` pack hooks, not a "generic BFS"; `is_ui_primitive` → a `classify(path) -> FileRole` with `Domain/Primitive/Test/Generated`; the "zero behavior change" claim is scoped to *spec files + coverage output*, not the cache JSON; ~97 pack-function call sites mean real test churn, budgeted via shims. The feature layer is flagged as where the risk concentrates — it's the only part that models a product, not code, and its `core`-admission heuristic has no mechanical ground truth.

Revised again 2026-09-07, after M12 shipped: **the Java track pulled into Phase 2.** The owner's highest-value second target is a JVM service corpus, not the eventual Rust self-dogfood. Rust-on-self stays **M14** (it validates the trait seams for near-zero cost — no repo setup, no build system, no schema layer), and Java is added as **M16 (`JavaStack` on commons-lang)** + **M17 (Quarkus on quarkus-super-heroes)**, with **M18** folding the Java findings back into the trait and generalizing the schema layer off M10's `.sql`-file assumption. M13-pre gains a second question — a forward sketch of the Java entry-point model — so M13's `trait FeatureModel` is shaped for heterogeneous entry-point *kinds* (HTTP resource / Kafka listener / scheduled job / gRPC method), not just Next.js pages and Rust subcommands. M16 becomes the milestone that exercises `feature_model() -> None` on a real corpus (commons-lang is a pure utility library). Test-repo table reordered to milestone order; `leveldb` explicitly marked not-scheduled (C++ held in reserve).

Revised again 2026-09-07, after **M13 shipped** (PR #7): the `StackPack` trait is in — `src/stack.rs` with `trait StackPack` + `TypeScriptNextStack`, `graph.rs`'s four typed edge fields collapsed to one `flow_edges: Vec<FlowEdge>`, the feature layer behind `trait FeatureModel` + `TypeScriptNextFeatureModel` with a generic `assemble_participants`. `FORMAT_VERSION` 1 → 3. Design decisions 1 (`SymbolKind` → `Container|Callable|Value|Schema` + `raw`, reshape deferred to M14), 2 (one generic `flow_edges`), and 4 (`resolve_flow_edge` + `admits_to_core` as the two pack hooks) decided. Validated inert on the pilot — `structural_sweep.py` byte-identical vs `origin/master`, plus a real generation run confirming the rebuilt v3 graph reconciles. Test churn came in at 4 call sites, far under the ~97 budgeted. Deferred to M14: routing `spec.rs`/`mcp.rs` through `pack.feature_model()`, and the `SymbolKind` enum reshape. Next: **M14** — `RustStack` on CodeOwl's own repo.

Revised again 2026-09-08, **M14 code-complete** (branch `m14-rust-stack`, 8 commits + a wrap-up): the second `StackPack` is in and it exercised the whole trait. `tree-sitter-rust` extractor (`src/rust.rs`), `detect()` now branches on primary language and errors on a dual-stack repo, Rust import resolution is a filesystem-convention module-tree walk, and `spec.rs`/`mcp.rs` recover the pack from a persisted `Graph::pack_name` for `classify` + `feature_model` (`RustStack` → `None`, zero-feature system-spec path verified). `SymbolKind` reshaped (decision 1); `FORMAT_VERSION` 3 → 5. A ~35-spec self-corpus (symbol.rs / imports.rs / graph.rs) validated extraction and resolution and found **no trait shape leaks** — the findings were cosmetic (deps-list noise, fixed by `scoped_symbol_deps` folding externals into one line) or stale source comments (fixed in the wrap-up). Deferred to M15: the exhaustive self-corpus + `rollup:src` + `system`, and the impl-block spec-section question. Next: **M15**.

Revised again 2026-09-08, **M15 code + docs done** (branch `m15-trait-iterate-self-corpus`): the impl-block spec-section question resolved — an inherent `impl Foo` folds into the `Foo` symbol at extraction time (`rust::merge_inherent_impls`), so `spec.rs` still sees one containment unit per type with **no `spec.rs` change**; trait impls stay separate. Follow-on `spec::symbol_span_text` — a symbol's source span covers its folded methods wherever they sit — so method-only deps reach `### Depends on` and the generation task. `FORMAT_VERSION` 5 → 6. Doc-neutralization first pass (`ARCHITECTURE.md` §1/§2 + "Feature specs", `setup/codeowl-generate.md`). `symbol.rs`/`graph.rs` self-specs re-rendered. The owner scoped M15 to trait + docs + the 3-file re-render; the exhaustive ~240-spec self-corpus (+ `rollup:src` + `system`) is a named follow-up. Next: **M16** — `JavaStack` on commons-lang.

Revised again 2026-09-09, **M16 done** (branch `m16-java-stack`, 6 commits, not yet merged): `src/java.rs` (`tree-sitter-java`), `JavaStack` in `stack.rs`, `lang::detect` restructured from a 2-tuple to an N-candidate count-and-match, `tests/java_spec.rs`. Design decision 1's Java-`record` sub-question **resolved** — every named type kind → `Container`, no size test (a record is a restricted `final class`; `Value` would drop DTO-per-file layouts from the corpus). Resolution keys on path-suffix FQN matching + a same-package source scan, not `pom.xml`. `feature_model() -> None`; `extract_flow_edges` empty. `FORMAT_VERSION` unchanged. 200 tests. **commons-lang dogfood** (`serve` + `/codeowl-generate --all --budget=15`, 7 shared-code specs read): 14,725 nodes, **internal import resolution 100 %**, same-package edges accurate, `feature_model() -> None` composes, spec quality high. **M18 findings:** headline — `get_next_spec_task` returns a folded Container's whole-file source (`StringUtils` 420 KB), over the MCP token limit; plus `get_spec_coverage` `pending` over-limit, same-package scan over-inclusive on an import-shadowed simple name, static-import member-vs-class dep granularity. The 7 specs are left uncommitted in the commons-lang tree (corpora are M18 scope). Two bugs found in passing, tracked outside M16: the `submit`/`next_task` smelly-prose infinite loop (fixed, PR #17, **merged**) and the oxc_resolver `strip_prefix`-under-symlink bug that breaks TS resolution on macOS (fix after M16, own PR — canonicalize `repo_root`). Next: **M17**.
