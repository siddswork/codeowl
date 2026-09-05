# CodeOwl — Roadmap (Working Draft)

> Companion to `ARCHITECTURE.md` (how) and `REQUIREMENTS.md` (what/for whom). This document is the *when* and *in what order* — a walking-skeleton sequence where each milestone is independently runnable, has a concrete pass/fail test, and adds real MVP value rather than being scaffolding for its own sake. Pending trims from `CLAUDE.md` (tool surface, commit hygiene) get resolved as they're hit, not decided up front.

## Test repos

Fixed set of real repos the milestone validations below run against — chosen for language coverage and for actually stressing specific open questions, not just for being famous. Not part of CodeOwl's own repo; live as siblings on disk.

| Repo | Path | Language | Size | Why this one |
|---|---|---|---|---|
| talentTrail | `~/dev/startup/talentTrail` | TypeScript/TSX | 307 files | The Phase 1 pilot repo itself (Next.js 16, Supabase, Upstash — matches `REQUIREMENTS.md`'s pilot description exactly). Primary target for M1–M7, M9. |
| memolink | `~/dev/openSource/inspirations/memolink` | Java (+ a little Python) | 45 Java files, Maven multi-module | The project CodeOwl is architecturally inspired by. Multi-module Maven build exercises build-manifest parsing beyond a single-module case. |
| leveldb | `~/dev/openSource/test-repos/leveldb` | C++ | 132 files, CMake | Stresses open question 1 directly — tree-sitter's weak spot on overloads/virtual dispatch. Real interface hierarchies (`Comparator`, `Iterator`, `WriteBatch::Handler`), well-documented enough to check generated specs against. |
| commons-lang | `~/dev/openSource/test-repos/commons-lang` | Java | 627 files, Maven | Plain classic Java, no framework magic — a second Java data point distinct from memolink's Spring-adjacent style. |

Only talentTrail is load-bearing for the Phase 1 milestones as written (M1–M9 all target it). The other three exist to catch language-specific extraction bugs early rather than discovering them only once Phase 2's polyglot ambitions are actually being built — worth a quick M1/M2 smoke pass against each once the TS pipeline works, even though the detailed milestone validations above are TS-specific.

## Sequencing principle

Each milestone below satisfies three things or it isn't a milestone:
1. **Runs on its own** — a CLI invocation or an MCP query you can actually execute, not "code exists but nothing calls it."
2. **Has a concrete validation**, not "add tests" — specific inputs (mostly against the pilot repo) and the exact output that means it worked.
3. **Adds value that compounds** — later milestones consume earlier ones' output; nothing here is throwaway scaffolding.

Sizing (S/M/L) is relative effort, not a time estimate — useful for sequencing decisions, not for a calendar.

---

## Phase 1 milestones

### M1 — Extraction walking skeleton
**Size:** M · **Builds on:** nothing

**Scope:** A CLI binary that walks the pilot repo's `.ts`/`.tsx` files, parses each with tree-sitter, and emits the `Symbol` record for every function/class/const — `id, kind, file, lines, signature, docstring` plus `parent`/`children` (containment tree only). Dump as JSON to stdout.

**Explicitly not in scope:** no import/call resolution, no hashing, no MCP, no persistence. One file in, its symbols out — that's the whole loop.

**Validation:** run against 5–10 hand-picked pilot-repo files spanning your hardest real cases — a React component with hooks, a barrel file, a class, a file with generics — and manually verify the emitted symbol list and containment tree against what you know is actually there. This is the milestone where tree-sitter's cursor API and the "don't let `Node<'a>` escape the parse function" trap (`CLAUDE.md`) actually get learned.

**MVP value:** proves the parser layer works on your actual pilot repo, not a toy example — the highest-uncertainty piece resolved first.

---

### M2 — Module resolution + caching backbone
**Size:** L · **Builds on:** M1

**Scope:** Three things that land together because they share the same graph data structure:
- Wire in `oxc_resolver` to turn each `import` into a real target `SymbolId` (path aliases, barrels, `exports` maps) — populates `imports`/reference edges.
- Move the graph into a proper arena (`Vec<Symbol>`, `SymbolId` indices — per `CLAUDE.md`) and persist it to `.codeowl/graph` (pick a serialization format here — `serde` + bincode or JSON, doesn't matter yet).
- Compute `source_hash` per symbol and propagate it up the containment tree (the Merkle-style aggregation from "Caching and invalidation"), and compute `interfaceHash` per exported symbol (from "Recursive spec generation" / the `Symbol` record).

**Explicitly not in scope:** no spec generation, still no MCP. Call resolution (which function calls which) can wait — only *import*-level (file-to-file) reference edges are needed for the reference-edge invalidation model.

**Validation, three separate checks:**
- *Resolution*: pick 5 files with known import graphs (a `@/lib/...` alias, a barrel re-export, a relative import) — assert each resolves to the correct target `SymbolId`.
- *Hash propagation*: edit one leaf function's body, re-run — assert exactly the expected ancestor chain's `source_hash` changed and nothing else did.
- *interfaceHash*: craft a before/after pair with a body-only change (hash must **not** move) and a signature change (hash **must** move) — this is the fixture that directly tests gap 2's fix.

**MVP value:** this is "the brain's" actual skeleton — the graph the rest of the system reads and writes now genuinely exists and is provably correct on the invalidation logic that gap 1 and gap 2 were about.

---

### M3 — MCP read surface
**Size:** S · **Builds on:** M2

**Scope:** Stand up the `rmcp` stdio server. Implement the pure-read tools: `get_symbol(id)`, `get_callers(id)`/`get_callees(id)` (now resolvable via M2's import edges), and `get_spec(id)` — which, since nothing has been generated yet, always returns the deterministic stub flagged `missing` (signature + docstring, no LLM). Add `search_code` as embedded ripgrep (the `grep` crate) — zero dependency on the graph, cheap to include here since the server now exists to expose it.

**Explicitly not in scope:** no generation — `get_next_spec_task`/`submit_spec` don't exist yet.

**Validation:** connect from Claude Code (or the MCP inspector) and query a handful of known symbols from M1/M2's fixtures — confirm the response shapes match, confirm `get_spec` on anything returns `missing` with a sane stub, confirm `search_code` finds a known string.

**MVP value:** **the first point CodeOwl is actually usable from an agent.** Even with zero specs generated, an agent can now query real structural facts about the repo instead of grepping blind — this alone is a testable slice of the value proposition.

---

### M4 — Generation loop
**Size:** M · **Builds on:** M3

**Scope:** `get_next_spec_task()` and `submit_spec(id, content)`, the spec file writer (mirrored `docs/specs/` tree, two-hash frontmatter — `source_hash`/`spec_hash`), and the `/codeowl generate <id>` command scoped to a single node (no `--all`/`--budget` yet).

**Explicitly not in scope:** staleness/regeneration logic — this milestone only covers the *first-ever* generation of something that's currently `missing`.

**Validation:** run `/codeowl generate` on one real file. Confirm: the spec lands at the correct mirrored path, frontmatter hashes are present and correct, `get_spec` on that id now returns the real spec instead of the stub, and re-running `generate` on the same id with source unchanged does **not** call the LLM again (source_hash match, per "Ordering" step 1).

**MVP value:** the actual deliverable — a real, LLM-written, cached spec exists and is queryable. This is the milestone where "the brain" produces its first thought.

---

### M5 — Staleness & invalidation end-to-end
**Size:** M · **Builds on:** M4

**Scope:** Wire up the parts of M2's hashing that M4 didn't yet exercise: `get_spec` on a stale node returns the last-known-good spec flagged `stale` with what changed (not just `missing`), and `/codeowl generate` on a stale node does a real cascade-bounded regeneration.

**Validation — this is the milestone that specifically proves gaps 1–3 work *together*, not just individually:**
- Generate specs for a small module tree (a file with 2–3 functions, contained in a directory with an `_index.md`).
- Edit one leaf function's body only → confirm exactly its file spec and the containing directory's `_index.md` go stale, nothing else.
- Edit a widely-imported utility's *signature* → confirm its direct importers' specs go stale (via `interfaceHash`) but nothing importing *those* importers does, and nothing containing the utility's unrelated siblings does.
- Edit the utility's implementation only (signature unchanged) → confirm **zero** importers go stale. This is the literal gap-2 regression test.

**MVP value:** the correctness properties the whole design exists to guarantee are now demonstrated on real code, not just argued in a doc.

---

### M6 — Completeness & correction mechanics
**Size:** M · **Builds on:** M5

**Scope:** `get_spec_coverage(scope?)`, `/codeowl generate --all` and `--all --budget=N` (fan-in-ordered), and the full four-case human-correction reconciliation from "Human corrections."

**Validation:**
- Run `get_spec_coverage` on the pilot repo pre-generation — confirm it reports the right missing/stale/current breakdown.
- Run `--all --budget=10` — confirm exactly ≤10 generations happen, and that a crafted fixture with known fan-in differences (one file imported by 5 others, one imported by none) gets the high-fan-in one generated first.
- Hand-edit a generated spec file with source unchanged → confirm a subsequent `generate` leaves it untouched and updates `spec_hash` to match (case 3). Then change the underlying source too → confirm a reconciliation regeneration that preserves the human correction where still accurate (case 4).

**MVP value:** the design's full feature set for Phase 1 is now built — this is functionally "Phase 1 complete" from a capabilities standpoint, before the incremental/live-session and validation milestones.

---

### M7 — Incremental indexing (live sessions)
**Size:** S · **Builds on:** M6

**Scope:** `notify`-based file watcher for the remainder of an MCP server session, plus the fresh-spawn catch-up pass (hash-check everything against `.codeowl/` on startup).

**Validation:** start the server, edit a file while it's running, confirm a subsequent `get_spec` on an affected node reflects the change without restarting the server. Kill the server, edit more files, restart, confirm the catch-up pass reindexes exactly the changed files.

**MVP value:** turns CodeOwl from "a tool you re-run" into something that behaves like a language server across a real multi-hour coding session — the actual target interaction model from `REQUIREMENTS.md`.

---

### M8 — SQL/schema boundary resolution
**Size:** M · **Builds on:** M2 (graph), independent of M3–M7

**Scope:** The dedicated SQL DDL extractor (schema nodes for tables/columns/constraints) plus the fuzzy string/ORM-aware matcher on the application-code side, per open question 3's second bullet in `ARCHITECTURE.md`. Called out there as "directly relevant to the Phase 1 pilot… not a future concern" — the pilot repo's Supabase migrations are real, not hypothetical.

**Explicitly not in scope:** the other two polyglot sub-problems from that open question (call/invocation boundaries, standalone "island" nodes) — genuinely lower-stakes for this pilot, can stay deferred past Phase 1.

**Validation:** point it at the pilot repo's actual migration files, confirm schema nodes are created for known tables, and confirm at least one known ORM/string-literal reference in application code resolves to the right schema node (fuzzy match, so "resolves to a plausible candidate" is the bar, not exact precision).

**MVP value:** without this, any exit-criterion task touching schema-coupled code (exactly the kind of task the deleted exp-01 scaffolding picked, for good reason) gets an unfairly degraded test — schema nodes would just be invisible. This milestone is what makes M9 a fair test rather than an easy one.

---

### M9 — Exit criterion validation
**Size:** S (mechanically) · **Builds on:** M6 and M8

**Scope:** The thing Phase 1 actually exists to answer. Populate `docs/specs/` for a meaningful slice of the pilot repo (via M6's `--budget=N`), add the `CLAUDE.md` line telling an agent to check specs first, then run several real tasks with and without CodeOwl available and compare — using `utility/mine.py`, already built, for the token/exploration-tool measurement.

**Validation:** this milestone *is* the validation — the deliverable is a verdict, not code. Concretely: for each task, does `mine.py`'s exploration-tool count and token estimate drop meaningfully with specs available, and does the agent actually use `get_spec` rather than defaulting to `Read`/`Grep` (the H1 gate from the deleted exp-01 scaffolding's framing — worth reusing that hypothesis table even though the manual pre-code version isn't being run).

**MVP value:** this is the actual Phase 1 exit gate. A "no" here is a valid, useful outcome — it means stopping before any Phase 2 investment, which is the entire reason Phase 1 was scoped this way.

---

## Phase 2 — sketch only, revisit after M9

Deliberately coarse — detailed planning here is premature until M9 answers whether Phase 2 happens at all. Rough shape, in likely order:

1. **HTTP/SSE transport** — `rmcp` over HTTP instead of stdio, repo-scoped tool calls.
2. **`tantivy` + ONNX embeddings** — the real search index deferred out of Phase 1 (see `ARCHITECTURE.md` "Storage"), now justified by multi-user load.
3. **Multi-repo namespacing** — per-repo index/graph/spec-cache within one shared process (`REQUIREMENTS.md` "Hosting granularity").
4. **Stub nodes + cross-team delegation** — the cross-repo dependency model (`REQUIREMENTS.md` "Multi-repo & team ownership").
5. **Web viewer** — the Cytoscape.js-style graph browser for BAs/QA/SREs.
6. **Auth/roles** — reopens once multiple users share one hosted instance (`REQUIREMENTS.md` open question 2).

## Provisional decisions this ordering makes

Worth flagging since I made these calls rather than asking:
- **M8 (SQL) is sequenced before M9**, not folded into M4–M7's core TS pipeline. If you'd rather ship M9 sooner and pick a validation task that avoids schema-coupled code, M8 can move after M9 or drop out of Phase 1 entirely — it's the one milestone here that isn't strictly load-bearing for the others.
- **`CLAUDE.md`'s two pending items** (tool-surface trimming, spec-regen commit hygiene) aren't given their own milestones — the plan resolves the tool-surface one implicitly (M3–M6 only ever build the tools actually used: `get_spec`, `get_symbol`, `get_callers`/`get_callees`, `search_code`, the three generation tools, `get_spec_coverage` — `trace_path`/`get_tests_for`/`get_dependencies` simply never get built unless a later milestone needs them), and commit hygiene is a workflow habit to adopt once M6 exists, not something to build.
