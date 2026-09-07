# CodeOwl — working conventions

CodeOwl extracts a structural graph from a codebase and serves LLM-authored specs (the semantic layer) over MCP. Design lives in `ARCHITECTURE.md` (how it's built) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos. Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-09-07, part 3 — Phase 2: M13 done + validated on the pilot)

**Goal:** execute **M13** — the `StackPack` trait — as one branch, one PR: put a stack-neutral socket on the core and move the whole TS+Next+SQL+Supabase extractor behind `TypeScriptNextStack`, with the pilot's spec output byte-identical throughout. Then verify it end-to-end on the pilot with a real generation run.

### What was completed — all merged to `origin/master` (`HEAD` = `3424269`)

**M13 — the `StackPack` trait — COMPLETE** (PR #7, 8 commits `3b6e34a`…`d0e4bbd`):
- `3b6e34a` — **`markers: Vec<String>`** on `ExtractedSymbol` / `Symbol` / `SymbolView` (`#[serde(default)]`), the annotation slot design decision 9 called for. Empty for TS; populated from M14. `FORMAT_VERSION` 1 → 2.
- `fcac579` — **`trait StackPack`** (`src/stack.rs`) with `name` / `source_kind` / `classify` / `extract_symbols` / `extract_imports` / `resolve_imports`; `struct TypeScriptNextStack` delegates to the existing free functions (kept `pub` as shims); `lang::detect(root) -> Result<Box<dyn StackPack>>`; `RepoIndex` owns the pack and routes every call through `self.pack.*`. Zero test churn — `build`/`open` signatures unchanged.
- `d440c61` — ROADMAP M13 rewritten in plain English (socket/adapter framing) + a Progress subsection tracking each commit.
- `9e5c427` — drop the redundant `route_literals` parameter threaded through ~8 `spec.rs`/`mcp.rs` signatures; read it off the graph instead.
- `cb774df` — **collapse `graph.rs`'s three typed edge collections** (`route_literals` / `table_refs` / `rendered_components`) into one `flow_edges: Vec<FlowEdge { from_file, kind, raw, target: FlowTarget }>`, produced by `pack.extract_flow_edges` and resolved at build time by `pack.resolve_flow_edge`. `FlowTarget::{Node(SymbolId), Unresolved}`. `resolved_default_imports` stays a separate field (folds in at M18). `FORMAT_VERSION` 2 → 3.
- `90c09ea` — **`utility/structural_sweep.py`** — the reusable "is this refactor inert?" check. One long-lived `codeowl serve` MCP session per binary; diffs `codeowl extract`, `get_spec_coverage`, `get_next_spec_task` (every feature/rollup/system + sample files), `get_callers`/`get_callees`/`get_symbol`; backs up + restores the target repo's `.codeowl/`. ~14s per run. **This is the standard Phase-2-milestone validation tool from here on.**
- `9f9d751` — **the feature layer behind `trait FeatureModel`** (`enumerate_entry_points` + `admits_to_core`), implemented once by `TypeScriptNextFeatureModel` (holds `is_page` / `is_api_route` / the route-path slug / the `is_colocated || does_data_work` rule). `assemble_participants` is now a **generic walk**: a flow edge into a file node joins `core` iff `fm.admits_to_core`; a flow edge into a symbol node (a SQL table) is the `data` tier — no pack string left in the walk. `StackPack::feature_model() -> Option<&dyn FeatureModel>` (default `None`; TS overrides). `EntryPoint { file, slug }` → `{ kind, id, title, file }` with `id` == the old `feature_slug` output, so **spec filenames are unchanged**. `spec.rs`/`mcp.rs` still call `default_feature_model()` / the `enumerate_entry_points` free-fn shim — routing them through `pack.feature_model()` is deferred to M14/M15 (a Rust/Java repo has no `app/**/page.tsx`, so the shim enumerates nothing there anyway). No persisted-shape change → `FORMAT_VERSION` stays 3.
- `d0e4bbd` — **design decision 1 (`SymbolKind`) decided: option (a)**, documented in ROADMAP, implemented in M14. The enum becomes `Container | Callable | Value | Schema` + a pack-owned `raw: String`; `spec.rs`'s granularity rule becomes "generate for `Container`/`Callable`". The enum reshape + TS remap waits for M14 when there's `tree-sitter-rust` output to check the generic set against. Open sub-question left for the owner: how a Java `record` classifies (leaning `Value`), settle in M16.

**Validation — M13 is inert on the pilot, confirmed two ways:**
1. `structural_sweep.py --repo ~/dev/startup/talentTrail` against `origin/master` (the pre-M13-branch base): `codeowl extract`, `get_spec_coverage`, `get_next_spec_task` (103 targets), `get_callers` (25 tables), `get_callees`, `get_symbol` — **all byte-identical**. The `.codeowl/graph` JSON *does* change (new `flow_edges` shape, `format_version`) — expected.
2. **Real generation run** on the pilot with the M13 binary (owner, sessions `codeowl-eval-6`/`-7`): `.codeowl/` rebuilt to `format_version: 3`, `nodes: 1122` (unchanged), `flow_edges: 759` — and **759 = 29 route-literal + 304 table-ref + 426 rendered-component**, the exact totals of the old three collections. 8 feature ladders generated; the 5 written earlier against a *stale pre-M12 binary* reconcile as **current** under M13 (participant hashes match). The 5 stale + 6 smelly specs flagged are all **pre-existing** corpus quality issues, not M13 regressions.

**Stale-binary process fix.** `codeowl-eval-6`'s generation run silently used `target/release/codeowl` (built 06:57, pre-M12 — no `format_version`) because `talentTrail/.mcp.json` pointed there and the dev loop only ever builds `target/debug/`. Fixed: `.mcp.json` → `target/debug/codeowl` (uncommitted in talentTrail; owner folds in), and `cargo build --release` refreshed the release binary so it's not a stale trap. **Convention going forward: the pilot's MCP server uses `target/debug/codeowl`, which `cargo build`/`cargo test` keep current.**

### Where we stopped

**M13 is fully merged and validated. Nothing is mid-flight.** Next work is **M14** — not started, no branch.

### Known issues / blockers

- **M14 implements design decision 1** (`SymbolKind` → `Container|Callable|Value|Schema` + `raw: String`). This reshapes a serialized enum → bump `FORMAT_VERSION` to 4 and remap the TS grammar-kind mapping in the same commit. Java `record` classification is still an open sub-question (settle in M16).
- **`FeatureModel` still has ONE impl** (design decision 8) — M14 (Rust) returns `feature_model() -> None`, so M14 validates every part of `StackPack` *except* `FeatureModel`; that seam gets its second implementation only at M17. Keep `FeatureModel` to its two methods.
- **`spec.rs`/`mcp.rs` are not yet routed through `pack.feature_model()`** — they call `features::default_feature_model()` / the `enumerate_entry_points` free-fn shim, which hardcodes the one stack. Harmless for M14 (a Rust repo enumerates zero Next pages), but M14 should do the threading so `feature_model() -> None` actually takes effect in the generic path, and verify `spec.rs`'s system-spec composition tolerates a zero-feature repo (design decision 3).
- **M17 breaks M10's schema model.** `SourceKind::Schema` dispatches on file extension; Quarkus persistence is a `@Entity` annotation on a Java class (no schema *file*). M18 generalizes to a symbol-level `is_schema_symbol` hook (`markers` makes it cheap) — don't deepen the file assumption before then.
- **Pilot corpus is uncommitted.** `~/dev/startup/talentTrail/docs/specs/` has ~25 newly-generated spec files (8 feature ladders) untracked, plus the `.mcp.json` release→debug change, plus the older "Structural specs (CodeOwl MCP)" `CLAUDE.md` section. Owner folds all of it into their own PR. 61 features / 178 files / 18 rollups / system still pending there.
- Carried from M8, still open: human-edit reconciliation is file-specs-only — feature / rollup / system specs aren't reconciled against manual edits.

### Exact next step to resume

**Start M14 — `RustStack` on CodeOwl's own repo.** Read `ROADMAP.md` → "Phase 2 — the polyglot core first" → the **M14 section** and design decisions 1 + 8. Then:
1. New branch off `master` (`m14-rust-stack` or similar). One PR at the end; Sidd merges via bypass checkbox.
2. **TDD first:** a failing test that `codeowl extract` on this repo's own `src/` produces `fn` / `struct` / `enum` / `trait` / `impl` / `mod` symbols with sane parent/child containment. Then `tree-sitter-rust` + `struct RustStack` to green it.
3. Reshape `SymbolKind` → `Container | Callable | Value | Schema` + `raw: String`; remap the TS mapping in `lang::extract_symbols`; `spec.rs` granularity rule → "generate for `Container`/`Callable`". `FORMAT_VERSION` 3 → 4.
4. Rust import resolution as a **module-tree walk** (not `oxc_resolver` — design decision 5), behind `RustStack::resolve_imports`.
5. `RustStack::feature_model() -> None`; thread `pack.feature_model()` into `spec.rs`/`mcp.rs` (replacing the `default_feature_model()` shim); verify the zero-feature system-spec path.
6. Populate `markers` from Rust decorators (`#[tool]`, `#[derive(...)]`, `#[serde(...)]`).
7. **Validation is a human read of ~10 CodeOwl self-specs**, not a diff — there is no prior self-corpus. Generate them (`/codeowl-generate` against this repo once `.mcp.json` here points at the M14 binary — currently there is no `.mcp.json` in the codeowl repo), read them, judge whether the trait produced something coherent for a non-web stack.

### State

`cargo test`: **144 unit + 10 integration**, all green (`tests/`: extraction 2, feature_components 1, incremental 3, schema 4). `clippy --all-targets -D warnings` + `fmt --check` clean at `9f9d751`; docs-only since. `origin/master` = `HEAD` = `3424269`. Working tree clean. `target/debug/codeowl` and `target/release/codeowl` both current at M13.

### Git workflow

- **`master` is protected.** Every change → branch → PR → Sidd merges via the **"Merge without waiting for requirements (bypass rules)"** checkbox (he's the last pusher, so the 1-code-owner-approval gate can't be met any other way — this is the intended path). Claude opens PRs (`gh pr create`) but **cannot merge** (`gh pr merge` is permission-blocked).
- Commits milestone-scoped (`M14: …`), small, atomic, each compiles + passes its tests. `Authored by: Sidd & Claude Sonnet 5` line in the body, plus the harness `Co-Authored-By:` / `Claude-Session:` trailer.
- `clippy` + `rustfmt` + a `git diff --cached` secret scan before every commit.

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
