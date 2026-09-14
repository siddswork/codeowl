---
kind: file
source_paths: [src/graph.rs]
file: { source_hash: 00e2ac9d5bd384b334f406ccab6c3a51fa0871b6ae24826868ebde62b0e268f4, deps_hash: 8504fda56ce89e6187e8b0b8d3bf00c2537130f48a6b0894b729c7fb8474de19, spec_hash: bd82c6062e0ed18ba671bd36cbdebf1cb9eea6bd04a721b17419740c80bcc7e4 }
symbols:
  src/graph.rs::SymbolView: { source_hash: 74beb26c09696c3a4861f5c5dc167d532fa8f945f6c516c357d623404f9e47a2, deps_hash: b83160b92c2fec79f3d97a822ad50ca9a2484bae2a019246166efc86c573959c, spec_hash: 770e547feccd1b1de8c211618c83fb64b2eaf61511e14330aded15320b7febb4 }
  src/graph.rs::FileNode: { source_hash: 50496f192133c817fce8f5cec823087bc9f7a667ab07113c34cc6960736f3040, deps_hash: ac583dfc44b8c03e1d77dc3191f47c59b6bb6a7b182fa0031e169bb47d564290, spec_hash: c290bcf6428116f4501ee1e393f15a54d5fba3d9e8115192ed593f9641cc8ff7 }
  src/graph.rs::Node: { source_hash: d54a33462ad83d4a30a907cbd652600809f5bb2bbda1968951e52c3e967f45c5, deps_hash: 76440fca2ed3df1f31a2a7eedb74846c2e9b886c6429549021f34e529fd3e73c, spec_hash: b67e4d211af1fd19c01ee1eacd0b6c6e244b6753d6d4a4092e8e3c13d1f775c2 }
  src/graph.rs::FileExtraction: { source_hash: 87bed0efb5a8d642238dd3621226763a4924309bf17ae397a26611aa78766000, deps_hash: ce7d9d7fc16420378d45b890370f2eb5a11a122aee5b39ac6d807d291364fb0f, spec_hash: 3c89cb954fd43bd06849379e034d359f9e820162ca0bccc413bd4e11254654a5 }
  src/graph.rs::extract_and_hash: { source_hash: 90b73a4ee99da81bd1bf071b59a555179cedb73e224d9146186258778cd78f72, deps_hash: d7da40bd5b91baea3db803ad745a18c4adf9c2308a97b126dd894920bb84e365, spec_hash: ca69d203ed27f2cbe8c29e1be937dcfaaca88cc0686d9b93907d6f5ef1bdc4a5 }
  src/graph.rs::FlowEdge: { source_hash: 9d21ac10960ef83821fcee10fb98eae1778bcda292f9684dafa5742f34f329b5, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 4323ca304e0ced4361928d8dd038b2851f2086bcc59ba72a802156548261667e }
  src/graph.rs::FlowTarget: { source_hash: 181f53cda8573f83b630a41747ca62f48694931c538e4c8e6f762ca4845088dc, deps_hash: ac583dfc44b8c03e1d77dc3191f47c59b6bb6a7b182fa0031e169bb47d564290, spec_hash: b2752135ad033bc9d3fef9a4115ce5e76bcee552b14e69d9521f90bb745de560 }
  src/graph.rs::UnresolvedFlowEdge: { source_hash: be972c52ae0f1251d1861771d972fe9a06a5f0e4d198f3cf50cae3f7f3e4ce82, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c7d647fc024badc8b4bd54d62910a96bd57342797fcfa35f76255a7e3ad92ac1 }
  src/graph.rs::Graph: { source_hash: 4e5507e6985593bcff4cb274b4760ecabc3000a77c0f9d7091ab641e6c64fd65, deps_hash: 4eb47fa613708545c614af5c29d385eb5c650921b78be2ee5279c0ae403760af, spec_hash: 216db85619583c37e3e0a16c69067483d7ccb28a7766bcf95cbd99621b446cf7 }
  src/graph.rs::build_graph_from_sources: { source_hash: dd5e3e0c36327ad645a76af4a2c9fd5bb8cbe4f36f51ed9217dc8a15a1d80028, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 55df20b1897ceed56015a0ace1c249b8db22646e47caf5beeb18422d4b6caa47 }
---
# src/graph.rs
## Summary
This file defines the `Graph` — the in-memory structure that holds everything CodeOwl knows about a repo once every file has been scanned. Each source file, and every function/class/table found inside it, becomes one entry in a single flat list (the "arena" — one big vector every other part of the codebase points into by a small numeric id rather than by name, which keeps lookups cheap and avoids the borrow-checker headaches of pointers pointing at each other). `Graph::build` is the one place that turns the raw, per-file extraction output (which just uses string names like `"a.ts::double"`) into that arena, wiring up parent/child relationships (a method belongs to its class, a class belongs to its file) as numeric ids. Beyond containment, the graph also stores two other kinds of cross-file links computed after the initial build: resolved imports (which file an `import` statement actually points at) and "flow edges" — looser connections a plain import graph can't see, like a web request path or a database query naming a table by string. The whole structure can be saved to and loaded from a JSON cache file (`.codeowl/graph`) so a repo doesn't need to be re-scanned from scratch on every run; a version stamp baked into that file makes sure an old, incompatible cache gets rejected and rebuilt rather than silently misread.</content>

## `SymbolView`
`pub struct SymbolView`
### Summary
The serialization-safe projection of a `Symbol` — the same fields, but `parent`/`children` are the target nodes' stable string ids instead of the internal `SymbolId`s. This is what `get_symbol` and `codeowl extract` return and what leaves the process, since a `SymbolId` is only valid inside the `Graph` that made it.
### Behavior
`SymbolView::from_graph(graph, id)` is the single constructor: it looks the node up with `graph.get_symbol` (which returns `None` for a `FileNode`), clones every scalar field, and maps `parent` plus each `children` `SymbolId` through `graph.string_id` back to its stable string form. This is the only place a whole symbol's `SymbolId` links get translated to strings, and it is why nothing downstream of `get_symbol` ever sees a raw arena index. `raw` and `markers` are `#[serde(default)]` so a view written by an older cache still deserializes. It derives `Serialize` + `JsonSchema` (the latter for the MCP tool schema) but deliberately not `Deserialize` — an output-only view, never read back.
### Depends on
- `src/symbol.rs::SymbolKind` — crate::symbol
- `src/symbol.rs::SymbolId` — crate::symbol

## `FileNode`
`pub struct FileNode`
### Summary
A file's own node in the arena, one per walked source file. `id` is the repo-relative path, `children` are the `SymbolId`s of its top-level symbols, `source_hash` is a hash of the file's raw text.
### Behavior
`source_hash` is deliberately a plain hash of the whole file, not a Merkle rollup of its children's hashes the way a container symbol's is: a file's spec-relevant surface (imports, doc comments, type aliases) isn't fully captured by its declarations, so any textual edit should be able to move the file spec's staleness. `FileNode` and `Symbol` are the two variants of the arena's `Node` enum.
### Depends on
- `src/symbol.rs::SymbolId` — crate::symbol

## `Node`
`pub enum Node`
### Summary
One entry in the arena — a two-variant enum wrapping either a `Symbol` (a declaration) or a `FileNode` (a file). The uniform node type that lets containment links point at files and symbols alike by a single `SymbolId`.
### Behavior
Constructed only by `Graph::build`; everything else reads it back through the typed accessors `get_symbol` / `get_file` (each returns `None` for the wrong variant) or the untyped `get` when the distinction genuinely doesn't matter, such as hashing. Its one private helper, `string_id`, returns whichever variant's stable string `id` (`Symbol::id` or `FileNode::id`) without the caller matching on the variant — `Graph::string_id` and the hash-propagation paths build on it. Both variants also carry a `children: Vec<SymbolId>`, which `children_ids` abstracts over.
### Depends on
- `src/symbol.rs::Symbol` — crate::symbol

## `FileExtraction`
`pub struct FileExtraction`
### Summary
The per-file input to `Graph::build`: a repo-relative path, a hash of the file's raw text, and the `ExtractedSymbol`s a pack's extractor produced from it. `RepoIndex::rebuild` assembles one of these per walked file.
### Behavior
Purely a carrier — no logic. `Graph::build` takes a `Vec<FileExtraction>`, creates a `FileNode` per entry keyed on `rel_path`, and resolves each `ExtractedSymbol`'s string containment links into `SymbolId`s.
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
One resolved "this file reaches that thing" edge that the import graph can't see — a `fetch("/api/...")` call, a `.from("table")` query, a rendered `<Component/>`. Holds the origin file, a pack-owned `kind` tag, the raw literal the pack matched, and a `FlowTarget` (a resolved node or unresolved).
### Behavior
The pack produces these unresolved (`UnresolvedFlowEdge`, no target) during extraction; `Graph::build`'s consumer resolves each one against the finished graph via `pack.resolve_flow_edge`. `kind` is free text so the feature model can branch on it, and `raw` is kept so a `kind`-specific reader like `get_callers` on a table needs no parallel structure. This one generic field replaced M10/M11's three typed collections (`route_literals`, `table_refs`, `rendered_components`) in M13.
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
A `FlowEdge` before resolution: just the origin file, the pack's `kind` tag, and the raw literal. What `pack.extract_flow_edges` returns per file.
### Behavior
Persisted in `RepoIndex` alongside the file's other cached inputs, so resolution can be redone against a rebuilt graph without re-parsing. `RepoIndex::rebuild` turns each into a resolved `FlowEdge` by calling `pack.resolve_flow_edge` once the arena and imports exist.
### Depends on
- (none)

## `Graph`
`pub struct Graph`
### Summary
The whole structural index of a repo: a flat arena of `Node`s (files and symbols), a `BTreeMap` string-id lookup, the resolved file-to-file `imports`, the resolved `flow_edges`, and `resolved_default_imports`. Every `get_symbol` / `get_callers` / `get_callees` / feature query reads off one of these, and the whole struct is what serializes to `.codeowl/graph`. Its inherent methods are the two-pass `build` constructor, the arena accessors (`get` / `get_symbol` / `get_file` / `string_id` / `find` / `parent_id` / `children_ids` / `symbols` / `files`), the reference-edge getters and their `set_*` mutators, `table_callers`, `pack_name` / `set_pack_name`, `len` / `is_empty`, and `save` / `load`.
### Behavior
Built in two phases. `Graph::build` takes a repo's worth of `FileExtraction`s and runs two passes: first reserve one arena slot per node — a file, then each of its symbols in declaration order — so every string id already has a `SymbolId` to map to; then construct the real `Node`s, resolving each `ExtractedSymbol`'s string `parent`/`children` (and a top-level symbol's implicit membership in its file) into `SymbolId`s. `format_version` is stamped to `FORMAT_VERSION` here and `pack_name` starts empty (set later via `set_pack_name`). The reference edges can't be known at build time — resolving them needs the finished arena to look symbols up in — so `imports`, `flow_edges`, and `resolved_default_imports` start empty and are filled by the `set_*` mutators during `RepoIndex::rebuild`. `find` is the string-id → `SymbolId` entry point; `string_id` is the reverse and the only safe way to name a node outside the process (a `SymbolId` must never leak). `nodes` / `by_id` and the edge vecs are private, so all traversal goes through the accessors; `by_id` is a `BTreeMap` rather than a `HashMap` specifically so the serialized cache is byte-identical between otherwise-identical runs. `table_callers` is the schema-side query behind `get_callers` on a SQL table — every `.from("<table>")` flow edge that resolved to it. `load` checks `format_version` against `FORMAT_VERSION` and bails on a mismatch rather than trusting a stale on-disk shape; `save` writes pretty JSON (not bincode — Phase 1 favours a `cat`/`jq`-able cache).
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
