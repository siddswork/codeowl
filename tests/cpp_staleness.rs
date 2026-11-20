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
    // The class is the home of `size`'s body (pairing moved it out of
    // widget.cc), so the class is what has a spec.
    let class_pair = submit(&graph, &repo.0, "widget.h::Widget", PROSE).unwrap();
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
    // The limit this test used to pin is gone: `Widget::size` in widget.cc
    // reads `n_`, and since pairing it is part of the class, so a change to
    // the class's own text makes the class's spec stale while its users stay
    // current (a private member is no part of the interface they read).
    assert_eq!(
        changes_of_symbol(&graph, &repo.0, "widget.h::Widget", &class_pair),
        ["changed:source"]
    );

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
fn a_body_edit_in_the_source_file_stales_the_class_and_nothing_else() {
    // Step 2b part C, C1: what a user sees after editing an implementation.
    // `Widget::size`'s body lives in widget.cc and belongs to the class.
    let repo = Repo::new("body");
    let graph = repo.graph();
    let class_pair = submit(&graph, &repo.0, "widget.h::Widget", PROSE).unwrap();
    let header_pair = submit(&graph, &repo.0, "widget.h", FILE_PROSE).unwrap();
    let user_pair = submit(&graph, &repo.0, "user.cc::use_widget", PROSE).unwrap();
    let user_file_pair = submit(&graph, &repo.0, "user.cc", FILE_PROSE).unwrap();
    // widget.cc holds only a paired body: no symbol, so nothing to submit.
    assert!(
        graph
            .children_ids(graph.find("widget.cc").unwrap())
            .is_empty()
    );

    repo.write("widget.cc", &IMPL.replace("return n_;", "return n_ + 1;"));
    let graph = repo.graph();
    assert_eq!(
        changes_of_symbol(&graph, &repo.0, "widget.h::Widget", &class_pair),
        ["changed:source"],
        "the class's spec describes the body that changed"
    );
    assert!(
        changes_of_symbol(&graph, &repo.0, "user.cc::use_widget", &user_pair).is_empty(),
        "a user reads the interface, which did not change"
    );
    let user_file = graph.find("user.cc").unwrap();
    assert!(file_changes(&graph, user_file, &user_file_pair).is_empty());
    // P8: the header's file summary describes the header, which is unchanged.
    let header = graph.find("widget.h").unwrap();
    assert!(
        file_changes(&graph, header, &header_pair).is_empty(),
        "the header's file summary stays current"
    );
}

#[test]
fn a_dependency_only_a_body_uses_stales_the_class_and_not_its_users() {
    // C2: `crc.h` is included by widget.cc alone, and only `size`'s body
    // calls `Checksum`.
    let repo = Repo::new("bodydep");
    repo.write("crc.h", "int Checksum(int k);\n");
    repo.write(
        "widget.cc",
        "#include \"widget.h\"\n#include \"crc.h\"\nint Widget::size() { return Checksum(n_); }\n",
    );
    let graph = repo.graph();
    // The edge is the `.cc`'s, so the class's dependency can only come from
    // reading the body in the other file.
    assert!(
        graph
            .imports()
            .iter()
            .any(|e| e.from_file == "widget.cc" && e.imported_name == "Checksum")
    );
    let class_pair = submit(&graph, &repo.0, "widget.h::Widget", PROSE).unwrap();
    let user_pair = submit(&graph, &repo.0, "user.cc::use_widget", PROSE).unwrap();

    repo.write("crc.h", "long Checksum(int k);\n");
    let graph = repo.graph();
    assert_eq!(
        changes_of_symbol(&graph, &repo.0, "widget.h::Widget", &class_pair),
        ["changed:dependencies"],
        "the body's dependency is the class's"
    );
    assert!(
        changes_of_symbol(&graph, &repo.0, "user.cc::use_widget", &user_pair).is_empty(),
        "the user never named Checksum"
    );
}

#[test]
fn a_body_that_starts_calling_a_new_function_stales_the_class_on_its_dependencies() {
    // C4: the call is new, the include is old; only the body changes.
    let repo = Repo::new("newcall");
    repo.write("crc.h", "int Checksum(int k);\n");
    repo.write(
        "widget.cc",
        "#include \"widget.h\"\n#include \"crc.h\"\nint Widget::size() { return n_; }\n",
    );
    let graph = repo.graph();
    let class_pair = submit(&graph, &repo.0, "widget.h::Widget", PROSE).unwrap();
    assert!(changes_of_symbol(&graph, &repo.0, "widget.h::Widget", &class_pair).is_empty());

    repo.write(
        "widget.cc",
        "#include \"widget.h\"\n#include \"crc.h\"\nint Widget::size() { return Checksum(n_); }\n",
    );
    let graph = repo.graph();
    let changes = changes_of_symbol(&graph, &repo.0, "widget.h::Widget", &class_pair);
    assert!(
        changes.contains(&"changed:dependencies".to_string()),
        "{changes:?}"
    );
    assert!(
        changes.contains(&"changed:source".to_string()),
        "{changes:?}"
    );
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
