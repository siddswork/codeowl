# CodeOwl — working conventions

CodeOwl extracts a structural graph from a codebase and serves LLM-authored specs (the semantic layer) over MCP. Design lives in `ARCHITECTURE.md` (how it's built) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos; `GLOSSARY.md` defines the static-analysis / graph / CodeOwl-coined vocabulary the rest use (symbol, spec-bearing, flow edge, fan-in, the four hashes, "the socket held", …). Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-09-10 — M16 shipped, 3 rounds of macOS path fixes, big docs pass, self-corpus refresh)

**Goal:** started as "finish M16 commit 5" (branch `m16-java-stack` had commits 1–4). Expanded, driven by the owner, into: ship M16; fix a macOS-only path bug that surfaced in 3 rounds of `cargo test` failures on his Mac; a large documentation pass (VS Code/Copilot setup, GLOSSARY rewrite, spec STYLE.md, `--stale` flag, team-split docs); and a self-corpus refresh + `src/java.rs` generation.

### What was completed — everything merged to `master` (`d5a2c43`), PRs #18–#29

**M16 — `JavaStack` on commons-lang (PR #18):**
- `src/java.rs` — `tree-sitter-java` extractor (class/interface/enum/`@interface`/record → `Container`, methods/constructors/fields → Callable/Value, nested types recursed, Javadoc → docstring, annotations → `markers`), no `merge_inherent_impls` step. Import half: `extract_imports` splits FQNs; `resolve_imports` does path-suffix FQN→file matching (no `pom.xml`, Gradle-free) + a same-package source scan for import-less references.
- `JavaStack` in `stack.rs`; `stack::for_name` gains `"java"`; `lang::detect` restructured from a `match (ts, rs)` 2-tuple to an N-candidate count-and-match; `is_test_tree` gains a `src/test/` clause.
- **Design decision 1 resolved:** every named Java type kind → `Container`, no size test (a record is a restricted `final class`; `Value` would drop DTO-per-file layouts from the corpus). ROADMAP DD1 + exp-02 updated.
- `tests/java_spec.rs` (2 tests). `feature_model()` = trait default `None`. `FORMAT_VERSION` unchanged.
- **commons-lang dogfood** (`serve` + `/codeowl-generate --all --budget=15`, 7 specs read): internal import resolution 100 %, same-package edges accurate, `feature_model() -> None` composes, spec quality high. Findings → M19 (below). The 7 commons-lang specs are left uncommitted in `~/dev/openSource/test-repos/commons-lang/` (corpora are M19 scope).

**macOS symlink `strip_prefix` bug — 3 PRs (#19, #20, #21).** Root cause: an un-canonicalized repo root compared against a symlink-resolved path (oxc_resolver's output, and the file-watcher's event paths). macOS `std::env::temp_dir()` sits under `/var` → `/private/var`; Linux `/tmp` is real, so it only showed on his Mac. **Production (`main.rs` already canonicalizes) was never affected — it's a direct-library-caller (test) gap.** Fixes canonicalize at every path boundary:
- #19 — `resolve.rs`: `resolve_imports` / `resolve_default_imports` canonicalize `repo_root`.
- #20 — `index.rs`: `RepoIndex::build`/`open` add `canonical_root(root)`; `watch::spawn` canonicalizes its `root`; `CodeOwlServer::root()` test accessor added.
- #21 — `index.rs`: `apply_changes` runs each event path through `canonicalize_event_path` (canonicalize, or canonicalize the parent + re-attach the name for a just-deleted file).
- Full audit done — `lang.rs:detect`, `search.rs`, `rel_path` all walk-and-strip the *same* root so they're internally consistent; no other sites. `structural_sweep.py` byte-identical on the pilot for all three.

**Docs (PRs #22–#25, #29):**
- **VS Code + GitHub Copilot** — `setup/COPILOT.md`, `setup/codeowl-generate.prompt.md` (Copilot port of the slash command), `setup/mcp-vscode.json`. Frontmatter/config **verified against current VS Code docs**: `.vscode/mcp.json` uses `servers` (not `mcpServers`) + `type: "stdio"` + `${workspaceFolder}`; the prompt file uses `tools: ['codeowl/*']` (MCP-server wildcard, also implies agent mode — no `mode:`/`agent:` field); prompt files load automatically (no `chat.promptFiles` toggle in current VS Code) and are *not* loaded by Agent Host sessions.
- **`GLOSSARY.md`** — full plain-English rewrite (295 → ~600 lines): every entry leads with the plain idea → example → precise detail. Expanded "Stacks and the pluggability work" section (added `Pack`, `detect`, `Pack hook / seam`, `The generic pipeline`). Defined `Slug`. Linked Niko Matsakis's "graphs via vector indices" post on the `Arena` entry.
- **`docs/specs/STYLE.md`** — repo-specific spec-generation guidance (audience = app dev, no compiler background; Summary jargon-free; gloss terms inline). `/codeowl-generate` + `.prompt.md` gained a hook: "if the repo has `docs/specs/STYLE.md`, read it first." Generic (any repo can drop one).
- **`--stale` flag** on `/codeowl-generate` (PR #26) — `--all --stale [--budget=N]` filters the batch's `pending` to `status: "stale"` only (skips `missing` and `smelly`). Purely client-side; `get_spec_coverage` already reports per-item `status`. Documented in `README.md` / `USAGE.md`.
- **`setup/USAGE.md`** (PR #29) — "Splitting the work across a team" (divide by directory, `get_spec_coverage` `scope`, one PR per slice) + "What CodeOwl stores" (`docs/specs/**.md` committed vs `.codeowl/graph`+`.codeowl/index` per-machine cache, safe to delete).

**Self-corpus (PRs #27, #28):**
- #27 — refreshed the **6 stale file specs** (`lang`, `resolve`, `stack`, `index`, `mcp`, `watch`) via `/codeowl-generate --all --stale --budget=20` — 17 generations, STYLE.md applied (first real use of both `--stale` and STYLE.md).
- #28 — generated **`src/java.rs`** (23 generations, no spec existed).

### Self-corpus state (live `get_spec_coverage` on this repo)

**15 current · 0 stale · 4 missing · 0 smelly** · `generations_remaining` 120. Missing = `src/rust.rs` (28 gens), `src/spec.rs` (90), `rollup:src` (1, **blocked**), `system` (1, **blocked**). `rollup:src` and `system` only become generable once every `src/` file is current — so `rust.rs` + `spec.rs` must be filled first. This is the **post-polyglot-core batch** (owner scoped it there in M15; the polyglot core now ends at M19).

### Known issues / carry-forward (mostly → M19)

1. **[M18 — owner scoped it with the Java revisit; from the M16 dogfood] `get_next_spec_task` returns a folded Container's whole-file source.** For a large file (`StringUtils` 420 KB on commons-lang; CodeOwl's own `mcp.rs`/`spec.rs` hit ~95 KB) this exceeds the MCP tool-result token limit → the client persists it to a file and greps. Fix: for a large Container task, send member signatures + docstrings, not full bodies. Related enhancement: **serve `docs/specs/STYLE.md` in `get_next_spec_task`'s context** so the guidance travels with the task regardless of client (currently only the slash command reads it).
2. **`get_spec_coverage` `pending` payload** also over-limit on a big repo (commons-lang: 95 KB / 626 items) — cap or paginate. M19.
3. **Java same-package scan over-inclusive** when a simple name is shadowed by an explicit `import` of a different class (commons-lang `lang3.Streams` vs `lang3.stream.Streams`). Also: a resolved `import static` lands a member-level dep next to a class-level one — decide the granularity. M19.
4. **No pre-built binaries.** A GitHub Actions release workflow (macOS + Linux) is the real fix for any non-dev consumer — surfaced by the Copilot-setup work, **not yet a ROADMAP item**. Near-term workaround documented in `COPILOT.md` (`cargo install --git`).
5. **Windows-native not covered** (owner set it aside). If revisited: `dunce::canonicalize` for the `\\?\` verbatim prefix + case-insensitivity in the `RepoIndex.files` lookup. macOS + WSL2-Ubuntu both green now. See memory `dev-environment`.
6. `FeatureModel` still has ONE impl — M17 (FastAPI) is the second, M18 (Quarkus) the third and first non-web.
7. `resolved_default_imports` still a separate `Graph` field — folds into `pack.resolve_imports`'s output at M19.
8. **M17 fixes** M10's `.sql`-file schema assumption — a symbol-level `is_schema_symbol` pack hook, `.sql` out of `lang.rs` core, SQLModel/SQLAlchemy impl. (Was slated for M18/"generalize later"; pulled forward into M17 so a second persistence idiom isn't bolted onto the wrong shape.) M18 adds the JPA/Panache impl.
9. **`/home/sidd/.claude/plans/dapper-greeting-sprout.md`** is the M16 plan — M16 is done, so it's spent (leave or delete).

### Process notes for future-me

- **Branch-deletion mistake made TWICE this session** — deleted branches whose PRs were still OPEN (recovered both via reflog + `gh pr reopen`). **New rule (memory `never-delete-branch-before-pr-merged`): before ANY `git branch -d/-D` or `git push origin --delete`, run `gh pr view <n> --json state` and confirm `MERGED`. Do not tidy branches proactively — wait for the owner to say a PR is merged.**
- **After any `cargo build`, ask the owner to `/mcp` reconnect** before trusting `mcp__codeowl__*` results — the server keeps its spawn-time binary. The *graph* is kept live by the watcher, but the binary's hashing/coverage logic isn't. Memory: `reconnect-mcp-after-code-change`.
- The oxc_resolver-symlink memory (`fix-oxc-resolver-symlink-strip-prefix`) is now fully resolved (PRs #19–21 all merged) — safe to delete.

### State

- `cargo test`: **204** (189 unit + 15 integration), all green. `utility/check.sh` clean on `master` (`d5a2c43`). No open PRs, no local branches but `master`, working tree clean.
- `.codeowl/` in this repo: pack `rust`, `format_version` 6, warm. The session's `codeowl` MCP server was reconnected after PR #26 and used for the #27/#28 generation runs; **rebuild + `/mcp` reconnect at the start of the next code session** (many builds happened; the last generation run's server is on a debug binary that predates nothing code-relevant but reconnect anyway).
- Test repos present: `~/dev/openSource/test-repos/{commons-lang, quarkus-super-heroes, leveldb, full-stack-fastapi-template}`.

### Exact next step to resume

**M17 was rescoped 2026-09-10 (owner's call) — it is now `PythonStack` + FastAPI, not Quarkus.** Quarkus → M18, iterate-and-ship → M19. Docs for this land on branch `m17-python-fastapi-docs` (PR pending review — no code yet). Once merged:

**A. M17 — `PythonStack` + FastAPI** on `~/dev/openSource/test-repos/full-stack-fastapi-template/backend` (the `backend/` subdir — the repo root is polyglot and would `detect()` as TypeScript). Read `ROADMAP.md` → `### M17`. The work, TDD (`tests/python_spec.rs` first): `src/python.rs` (`tree-sitter-python`; classes/defs/assignments; decorators-with-args → `markers`; `"""docstrings"""`; path-suffix + relative-import resolution); the **schema seam** — `fn is_schema_symbol(&self, sym) -> bool` on `StackPack` (default `false`), pipeline retags to `SymbolKind::Schema`, `SourceKind::Schema` + the `.sql` branch leave `lang.rs`; the Python body keys on `table=True` (SQLModel) / `Base` subclass (SQLAlchemy); the **FastAPI feature model** (`feature_model() -> Some` — the second impl) — decorator scan, `APIRouter`/`include_router` prefix assembly (a flow edge), `Depends()`/`Annotated`-alias `core` walk. No cross-service edges (that's M18). Pilot must regenerate schema specs byte-identical (`structural_sweep.py`). Design-decision notes to log: `is_schema_symbol` signature (needs class args — a new `ExtractedSymbol.bases` field), `Depends()` chain depth, `tree-sitter-python` crate version.

**B. Finish the self-corpus** (independent, anytime, its own PR): `/codeowl-generate src/rust.rs` (28 gens) after `/mcp` reconnect, then `src/spec.rs` (90 gens — its own PR, maybe in `--budget` chunks), which unblocks `rollup:src` + `system`. STYLE.md applies. Commit `docs/specs/` on its own.

**Later — M18: Quarkus on quarkus-super-heroes.** The Java *feature layer* + the JPA/Panache `is_schema_symbol` impl (second one) + `@RegisterRestClient` cross-service edges + multi-module Maven. See `ROADMAP.md` → `### M18` and `experiments/exp-02-feature-layer.md` Part 2.

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
