---
kind: system
modules:
  src: 7e38f8ade33c6acc84093f7d07d34c6ae1d144b48a3d69d52d8910731a949716
features:
spec_hash: c2dc982c015c196211492e474534dc44ecde9c9712690508f7641761d12b405f
---
# CodeOwl

CodeOwl is a developer tool that extracts a structural graph from a codebase — every function, class, database table, and how they reference each other — and serves both that graph and an LLM-authored "semantic layer" of specs (prose explanations of what things do) to a connected AI coding assistant over MCP (Model Context Protocol). It never writes prose itself or holds any LLM credentials: it assembles context and persists whatever the calling assistant writes, driven through a small, well-defined generate loop. It supports several languages/frameworks (TypeScript + Next.js, Rust, Java, Python + FastAPI) through a common pluggable interface, so the same structural and spec machinery works across a polyglot set of codebases without each one needing its own bespoke tooling.

## Key flows

**Cold-start indexing.** When CodeOwl is pointed at a repo for the first time, it detects which single supported language stack the repo is written in, walks every source file, and asks that stack to parse each one into symbols, imports, and any framework-specific "flow edges" (looser links like a URL string or a rendered UI component a plain import graph can't see). It builds the in-memory graph from that, resolves every import and flow edge against it, and persists both the graph and the raw per-file inputs to an on-disk cache so a later run doesn't need to re-parse everything from scratch.

**Live editing during a session.** While `codeowl serve` is running, a background file watcher notices edits, debounces a burst of saves into one update, and feeds the changed paths back through the same extraction-and-resolution path — but only for the files that actually changed. The freshly rebuilt graph is swapped into a lock-free cell the MCP server reads from, so a long-running session always answers from current code without needing a restart.

**Spec generation.** A connected AI assistant drives generation itself, one step at a time: it asks for the next thing that needs writing (a specific symbol, a file's own summary, a whole feature, a directory rollup, or the top-level system document), reads the exact context handed back, writes the prose, and submits it. Generation recurses bottom-up — a file's symbols before the file's own summary, every file in a directory before that directory's rollup, every module and feature before the one system spec — so a higher-level document is always composed from already-written lower-level summaries rather than reading raw source itself.

**Staleness tracking and reconciliation.** Every generated spec is pinned to layered content hashes: the underlying code's own text, the shape of what it depends on, and the written prose itself. Comparing these against freshly computed values is what lets CodeOwl tell a caller precisely what changed — source drift versus a dependency's shape changing versus a human having hand-edited the spec file directly, in which case it silently re-stamps the hash to accept the correction rather than discarding it or asking for it to be rewritten.

**Feature recognition.** For a stack with a runtime entry surface (a web framework), CodeOwl recognizes an end-to-end capability — a route plus everything it reaches — using framework-specific conventions (a route decorator, a `fetch()` call, a database query naming a table by string) layered on top of the otherwise generic graph walk. This lets a feature spec describe a whole flow across several files without the underlying import-resolution or hashing machinery needing to know anything framework-specific itself.

**Structural queries.** Independent of the spec layer, an assistant can ask direct structural questions at any time — what a symbol's full record looks like, what references it, what its containing file imports, or a plain regex search across the repo's source. These are always pure reads off the live graph and never trigger any generation on their own.</content>

## Modules
- `src` — This directory is CodeOwl's entire Rust implementation. At its core is a language-agnostic structural graph (`graph.rs`, `symbol.rs`, `index.rs`) that every supported language "stack" feeds into through one shared interface (`stack.rs`): TypeScript/Next.js (`extract.rs`, `imports.rs`, `resolve.rs`, `lang.rs`), Rust (`rust.rs`), Java (`java.rs`), and Python/FastAPI (`python.rs`, `fastapi.rs`) each parse their own language's syntax and resolve their own import conventions into the same shared symbol/containment vocabulary, so downstream tools never need to know which language they're looking at. On top of that graph sits a "feature" layer (`features.rs`) that recognizes end-to-end capabilities — a web route plus everything it reaches — using framework-specific rules (route decorators, `fetch()` calls, database queries) layered over the generic graph, plus a SQL schema extractor (`schema.rs`) so a database table can be a first-class node too.

## Features
