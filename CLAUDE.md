# CodeOwl — working conventions

CodeOwl extracts a structural graph from a codebase and serves LLM-authored specs (the semantic layer) over MCP. Design lives in `ARCHITECTURE.md` (how it's built) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos. Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-09-07)

**Goal:** close out Phase 1 (M10 + M11) and write a detailed, reviewed plan for Phase 2 — the polyglot core (`StackPack` trait + modularization), which the owner has prioritized ahead of the web viewer.

### What was completed (all committed + pushed to `origin/master`)

**M10 — SQL/schema boundary resolution.** `1c6c603` (docs) → `0c0b4ac` (prep: grammar-pick dedup + doc-marks) → `fe95f14` (main): `src/schema.rs` parses `CREATE TABLE` from `.sql` via `tree-sitter-sequel` 0.3.11 + a header line-scan backstop → `SymbolKind::Table` nodes; `features.rs::extract_table_refs`/`resolve_table_ref` for Supabase `.from("table")`; `get_callers` on a table lists the files that query it. Pilot: 25/25 tables, 275/304 `.from()` refs resolve (misses are DB views, out of scope). Plus `4970ce8` — `setup/USAGE.md` operator guide.

**M11 codeowl-side prep** (9 commits, `2b11b25`…`2dd2b90` + docs):
- `2b11b25` — **`src/lang.rs`** (the stack-modularization seam): `is_extractable`, `is_schema_file`, `ts_parser` (from the now-deleted `parse.rs`), `RESOLVER_EXTENSIONS`, `extract_symbols` (`.sql`-vs-TS dispatch), `detect(root)` startup fail-fast. Free functions, no trait yet.
- `e0982ac` — **data-touched participants**: `Participants` gains a `data` tier (SQL tables the core `.from()`s); `FeatureTask.data` / `TableContext`; schema-change staleness.
- `9fb0124` — **`--all` ordering**: `get_next_spec_task` accepts `feature:<slug>` / `rollup:<dir>` ids; `prioritize()` reordered to shared-code files → features → long-tail → rollups → system last.
- `245998a` — **`null`-result bug**: exhausted task returned bare `null` (rejected by Claude Code's `structuredContent` check) → now `SpecTaskResponse::Done {}` → `{"kind":"done"}`. Plus `is_test_path` → test files to tier 5, test importers dropped from fan-in.
- `45ddc92` — **rendered-component `core` expansion**: `page.tsx` → `<EvaluationClient/>` → `<EvaluationForm/>` subtree now pulled into `core` when co-located or data-touching; `components/ui/*` stay stubs.
- `dac7194` — **default-import resolution**: #45ddc92 was inert (React components are default-exported, `imports.rs` tracked only named). Added `DefaultImport` + `resolve_default_imports`.
- `2dd2b90` — **tier-0 cap**: `SHARED_CODE_MAX_FILES = 8`, cutoff anchored to the full item set (not `pending`, which refilled forever), `is_ui_primitive` excluded. Pilot shared tier = 8 `lib/` files.

**Phase 1 complete** — `7661141` ("Mark M11 validated by sample; Phase 1 complete"). M1–M10 shipped; **M11 "validated by sample"** — owner reviewed the pilot feature specs and judged them clearly good. Clincher: a spec correctly reported a flow as *dormant* after it was disabled by a one-line flag flip (`b6d8965f` in the pilot). Full corpus generation deliberately stopped at ~15% — the rest is mechanical volume.

### In progress — ONE uncommitted change

**`ROADMAP.md` + `CLAUDE.md` are modified on top of `7661141`, NOT committed** (`ROADMAP.md` staged; `CLAUDE.md` partly staged + this handoff rewrite unstaged — `git add -A` before committing). This is the detailed Phase 2 plan + 8 review fixes folded in. Content is final and reviewed; it just needs the owner's go-ahead to commit (per the workflow rule — only commit/push when a message says "commit" or "push").

Proposed commit message:
> **Plan Phase 2's polyglot core (M12–M15) and fold in the review**
>
> LanguagePack→StackPack, cache versioning as M12's first task, optional feature layer + M13-pre spike, FlowEdge resolution as pack hooks, classify()→FileRole, honest "identical spec output" (not cache) and test-churn budget. Full M1–M11 coupling inventory + a "where the risk concentrates" note on the feature layer.

The Phase 2 plan itself (now in `ROADMAP.md` "Phase 2 — the polyglot core first" — read the whole section before starting):
- **M12** (S) — interim de-coupling. **First job: add a cache `format_version: u32`** to the persisted index/graph; mismatch/absence → full rebuild. This fixes a **latent M11 bug**: a `#[serde(default)]` field reads empty on an old `.codeowl/` cache with no rebuild, yielding wrong data. Then `is_test_path`/`is_ui_primitive` → a `classify(path) -> FileRole` seam (`Domain|Primitive|Test|Generated`); doc-mark `graph.rs`'s 4 pack-contributed fields; `SourceKind` enum for the `.sql` dispatch. No behavior change — pilot regenerates identical spec files.
- **M13-pre** (XS) — paper spike: what would CodeOwl's own feature specs be? Drives making the feature layer *optional* in the trait.
- **M13** (M–L) — `trait StackPack` (renamed from `LanguagePack` — the unit is a *stack*: TS + SQL + Next + Supabase, not one language); move `extract/imports/resolve/schema/features` behind `TypeScriptNextStack`. `graph.rs`'s 4 typed edge fields → one generic `flow_edges` resolved via `pack.resolve_flow_edge`; `assemble_participants` becomes generic traversal + `admits_to_core` hook. `feature_model() -> Option<&dyn FeatureModel>`. ~97 pack call sites = real test churn, absorbed via free-function shims. Spec files + `get_spec_coverage` stay byte-identical to M11; the `.codeowl/graph` JSON is *expected* to change.
- **M14** (L) — `RustStack` on CodeOwl's own repo. The only milestone that actually validates the seams; validation is a human dev cut on ~10 self-specs, not a diff.
- **M15** (M) — fold M14's findings into the trait; commit CodeOwl's self-spec corpus; make `setup/codeowl-generate.md` stack-neutral.

### Known issues / blockers

- **Latent M11 cache bug** (described above) — not yet fixed; it's M12's first task. Anyone regenerating on a pre-M10 `.codeowl/` cache should `rm -rf .codeowl/` first as a workaround.
- **Test churn in M13 is real** (~97 call sites) — the plan budgets for it via shims; don't start M13 expecting it to be free.
- **The feature layer is the risk concentration** for Phase 2 — it's the only part that models a *product*, not code. `core`-admission (`is_colocated || does_data_work`) has no mechanical ground truth (took 3 iterations against *one* repo this session). De-risked via M13-pre + optional `feature_model()` + M14's human read.
- Outside this repo: the pilot's `CLAUDE.md` gained a "Structural specs (CodeOwl MCP)" section and holds a ~45-spec partial corpus — both uncommitted there; the owner folds them into their own PR.
- Carried from M8, still open: human-edit reconciliation is file-specs-only — feature / rollup / system specs aren't reconciled against manual edits.

### Exact next step to resume

1. If the owner says "commit"/"push": `git add -A` (`CLAUDE.md` + `ROADMAP.md`), commit with the message above, push.
2. Then **start M12**: TDD. Failing test first — an old persisted index/graph JSON with no `format_version` (or a wrong one) must trigger a full rebuild, not `#[serde(default)]`-silent partial reuse. Add `format_version: u32` to the persisted `RepoIndex` and `Graph`, bump on read-mismatch. `src/index.rs` + `src/graph.rs`. Keep it the smallest standalone commit; the `classify()`/doc-mark/`SourceKind` parts of M12 are separate commits after it.

### State

`cargo test`: **134 unit + 10 integration**, all green (`tests/`: extraction 2, feature_components 1, incremental 3, schema 4). `clippy -D warnings` / `fmt --check` were clean at the last code commit (`2dd2b90`); the only working-tree change is docs (`ROADMAP.md` + this `CLAUDE.md`). `origin/master` = `HEAD` = `7661141`, with the Phase 2-plan edits uncommitted on top, pending approval.

## Hard invariants

- **CodeOwl never calls an LLM API and never holds LLM credentials.** It assembles context and persists results; the *calling agent* writes spec text via `get_next_spec_task` → write → `submit_spec`. If a task seems to need an LLM call from inside CodeOwl, the design is being violated — stop and flag it.
- **`get_spec` is a pure read.** It never triggers generation. Writes happen only via the `/codeowl generate` command. See ARCHITECTURE.md "Ordering".
- **Generation recurses across containment edges only, never reference edges.** See ARCHITECTURE.md "Recursive spec generation".
- **Invalidation never hashes LLM prose.** Reference edges key on `interfaceHash` (deterministic, off the graph). See ARCHITECTURE.md "Caching and invalidation".
- **The LLM never writes what the graph already knows.** Signatures come from extraction, dependency lists from resolved edges — CodeOwl fills those into a spec itself, and `spec_hash` covers only the LLM-written prose. If a generation prompt asks the agent to restate a signature, that's tokens spent on something we can't afford to have hallucinated. See ARCHITECTURE.md "Spec document format".

## Rust conventions

Written in Rust; see ARCHITECTURE.md "Implementation stack" for why and for the crate list.

- **Graph nodes reference each other by `SymbolId`, never by Rust references or `Rc<RefCell<…>>`.** Nodes live in a flat arena. This is both the idiomatic answer for a cyclic graph and what makes the `.codeowl/` cache cheap to serialize.
- **Don't let tree-sitter's `Node<'a>` escape the parse function.** Its lifetime is tied to its `Tree`, so storing one lifetime-infects everything downstream. Parse → walk → extract into owned structs → drop the tree.
- **Prefer owned `String` over borrowed `&str` in stored structs, and clone freely.** At laptop-repo scale the allocation cost is noise, and it avoids lifetime annotations spreading through the codebase. Reach for interning (`lasso`) only if profiling says string allocation is actually hot.
- **`anyhow::Result` with `.context(…)` for application errors.** Hand-rolled `thiserror` enums are for typed library contracts and are friction here.
- **Keep the core synchronous.** Extraction, resolution, hashing, and spec assembly are plain sync functions. Async appears only at the MCP transport (`rmcp` is tokio-based) and the file watcher.

This is a deliberate Rust learning project. When writing or reviewing Rust here, explain the idiom rather than just landing the fix — the reasoning is the point, not only the compiling code.

## Workflow

- **Commit attribution.** Git's `Author:` is Sidd (already the case). Add a human-readable `Authored by: Sidd & Claude Sonnet 5` line in the commit body — in addition to, never instead of, Claude Code's required `Co-Authored-By:`/`Claude-Session:` trailer, which is a fixed harness convention and stays on every commit regardless.
- **TDD is required for all major changes** — write the failing test first, implement to green. Concretely for this codebase: each `ROADMAP.md` milestone's stated validation *is* that test — write it (as an actual runnable test, not a manual check) before writing the code that satisfies it, not after. Skippable only for genuinely trivial changes (doc fixes, comment tweaks, a rename) — when in doubt, write the test first.
- **Never put secrets in a commit message or diff.** Scan staged content before every commit (`git diff --cached`, grep for key/token patterns), not only when something looks suspicious.
- **Commits are small, atomic, and milestone-scoped.** Reference the `ROADMAP.md` milestone a commit advances where applicable (e.g. `M3: add get_symbol tool`), and never leave a commit that doesn't compile or doesn't pass its tests (once there's code — see TDD above).
- **`.gitignore` from the first crate-scaffolding commit, not an afterthought.** `target/` and `.codeowl/` (the local gitignored cache — see ARCHITECTURE.md "Storage") must never land in git history, not even once.
- **`clippy` + `rustfmt` run before every commit, not just in CI.** Catches idiom mistakes early — matters more here given this is a Rust-learning project.

## Pending decisions

Deliberately unresolved, to revisit when implementation makes them concrete. Design-level open questions live in ARCHITECTURE.md's "Open questions" list (tree-sitter vs. LSP, the token-budget threshold, polyglot/non-code artifacts); these are the scope-trimming ones that don't belong there:

- **Trim the MCP tool surface.** Twelve tools is a lot for Phase 1. A plausible minimum is `get_spec`, `get_next_spec_task`, `submit_spec`, `get_spec_coverage`, and `get_callers`/`get_callees` — with `trace_path`, `get_tests_for`, `get_dependencies` deferred until something actually calls for them. (`search_code` itself is settled — see ARCHITECTURE.md "Storage": Phase 1 is ripgrep, no index — but whether it's even one of the five kept vs. cut entirely from Phase 1 is still open.) Cutting these is cheap now and expensive once they have consumers.
- **Spec-regeneration commit hygiene.** Specs live in git, so a `/codeowl generate` run mid-feature drags spec diffs into an unrelated PR. Explicit generation (rather than silent) already makes this avoidable; the convention that makes it reliable — regenerate as its own commit, or as a deliberate pre-PR step — should be settled once there's a real workflow to test it against.

## Docs

Design decisions go in `ARCHITECTURE.md` / `REQUIREMENTS.md`, not in commit messages or code comments. When resolving an open question, update the open-questions list *and* the section it affects — leaving a resolved question listed as open is the drift this project exists to prevent.
