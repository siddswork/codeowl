---
kind: file
source_paths: [src/graph.rs]
file: { source_hash: e6f892fd557ed6820c3634c3cea5f8dc12a87472598473f7baa24d2d15d27dc6, deps_hash: f9e7491287964d290209f3b34d9e5861fdc7d443ab6cb4b2a2bd78da2a4229d0, spec_hash: 443634927035e90e48251eaf358f999aaa147e8cb21657ccb39b0a96462fdd96 }
symbols:
  src/graph.rs::SymbolView: { source_hash: ca3259f0c9f3c6641e2a79400cc56590870eabe278c316c5c6f01767376629b5, deps_hash: 75e597a386580b65084ca8f765b6795ac8743e4e1000eb34c14097f75222d25f, spec_hash: 0f2eca31bca89dd5b16a12aadfde121b04ecf9f2762c481331c1dbef1ec340f7 }
  src/graph.rs::FileNode: { source_hash: 1cb0e932df0ba6250e1e2f9a72576a667d908f5f70ebeb405ae82f825d777545, deps_hash: d6f92d466688b754b386494c91e0b98c53e59b4ac03c775d21b69d67284b0b70, spec_hash: 441ac9483212e12f7eac5c49c7132b054d0cd191abf6af252252bc679cec8974 }
  src/graph.rs::Node: { source_hash: d54a33462ad83d4a30a907cbd652600809f5bb2bbda1968951e52c3e967f45c5, deps_hash: 34a95995527dc5f673d09639b70c27217984732249263b23f878b003a1bf1598, spec_hash: aa4c78080483245326dcb447f34ea9c3f549720e25bb03fa5989daa5a232733b }
  src/graph.rs::FileExtraction: { source_hash: ed79c4363575bcf427c97838ad5d310bf8e2724ed086fa6baaed390d8dbcf84c, deps_hash: 2f2d813cb41bd73f13411f197c03a6eeb3990c05207868c8d2422e2775913c15, spec_hash: 269dd7feb7865c625e2ede32fbd4885d8c0dc7d7b133374f188435534d111d94 }
  src/graph.rs::extract_and_hash: { source_hash: 90b73a4ee99da81bd1bf071b59a555179cedb73e224d9146186258778cd78f72, deps_hash: dea75763eb06a70a4b24fa5e633ae7315ae0b2349ee4246fbb0ec52edb4dc9af, spec_hash: 643963be19a0907607eebde105c58aba43ea9e31b0c0b6d52a58ca56c9903f1a }
  src/graph.rs::FlowEdge: { source_hash: 8303f3f670978cf87fcfda541c910676baa4e577f2929431a880fe0c41551c36, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: d7756de2ae6c9cf6f564e63f12ce6a4b80f68f70018f3c8f6069cb1c3cd591d9 }
  src/graph.rs::FlowTarget: { source_hash: 181f53cda8573f83b630a41747ca62f48694931c538e4c8e6f762ca4845088dc, deps_hash: 2c612d8ff3471f6990442eae121f91802b45c305137b9b48b06f2d44bc45a54c, spec_hash: 8799b33e1b7d587396d3e8f615fd180322cb8cd8c0fdf4692cb468d750a76ef3 }
  src/graph.rs::UnresolvedFlowEdge: { source_hash: 247d5a418c02e679b95f2a79f6b153fbe044257e740deca6380e82514abcb422, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 1b08108bf64809c00a1e59025885a05382ab85ded5b10172af966abafa84f9fe }
  src/graph.rs::Graph: { source_hash: 481901ceff94a9240895681ab2d2dbd6fb74548c595980d2f08a9140eb769a75, deps_hash: 1363fd8e5ec54006f0090e49de57e85db19100cd42d3aa0580838e4897b516ec, spec_hash: 3113e6ab5d0b24ba9721b273635f7b06cbf9d42dae06f8d514edf4423c7e27b6 }
  src/graph.rs::build_graph_from_sources: { source_hash: dd5e3e0c36327ad645a76af4a2c9fd5bb8cbe4f36f51ed9217dc8a15a1d80028, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 55df20b1897ceed56015a0ace1c249b8db22646e47caf5beeb18422d4b6caa47 }
dep_targets:
  file -> src/hash.rs::hash_text: 70c099c1622c
  file -> src/lang.rs::FileRole: 2e8bc021ccde
  file -> src/lang.rs::extract_symbols: 86f13d92fbdd
  file -> src/resolve.rs::ResolvedDefaultImport: bebbd77bb6f0
  file -> src/resolve.rs::ResolvedImport: cfdf3812ee69
  file -> src/schema.rs::extract_tables: ac1e0de7c6f8
  file -> src/stack.rs::for_name: 45d7be46f14a
  file -> src/symbol.rs::ExtractedSymbol: 314759561cf6
  file -> src/symbol.rs::Symbol: 2bab71782e46
  file -> src/symbol.rs::SymbolId: 6733b5c62e75
  file -> src/symbol.rs::SymbolKind: 264efecb34de
  src/graph.rs::SymbolView -> src/symbol.rs::SymbolId: 6733b5c62e75
  src/graph.rs::SymbolView -> src/symbol.rs::SymbolKind: 264efecb34de
  src/graph.rs::FileNode -> src/symbol.rs::Symbol: 2bab71782e46
  src/graph.rs::FileNode -> src/symbol.rs::SymbolId: 6733b5c62e75
  src/graph.rs::Node -> src/symbol.rs::Symbol: 2bab71782e46
  src/graph.rs::FileExtraction -> src/symbol.rs::ExtractedSymbol: 314759561cf6
  src/graph.rs::extract_and_hash -> src/hash.rs::hash_text: 70c099c1622c
  src/graph.rs::extract_and_hash -> src/lang.rs::extract_symbols: 86f13d92fbdd
  src/graph.rs::extract_and_hash -> src/schema.rs::extract_tables: ac1e0de7c6f8
  src/graph.rs::FlowTarget -> src/symbol.rs::SymbolId: 6733b5c62e75
  src/graph.rs::Graph -> src/lang.rs::FileRole: 2e8bc021ccde
  src/graph.rs::Graph -> src/resolve.rs::ResolvedDefaultImport: bebbd77bb6f0
  src/graph.rs::Graph -> src/resolve.rs::ResolvedImport: cfdf3812ee69
  src/graph.rs::Graph -> src/stack.rs::for_name: 45d7be46f14a
  src/graph.rs::Graph -> src/symbol.rs::Symbol: 2bab71782e46
  src/graph.rs::Graph -> src/symbol.rs::SymbolId: 6733b5c62e75
  src/graph.rs::Graph -> src/symbol.rs::SymbolKind: 264efecb34de
---
# src/graph.rs
## Summary
This file defines CodeOwl's graph: the arena (one flat list holding every node, referenced by position rather than by pointer) that turns per-file extraction output into something indexable and cross-referenceable. Extraction only ever sees one file at a time and produces declarations whose links are plain text. `Graph::build` is the one place that turns a whole repo's worth of those into the real arena. It gives every file and every declaration a position, and resolves containment (a declaration's parent and children) against it. Imports, and the extra "flow edges" a language pack finds beyond plain imports, such as a `fetch("/api/...")` call or a `.from("table")` query, are attached afterwards, since resolving them needs the built graph. `SymbolView` is the safe copy of a declaration to hand outside the process (an MCP reply, `codeowl extract` output), because the internal arena position only means something inside the graph that produced it. The graph saves itself as plain JSON to `.codeowl/graph`, stamped with a format version so a cache from older logic is discarded and rebuilt rather than trusted. Nearly every other module reads its data through the lookup functions here rather than walking the arena directly.

## `SymbolView`
`pub struct SymbolView`
### Summary
A copy of a symbol that is safe to hand outside the program, such as in an MCP reply, `codeowl extract` output or spec files. Its links to the parent and children are stable text names instead of internal numbers.
### Behavior
Carries the same fields as `Symbol` (id, kind, raw kind, file, lines, signature, docstring, export flag, both hashes, markers) but with `parent` and `children` as string ids. The internal `SymbolId` is only valid for the graph that produced it, so it must never appear in output someone might keep past this process. It leaves out `extra_spans`, which is internal to dependency scanning. `SymbolView::from_graph(graph, id)` builds one: it returns `None` if the id is not a symbol (a file node, say), otherwise clones the fields and translates the parent and each child through `graph.string_id`. Any place a symbol crosses the process boundary should build one of these and never serialise a `Symbol` directly.
### Depends on
- `src/symbol.rs::SymbolKind` — crate::symbol
- `src/symbol.rs::SymbolId` — crate::symbol

## `FileNode`
`pub struct FileNode`
### Summary
A source file's own entry in the graph, holding its path, a fingerprint of its full text, and the list of its top-level declarations.
### Behavior
`id` is the repo-relative path, the same naming `Symbol::file` uses. `source_hash` hashes the file's raw text as a whole, not a roll-up of its declarations' hashes the way a class's hash folds in its methods. A file's content that matters for a spec, such as imports, comments and type-only lines, is not fully captured by its declared symbols, and a whole-file hash is simpler and strictly more sensitive to change. `children` lists the file's top-level symbols in declaration order, as arena ids. Plain data, no logic.
### Depends on
- `src/symbol.rs::Symbol` — crate::symbol
- `src/symbol.rs::SymbolId` — crate::symbol

## `Node`
`pub enum Node`
### Summary
One entry in the graph's arena, which is either a declaration (`Symbol`) or a whole source file (`FileNode`). Files and declarations live in the same list so containment links can point at either.
### Behavior
Two variants: `Node::Symbol(Symbol)` and `Node::File(FileNode)`. Only `Graph::build` creates them; everything else reads them back through `get_symbol` or `get_file`, which return an `Option` for the expected kind, or through `get` when the kind does not matter, for example when hashing. A private helper `string_id` returns the text id of either variant (the symbol's id or the file's path), so the graph can keep one lookup table from text id to arena position over both kinds. It has no failure mode.
### Depends on
- `src/symbol.rs::Symbol` — crate::symbol

## `FileExtraction`
`pub struct FileExtraction`
### Summary
The result of parsing one file: its path, a fingerprint of its text, and the declarations found in it. It is the per-file input from which the graph is built.
### Behavior
Holds `rel_path` (the repo-relative path), `source_hash` (a hash of the file's raw text) and `symbols` (a list of `ExtractedSymbol`, whose links are still plain text). `Graph::build` takes a list of these for the whole repo and turns each into a file node and symbol nodes in the arena. Nothing here depends on other files, which is what lets extraction run per file and be cached per file. Plain data, no logic.
### Depends on
- `src/symbol.rs::ExtractedSymbol` — crate::symbol

## `extract_and_hash`
`pub fn extract_and_hash(rel_path: &str, source: &str) -> FileExtraction`
### Summary
A shortcut for tests: given a file's path and text, it parses the file and fingerprints it in one call, returning the per-file result the graph builder takes.
### Behavior
If the path ends in `.sql` it extracts tables with `schema::extract_tables`, otherwise it extracts declarations with the TypeScript extractor `lang::extract_symbols`. It then builds a `FileExtraction` with the path, a hash of the source text via `hash_text`, and the symbols. It is for test fixtures only: it always uses the TypeScript and SQL rules, not whichever language pack the repo was detected as, so it produces nothing useful for Rust, Java or Python text. Real extraction goes through `RepoIndex::build` and the detected pack. It cannot fail.
### Depends on
- `src/hash.rs::hash_text` — crate::hash
- `src/schema.rs::extract_tables` — crate::schema
- `src/lang.rs::extract_symbols` — crate::lang

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
Says where a flow edge ends up: at a real node in the graph, or nowhere, when the text it named could not be matched to anything.
### Behavior
Two variants. `Node(SymbolId)` means the edge resolved to a file or a schema (table) symbol. `Unresolved` means the raw string matched nothing, for example an external URL, a database view, a path built at runtime, or a typo. An unresolved edge is still kept, because it records that this file does data or network work; it just has no destination to follow. Plain data, no logic.
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
The whole-repo index: every file and every declaration in one flat list (the arena), plus the links between files. Almost every other part of CodeOwl asks this type questions such as "what does this id point to", "what does this file import" and "which files touch this table".
### Behavior
Contents: the arena of nodes (files and symbols), a map from text id to arena position, the resolved file-to-file import edges, pack-specific flow edges (a `fetch("/api/..")` call or a `.from("table")` query), resolved default imports (needed so a rendered `<Component/>` can be traced back to its file), the name of the language pack that built it, and a format version stamp. The id map is a sorted `BTreeMap` rather than a hash map so the saved cache is byte-for-byte identical between identical runs.

Building: `build(files)` works in two passes. The first reserves one slot per file and one per symbol, so every text id has a position to resolve to. The second builds the real nodes now that any node may refer to any other, turning every text parent and child link into an arena id, including each top-level symbol's membership in its file. Imports, flow edges and default imports start empty and are filled in later through the `set_` methods, because resolving them needs the built graph to look symbols up in.

Lookups: `get`, `get_symbol` and `get_file` return a node (the last two return `None` for the wrong kind). `find` turns a text id into an arena id, and `string_id` does the reverse; the arena id must never be handed outside the process. `parent_id`, `children_ids` and `owning_file_id` give containment, `symbols()` and `files()` iterate, and `file_role(path)` classifies a path under the current pack, which spec code and the Quarkus feature code both need. `table_callers` lists the files whose `.from("table")` query resolved to a given table. `pack_name` returns the pack that built the graph, where an empty name means the TypeScript pack, as for graphs built by test helpers or very old caches.

Persistence: `save` writes the arena as plain JSON (readable with `cat` or `jq`, which was judged worth more than a smaller binary format at this scale) and `load` reads it back. A cache written without a version stamp reads as version 0, which never equals the current one, so `load` rejects it and forces a rebuild. `save` and `load` return errors on file or parse failures.
### Depends on
- `src/resolve.rs::ResolvedImport` — crate::resolve
- `src/symbol.rs::Symbol` — crate::symbol
- `src/symbol.rs::SymbolKind` — crate::symbol
- `src/symbol.rs::SymbolId` — crate::symbol
- `src/resolve.rs::ResolvedDefaultImport` — crate::resolve
- `src/lang.rs::FileRole` — crate::lang
- `src/stack.rs::for_name` — crate::stack
- externals: anyhow, std

## `build_graph_from_sources`
`pub fn build_graph_from_sources(files: &[(&str, &str)]) -> Graph`
### Summary
Test helper: build a `Graph` straight from `(path, source)` pairs, extracting and hashing each. Used by unit tests that need a real arena but not a `RepoIndex` or a temp directory.
### Behavior
Maps each pair through `extract_and_hash` and calls `Graph::build`. Because `extract_and_hash` routes through the TypeScript pack's extension dispatch, the graph it produces has TypeScript symbols and no `pack_name` stamp — `for_name("")` then resolves it as the TypeScript pack. Fine for TS-oriented tests; Rust and other packs' tests build their `FileExtraction`s directly.
### Depends on
- (none)
