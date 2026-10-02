---
kind: file
source_paths: [src/watch.rs]
file: { source_hash: 5a233068bb9858d0af025543cd933f7a6552e39e98d4b90ae1326c5bb2f369b3, deps_hash: aec1102fdf89ae067eb2fba4cc0acad666f9a5a607fcab4c157bffe92cd5df8c, spec_hash: 7bb2d2c9cbeb9f34e907198177e01513bde421264895192a7a4bee03e5ee5cde }
symbols:
  src/watch.rs::RepoWatcher: { source_hash: a687f77bc819f3760626650217f2d7c9c795170040e5025d942fe9944ac9c958, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: ef16605ce4789d950d618b629e4be09526234f78f39c434c5bec28b808961dfc }
  src/watch.rs::spawn: { source_hash: 53d8e42152c83bbfe3e230d000b76a3dce79065de996cb02eaf4701a8b0ef227, deps_hash: aec1102fdf89ae067eb2fba4cc0acad666f9a5a607fcab4c157bffe92cd5df8c, spec_hash: cc326badc569dd861fd22a2afe7b73eb1b461ae03d67d2eece89a725f2f1c63b }
  src/watch.rs::watch_loop: { source_hash: 4675d570d6ecc15f05360defd6042e9abd01623b76a48dd77d3d3203c8737d45, deps_hash: aec1102fdf89ae067eb2fba4cc0acad666f9a5a607fcab4c157bffe92cd5df8c, spec_hash: 6d60fa759bbffed5f9c610ccad0f739b0bd8d221898ffac59979d1a5cdabf73b }
  src/watch.rs::collect: { source_hash: 10e08326e50f6e590691b7444d3662ed876c843287a218626b1deccba260249a, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: be0985e33d578cb512fef6ac0f6cb693a3d0c8fa23eafef1e6f4f25ca12c98dc }
---
# src/watch.rs
## Summary
This is the in-session file watcher. While `codeowl serve` runs, it keeps the served graph in step with the working directory as the developer edits, so the tools behave like a language server across a long session, not something to re-run. `spawn` starts a background thread, and that thread collects file-system events and waits a short moment (300 milliseconds) after the last one, so a burst of saves from an editor becomes a single update. It hands the changed paths to `RepoIndex::apply_changes`, which re-reads only what changed, and, only if something really changed, swaps the freshly rebuilt graph into a shared atomic cell that the server reads. Request handlers therefore never wait for the watcher and never see a half-built graph, and each works from one consistent snapshot. Watches are placed on each folder in the ignore-aware tree one by one, not recursively on the repo root, because a recursive watch would also cover `node_modules` and exceed the operating system's limit on watches in a real project. A folder created mid-session is picked up and watched on the spot, and a failed update is reported to standard error without stopping later ones. `RepoWatcher` is a handle to the thread. Dropping it only detaches the thread, which runs until the process ends, so it is a keep-alive and not a stop switch.

## `RepoWatcher`
`pub struct RepoWatcher`
### Summary
A handle to the background thread that watches the repo for edits while the server runs.
### Behavior
Holds the join handle of the watcher thread, which owns the operating system's file watch and runs for the life of the process. Dropping the handle does not stop the thread; it simply detaches it, because there is no clean-shutdown path at present: `serve` only ends when the client closes its input and the process exits, which ends the thread with it. The handle is therefore a keep-alive marker and not a stop switch, and `main.rs` keeps it bound to a variable so it is not discarded immediately. The struct is created only by `spawn` and has no methods.
### Depends on
- (none)

## `spawn`
`pub fn spawn(root: PathBuf, graph: Arc<ArcSwap<Graph>>, index: RepoIndex) -> Result<RepoWatcher>`
### Summary
Starts watching the repo for file changes in the background and keeps the server's graph up to date as source files are edited, so the tools behave like a language server across a long session.
### Behavior
Makes the root path absolute and symlink-free first, so the folders it registers watches on are under the same path the operating system will report events for, and so it matches the form the index strips off event paths. If canonicalising fails it uses the root as given. It creates a channel and a recommended file watcher from the `notify` library whose callback just sends each event into the channel; a failed send only means the loop has already exited and is ignored. It registers a non-recursive watch on every folder returned by `RepoIndex::watchable_dirs`, which uses the repo's ignore rules, so a `node_modules` folder is never watched. Each one is registered separately, and a failure to watch any folder is returned with the folder named.

It then starts a thread named `codeowl-watch` that runs the watch loop, handing it the receiving end of the channel, the watcher itself so the watch stays alive, the shared graph cell, and the index, which the thread takes ownership of and updates in place on every change. It returns a handle holding that thread. Errors creating the watcher, registering a watch, or starting the thread are returned with context. The watch loop is what debounces bursts of saves and swaps in the rebuilt graph.
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
The background loop that waits for file changes, groups a quick burst of them into one update, re-reads only what changed, and publishes the refreshed graph so the server sees it immediately.
### Behavior
Blocks until the first event arrives, then adds the paths from it to a set (`collect`, which also starts watching any directory newly created mid-session). It then keeps draining events that arrive within the debounce window, restarting the wait for each new one, so one editor save burst, which can write several times, produces a single rebuild. If the wait times out it stops collecting, and if the sending side has disconnected it exits the thread.

With the batch of paths it calls `index.apply_changes`. If at least one change really affected an input, it receives a rebuilt graph and the list of what changed, swaps the new graph into the shared cell so every request handler sees it from then on, and prints how many files were re-indexed to standard error. If nothing relevant changed, such as a touched non-source file or identical text, nothing happens. An error is printed to standard error and the loop continues, so one failed update does not stop later ones. Request handlers never block on this thread, because they read the graph snapshot atomically.

The watcher is passed in and kept alive for the whole loop, since event delivery would stop if it were dropped. The end of the function, which drops it explicitly, is not reachable in practice, because the callback holds the only sender and lives inside the watcher, but it makes that ownership visible. Nothing is returned.
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
