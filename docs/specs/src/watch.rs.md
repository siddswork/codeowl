---
kind: file
source_paths: [src/watch.rs]
file: { source_hash: 5e8eb1c334d1728dec8f66cfc3ce75ade2dba0564ff9c7fe392df3edd0249104, deps_hash: 1df7f35b02744f999ddbf18fea87295874dd064996241e6d9206828184ddc205, spec_hash: d524976cb5664eebecbe82eb31c85f86cdbd3e511b944db14887bd9d2218d8a5 }
symbols:
  src/watch.rs::RepoWatcher: { source_hash: 4d5c0ef544e0b633a82d64c394c500df4fd0f9397e8a1bd09afcb6070c84d039, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 8125586fc161a88292dc53d8556ed1dfb57c4eb9ecc0b787265362ebbb13f435 }
  src/watch.rs::spawn: { source_hash: cfc031ad531cf0d19cfa52fe285164a638a6b50026d3cb9dfc720c7bbb74acf4, deps_hash: 1df7f35b02744f999ddbf18fea87295874dd064996241e6d9206828184ddc205, spec_hash: de537dfc926ecc8b4d631fcc4f326c49089b57c5a3767fcfce97578fbb10dbc1 }
  src/watch.rs::watch_loop: { source_hash: 4675d570d6ecc15f05360defd6042e9abd01623b76a48dd77d3d3203c8737d45, deps_hash: 1df7f35b02744f999ddbf18fea87295874dd064996241e6d9206828184ddc205, spec_hash: 46f78b5351c8bf39363da85649a41503db1191b633c2439c3aa69c59f602400c }
  src/watch.rs::collect: { source_hash: 10e08326e50f6e590691b7444d3662ed876c843287a218626b1deccba260249a, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: be0985e33d578cb512fef6ac0f6cb693a3d0c8fa23eafef1e6f4f25ca12c98dc }
---
# src/watch.rs
## Summary
The in-session file watcher (M9) — the second of the two moments `ARCHITECTURE.md`'s "Incremental indexing" describes. For the life of one `codeowl serve` process it keeps the served graph in step with the working directory as the developer edits, so the MCP surface behaves like a language server across a long session rather than something you re-run. `spawn` registers per-directory (non-recursive) watches over the gitignore-visible tree — a recursive watch on the root would blow past the OS inotify limit on a real Next.js repo — and starts one background thread running `watch_loop`, which debounces an editor's save-burst into a single `RepoIndex::apply_changes` rebuild and publishes the result by atomically swapping a fresh `Arc<Graph>` into the `ArcSwap` every request handler reads through. `collect` folds each raw event into the batch and adds a watch to any newly-created directory. There is no clean shutdown in Phase 1 — the thread runs until the process exits.

## `RepoWatcher`
`pub struct RepoWatcher`
### Summary
The handle to the background file-watcher thread. Held by `main.rs` for the life of a `serve` process so the thread isn't dropped.
### Behavior
Wraps only a `JoinHandle`, prefixed `_` because nothing joins it — the thread owns the OS watch and runs until the process exits. There is no clean-shutdown path in Phase 1: `serve` ends only when the client closes stdin. Dropping a `RepoWatcher` detaches the thread rather than stopping it.
### Depends on
- (none)

## `spawn`
`pub fn spawn(root: PathBuf, graph: Arc<ArcSwap<Graph>>, index: RepoIndex) -> Result<RepoWatcher>`
### Summary
Starts the background watcher: registers OS watches on every directory under `root` and spawns the thread that re-parses changed files and republishes the graph. Takes ownership of the already-warm `RepoIndex` from the catch-up pass.
### Behavior
Creates a `notify::RecommendedWatcher` feeding an `mpsc` channel (a send failure is ignored — it just means the loop exited). Registers a **non-recursive** watch per directory from `RepoIndex::watchable_dirs` — non-recursive plus the gitignore-filtered directory list is what keeps `node_modules` unwatched even though it's a subtree of `root`. Then spawns a named thread running `watch_loop`, and returns the `RepoWatcher` handle. A failure to create the watcher, watch a directory, or spawn the thread is a hard error from `serve` startup.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/index.rs::RepoIndex` — crate::index
- externals: anyhow, arc_swap, notify, std

## `watch_loop`
`fn watch_loop(
    rx: mpsc::Receiver<notify::Result<Event>>,
    mut watcher: RecommendedWatcher,
    graph: Arc<ArcSwap<Graph>>,
    mut index: RepoIndex,
)`
### Summary
The watcher thread's body: batch filesystem events through a debounce window, apply them to the index, and atomically republish the graph.
### Behavior
Blocks on `rx.recv()` for the first event, then drains every further event within a `DEBOUNCE` window that *resets on each new event* — so one editor save-burst becomes one rebuild, not many. Events are accumulated into a `HashSet<PathBuf>` (dedup). The batch goes to `index.apply_changes`: `Some((rebuilt, caught))` → `graph.store` the new graph (the atomic swap every MCP handler reads through) and log the count; `None` → nothing spec-relevant changed, no swap; `Err` → logged, loop continues. A `Disconnected` channel ends the thread. `watcher` is moved into this function and explicitly `drop`ped at the end to make the ownership requirement visible — it must outlive the loop or event delivery stops.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/index.rs::RepoIndex` — crate::index
- externals: anyhow, arc_swap, notify, std

## `collect`
`fn collect(
    res: notify::Result<Event>,
    batch: &mut HashSet<PathBuf>,
    watcher: &mut RecommendedWatcher,
)`
### Summary
Folds one filesystem event into the pending batch, and registers a watch on any newly-created directory so edits inside it are seen without a restart.
### Behavior
Ignores an `Err` event. For each path in the event: if it's a `Create` of a directory, add a non-recursive watch on it (errors ignored — a racing delete, a permissions issue), then insert the path into the batch set regardless. `watch_loop`'s `apply_changes` later sorts out create vs modify vs delete by reading (or failing to read) each path — `collect` doesn't need to distinguish them.
### Depends on
- externals: anyhow, notify, std
