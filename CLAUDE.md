# CodeOwl — working conventions

CodeOwl extracts a structural graph from a codebase and serves LLM-authored specs (the semantic layer) over MCP. Design lives in `ARCHITECTURE.md` (how it's built) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos; `GLOSSARY.md` defines the static-analysis / graph / CodeOwl-coined vocabulary the rest use (symbol, spec-bearing, flow edge, fan-in, the four hashes, "the socket held", …). Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-09-14/15 — M17 merged after a 4th dogfood bug + a 10-finding code review; self-corpus completed; polyglot spike revised and merged; M18/Quarkus started)

**Goal:** picked up mid-M17-dogfood, from a prior session's own uncommitted handoff draft (never committed — this section replaces it). The actual job: finish what that draft flagged as still missing — **the real M17 validation gate** (a human quality read of generated specs, not just infrastructure passing) — get PR #33 merged, then figure out what's next. Ended up covering all of that plus a full `/code-review` pass, the self-corpus, the polyglot spike's review, and the start of M18.

### What was completed

**PR #33 (`m17-python-stack`) — MERGED into `master` (`4ae828f`).** The prior session's 10 commits, plus 12 more this session:
- **4th dogfood finding**, found *during* the actual quality-read validation (not before it, unlike the first 3): `get_next_spec_task`'s `feature:<slug>` branch resolved the exact requested entry, then discarded it except for `.file`, falling into the same file-level round-robin the 2nd fix built — so `feature:http-get-items` could silently return a sibling's task instead, since entry points sort globally by slug id. Fixed (`next_task_for_feature`, TDD).
- **The M17 validation gate finally run**: read 4 generated feature specs against real source (a trivial unauthenticated route, a read path, a write path, a superuser-gated delete with cascading data), fact-checked specific claims line by line. Passed — one logged non-blocking gap: the `/api/v1` prefix is accurate in every spec, but only because the LLM read beyond its assigned `participants`, not because anything hash-checked verifies it.
- **A real bug found in the dogfood *target* repo, not CodeOwl**: `full-stack-fastapi-template/backend/app/api/deps.py:36` has genuine Python-2 tuple-exception syntax (`except InvalidTokenError, ValidationError:`) — confirmed via `git blame` as landed on the real upstream `master`, not a local artifact. Fixed in the local clone; **not yet contributed upstream** (MIT-licensed, has `CONTRIBUTING.md` — a good small first PR, still on the to-do list).
- **`/code-review` (high effort) run against the branch — all 10 findings fixed, each with TDD, each confirmed red before green:** `resolve_flow_edge` picking the wrong file for a same-named cross-file `Depends()`; a bare `import app` (single-segment, no `from`) never resolving; colliding route slugs silently dropped by `dedup_by`; `submit_feature` silently succeeding on an id that merely *collides* with an unrelated real file/symbol, corrupting the spec store; `kwarg_string_literal` matching a kwarg's name inside another kwarg's *string value*; `docstring()` missing uppercase `F`/`U` string prefixes; a `data`-tier classification asymmetry between the import-resolved and flow-edge-resolved paths — 7 real bugs. Plus 3 pure refactors (zero behavior change, full suite green unchanged): deduped `mcp.rs`'s two near-duplicate feature-task paths; indexed `flow_edges`/`imports` by file once in `assemble_participants` instead of rescanning per pass; memoized `router_prefix`/`admits_to_core` within one call (deliberately *not* across-run — that's a bigger design decision, logged not built).
- `ARCHITECTURE.md` **open question 9** added: `FORMAT_VERSION` doesn't catch a pack's logic changing under an unchanged cache shape — found when a stale `.codeowl/` cache from before this session's fixes nearly got silently reused. Logged, not acted on (existing rebuild-discipline mitigation holds).
- `check.sh` clean and full suite green after every commit; 211 → 218 lib tests by the end. PR title/body rewritten twice (it still said "Draft — commit 1 of ~4" from the very start of the branch) before undrafting and merge.

**PR #34 (self-corpus / Track C) — MERGED into `master` (`6a53d4c`).** A peer session (`dogfooding-codeowl`, sharing this working tree) finished CodeOwl's own corpus in parallel — 15 current/4 missing → 21/0/0/0 (`src/rust.rs`, `src/spec.rs`, `rollup:src`, `system` all landed) — plus found a stale docstring in `src/stack.rs` (said the FastAPI feature model was "still to come," true pre-M17). Branched, committed both, PR'd, merged.

**PR #32 (`experiments/exp-03-polyglot.md`) — MERGED into `master` (`9772180`), after 3 revision commits this session:**
- Verified the polyglot claim empirically (41 Python / 108 TS by `detect()`'s own count on the real repo; confirmed `detect()` at the repo root errors cleanly today and never silently picks — a claim to the contrary in the doc itself was stale and got corrected).
- **Q2 rewritten**: retired the primary=full-capability/secondary=reduced-mode split. Every root now gets whatever its own pack supports — including a full feature layer — gated only on whether that capability fires for that specific subtree, never on primary/secondary status. Owner's framing, driving the change: once a language has a `StackPack`, it works the same in any subtree.
- Two more "Open, deliberately" items resolved: the two-firing-feature-models tiebreak (deterministic — alphabetical by root path, `.codeowl.toml` overrides, never an error) and `packages/`/monorepo discovery (discover-by-default once a floor clears, never gated on being imported — explicitly rejecting "only if imported" as the default, since that reintroduces the exact silent-omission bug this milestone exists to fix).
- A full review pass fixed real self-contradictions the revisions introduced (a still-undecided-sounding bullet after its own resolution; a heading contradicting its own updated step; a circular doc pointer), retitled the doc ("one primary stack + secondary-language subtrees" → "one arena, many roots" — the old title named the split Q2 just retired), corrected stale file counts, and flagged `RootSpec.kind` explicitly as ordering-only metadata — the single most likely place an `if kind == Secondary { skip }` would quietly undo the whole Q2 revision during implementation.
- Owner reviewed, approved, merged.

**M18 (Quarkus) started, branch `m18-quarkus`:**
- **Sequencing decided**: build Quarkus before the polyglot milestone (reversing the owner's own 2026-09-10 call). Recommended this — Quarkus is the smaller, better-understood addition, already de-risked by this session's "several entry points share one file" fixes (the identical shape a JAX-RS resource class hits), and validating the bigger multi-root change afterward against 4 mature stacks is a better test than 3. Owner agreed.
- `ROADMAP.md`'s status block updated (was stale since 2026-09-10, written before any M17 code existed) to record M17/self-corpus/polyglot-spike completion and "Next: M18."
- **Smoke-tested `JavaStack` (M16, unmodified) against the real `quarkus-super-heroes` repo**, read-only, no code written: whole 7-module reactor extracts cleanly, 1400 symbols, **no polyglot-ambiguity problem** (the `ui-super-heroes` frontend is only 10 plain `.js` files — nowhere near enough to contest Java). **Multi-module resolution is likely already a non-issue**: each service module has its own distinct Java package namespace and they only talk via REST/Kafka/gRPC at runtime (the `@RegisterRestClient` scope item), never direct cross-module imports — M16's existing resolution has nothing new to resolve.
- `ARCHITECTURE.md` **open question 10** added: `feature_model()` is fixed per-`StackPack` at compile time with no repo-context parameter — fine so far since each language has had exactly one real framework worth modeling. Surfaced by the owner asking what happens when Spring Boot support is wanted alongside Quarkus: two possible shapes sketched (one recognizer tolerant of several annotation vocabularies onto the same `EntryPoint.kind`; or several `FeatureModel` impls composed/merged), neither built, deliberately not decided — matches this project's "wait for a real repo" policy (Spring Boot isn't scheduled).

### In progress / current state

- Branch `m18-quarkus`, pushed, one commit so far (`876e48d`, the `ROADMAP.md` status update) plus this handoff + the `ARCHITECTURE.md` open-question-10 addition as a second commit.
- **No Quarkus implementation code exists yet** — only bookkeeping and read-only reconnaissance. `JavaStack` is exactly as M16 left it.
- `target/debug/codeowl` is built from `master` (`9772180`) plus the uncommitted `ARCHITECTURE.md` edit; confirmed working against `quarkus-super-heroes`.
- Three now-merged branches (`m17-python-stack`, `docs-self-corpus-completion`, `exp-03-polyglot-spike`) are being deleted, locally and on `origin`, as part of this same wrap-up — each confirmed `MERGED` via `gh pr view` immediately beforehand.

### Known issues / carry-forward

1. **`ROADMAP.md`'s "Planned insert" note is now stale.** It still says the polyglot milestone "inserts as M18, shifting Quarkus → M19" when it lands — but the owner's call this session was Quarkus *first*. Quarkus keeps its original M18 number; the polyglot milestone becomes whichever number follows once it's picked up. **Not yet fixed** — noticed while writing this handoff, not before. Fix this before it causes confusion.
2. **The `deps.py` Python-2 syntax bug is fixed only locally**, not contributed upstream to `fastapi/full-stack-fastapi-template`. Owner wants to do this "later" — a real, easy, MIT-licensed first PR when picked up.
3. **`ARCHITECTURE.md` open questions 9 and 10** are both deliberately deferred, not blocking — logged so neither gets rediscovered from scratch.
4. `dogfooding-codeowl`, the peer session that did the self-corpus work, ended cleanly sometime this session (no longer listed by `ListAgents`) — not an issue, just noting it's gone.

### Exact next step to resume

**Fix carry-forward item 1 first** (a two-line `ROADMAP.md` edit — the "Planned insert" note's renumbering claim), then **start M18 implementation, commit 1: the entry-point model.**

1. `/mcp` reconnect if resuming in a live session (binary was rebuilt during the smoke test).
2. TDD first: write the failing test for `JavaStack::feature_model()` enumerating a JAX-RS `@Path` resource in `quarkus-super-heroes` as an `EntryPoint`, confirm it fails against today's `-> None`, then implement.
3. **Extend `JavaStack`, don't add a separate `QuarkusStack`** — matches the established `TypeScriptNextStack`+`TypeScriptNextFeatureModel` / `PythonStack`+`FastApiFeatureModel` precedent (one stack, one feature model, fires-or-doesn't per real repo conventions).
4. Start with HTTP resources only (JAX-RS `@Path` + verb annotations); Kafka/`@Scheduled`/gRPC are separate, additive commits once the first kind's shape is proven — same incremental pattern M17 used.
5. Build the kind-prefixed-slug rule in from the start (`ROADMAP.md` already flags the `http-fights`-vs-`kafka-fights` collision risk) — cheaper than retrofitting.
6. After the entry-point model: `admits_to_core` (CDI-shaped), the second `is_schema_symbol` body (JPA/Panache), `@RegisterRestClient` cross-service edges, then the God-class `get_next_spec_task` payload fix (generic core, motivated by Java's large classes) — full scope already written in `ROADMAP.md`'s M18 section.

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

## Pending decisions

Deliberately unresolved, to revisit when implementation makes them concrete. Design-level open questions live in ARCHITECTURE.md's "Open questions" list (tree-sitter vs. LSP, the token-budget threshold, polyglot/non-code artifacts); these are the scope-trimming ones that don't belong there:

- **Trim the MCP tool surface.** Twelve tools is a lot for Phase 1. A plausible minimum is `get_spec`, `get_next_spec_task`, `submit_spec`, `get_spec_coverage`, and `get_callers`/`get_callees` — with `trace_path`, `get_tests_for`, `get_dependencies` deferred until something actually calls for them. (`search_code` itself is settled — see ARCHITECTURE.md "Storage": Phase 1 is ripgrep, no index — but whether it's even one of the five kept vs. cut entirely from Phase 1 is still open.) Cutting these is cheap now and expensive once they have consumers.
- **Spec-regeneration commit hygiene.** Specs live in git, so a `/codeowl generate` run mid-feature drags spec diffs into an unrelated PR. Explicit generation (rather than silent) already makes this avoidable; the convention that makes it reliable — regenerate as its own commit, or as a deliberate pre-PR step — should be settled once there's a real workflow to test it against.

## Docs

Design decisions go in `ARCHITECTURE.md` / `REQUIREMENTS.md`, not in commit messages or code comments. When resolving an open question, update the open-questions list *and* the section it affects — leaving a resolved question listed as open is the drift this project exists to prevent.

When a doc or comment coins or leans on a domain term (static-analysis, graph-theory, or CodeOwl-specific), it belongs in `GLOSSARY.md` — keep that file in step as the vocabulary grows.
