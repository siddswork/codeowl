//! M9 end-to-end: the in-session file watcher keeps the served graph in
//! step with edits to the working directory, without restarting the server
//! (`ROADMAP.md`'s M9 validation).

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use codeowl::index::RepoIndex;
use codeowl::mcp::CodeOwlServer;

fn tempdir(tag: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("codeowl-m9-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn source_hash(graph: &codeowl::Graph, id: &str) -> String {
    graph
        .get_symbol(graph.find(id).expect("symbol present"))
        .unwrap()
        .source_hash
        .clone()
}

#[test]
fn watcher_reindexes_an_edited_file_without_restart() {
    let dir = tempdir("watch");
    std::fs::write(
        dir.join("a.ts"),
        "export function f(): number { return 1; }\n",
    )
    .unwrap();

    let (index, graph, _) = RepoIndex::open(&dir).unwrap();
    let before = source_hash(&graph, "a.ts::f");

    let server = CodeOwlServer::new(dir.clone(), graph);
    let store = server.graph_store();
    // `spawn` registers the directory watches before it returns, so an edit
    // right after this is guaranteed to be seen.
    let _watcher = codeowl::watch::spawn(dir.clone(), store.clone(), index).unwrap();

    std::fs::write(
        dir.join("a.ts"),
        "export function f(): number { return 42; }\n",
    )
    .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let g = store.load_full();
        if g.find("a.ts::f").is_some() && source_hash(&g, "a.ts::f") != before {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "watcher never republished the edited graph"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn watcher_picks_up_a_newly_created_file() {
    let dir = tempdir("watch-add");
    std::fs::write(dir.join("lib.ts"), "export function helper() {}\n").unwrap();

    let (index, graph, _) = RepoIndex::open(&dir).unwrap();
    let server = CodeOwlServer::new(dir.clone(), graph);
    let store = server.graph_store();
    let _watcher = codeowl::watch::spawn(dir.clone(), store.clone(), index).unwrap();

    std::fs::write(dir.join("app.ts"), "import { helper } from './lib';\n").unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let g = store.load_full();
        let resolved = g
            .find("lib.ts::helper")
            .is_some_and(|target| g.imports().iter().any(|i| i.target == Some(target)));
        if resolved {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "watcher never resolved the new importer's edge"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_fresh_spawn_reuses_the_persisted_index() {
    let dir = tempdir("respawn");
    std::fs::write(dir.join("a.ts"), "export const a = 1;\n").unwrap();
    std::fs::write(dir.join("b.ts"), "export const b = 2;\n").unwrap();

    // First "process": builds and persists `.codeowl/index`.
    RepoIndex::open(&dir).unwrap();
    assert!(dir.join(".codeowl").join("index").exists());

    // Edit while "nothing is running", then a second "process" starts.
    std::fs::write(dir.join("b.ts"), "export const b = 222;\n").unwrap();
    let (_index, graph, caught) = RepoIndex::open(&dir).unwrap();

    assert_eq!(caught.modified, vec!["b.ts"]);
    assert!(caught.added.is_empty() && caught.removed.is_empty());
    assert!(graph.find("a.ts::a").is_some());
    assert!(graph.find("b.ts::b").is_some());
}
