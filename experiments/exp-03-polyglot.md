# exp-03 — Polyglot repos: one arena, many roots

**Status:** spike, on paper. Written before the polyglot milestone (the "Planned insert" in `ROADMAP.md`, provisionally M18 — shifting Quarkus → M19, iterate → M20) freezes any trait or cache shape.
**Feeds:** the polyglot milestone plan; `ARCHITECTURE.md` open question 3 ("Secondary-language subtrees") and the "Language & stack coverage" scope decision in `REQUIREMENTS.md`.
**Not a decision doc** — its conclusions fold into `ARCHITECTURE.md` and the milestone's own plan when it starts.

**Revised 2026-09-14, before first review** (three passes the same day, folded into one note — nothing here is a post-review change):

- **Q2 rewritten.** The original answer — primary gets full capability, every secondary gets a fixed *reduced mode* — is gone. The owner's framing: once a language has a `StackPack`, it should work the same way in *any* subtree, not only as a designated primary. Every root now gets whatever its own pack supports, gated on whether that capability actually fires, never on primary/secondary status. This is the substantive change; everything else follows from it.
- **Three of the four "Open, deliberately" items resolved** — the feature-layer question (by Q2 above), the two-firing-feature-models tiebreak (deterministic; Q1 step 4), and `packages/`/monorepo discovery (discover-by-default + an unreferenced-root nudge; Q1 step 2). Each rule lives in the section an implementer reads; the open-questions list keeps pointers as a decision log. Only the cross-language edge stays open, deliberately.
- **Accuracy pass.** Title changed (the old one named the split Q2 retired). File counts corrected against the repo as it stands today. The status-quo claim in "Why this spike exists" fixed: `detect()` errors cleanly on a polyglot root and has never silently picked a winner — verified by running it, not assumed.

---

## Why this spike exists

`detect()` today picks **exactly one** `StackPack` for a repo by primary-source-file count, and errors on genuine ambiguity (design decision 6). That holds for every repo on the schedule so far — the pilot (TS + a little SQL), CodeOwl (Rust), commons-lang and quarkus-super-heroes (Java), and M17's target is deliberately a *subdirectory* (`full-stack-fastapi-template/backend`) precisely to dodge the question.

But real repos are polyglot. `full-stack-fastapi-template` in full is a **41-file Python FastAPI backend + a 108-file React/Vite frontend** in one git repo (`detect()`'s own non-test counts, 2026-09-14). Point CodeOwl at the root today and it errors — cleanly, naming both stacks, refusing to guess:

```
Error: … contains more than one stack CodeOwl recognises — TypeScript/TSX (108 files),
Python (41 files). CodeOwl serves one stack per repo (ROADMAP.md design decision 6).
Point it at a subdirectory that's a single stack.
```

That error is the *right* behaviour for the current design — a silent pick would be worse, and design decision 6 is deliberate about that. But it means the only way to index this repo at all is to point at one subtree and accept that the other half is invisible: a `system` spec describing "the product" with a whole executable component missing is a spec that lies by omission. The workaround M17 actually shipped with (`serve <repo>/backend`) buys a correct Python corpus at exactly that cost — 108 frontend files CodeOwl knows nothing about.

The owner wants this worked **before** the Java service milestone (Quarkus), because polyglot repos are the higher-value target and `full-stack-fastapi-template` — the M17 repo — is the forcing case, already on disk.

Five questions the milestone can't answer from the current code. This note answers them on paper so the milestone executes without re-litigating.

---

## The concrete repo — `full-stack-fastapi-template`

```
full-stack-fastapi-template/
├── backend/                 pyproject.toml, uv.lock
│   └── app/                  40 .py total — FastAPI, SQLModel, routers, crud.py
│       └── alembic/versions  5 migration .py
├── frontend/                 package.json, vite.config.ts
│   └── src/                  101 .ts/.tsx total — React 19, TanStack Router + Query
│       └── client/           GENERATED — @hey-api/openapi-ts from the backend's OpenAPI
├── packages/react-email/     a few more .tsx (email templates)
└── scripts/                  2 shell
```

Counts are `find`'s, 2026-09-14; `detect()`'s own non-test tallies for the same tree are 41 Python / 108 TS-TSX (it counts a few files `find` filtered and excludes test trees differently). Either way the ratio is what matters below.

Facts that shape the answers:

- **The frontend is not Next.js.** Vite + TanStack Router (`createFileRoute("/_layout/items")`). `TypeScriptNextStack`'s feature model keys on `app/**/page.tsx` and would enumerate **zero** entry points here. Its *extraction* and *`oxc_resolver` module resolution* still work fine on `.ts`/`.tsx`.
- **The cross-language call is a generated-client method call, not a string.** A route component calls `ItemsService.readItems({ query: {...} })`. `ItemsService` lives in `frontend/src/client/sdk.gen.ts` (generated), where the path *is* a string literal (`url: '/api/v1/items/'`). So the frontend→backend edge is: `ItemsService.readItems` → `sdk.gen.ts` path literal → `GET /api/v1/items/` → `read_items` in `backend/app/api/routes/items.py`. Resolvable in principle, real work in practice.
- **File-count ranking gives the wrong primary.** `detect()`'s own non-test counts: frontend 108 `.tsx`/`.ts`, backend 41 `.py` — better than 2.5:1 for the frontend. "Most files wins" → the frontend is primary. That is backwards — the backend is the product, the frontend consumes a *generated* client of it, and a good chunk of that 108 *is* the generated client.

---

## Q1 — How does `detect()` choose a primary, and enumerate secondaries?

### The problem with "most non-test source files wins"

It's the current rule, and this repo breaks it. A generated API client, a large design-system component library, or a test-heavy service can all out-count the actual product. File count is a weak signal made weaker by asking it to pick a *primary* rather than just *a* stack.

### Candidate A — explicit config (`.codeowl.toml`)

```toml
[stack]
primary = "python"
primary_root = "backend"
secondary = [{ pack = "typescript", root = "frontend" }]
```

- **For:** unambiguous, self-documenting, survives a repo whose shape the heuristics guess wrong, and it's the natural home for other per-repo knobs later (ignore globs, the persistence idiom hint from Q on schema, feature-merge manifest from open question 4).
- **Against:** new config surface; every consumer repo needs one written by hand (the pilot's `.mcp.json` setup is already unscripted — this adds a second file). A repo with no config still needs *a* behaviour.

### Candidate B — "point at the primary; secondaries are auto-discovered siblings"

`serve <repo>/backend` → Python is primary (unambiguous — the subtree is ~all `.py`). CodeOwl then walks *up* one level, sees `../frontend` is a coherent `.ts`/`.tsx` subtree, and registers it as a secondary automatically.

- **For:** aligns exactly with the M17 decision (target the `backend/` subdir). No config. The human's choice of path *is* the primary signal, which is the strongest signal available.
- **Against:** "walk up and guess siblings" is its own heuristic; surprising if it pulls in a sibling the user didn't want. Doesn't help `serve <repo>` (root) at all.

### Candidate C — per-top-level-directory detection, then a primary rule

`detect()` runs per immediate subdirectory: `backend/` → Python, `frontend/` → TS, `packages/` → TS. Primary = the subtree whose pack has a **feature model that actually fires** (a `FastAPI()` / `@Path` / `app/**/page.tsx` present), falling back to file count when none or several do.

- **For:** "the subtree with a running product surface is primary" is the right intuition for this repo and most backend+frontend splits. No config for the common case.
- **Against:** "feature model fires" is a fragile primary signal — a pure-API backend with only a health check, or a Next.js frontend + a thin Python cron, would confuse it. Multiple feature models firing (a Next SSR frontend + a FastAPI backend) needs a tiebreak anyway.

### Recommendation: **B as the default, A as the override, a deterministic rule for ties.**

*(Originally "…C's instinct as the tiebreak." Retired: step 4 now rejects "feature model fires" as a tiebreak for the same fragility reason C's own "Against" names — and after Q2's revision the tie barely matters, so it doesn't warrant a judgment-call signal at all. C's real contribution to this recommendation is its per-subdirectory detection idea, which step 2 uses; its primary-selection rule is not adopted.)*

1. **`serve <path>`** — the path *is* the primary. `detect()` runs on it as today; if it's unambiguous, that pack is primary.
2. **Secondary discovery** — from the primary root, look at sibling and child directories that the primary pack's `source_kind` rejects wholesale but another registered pack claims as a coherent subtree (≥ some floor, e.g. 5 non-test files, and > some fraction of that directory). Each becomes a secondary `(root, pack)`. **Discovery is by default, never gated on whether another root imports it** — the tempting alternative ("only discover subtrees something actually references") would silently omit a real-but-unreferenced subtree, a service still being scaffolded or a genuinely standalone CLI tool, which is a false negative in the same direction as the frontend-invisibility bug this milestone exists to fix; it also needs the primary's imports resolved before the roots list is settled, which inverts the order everything else runs in. The floor already filters real noise (a 2-file tooling package doesn't clear it); the residual false-positive risk is better paid down by a human writing one `.codeowl.toml` exclusion than by an import-graph heuristic. A discovered root that nothing else references still gets discovered — plus a **non-blocking nudge**: *"`packages/react-email` wasn't found to be imported anywhere else — confirm this belongs, or exclude it in `.codeowl.toml`."* Informational, never a gate.
3. **`.codeowl.toml`** — if present, its `[stack]` block wins outright over 1–2. This is also where a repo says "ignore `packages/`" or "the frontend is a secondary but don't bother".
4. **Several valid roots, more than one with a firing feature model** — **not an error, and no config required.** Pick a primary by a fully deterministic rule (alphabetical by root path), and let `.codeowl.toml`'s `[stack] primary` override it for anyone who cares. This is safe to under-specify *because of Q2's revision*: every root gets its own feature layer regardless of which one is called primary, so the choice only decides `prioritize`'s generation order within a budgeted run — not what ends up in the corpus. A cleverer heuristic here would repeat the mistake this section already rejected twice (file count; "feature model fires") — both are fragile as *primary-selection* signals, and neither gets less fragile when several models fire at once. A boring deterministic rule matches the actual stakes.
5. **`serve <repo-root>` with genuine ambiguity and no config** — still an error, but a *better* one: "found Python under `backend/` and TypeScript under `frontend/` — point me at one (`serve <repo>/backend`) or add a `.codeowl.toml`." Note this is a *different* case from step 4: step 4 is discovery **succeeding** and yielding several coherent roots (just needing one named primary); step 5 is discovery **failing** to resolve the tree into coherent per-pack subtrees at all.

M17's `serve <repo>/backend` keeps working unchanged; the milestone adds step 2 (discovery), step 3 (config), and step 4 (the tiebreak).

---

## Q2 — What does a secondary pack *do*? (revised: no fixed "reduced mode")

*Original framing, superseded below:* "a secondary is not 'run the full pack on a subtree' — which capabilities does a fixed reduced mode expose?" That framing is the wrong shape. The question isn't what a *secondary* gets; it's what *every root* gets, and the answer is: **whatever its own pack supports, applied to its own subtree, exactly as if that subtree were the only thing CodeOwl was pointed at.**

| Capability | Every root | Why |
|---|---|---|
| `extract_symbols` | ✔ | file/symbol specs need it; cheap |
| `classify` | ✔ | test/generated sorting still matters everywhere |
| `extract_imports` / `resolve_imports` | ✔ | the same `oxc_resolver` / module-tree walk any single-stack repo already gets — cheap, and what makes `get_callers`/`get_callees` work *inside* a subtree ("what uses this hook?") |
| `extract_flow_edges` / `resolve_flow_edge` | ✔, scoped to the root | needed by that root's *own* `feature_model()`, if it has one — a Next.js frontend secondary can't get real feature specs without its own route-literal/rendered-component edges |
| `feature_model()` | ✔, gated on firing | a CLI/library pack returns `None` regardless of root kind (unchanged, existing behavior); a pack whose model *does* fire for that subtree (real routes, real entry points) gets real feature specs, whether it's the primary or not |

**What's actually excluded, and it's a different axis than "primary vs. secondary":** only work that crosses a root boundary. Resolving `ItemsService.readItems` (a frontend root) to `read_items` (a Python root) needs a generated-client parser plus cross-graph matching — Q4's problem, genuinely separate from whether either side gets its own feature layer, and it stays deferred regardless of this revision.

**Why this is a better answer than the original table, not just a different one:** the old version made "does this subtree get a feature layer" depend on an accident of which path you typed into `serve`, not on whether the subtree has a real product surface. Two repos with the *identical* set of subtrees, pointed at from different starting directories, would have produced different corpora under the old model — the same subtree tree having two different valid interpretations depending on which one you called "primary" is a smell, not a feature. Under the revised model, the corpus a polyglot repo produces is the same regardless of which root you pointed `serve` at first; "primary" stops being a capability gate and becomes purely which root `detect()`'s discovery step starts walking outward from (Q1) — an ergonomics/bootstrapping detail, not a modeling decision.

**Cost, honestly.** The old design's "secondary = reduced" was partly a scope decision, not only a capability one — bounding feature-layer work to one root keeps a repo with several subtrees cheap to generate. The revised model doesn't reopen that: feature-layer cost is still bounded by *how many roots' packs actually produce entry points*, not by root count. This repo's own Vite frontend is the concrete proof — its `feature_model()` correctly produces zero entry points either way (Vite/TanStack Router, not Next.js's `app/**/page.tsx` convention), so nothing about this validation repo's expected output changes. What the revision actually changes is the *next* repo: a Next.js frontend as a secondary, previously guaranteed no feature layer by construction, now correctly gets one if its routes are real — which is what the "whether a secondary ever gets a feature layer" question was waiting on a real example to decide, and this resolves it without needing one.

So every root gets: symbol specs, file specs, directory rollups, a working reference graph *within itself*, and — if its own pack's `feature_model()` fires for it — feature specs too. What no root gets, ever, in the first cut: a resolved edge *to* or *from* another root.

The `system` spec composes over **every root**, uniformly — each root's module rollups, and each root's feature specs if it produced any — so "the product is a FastAPI service with a React SPA client" is sayable regardless of which one happened to be primary. That's the one place all the graphs meet in the first cut.

---

## Q3 — One arena, or a `Graph` per language?

### Candidate A — one arena, symbols tagged by pack

Every symbol from every subtree lands in the same `Graph`, carrying which pack produced it (extend the existing persisted `Graph::pack_name` → a per-symbol `pack` or a `roots: Vec<(PathBuf, String)>` map).

- **For:** `spec.rs` / `mcp.rs` / `index.rs` already reason over one `Graph`; the granularity rules, the four hashes, and rendering don't change. `SymbolId`s stay globally unique in one arena (they already are). The `system` spec composing over everything is trivial — it's all one graph.
- **Against:** `resolve_imports` must not try to resolve a `.tsx` import against a `.py` file — resolution has to be scoped to a subtree. But that's a `for each (root, pack)` loop around today's single call, not a structural change.

### Candidate B — a `Graph` per subtree, federated

`RepoIndex` holds `Vec<(root, pack, Graph)>`; queries fan out.

- **For:** hard isolation — no chance of a cross-language `SymbolId` collision or a mis-scoped resolve.
- **Against:** every consumer of `Graph` (`spec.rs`, `mcp.rs`, `get_callers`, coverage) grows a "which graph / all graphs" dimension. This is the `graph.rs`-touching change design decision 8 warns against — and there's no second stack *in the same graph* forcing it, because the secondary is deliberately edge-isolated from the primary.

### Recommendation: **A — one arena, resolution scoped per subtree.**

`RepoIndex` gains `roots: Vec<RootSpec { path, pack_name, kind: Primary | Secondary }>` (replacing the single `root` + `pack`). The catch-up/watch walk visits every root; `extract_*` dispatches on the owning root's pack; `resolve_imports` runs once per root against only that root's files. `graph.rs` is untouched except for the `roots` metadata. `FORMAT_VERSION` bumps (the persisted `RepoIndex`/`Graph` shape changes) — expected, not a concern (`pack_name` already forces a rebuild on pack change; this generalises it).

> **`kind` is ordering metadata, never a capability gate.** Post-Q2, the only thing `Primary | Secondary` decides is `prioritize`'s tiebreak (which root's work sorts first within an otherwise-equal tier) and where discovery started. Any `if kind == Secondary { … skip … }` that suppresses extraction, resolution, flow edges, or `feature_model()` is the exact bug Q2 exists to prevent, and this struct is where it would most naturally get reintroduced — flagging it here on purpose.

One real subtlety, updated by Q2's revision: **`classify` and the shared-code / fan-in tiers** in `spec.rs::prioritize` currently assume one pack's conventions, and — since a non-primary root can now produce real feature specs too (Q2) — a secondary's files are no longer barred from the shared-code or feature tiers on principle; a high-fan-in file or a firing entry point in a secondary root earns its tier the same way a primary root's does. What *does* still favor the primary: within a budgeted `--all` run, primary-root work sorts first as a tiebreak when tiers are otherwise equal — the root you pointed `serve` at is the one you most likely want covered first — not because a secondary's work is excluded from a tier, only deprioritized within it.

---

## Q4 — Cross-language edges: in or out for the first cut?

The edge in this repo: `ItemsService.readItems` (frontend) → `read_items` (`backend/app/api/routes/items.py`). It's genuinely valuable — it's the "how does the UI talk to the API" story, exactly what feature specs exist for.

**Out for the first cut.** Reasons:

- It needs the primary (Python) graph *fully resolved first*, then a second pass parsing the generated `sdk.gen.ts` for its path literals, then matching those against the primary's `EntryPoint` set. That's the M18/Quarkus `@RegisterRestClient` machinery (`extract_flow_edges` / `resolve_flow_edge` across a boundary) plus a generated-client parser — a milestone's worth on its own.
- The generated client is a moving target (`@hey-api/openapi-ts` output format is not stable across versions).
- Without it, the first cut still delivers the main win: the frontend stops being invisible. File specs, rollups, and a `system` spec that names both halves.

**What to leave a hook for:** a `## How the pieces fit` section on the `system` spec — **a new section this milestone would add**, not one the current format has — should be *allowed* to describe the frontend↔backend relationship in prose even without a resolved edge, since the LLM can see both subtrees' summaries. That's a prompt/rendering note for the milestone, not a mechanism. (Closest existing precedent: the `## Key flows` section a no-feature-layer system spec already gets.)

Revisit the resolved edge once M18 has built the cross-service `@RegisterRestClient` resolver — the shapes are the same ("a string in a client call → an endpoint in another graph").

---

## Q5 — Test repo

`full-stack-fastapi-template` (whole repo, not the `backend/` subdir M17 uses). Same clone, already on disk. It exercises:

- primary discovery via `serve <repo>/backend` + secondary auto-discovery of `../frontend`;
- a secondary whose pack (`TypeScriptNextStack`) has a feature model that correctly **does not fire** (Vite, not Next) — validating that a non-firing model produces no feature layer *because it doesn't fire*, the same as it would for a primary, not because secondaries are excluded by rule (Q2's revision);
- `packages/react-email/` as a *third* subtree — under the resolved discovery rule (Q1 step 2) it **should** be pulled in as a second secondary if it clears the floor, and should trigger the unreferenced-secondary nudge if nothing else imports it. Both halves of that are worth checking here: that it's discovered at all, and that excluding it via `.codeowl.toml` actually works — this is the forcing case for the config surface.

No second repo needed for the first cut. A backend+frontend split where the frontend *is* Next.js (so its feature model *would* fire) is the obvious follow-up repo — under Q2's revision the expected outcome is no longer an open question (that root gets real feature specs, symmetrically with the primary), just unvalidated against a real repo. Deferred with the resolved cross-language edge.

---

## Consolidated implications for the milestone

1. **`RepoIndex`: `root: PathBuf` → `roots: Vec<RootSpec>`.** One primary, zero-or-more secondaries. The walk, catch-up, and watcher iterate roots; `extract_*` and `resolve_imports` dispatch per root's pack. `FORMAT_VERSION` bump.
2. **`detect()` gains a discovery step** — from the primary root, find sibling/child subtrees another registered pack claims (floor: ~5 non-test files). A subtree clearing the floor is discovered **by default**, never gated on being imported by another root; one with zero inbound references still gets discovered, plus a non-blocking "confirm this belongs, or exclude it" nudge. Plus a `.codeowl.toml` `[stack]` block that overrides everything.
2b. **Primary selection needs no cleverness** — `serve <path>` makes the path primary; if several discovered roots have firing feature models, break the tie deterministically (alphabetical by root path), `.codeowl.toml` overriding. Never an error for *this* case — only for discovery genuinely failing to resolve coherent subtrees (Q1 steps 4–5).
3. **No fixed reduced mode (revised, Q2).** Every root runs its own pack's full capability set against its own subtree — `extract_symbols`, `classify`, `resolve_imports`, `extract_flow_edges`/`resolve_flow_edge`, and `feature_model()` if the pack has one and it fires. Nothing is structurally suppressed for being a "secondary"; a secondary's `feature_model()` result is used exactly like the primary's. The only thing that's still primary-favoring is `prioritize`'s tiebreak order (primary-root work sorts first within an otherwise-equal tier), not capability.
4. **One arena.** `graph.rs` untouched bar the `roots` metadata. Resolution scoped per root.
5. **`system` spec composes over every root uniformly** — each root's module rollups, and each root's feature specs if it produced any. The one place the graphs meet in the first cut. `spec.rs`'s system composition already tolerates "zero features"; it now also has to tolerate "modules (and possibly features) from more than one pack."
6. **No cross-language edges.** `fetch`/generated-client → route resolution is deferred behind M18's `@RegisterRestClient` work.
7. **`.codeowl.toml`** is introduced here (minimal: `[stack]` only). Document it in `setup/`.

### Not changing

- `graph.rs`'s arena, edge types, `SymbolId` scheme.
- The four hashes, granularity rules, staleness diffing.
- `feature_model()` / `is_schema_symbol` / the trait shape themselves — unchanged; what changed (Q2) is only that every root's pack now gets to *call* all of it, not a fixed subset gated on primary/secondary.
- M17's `serve <repo>/backend` flow — still valid, now a subset of the general case.

---

## What the milestone executes against

**Validation:** `codeowl serve ~/dev/openSource/test-repos/full-stack-fastapi-template/backend` discovers `frontend/` as a secondary. Generate:

- the Python backend corpus as M17 already specifies (unchanged);
- file specs + directory rollups for a sample of the frontend (`src/routes/`, `src/components/Items/`, `src/hooks/`) — a human read confirms extraction/resolution is right and that **no feature specs** were produced for it *because `TypeScriptNextFeatureModel` correctly finds no `app/**/page.tsx` convention to fire on* (Vite + TanStack Router, not Next.js) — not because it's a secondary. This is also where the revised Q2 gets its first real check: if a feature spec *did* somehow appear for this frontend, that's a bug, but "it's a secondary" must not be the reason cited for its absence anywhere in the implementation;
- `get_callers` on a frontend hook lists its frontend callers (intra-subtree resolution works);
- the `system` spec — a human read confirms it names *both* the FastAPI service and the React client, and describes their relationship in prose even though no resolved edge connects them.

Every place `spec.rs` / `mcp.rs` / `index.rs` needs a "which pack / which root" branch that isn't a clean `for root in roots` loop is a finding — the socket should hold here too, one layer up.

---

## Open, deliberately

Kept as a decision log — struck-through items are settled, and the rule each one settled on now lives in the section an implementer would actually read. Only the last bullet is still genuinely undecided.

- ~~**`serve <repo-root>` with two firing feature models**~~ — Resolved (2026-09-14): deterministic tiebreak, not an error and not a config requirement. **Rule now lives in Q1 step 4 + implication 2b.** Why it's safe to settle so cheaply: Q2's revision means both roots get a feature layer either way, so the tiebreak only decides generation *order*, not corpus content.
- ~~**Whether a secondary ever gets a feature layer**~~ — Resolved (2026-09-14): yes, symmetrically, gated only on whether that root's own `feature_model()` fires. **Rule now lives in Q2 + implication 3.** Caveat worth carrying forward: still unvalidated against a repo where a *secondary's* model actually fires — this repo's Vite frontend correctly produces zero entry points either way, so the revision is reasoned, not yet observed. Decided, but not yet proven.
- ~~**`packages/` / monorepo workspaces**~~ — Resolved (2026-09-14): discover by default once the floor is cleared, excludable via `.codeowl.toml`, plus a non-blocking nudge when nothing references a discovered root. **Rule now lives in Q1 step 2 + implication 2.** Explicitly rejected the tempting alternative ("only discover what something imports"): it would silently omit a real-but-unreferenced subtree — a service still being scaffolded, a standalone CLI tool — which is a false negative in the same direction as the frontend-invisibility bug this milestone exists to fix, and it needs the primary's imports resolved before the roots list is even settled.
- **The resolved frontend→backend edge** — still open, deliberately. Deferred behind M18's cross-service `@RegisterRestClient` resolver; the shapes match ("a string in a client call → an endpoint in another graph"). This is the one item here that isn't a decision this doc should make.
