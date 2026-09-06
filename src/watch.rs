//! The in-session file watcher (M9) — the second of the two moments
//! `ARCHITECTURE.md`'s "Incremental indexing" describes. For the lifetime
//! of one `codeowl serve` process, this keeps the served graph in step with
//! the working directory as the developer edits it, so the MCP surface
//! behaves like a language server across a multi-hour session rather than
//! something you re-run.
//!
//! Shape follows MemoLink's `GraphWatchService`: one background thread
//! draining filesystem events, a short debounce so an editor's burst of
//! saves collapses into a single rebuild, and per-directory (not recursive)
//! watches over the gitignore-visible tree — a recursive watch on the repo
//! root would also register `node_modules` and blow past the OS's inotify
//! watch limit on a real Next.js repo.
//!
//! The rebuild itself is `RepoIndex::apply_changes` (re-parses only what
//! changed); the result is published by swapping a fresh `Arc<Graph>` into
//! the `ArcSwap` the server reads. `ArcSwap` gives a lock-free atomic
//! pointer swap — request handlers loading a snapshot never block on the
//! watcher and vice versa, which is what we want when the whole graph is
//! replaced wholesale and a handler only needs a consistent view for the
//! duration of one call.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::graph::Graph;
use crate::index::RepoIndex;

/// How long to wait for the filesystem to go quiet before rebuilding. Long
/// enough to collapse an editor's write burst, short enough to feel live.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// Handle to the background watcher thread. The thread owns the OS watch
/// and runs for the lifetime of the process — dropping this handle just
/// detaches it (there's no clean-shutdown path in Phase 1, since `serve`
/// only ever ends when the client closes stdin and the process exits).
/// `main.rs` keeps it bound so the handle isn't immediately discarded.
pub struct RepoWatcher {
    _thread: std::thread::JoinHandle<()>,
}

/// Start watching `root` and republishing `graph` as its source files
/// change. `index` is the already-warm index from the catch-up pass — the
/// watcher takes ownership and mutates it in place on every change.
pub fn spawn(root: PathBuf, graph: Arc<ArcSwap<Graph>>, index: RepoIndex) -> Result<RepoWatcher> {
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        // A send failure just means the loop has exited — nothing to do.
        let _ = tx.send(res);
    })
    .context("creating the file watcher")?;

    for dir in RepoIndex::watchable_dirs(&root)? {
        watcher
            .watch(&dir, RecursiveMode::NonRecursive)
            .with_context(|| format!("watching {}", dir.display()))?;
    }

    let thread = std::thread::Builder::new()
        .name("codeowl-watch".into())
        .spawn(move || watch_loop(rx, watcher, graph, index))
        .context("spawning the watcher thread")?;

    Ok(RepoWatcher { _thread: thread })
}

fn watch_loop(
    rx: mpsc::Receiver<notify::Result<Event>>,
    mut watcher: RecommendedWatcher,
    graph: Arc<ArcSwap<Graph>>,
    mut index: RepoIndex,
) {
    while let Ok(first) = rx.recv() {
        let mut batch: HashSet<PathBuf> = HashSet::new();
        collect(first, &mut batch, &mut watcher);

        // Drain everything that lands within the debounce window, resetting
        // it on each new event, so one editor save-burst is one rebuild.
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(res) => collect(res, &mut batch, &mut watcher),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }

        let paths: Vec<PathBuf> = batch.into_iter().collect();
        match index.apply_changes(&paths) {
            Ok(Some((rebuilt, caught))) => {
                graph.store(Arc::new(rebuilt));
                eprintln!("codeowl: reindexed {} file(s) after edit", caught.total());
            }
            Ok(None) => {}
            Err(e) => eprintln!("codeowl: incremental reindex failed: {e:#}"),
        }
    }
    // Unreachable in practice (the notify callback holds the only sender,
    // and it lives here in `watcher`), but makes the ownership explicit:
    // `watcher` must outlive the loop or event delivery stops.
    drop(watcher);
}

/// Fold one filesystem event into the pending batch, and register a watch
/// on any directory that was just created so edits inside it are seen
/// without restarting.
fn collect(
    res: notify::Result<Event>,
    batch: &mut HashSet<PathBuf>,
    watcher: &mut RecommendedWatcher,
) {
    let Ok(event) = res else {
        return;
    };
    for path in event.paths {
        if matches!(event.kind, EventKind::Create(_)) && path.is_dir() {
            let _ = watcher.watch(&path, RecursiveMode::NonRecursive);
        }
        batch.insert(path);
    }
}
