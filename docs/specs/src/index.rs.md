---
kind: file
source_paths: [src/index.rs]
file: { source_hash: 45e891ff0e2acdbdce12446a57e6d150c84741a2ddc97bbb1543372bd157a170, deps_hash: d46ca85156fba54dcdee1dbb3de6f6d202918947d5733888806580a8a18bc595, spec_hash: e83b3b04f36831fe6c939f848ca476408ce58f2971823f6e468562873d2a4219 }
symbols:
  src/index.rs::FileInputs: { source_hash: 5d489492ec616b18b0be27bfae344514646ac53700a7973f8dc8ff6bed62f939, deps_hash: 5b97c6225d0be7f3c906750f5e4443b00e9f9ffa801407ddbbb089efd9da46d6, spec_hash: 97bfa518ac935fdbfd888fe92e4c58f7dc1fa47a1dd87eec46fd23a80ad35ed5 }
  src/index.rs::CatchUp: { source_hash: 677e9d2d3bd8a4bd4101ed2c956aa8dfb785a27dca9f0f6fb4994179f445a6dd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3dee682eb6762f6361122003798ef4c5917cbc7bdb96a62b8fdb8ed1b4867085 }
  src/index.rs::RepoIndex: { source_hash: 35d4957e14d8e2f99cef049ee3db84574fe4875c40056414c91741aff238526f, deps_hash: f4472e7729baf28d1ef0aa1677f7817d40e163c3b948f849ca5204ba3bd9746f, spec_hash: 024200935414252f806b3aee536cedd4edd7299689f8f580f4350b07a71e6525 }
  src/index.rs::canonical_root: { source_hash: a12afe5bc286ba94b312c3a2ed43df1be87659af6c1dc03d27fb702901f0ad8e, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c006dd5c57e68d23df0160e998922dd80f01dcdcbe723ac1f3913c9cc2c77ff8 }
  src/index.rs::canonicalize_event_path: { source_hash: 3bebe39ce6f00d1ab58958d5e87cf298bb3fcdc0f44186a7170e195f8e6ad359, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: bed0673d7e2ef62cc32f60efe539242a27ec17aee128e91c10688a71bfa63ab8 }
  src/index.rs::rel_path: { source_hash: 8fb5e79ec2f2f3bc5d8fed6fdbb54b303b47381e0bd2943f80b3745a47e1ee9c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b887526579e14f7161923631b7b35c6d45fe67e3905af60c966a59b04e051225 }
---
# src/index.rs
## Summary
Makes CodeOwl's re-indexing incremental. Rather than re-parsing the whole repo on every startup or every keystroke, it caches the per-file inputs the graph is built from — a file's extracted symbols, its imports, its flow edges, and a hash of its text — and re-parses only the files whose hash changed. `RepoIndex` is the type; `FileInputs` is one file's cache entry; `CatchUp` records what a re-scan actually touched (added / modified / removed). It drives two moments: `open`, the fresh-process catch-up that diffs the saved cache against what's on disk now, and `apply_changes`, the live update the file watcher calls with each batch of changed paths. The `canonical_root` / `canonicalize_event_path` helpers keep every path in one symlink-free form, so the prefix-stripping that turns absolute paths into repo-relative ones always matches.

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
Keeps the per-file inputs the graph is built from — each file's extracted symbols, its imports, and a hash of its raw text — so that after an edit CodeOwl re-parses only the files that actually changed, not the whole repo. It's what makes both startup and the live file-watcher fast. Saved to disk at `.codeowl/index`, next to the graph.
### Behavior
`build` does a cold start: canonicalize the repo root (resolve any symlinks — everything downstream strips this prefix off absolute paths, so the two forms have to match), pick the language pack via `lang::detect`, walk every file the pack reads, and cache each one's inputs.

`open` is the normal startup path. It canonicalizes the root up front, then tries `load` on the saved `.codeowl/index`; if that returns something usable it runs `rescan` (hash-check every file on disk, re-parse only what changed while nothing was running) then `rebuild`, otherwise it falls back to a full `build`. `load` discards the cache — forcing a full rebuild — if the on-disk format stamp doesn't match the current one, or if `lang::detect` now picks a different pack than the cache was built with (the cached parse would be from the wrong grammar).

`apply_changes` is the file-watcher's entry point: given a batch of changed paths, it first normalizes each to the same canonical form as the stored root (`canonicalize_event_path` — watcher backends report paths differently, and a form mismatch would silently drop the event), then per file: unchanged content → nothing; changed → re-extract and record `modified`/`added`; a path that no longer reads → treat as a deletion. If anything actually changed it calls `rebuild` and hands back the fresh graph.

`rebuild` reconstructs the whole graph from the cached inputs — cheap, because no parsing happens: reassemble it in memory, run the pack's import resolution and flow-edge resolution against it, then write both `.codeowl/graph` and the updated index back to disk. `watchable_dirs` lists every directory under the root for the watcher to register.

The struct carries the format stamp, the pack name it was built with, and the per-file map (a `BTreeMap`, so a rebuilt graph's node order is deterministic). The absolute root and the pack object are never serialized — they're re-derived from the repo on `load`.
### Depends on
- `src/graph.rs::FileExtraction` — crate::graph
- `src/graph.rs::FlowEdge` — crate::graph
- `src/graph.rs::Graph` — crate::graph
- `src/hash.rs::hash_text` — crate::hash
- `src/imports.rs::FileImports` — crate::imports
- `src/stack.rs::StackPack` — crate::stack
- externals: anyhow, std

## `canonical_root`
`fn canonical_root(root: &Path) -> PathBuf`
### Summary
Resolves the repo root to its real, symlink-free path. Every later step that turns an absolute path into a repo-relative one strips this prefix off, so the root has to be in the same form the operating system reports paths in.
### Behavior
Calls `Path::canonicalize` — which follows every symlink in the path — and returns the result. If that fails (usually a missing directory), it returns the path unchanged and lets `build`/`detect` report their own error.

Why it's needed: on macOS, temp directories sit under `/var`, a symlink to `/private/var`, and both the file-watcher and the import resolver return paths with symlinks already resolved. Strip an un-canonicalized root against one of those and it fails — so every import, and every watcher event, gets silently dropped. `main.rs` canonicalizes before starting the server; doing it here as well makes `RepoIndex` safe for a direct library caller (a test, an embedder) too.
### Depends on
- externals: std

## `canonicalize_event_path`
`fn canonicalize_event_path(p: &Path) -> PathBuf`
### Summary
Puts a file-change event's path into the same symlink-free form as the stored repo root, so the two can be compared. Handles the awkward case where the event is a *deletion* and the file is already gone.
### Behavior
Tries `Path::canonicalize` on the path directly — that works for a file that still exists. If it fails (the file was just deleted, so there's nothing to resolve), it canonicalizes the *parent directory* instead — still present — and re-attaches the filename. If even that fails, it returns the path unchanged.

This is what lets `apply_changes` match an event path against `self.root` no matter which watcher backend produced it: macOS's FSEvents resolves symlinks in the paths it reports, Linux's inotify echoes back the path that was registered, and a direct caller (a test) might build a path straight off a raw repo directory.
### Depends on
- externals: std

## `rel_path`
`fn rel_path(root: &Path, path: &Path) -> String`
### Summary
An absolute path as a repo-relative, forward-slash string — the id scheme `Symbol::file` and the `files` map key both use.
### Behavior
Strips `root` (falling back to the whole path if it isn't a prefix), lossily converts to a `String`, and replaces `\` with `/` so a Windows path matches the same file's id as it would on Linux. The lossy conversion means a non-UTF-8 filename becomes `�`-containing text rather than an error — acceptable, since such a file wouldn't be valid TS/Rust source anyway.
### Depends on
- externals: std
