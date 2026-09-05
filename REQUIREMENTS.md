# CodeOwl — Requirements (Working Draft)

> Status: early draft, being resolved incrementally through discussion. Companion to `ARCHITECTURE.md`, which covers design and components once these requirements are settled — this document covers what we're building and for whom, and the scope decisions that shape it.

## Problem statement

Large brownfield orgs (multi-language: C++, Java, Python, etc.) lack a shared, accurate, up-to-date source of truth about how their codebase actually works. Teams (devs, BAs, QA, SREs, DevOps) and their LLM coding agents (VS Code, Kiro, Claude Code) each re-derive this understanding independently and repeatedly, burning tokens and time.

## Goal

A single shared "brain" — a structural + semantic knowledge graph extracted from the codebase — exposed via MCP so any IDE/CLI agent can query precise facts instead of re-exploring the repo, and via a web UI so non-technical/non-IDE team members (BAs, QA, SREs) can browse and search it too.

Secondary/future goal: ingest personal notes (currently in OneNote, to be exported as `.md`) into the same vault-style pipeline once the CodeOwl pipeline is proven.

## Phasing

The project is deliberately built in two phases. Phase 1 proves the core idea (does recursive, graph-scoped spec generation actually produce something meaningful and token-saving) at zero infra cost, before any investment is made in the org-scale version.

### Phase 1 — Personal, local-only proof of concept

- **Who**: just the author, on personal mono repos.
- **LLM access**: covered entirely by an existing Claude Code subscription — no separate privacy concern, since the author is already comfortable sharing this code with Claude.
- **Interaction model**: a local MCP server, stdio-based (same default pattern as MemoLink), invoked from Claude Code. No hosted service, no network-facing component.
- **Spec generation, key design point**: CodeOwl itself never calls an LLM API or holds credentials. It builds the dependency graph and a generation plan (what needs a spec, in what order, with what scoped context), and hands each unit off — via MCP tool calls — to whichever coding agent is already in the session (Claude Code) to actually write the spec text. CodeOwl's job is orchestration, not generation.
- **Storage**: specs are committed to git — valuable, human-reviewable, and diffable in a PR. The dependency graph is treated as a derived, local, gitignored cache — cheap to rebuild from source + specs, not worth committing. No search index and no embeddings exist in Phase 1 at all: `search_code` is plain-text ripgrep, and a real `tantivy`/ONNX-backed index is deferred to Phase 2, since neither is on the path to Phase 1's exit criterion — see `ARCHITECTURE.md`'s "Storage".
- **Caching/invalidation**: no separate cache database. Each spec file carries two hashes as frontmatter — the source hash it was generated from, and a hash of its own generated body — so regeneration is a simple comparison, and a human edit to the spec itself is distinguishable from a source change. See `ARCHITECTURE.md`'s "Human corrections — reconciliation, not overwrite": a hand-corrected spec is reconciled with new source on the next regeneration, not silently overwritten by it.
- **Indexing trigger**: on a fresh MCP server spawn, a catch-up pass hash-checks and reparses whatever changed while no process was running; for the remainder of that session, an in-process file watcher reindexes incrementally in real time. No git webhook — there's no shared canonical index to keep in sync.
- **Spec content priority**: written for Devs, QA, DevOps, and LLM agents — precise and technical (signatures, side effects, error cases, dependencies) rather than plain-business-language framing. BAs aren't a primary consumer until Phase 2.
- **Staleness policy**: reading and generating are separate acts, not one implicit step. A query (`get_spec`) against a stale or missing node returns immediately — the last-known-good spec flagged `stale` (with what changed) or a deterministic stub flagged `missing` — and never itself triggers regeneration. Generation only happens via the explicit `/codeowl generate` command (scoped to one id, `--all` for full coverage, or `--all --budget=N` to spend a capped amount per run as an incremental push toward completeness — see `ARCHITECTURE.md`'s "Pushing toward completeness"), so it's never silently hijacking whatever task the calling agent is actually mid-way through. The cascade cap still applies, now to a `generate` run rather than a query: a hard limit on how many nodes one invocation will regenerate before stopping and reporting instead of continuing unbounded.
- **MCP server lifecycle**: not something the user starts or stops manually. Claude Code (the client) spawns it as a subprocess when a session begins and kills it when the session ends — the same model as a language server. No standalone background service to manage.
- **Runtime environment**: the server runs wherever the developer's actual toolchain lives, not tied to any one OS — WSL for a Claude Code-in-WSL setup, inside a devcontainer/DevBox for VS Code or Kiro configured that way, or directly on native Windows for a developer (e.g. a Java dev) who works there without a Linux layer at all. This was the primary constraint on CodeOwl's own implementation stack: it must run cross-platform — native Windows, Linux (WSL/devcontainer), and macOS — not Linux-only. **Resolved: CodeOwl is written in Rust**, which serves this requirement best (one self-contained binary per platform, no runtime to install, no native-addon builds on Windows) — see `ARCHITECTURE.md`'s "Implementation stack".
- **Auth/roles**: not needed — single user, single machine.
- **Local-uncommitted-change handling**: not a separate problem to solve. The tool reads the actual working directory directly, so whatever's on disk (committed or not) is exactly what's reflected — there's no separate canonical index to be stale relative to.
- **Exit criteria**: specs are actually meaningful (useful to a human, useful to an agent) and demonstrably reduce token usage/re-exploration before any Phase 2 investment is made.

### Phase 2 — Org rollout (the larger win)

- **Who**: the full team — devs, BAs, QA, SREs, DevOps — using VS Code and Kiro, not just Claude Code.
- **Interaction model**: CodeOwl needs to be consumable as a VS Code/Kiro extension or MCP setup in those IDEs — the same interaction pattern proven in Phase 1 (an IDE-resident agent talking to CodeOwl over MCP), just extended to more IDEs.
- **LLM access**: the org has multiple provider contracts (Claude, GPT, Gemini). CodeOwl still never manages credentials or picks a provider — generation is still delegated to whichever IDE agent/LLM the developer already has authorized. The "which LLM" decision stays with the org's existing tooling, not with CodeOwl.
- **Everything under "Scope decisions" below** (multi-repo/team ownership, hosting granularity) applies from this phase onward.
- **Local-uncommitted-change overlay and auth/roles** (see Open questions) become real decisions again here, once there's a shared, hosted, canonical index that is genuinely separate from any one developer's own checkout.

## Users / consumers

Fully applicable from Phase 2 onward (Phase 1's only user is the author):

- **Developers** — via VS Code, Kiro, or Claude Code, both directly and through their LLM coding agent.
- **Business Analysts** — via the web viewer, asking what a service/module does in plain terms.
- **QA** — tracing what a change actually touches, what tests cover a symbol.
- **SREs / DevOps** — dependency graphs, especially cross-service ones, for impact analysis and incident response.
- **LLM coding agents themselves** — a first-class consumer, not just a proxy for a human: agents query CodeOwl instead of re-exploring the repo from scratch, which is the primary source of the token savings this project is meant to deliver.

## Scope decisions (resolved) — Phase 1

### Pilot target

- The first repo CodeOwl is built against: a personal Next.js 16 (App Router) / React 19 / TypeScript application. UI via Tailwind v4, Radix, shadcn-style components; backend via Supabase (Postgres) and Upstash Redis; NextAuth v5 for auth; Razorpay, Cloudinary, Resend/Nodemailer as third-party integrations; Vitest/Playwright for testing; deployed on Vercel.
- This resolves the "which language first" question: TypeScript/TSX is the primary source language, via tree-sitter's existing grammars. It is not a single-artifact-type repo, though — a Supabase-backed app almost certainly carries SQL migration/DDL files alongside the TypeScript source, possibly shell scripts too. This pilot is a real, immediate case of the polyglot/non-code-artifact open question in `ARCHITECTURE.md`, not a deferred one.
- **This is the target repo's tech stack — i.e. what CodeOwl parses and analyzes.** It is a separate, independent decision from what CodeOwl **itself** is implemented in, and the two deliberately differ: CodeOwl is written in **Rust** (see `ARCHITECTURE.md`'s "Implementation stack") while its first parse target is TypeScript. Nothing about the pilot being TypeScript implies the host language — tree-sitter, MCP SDKs, and ONNX runtimes all have bindings in multiple languages.

### Parsing vs. generation cost policy

- Parsing (extraction + graph resolution) always runs **eagerly** — it's CPU-only, cheap, and a prerequisite: even a single on-demand spec request needs the graph already resolved. Not a user-facing setting.
- Spec generation defaults to **lazy in scope** — nothing is pre-generated across the whole repo on its own — but is always **explicit in trigger**: a query (`get_spec`) never produces a spec as a side effect, only the `/codeowl generate` command does (see "Staleness policy" above and `ARCHITECTURE.md`'s "Ordering"). This isn't asked of the user either (there's no basis for a new user to answer it correctly); it's just the default behavior.
- `/codeowl generate` covers three cases with the same underlying mechanism, just driven in a loop instead of on-demand: scoped to one id (fill in exactly what's missing/stale under it), `--all` (full eager coverage, to exhaustion), or `--all --budget=N` (an incremental, capped push toward full coverage — run periodically instead of committing to one full sweep). See `get_spec_coverage`, `get_next_spec_task`, and "Pushing toward completeness" in `ARCHITECTURE.md`.

### Spec file location and granularity

- **Location**: a mirrored tree (`docs/specs/`), not sidecar files next to source. Sidecar works well for a clean 1:1 pairing (a test file per source file), but specs exist at multiple hierarchy levels (function, file, submodule, module, system) with no single natural sidecar for each — trying to force that would clutter `src/` far more than a test-file sidecar ever does. A separate mirrored tree also keeps source-directory listings, `git status`, and IDE fuzzy-search clean of generated artifacts, and gives PR reviewers a clean, separate "what specs changed" diff.
- **Granularity**: physical files exist at file-level and above only — never one file per function/symbol. Leaf-level (function/class) specs are collapsed into *sections within* their containing file's spec document, so physical file count stays roughly 1:1 with source files, not with every symbol in them.
- **Structure**:
  - `docs/specs/<mirrored-path>/<filename>.md` — file-level spec, containing a file-level summary plus one section per significant function/class in that source file.
  - `docs/specs/<mirrored-path>/_index.md` — the submodule/directory-level rollup.
  - `docs/specs/_index.md` — the system-level spec, at the root.
- **Frontmatter**: each spec file records the source path(s) it covers, the source hash it was generated from, and a hash of its own generated body — the pair used both for cache invalidation and for detecting human edits, so a correction is reconciled on the next regeneration rather than silently overwritten (see `ARCHITECTURE.md`, "Recursive spec generation" → "Human corrections").

## Scope decisions (resolved) — Phase 2

### Multi-repo & team ownership

- One graph per repo, owned by that repo's team — not a single merged graph across repos.
- Cross-repo dependencies are represented as coarse-grained **stub nodes** derived from API contracts (OpenAPI specs, protobuf/gRPC definitions, message queue topics, client SDK usage) rather than inlined, function-level call graphs — function-level depth into another team's internals is neither reliably available nor desirable across a team boundary.
- Deep traversal past a stub node **delegates** to the target repo/team's own CodeOwl instance over the same MCP protocol, rather than merging graphs together.
- A discovery/orchestration layer that lets any team query any other team's graph automatically is explicitly **out of scope for v1** (see Non-goals) — it needs org-wide service discovery and cross-team auth/ACL that aren't required to prove value within one team first.

### Hosting granularity

- One CodeOwl **instance per team**, not per repo and not per org.
- A team's instance hosts multiple repos' graphs as independently versioned/invalidated namespaces within the same running service — one shared embedding model and server process, but a separate index/spec-cache per repo, each triggered by its own repo's git webhook.
- Same-team, cross-repo dependencies resolve at full fidelity **in-process** (the instance already holds both graphs locally) — the stub/delegate pattern above is specifically for crossing into a *different* team's instance.
- Scaling lever before ever splitting a team's instance: push individual repos into the disk-backed storage mode as they grow. Splitting one team's workload across multiple instances is a last resort driven by actual resource limits, not a fixed repo-count cap.

## Open requirements questions (unresolved)

All remaining open questions are Phase 2 — Phase 1 has none outstanding right now.

1. **Local uncommitted-change overlay** — re-opens once there's a shared canonical index separate from a developer's own checkout: should it reflect a developer's in-progress branch changes, or only the canonical index built from `main`/trunk?
2. **Auth/roles** — needed once multiple users/clients share one hosted team instance, given proprietary code is involved. At minimum needs a coarse decision (e.g. API-key per client, similar to MemoLink's model).

## Non-goals

- **A hosted/shared instance in Phase 1** — deliberately deferred. Phase 1 runs entirely on one laptop; hosting is only taken on once Phase 1 proves the approach's value.
- **Automatic cross-team graph discovery/orchestration** — deferred pending the stub-node pattern proving out within a team first; not ruled out forever, just out of scope for v1 (see Scope decisions above).
- **Editing or writing back to the codebase** — CodeOwl is read-oriented with respect to source code: it answers questions about code and does not modify it. (This doesn't cover CodeOwl's own generated spec files, which the system does write and update by design — see "Recursive spec generation" in `ARCHITECTURE.md`.)
- **OneNote ingestion** — a secondary/future use case, deferred until the primary code-spec pipeline is proven.
