# exp-03 — Polyglot repos: one primary stack + secondary-language subtrees

**Status:** spike, on paper. Written before the polyglot milestone (the "Planned insert" in `ROADMAP.md`, provisionally M18 — shifting Quarkus → M19, iterate → M20) freezes any trait or cache shape.
**Feeds:** the polyglot milestone plan; `ARCHITECTURE.md` open question 3 ("Secondary-language subtrees") and the "Language & stack coverage" scope decision in `REQUIREMENTS.md`.
**Not a decision doc** — its conclusions fold into `ARCHITECTURE.md` and the milestone's own plan when it starts.

---

## Why this spike exists

`detect()` today picks **exactly one** `StackPack` for a repo by primary-source-file count, and errors on genuine ambiguity (design decision 6). That holds for every repo on the schedule so far — the pilot (TS + a little SQL), CodeOwl (Rust), commons-lang and quarkus-super-heroes (Java), and M17's target is deliberately a *subdirectory* (`full-stack-fastapi-template/backend`) precisely to dodge the question.

But real repos are polyglot. `full-stack-fastapi-template` in full is a **~19-file Python FastAPI backend + a ~86-file React/Vite frontend** in one git repo. Point CodeOwl at the root and one of two bad things happens: it errors on ambiguity, or — worse — it counts more TypeScript than Python and analyses the *frontend*, calling the backend invisible. A `system` spec that describes "the product" with a whole executable component missing is a spec that lies by omission.

The owner wants this worked **before** the Java service milestone (Quarkus), because polyglot repos are the higher-value target and `full-stack-fastapi-template` — the M17 repo — is the forcing case, already on disk.

Five questions the milestone can't answer from the current code. This note answers them on paper so the milestone executes without re-litigating.

---

## The concrete repo — `full-stack-fastapi-template`

```
full-stack-fastapi-template/
├── backend/                 pyproject.toml, uv.lock
│   └── app/                  ~19 .py — FastAPI, SQLModel, routers, crud.py
│       └── alembic/versions  5 migration .py
├── frontend/                 package.json, vite.config.ts
│   └── src/                  ~86 .ts/.tsx — React 19, TanStack Router + Query
│       └── client/           GENERATED — @hey-api/openapi-ts from the backend's OpenAPI
├── packages/react-email/     a few more .tsx (email templates)
└── scripts/                  2 shell
```

Facts that shape the answers:

- **The frontend is not Next.js.** Vite + TanStack Router (`createFileRoute("/_layout/items")`). `TypeScriptNextStack`'s feature model keys on `app/**/page.tsx` and would enumerate **zero** entry points here. Its *extraction* and *`oxc_resolver` module resolution* still work fine on `.ts`/`.tsx`.
- **The cross-language call is a generated-client method call, not a string.** A route component calls `ItemsService.readItems({ query: {...} })`. `ItemsService` lives in `frontend/src/client/sdk.gen.ts` (generated), where the path *is* a string literal (`url: '/api/v1/items/'`). So the frontend→backend edge is: `ItemsService.readItems` → `sdk.gen.ts` path literal → `GET /api/v1/items/` → `read_items` in `backend/app/api/routes/items.py`. Resolvable in principle, real work in practice.
- **File-count ranking gives the wrong primary.** Non-test, non-generated: frontend ≈ 70 `.tsx`/`.ts`, backend ≈ 19 `.py`. "Most files wins" → the frontend is primary. That is backwards — the backend is the product, the frontend consumes a *generated* client of it.

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

### Recommendation: **B as the default, A as the override, C's instinct as the tiebreak.**

1. **`serve <path>`** — the path *is* the primary. `detect()` runs on it as today; if it's unambiguous, that pack is primary.
2. **Secondary discovery** — from the primary root, look at sibling and child directories that the primary pack's `source_kind` rejects wholesale but another registered pack claims as a coherent subtree (≥ some floor, e.g. 5 non-test files, and > some fraction of that directory). Each becomes a secondary `(root, pack)`.
3. **`.codeowl.toml`** — if present, its `[stack]` block wins outright over 1–2. This is also where a repo says "ignore `packages/`" or "the frontend is a secondary but don't bother".
4. **`serve <repo-root>` with genuine ambiguity and no config** — still an error, but a *better* one: "found Python under `backend/` and TypeScript under `frontend/` — point me at one (`serve <repo>/backend`) or add a `.codeowl.toml`."

M17's `serve <repo>/backend` keeps working unchanged; the milestone adds step 2 (discovery) and step 3 (config).

---

## Q2 — What does a secondary pack *do*? ("reduced mode")

A secondary is not "run the full pack on a subtree." The question is which pack capabilities a secondary exercises.

| Capability | Primary | Secondary (proposed) | Why |
|---|---|---|---|
| `extract_symbols` | ✔ | ✔ | file/symbol specs need it; cheap |
| `classify` | ✔ | ✔ | test/generated sorting still matters |
| `extract_imports` / `resolve_imports` | ✔ | **✔** | see below — cheap and high-value *within* the subtree |
| `extract_flow_edges` / `resolve_flow_edge` | ✔ | ✘ | cross-language edges are Q4; intra-frontend `fetch` strings aren't worth it alone |
| `feature_model()` | ✔ | ✘ | a secondary has no BA-facing narrative of its own in this model |

**Revision from the handoff's first proposal.** The handoff floated "symbols + rollups only, no resolution." On reflection, **keep `resolve_imports` for secondaries** — it's the same `oxc_resolver` / module-tree walk the primary uses, it's cheap, and it's what makes `get_callers`/`get_callees` work *inside* the frontend ("what uses this hook?"). Dropping it would make the secondary graph nearly inert for no real saving. What a secondary drops is the **feature layer** and **cross-language flow edges** — the parts that model a product or cross a language boundary.

So a secondary subtree gets: symbol specs, file specs, directory rollups, and a working reference graph *within itself*. It does **not** get: feature specs, a slot in the feature tier of `--all`, or resolved edges to/from the primary.

The `system` spec composes over **both** — the primary's module rollups + feature specs *and* each secondary's top-level rollup — so "the product is a FastAPI service (primary) with a React SPA client (secondary)" is sayable. That's the one place the two graphs meet in the first cut.

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

One real subtlety: **`classify` and the shared-code / fan-in tiers** in `spec.rs::prioritize` currently assume one pack's conventions. A secondary's files must sort into the long tail (or after it), never the shared-code tier or the feature tier — they have no feature to feed. Simplest rule: **secondary files always sort after every primary file**, regardless of fan-in, the same way test code already does.

---

## Q4 — Cross-language edges: in or out for the first cut?

The edge in this repo: `ItemsService.readItems` (frontend) → `read_items` (`backend/app/api/routes/items.py`). It's genuinely valuable — it's the "how does the UI talk to the API" story, exactly what feature specs exist for.

**Out for the first cut.** Reasons:

- It needs the primary (Python) graph *fully resolved first*, then a second pass parsing the generated `sdk.gen.ts` for its path literals, then matching those against the primary's `EntryPoint` set. That's the M18/Quarkus `@RegisterRestClient` machinery (`extract_flow_edges` / `resolve_flow_edge` across a boundary) plus a generated-client parser — a milestone's worth on its own.
- The generated client is a moving target (`@hey-api/openapi-ts` output format is not stable across versions).
- Without it, the first cut still delivers the main win: the frontend stops being invisible. File specs, rollups, and a `system` spec that names both halves.

**What to leave a hook for:** the `system` spec's `## How the pieces fit` (or equivalent) section should be *allowed* to describe the frontend↔backend relationship in prose even without a resolved edge — the LLM can see both subtrees' summaries. That's a prompt/rendering note for the milestone, not a mechanism.

Revisit the resolved edge once M18 has built the cross-service `@RegisterRestClient` resolver — the shapes are the same ("a string in a client call → an endpoint in another graph").

---

## Q5 — Test repo

`full-stack-fastapi-template` (whole repo, not the `backend/` subdir M17 uses). Same clone, already on disk. It exercises:

- primary discovery via `serve <repo>/backend` + secondary auto-discovery of `../frontend`;
- a secondary whose pack (`TypeScriptNextStack`) has a feature model that correctly **does not fire** (Vite, not Next) — validating that "secondary ⇒ no feature layer" is the right cut and not a limitation;
- `packages/react-email/` as a *third* subtree — does discovery pull it in, and should it? (probably: yes as a second secondary, or excluded via `.codeowl.toml` — a good forcing case for the config).

No second repo needed for the first cut. A backend+frontend split where the frontend *is* Next.js (so its feature model *would* fire, and we'd have to decide whether a secondary ever gets a feature layer) is the obvious follow-up repo — deferred with the resolved cross-language edge.

---

## Consolidated implications for the milestone

1. **`RepoIndex`: `root: PathBuf` → `roots: Vec<RootSpec>`.** One primary, zero-or-more secondaries. The walk, catch-up, and watcher iterate roots; `extract_*` and `resolve_imports` dispatch per root's pack. `FORMAT_VERSION` bump.
2. **`detect()` gains a discovery step** — from the primary root, find sibling/child subtrees another registered pack claims (floor: ~5 non-test files). Plus a `.codeowl.toml` `[stack]` block that overrides everything.
3. **Secondary = reduced mode** — `extract_symbols` + `classify` + `resolve_imports` (scoped to the subtree), **no** `feature_model`, **no** flow edges. Enforced structurally: a secondary's `feature_model()` result is ignored, its files sort after all primary files in `prioritize`.
4. **One arena.** `graph.rs` untouched bar the `roots` metadata. Resolution scoped per root.
5. **`system` spec composes over primary rollups + primary features + each secondary's top-level rollup.** The one place the graphs meet in the first cut. `spec.rs`'s system composition already tolerates "zero features"; it now also has to tolerate "modules from more than one pack."
6. **No cross-language edges.** `fetch`/generated-client → route resolution is deferred behind M18's `@RegisterRestClient` work.
7. **`.codeowl.toml`** is introduced here (minimal: `[stack]` only). Document it in `setup/`.

### Not changing

- `graph.rs`'s arena, edge types, `SymbolId` scheme.
- The four hashes, granularity rules, staleness diffing.
- `feature_model()` / `is_schema_symbol` / the trait shape — a secondary just doesn't call some of it.
- M17's `serve <repo>/backend` flow — still valid, now a subset of the general case.

---

## What the milestone executes against

**Validation:** `codeowl serve ~/dev/openSource/test-repos/full-stack-fastapi-template/backend` discovers `frontend/` as a secondary. Generate:

- the Python backend corpus as M17 already specifies (unchanged);
- file specs + directory rollups for a sample of the frontend (`src/routes/`, `src/components/Items/`, `src/hooks/`) — a human read confirms extraction/resolution is right and that **no feature specs** were produced for it;
- `get_callers` on a frontend hook lists its frontend callers (intra-subtree resolution works);
- the `system` spec — a human read confirms it names *both* the FastAPI service and the React client, and describes their relationship in prose even though no resolved edge connects them.

Every place `spec.rs` / `mcp.rs` / `index.rs` needs a "which pack / which root" branch that isn't a clean `for root in roots` loop is a finding — the socket should hold here too, one layer up.

---

## Open, deliberately

- **`serve <repo-root>` with two firing feature models** (a Next frontend + a FastAPI backend) — the primary tiebreak. No repo forces it yet; `.codeowl.toml` is the escape hatch until one does.
- **Whether a secondary ever gets a feature layer.** The model says no. A Next.js frontend as a secondary would test whether that's right or whether "secondary" needs a `full | reduced` dial. Deferred with the follow-up repo.
- **The resolved frontend→backend edge.** Deferred behind M18's cross-service resolver; the shapes match.
- **`packages/` / monorepo workspaces** — is every `package.json` subtree a secondary, or only ones the primary or another secondary actually imports? `full-stack-fastapi-template`'s `packages/react-email/` is the forcing case; lean toward "discovered but excludable via config".
