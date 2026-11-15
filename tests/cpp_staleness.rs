//! Part D of the C++ step 2, end to end: a header edit makes stale exactly
//! the dependants that name what changed, and `get_callers`' filter (an
//! edge whose target is the symbol) lists the including and the
//! implementing file. Pattern of `tests/java_spec.rs`.

use codeowl::graph::Graph;
use codeowl::index::RepoIndex;
use codeowl::spec::{file_changes, submit, symbol_changes};
use std::path::{Path, PathBuf};

const HEADER: &str =
    "class Widget {\npublic:\n  int size();\nprivate:\n  int n_;\n};\nint other_fn();\n";
const USER: &str = "#include \"widget.h\"\nint use_widget(Widget& w) { return w.size(); }\n";
const IMPL: &str = "#include \"widget.h\"\nint Widget::size() { return n_; }\n";

struct Repo(PathBuf);

impl Repo {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("codeowl-cpp-stale-{}-{tag}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        for (p, t) in [("widget.h", HEADER), ("user.cc", USER), ("widget.cc", IMPL)] {
            std::fs::write(dir.join(p), t).unwrap();
        }
        Repo(dir)
    }

    fn graph(&self) -> Graph {
        RepoIndex::open(&self.0).unwrap().1
    }

    fn write(&self, path: &str, text: &str) {
        std::fs::write(self.0.join(path), text).unwrap();
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn changes_of_symbol(
    graph: &Graph,
    root: &Path,
    id: &str,
    stored: &codeowl::spec::HashPair,
) -> Vec<String> {
    let sym_id = graph.find(id).unwrap();
    let file_id = graph.find(graph.owning_file_id(sym_id)).unwrap();
    symbol_changes(
        graph,
        root,
        file_id,
        graph.get_symbol(sym_id).unwrap(),
        stored,
    )
}

#[test]
fn a_header_edit_stales_only_the_dependants_that_name_what_changed() {
    let repo = Repo::new("stale");
    let graph = repo.graph();
    let prose = "### Summary\nReturns the size of the widget it is given.\n### Behavior\nAsks the widget for its size and hands that value back unchanged.\n";
    let sym_pair = submit(&graph, &repo.0, "user.cc::use_widget", prose).unwrap();
    let file_pair = submit(
        &graph,
        &repo.0,
        "user.cc",
        "Wraps the widget class for callers that only need its size.",
    )
    .unwrap();
    let file_id = graph.find("user.cc").unwrap();
    assert!(changes_of_symbol(&graph, &repo.0, "user.cc::use_widget", &sym_pair).is_empty());
    assert!(file_changes(&graph, file_id, &file_pair).is_empty());

    // An unrelated symbol of the header: neither the symbol nor its file moves.
    repo.write(
        "widget.h",
        &HEADER.replace("int other_fn();", "long other_fn();"),
    );
    let graph = repo.graph();
    let file_id = graph.find("user.cc").unwrap();
    assert!(
        changes_of_symbol(&graph, &repo.0, "user.cc::use_widget", &sym_pair).is_empty(),
        "an unrelated header symbol must not stale the user"
    );
    assert!(file_changes(&graph, file_id, &file_pair).is_empty());

    // A private member of the class is no part of its interface.
    repo.write("widget.h", &HEADER.replace("int n_;", "long n_;"));
    let graph = repo.graph();
    assert!(changes_of_symbol(&graph, &repo.0, "user.cc::use_widget", &sym_pair).is_empty());

    // A public member's signature: the user symbol and its file both move.
    repo.write("widget.h", &HEADER.replace("int size();", "long size();"));
    let graph = repo.graph();
    let file_id = graph.find("user.cc").unwrap();
    assert_eq!(
        changes_of_symbol(&graph, &repo.0, "user.cc::use_widget", &sym_pair),
        ["changed:dependencies"]
    );
    assert_eq!(
        file_changes(&graph, file_id, &file_pair),
        ["changed:dependencies"]
    );
}

#[test]
fn callers_of_the_class_are_the_including_and_the_implementing_file() {
    let repo = Repo::new("callers");
    let graph = repo.graph();
    let id = graph.find("widget.h::Widget").unwrap();
    // `get_callers` is exactly this filter over the graph's edges.
    let mut callers: Vec<&str> = graph
        .imports()
        .iter()
        .filter(|e| e.target == Some(id))
        .map(|e| e.from_file.as_str())
        .collect();
    callers.sort();
    assert_eq!(callers, ["user.cc", "widget.cc"]);
}
