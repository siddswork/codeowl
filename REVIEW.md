# CodeOwl — Design Review (2026-09-05)

> Working notes from a second-pair-of-eyes review of `REQUIREMENTS.md` and `ARCHITECTURE.md`, to be resolved one item at a time before implementation starts.

## What's genuinely strong

- **"Orchestrate, don't generate"** — the best call in the whole design. It kills credentials, provider choice, cost, and privacy debates in one move, and makes Phase 1 free. Keep this no matter what.
- **Specs in git, graph as a gitignored cache** — right split. Specs are the valuable, reviewable thing; the graph is reproducible.
- **`spec_hash` for human-edit reconciliation** — the four-case table (ARCHITECTURE.md "Human corrections") is the kind of thing you only think of after being burned. Good.
- **Phase discipline** — Phase 2 is well-parked. Just make sure it doesn't leak into Phase 1 code (per-repo namespacing, stub nodes, etc.).

## The gaps

### 1. The dependency graph is not a tree, and the design treats it like one

`get_spec(id)` "recursively ensures direct dependencies have current specs first" (ARCHITECTURE.md, "Recursive spec generation" step 2). In a Next.js app, nearly every file imports `lib/utils`, `components/ui/*`, and a few hooks. So:

- **Cold start**: the *first* `get_spec` on any page component transitively pulls in most of the repo. You hit the cascade cap immediately, the agent is "told" — and then what? Lazy generation degenerates into batch generation on the first query.
- **Cycles**: TS codebases have import cycles constantly (barrel files, mutual type imports). The recursion as written doesn't terminate. You need SCC detection or an explicit cut rule — it's not mentioned anywhere.
- **Two structures are conflated**: the *containment* hierarchy (symbol → file → dir → system) is a tree; the *import/call* graph is a cyclic DAG. Bottom-up composition works cleanly on the first; it's what breaks on the second.

**Suggested fix**: a spec consumes only its *containment* children's specs (real bottom-up) and, for import dependencies, a **deterministic, LLM-free stub** from the graph — signature + one-line docstring + the dep's spec *if it happens to exist*, else nothing. Never recurse across an import edge to generate. That makes cold start O(1) LLM calls per query and makes cycles irrelevant.

### 2. Invalidation as written will make the whole repo permanently stale

Parent specs are keyed on "the hashes of the child specs it consumed" (ARCHITECTURE.md, "Caching and invalidation"). LLM output is non-deterministic, so a regenerated child spec is *textually* different almost every time → every parent goes stale → cascade → cap → stale forever. A one-line change to `cn()` in `lib/utils.ts` invalidates the entire graph.

**Fix**: key parents on dependencies' **interface hash** (exported signatures/types, computed deterministically from the graph), not their spec body. A parent only goes stale when a dependency's *public surface* changes, not its implementation or its prose. This one change is probably the difference between the system working and not.

### 3. "Silent regeneration" contradicts the mechanism

REQUIREMENTS.md ("Staleness policy") says a stale spec triggers *silent* regeneration before answering. But the mechanism (ARCHITECTURE.md, "Who actually writes the spec text") is that the *calling agent* runs a `get_next_spec_task → write → submit_spec` loop. That's not silent — the agent is mid-task for the user, and its context gets hijacked into a generation loop. In practice Claude Code will either skip it or derail.

**Fix**: `get_spec` on a stale node returns the stale spec immediately, flagged with *what changed* (the source diff summary). Generation is a separate, explicit act — a `/codeowl` command, a Claude Code hook, or a sub-agent. Decouple reading from writing.

### 4. The biggest risk isn't the architecture — it's that the core hypothesis is untested

The exit criterion is "specs demonstrably reduce token usage/re-exploration." Nothing in the plan says how you'd measure that, and you're about to build a parser + graph resolver + MCP server + hash cache before finding out.

**You can test the hypothesis in a day with zero code**: have Claude Code hand-write specs for ~15 files of the pilot repo into `docs/specs/`, add a CLAUDE.md line saying "check docs/specs before exploring," then run 5 real tasks with and without it and compare tokens + outcome. If agents ignore the specs or they don't help, you've saved yourself months. If they do help, you also learn *what a useful spec looks like* — which is currently undefined and is the actual product.

## Smaller things

- **Stack**: kept open, but the pilot being TS is a strong signal. Import resolution in TS (path aliases, barrels, re-exports, Next.js file conventions) is painful on tree-sitter alone; the TypeScript compiler API gives it to you for free. Node/TS also has the reference MCP SDK, tree-sitter bindings, and onnxruntime-node. Pick it and keep the extractor interface pluggable.
- **Lucene + ONNX embeddings in Phase 1 is YAGNI** — and Lucene is JVM, which conflicts with the point above. `search_code` can be ripgrep for Phase 1. Semantic search isn't on the path to proving the hypothesis.
- **Tool surface**: 12 tools is a lot for Phase 1. Minimum viable is `get_spec`, `get_next_spec_task`, `submit_spec`, `get_spec_coverage`, `get_callers`/`get_callees`. Cut `trace_path`, `get_tests_for`, `get_dependencies`, `search_code` until something needs them.
- **PR noise**: silent regeneration + specs-in-git means every feature PR drags spec diffs along. Consider specs regenerated as a separate commit or a pre-PR step.
- **Doc drift**: ARCHITECTURE.md's Storage section says file layout is "still open — see REQUIREMENTS.md open question #1," but REQUIREMENTS resolved it (`docs/specs/` mirror) and has no Phase 1 open questions.

## Recommendation

Do the manual experiment first (#4). Then, before writing code, revise the "Recursive spec generation" section of ARCHITECTURE.md around the containment-vs-import split and interface-hash invalidation (#1, #2), and split read from generate (#3). Those three edits are small in the doc but they're the difference between a system that stays fresh and one that's stale from day one.
