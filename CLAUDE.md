# CodeOwl — working conventions

CodeOwl extracts a structural graph from a codebase and serves LLM-authored specs (the semantic layer) over MCP. Design lives in `ARCHITECTURE.md` (how it's built) and `REQUIREMENTS.md` (what and for whom); `ROADMAP.md` has the build sequence and test repos; `GLOSSARY.md` defines the static-analysis / graph / CodeOwl-coined vocabulary the rest use (symbol, spec-bearing, flow edge, fan-in, the four hashes, "the socket held", …). Read those before proposing design changes — most "obvious" improvements have already been argued through and resolved there.

## Last Session (2026-09-10 — design review → M17 rescoped to Python/FastAPI + schema-seam fix; docs only, no code)

**Goal:** started as "check on the next set of tasks". Became, driven by the owner: a design review of the stack-pack model (prompted by owner concerns about polyglot repos and the schema layer), then a **rescope of M17** from Quarkus to `PythonStack` + FastAPI + the schema-seam fix, then a **doc-drift sweep** across ROADMAP / ARCHITECTURE / GLOSSARY / REQUIREMENTS. **No code this session** — one docs-only PR (#31), pending merge.

### Design conclusions reached (so future-me doesn't relitigate — all now in the docs)

- **The `StackPack` trait decomposition is sound and well-validated** — 3 stacks in (TS+Next, Rust, Java), the generic core (`spec.rs`/`mcp.rs`/`graph.rs`/`index.rs`) never grew a stack-specific `if`, "the socket held" each time. Don't reopen the hooks or `feature_model()`-optionality.
- **"Stack, not language" has only ONE non-degenerate data point** — TS+Next (SQL schema + `.from()` + routes + feature model). Rust and commons-lang are *degenerate* (`feature_model() -> None`, no schema, empty flow edges) — under a plain "LanguagePack" model they'd look identical. **M17 (FastAPI) and M18 (Quarkus) are the real tests of the bundle framing.**
- **"One stack per repo" (`detect()` = one winner or error) is the weak decision** — the polyglot-repo case (a FastAPI backend + a React frontend in one repo). Deferred so far; now getting its own milestone before Java (see below).
- **The schema layer is concretely wrong, and M17 fixes it.** M10 bound `SymbolKind::Schema` to `.sql` *files* via `SourceKind::Schema` in the generic core (`lang.rs`). That doesn't survive an ORM (SQLModel/SQLAlchemy/JPA declare tables as classes in code files) and a non-TS repo's `.sql` files are invisible (only the TS pack's `source_kind` claims them). Fix = a **symbol-level `is_schema_symbol(&ExtractedSymbol) -> bool` pack hook** (default `false`); the pipeline retags to `SymbolKind::Schema` after `extract_symbols`; `.sql` becomes a pack-*declared* capability, not a core concept. M17 does the seam + the SQLModel/SQLAlchemy body; M18 adds the JPA/Panache body (the second impl confirms the shape). Doing it now avoids M18 reverse-engineering an abstraction from two hardcoded cases.
- **ORM coverage is a growing list, not a closed set.** Dozens of idioms, several per language, which one a repo uses is independent of its primary language. Each is a small pack-owned `is_schema_symbol` body added when a target repo needs it (same "wait for a real repo" rule as `feature_model()`). Degrades in tiers: **full** (idiom recognised → table nodes + resolved edges), **partial** (`.sql`/migration present but idiom unrecognised → table nodes, weak edges), **none** (nothing recognised → no schema layer; feature specs describe behaviour but not data). "None" is honest under-reporting, not a bug — but a target repo whose persistence idiom isn't covered must be a **logged known-gap before its corpus is judged**.

### What was completed — PR #31, branch `m17-python-fastapi-docs`, **not yet merged**. Docs only.

**Owner decisions (all captured in the docs):**
1. **M17 rescoped** → `PythonStack` + FastAPI + schema seam. **Quarkus → M18**, iterate-and-ship → **M19**.
2. **M17 target = `full-stack-fastapi-template/backend`** (the `backend/` subdir — repo root is polyglot, ~86 `.tsx` vs ~19 `.py`, would `detect()` as TypeScript). Confirmed after weighing `dispatch`/`fief` (repos whose root is Python) — kept the subdir because it's purpose-built, right-sized for TDD + human review, hits every M17 feature, and the same clone drives the polyglot milestone.
3. **God-class `get_next_spec_task` payload fix (M16 finding) moved M19 → M18** — the owner scoped it with the Java revisit (Java God-classes are where it bites: `StringUtils` 420 KB).
4. **A polyglot / secondary-language milestone inserts BEFORE Java.** Owner wants it worked before M18/Quarkus, driven by `full-stack-fastapi-template` (whole repo). **Captured as a forward note, NOT renumbered** (scope undesigned — needs a spike). When it lands it's M18, shifting Quarkus → M19, iterate → M20.

**Commit `4308538` — rescope M17, renumber, docs align:**
- `ROADMAP.md` — new `### M17` milestone (full scope: `src/python.rs`, the schema seam, the FastAPI feature model, TDD `tests/python_spec.rs` first, design-decision notes); Quarkus/iterate renumbered to M18/M19; M16's God-class finding moved into M18; test-repo table row for `full-stack-fastapi-template/backend`; a **"Planned insert"** block for the polyglot milestone; `2026-09-10` changelog entry. Dated `Revised again` changelog entries left as written (they say "M18 finding" meaning the old iterate milestone = now M19 — the 2026-09-10 entry explains this; **do not "fix" them**).
- `ARCHITECTURE.md` — open question 3 schema/data bullet updated (file-level → symbol-level seam); new **"Secondary-language subtrees"** sub-bullet under q3; new **open question 8** (schema/persistence coverage = growing list, tiered degradation); §6 `data` participant tier now admits ORM-model symbols.
- `GLOSSARY.md` — new **`is_schema_symbol`** entry; `FeatureModel` / `Marker` entries note FastAPI + Python decorators.
- `experiments/exp-02-feature-layer.md` — numbering note at top (its "M17" = the Quarkus milestone, now M18).
- `CLAUDE.md` — "Exact next step" + carry-forward milestone numbers updated (this handoff supersedes that).

**Commit `6333003` — 4 REQUIREMENTS.md drift fixes:**
1. Native Windows was asserted as a hard requirement + "the primary constraint" on the Rust choice → softened to aspirational + a "Phase 1 reality" clause (validated on macOS + WSL2; native Windows set aside).
2. The polyglot / multi-stack scope decision was **entirely absent** → new "Language & stack coverage" subsection (one `StackPack` per repo; stacks supported; schema as a per-stack companion; multi-language-in-one-repo as a planned-not-scoped milestone).
3. Open-questions list missing the secondary-language question → added as question 3.
4. Phase 1 read as a live hypothesis → added a Status line (shipped + validated by sample 2026-09-07).

**Also:** `full-stack-fastapi-template` cloned to `~/dev/openSource/test-repos/`.

### Concrete findings from inspecting `full-stack-fastapi-template/backend` (so M17 impl doesn't re-derive)

- **SQLModel `table=True` is the whole "is this a table" signal.** `class Item(ItemBase, table=True)` is a table; siblings `ItemCreate` / `ItemPublic` share the `SQLModel` base and are request/response schemas — **only `table=True` discriminates**. So `is_schema_symbol` must see the class *argument list* → a new `ExtractedSymbol` field (`bases` / class-args); may need a `FORMAT_VERSION` bump if it lands on the persisted `Symbol`.
- **The code→table edge is FREE for the SQLModel case.** Routes do `session.exec(select(Item))` with `Item` the imported class — an ordinary resolved reference once `Item` is a `Schema` node. `get_callers(Item)` then lists `app/api/routes/items.py` + `app/crud.py` with no flow-edge machinery. The fuzzy `FROM (\w+)` string path is a fallback for raw-SQL repos only — this repo has none.
- **Router prefix nests twice:** `APIRouter(prefix="/items", tags=["items"])` constructor kwarg in `app/api/routes/items.py`, **then** `app.include_router(api_router, prefix=settings.API_V1_STR)` in `app/main.py`. The outer prefix is a `settings.X` constant — record it symbolically (`{API_V1_STR}/items/`); the literal segment (`/items/`) is what matters for the slug/title.
- **`Depends` is hidden behind module-level `Annotated` aliases:** `SessionDep = Annotated[Session, Depends(get_db)]` in `app/api/deps.py`; routes write `session: SessionDep`. `admits_to_core`'s Depends-follow must resolve the alias name → its `Annotated[..., Depends(X)]` → `X` first. `Depends()` nests.
- `app/crud.py` is the repository module → `admits_to_core` should admit it. Non-`table=True` Pydantic/SQLModel classes are **not** core data work — one-hop stubs.
- `java.rs::annotations()` already captures full annotation text **including arguments** — the same approach ports directly to Python decorators (`@router.get("/items/{id}")` → marker string verbatim).
- Sizes: 19 backend `.py` (non-test), 5 Alembic `versions/*.py` (→ `Generated` — schema *changes*, not current state), ~86 frontend `.ts`/`.tsx` (Vite + TanStack Router — out of scope).

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
10. **PR #31 not merged** — the whole of this session's work. Docs only, owner merges via the bypass checkbox. M17 code / the polyglot spike / self-corpus should build on it (or at least not diverge).
11. **The polyglot / secondary-language milestone is undesigned.** Needs a paper spike first (`experiments/exp-03-polyglot.md`, like M13-pre / exp-02). Five open questions to answer before any code: (a) `detect()` from "one winner or error" → "one primary (highest non-test count) + N secondaries" — what's the bar for a secondary to count; (b) "reduced mode" for a secondary pack — my proposal is `extract_symbols` + `classify` only, no `resolve_imports` / flow edges / feature model (so the frontend gets file specs + rollups + a system-spec mention, no `get_callers` inside it, no feature narratives) — is that the right cut; (c) one arena or a `Graph`-per-language (affects `.codeowl/` shape + `FORMAT_VERSION`); (d) cross-language edges (generated OpenAPI client, `fetch` → route) in or out — leaning **out** for the first cut; (e) test repo — `full-stack-fastapi-template` whole-repo is the obvious one. **Tradeoff the owner is accepting:** polyglot is genuinely new architecture (multi-graph, `detect()` changes) where Quarkus is "more of the same" (lower risk, further seam validation) — doing the risky thing first is a deliberate priority call ("polyglot repos matter more than a second JVM corpus").

### Process notes for future-me

- **Branch-deletion mistake made TWICE this session** — deleted branches whose PRs were still OPEN (recovered both via reflog + `gh pr reopen`). **New rule (memory `never-delete-branch-before-pr-merged`): before ANY `git branch -d/-D` or `git push origin --delete`, run `gh pr view <n> --json state` and confirm `MERGED`. Do not tidy branches proactively — wait for the owner to say a PR is merged.**
- **After any `cargo build`, ask the owner to `/mcp` reconnect** before trusting `mcp__codeowl__*` results — the server keeps its spawn-time binary. The *graph* is kept live by the watcher, but the binary's hashing/coverage logic isn't. Memory: `reconnect-mcp-after-code-change`.
- The oxc_resolver-symlink memory (`fix-oxc-resolver-symlink-strip-prefix`) is now fully resolved (PRs #19–21 all merged) — safe to delete (still not actioned as of this session).
- **Renumbering convention (used this session):** forward-looking milestone refs get renumbered; **dated `Revised again <date>` changelog entries and dated `### Mn` status boxes are left as written** — a new dated changelog entry explains the shift. Don't sweep-replace across the whole file.
- This session was docs-only and clean — no process mistakes to flag.

### State

- **No code changed this session.** `cargo test` still **204** (189 unit + 15 integration) — not rerun, zero `.rs` touched. `master` is at `c6f9ae8` (PR #30 merged); `.codeowl/` pack `rust`, `format_version` 6.
- **Branch `m17-python-fastapi-docs` → PR #31**, docs only: `ROADMAP.md`, `ARCHITECTURE.md`, `GLOSSARY.md`, `REQUIREMENTS.md`, `CLAUDE.md`, `experiments/exp-02-feature-layer.md`. `cargo fmt --check` clean; staged-diff secret scan clean; `check.sh` not run (no `.rs`). **Pending owner merge** (bypass checkbox). This handoff is the 3rd commit on the branch.
- **MCP server is on a stale binary from a prior session — `cargo build` + `/mcp` reconnect before ANY M17 code or generation** (memory `reconnect-mcp-after-code-change`).
- Test repos present: `~/dev/openSource/test-repos/{commons-lang, quarkus-super-heroes, leveldb, full-stack-fastapi-template}`.
- Self-corpus unchanged: **15 current · 0 stale · 4 missing** (`src/rust.rs` 28 gens, `src/spec.rs` 90, `rollup:src` + `system` blocked until those two land).

### Exact next step to resume

**Merge PR #31 first** (or build on the branch). Then three tracks — owner picks:

**A. Polyglot / secondary-language spike — `experiments/exp-03-polyglot.md`.** The owner wants this **before Java** (M18-to-be). Paper first, like M13-pre / exp-02 — answer the five questions in carry-forward item 11, then fold conclusions into `ARCHITECTURE.md` open q3 and a real milestone entry (which does the renumber: polyglot = M18, Quarkus → M19, iterate → M20). Claude offered to draft it.

**B. M17 — `PythonStack` + FastAPI** on `~/dev/openSource/test-repos/full-stack-fastapi-template/backend` (the subdir — the root would `detect()` as TypeScript). Full scope in `ROADMAP.md` → `### M17`. TDD — `tests/python_spec.rs` first. In brief: `src/python.rs` (`tree-sitter-python`; classes/defs/assignments; decorators-with-args → `markers`; `"""docstrings"""`; path-suffix + relative-import resolution); the **schema seam** (`fn is_schema_symbol(&self, sym) -> bool` default `false`, pipeline retags to `SymbolKind::Schema`, `SourceKind::Schema` + the `.sql` branch leave `lang.rs`; Python body = `table=True` in class args / `Base` subclass); the **FastAPI feature model** (2nd `FeatureModel` impl — decorator scan, `APIRouter`/`include_router` prefix assembly as a flow edge, `Depends()`/`Annotated`-alias `core` walk). No cross-service edges (M18). Pilot must regenerate schema specs byte-identical (`structural_sweep.py`). See "Concrete findings" above — especially: `is_schema_symbol` needs class args → a new `ExtractedSymbol` field; the SQLModel code→table edge is free via ordinary import resolution. Log design-decision notes: the new field's shape + `FORMAT_VERSION`, `Depends()` chain depth, `tree-sitter-python` crate version (must match the workspace `tree-sitter` core version).

**C. Finish the self-corpus** (independent, its own PR): `/codeowl-generate src/rust.rs` (28 gens) after `/mcp` reconnect, then `src/spec.rs` (90 gens — own PR, maybe `--budget` chunks), which unblocks `rollup:src` + `system`. STYLE.md applies. Commit `docs/specs/` on its own.

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
