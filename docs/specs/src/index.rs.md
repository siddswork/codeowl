---
kind: file
source_paths: [src/index.rs]
file: { source_hash: 25da236d1d71aad7ca5ff7d01bed9cd9e0317e741cff7c27faef60d716d1df7e, deps_hash: 93eda883167e5bac863605a36813f85e5a495af3a93606ec90b4dcbc5ba9a508, spec_hash: 82a98407ccd2d038d65268f94c42295b795f9b2e224bcfada819891638a576e0 }
symbols:
  src/index.rs::FileInputs: { source_hash: 47e412f2023e265aec973a76fa55415237455a745e24eab6170a6fbf3451b8db, deps_hash: 1623a816d0d741f900c9c77e8c39d4d9d041c185410c6dfe941cd80dd2c7b456, spec_hash: a2cd33f6c64045cddeafff3f73c818456a0ea43330e704e8bc1edf950761cc7d }
  src/index.rs::CatchUp: { source_hash: 07805fea5febc54c573e8083754584ac2d977962598fdd6afd4b01dca8bb27b6, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 52e10ce4b7920943505c7dffaf7c32c0086febee8f384aa2004fa98bb9c4568e }
  src/index.rs::RepoIndex: { source_hash: 37aaea67d9aa2a57a58fdecf0b372f0a70f75033a17881f120665cf28d986a6b, deps_hash: 6781fd3bc3eda72c5dd88b4964c1369a538dec09e815f6d294ff3ca295a9e3c6, spec_hash: 7b35019a573085b15ae37bcf3723e6197dd5e51653a01c0480da79041106aede }
  src/index.rs::canonical_root: { source_hash: a12afe5bc286ba94b312c3a2ed43df1be87659af6c1dc03d27fb702901f0ad8e, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c006dd5c57e68d23df0160e998922dd80f01dcdcbe723ac1f3913c9cc2c77ff8 }
  src/index.rs::canonicalize_event_path: { source_hash: 3bebe39ce6f00d1ab58958d5e87cf298bb3fcdc0f44186a7170e195f8e6ad359, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: bed0673d7e2ef62cc32f60efe539242a27ec17aee128e91c10688a71bfa63ab8 }
  src/index.rs::rel_path: { source_hash: 8fb5e79ec2f2f3bc5d8fed6fdbb54b303b47381e0bd2943f80b3745a47e1ee9c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b887526579e14f7161923631b7b35c6d45fe67e3905af60c966a59b04e051225 }
  src/index.rs::ingest_build_entry: { source_hash: 63ba0acdc73a1e4f3d4604afa6a49459fe0449b7e967731ea531d37ebf021354, deps_hash: f5afa2f13a5fae7ce9579309271426d63ca953f7caf6f86202a89f7f91e5a00f, spec_hash: 1ed7ee10f547817373ff2abb2858029e7108bb73c26332f81e113c887f104ea3 }
  src/index.rs::generated_source_entries: { source_hash: 13f83892670cc66f4eb2da9852c203b725178b2d9f14664a078a547198ded4e1, deps_hash: f5afa2f13a5fae7ce9579309271426d63ca953f7caf6f86202a89f7f91e5a00f, spec_hash: 39836eaf94e3f79dd80f32baa688ef9e0052771c23eb875e1c8c9e9b826befc1 }
---
# src/index.rs
## Summary
This file implements incremental indexing: it caches, per file, exactly what was extracted from it (its symbols, imports, and any cross-cutting flow edges) plus a hash of that file's contents, so a rebuild after an edit only re-parses the files that actually changed rather than the whole repo. `RepoIndex` is the cache itself, persisted to `.codeowl/index` alongside the graph it can rebuild from those cached inputs. This backbone serves two moments: a fresh process starting up (`RepoIndex::open`, which hash-checks every file against the cache and only re-parses what moved since the last run) and the in-session file watcher (`RepoIndex::apply_changes`, driven from `watch.rs`, which reindexes only the specific paths reported as touched). It also walks any build-tool-generated source directories (like Maven's `target/generated-sources`) separately from the normal repo walk, since a repo's own `.gitignore` deliberately excludes those directories from everything else.

## `FileInputs`
`pub struct FileInputs`
### Summary
Everything CodeOwl caches about one source file so it never has to re-parse and re-extract a file that hasn't changed since the last time it ran.
### Behavior
Holds a file's `source_hash` (used to detect whether the file changed at all since it was last cached), its extracted `symbols`, its `imports` (already-parsed but not-yet-resolved import statements), and its `flow_edges` — flow edges (a cross-cutting reference this stack's pack knows how to spot, like a UI component rendering a route) that are still unresolved at this point and only get matched up against the whole graph later, during `rebuild`. `flow_edges` defaults to an empty list when deserializing an older cache written before this field existed (`#[serde(default)]`), so an existing `.codeowl` cache doesn't break just because a new field was added.

Its associated `extract` function builds one of these from scratch for a file: it hashes the source, asks the language-specific `pack` (a `StackPack` implementation — the trait that knows how to parse one particular language/framework) to extract the file's symbols, then reclassifies any symbol the pack recognizes as a schema declaration (a SQL table, say) to `SymbolKind::Schema` if it wasn't already tagged that way. What happens next depends on the file's `SourceKind`: a schema file (e.g. a `.sql` file) gets empty `imports`/`flow_edges`, since a schema file doesn't import anything or participate in cross-cutting flows the way ordinary code does; any other file (`Code`, or unclassified) gets its imports and flow edges extracted from the pack as normal.
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
The on-disk cache of everything CodeOwl has already parsed and extracted from a repo — what makes starting up on a repo you've indexed before fast, since only files that actually changed since last time need re-parsing.
### Behavior
Holds, per repo-relative file path, that file's cached `FileInputs` (in a `BTreeMap`, so the graph built from it always comes out in the same deterministic order regardless of filesystem walk order). It also records `format_version` and `pack_name` as stamps: if the on-disk cache's format is older than what this build expects, or if `lang::detect` now picks a different stack pack (framework/language detector) than the one this cache was built with, `load` treats the whole cache as unusable and forces a full rebuild rather than trusting inputs parsed by the wrong grammar. `root` (the absolute repo path) and `pack` (the detected `StackPack`, boxed so `RepoIndex` stays a concrete type usable from `watch.rs`) are both machine/run-specific and never serialized — they're set fresh from context every time the cache is loaded or built.

Key entry points:
- `build(root)` does a full walk and parse of every extractable file — the cold-start path, used whenever there's no usable cache yet.
- `open(root)` is the normal startup path: load the cached index, hash-check every file on disk, re-parse only what changed while nothing was running, then rebuild and persist the graph. Falls back to `build` when there's no usable cache, reporting an empty `CatchUp` in that case (there's nothing to diff against on a genuine first run).
- `rescan()` walks the tree, diffing every extractable file's current hash against its cached one, re-extracting whatever moved and dropping whatever's gone; `rescan_entry` is the shared per-file logic both the main walk and a generated-source walk drive identically.
- `apply_changes(paths)` is the file-watcher-driven path: given the absolute paths the watcher reported as touched, it only triggers a rebuild (`Some`) if at least one path actually changed something the graph depends on — an editor resaving identical content, or a touch to a non-source file, is a genuine no-op that returns `None`.
- `rebuild()` reconstructs the whole `Graph` from whatever's currently cached and re-persists both the graph and the index to `.codeowl/`. This step is cheap — arena construction and import resolution only, since every file's symbols and imports are already sitting in the cache; no parsing happens here.
- `watchable_dirs(root)` lists every directory the file watcher should register on — only the part of the tree visible under `.gitignore` rules, so something like `node_modules` in a real repo is never watched.
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

## `ingest_build_entry`
`fn ingest_build_entry(
    files: &mut BTreeMap<String, FileInputs>,
    root: &Path,
    pack: &dyn StackPack,
    entry: &ignore::DirEntry,
) -> Result<()>`
### Summary
Processes one file found while walking the repo during a full build: reads it, extracts its symbols, and inserts the result into the in-progress cache — skipping anything that isn't a real, extractable source file.
### Behavior
Returns immediately (doing nothing) if the walked entry isn't a plain file (e.g. it's a directory), or if the stack pack (the language/framework-specific extractor) says this path's `source_kind` is `None` — meaning the pack doesn't recognize it as a file worth extracting at all. Otherwise it reads the file's contents to a string (surfacing a read error with `.context(...)` naming the path, if the read fails), computes its path relative to the repo root, and inserts a freshly built `FileInputs` for it into the `files` map, keyed by that relative path — overwriting any existing entry for the same path.

This logic is factored out of `RepoIndex::build` specifically so that both the primary directory walk and a separate generated-source walk (for files a build tool produces, which live outside the normal source tree) can drive the exact same extract-and-insert steps instead of each maintaining its own copy.
### Depends on
- `src/stack.rs::StackPack` — crate::stack
- externals: anyhow, std

## `generated_source_entries`
`fn generated_source_entries<'a>(
    candidate_roots: &'a [PathBuf],
    pack: &'a dyn StackPack,
) -> impl Iterator<Item = Result<ignore::DirEntry, ignore::Error>> + 'a`
### Summary
Finds every file sitting inside a build tool's generated-source directories (like Maven's `target/generated-sources`) across the whole repo, so code that only exists after a real build — a generated JAX-RS resource, say — still gets indexed.
### Behavior
Asks the stack `pack` (the language/framework-specific extractor) which subdirectory names it considers generated-source directories, then, for each of `candidate_roots`, joins each of those directory names on and keeps only the ones that actually exist on disk (`is_dir()`) — a repo that hasn't been built locally simply won't have one yet, which is treated as normal, not an error. Each surviving directory is walked with `ignore::WalkBuilder` configured with `standard_filters(false)`, meaning `.gitignore` rules are deliberately *not* applied here — the whole point of a generated-source directory is that a repo's own `.gitignore` normally excludes it (`target/`, `build/`), and this is the one place CodeOwl reads through that exclusion on purpose.

`candidate_roots` is not rediscovered by walking the repo again — it's handed in from the caller's own primary directory walk, which already visits every directory in the repo as part of finding files. A real multi-module build (where generated sources live per-module, e.g. `rest-heroes/target/generated-sources`, never once at the repo root) needs exactly this: checking only the repo root would silently find nothing on a multi-module repo, and doing a second full-repo walk to rediscover module boundaries would walk the tree twice for no reason. Reusing the primary walk's already-visited directories avoids both problems, and — since that walk visits each directory exactly once — also avoids ever checking the same directory's generated-source subfolder twice.
### Depends on
- `src/stack.rs::StackPack` — crate::stack
- externals: anyhow, std
