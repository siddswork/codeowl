# CodeOwl — working conventions

CodeOwl extracts a structural + semantic graph from a codebase and serves LLM-generated specs over MCP. Design lives in `ARCHITECTURE.md` (how it's built) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos. Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-09-06)

**Goal:** continue Phase 1 past M5, then — mid-session — a detour into whether the generated-spec "byproduct" has standalone value (brownfield docs) independent of the original agent-context hypothesis, since M11 (the actual exit-criterion test) hasn't run yet.

**Completed, each its own commit, all pushed to `origin/master`:**
1. `afa75dd` — **M6, directory rollups.** Decided + built the `_index.md` shape `ARCHITECTURE.md` had left an open question.
2. `eab7af1` — **M7, staleness & invalidation end-to-end.** Added `deps_hash` (a frontmatter dimension distinct from `source_hash`) so a dependency's *signature* change — not just its own source — correctly stales importers. `get_spec` now reports `missing`/`current`/`stale` + `changed: Vec<String>` across all four document kinds (symbol, file, feature, rollup).
3. `52c21c4` — **M8, system spec + `get_spec_coverage` + human-correction reconciliation.** One root `docs/specs/_index.md` per repo (pseudo-id `"system"`/`"."`); `get_spec_coverage(scope?)` returns a priority-ordered `pending` list (system → features → files by fan-in → rollups) that `--all`/`--budget=N` (a client-side slash-command loop — no new MCP tool) consumes. Human-edit reconciliation (cases 3/4) implemented for **file specs only** — feature/rollup/system specs share the same detection shape but aren't wired yet; flagged in `ARCHITECTURE.md`, not an oversight.
4. `8ab5689` — **Quality-smell detection** (unplanned follow-up, not a ROADMAP milestone). A fork-based white-box audit of talentTrail's real generated specs found two file specs — `docs/specs/app/submit/page.tsx.md`, `docs/specs/app/api/payments/webhook/route.ts.md` — that are template-stub junk (`"<name> does its job."` / `"See source."`) from an early pre-M5 validation pass, still un-regenerated, reporting `stale` only because they predate `deps_hash` — never because anyone flagged the *content* as bad. Built `prose_smells` (cop-out-phrase denylist + word-count floor + the pre-M5 identical-dependency-list signature), wired into `get_spec`/`get_spec_coverage` (`smells`/`smelly` fields) **and** into `next_task`/`next_feature_task`/`next_rollup_task`/`next_system_task` themselves, so a smelly-but-hash-current document gets offered as a task again, not just diagnosed. `status` was deliberately kept hash-pure throughout — a real bug (`get_spec_coverage`'s file-status check initially delegated to the now-smell-aware `next_task`, conflating the two signals) was caught mid-build and fixed by giving file status its own hash-only check. 105 tests passing; live-validated against the pilot repo.

**In progress / open, no code written:**
- Same-turn audit also found `features.rs` (`is_page`/`is_api_route`/the `fetch()`-only route-literal matcher) is hard-coded to Next.js **App Router** conventions specifically — it would find *nothing* (not degraded, zero) on Pages Router, Remix, Express, or even a modern Next.js app built on Server Actions instead of `fetch`. A known, deliberately-scoped Phase-1 choice per `REQUIREMENTS.md` (the pilot is explicitly Next.js App Router), not a bug — but it's the concrete first blocker if a "works on other stacks" brownfield angle ever gets pursued. Pure finding, no follow-up started.
- The brownfield-docs pivot question itself (raised by the user, answered with a qualified "yes, real value, but it pulls the deferred headless-trigger idea and the Phase-2 web viewer forward in priority") was **never decided** — it's a live open question, not a plan committed to.
- **Headless batch-generation mode** — the thing that originally motivated this whole detour — was explicitly deprioritized in favor of building the quality-smell check first ("build the quality-smell check before batch mode"). That's now done. Batch generation itself has not been started.

**Known issues / blockers:**
- Two real spec files in the pilot repo are still junk and un-regenerated (see above) — a ~2-minute `/codeowl-generate` run against the live pilot repo would fix them, not a code change. Left alone deliberately; talentTrail's `docs/specs/`/`.mcp.json`/`.codeowl/` are still untracked there, the user's call to commit, not ours.
- No blockers on codeowl's own repo: `cargo test` (105 passing) / `clippy -D warnings` / `fmt --check` all clean as of `8ab5689`; working tree clean, `origin/master` up to date.
- M9 (incremental indexing/live sessions), M10 (SQL/schema boundary resolution), M11 (the exit-criterion test itself) remain, fully scoped in `ROADMAP.md`, not started.

**Exact next step:** the last explicit instruction ("build the quality-smell check before batch mode") is done and pushed — there is no in-flight code to resume. The next thing is a **decision**, not code: does the user want to (a) resume the plain ROADMAP sequence (M9 next), (b) actually build headless batch generation now that a quality gate exists to protect it, or (c) start on the brownfield-pivot's real blocker (making `features.rs`'s entry-point/route-literal logic pluggable per framework, per the audit's Part 2)? None of the three was chosen before the session ended — ask, don't assume.

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
