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
const PROSE: &str = "### Summary\nReturns the size of the widget it is given.\n### Behavior\nAsks the widget for its size and hands that value back unchanged.\n";
const FILE_PROSE: &str = "Wraps the widget class for callers that only need its size.";
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
    // The edge exists, so the "nobody moves" steps below mean something.
    assert!(
        graph
            .imports()
            .iter()
            .any(|e| e.from_file == "user.cc" && e.imported_name == "Widget")
    );
    let sym_pair = submit(&graph, &repo.0, "user.cc::use_widget", PROSE).unwrap();
    let file_pair = submit(&graph, &repo.0, "user.cc", FILE_PROSE).unwrap();
    let impl_pair = submit(&graph, &repo.0, "widget.cc::Widget::size", PROSE).unwrap();
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
    // KNOWN LIMIT until pairing (step 2b): `Widget::size` in widget.cc reads
    // `n_`, but a private member is no part of the class's interface hash and
    // the method is its own symbol, so it stays current. Pairing should make
    // this stale; when it does, change this assertion.
    assert!(changes_of_symbol(&graph, &repo.0, "widget.cc::Widget::size", &impl_pair).is_empty());

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
fn the_class_is_reached_by_one_edge_from_the_including_and_the_implementing_file() {
    let repo = Repo::new("callers");
    let graph = repo.graph();
    let id = graph.find("widget.h::Widget").unwrap();
    let mut edges: Vec<(&str, &str)> = graph
        .imports()
        .iter()
        .filter(|e| e.target == Some(id))
        .map(|e| (e.from_file.as_str(), e.imported_name.as_str()))
        .collect();
    edges.sort();
    // One edge per file and word, no duplicate.
    assert_eq!(edges, [("user.cc", "Widget"), ("widget.cc", "Widget")]);
}

#[test]
fn a_comment_only_header_edit_stales_nobody() {
    let repo = Repo::new("comment");
    let graph = repo.graph();
    let sym_pair = submit(&graph, &repo.0, "user.cc::use_widget", PROSE).unwrap();
    let file_pair = submit(&graph, &repo.0, "user.cc", FILE_PROSE).unwrap();
    // The edge must exist, or "nobody is stale" would hold for any pack.
    assert!(graph.imports().iter().any(|e| e.from_file == "user.cc"));

    repo.write(
        "widget.h",
        &HEADER.replace("  int size();", "  // How many items.\n  int size();   "),
    );
    let graph = repo.graph();
    let file_id = graph.find("user.cc").unwrap();
    assert!(changes_of_symbol(&graph, &repo.0, "user.cc::use_widget", &sym_pair).is_empty());
    assert!(file_changes(&graph, file_id, &file_pair).is_empty());
}

#[test]
fn a_transitive_dependant_goes_stale_through_the_header_in_between() {
    let repo = Repo::new("transitive");
    repo.write("mid.h", "#include \"widget.h\"\nclass Mid {};\n");
    repo.write(
        "far.cc",
        "#include \"mid.h\"\nint far(Widget& w) { return w.size(); }\n",
    );
    let graph = repo.graph();
    let sym_pair = submit(&graph, &repo.0, "far.cc::far", PROSE).unwrap();
    assert!(changes_of_symbol(&graph, &repo.0, "far.cc::far", &sym_pair).is_empty());

    // `mid.h` never changes; the edit is two includes away.
    repo.write("widget.h", &HEADER.replace("int size();", "long size();"));
    let graph = repo.graph();
    assert_eq!(
        changes_of_symbol(&graph, &repo.0, "far.cc::far", &sym_pair),
        ["changed:dependencies"]
    );
}
