# CodeOwl — working conventions

CodeOwl extracts a structural graph from a codebase and serves LLM-authored specs (the semantic layer) over MCP. Design lives in `ARCHITECTURE.md` (how it's built) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos. Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-09-07)

**M10 done + M11 started (codeowl-side prep).**

M10 — SQL/schema boundary resolution — done, three commits: `1c6c603` (docs, milestone-plan fold) → `0c0b4ac` (prep: `src/parse.rs` grammar-pick dedup + TS+Next doc-marks) → `fe95f14` (main: `src/schema.rs` parses `CREATE TABLE` from `.sql` via `tree-sitter-sequel` + a header line-scan backstop → `SymbolKind::Table` nodes; `features.rs` `extract_table_refs`/`resolve_table_ref` for `.from("table")`; `get_callers` on a table lists the files that query it; instructions string notes table nodes). Pilot: 25/25 tables, 275/304 `.from()` refs resolve (misses are DB views, out of scope). Also `4970ce8` — `setup/USAGE.md` operator guide.

**M11 codeowl-side prep — done this session:**
1. `2b11b25` — **`src/lang.rs`**, the stack-modularization item. Centralizes everything TS/SQL-specific that was spread across modules: `is_extractable`, `is_schema_file`, `ts_parser` (moved from the now-deleted `parse.rs`), `RESOLVER_EXTENSIONS` (was inline in `resolve.rs`), `extract_symbols` (the `.sql`-vs-TS dispatch), and `detect(root)` — startup fail-fast (`main.rs`) if a repo has no `.ts/.tsx/.sql`. Free functions, no trait (Phase 2).
2. `e0982ac` — **data-touched participant wiring** (deferred from M10): `Participants` gains a `data` tier — SQL tables the core code `.from()`s. Hashed by `source_hash`, passed to the feature task as `FeatureTask.data` / `TableContext { id, columns }`. Feature "Data touched" now has graph backing + schema-change staleness.
3. `bda59df` (docs), `121cf25` (USAGE "How generation works").
4. `9fb0124` — **`--all` ordering fix**: `get_next_spec_task` accepts `feature:<slug>` / `rollup:<dir>` targets (the ids `get_spec_coverage` emits — previously silently `null`). `prioritize()` reordered to **high-fan-in files → features → long-tail files → rollups → system last** (`SHARED_CODE_FAN_IN = 3`) — was "system first", which defeated fan-in ordering. Skill batch mode simplified.
5. `245998a` — **`null`-result bug + test-code deprioritization**: `get_next_spec_task` returned a bare `null` when exhausted, which Claude Code's client rejects as an invalid `structuredContent`. Now returns `SpecTaskResponse::Done {}` → `{"kind":"done"}`; skill checks `kind == "done"`. Separately, `spec.rs::is_test_path` (`e2e/`, `__tests__/`, `*.test.*`, `*.spec.*`, `cypress/`, `playwright/`) — test files → tier 5 (below rollups, before system), test dirs excluded from `enumerate_modules`, `file_fan_in` drops test importers. Found in a pilot run where `e2e/helpers/api-client.ts` (fan-in 71, all from tests) outranked every real `lib/` file.
6. `45ddc92` — **rendered-component `core` expansion**: a feature's `core` was entry file + `fetch()`-reached routes only, missing the client-component subtree (`page.tsx` → `<EvaluationClient/>` → `<EvaluationForm/>` where the real flow lives). `features.rs::extract_rendered_components` collects uppercase JSX tags per file; `assemble_participants` resolves each `<Tag/>` to its file and pulls it into `core` if co-located with the entry OR does data work. `components/ui/*` primitives stay stubs. Once in `core`, a component's own fetches are followed.
7. `dac7194` — **default-import resolution**: #6 was inert on the pilot — `imports.rs` tracked only *named* imports, React components are default-exported. Now `imports.rs::DefaultImport` + `resolve.rs::resolve_default_imports` (specifier → target file) on `Graph`; `resolve_rendered_component` checks those first. Validated: the evaluate page's `EvaluationFormClient` now joins `core`.
8. `2dd2b90` — **tier-0 (shared-code) cap**: `SHARED_CODE_FAN_IN = 3` made the shared tier ~25 `lib/` files, so `--all` reached zero features. `prioritize()` now bounds it to the top `SHARED_CODE_MAX_FILES = 8` by fan-in, cutoff computed from the *full `items`* (not `pending`, which kept refilling the tier forever), and `components/ui/` primitives excluded via `is_ui_primitive`. Pilot shared tier = 8 real `lib/` files, cutoff 12.
9. `ba3b69b` (docs) — the "model coupling" leak documented in `ROADMAP.md` (see below).

**Phase 1 is complete (2026-09-07).** M1–M10 shipped; **M11 marked "validated by sample"** — the project owner reviewed the pilot's feature specs and judged them clearly good (clincher: a spec correctly reporting a feature as dormant after a one-line flag flip, `b6d8965f` in the pilot). Full corpus generation stopped at ~15% coverage on purpose; the rest is mechanical volume. The pilot's `CLAUDE.md` gained a "Structural specs (CodeOwl MCP)" section (uncommitted in that repo — the user folds it into their partial-corpus PR).

**Next: Phase 2**, led by the `LanguagePack` trait + a second language (CodeOwl's own Rust, likely). Before that, an **interim de-coupling step** is on the table (ROADMAP "Stack modularization"): every milestone since M10 leaked TS+Next *model* coupling into the supposedly-generic core — `graph.rs` carries 4 pack-specific edge collections, `spec.rs::prioritize` has `is_test_path`/`is_ui_primitive`, the feature concept is routing-shaped. The interim step moves those behind a named seam + doc-marks the fields (no behavior change), making the Phase 2 extraction a rename.

## Prior Session (2026-09-06, cont.)

**Completed this session, each its own commit, all pushed to `origin/master`:**
1. `f865e81` — **M9, incremental indexing + in-session file watcher.** New `RepoIndex` (`src/index.rs`) caches per-file inputs at `.codeowl/index`; `RepoIndex::open` is the fresh-spawn catch-up, `apply_changes` the watcher's incremental update. `src/watch.rs` is the `notify` watcher (300ms debounce, per-directory watches over the gitignore-visible tree). MCP server graph went `Arc<Graph>` → `Arc<ArcSwap<Graph>>`; handlers snapshot once up front. 112 unit + 5 integration tests.
2. `cb24dd1` — ROADMAP note: M9 live-validated against the pilot repo (~0.5s edit-to-visible, catch-up on respawn).
3. `62ee71e` — replaced the MCP server's stale `get_info` instructions string (was M3-era, claimed generation didn't exist).
4. `aefdc42` — **`setup/` folder**: the "wire CodeOwl into your repo" guide + example `.mcp.json`. The `/codeowl-generate` command moved here from `.claude/commands/` (does nothing in a Rust repo). Instructions string expanded (M9, smells, feature concept, points at `/codeowl-generate`).
5. `ccda54c` — M9 note in `setup/codeowl-generate.md`; synced the user's global `~/.claude/commands/` copy.
6. **Phase 1 reframe** (docs-only commit): the original exit criterion (does spec generation cut agent token use — a go/no-go gate) is retired. Value proposition is now taken as the premise: dual-audience (human + LLM) brownfield documentation. **M11 changed** from "run tasks, reach a verdict" to "generate a real spec corpus over the pilot + hold it to a dual-audience quality bar" (BA cut / dev cut / smell cut / optional `mine.py` data point). **M10's rationale shifted** ("lets the corpus describe the DB") and it's now load-bearing, not optional. Phase 2 reprioritized: web viewer + headless generation moved to the front. `REQUIREMENTS.md`, `ROADMAP.md`, `ARCHITECTURE.md` all updated.

**Also, outside this repo:** the pilot repo's two junk specs (`app/submit/page.tsx.md`, `app/api/payments/webhook/route.ts.md`) were regenerated by driving the MCP loop manually over stdio (not via the skill — this session had no codeowl MCP connection). User committed them there as `6f9bae22`. The pilot repo still has only ~9 spec files total — building the rest is M11's job.

**Open / not decided:**
- Feature/rollup/system human-edit reconciliation is still file-specs-only (from M8).
- Phase 2 promotes `src/lang.rs` (free functions) to a `LanguagePack` trait once a genuinely different language exists to check the seams against.

**Exact next step:** M11 corpus generation — user drives `/codeowl-generate --all` from a session inside the pilot repo, then the four quality cuts. The codeowl-side M11 prep (`lang.rs`, data-touched wiring) is done. See `ROADMAP.md`'s M11 "Execution / sequencing".

**State:** `cargo test` 123 unit + 9 integration, `clippy -D warnings` / `fmt --check` clean; release binary rebuilt (M9 + M10 + M11 prep); `tree-sitter-sequel` 0.3.11 added (forced `cc` 1.4→1.2 in the lockfile). `origin/master` at `4970ce8` + the two uncommitted M11-prep commits pending approval.

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
