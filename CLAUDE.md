# CodeOwl — working conventions

CodeOwl extracts a structural graph from a codebase and serves LLM-authored specs (the semantic layer) over MCP. Design lives in `ARCHITECTURE.md` (how it's built) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos; `GLOSSARY.md` defines the static-analysis / graph / CodeOwl-coined vocabulary the rest use (symbol, spec-bearing, flow edge, fan-in, the four hashes, "the socket held", …). Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-09-24 — PR #36 retired after salvaging its content into `setup/USAGE.md` (PR #49, merged); the agent-reliance track planned (PR #50, merged); then milestone numbers stripped from agent-facing surfaces (PR #51) and the whole track resequenced ahead of the polyglot milestone (PR #52). Mostly docs; PR #51 touches `src/mcp.rs`.)

**Goal:** three arcs. Two were documentation; the third began as a question about my own tool use and ended in a real code change.

**(a) Retire `docs-presentation-narrative`/PR #36** — the launch-talk narrative that had been open since 2026-09-15 (the previous entry's carry-forward item 4). Rebased onto master, reviewed for staleness, its Q&A cheat sheet expanded through several rounds of source-grounded research, then — the owner's explicit call — **salvage the Q&A into a permanent home before deleting anything.** He chose `setup/USAGE.md` and required it be done on a **branch spawned fresh from master**, not by merging the narrative branch.

**(b) Plan the agent-reliance track.** Arose from a direct question the owner asked after watching the session: *"even though we have codeowl for this repo, you still grep … honestly tell me what will make codeowl useful to you?"* Answering it honestly turned into a real design question — which lookups genuinely have no CodeOwl answer — and then into two planned milestones.

### What was completed

**PR #49 (`usage-md-graph-faq`) — MERGED** (`32bd613`). `setup/USAGE.md` gained a **"Frequently asked questions"** section, ~14 entries, each one originating from a live technical question the owner asked, answered against the Rust source, then converted to Q&A on request. Covers: graph freshness (event-driven, not timed), `.codeowl/index`'s purpose, batched rebuilds (300ms debounce / dual-persist / `ArcSwap`), spec-vs-graph staleness comparison, hand-edit reconciliation, monorepo scale, `prose_smells`, hallucination-proofing, orphaned specs, one-hop cascade, node shape (`Symbol` vs `File`, as Markdown tables), and the Merkle fold. Working style that should carry forward: **every claim verified against `src/*.rs`, never against `ARCHITECTURE.md`'s sometimes-aspirational prose** — that caught several real errors, including one in my own earlier answers.

**PR #36 / `docs-presentation-narrative` — CLOSED (not merged) and the branch deleted**, only after the salvage above landed and the owner confirmed. Carry-forward item 4 from the previous entry is now resolved. `docs/talks/codeowl-launch-narrative.md` no longer exists in the repo; its content survives in the USAGE.md FAQ plus two Claude Artifacts, which are now the only copy:
- Slide deck (10 slides, dense/light pairs): `https://claude.ai/artifact/YM8PKVrNbB78iuB6mWMNvc`
- "The CodeOwl Graph" visual explainer (hand-authored inline SVG, v4 after three rounds of source-verified corrections): `https://claude.ai/artifact/15bTkfZxuKavwqHy4Vvd8a`

**PR #50 (`agent-reliance-surface-design`) — OPEN, awaiting review.** Commit `55ef997`, 4 files, +336/−2, **all Markdown**, `check.sh` clean. Plans two milestones and reframes one deferral:
- **`experiments/exp-04-agent-reliance.md`** (new) — the design discussion, in the established `exp-NN` spike shape with the same *"Not a decision doc"* authority line as exp-02/exp-03.
- **`ROADMAP.md`** — a new `## The agent-reliance track (M20–M21)` section, placed *after* the Phase-2 `###` subsections so it doesn't re-parent them. (Numbered M21/M22 when first written; **renumbered to M20/M21 later the same day** — see "Resequenced" below.) **M20 (S)**: `get_source` + `search_code` ergonomics — no new deps, no invalidation; `get_source` is mostly `spec::symbol_span_text` + `cap_generation_text` behind a tool, plus a `graph_in_sync` flag (the watcher's 300ms debounce means an edit-then-verify loop can read the current file at a pre-edit span; the check must compare the **file's** `source_hash`, since a container's own is a Merkle fold, not a hash of span text). **M21 (M)**: member-level extraction — fields as `SymbolKind::Value` members, which moves every container's `source_hash` in three of four packs; that invalidation is taken deliberately (`FORMAT_VERSION` bump), not engineered around, since a Rust field reorder is a real layout change.
- **`ARCHITECTURE.md`** — a **"Designed, not yet built"** subsection in §7 (kept *out* of the shipped tool list, which tracks `src/mcp.rs` and must never lead it); a §1 paragraph on the member-extraction asymmetry; **new open question 12** (exported fields and `interface_hash`); §4 Storage search paragraph reframed.
- **`GLOSSARY.md`** — an "Agent reliance" entry.
- The `tantivy` + ONNX deferral **reframed as `search_specs`**: index the hash-checked prose corpus rather than raw source, returning symbol ids that compose into `get_source`/`get_callers`. Gated on measuring after M20/M21; the ~90 MB model file vs. the single-self-contained-binary principle is the real decision, not embedding quality.

**Resequenced, and a prerequisite shipped to make it safe (PR #51, open).** Late in the session the owner reordered the work: the agent-reliance track runs **before** the trait-iteration/polyglot milestone, not after. Execution is now **M19 → M20 → M21 → M22**; old M21/M22 became M20/M21, and the trait milestone (already renumbered M19 → M20 on 2026-09-17) became **M22**. The reasoning reverses a recommendation I made earlier the same day: consolidating the trait *before* a change that touches all four packs' extractors means consolidating, then immediately having fresh divergence — and M21's member-level extraction is the first change since M14 to touch every pack's extractor the same way, so the trait iteration should be informed by it, not precede it.

The renumber was only safe because of **PR #51**, which strips milestone numbers from everything an agent reads. They had been baked into `src/mcp.rs` and the `codeowl-generate` instruction files by the pagination work that shipped under the old M20 — and three of those were `///` doc comments on `JsonSchema`-deriving types, which `schemars` turns into the JSON Schema `description` shipped over the wire, so they were agent-facing despite looking like ordinary source comments. That trap is now recorded as a rule in this file's "Docs" section. PR #51 also fixes `check.sh`, which printed a confident pass for a secret scan that had nothing staged to scan.

**Two factual corrections of record — do not re-assert the old versions.** Both were stated confidently by me earlier in this same session, then checked against source and found wrong:
1. **Field extraction is not missing, it is inconsistent.** `java.rs` has extracted fields as `SymbolKind::Value` (`raw: "field"`/`"constant"`) since M16, including the `int a, b;` multi-declarator case, with a test. `extract.rs` (TS, `method_definition` only), `rust.rs` (`struct_item` → leaf push, body never walked) and `python.rs` (module-level assignments only) extract none. This is what makes `get_symbol`'s `children: []` ambiguous between "no members" and "this pack doesn't look" — the actual defect M21 fixes. It also means open question 12 concerns a rule **already in force** on a real Java corpus.
2. **`search_code` does search `.md` files** — confirmed live (hits in `ROADMAP.md`, `GLOSSARY.md`, `setup/USAGE.md`, `docs/specs/*.md`). But that same call demonstrated the payload bug the trait-iteration milestone's own ROADMAP entry had filed as *"not observed failing yet"*: ~15 KB returned for ~20 matches of `Merkle`, because `MAX_RESULTS` caps match **count** while `SearchMatch.text` carries a whole untruncated line and committed spec prose puts 1–3 KB on one line. Now observed; folded into M20. **Third instance of one failure mode**, described by what shipped rather than by number, since two of the three predate the renumber — blob (God-class, PR #37) → list (`pending` pagination, shipped 2026-09-20) → lines (M20's `search_code` truncation, not yet built). No MCP response in this codebase has a size budget by construction; exp-04 Q3 suggests a test-harness assertion so the fourth is caught by CI, not a user.

### Known issues / carry-forward

1. **PR #50 merged** (`912ce50`), branch deleted. **PR #51** (strip milestone numbers from agent surfaces + the `check.sh` guard) and **PR #52** (the renumber) are open and unmerged — #52 is stacked on #51's branch, so merge #51 first or #52's diff will look larger than it is. Both need the bypass-rules checkbox.
2. **The `deps.py` Python-2 syntax bug** (found back in M17) is still fixed only locally, never contributed upstream to `fastapi/full-stack-fastapi-template` — a real, easy, MIT-licensed first PR, still not picked up.
3. **Gradle-based codegen is entirely unverified.** No Gradle-based Quarkus repo has been dogfooded; don't trust any "(or the Gradle equivalent)" language anywhere in these docs until a real one is checked the same way the Maven case was.
4. **`ARCHITECTURE.md` open questions 9, 10, and 11's two remaining manifestations** (docstring inheritance, unoverridden members — both real, live in `commons-lang`, with a concrete forcing check already identified) are all deliberately deferred, not blocking. **New: open question 12** (exported fields in `interface_hash`) joins them — deliberately undecided, because it changes reference-edge invalidation and its cascade can't be measured until M22 puts fields in the graph.
5. **Attribution line drift.** The "Workflow" section below hardcodes `Authored by: Sidd & Claude Sonnet 5`. This session ran on Sonnet 5 and switched to Opus 5 partway; `55ef997` is signed `Authored by: Sidd & Claude Opus 5`. Read that convention as "name the model that did the work," not as a fixed string — or normalize it here if the owner prefers.
6. **`.claude/` is untracked** and predates this session. Not mine, not a leftover — leave it.
7. **One dangling proposal, never accepted or declined:** a FAQ entry distinguishing `get_callers` from `deps_hash` was drafted for `setup/USAGE.md` but the owner moved to a different question before ruling on it. Not added. Surface it again only if relevant; don't add it unilaterally.

### Exact next step to resume

**First: check PRs #51 and #52** (`gh pr view 51`, `gh pr view 52`). #52 is stacked on #51, so merge #51 first. Delete a branch only once `gh pr view` shows `MERGED`, per the standing memory rule. **#51 touches `src/mcp.rs`** — the first code change in several sessions — so `/mcp` reconnect after building it.

**Then: M20 — `get_source` + `search_code` ergonomics.** The order changed late in the session (see "Resequenced" above): execution is now **M19 → M20 → M21 → M22**, where M20/M21 are the agent-reliance track and **M22 is the trait-iteration/polyglot-core milestone that used to be M20**. Do not read an old "M20" reference as meaning the trait work — the dated `ROADMAP.md` changelog entries before 2026-09-24 still use the old numbering, deliberately, per that file's convention.

M20's own scope, in `ROADMAP.md`'s "The agent-reliance track": `get_source` (mostly `spec::symbol_span_text` + `cap_generation_text` behind a tool, plus a `--max-source-bytes` cap and a `graph_in_sync` flag), and `search_code` gaining `path`/`ignore_case`/`context_lines`/`max_results` plus per-line truncation. That truncation is the urgent bit — it's a live latent payload bug, third instance of a failure mode whose two siblings both caused real failures. S-sized, no new dependencies, no cache invalidation. TDD per `CLAUDE.md`: each test is written from the real lookup that failed, listed in the milestone.

**M22, when it comes:** fold M17/M18/M19 (and now M21's cross-pack extraction findings) back into `trait StackPack`/`trait FeatureModel`, finish M15's doc-neutralization, commit the Java corpora. Two of its bullets already shipped early on 2026-09-20 (`pending` pagination, same-package scan precision) — read the section rather than assuming its whole list is outstanding.

1. **`/mcp` reconnect once PR #51 is built** — it changes `src/mcp.rs` tool descriptions and schema doc comments, which the running binary still has the old copies of. Nothing else this session touched code.
2. The live MCP tools were used successfully throughout this session; the server and graph are healthy.
3. `master` is at `912ce50` (PR #50). Two open branches, stacked: `strip-milestone-refs-from-agent-surfaces` (#51) and `renumber-agent-reliance-first` (#52, based on #51). Neither needs further work.

### Git workflow (evergreen)

- **`master` is protected.** Every change → branch → PR → **Sidd merges** via the "Merge without waiting for requirements (bypass rules)" checkbox (he's the last pusher, so the 1-owner-approval gate can't be met otherwise — the intended path). Claude opens PRs (`gh pr create`) but **cannot merge** (`gh pr merge` is permission-blocked).
- Commits small, atomic, milestone-scoped where applicable (`M17: …`), each compiles + passes tests. Body carries `Authored by: Sidd & Claude Sonnet 5` plus the harness `Co-Authored-By:` / `Claude-Session:` trailers. PR body ends with the `🤖 Generated with Claude Code` line.
- `utility/check.sh` before every commit (fmt + clippy `-D warnings` + `cargo test` + staged-diff secret scan). `utility/release.sh` = same in `--release` + a release build, before a release or after perf/overflow-sensitive changes.
- **Delete a branch only after `gh pr view <n>` shows `MERGED`** (see Process notes).

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
- **Stage first, then run the gate: `git add -A && utility/check.sh && git commit`.** The secret scan reads the *staged* diff, so running the gate before `git add` silently checks an empty index — fmt/clippy/tests still run against the working tree, but the scan is a no-op. It used to print a confident "nothing secret-shaped staged" for that case; it now refuses with a non-zero exit unless you pass `--unstaged`, which runs fmt/clippy/tests and prints `SKIPPED` for the scan. Use `--unstaged` for mid-work checks, never as the pre-commit path.

## Pending decisions

Deliberately unresolved, to revisit when implementation makes them concrete. Design-level open questions live in ARCHITECTURE.md's "Open questions" list (tree-sitter vs. LSP, the token-budget threshold, polyglot/non-code artifacts); these are the scope-trimming ones that don't belong there:

- **MCP tool surface — resolved by inaction, worth noting rather than reopening.** The shipped surface is 8 tools (`get_symbol`, `get_callers`, `get_callees`, `get_spec`, `get_next_spec_task`, `submit_spec`, `search_code`, `get_spec_coverage`) — `trace_path`, `get_tests_for`, `get_dependencies` were sketched but never built, simply because nothing has needed them yet; `impact_analysis` was never built either, but for a different reason — `get_callers` already answers "what breaks if I change this," so there was never a separate tool to build. See `ARCHITECTURE.md` §7 for the full current reference. Revisit only when something real calls for one of the three still-deferred tools.
- **Spec-regeneration commit hygiene.** Specs live in git, so a `/codeowl generate` run mid-feature drags spec diffs into an unrelated PR. Explicit generation (rather than silent) already makes this avoidable; the convention that makes it reliable — regenerate as its own commit, or as a deliberate pre-PR step — should be settled once there's a real workflow to test it against.

## Docs

Design decisions go in `ARCHITECTURE.md` / `REQUIREMENTS.md`, not in commit messages or code comments. When resolving an open question, update the open-questions list *and* the section it affects — leaving a resolved question listed as open is the drift this project exists to prevent.

When a doc or comment coins or leans on a domain term (static-analysis, graph-theory, or CodeOwl-specific), it belongs in `GLOSSARY.md` — keep that file in step as the vocabulary grows.

**Milestone numbers never appear in anything an agent reads.** `M17`, `M20`, "Phase 1" and friends are project bookkeeping — a calling agent can't act on them, they cost tokens in every session, and they silently rot the moment a milestone is renumbered. Keep them out of: `#[tool(description = …)]` strings, the `info.instructions` block, and **any `///` doc comment on a type deriving `JsonSchema`** — `schemars` turns those into the JSON Schema `description` shipped over the wire, so they're agent-facing even though they look like ordinary source comments. Describe the behaviour instead ("`pending` is paginated — 50 entries per call"), not its provenance. They stay welcome in `//` implementation and test comments, in `///` docs on internal types, and throughout `ROADMAP.md` / `ARCHITECTURE.md` / `GLOSSARY.md` / `experiments/` — those are read by humans and future sessions, where the provenance is the point. Same rule covers `setup/codeowl-generate.md` and its `.prompt.md` port: an agent executes those.
