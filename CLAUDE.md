# CodeOwl — working conventions

CodeOwl extracts a structural graph from a codebase and serves LLM-authored specs (the semantic layer) over MCP. Design lives in `ARCHITECTURE.md` (how it's built) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos. Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-09-08 — Phase 2: M15 shipped — trait iterated, docs neutralized, MCP fixes)

**Goal:** execute **M15** — the TS+Rust two-stack checkpoint: fold M14's one finding into the trait, first doc-neutralization pass, commit CodeOwl's self-spec corpus. One branch, one PR. **Mid-session the owner scoped the corpus down** to the 3-file re-render + defer the exhaustive corpus (it's big — see below).

### What was completed — all merged (PR #12, `HEAD` = `master` = `e53f837`, 14 commits `bf9d8d0`…`3049f75`)

**Trait iteration — M14's one finding (a Rust type as two spec sections `## Graph` + `## impl Graph`):**
- `bf9d8d0` — **`rust::merge_inherent_impls`** folds an inherent `impl Foo { … }` block into the `Foo` symbol *after* the tree-sitter walk: methods reparented (`<file>::impl Foo::m` → `<file>::Foo::m`), block `source_hash` Merkle-folded into `Foo`'s, attribute `markers` unioned, block dropped. A type's several inherent impls collapse in source order. **Trait impls (`impl Trait for Foo` — ` for ` in the header) stay their own symbols.** Done in the pack, not `spec.rs` — the renderer stays stack-neutral and sees a Rust type exactly like a TS class. **Zero `spec.rs` change; the socket held.** Incidental fix: two `impl CodeOwlServer` blocks in `mcp.rs` were colliding on one arena id (`src/mcp.rs::impl CodeOwlServer`) — now distinct `CodeOwlServer::<method>` ids. `FORMAT_VERSION` 5 → 6. `rust.rs` helpers: `inherent_impl_target`, `strip_leading_generics`, `short_id`.
- `fb915d4` — **`spec::symbol_span_text(root, graph, rel_path, sym)`** replaces `read_symbol_text`/`read_lines`: a symbol's own line span **plus any direct child span that falls outside it**, adjacent spans merged. For a TS class the methods are already inside the class braces → identical output; for a folded Rust type the method bodies live elsewhere and are appended. Fixes two blind spots the fold created: `spec::dependency_lines`/`dependency_hash` and `get_next_spec_task`'s symbol `source` couldn't see a folded method's body (a dep used only in `Graph::save` wasn't listed). **`SpecTask::Symbol` loses its now-unused `lines` field.**

**Doc-neutralization, first pass** (`M18` finishes it):
- `d013183` — `ARCHITECTURE.md` §1/§2 "per language" → "per stack" + a `StackPack` paragraph (one-stack-per-repo, `Container|Callable|Value|Schema` + `raw`); "Feature specs / Making them derivable" now leads with "these are a pack's *optional* hooks — a CLI/library has none" and names each generic seam (`extract_flow_edges`/`resolve_flow_edge`, `admits_to_core`). `setup/codeowl-generate.md` drops the hard-coded `app/submit/page.tsx`.
- `d398d13` — `setup/USAGE.md` + `setup/README.md`: "TypeScript / TSX only" was M13-era. Now: one `StackPack` per repo auto-detected, dual-stack repo rejected, CLI/library stack → no feature layer / `## Key flows`. USAGE gains **"How the structural tools actually behave"** — `get_callers` is an import/`use` edge **not** a call graph (query the type/free-fn, **not** a method — a method comes back empty); `get_callees` is file-level; `search_code` is plain regex. Also: driving the tools yourself in chat is a normal workflow.

**Self-spec corpus — SCOPED DOWN by the owner:**
- `927de80` — `symbol.rs` + `graph.rs` self-specs re-rendered for the folded shape (`## impl SymbolId`/`## impl SymbolView`/`## impl Node`/`## impl Graph` folded into their types; `Graph`'s prose rewritten to cover its ~23 methods; deps now include `Symbol`/`SymbolId`/`anyhow` from method bodies). `imports.rs` was unaffected (no inherent impls) and the "pre-`50dcb1d` re-render" worry turned out moot (they were already in the new deps format). **The exhaustive ~239-generation corpus is DEFERRED** — see next step.

**MCP fixes surfaced while dogfooding:**
- `7f6bea3` — **`get_spec_coverage` → `generations_remaining`** (top-level) + per-`pending` `generations`: the real `--budget=N` for a complete run, counting uncovered symbols not documents. `spec::file_pending_generations` is a read-only mirror of `next_task`'s per-symbol decision (never-generated / source-moved / smelly → a cycle; source-unchanged + human-edited → silent reconcile, not counted). CodeOwl's own remaining corpus = **239** (`spec.rs` = 88, `features.rs` = 36, `rust.rs` = 28, `mcp.rs` = 21, rest ≤12).
- `70b0a32` — `structural_sweep.py` `normalize_coverage` folds the additive `generations` fields away before its byte-diff, like `markers`/`raw`.
- `896ce4a` — **`get_callers` / `get_callees` / `search_code` returned a top-level JSON array** as `structuredContent`, which MCP forbids (must be an object) — a spec-compliant client (Claude Code) rejected them with a schema error; **broken from any real client since M3**. Now `{ "callers": [...] }` / `{ "callees": [...] }` / `{ "matches": [...] }` (`CallersResponse`/`CalleesResponse`/`SearchResponse`). Element shape untouched. Sweep gets `as_list()` to unwrap either form.

**Tooling:**
- `47f652e` — `tests/rust_spec.rs`: a Rust file through `Graph::build` → the `next_task`/`submit`/`render` loop, asserting the folded section structure (one `## Counter`, no `## impl Counter`, trait impl keeps its section). The end-to-end version of `rust.rs`'s unit tests.
- `7f487ca` — **`utility/check.sh`** — the pre-commit gate in one command: `fmt --check` (or `--fix`) → `clippy --all-targets -D warnings` → `cargo test` → staged-diff secret scan, exit on first failure.
- `3049f75` — **`utility/release.sh`** — same gate in `--release` + `cargo build --release`. Slow; for before a release / after perf- or overflow-sensitive changes. Refreshed `target/release/codeowl` (was stale).
- `211bd8a`, `7d77b3a` — ROADMAP M15 status boxes + sequence list + changelog.

### Validation done

- **`structural_sweep.py` byte-identical on the pilot** (`~/dev/startup/talentTrail`, TS+Next), M15 vs `master` — all six categories. The additive `generations` fields and the callers/callees envelope normalized away.
- **Dual-binary generation on the pilot** — full `get_next_spec_task → submit_spec → render` loop for 3 *missing* files (`lib/date-utils.ts`, `lib/grade-utils.ts`, `lib/view-as-judge.ts`), `master` binary vs M15 binary, byte-identical `.md` output. Pilot working tree left pristine (only its pre-existing `.mcp.json` mod + 13 untracked owner specs).
- `tests/rust_spec.rs` + the 6 new unit tests (`rust::inherent_impl_folds_into_its_type`, `…several_inherent_impls…`, `…folded_type_source_hash_moves…`, `…trait_impl_is_left_as_its_own_symbol`, `…inherent_impl_target_parses_headers`, `spec::symbol_span_text_includes_a_folded_impl_s_method_bodies`, `spec::coverage_counts_generations_not_just_documents`).

### Known issues / carry-forward

- **The exhaustive self-spec corpus is the deferred follow-up — 239 generations.** `get_spec_coverage` (no scope, run in this repo) reports it live: `src/spec.rs` = 88, `src/features.rs` = 36, `src/rust.rs` = 28, `src/mcp.rs` = 21, `src/extract.rs` = 12, `src/lang.rs`/`src/schema.rs` = 10, `src/resolve.rs` = 9, `src/stack.rs` = 8, `src/index.rs`/`src/watch.rs` = 5, `src/search.rs` = 3, `src/hash.rs` = 2, `rollup:src` = 1, `system` = 1 (the `## Key flows` one — extraction / spec-generation / structural-query / incremental-reindex flows per `experiments/exp-02-feature-layer.md` Part 1). `src/main.rs` is not spec-bearing (no `pub` items). Run `/codeowl-generate --all --budget=40` in chunks, commit `docs/specs/` after each; `spec.rs` deserves its own chunk or two. `--all` order is fan-in first, so `lang`/`features`/`hash`/`resolve` generate before the long tail.
- **`FeatureModel` still has ONE impl** (design decision 8) — M15 (`None`) and M16 (`None`) both take the trait default; the second real impl is M17 (Quarkus). Keep it to its two methods.
- **`resolved_default_imports`** is still a separate `Graph` field — folds into `pack.resolve_imports`'s output at M18.
- **M17 breaks M10's schema model.** `SourceKind::Schema` dispatches on file extension; Quarkus persistence is a `@Entity` annotation, no schema *file*. M18 generalizes to a symbol-level `is_schema_symbol` hook (`markers` makes it cheap) — don't deepen the file assumption before then.
- **`utility/structural_sweep.py`'s "byte-identical" gate** now normalizes away `markers`/`raw` (M14), the `generations` coverage fields, and the callers/callees/search envelope (M15). Still the right tool for "did I *accidentally* change spec output for TS" — but the list of deliberate, normalized-away changes grows each milestone.
- **Pilot corpus is uncommitted** in `~/dev/startup/talentTrail/` (owner's own PR): `docs/specs/` + `.mcp.json` release→debug + a `CLAUDE.md` section.
- Carried from M8: human-edit reconciliation is file-specs-only.
- **ROADMAP.md is ~570 lines** and carries finished Phase-1 milestones at full detail. Owner **decided no change** — the argued-through rationale is worth the weight.

### Exact next step to resume

**Start M16 — `JavaStack` on commons-lang.** Read `ROADMAP.md` → M16 section (`### M16 — JavaStack on commons-lang`). A third stack the TS/Rust impls didn't shape, and the first real corpus exercising `feature_model() -> None` end to end. Highlights: `tree-sitter-java`; extract `class`/`interface`/`enum`/`record`/`@interface` + methods + nested types + fields (map onto `Container|Callable|Value` — records and annotation types are the awkward cases); **package→file resolution keyed on the `src/main/java` layout convention, not `pom.xml`** (so Gradle works free), the hard part being star imports and unqualified-name-against-enclosing-package + `java.lang`; Javadoc `/** */` as the docstring; `JavaStack::classify` returns `Test` for `src/test/java`; `feature_model()` returns `None` (verify `spec.rs`'s system-spec composition tolerates zero features — already true, but re-check on a real Java corpus); `extract_flow_edges` empty. Test repo: **commons-lang** (627 files, single-module Maven — see ROADMAP's test-repo list). Validation *is* the test (TDD): `codeowl extract` + `serve` on commons-lang, generate a partial corpus (shared-code tier + a sample of module rollups + the system spec), a human read of ~8–10 specs. Every place the trait still assumes a resolver config, a route-shaped feature, or a `.sql` schema file is an M18 finding.

**Also pending (not blocking M16):** the deferred CodeOwl self-corpus (239 gens, above) — can be done anytime as its own PR; and the owner needs to re-sync `setup/codeowl-generate.md` → `~/.claude/commands/codeowl-generate.md` (entry-point wording changed).

### State

`cargo test`: **170 unit + 10 integration** (180 total), all green (`tests/`: extraction 2, feature_components 1, incremental 3, **rust_spec 1**, schema 4). `utility/check.sh` clean on `master` (`e53f837`). `.codeowl/` in this repo: pack `rust`, `format_version` **6**, ~410 nodes, warm. `target/debug/codeowl` and `target/release/codeowl` both current at M15. The session's `codeowl` MCP server was last reconnected after `896ce4a` and is current (everything since was docs/scripts).

**MCP dogfood note:** the `codeowl` MCP server in a session keeps the binary it was spawned with — after any `cargo build` that could change the graph, **ask the owner to reconnect** (`/mcp` → reconnect `codeowl`) before trusting `mcp__codeowl__*` results; verify with `get_symbol` on something the change should have moved. (Saved as a memory: `reconnect-mcp-after-code-change`.)

### Git workflow

- **`master` is protected.** Every change → branch → PR → Sidd merges via the **"Merge without waiting for requirements (bypass rules)"** checkbox (he's the last pusher, so the 1-code-owner-approval gate can't be met any other way — this is the intended path). Claude opens PRs (`gh pr create`) but **cannot merge** (`gh pr merge` is permission-blocked).
- Commits milestone-scoped (`M16: …`), small, atomic, each compiles + passes its tests. `Authored by: Sidd & Claude Sonnet 5` line in the body, plus the harness `Co-Authored-By:` / `Claude-Session:` trailer.
- `utility/check.sh` before every commit (`clippy` + `rustfmt` + `cargo test` + staged-diff secret scan).

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
