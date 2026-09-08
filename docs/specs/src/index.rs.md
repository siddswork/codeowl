---
kind: file
source_paths: [src/index.rs]
file: { source_hash: 228e7e6258c32a9dbc14dfd1740ee745110dfc9ca53abe42874fd9e722f5ec62, deps_hash: d46ca85156fba54dcdee1dbb3de6f6d202918947d5733888806580a8a18bc595, spec_hash: bc0183a28eb8248e057b774cd464cfec3c548e34a36cb01d3121e7711c2214b7 }
symbols:
  src/index.rs::FileInputs: { source_hash: 5d489492ec616b18b0be27bfae344514646ac53700a7973f8dc8ff6bed62f939, deps_hash: 5b97c6225d0be7f3c906750f5e4443b00e9f9ffa801407ddbbb089efd9da46d6, spec_hash: 97bfa518ac935fdbfd888fe92e4c58f7dc1fa47a1dd87eec46fd23a80ad35ed5 }
  src/index.rs::CatchUp: { source_hash: 677e9d2d3bd8a4bd4101ed2c956aa8dfb785a27dca9f0f6fb4994179f445a6dd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3dee682eb6762f6361122003798ef4c5917cbc7bdb96a62b8fdb8ed1b4867085 }
  src/index.rs::RepoIndex: { source_hash: e6a0055dcd64e2f4321a80215988958016882fd9c29129fdf631c8437bc328c0, deps_hash: f4472e7729baf28d1ef0aa1677f7817d40e163c3b948f849ca5204ba3bd9746f, spec_hash: 72966c5718ebc918a2611cf31ab0e3ed2c38813ea0005b349b4de790d02fbb3b }
  src/index.rs::rel_path: { source_hash: 8fb5e79ec2f2f3bc5d8fed6fdbb54b303b47381e0bd2943f80b3745a47e1ee9c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b887526579e14f7161923631b7b35c6d45fe67e3905af60c966a59b04e051225 }
---
# src/index.rs
## Summary
Incremental repo indexing (M9). `RepoIndex` keeps the per-file inputs the graph is built from — each file's extracted symbols, imports, unresolved flow edges, and a hash of its raw text — persisted to `.codeowl/index` so rebuilding after an edit re-parses only what changed, never the whole tree. It backs both moments `ARCHITECTURE.md`'s "Incremental indexing" calls out: the fresh-spawn catch-up pass (`open` → `load` + `rescan` + `rebuild`) and the in-session watcher (`apply_changes`, driven by `watch.rs`). `FileInputs` is the per-file cache entry (with an `extract` that skips the import/flow passes for a `.sql` file); `CatchUp` is the added/modified/removed record; `rebuild` is the pure assembly step (arena + import resolution + flow-edge resolution, no parsing) that also re-persists `.codeowl/graph`. `format_version` and `pack_name` on the cache are both checked by `load` — a stale format or a changed stack discards the cache entirely rather than reuse inputs parsed by the wrong grammar.

## `FileInputs`
`pub struct FileInputs`
### Summary
Everything `Graph::build` and import resolution need from one file — its raw-text hash, extracted symbols, imports, and unresolved flow edges — cached (`.codeowl/index`) so an unchanged file is never re-parsed on a rebuild.
### Behavior
Its constructor `FileInputs::extract(pack, rel_path, source)` hashes the source, runs `pack.extract_symbols`, then branches on `pack.source_kind`: a `Schema` file gets empty `imports`/`flow_edges` (a `.sql` file has neither), while `Code` (and the `None` fallback) additionally runs `pack.extract_imports` and `pack.extract_flow_edges`. `flow_edges` is `#[serde(default)]` so a pre-M13 cache without the field deserializes cleanly. The flow edges are stored *unresolved* here — resolution against the whole graph happens in `rebuild`, once every file's inputs are in hand.
### Depends on
- `src/graph.rs::UnresolvedFlowEdge` — crate::graph
- `src/hash.rs::hash_text` — crate::hash
- `src/imports.rs::FileImports` — crate::imports
- `src/lang.rs::SourceKind` — crate::lang
- `src/stack.rs::StackPack` — crate::stack
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- externals: std

## `CatchUp`
`pub struct CatchUp`
### Summary
The record of which files a rescan or incremental update touched — added / modified / removed paths. The evidence M9's validation asks for ("reindexes exactly the changed files") and what the watcher logs.
### Behavior
Three string vecs plus helpers: `is_empty` (nothing changed — a fast-path no-op rebuild), `total` (count across all three), and `sorted` (sorts each vec, consumed and returned) so the result is deterministic for tests and logs regardless of directory-walk order.
### Depends on
- (none)

## `RepoIndex`
`pub struct RepoIndex`
### Summary
The per-file input cache (`.codeowl/index`) plus the routines that build, catch up, incrementally update, and rebuild the graph from it. Owns the repo root and the chosen `StackPack`. The layer between "files on disk" and "a built `Graph`".
### Behavior
Persisted fields: `format_version` and `pack_name` (both `#[serde(default)]`, both checked by `load` — a stale format or a changed pack discards the whole cache) and `files: BTreeMap<path, FileInputs>` (a `BTreeMap` for deterministic node order). `root` and `pack` are `#[serde(skip)]` and re-derived on load.

- `build` walks the repo (via `ignore`, so `.gitignore` is respected), extracts `FileInputs` for every file the pack claims, and stamps the current `FORMAT_VERSION` + pack name.
- `open` is the fresh-spawn entry: `load` the cache and `rescan` (hash every file, record added/modified/removed, re-extract only what moved) then `rebuild`; or `build` from scratch if there's no usable cache.
- `apply_changes` is the watcher's path — same diff logic but scoped to a list of changed paths; returns `None` when nothing spec-relevant actually changed.
- `rebuild` is where the `Graph` is assembled: `Graph::build` from the cached extractions, stamp the pack name, `pack.resolve_imports`, `resolve_default_imports` (still a free call — M18 folds it in), then resolve every file's unresolved flow edges against the now-complete graph via `pack.resolve_flow_edge`. Finally it writes both `.codeowl/graph` and `.codeowl/index`.
- `watchable_dirs` lists every directory under the root for the file watcher to register.
### Depends on
- `src/graph.rs::FileExtraction` — crate::graph
- `src/graph.rs::FlowEdge` — crate::graph
- `src/graph.rs::Graph` — crate::graph
- `src/hash.rs::hash_text` — crate::hash
- `src/imports.rs::FileImports` — crate::imports
- `src/stack.rs::StackPack` — crate::stack
- externals: anyhow, std

## `rel_path`
`fn rel_path(root: &Path, path: &Path) -> String`
### Summary
An absolute path as a repo-relative, forward-slash string — the id scheme `Symbol::file` and the `files` map key both use.
### Behavior
Strips `root` (falling back to the whole path if it isn't a prefix), lossily converts to a `String`, and replaces `\` with `/` so a Windows path matches the same file's id as it would on Linux. The lossy conversion means a non-UTF-8 filename becomes `�`-containing text rather than an error — acceptable, since such a file wouldn't be valid TS/Rust source anyway.
### Depends on
- externals: std
