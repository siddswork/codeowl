# CodeOwl — working conventions

CodeOwl extracts a structural graph from a codebase and serves LLM-authored specs (the semantic layer) over MCP. Design lives in `ARCHITECTURE.md` (how it's built), `DECISIONS.md` (why: the open-question reasoning trail and its index lives in ARCHITECTURE.md) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos; `NORTHSTAR.md` has the competitor matrix and the KPIs (K1–K10) that work should move, with an append-only measurement log; `GLOSSARY.md` defines the static-analysis / graph / CodeOwl-coined vocabulary the rest use (symbol, spec-bearing, flow edge, fan-in, the four hashes, "the socket held", …). Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-10-02 to 2026-10-04 — took the spec corpus to 100 % current, then followed "a comment-only edit stales hub file summaries" into a seven-round experiment on what a file summary should be written from and what should make it stale, then built the first step of the resulting plan, the `explain_stale` tool. The experiment is merged (PR #88). The tool is on branch `explain-stale`, five commits, with its PR opened by Claude for Sidd to merge. `FORMAT_VERSION` is 20.)

**Goal:** Sidd drives toward accuracy guided by `NORTHSTAR.md`. This stretch: finish K5 (100 % spec coverage), fix the doc contradictions found while writing specs, then "the finer file-level fingerprint, so a comment-only edit stops staling hub file summaries". That exposed that a file summary is written from only the first 8,000 bytes of the file (23 of 35 `src` files are over), so the input had to be decided before the fingerprint. Everything below the first table is the consequence of that one question.

### What was completed
**Merged since the previous handoff (every PR merged by Sidd):**

| PR | What |
|---|---|
| #82 | Docs: human readers are the audience, agents a bonus with no claims. |
| #83 | `NORTHSTAR.md`: competitor matrix, KPIs K1 to K10, append-only measurement log. |
| #84, #85 | **Question 21 (TypeScript):** `interface`/`type`/`enum` and destructured-export symbols, then export clauses. `talentTrail`: unresolved imports naming a walked file 141 of 858 to 0 of 717; coarse-hash targets 131 to 0. |
| #86 | K1 split into K1a (dependency tracking, deterministic) and K1b (prose truth, judged); **spec corpus regenerated to 37/37 current** (359 generation cycles, about 540k tokens). |
| #87 | 7 doc comments and tool descriptions that contradicted the code fixed, the specs they staled regenerated. A comment-only commit had taken the corpus from 100 % to 40.2 % current. |

**PR #88 (merged):** `experiments/exp-05-file-summary-input.md` (seven rounds, conclusions, limits, cost, plan, the Rust and Java key points), `DECISIONS.md` open question 22, and its `ARCHITECTURE.md` index row. About 5.4M tokens of experiments across 110 sub-agents; the history replay in Round 7 cost none.

**What the experiments established** (details and tables in exp-05):
- Today's capped input is **blind past the cut** on every stack (perfect before it, zero after).
- **No single replacement input fits every file:** raw when it fits; paged raw up to about 12 pages; symbol summaries (also paged) beyond that for many-symbol files; a deterministic member outline for a single large class (Java: 47 to 66).
- The coverage-and-specifics prompt helps **only when the input spans the whole file** (+16 and +20 on Rust summaries); it did nothing on capped raw text.
- Paging did not hurt (within 2 points, lenient case). Raw beat summaries by 14 points at 12.4× the cap and lost by 6 to 7 at 22× and 31×, so the cutoff is between about 12 and 22 pages.
- **"Hash exactly what the writer saw" spares 0 of 324 non-test edits** in CodeOwl's Rust history, so it is rejected as the fingerprint rule.

### Decisions the owner made (2026-10-04; all recorded in `DECISIONS.md` q22 and exp-05)
1. Paged-raw cutoff starts at **12 pages**.
2. **For Rust, drop the `#[cfg(test)]` module before paging** (tested: quality 76 against 75, 43 % fewer pages).
3. **Fingerprint rule:** shape (symbol ids, kinds, signatures, export flags, top-level constant values, imports) **+ doc comments + body literals (numbers other than 0, 1, 2 and strings of 8+ characters), on every file** regardless of input rung. Spares about 21 % of edits.
4. **First step of the plan: a new read-only `explain_stale` MCP tool**, name, frontmatter format `  <owner id> -> <target id>: <hash>` (owner `file` for the file itself), hashes shortened to 12 hex, chain depth 3, **no migration**; plus a **backfill of the per-target record for CodeOwl's own current specs as its own commit**.
5. **The import-hash fix was first moved ahead of the file-summary work (definition confirmed), then demoted to pending evidence** after a replay of three repos' history spared only 10 of 14 stale events in about 900 commits. One migration tool is built with the fingerprint change instead.
6. The writer keeps rewriting from scratch (it is not shown the old text or the cause), on purpose: old text carries old mistakes.

### In progress / current state
**`explain-stale` (code, five commits, PR open).** The tenth MCP tool, `explain_stale { id? }`, a pure read. (1) `dependency_target_pairs` and `hash_dependency_pairs` split out of the deps hash, byte-identical. (2) Every submit writes a `dep_targets:` frontmatter block (`  <owner id> -> <target id>: <12-hex hash>`, owner `file` for the file itself); the parser reads it and tolerates specs without it. (3) `src/explain.rs`: the cause chain (`source`, `dependency`, `added`, `removed`, `child`, `rewritten`, `participant`, `unknown`), three levels deep. (4) The tool in `src/mcp.rs`: with an id the chain, without one counts across the corpus (causes, top staling targets, importers resting on a coarse target). (5) Docs. 475+ library tests, gate clean. `FORMAT_VERSION` unchanged. Sidd must reconnect the MCP server after the next `cargo build`.

**After that, in order (exp-05 plan):** 0b the guarded backfill commit (recompute each spec's combined `deps_hash`; write the pairs only if it matches the stored value; skip the rest); then the file-task ladder (step 4), the fingerprint with its migration tool (steps 5 and 0d), the `/codeowl-generate` prompt change in both copies (step 6), housekeeping (step 7). **Step 0c, the import-hash fix, is demoted to pending evidence** (revisit if `explain_stale`'s coarse-target count shows it firing in a real repo).

### Known issues / blockers
1. **The `explain-stale` PR is merged only when Sidd merges it.** Check `gh pr list --head explain-stale` and the state value before deleting the branch.
2. **The import-hash fix (0c) is defined and measured, and demoted.** An importer's `deps_hash` falls back to the target's whole `source_hash` when the target has no `interface_hash` (`interface_or_source_hash`, `spec.rs`); the fix gives a symbol target a signature hash and a file target an exported-interface hash (confirmed by Sidd; migration needed, **no `FORMAT_VERSION` bump**, since `deps_hash` is computed at check time and never cached in the graph). Exposure (resolved importer-and-target pairs with a coarse target): CodeOwl 0 of 292, `ripgrep` 37 of 181, `commons-lang` 818 of 2,049, `quarkus-super-heroes` 30 of 435, Python template 15 of 130, `talentTrail` 0 of 948. **Saving, from replaying history: 14 importer stale events to 4 (10 spared) in about 900 commits** (Java 4 to 3, Rust 3 to 1, Python 7 to 0): real but rare, over-reports only. Build only if `explain_stale` shows it firing in a real repo. The `commons-lang`, `ripgrep` and FastAPI-template clones were deepened to about 300 commits for this (working trees unchanged; the template still has Sidd's own uncommitted `.gitignore` change).
3. **Untested:** a writer that must forget earlier pages and carry notes (needs one agent per page); a mid-size file (16 to 20× the cap) in another stack; Python modules, React components and Next.js routes. Rule L2 was measured on Rust only; other packs need their own literal node kinds.
4. **The experiment data is gone.** Candidates, judge JSON and the private label keys lived in the session scratchpad (`/tmp/claude-1000/...`), not the repo. The method and the Rust and Java key points are in exp-05; the **TypeScript control and key points were withheld on purpose** (`talentTrail` is private). Do not put that repo's logic in the public repo or publish its graph as an artifact.
5. **Stored-spec defects not fixed:** the `src/spec.rs` file summary says "five kinds of document" and lists four; the local `FightService` spec in `quarkus-super-heroes` has two false claims (every public method has `@WithSpan`; two package-private helpers called private).
6. **Stale remote branches:** `origin/docs/fix-doc-contradictions` and `origin/docs/k1-wording-and-spec-regen` still exist though PRs #86 and #87 merged. Verify state `MERGED` and ask Sidd before deleting anything.
7. **Carried forward, still true:** the `codeowl viz` plan still has three unanswered questions (library loading, scope, command shape); open design questions 1, 2, 3, 4, 8, 10, 11, 15, 17, 18, 22 (21 mostly built: `abstract class`/`declare`/`namespace`, renamed clause exports and the Java ambiguous-pick check remain); question 20 deliberately not covered; question 19 same-file-trait residual; K1a plan; the `deps.py` Python-2 bug (fixed only locally); Gradle codegen unverified; the two diverged `/codeowl-generate` copies; `.claude/` is untracked and not to be touched; docs audit gaps (`ROADMAP.md` M20 scope, `REQUIREMENTS.md`, `setup/USAGE.md`, `setup/COPILOT.md`). After any `cargo build`, ask Sidd to reconnect the MCP server (it keeps its spawn-time binary).

### Process lessons from this stretch (do not repeat)
- **Owner approves controls and key points before any generation**, and expects each result "recorded and committed to PR #88" on request. Verify with `gh pr view`, not memory.
- **Never leave a commit that does not compile.** To commit to another branch while the code branch is dirty, use `git worktree add <path> <branch>`, commit and push there, then `git worktree remove`. Do not stash or switch branches under uncommitted work.
- **Say so when the transcript does not hold a definition.** I answered "what is the import-hash fix" from a one-line note, then checked the transcript, found it was never written out, and said so. Check first, then answer.
- **Treat a rough proxy as a rough proxy.** My first "49 % of body edits touch a literal" counted digits inside types (`[usize; 2]`); the real tree-sitter measurement was much lower. Correct it in the same turn.
- **Judge noise is about 5 points for one judge**; only same-session comparisons count. Anchors with known scores catch drift.
- **A measurement of exposure is not a measurement of saving.** Say which one a number is.
- **Throwaway probes** (`examples/probe_*.rs`) stay untracked and are deleted after use; none is left. The "file changed on disk" notices after my own `sed` or `python` edits are harmless.
- Earlier lessons stand: delete a branch only after the PR's state value is `MERGED`; bump `FORMAT_VERSION` with its doc comment; show diffs with Edit/Write and `git diff` before committing; a claim of "unaffected" is a claim to measure.

### Exact next step to resume
1. **Verify the baseline:** `git status -sb`, `gh pr view <tool PR> --json state`. If it is `MERGED`, tell Sidd and sync `master` with the conditional-delete pattern (compare the state *value*).
2. **Step 0b, the backfill, its own commit and branch** (plan, then approval first): for each stored spec, recompute the combined `deps_hash` from the current graph; write the per-target pairs only if it matches the stored value; skip the rest. Then run `explain_stale` with no id on CodeOwl itself; its coarse-target count is the evidence for the demoted import-hash fix (0c).
3. **Then propose the build plan for the file-summary work** (exp-05 steps 4 and 5 with the migration tool 0d): the file-task shape (raw up to about 12 pages, Rust test module dropped first, summaries beyond, a deterministic Java outline), the shape + doc comments + literals fingerprint, and the guarded migration. Branch, plan grounded in the code, tests first, Sidd's approval.

### Git workflow (evergreen)

- **`master` is protected — GitHub-enforced since 2026-09-26, not just convention.** PR required, 1 approval, admins (Sidd) can bypass (`enforce_admins: false`), force-push and deletion both blocked. Every change → branch → PR → **Sidd merges** via the "Merge without waiting for requirements (bypass rules)" checkbox (he's the last pusher, so the 1-owner-approval gate can't be met otherwise — the intended path, now backed by an actual GitHub rule rather than just habit). Claude opens PRs (`gh pr create`) but **cannot merge** (`gh pr merge` is permission-blocked). A merge can be a merge commit or a squash.
- Commits small, atomic, milestone-scoped where applicable (`M17: …`), each compiles + passes tests. Body carries `Authored by: Sidd & Claude Sonnet 5` plus the harness `Co-Authored-By:` / `Claude-Session:` trailers. PR body ends with the `🤖 Generated with [Claude Code](https://claude.com/claude-code)` line.
- `utility/check.sh` before every commit (fmt + clippy `-D warnings` + `cargo test` + staged-diff secret scan; clippy covers `examples/` too). `utility/release.sh` = same in `--release` + a release build, before a release or after perf/overflow-sensitive changes.
- **Delete a branch only after the PR's state *value* is `MERGED`** — compare the value, never chain on `gh`'s exit code (see the process lessons above).

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
- **Show your edits.** Make file changes with the Edit and Write tools so the diff is visible, reserve scripts for counting and checking, and show `git diff` before committing. The owner follows and trusts the work through the diffs.
- **TDD is required for all major changes** — write the failing test first, implement to green. Concretely for this codebase: each `ROADMAP.md` milestone's stated validation *is* that test — write it (as an actual runnable test, not a manual check) before writing the code that satisfies it, not after. Skippable only for genuinely trivial changes (doc fixes, comment tweaks, a rename) — when in doubt, write the test first.
- **Never put secrets in a commit message or diff.** Scan staged content before every commit (`git diff --cached`, grep for key/token patterns), not only when something looks suspicious.
- **Commits are small, atomic, and milestone-scoped.** Reference the `ROADMAP.md` milestone a commit advances where applicable (e.g. `M3: add get_symbol tool`), and never leave a commit that doesn't compile or doesn't pass its tests (once there's code — see TDD above).
- **Bump `FORMAT_VERSION` (`graph.rs`) for a pure logic change too, not only a shape change, and add its entry to the constant's doc comment.** If a commit changes what any pack's `extract_symbols`/`resolve_import`/`feature_model`/`is_schema_symbol` produces for a given input — even with `ExtractedSymbol`/`Symbol`'s own fields untouched — bump it, the same as for an actual struct-shape change. Two real incidents (`ARCHITECTURE.md` open question 9) came from exactly this gap: a pack's logic changed, no shape changed, nobody bumped anything, and a stale `.codeowl/` cache kept silently serving pre-fix results until someone happened to delete it by hand. This is a discipline, not a mechanism — deliberately not a separate per-pack counter, since a full rebuild costs well under a second (confirmed by measurement; it never calls an LLM) and a second counter would carry the identical "someone has to remember" failure, just with a smaller blast radius per miss.
- **`.gitignore` from the first crate-scaffolding commit, not an afterthought.** `target/` and `.codeowl/` (the local gitignored cache — see ARCHITECTURE.md "Storage") must never land in git history, not even once.
- **`clippy` + `rustfmt` run before every commit, not just in CI.** Catches idiom mistakes early — matters more here given this is a Rust-learning project.
- **`utility/check.sh` bundles the whole gate** — `fmt --check` (or `--fix`), `clippy --all-targets -D warnings`, `cargo test`, and the staged-diff secret scan, in order, exiting on the first failure. Run it before every commit instead of the four commands separately. It's **debug-profile only**; `utility/release.sh` runs the same gate in `--release` plus `cargo build --release` (slower — for before a release or after touching perf/overflow-sensitive code, not every commit).
- **Stage first, then run the gate: `git add -A && utility/check.sh && git commit`.** The secret scan reads the *staged* diff, so running the gate before `git add` silently checks an empty index — fmt/clippy/tests still run against the working tree, but the scan is a no-op. It used to print a confident "nothing secret-shaped staged" for that case; it now refuses with a non-zero exit unless you pass `--unstaged`, which runs fmt/clippy/tests and prints `SKIPPED` for the scan. Use `--unstaged` for mid-work checks, never as the pre-commit path.

## Pending decisions

Deliberately unresolved, to revisit when implementation makes them concrete. Design-level open questions are indexed in ARCHITECTURE.md's "Open questions" section, with the full reasoning in `DECISIONS.md` (tree-sitter vs. LSP, the token-budget threshold, polyglot/non-code artifacts); these are the scope-trimming ones that don't belong there:

- **MCP tool surface — resolved by inaction, worth noting rather than reopening.** The shipped surface is 10 tools (`get_symbol`, `get_source`, `get_callers`, `get_callees`, `get_spec`, `get_next_spec_task`, `submit_spec`, `search_code`, `get_spec_coverage`, `explain_stale` — `explain_stale` is a pure read that says why a spec is stale; `get_source` shipped under the agent-reliance track's M20, between the session that first wrote this note and the one that corrected it) — `trace_path`, `get_tests_for`, `get_dependencies` were sketched but never built, simply because nothing has needed them yet; `impact_analysis` was never built either, but for a different reason — `get_callers` already answers "what breaks if I change this," so there was never a separate tool to build. See `ARCHITECTURE.md` §7 for the full current reference. Revisit only when something real calls for one of the three still-deferred tools.
- **Spec-regeneration commit hygiene.** Specs live in git, so a `/codeowl generate` run mid-feature drags spec diffs into an unrelated PR. Explicit generation (rather than silent) already makes this avoidable; the convention that makes it reliable — regenerate as its own commit, or as a deliberate pre-PR step — should be settled once there's a real workflow to test it against.
- **`codeowl viz`** (see Last Session): three questions awaiting the owner's answers.

## Docs

Design decisions go in `ARCHITECTURE.md` / `DECISIONS.md` / `REQUIREMENTS.md`, not in commit messages or code comments. When resolving an open question, update the index in `ARCHITECTURE.md`, the entry in `DECISIONS.md`, *and* the section it affects — leaving a resolved question listed as open is the drift this project exists to prevent.

When a doc or comment coins or leans on a domain term (static-analysis, graph-theory, or CodeOwl-specific), it belongs in `GLOSSARY.md` — keep that file in step as the vocabulary grows.

**Milestone numbers never appear in anything an agent reads.** `M17`, `M20`, "Phase 1" and friends are project bookkeeping — a calling agent can't act on them, they cost tokens in every session, and they silently rot the moment a milestone is renumbered. Keep them out of: `#[tool(description = …)]` strings, the `info.instructions` block, and **any `///` doc comment on a type deriving `JsonSchema`** — `schemars` turns those into the JSON Schema `description` shipped over the wire, so they're agent-facing even though they look like ordinary source comments. Describe the behaviour instead ("`pending` is paginated — 50 entries per call"), not its provenance. They stay welcome in `//` implementation and test comments, in `///` docs on internal types, and throughout `ROADMAP.md` / `ARCHITECTURE.md` / `GLOSSARY.md` / `experiments/` — those are read by humans and future sessions, where the provenance is the point. Same rule covers `setup/codeowl-generate.md` and its `.prompt.md` port: an agent executes those.
