---
kind: file
source_paths: [src/watch.rs]
file: { source_hash: 5a233068bb9858d0af025543cd933f7a6552e39e98d4b90ae1326c5bb2f369b3, deps_hash: 1df7f35b02744f999ddbf18fea87295874dd064996241e6d9206828184ddc205, spec_hash: 094482d1a876dce40fbb4c6a7a60e5ef963f7525391bf09babb611fe56bc1dfa }
symbols:
  src/watch.rs::RepoWatcher: { source_hash: 4d5c0ef544e0b633a82d64c394c500df4fd0f9397e8a1bd09afcb6070c84d039, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 8125586fc161a88292dc53d8556ed1dfb57c4eb9ecc0b787265362ebbb13f435 }
  src/watch.rs::spawn: { source_hash: 53d8e42152c83bbfe3e230d000b76a3dce79065de996cb02eaf4701a8b0ef227, deps_hash: 1df7f35b02744f999ddbf18fea87295874dd064996241e6d9206828184ddc205, spec_hash: 54a13ba501e76251fa5f9b020f209a34ce2340f6668484e4b67b6db366406a74 }
  src/watch.rs::watch_loop: { source_hash: 4675d570d6ecc15f05360defd6042e9abd01623b76a48dd77d3d3203c8737d45, deps_hash: 1df7f35b02744f999ddbf18fea87295874dd064996241e6d9206828184ddc205, spec_hash: 46f78b5351c8bf39363da85649a41503db1191b633c2439c3aa69c59f602400c }
  src/watch.rs::collect: { source_hash: 10e08326e50f6e590691b7444d3662ed876c843287a218626b1deccba260249a, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: be0985e33d578cb512fef6ac0f6cb693a3d0c8fa23eafef1e6f4f25ca12c98dc }
---
# src/watch.rs
## Summary
The in-session file watcher. While `codeowl serve` is running, this keeps the served graph in step with the working directory as the developer edits — so the MCP tools behave like a language server across a long session rather than something you re-run. `spawn` starts a background thread; that thread drains filesystem events, waits a short debounce (300 ms) so a burst of editor saves becomes a single rebuild, hands the changed paths to `RepoIndex::apply_changes`, and — only if something actually changed — swaps the freshly rebuilt graph into the atomic cell the server reads from, so request handlers never block on the watcher. A directory created mid-session is picked up and watched on the fly. `RepoWatcher` is the handle that keeps the thread and the OS watch alive; dropping it (when `serve` exits) stops watching.

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
Starts the background file-watcher. From here on, whenever a source file in the repo changes, CodeOwl re-indexes just that file and swaps the updated graph in — so the served data stays live across a long session without a restart.
### Behavior
Canonicalizes `root` first (resolving symlinks) so the directories it watches are named the same way the operating system will report events under them — without this, a symlinked root would make every event fail to match and get dropped.

Sets up a filesystem watcher (the `notify` crate) whose callback forwards each raw event down a channel. Registers a *non-recursive* watch on every directory in the repo, from `RepoIndex::watchable_dirs` (which honors `.gitignore`) — non-recursive on purpose, because a recursive watch on the root would also cover `node_modules` and blow past the OS's per-process watch limit on a real project. Then spawns a background thread running `watch_loop`, which drains events, collapses a burst of editor saves into one rebuild, calls `RepoIndex::apply_changes`, and publishes the fresh graph. The returned `RepoWatcher` keeps the thread and the OS watch alive; dropping it (when `serve` exits) stops watching.
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
