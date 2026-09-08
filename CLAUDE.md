# CodeOwl — working conventions

CodeOwl extracts a structural graph from a codebase and serves LLM-authored specs (the semantic layer) over MCP. Design lives in `ARCHITECTURE.md` (how it's built) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos. Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-09-08 — Phase 2: M14 done, self-corpus started, README overhaul)

**Goal:** execute **M14** — `RustStack`, the second `StackPack`, on CodeOwl's own repo — as one branch, one PR. The milestone that actually exercises the trait (M12/M13 only rearranged TS code). Then a human read of a self-corpus sample to confirm the trait produced something coherent for a non-web stack.

### What was completed — all merged (`origin/master` = `HEAD` = `02f05a4`)

**M14 — `RustStack` — COMPLETE** (PR #9, 8 commits `86b591f`…`267da3d`):
- `86b591f` — **`SymbolKind` reshaped** to `Container | Callable | Value | Schema` + a pack-owned `raw: String` on `Symbol` / `ExtractedSymbol` / `SymbolView` (design decision 1). TS remap: function/method → `Callable`, class → `Container`, const → `Value`, table → `Schema`; `raw` carries the old snake_case kind. `spec.rs` granularity rule → `matches!(kind, Callable | Container)` over a file's top-level children (a method is a `Callable` with a `Container` parent, still not spec-bearing). `FORMAT_VERSION` 3 → 4. `structural_sweep.py` folds `raw` back into `kind` → **byte-identical on the pilot**.
- `18f9a49` — **`src/rust.rs`** (986 lines): the `tree-sitter-rust` extractor + `RustStack`. Top-level `fn` / `struct` / `enum` / `union` / `trait` / `impl` / `mod` / `const` / `static` / `type` / `macro_rules!`, plus one level into `impl` / `trait` / inline `mod` bodies. `impl` blocks are their own `Container` symbol with id `<file>::impl Foo` (so it can't collide with `struct Foo`); `impl Debug for S` → `<file>::impl Debug for S`. `#[cfg(test)] mod tests` skipped whole. `///` + `//!` docs (stepping over intervening `#[attr]` and plain `//`). `#[derive(...)]` / attributes → `markers`. `RustStack::classify`: `tests/` / `benches/` → `Test`.
- `19aebee` — **`lang::detect()` branches**: counts each pack's *primary* language (`SourceKind::Code` — `.ts/.tsx` for TS, `.rs` for Rust; `.sql` is `Schema`, so migrations alone don't make a repo "TypeScript"), picks the one that matches, bails on a dual-stack repo (design decision 6). Skips `tests/` / `test/` / `benches/` trees for the detection count (so this repo's `tests/fixtures/*.tsx` don't make it dual-stack). `lang.rs` module doc updated — it's now stack-selection + the TS pack's internals.
- `9d0c699` — **Rust import resolution** (`rust::extract_imports` + `rust::resolve_imports`): parse `use` (plain / grouped / aliased / `pub use`), then map a module path to a file by Rust's *filesystem convention* — `crate::` / `self::` / `super::` against `<src>/a/b.rs` or `.../mod.rs`, from the walked file set, no `mod` parsing and no FS access — then look the item up as a top-level symbol, following one `pub use` hop. `extract_flow_edges` stays empty (a Rust service's reach is plain calls; call analysis deferred like M10). On this repo: 194 imports, 69 `crate::` and **100% resolved**.
- `f3672e1` — **`spec.rs`/`mcp.rs` routed through the pack.** New persisted `Graph::pack_name` (`#[serde(default)]`); `stack::for_name(name) -> Box<dyn StackPack>` recovers the pack for a pure-read caller. `is_test_path(graph, path)` and `prioritize(items, graph)` now go through `pack.classify()` (Rust `tests/` files tier correctly; `TypeScriptNextStack::classify` still delegates to the JS-convention `lang::classify`, so the pilot is unchanged). `features::feature_model_for(graph) -> Option` replaces the `default_feature_model()` stub — `next_feature_task` / `submit_feature` / `current_feature_hashes` / `feature_status` / the mcp feature handlers all guard `None`. `StackPack::feature_model()` returns `Option<&'static dyn FeatureModel>` (a feature model is a stateless ZST). `RepoIndex` gains `pack_name`; `load` discards a cache whose pack changed (design decision 7, finally wired). `FORMAT_VERSION` 4 → 5. New `spec.rs` test drives the zero-feature system-spec composition directly. **`structural_sweep.py` byte-identical on the pilot** (all six categories).
- `e7005c5` — **`.mcp.json` at the repo root** (`./target/debug/codeowl serve .`) — the dogfood config, so a session opened here queries CodeOwl's own graph. `setup/codeowl-generate.md`: the `kind: "system"` instruction gains a clause — when `features` is empty (a CLI/library), add a `## Key flows` section naming the 3–6 principal end-to-end flows and the modules each crosses (exp-02's answer for CodeOwl's own system spec). `setup/README.md`: note the debug-binary choice for people hacking on CodeOwl itself. **The user synced `setup/codeowl-generate.md` → `~/.claude/commands/codeowl-generate.md`** (only diff was this clause).
- `50dcb1d` — **`scoped_symbol_deps` (`pub(crate)`)**, shared by the rendered `### Depends on` section and the `get_next_spec_task` context: resolved intra-repo deps as lines, unresolved imports folded into one sorted `externals: std, anyhow, tree_sitter` line by package name (a relative specifier that resolved to nowhere is kept verbatim — a broken import is real signal). The `identical_dependencies_across_symbols` smell ignores the `externals:` line. **Not a correctness change** (`dependency_hash` already only counted resolved targets), but it *does* change the pilot's `get_spec` / `get_next_spec_task` render — deliberate; specs don't go stale (`spec_hash` covers only LLM prose). This was the one M14 self-corpus finding worth a code fix — Rust code names `Vec`/`Result`/`BTreeMap` constantly and every one was a separate `(external or unresolved)` line burying the signal.
- `267da3d` — wrap-up: `#[cfg(test)]`-gate `build_graph_from_sources` / `extract_and_hash` (+ the now-test-only `hash_text` import in `graph.rs`) — their "`main.rs` uses it too" claim was false (`Command::Extract` goes through `RepoIndex::build → pack.extract_symbols`); every caller is a `#[cfg(test)] mod tests`. `FORMAT_VERSION` history comment updated (added 4, 5). ROADMAP M14 status box + Phase-1 status note + sequence list + changelog.

**Self-corpus validation** (the M14 gate — a dev read, not a diff): ~35 specs generated for `symbol.rs` (5 symbols + file), `imports.rs` (12 + file), `graph.rs` (14 + file) — the three most central modules — via the native `mcp__codeowl__*` tools driving `get_next_spec_task` → write → `submit_spec`. **Extraction correct** (private fns, generics + lifetimes, tuple-structs like `SymbolId(u32)`, `impl` blocks, `#[cfg(test)]` skipped). **Resolution working** (every internal `use crate::` resolved). Prose coherent and well-referenced. **No trait *shape* leaks — the socket held.** Findings: (a) deps-list noise → fixed in `50dcb1d`; (b) `impl`-blocks-as-separate-spec-sections (`## Graph` then `## impl Graph`) → **shipped as-is per the owner's call**, revisit in M15 if the read is poor; (c) stale `build_graph_from_sources` comment → fixed in `267da3d`; (d) `FORMAT_VERSION` history → fixed.

**README overhaul** (PR #10, `5e2b807`): replaced the four-line blurb with the full pitch — the problem (agent re-explores a brownfield repo, hallucinates edges grep can't see), the approach (graph extracted once, prose specs over it via MCP, hash-invalidated), the two rules (never calls an LLM; never hashes prose), a "Why it holds up" list, and a per-stack support table (TypeScript/TSX, SQL schema, Rust) — built in Rust, single binary, one stack per repo auto-detected.

### Where we stopped

**M14 and the README are merged. Nothing is mid-flight in code.** The self-corpus generation is **partially done** — 3 of 16 files (`symbol.rs`, `imports.rs`, `graph.rs`) in `docs/specs/` (untracked). The rest, `rollup:src`, and `system` are **M15's "commit CodeOwl's self-spec corpus"**.

### Known issues / carry-forward

- **Self-corpus is 3/16 files and uncommitted.** `docs/specs/src/{symbol,imports,graph}.rs.md` exist, untracked. The imports.rs / graph.rs specs were rendered **before** commit `50dcb1d`, so their `### Depends on` uses the old per-line `(external or unresolved)` format — **M15 needs a re-render pass** (a fresh `submit_spec` or `get_spec` re-renders the deterministic parts) before committing. Full corpus ≈ **240 symbol specs** (spec.rs alone ≈ 87) + 13 file + `rollup:src` + `system`.
- **`impl`-block spec-section presentation** — an inherent `impl Foo` renders as its own `## impl Foo` section, separate from `## Foo`. Shipped for M14; M15 decides whether to merge inherent impls into the type's section (trait impls stay separate).
- **`FeatureModel` still has ONE impl** (design decision 8) — M14 (`None`) and M16 (`None`) both take the trait default; the second real impl is M17 (Quarkus). Keep it to its two methods.
- **`resolved_default_imports`** is still a separate `Graph` field — folds into `pack.resolve_imports`'s output at M18.
- **M17 breaks M10's schema model.** `SourceKind::Schema` dispatches on file extension; Quarkus persistence is a `@Entity` annotation, no schema *file*. M18 generalizes to a symbol-level `is_schema_symbol` hook (`markers` makes it cheap) — don't deepen the file assumption before then.
- **`utility/structural_sweep.py`** is byte-identical for pure refactors, but M14's `50dcb1d` deliberately changed the render — the sweep's "byte-identical" gate no longer fully applies past M13. It's still the right tool for "did I *accidentally* change spec output."
- **Pilot corpus is uncommitted** in `~/dev/startup/talentTrail/` (owner's own PR): `docs/specs/` + `.mcp.json` release→debug + a `CLAUDE.md` section. Owner also ran `/codeowl-generate` there with the M14 binary — 8 feature ladders, reconciled clean.
- Carried from M8: human-edit reconciliation is file-specs-only.
- **ROADMAP.md is 559 lines** and carries finished Phase-1 milestones at full planning detail. Owner considered trimming (archive Phase 1, shrink shipped status boxes) but **decided no change** — the argued-through rationale is worth the weight.

### Exact next step to resume

**Start M15 — iterate the trait; commit the self-corpus; first doc-neutralization pass.** Read `ROADMAP.md` → M15 section. Then:
1. New branch off `master`. One PR; Sidd merges via bypass checkbox.
2. **Fold M14's findings** — M14 found *no trait shape leaks*, so this is light. The real decision: `impl`-block spec-section presentation (merge inherent `impl` into the type's section in `spec.rs::render`, or keep separate). Small `rust.rs` / `spec.rs` change either way.
3. **Commit CodeOwl's self-spec corpus.** The `.mcp.json` is committed — a session opened in this repo auto-loads the `codeowl` MCP server (`./target/debug/codeowl serve .`; approve once). Run `/codeowl-generate --all` (or `--all --budget=N` in chunks) — it drives `get_next_spec_task` → write → `submit_spec`. Re-render the 3 pre-`50dcb1d` specs first. Then `## Key flows` on the `system` spec: extraction (`lang`→`extract`/`schema`→`imports`→`resolve`→`graph`→`index`), spec generation (`mcp`→`spec`→`graph`→`features`), structural query (`mcp`→`graph`/`search`), incremental reindex (`watch`→`index`→`graph`) — per `experiments/exp-02-feature-layer.md` Part 1. Commit `docs/specs/`.
4. **First doc-neutralization pass:** `ARCHITECTURE.md` "Extractors" / "Feature specs" sections and `setup/codeowl-generate.md` (it still says "a page like `app/submit/page.tsx`" — pack-specific examples move behind "your stack's entry points"). `README.md` was already done this session.
5. **Validation:** the pilot (TS+Next) *and* CodeOwl (Rust) generate current corpora from the same binary, with zero stack-specific branches outside the two `StackPack` impls.

### State

`cargo test`: **165 unit + 10 integration**, all green (`tests/`: extraction 2, feature_components 1, incremental 3, schema 4). `clippy --all-targets -D warnings` + `fmt --check` clean at `267da3d`; docs-only since. `origin/master` = `HEAD` = `02f05a4`. Working tree: `docs/specs/` untracked (3 partial self-corpus files). `.codeowl/` in this repo is warm — pack `rust`, `format_version` 5, ~407 nodes. `target/debug/codeowl` current at M14; `target/release/` was refreshed last session, may lag.

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
- **`utility/check.sh` bundles the whole gate** — `fmt --check` (or `--fix`), `clippy --all-targets -D warnings`, `cargo test`, and the staged-diff secret scan, in order, exiting on the first failure. Run it before every commit instead of the four commands separately. It's **debug-profile only**; `utility/release.sh` runs the same gate in `--release` plus `cargo build --release` (slower — for before a release or after touching perf/overflow-sensitive code, not every commit).

## Pending decisions

Deliberately unresolved, to revisit when implementation makes them concrete. Design-level open questions live in ARCHITECTURE.md's "Open questions" list (tree-sitter vs. LSP, the token-budget threshold, polyglot/non-code artifacts); these are the scope-trimming ones that don't belong there:

- **Trim the MCP tool surface.** Twelve tools is a lot for Phase 1. A plausible minimum is `get_spec`, `get_next_spec_task`, `submit_spec`, `get_spec_coverage`, and `get_callers`/`get_callees` — with `trace_path`, `get_tests_for`, `get_dependencies` deferred until something actually calls for them. (`search_code` itself is settled — see ARCHITECTURE.md "Storage": Phase 1 is ripgrep, no index — but whether it's even one of the five kept vs. cut entirely from Phase 1 is still open.) Cutting these is cheap now and expensive once they have consumers.
- **Spec-regeneration commit hygiene.** Specs live in git, so a `/codeowl generate` run mid-feature drags spec diffs into an unrelated PR. Explicit generation (rather than silent) already makes this avoidable; the convention that makes it reliable — regenerate as its own commit, or as a deliberate pre-PR step — should be settled once there's a real workflow to test it against.

## Docs

Design decisions go in `ARCHITECTURE.md` / `REQUIREMENTS.md`, not in commit messages or code comments. When resolving an open question, update the open-questions list *and* the section it affects — leaving a resolved question listed as open is the drift this project exists to prevent.
