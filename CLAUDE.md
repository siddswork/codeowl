# CodeOwl — working conventions

CodeOwl extracts a structural graph from a codebase and serves LLM-authored specs (the semantic layer) over MCP. Design lives in `ARCHITECTURE.md` (how it's built) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos. Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-09-07, part 2 — Phase 2: M12 + M13-pre)

**Goal:** open-source the repo + protect `master`, then execute **M12** (interim de-coupling) and **M13-pre** (feature-layer spike), and re-plan Phase 2 to add a **Java track** ahead of the deferred items — a JVM service corpus is the owner's highest-value second target.

### What was completed — all merged to `origin/master` (`HEAD` = `a225a38`)

**Open-source + branch protection** (PRs #1, #3):
- MIT `LICENSE` (© 2026 Siddhartha Baidya), `license`/`repository`/`description` in `Cargo.toml`, README License section, `.github/CODEOWNERS` (`* @siddswork`).
- GitHub ruleset **"Protect master"** (id 22437397): PR required, 1 **code-owner** approval, require-last-push-approval, block force-push + deletion, **bypass: repo admin (Sidd) → always**. Net effect: nobody self-merges; Sidd lands PRs via the **"Merge without waiting for requirements (bypass rules)"** checkbox (he's the last pusher so the code-owner gate can't be satisfied otherwise — this is the intended path, not a workaround). **Claude cannot merge PRs** (permission classifier blocks `gh pr merge`).

**Test-repo swap** (PRs #2, #3): `memolink` → **`quarkus-super-heroes`** in `ROADMAP.md`'s test-repo table. Cloned shallow to `~/dev/openSource/test-repos/quarkus-super-heroes` (7-module Maven reactor, ~120 Java files, Panache `@Entity` persistence, zero `.ts`/`.tsx`). `commons-lang` and `leveldb` also present under `~/dev/openSource/test-repos/`.

**M12 — interim de-coupling — COMPLETE** (PR #4, 4 commits `494d62c`…`f2c1241`):
- `494d62c` — **cache `format_version`** (TDD, failing test first). `graph::FORMAT_VERSION = 1`; `#[serde(default)] format_version` on `Graph` + `RepoIndex`, set in their real constructors; `RepoIndex::load` returns `None` on any mismatch (0/absent included) → full `build`; `Graph::load` bails. Fixes the latent M11 `#[serde(default)]`-reads-empty bug.
- `805eec3` — **`lang::classify(path) -> FileRole { Domain | Primitive | Test | Generated }`**. `spec::is_test_path`/`is_ui_primitive` are now one-line delegations; `prioritize`'s tiering is one exhaustive match. `Generated` reserved (no Phase 1 rule produces it; sorted like `Primitive`).
- `1b2c6cf` — **`lang::SourceKind { Code | Schema }`** + `SourceKind::of(path)`. `.sql`-vs-TS dispatch in `extract_symbols` + `FileInputs::extract` is a named match; `is_schema_file` deleted. `graph.rs`'s 4 derived-edge fields (`route_literals`, `table_refs`, `rendered_components`, `resolved_default_imports`) grouped under a `// ---- Pack-contributed derived edges` comment block. Module doc `LanguagePack` → `StackPack`.
- `f2c1241` — **determinism fix** (found during verification): `resolve_imports`/`resolve_default_imports` now iterate `file_imports` sorted by path (`sorted_by_key()` helper); `Graph.by_id` `HashMap` → `BTreeMap`. `.codeowl/graph` is now byte-identical across runs.
- **Verified inert on the pilot** (pre-M12 binary `8078518` vs HEAD): `codeowl extract` symbol JSON byte-identical; `get_spec_coverage` byte-identical incl. the full 274-item `pending` order; same 4 pre-existing stale specs; the `format_version` guard correctly rejects the pilot's real pre-M12 cache and does a clean full rebuild. Pilot `.codeowl/` was restored to pristine pre-M12 after testing.
- **Skipped** ROADMAP M12 bullet 4 (fold `resolved_default_imports` into the import-resolution path) — not clean (`resolve_default_imports` is file-level, different shape); M13 collapses that field into `flow_edges` anyway.

**M13-pre — feature-layer spike — COMPLETE** (PR #5, 3 commits `319d974`…`babc0d4`):
- `319d974` — **Java track added to Phase 2.** Rust-on-self stays **M14** (validates the seams for near-zero cost). New **M16** (`JavaStack` on commons-lang — a third stack, exercises `feature_model() -> None` on a real 627-file corpus), **M17** (Quarkus on quarkus-super-heroes — the feature layer for a Java service), **M18** (fold Java findings, generalize the schema layer off `.sql`, ship). Test-repo table reordered to milestone order; `leveldb` marked not-scheduled.
- `eab50c5` — the spike doc **`experiments/exp-02-feature-layer.md`**. Q1 (CodeOwl's own feature specs): recommend **`feature_model() -> None`** — CodeOwl has workflows but no way to enumerate them mechanically; M14's self-corpus puts the 4 cross-cutting flow narratives (extraction / spec generation / query / reindex) in the **system spec**. Q2 (Java entry points): commons-lang → clean `None`; a Quarkus service → `Some` with **heterogeneous entry-point kinds** (verified against the checkout: `@Path`×9, `@Channel`×6, `@Incoming`/`@Outgoing`, `@RegisterRestClient`×2, Panache `@Entity` classes not `.sql`).
- `babc0d4` — **review pass**. **New M13 design decision 8**: keep `trait FeatureModel` minimal — M14 + M16 both return `None`, so it has ONE impl until M17; don't let it accrete TS+Next assumptions. **New M13 design decision 9**: `ExtractedSymbol` needs a pack-owned **`markers: Vec<String>`** — the entire Java feature/schema model keys on annotations (`@Path`, `@Entity`, `@ApplicationScoped`, `@RegisterRestClient`) and there is nowhere to record them; add it in M13 while the symbol shape is already open (cheap now, expensive at M17). Plus 5 factual fixes (`ui-super-heroes` is a Maven module / one repo = one graph / `EntryPoint.file` not `SymbolId` / CDI admission is type-reference not dataflow / `exp-01` isn't in git).

### Where we stopped

**M12 and M13-pre are both fully merged. Nothing is mid-flight.** Next work is **M13** — not started. No M13 branch exists.

### Known issues / blockers

- **`ExtractedSymbol.markers` MUST land in M13** (design decision 9) while the symbol shape is open — reopening `ExtractedSymbol` at M17 ripples through extraction, hashing, and the cache format.
- **`FeatureModel` has no second implementation until M17** (design decision 8) — M14 and M16 both `None`. M13 keeps it to its two methods; anything M17 needs is an M18 addition. If this gap feels too risky, the alternative is pulling M17's Quarkus feature work ahead of M16.
- **Test churn in M13 is real** (~97 pack-function call sites across `src/` + `tests/`) — plan budgets free-function shims (`extract::extract_file` → `default_ts_stack().extract_symbols(...)`) so existing tests compile unchanged, then a dedicated churn commit. Don't start M13 expecting it free.
- **M17 breaks M10's schema model.** `SourceKind::Schema` dispatches on file extension; Quarkus persistence is a `@Entity` annotation on a Java class (no schema *file*). M13 shouldn't deepen the file assumption; M18 generalizes to a symbol-level `is_schema_symbol` hook (which `markers` makes cheap).
- Pilot repo (`~/dev/startup/talentTrail`): its `.codeowl/` is a **pre-M12 cache** (no `format_version`) — the first `serve`/`extract` with a post-M12 binary will full-rebuild it once (correct behavior). Its `CLAUDE.md` "Structural specs" section + ~45-spec partial corpus are still uncommitted there; owner folds into their own PR.
- Carried from M8, still open: human-edit reconciliation is file-specs-only — feature / rollup / system specs aren't reconciled against manual edits.

### Exact next step to resume

**Start M13.** Read `ROADMAP.md` → "Phase 2 — the polyglot core first" → the **M13 section** *and* "Design decisions to resolve during M13" (now **9** items; 8 and 9 are new this session and are load-bearing). Then:
1. New branch off `master` (`m13-stackpack-trait` or similar). Every change lands via PR — Sidd merges via bypass checkbox.
2. Define `trait StackPack` + `struct TypeScriptNextStack` wrapping today's `extract`/`imports`/`resolve`/`schema`/`features`. Keep thin free-function shims so the ~97 call sites + tests compile unchanged.
3. `graph.rs`'s 4 derived-edge fields → one generic `flow_edges: Vec<FlowEdge { from_file, target: FlowTarget, kind: String }>`, resolved at build time via `pack.resolve_flow_edge`.
4. Add `ExtractedSymbol.markers: Vec<String>` (decision 9). `EntryPoint { kind: String, id, title, file: String }` — **not `SymbolId`** (decision 9 / spike). `feature_model() -> Option<&dyn FeatureModel>`, kept minimal (decision 8).
5. **Validation** (M13's stated test — write it as a runnable check): the pilot's *spec files* and `get_spec_coverage` output are byte-identical to `origin/master`; the `.codeowl/graph` JSON is *expected* to change (new `flow_edges` shape). Reuse the M12 verification approach: build the pre-M13 binary from `master`, diff `get_spec_coverage` old vs new against the pilot (there is a driver at `scratchpad/mcp_call.py` from this session's verification — an MCP stdio client for one `tools/call`).

### State

`cargo test`: **139 unit + 10 integration**, all green (`tests/`: extraction 2, feature_components 1, incremental 3, schema 4). `clippy --all-targets -D warnings` + `fmt --check` clean at `f2c1241`; docs-only since. `origin/master` = `HEAD` = `a225a38`. Working tree clean apart from this handoff.

### Git workflow this session established

- **`master` is protected.** Every change → branch → PR → Sidd merges via the bypass checkbox. Claude opens PRs (`gh pr create`) but cannot merge.
- Commit trailer drifted mid-session as the model was switched (Fable / Opus / Sonnet) — follow the current `<system-reminder>` attribution block, whatever it says now.
- Keep commits milestone-scoped (`M13: …`), small, atomic; `Authored by: Sidd & Claude <model>` line in the body per the Workflow section below.

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
