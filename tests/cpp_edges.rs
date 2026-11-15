//! Part C of the C++ step 2: a C++ repository's graph carries one edge per
//! (file, symbol it names, word), found through the include graph.

use codeowl::graph::{Graph, ResolvedImport};
use codeowl::index::RepoIndex;
use std::path::PathBuf;

fn repo(tag: &str, files: &[(&str, &str)]) -> (PathBuf, Graph) {
    let dir = std::env::temp_dir().join(format!("codeowl-cpp-edges-{}-{tag}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    for (path, text) in files {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, text).unwrap();
    }
    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    (dir, graph)
}

fn edges_from<'a>(graph: &'a Graph, file: &str) -> Vec<&'a ResolvedImport> {
    graph
        .imports()
        .iter()
        .filter(|e| e.from_file == file)
        .collect()
}

fn target_id(graph: &Graph, e: &ResolvedImport) -> String {
    graph
        .string_id(e.target.expect("a resolved edge"))
        .to_string()
}

#[test]
fn a_used_name_becomes_an_edge_with_the_headers_path_as_specifier() {
    let (dir, graph) = repo(
        "fields",
        &[
            ("db.h", "class DB {};\nint unused_fn();\n"),
            ("db.cc", "#include \"db.h\"\nvoid f() { DB d; }\n"),
        ],
    );
    let edges = edges_from(&graph, "db.cc");
    assert_eq!(edges.len(), 1, "only the name the file writes: {edges:?}");
    assert_eq!(edges[0].specifier, "db.h");
    assert_eq!(edges[0].imported_name, "DB");
    assert_eq!(target_id(&graph, edges[0]), "db.h::DB");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_name_only_in_a_comment_or_string_is_no_edge() {
    let (dir, graph) = repo(
        "comment",
        &[
            ("db.h", "class DB {};\n"),
            (
                "a.cc",
                "#include \"db.h\"\n// DB is great\nconst char* s = \"DB\";\n",
            ),
        ],
    );
    assert!(edges_from(&graph, "a.cc").is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_enumerator_links_its_enum_under_the_word_written() {
    let (dir, graph) = repo(
        "enum",
        &[
            ("m.h", "enum Mode { kRead, kWrite };\n"),
            ("a.cc", "#include \"m.h\"\nint x = kWrite;\n"),
        ],
    );
    let edges = edges_from(&graph, "a.cc");
    assert_eq!(edges.len(), 1, "{edges:?}");
    assert_eq!(edges[0].imported_name, "kWrite");
    assert_eq!(target_id(&graph, edges[0]), "m.h::Mode");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_file_naming_an_enum_and_its_enumerator_gets_an_edge_per_word() {
    let (dir, graph) = repo(
        "enum2",
        &[
            ("m.h", "enum Mode { kRead, kWrite };\n"),
            ("a.cc", "#include \"m.h\"\nMode m = kWrite;\n"),
        ],
    );
    let mut words: Vec<&str> = edges_from(&graph, "a.cc")
        .iter()
        .map(|e| e.imported_name.as_str())
        .collect();
    words.sort();
    assert_eq!(words, ["Mode", "kWrite"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_transitive_include_links_straight_to_the_defining_header() {
    let (dir, graph) = repo(
        "transitive",
        &[
            ("c.h", "class Deep {};\n"),
            ("b.h", "#include \"c.h\"\nclass Mid {};\n"),
            ("a.cc", "#include \"b.h\"\nDeep d;\n"),
        ],
    );
    let edges = edges_from(&graph, "a.cc");
    assert_eq!(edges.len(), 1, "{edges:?}");
    assert_eq!(edges[0].specifier, "c.h");
    assert_eq!(target_id(&graph, edges[0]), "c.h::Deep");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_file_has_no_edge_to_its_own_symbols_and_a_cycle_ends() {
    let (dir, graph) = repo(
        "self",
        &[
            ("a.h", "#include \"b.h\"\nclass A { B* b; };\n"),
            ("b.h", "#include \"a.h\"\nclass B { A* a; };\n"),
        ],
    );
    let from_a = edges_from(&graph, "a.h");
    assert_eq!(from_a.len(), 1, "{from_a:?}");
    assert_eq!(target_id(&graph, from_a[0]), "b.h::B");
    let from_b = edges_from(&graph, "b.h");
    assert_eq!(from_b.len(), 1, "{from_b:?}");
    assert_eq!(target_id(&graph, from_b[0]), "a.h::A");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_out_of_class_definition_is_not_a_target() {
    let (dir, graph) = repo(
        "ooc",
        &[
            ("db.h", "class DB { void Open(); };\n"),
            ("db.cc", "#include \"db.h\"\nvoid DB::Open() {}\n"),
            ("use.cc", "#include \"db.cc\"\nvoid f() { Open(); }\n"),
        ],
    );
    // `db.cc::DB::Open` is a symbol, but never the target of an edge (no
    // word a file writes contains `::`, so nothing matches it).
    assert!(
        graph
            .imports()
            .iter()
            .all(|e| !target_id(&graph, e).ends_with("::DB::Open")),
        "{:?}",
        graph.imports()
    );
    // The class is still reached from the implementing file.
    assert!(
        edges_from(&graph, "db.cc")
            .iter()
            .any(|e| target_id(&graph, e) == "db.h::DB")
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_unresolved_include_makes_no_edge() {
    let (dir, graph) = repo(
        "sys",
        &[(
            "a.cc",
            "#include <vector>\nstd::vector<int> v;\nint vector;\n",
        )],
    );
    assert!(graph.imports().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn macros_prototypes_and_typedefs_are_targets() {
    let (dir, graph) = repo(
        "kinds",
        &[
            (
                "u.h",
                "#define LIMIT 10\nint compute(int);\ntypedef int Handle;\n",
            ),
            ("a.cc", "#include \"u.h\"\nHandle h = compute(LIMIT);\n"),
        ],
    );
    let mut got: Vec<String> = edges_from(&graph, "a.cc")
        .iter()
        .map(|e| target_id(&graph, e))
        .collect();
    got.sort();
    assert_eq!(got, ["u.h::Handle", "u.h::LIMIT", "u.h::compute"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_enumerator_inside_a_class_is_not_a_word_that_reaches_the_class() {
    // `buffer_size` is a member constant of `Buf`; another file writing the
    // same word names something of its own, not `Buf`.
    let (dir, graph) = repo(
        "class-enum",
        &[
            (
                "buf.h",
                "class Buf { public: enum { buffer_size = 8 }; };\n",
            ),
            ("a.cc", "#include \"buf.h\"\nint buffer_size = 4;\n"),
        ],
    );
    assert!(edges_from(&graph, "a.cc").is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn naming_the_class_still_links_it_and_a_named_enum_still_links_through_its_constants() {
    // The positive controls of the test above: without them "no edge" would
    // hold if `buf.h` stopped resolving at all.
    let (dir, graph) = repo(
        "class-enum-control",
        &[
            (
                "buf.h",
                "class Buf { public: enum { buffer_size = 8 }; };\n",
            ),
            ("mode.h", "enum Mode { kFast };\n"),
            (
                "a.cc",
                "#include \"buf.h\"\n#include \"mode.h\"\nBuf b; int x = kFast;\n",
            ),
        ],
    );
    let mut got: Vec<(String, String)> = edges_from(&graph, "a.cc")
        .iter()
        .map(|e| (e.imported_name.clone(), target_id(&graph, e)))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            ("Buf".to_string(), "buf.h::Buf".to_string()),
            ("kFast".to_string(), "mode.h::Mode".to_string())
        ]
    );
    std::fs::remove_dir_all(&dir).ok();
}
