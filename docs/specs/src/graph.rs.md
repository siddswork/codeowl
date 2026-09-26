---
kind: file
source_paths: [src/graph.rs]
file: { source_hash: 2dd78982dd0e6b18b620c29aa03aa0451e50adf16884c1a4e7462e1849a59504, deps_hash: 68bd5c6fdb871e097e8f32c4104ee86cf82fa1357ba42fe4f09949114805ab3f, spec_hash: d87ce3d9b4ca17796c36b9cd14484f662ab8103cc16711f657d15296bdcfbd16 }
symbols:
  src/graph.rs::SymbolView: { source_hash: ca3259f0c9f3c6641e2a79400cc56590870eabe278c316c5c6f01767376629b5, deps_hash: b83160b92c2fec79f3d97a822ad50ca9a2484bae2a019246166efc86c573959c, spec_hash: beba6f15b9ac1762f07aa71a52346bfef92032462eda50e9bdc0f71561d38a0a }
  src/graph.rs::FileNode: { source_hash: 1cb0e932df0ba6250e1e2f9a72576a667d908f5f70ebeb405ae82f825d777545, deps_hash: 6bfd686bea3a7eadb25bb4b6dadfaf6a93bea0e28872854d90268fc7b7daf993, spec_hash: 877ca36ce0292b88e271e61bdae192c78b9d8e062aba84a528cb27730c228825 }
  src/graph.rs::Node: { source_hash: d54a33462ad83d4a30a907cbd652600809f5bb2bbda1968951e52c3e967f45c5, deps_hash: 3f020ad9a958ab8d579a2087acfde4debe848a98ea126621dd23f3e3f3927e1e, spec_hash: 6b6bdd1226e3cbde9b515bd93512390bb2308facecff20d925ea2eb50d470306 }
  src/graph.rs::FileExtraction: { source_hash: ed79c4363575bcf427c97838ad5d310bf8e2724ed086fa6baaed390d8dbcf84c, deps_hash: 57b36ccf02abf241d59cf027b8af996e4e37cdf048813be00047a453efba7ce6, spec_hash: 1e80f3f5183e512702c2b4dfd716f9d7beba47a5bd9e627eaa1c142c63ef08c6 }
  src/graph.rs::extract_and_hash: { source_hash: 90b73a4ee99da81bd1bf071b59a555179cedb73e224d9146186258778cd78f72, deps_hash: d7da40bd5b91baea3db803ad745a18c4adf9c2308a97b126dd894920bb84e365, spec_hash: ca69d203ed27f2cbe8c29e1be937dcfaaca88cc0686d9b93907d6f5ef1bdc4a5 }
  src/graph.rs::FlowEdge: { source_hash: 8303f3f670978cf87fcfda541c910676baa4e577f2929431a880fe0c41551c36, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: d7756de2ae6c9cf6f564e63f12ce6a4b80f68f70018f3c8f6069cb1c3cd591d9 }
  src/graph.rs::FlowTarget: { source_hash: 181f53cda8573f83b630a41747ca62f48694931c538e4c8e6f762ca4845088dc, deps_hash: ac583dfc44b8c03e1d77dc3191f47c59b6bb6a7b182fa0031e169bb47d564290, spec_hash: b2752135ad033bc9d3fef9a4115ce5e76bcee552b14e69d9521f90bb745de560 }
  src/graph.rs::UnresolvedFlowEdge: { source_hash: 247d5a418c02e679b95f2a79f6b153fbe044257e740deca6380e82514abcb422, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 1b08108bf64809c00a1e59025885a05382ab85ded5b10172af966abafa84f9fe }
  src/graph.rs::Graph: { source_hash: 7abe8b3fd08b27724d4368093b04567ce9e68d6391d3f510179b4e9c533208ba, deps_hash: a9a1e579a0271250d774cce7b96ef742a85f88d6d15abff09c545cfc85e411cd, spec_hash: 72a25844b9a6d031c4b284d678211b6bc00a60c0fcf61d6954d72db101bcfa5b }
  src/graph.rs::build_graph_from_sources: { source_hash: dd5e3e0c36327ad645a76af4a2c9fd5bb8cbe4f36f51ed9217dc8a15a1d80028, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 55df20b1897ceed56015a0ace1c249b8db22646e47caf5beeb18422d4b6caa47 }
---
# src/graph.rs
## Summary
This file defines CodeOwl's graph — the arena (one flat list holding every node, referenced by position rather than by pointer) that turns per-file extraction output into something indexable and cross-referenceable. `extract.rs` only ever sees one file at a time and produces symbols with plain string containment; `Graph::build` is the one place that turns a whole repo's worth of those files into the real arena, assigning every file and every declaration a stable `SymbolId` and resolving all containment (parent/children) and cross-file references (imports, and the extra "flow edges" a pack finds beyond plain imports, like a `fetch("/api/…")` or a `.from("table")` query) against it. `SymbolView` is the external-safe copy of a declaration used whenever one needs to cross the process boundary — an MCP response, a spec's frontmatter — since a `SymbolId` itself is only meaningful within the graph that produced it. The graph also persists itself as plain JSON to `.codeowl/graph`, and every other module in this codebase reads its data through this file's lookup functions rather than walking the arena directly.

## `SymbolView`
`pub struct SymbolView`
### Summary
The "safe to hand outside this process" version of a `Symbol` — same information, but with every internal `SymbolId` translated back to a stable string id first. It's what an MCP response, `codeowl extract`'s JSON output, or a spec file's frontmatter actually serializes.
### Behavior
`SymbolView` mirrors `Symbol` field-for-field, except `parent`/`children` are `String`/`Vec<String>` instead of `SymbolId`/`Vec<SymbolId>`. That substitution exists because a `SymbolId` is only a valid pointer within the specific `Graph` that produced it (see its own spec) — it must never leak into output a caller might hold onto after this process exits, since the next run's graph could assign that same numeric id to a completely different node.

`from_graph(graph, id)` builds one: it looks the symbol up by `SymbolId`, returns `None` if that id doesn't resolve in this graph, and otherwise clones every plain field across as-is while converting `parent`'s `SymbolId` (and each entry in `children`) to its string id via `graph.string_id(...)`. Every place a `Symbol` needs to cross the process boundary should build a `SymbolView` this way rather than serializing a `Symbol` directly.
### Depends on
- `src/symbol.rs::SymbolKind` — crate::symbol
- `src/symbol.rs::SymbolId` — crate::symbol

## `FileNode`
`pub struct FileNode`
### Summary
A file's own entry in the graph's arena (the one flat list holding every node) — sitting alongside `Symbol` nodes, so a file can be pointed at by `SymbolId` and hold `children` just like any other container.
### Behavior
`id` is the file's repo-relative path, using the same string scheme `Symbol::file` uses. `children` lists the file's top-level symbols, in declaration order, by `SymbolId`.

`source_hash` is a hash of the file's *raw text*, not a Merkle-style rollup of its children's hashes the way a class folds in its methods. That's deliberate: a file's spec-relevant content — its imports, type annotations, comments outside any symbol's span — isn't fully captured by its declared symbols alone, and hashing the whole file's text is both simpler and strictly more sensitive to change than trying to reconstruct that from the children.
### Depends on
- `src/symbol.rs::Symbol` — crate::symbol
- `src/symbol.rs::SymbolId` — crate::symbol

## `Node`
`pub enum Node`
### Summary
One entry in the graph's arena — the two-way union of everything the containment tree can hold: either a declaration (`Symbol`) or a file (`FileNode`).
### Behavior
`Node` is a plain enum over the two possibilities. `Graph::build` is the only place `Node` values get constructed, when it assembles the arena from a repo's extracted files; everything else reads them back out through `get_symbol`/`get_file` (which unwrap to the specific variant) or the more general `get`, used when the caller genuinely doesn't care which kind it is — hashing code that just needs *a* node's `source_hash`, for instance.

`string_id()` returns the node's stable string id regardless of variant — `Symbol::id` for a `Symbol`, `FileNode::id` for a file — so code that only needs the id (not the rest of the node) doesn't have to match on the variant itself.
### Depends on
- `src/symbol.rs::Symbol` — crate::symbol

## `FileExtraction`
`pub struct FileExtraction`
### Summary
The result of walking one file, before that file becomes part of the graph — everything `Graph::build` needs to turn it into a `FileNode` plus its `Symbol` children.
### Behavior
`rel_path` is the file's repo-relative path. `source_hash` is a hash of the file's raw text, computed once here rather than re-read later — it becomes the resulting `FileNode`'s own `source_hash` unchanged. `symbols` holds every top-level declaration a pack's extractor found in the file, still in `ExtractedSymbol` form (string-based containment, no `SymbolId` yet, since there's no arena to assign positions from at this stage). `Graph::build` consumes one `FileExtraction` per file to produce that file's `FileNode` and resolve its symbols into real arena `Symbol`s.
### Depends on
- `src/symbol.rs::ExtractedSymbol` — crate::symbol

## `extract_and_hash`
`pub fn extract_and_hash(rel_path: &str, source: &str) -> FileExtraction`
### Summary
A test-only helper that reads a single file's source text and produces both its extracted symbols (the functions, classes, tables, etc. CodeOwl found in it) and a content hash in one call, so test fixtures don't have to do those two steps separately.
### Behavior
Dispatches purely on file extension: a `.sql` path goes through `schema::extract_tables` (pulls out `CREATE TABLE` definitions), anything else goes through `lang::extract_symbols`, which only knows TypeScript/TSX and SQL — not the language-specific `StackPack` extractors used in production (Rust, Python, Java). That makes this fine for TS/SQL fixtures but wrong to reach for when testing another language's extraction. Returns a `FileExtraction` bundling the relative path, a hash of the raw source text (via `hash_text`), and the extracted symbol list. Real extraction in the running server goes through `RepoIndex::build`, which calls the detected stack's own `pack.extract_symbols` instead of this function.
### Depends on
- `src/hash.rs::hash_text` — crate::hash

## `FlowEdge`
`pub struct FlowEdge`
### Summary
One "this file reaches that thing" connection that CodeOwl's structural import graph can't see on its own — a `fetch("/api/…")` call, a `.from("table")` query, a `<Component/>` rendered without a plain import. A pack's extractor finds these as raw strings; CodeOwl then resolves each one against the already-built graph.
### Behavior
`from_file` names the file the edge starts in. `kind` is free text owned by the pack itself (e.g. `"route-literal"`, `"table-ref"`, or `"rendered-component"` for the TypeScript+Next stack) rather than a fixed enum in the generic core — that's what lets a stack's feature model dispatch on edge kinds specific to its own framework conventions without the core needing to know about them. `raw` is the literal text the pack matched (a path string, a table name, a JSX tag name), kept around so a `kind`-specific consumer (`get_callers` run on a schema table, say) doesn't need a second parallel data structure, and so it's available for display even before resolution. `target` holds where the edge resolved to (or that it didn't resolve at all).

This struct replaces three separate ad hoc fields an earlier version of the extractor used for route literals, table references, and rendered components — one shape for all three, distinguished by `kind`.
### Depends on
- (none)

## `FlowTarget`
`pub enum FlowTarget`
### Summary
Where a resolved `FlowEdge` points: `Node(SymbolId)` for an edge that landed on an arena node (a file, or a schema table symbol), or `Unresolved` for a raw string that matched nothing.
### Behavior
An `Unresolved` edge is still meaningful — it counts as "this file does data work" for the feature model's `core` admission — it just has no destination to traverse. A `Copy` enum stored inline on each `FlowEdge`.
### Depends on
- `src/symbol.rs::SymbolId` — crate::symbol

## `UnresolvedFlowEdge`
`pub struct UnresolvedFlowEdge`
### Summary
A `FlowEdge` before it's been resolved to a target — just the raw text a pack found, with nowhere yet to say what it points at.
### Behavior
`UnresolvedFlowEdge` carries only `from_file`, `kind`, and `raw` — the same fields `FlowEdge` has, minus `target`. `pack.extract_flow_edges` produces one of these per file during extraction, before a graph exists to resolve anything against; these get cached in `RepoIndex` alongside the rest of a file's extraction output. At graph-build time, `pack.resolve_flow_edge` consumes each one and turns it into a full `FlowEdge` by working out its `target` — separating "find the literal" (per-file, cheap, no graph needed) from "figure out what it means" (needs the whole graph built first) into two distinct steps.
### Depends on
- (none)

## `Graph`
`pub struct Graph`
### Summary
The whole codebase, once extracted: every file and every declaration inside it, held in one flat arena (a single list every node lives in, referenced by position rather than by pointer — see `SymbolId`) plus the cross-file connections between them (imports, and the extra "flow edges" a pack finds beyond plain imports). This is the thing CodeOwl builds once per repo and persists to `.codeowl/graph`, and almost every other module reads from it.
### Behavior
**Building.** `Graph::build(files)` takes every file's `FileExtraction` and produces a `Graph` in two passes: first it reserves one arena slot per node (a file, then each of its symbols, in declaration order) so every string id has a `SymbolId` to resolve to; then it builds the actual nodes, since any node's `parent`/`children` may need to reference another node that didn't have a slot yet on a single pass. `pack_name`/`set_pack_name` stamp which `StackPack` built this graph (e.g. `"typescript-next"`, `"rust"`) so a later read-only caller can recover the right pack for classification or feature-model logic without threading it through every function call; an empty `pack_name` (a test helper, or a cache from before this field existed) falls back to the TypeScript pack. `format_version` is a version stamp checked on `load` — a mismatch means the cache's shape is out of date and must be rebuilt, never trusted as-is.

**Cross-file edges.** Two kinds of file-to-file connection live here, both computed once at build time and persisted alongside the graph, since resolving either needs the graph to already exist: `imports` (plain `import`/`require` resolution — see `resolve.rs`) and `flow_edges` (the connections a structural import graph can't see at all — a `fetch("/api/…")`, a `.from("table")` query, a rendered `<Component/>` — see `FlowEdge`). `resolved_default_imports` is one more narrow, pack-specific piece of import-resolution plumbing: default imports (`import Name from './x'`) resolved to their target file, needed specifically because a React component is usually default-exported, so the ordinary named-import list can't resolve a `<Component/>` reference on its own.

**Looking things up.** `get`/`get_symbol`/`get_file` fetch a node by its `SymbolId`, the latter two narrowing to one variant. `find` does the reverse — look up a node by its stable string id (`"<file>::<name>"`, or a bare path for a file). `string_id` translates a `SymbolId` back to that same stable string, which is how internal ids get made safe to hand to an external caller (a `SymbolId` is only valid within the `Graph` that produced it, and must never leak past this process — see its own spec). `parent_id`/`children_ids` walk the containment tree; only symbols carry a parent for now, since directory nodes don't exist yet. `owning_file_id` finds which file a node's source lives in — itself, for a file, or its `file` field, for a symbol — which any handler accepting "a symbol id or a bare file id" (`get_source`, `get_callees`) needs before doing anything specific to one kind. `file_role` looks up a repo-relative path's role (e.g. ordinary source vs. build-generated) under this graph's active pack — a lookup shared between `spec.rs`'s classification and `quarkus.rs`'s entry-point enumeration, so both stay in sync rather than each re-deriving it. `table_callers` answers "what app code touches this database table": every `(file, table-name)` pair whose flow edge resolved to the given schema table, surfaced through `get_callers`.

**Persistence.** `save`/`load` write and read the whole arena as plain JSON at a given path (`.codeowl/graph` in the target repo). JSON rather than a binary format like bincode is a deliberate simplicity/inspectability tradeoff — being able to `cat`/`jq` the cache while working on this repo matters more than raw serialization speed at this scale.
### Depends on
- `src/resolve.rs::ResolvedImport` — crate::resolve
- `src/symbol.rs::Symbol` — crate::symbol
- `src/symbol.rs::SymbolId` — crate::symbol
- externals: anyhow, std

## `build_graph_from_sources`
`pub fn build_graph_from_sources(files: &[(&str, &str)]) -> Graph`
### Summary
Test helper: build a `Graph` straight from `(path, source)` pairs, extracting and hashing each. Used by unit tests that need a real arena but not a `RepoIndex` or a temp directory.
### Behavior
Maps each pair through `extract_and_hash` and calls `Graph::build`. Because `extract_and_hash` routes through the TypeScript pack's extension dispatch, the graph it produces has TypeScript symbols and no `pack_name` stamp — `for_name("")` then resolves it as the TypeScript pack. Fine for TS-oriented tests; Rust and other packs' tests build their `FileExtraction`s directly.
### Depends on
- (none)
