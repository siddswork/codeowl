---
kind: file
source_paths: [src/index.rs]
file: { source_hash: 90c9ccd8f976a4af2783ba6f6a9d46e6c9c3b471909df3fd42787878a6e40dbc, deps_hash: 272826bcf6e38116ff6985b994d826d5e0f61540038f3c5b50c6724d3d64914f, spec_hash: 6b0a950e9b7cfbd3eccd4d49e7bc3e4580964566f262998f30f3dc9c518d06de }
symbols:
  src/index.rs::FileInputs: { source_hash: fbecd5dce11211eb08c26741360e048b5070afb4e23695b3e68ee0de30bbc445, deps_hash: b3e22fbedd140b149388c32715b9051e6eff3d7c6ade1e6e7ad1b060540d0825, spec_hash: d40cb41e07c71ff865a9b4d47e61d288ed018462fea2198c7f15118560c75512 }
  src/index.rs::CatchUp: { source_hash: 677e9d2d3bd8a4bd4101ed2c956aa8dfb785a27dca9f0f6fb4994179f445a6dd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3dee682eb6762f6361122003798ef4c5917cbc7bdb96a62b8fdb8ed1b4867085 }
  src/index.rs::RepoIndex: { source_hash: 35d4957e14d8e2f99cef049ee3db84574fe4875c40056414c91741aff238526f, deps_hash: f4472e7729baf28d1ef0aa1677f7817d40e163c3b948f849ca5204ba3bd9746f, spec_hash: 024200935414252f806b3aee536cedd4edd7299689f8f580f4350b07a71e6525 }
  src/index.rs::canonical_root: { source_hash: a12afe5bc286ba94b312c3a2ed43df1be87659af6c1dc03d27fb702901f0ad8e, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c006dd5c57e68d23df0160e998922dd80f01dcdcbe723ac1f3913c9cc2c77ff8 }
  src/index.rs::canonicalize_event_path: { source_hash: 3bebe39ce6f00d1ab58958d5e87cf298bb3fcdc0f44186a7170e195f8e6ad359, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: bed0673d7e2ef62cc32f60efe539242a27ec17aee128e91c10688a71bfa63ab8 }
  src/index.rs::rel_path: { source_hash: 8fb5e79ec2f2f3bc5d8fed6fdbb54b303b47381e0bd2943f80b3745a47e1ee9c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b887526579e14f7161923631b7b35c6d45fe67e3905af60c966a59b04e051225 }
---
# src/index.rs
## Summary
This file makes CodeOwl's indexing incremental: instead of re-parsing every file in a repo each time it runs, `RepoIndex` remembers each file's raw-text hash and extracted data on disk (in `.codeowl/index`), so on a fresh run it only re-parses files that actually changed since the last time — and while CodeOwl is running, a background file watcher feeds it individual edits (via `apply_changes`) so the graph stays current without waiting for a restart. It handles both the cold-start case (no cache yet, or a cache from an incompatible format/language stack, so a full walk-and-parse happens) and the warm-start case (a valid cache exists, so only added/modified/deleted files get re-processed). After updating its cached per-file data, it rebuilds the full graph from that data (cheap, since no parsing is needed) and resolves cross-file links — imports, and the "flow edges" that need the whole graph to resolve against — before saving both the graph and the index back to disk. It also tracks which directories the file watcher should actually watch, respecting `.gitignore` so things like `node_modules` are never monitored.</content>

## `FileInputs`
`pub struct FileInputs`
### Summary
A per-file cache entry holding everything needed to build the graph and resolve imports for one file, without re-parsing that file's text again — the reason an unchanged file in a repo isn't re-scanned every time CodeOwl rebuilds its index.
### Behavior
Holds a hash of the file's raw source text (used to detect whether it changed since it was cached), its extracted symbols, its parsed imports, and its unresolved "flow edges" (looser cross-file links, like a URL literal, that get resolved separately once the whole graph exists). The `extract` constructor is where a file actually gets processed for the first time: it hashes the source, asks the active language stack (a `StackPack`) to extract the file's symbols, and then applies the schema-detection hook (`is_schema_symbol`) to retag any symbol the stack recognizes as a database table (e.g. an ORM model class) from its default kind to `Schema`. For a dedicated schema file (like a `.sql` file), it skips import and flow-edge extraction entirely, since such a file has neither; for an ordinary code file (or an unrecognized kind), it also extracts imports and flow edges from the stack.</content>
### Depends on
- `src/graph.rs::UnresolvedFlowEdge` — crate::graph
- `src/hash.rs::hash_text` — crate::hash
- `src/imports.rs::FileImports` — crate::imports
- `src/lang.rs::SourceKind` — crate::lang
- `src/stack.rs::StackPack` — crate::stack
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- `src/symbol.rs::SymbolKind` — crate::symbol
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
