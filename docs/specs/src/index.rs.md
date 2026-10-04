---
kind: file
source_paths: [src/index.rs]
file: { source_hash: 750775b3737ad7f099a4736e0c38ead69f0ca58732530b9146f9fe5e35420dea, deps_hash: 179cc189a7518c4ec4d7c2578a10e5bdc1529536490227bae10d09a20fabe6ed, spec_hash: 2717a6828d5ca8fb1adcd8883f440fdc1403349613aa150a2b058798dcfbd3a4 }
symbols:
  src/index.rs::FileInputs: { source_hash: 47e412f2023e265aec973a76fa55415237455a745e24eab6170a6fbf3451b8db, deps_hash: 9e8a847f0552e2c9ddea078caf8ea5bdc58ee7f737ceffbb8aea9dc6cff99bb4, spec_hash: 93bc334e7c88c6d66fc3ca871db65fe288f429d7b6042601694d6b8e3b42ee88 }
  src/index.rs::CatchUp: { source_hash: 07805fea5febc54c573e8083754584ac2d977962598fdd6afd4b01dca8bb27b6, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 52e10ce4b7920943505c7dffaf7c32c0086febee8f384aa2004fa98bb9c4568e }
  src/index.rs::RepoIndex: { source_hash: 49c345f70d359cf6bbd25bbef99a7b8d0d5bacdd97f12e5ff16f69a3450016df, deps_hash: 7726a2d4de9b761662726b621b9f3c42271168003654f57fc121486edd9c1b5d, spec_hash: 93a5cdef3b7a920074e7ca37502a7cfba9fea726f3131bd59160e2a971f328dc }
  src/index.rs::canonical_root: { source_hash: a12afe5bc286ba94b312c3a2ed43df1be87659af6c1dc03d27fb702901f0ad8e, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c006dd5c57e68d23df0160e998922dd80f01dcdcbe723ac1f3913c9cc2c77ff8 }
  src/index.rs::canonicalize_event_path: { source_hash: 3bebe39ce6f00d1ab58958d5e87cf298bb3fcdc0f44186a7170e195f8e6ad359, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: bed0673d7e2ef62cc32f60efe539242a27ec17aee128e91c10688a71bfa63ab8 }
  src/index.rs::rel_path: { source_hash: 8fb5e79ec2f2f3bc5d8fed6fdbb54b303b47381e0bd2943f80b3745a47e1ee9c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b887526579e14f7161923631b7b35c6d45fe67e3905af60c966a59b04e051225 }
  src/index.rs::ingest_build_entry: { source_hash: 32b01140a21741c0250cf861084c81bb636721ea062d15a8642d39a3709270e4, deps_hash: 3cb515d9d23dd84457b02f8b3176d442ebe0ddad3d308cecd5753c3254b4da07, spec_hash: 027ecdb35ef41a3ddd233fd7f4531fe2ed2d7a23cfb2b306ec9a93215d12eea8 }
  src/index.rs::generated_source_entries: { source_hash: 13f83892670cc66f4eb2da9852c203b725178b2d9f14664a078a547198ded4e1, deps_hash: 3cb515d9d23dd84457b02f8b3176d442ebe0ddad3d308cecd5753c3254b4da07, spec_hash: 7bb78270cb332dc41d6562e817cc40de58cfa887bbde3174b3eba02428f10d15 }
dep_targets:
  file -> src/graph.rs::FORMAT_VERSION: d42a4415675a
  file -> src/graph.rs::FileExtraction: 0cb1c67d8a8b
  file -> src/graph.rs::FlowEdge: d62382a78a55
  file -> src/graph.rs::Graph: 13dbdfa88b0e
  file -> src/graph.rs::UnresolvedFlowEdge: 17c46f8128e8
  file -> src/hash.rs::hash_text: 70c099c1622c
  file -> src/imports.rs::FileImports: 7a834189d6b3
  file -> src/lang.rs::SourceKind: d4ad91928237
  file -> src/lang.rs::detect: 32ef7186ff88
  file -> src/resolve.rs::build_resolver: 640f64019c54
  file -> src/resolve.rs::resolve_default_imports: 39471d06e1f6
  file -> src/stack.rs::StackPack: 436012c73562
  file -> src/symbol.rs::ExtractedSymbol: 314759561cf6
  file -> src/symbol.rs::SymbolKind: 264efecb34de
  src/index.rs::FileInputs -> src/graph.rs::UnresolvedFlowEdge: 17c46f8128e8
  src/index.rs::FileInputs -> src/hash.rs::hash_text: 70c099c1622c
  src/index.rs::FileInputs -> src/imports.rs::FileImports: 7a834189d6b3
  src/index.rs::FileInputs -> src/lang.rs::SourceKind: d4ad91928237
  src/index.rs::FileInputs -> src/stack.rs::StackPack: 436012c73562
  src/index.rs::FileInputs -> src/symbol.rs::ExtractedSymbol: 314759561cf6
  src/index.rs::FileInputs -> src/symbol.rs::SymbolKind: 264efecb34de
  src/index.rs::RepoIndex -> src/graph.rs::FORMAT_VERSION: d42a4415675a
  src/index.rs::RepoIndex -> src/graph.rs::FileExtraction: 0cb1c67d8a8b
  src/index.rs::RepoIndex -> src/graph.rs::FlowEdge: d62382a78a55
  src/index.rs::RepoIndex -> src/graph.rs::Graph: 13dbdfa88b0e
  src/index.rs::RepoIndex -> src/hash.rs::hash_text: 70c099c1622c
  src/index.rs::RepoIndex -> src/imports.rs::FileImports: 7a834189d6b3
  src/index.rs::RepoIndex -> src/lang.rs::detect: 32ef7186ff88
  src/index.rs::RepoIndex -> src/resolve.rs::build_resolver: 640f64019c54
  src/index.rs::RepoIndex -> src/resolve.rs::resolve_default_imports: 39471d06e1f6
  src/index.rs::RepoIndex -> src/stack.rs::StackPack: 436012c73562
  src/index.rs::ingest_build_entry -> src/stack.rs::StackPack: 436012c73562
  src/index.rs::generated_source_entries -> src/stack.rs::StackPack: 436012c73562
---
# src/index.rs
## Summary
This file keeps the repo's index up to date without re-reading everything. It saves, for every source file, exactly what was extracted from it (its declarations, its imports and re-exports, any cross-file links beyond imports, and a fingerprint of its text), so after an edit only the files that really changed are parsed again and the graph is rebuilt from the saved results. `RepoIndex` is that saved copy, stored next to the graph in `.codeowl/index`. It serves two moments: starting up, where `open` hash-checks every file against the saved copy and re-parses only those that moved while nothing was running, and the live session, where `apply_changes`, driven by the file watcher, updates only the paths reported as touched and does nothing when an editor re-saves identical text. A saved copy is thrown away and rebuilt when its format stamp does not match or when the repo's language pack has changed, so stale logic is never trusted. Files that cannot be read are skipped instead of aborting the build. It also reads build-generated source folders (such as Maven's `target/generated-sources`) in a walk of their own, because a project's ignore file hides them from everything else, and it supplies the list of folders the file watcher should watch. `CatchUp` reports which files were added, modified or removed by a refresh, which is how tests show that exactly the changed files were re-read.

## `FileInputs`
`pub struct FileInputs`
### Summary
Everything CodeOwl learns from one source file, stored together so a file that has not changed never has to be parsed again when the index is rebuilt.
### Behavior
Holds `source_hash` (a fingerprint of the file's text, used to detect change), `symbols` (the declarations found), `imports` (the file's import statements and re-exports), and `flow_edges` (the unresolved links the language pack finds beyond imports, such as an API call or a database query, which are resolved against the whole graph later and default to empty when absent from an older cache).

`FileInputs::extract(pack, rel_path, source)` recomputes all of it from the text. It hashes the source and asks the language pack for the symbols. It then offers each symbol to the pack's table check, and any symbol the pack says is a table, such as an ORM model class or an `@Entity`, is retagged as a schema symbol, which is the symbol-level replacement for treating a whole `.sql` file as schema. Finally it branches on the file's source kind: a dedicated schema file gets only the table pass, with empty imports and flow edges, and an ordinary code file (or one the pack has no kind for) also gets the pack's imports and flow edges. It cannot fail; a file that parses badly gives empty lists.
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
A record of exactly which files a rescan or incremental update actually touched — added, modified, or removed — so a caller can confirm the index only reindexed what really changed, not the whole repo.
### Behavior
Three plain lists of file paths: `added`, `modified`, `removed`. `is_empty()` reports whether all three lists are empty (nothing changed at all). `total()` returns the combined count across all three, used for a quick "N files caught up" message. `sorted()` sorts each of the three lists independently and returns `self`, giving a stable, reproducible order regardless of the order files were discovered in during the scan.
### Depends on
- (none)

## `RepoIndex`
`pub struct RepoIndex`
### Summary
Keeps a saved copy of what was extracted from every file in the repo, so starting CodeOwl, or reacting to an edit, only re-reads the files that actually changed and not the whole repo. It then rebuilds the graph from those saved results.
### Behavior
State: `format_version` stamps the on-disk format, and a cache written before the stamp existed reads as 0, which never matches, so `load` forces a full rebuild rather than trusting fields that would read empty. `pack_name` records which language pack built the cache, and if detection now picks a different pack the cached results were parsed by the wrong grammar and are discarded. `files` maps a repo-relative path to its `FileInputs`, in a sorted map so the rebuilt graph's order does not depend on walk or event order. `root` is the absolute repo path and `pack` the chosen language pack, and neither is saved since both are derived from where the cache is read.

Starting: `build(root)` is the cold start. It chooses the pack (failing fast if none recognises the repo), walks and parses every extractable file, and also walks the build-generated source folders separately, since an ordinary ignore file hides them. `open(root)` is the normal start: it loads the cached index, hash-checks every file on disk, re-parses only those that moved while nothing was running, rebuilds and saves the graph, and returns a `CatchUp` describing what changed. With no usable cache it does a full build and reports an empty catch-up, since nothing was compared against. `rescan` and `rescan_entry` do the diffing, comparing each file to its cached hash, re-extracting changed files and dropping deleted ones, and the second is shared by both the main walk and the generated-source walk so they behave identically.

Updating: `apply_changes(paths)` is the watcher's entry point. It takes the paths reported touched and returns a new graph only if at least one really changed an input, so an editor re-saving identical text or a touched non-source file does nothing. `rebuild` constructs the whole graph again from the cached inputs, resolving imports, default imports and flow edges, and saves both the graph and the index. It is cheap because no parsing happens. `watchable_dirs` lists the directories for the file watcher to watch, using the same ignore rules as everything else, so a `node_modules` folder is never watched. Errors from reading, writing or walking are returned.
### Depends on
- `src/graph.rs::FileExtraction` — crate::graph
- `src/graph.rs::FlowEdge` — crate::graph
- `src/graph.rs::Graph` — crate::graph
- `src/hash.rs::hash_text` — crate::hash
- `src/imports.rs::FileImports` — crate::imports
- `src/stack.rs::StackPack` — crate::stack
- `src/lang.rs::detect` — crate::lang
- `src/graph.rs::FORMAT_VERSION` — crate::graph
- `src/resolve.rs::resolve_default_imports` — crate::resolve
- `src/resolve.rs::build_resolver` — crate::resolve
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

## `ingest_build_entry`
`fn ingest_build_entry(
    files: &mut BTreeMap<String, FileInputs>,
    root: &Path,
    pack: &dyn StackPack,
    entry: &ignore::DirEntry,
) -> Result<()>`
### Summary
Handles one file found during a full index build: if it is a source file the language pack reads, it parses it and stores the result. It is shared by both the main walk and the walk of build-generated folders so they behave the same way.
### Behavior
Returns without doing anything unless the entry is a regular file and the pack recognises its type (`source_kind` is not `None`). It then reads the file as text. A file that cannot be read, for example a non-UTF-8 test fixture, which was seen in a real repo, prints a short "skipping" notice to standard error and returns success, so one bad file does not abort the whole build and lose every other file's symbols. For a readable file it computes the repo-relative path and inserts the result of `FileInputs::extract` into the map under that path, replacing any earlier entry for the same path. Unlike the watcher path, a cold build has no earlier state to record a skip in. It returns an error only in the rare case the underlying helpers do.
### Depends on
- `src/stack.rs::StackPack` — crate::stack
- externals: anyhow, std

## `generated_source_entries`
`fn generated_source_entries<'a>(
    candidate_roots: &'a [PathBuf],
    pack: &'a dyn StackPack,
) -> impl Iterator<Item = Result<ignore::DirEntry, ignore::Error>> + 'a`
### Summary
Lists the files inside the build-output folders where generated source code lives, such as Maven's `target/generated-sources`. The normal walk skips those folders because a project's ignore file excludes them, and this is the one place CodeOwl deliberately reads through that.
### Behavior
Takes the folders to look under (`candidate_roots`) and the language pack. For each folder and each generated-source directory name the pack declares, it joins them and keeps the path only if it exists as a folder, so a repo that has never been built simply contributes nothing and this is not an error. Each existing folder is walked with all the ignore filters turned off, so ignored files are included, and the entries are produced lazily as an iterator, each of which may carry a walk error.

`candidate_roots` are the directories the caller's own main walk already visited, not rediscovered here. A real multi-module Maven project keeps its build output per module (`rest-heroes/target/generated-sources` and so on), never only at the repo root, so an early version that checked just the root found nothing in such repos. Reusing the main walk's directories avoids a second full-tree walk, and since each directory is visited once, no folder's output is read twice. It is used by both the full build and the incremental rescan, so generated interfaces appear on a cold start and on later updates alike.
### Depends on
- `src/stack.rs::StackPack` — crate::stack
- externals: anyhow, std
