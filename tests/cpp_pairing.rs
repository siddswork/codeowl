//! Part B of C++ step 2b through the real index: a definition in a source file
//! is paired with the class its header declares when the graph is built, and
//! an edit to either file re-pairs.

use codeowl::graph::Graph;
use codeowl::index::RepoIndex;
use std::path::PathBuf;

const DB_H: &str =
    "namespace leveldb {\nclass DB {\n public:\n  int Get(int k);\n  int Put(int k);\n};\n}\n";
const DB_CC: &str = "#include \"db.h\"\nnamespace leveldb {\nint DB::Get(int k) {\n  return k;\n}\nint DB::Put(int k) {\n  return k + 1;\n}\n}\n";

struct Repo(PathBuf);

impl Repo {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("codeowl-cpp-pairing-{}-{tag}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("db.h"), DB_H).unwrap();
        std::fs::write(dir.join("db.cc"), DB_CC).unwrap();
        Repo(dir)
    }

    fn graph(&self) -> Graph {
        RepoIndex::open(&self.0).unwrap().1
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn the_index_pairs_a_definition_with_its_class_and_the_source_file_loses_its_symbols() {
    let repo = Repo::new("wired");
    let graph = repo.graph();

    // The definitions are not symbols of `db.cc` any more.
    assert!(graph.find("db.cc::leveldb::DB::Get").is_none());
    assert!(
        graph.children_ids(graph.find("db.cc").unwrap()).is_empty(),
        "a source file that holds only bodies has no symbol, so no spec of its own"
    );
    // The member owns its body.
    let get = graph
        .get_symbol(graph.find("db.h::leveldb::DB::Get").unwrap())
        .unwrap();
    assert_eq!(get.other_file_spans, [("db.cc".to_string(), [3, 5])]);
    let put = graph
        .get_symbol(graph.find("db.h::leveldb::DB::Put").unwrap())
        .unwrap();
    assert_eq!(put.other_file_spans, [("db.cc".to_string(), [6, 8])]);

    // The implementing file still depends on the class it implements.
    let callers: Vec<&str> = graph
        .imports()
        .iter()
        .filter(|e| e.target == graph.find("db.h::leveldb::DB"))
        .map(|e| e.from_file.as_str())
        .collect();
    assert!(callers.contains(&"db.cc"), "{callers:?}");
}

#[test]
fn editing_a_body_in_the_source_file_moves_the_class_and_not_its_interface() {
    let repo = Repo::new("edit");
    let before = repo.graph();
    let class_before = before
        .get_symbol(before.find("db.h::leveldb::DB").unwrap())
        .unwrap()
        .clone();

    std::fs::write(
        repo.0.join("db.cc"),
        DB_CC.replace("return k + 1;", "return k + 2;"),
    )
    .unwrap();
    let after = repo.graph();
    let class_after = after
        .get_symbol(after.find("db.h::leveldb::DB").unwrap())
        .unwrap();
    assert_ne!(
        class_before.source_hash, class_after.source_hash,
        "the class's spec describes the body that changed"
    );
    assert_eq!(
        class_before.interface_hash, class_after.interface_hash,
        "its users read the interface, which did not"
    );
}

#[test]
fn the_watchers_incremental_path_re_pairs_on_a_body_edit_a_removed_definition_and_a_deleted_file() {
    // C3: every other test builds the index from scratch with `open`; the
    // watcher goes through `apply_changes`, which rebuilds from cached
    // per-file extractions.
    let repo = Repo::new("watch");
    let (mut index, graph, _) = RepoIndex::open(&repo.0).unwrap();
    let class_before = graph
        .get_symbol(graph.find("db.h::leveldb::DB").unwrap())
        .unwrap()
        .clone();
    let spans = |g: &Graph, id: &str| {
        g.get_symbol(g.find(id).unwrap())
            .unwrap()
            .other_file_spans
            .clone()
    };
    assert_eq!(
        spans(&graph, "db.h::leveldb::DB::Put"),
        [("db.cc".to_string(), [6, 8])]
    );

    // A body edit: the class's source hash moves, its interface hash does not.
    let cc = repo.0.join("db.cc");
    std::fs::write(&cc, DB_CC.replace("return k + 1;", "return k + 2;")).unwrap();
    let (graph, caught) = index
        .apply_changes(std::slice::from_ref(&cc))
        .unwrap()
        .unwrap();
    assert_eq!(caught.modified, ["db.cc"]);
    let class_after = graph
        .get_symbol(graph.find("db.h::leveldb::DB").unwrap())
        .unwrap();
    assert_ne!(class_before.source_hash, class_after.source_hash);
    assert_eq!(class_before.interface_hash, class_after.interface_hash);
    assert_eq!(
        spans(&graph, "db.h::leveldb::DB::Put"),
        [("db.cc".to_string(), [6, 8])]
    );

    // A line inserted above `Put`'s body: its span moves with it, and `Get`'s
    // (above the insertion) does not. A cached pre-edit span would show here.
    std::fs::write(&cc, DB_CC.replace("int DB::Put", "// a note\nint DB::Put")).unwrap();
    let (graph, _) = index
        .apply_changes(std::slice::from_ref(&cc))
        .unwrap()
        .unwrap();
    assert_eq!(
        spans(&graph, "db.h::leveldb::DB::Put"),
        [("db.cc".to_string(), [7, 9])]
    );
    assert_eq!(
        spans(&graph, "db.h::leveldb::DB::Get"),
        [("db.cc".to_string(), [3, 5])]
    );

    // One definition removed from the source file: its member has no body in
    // another file any more, the other still has, and nothing is left behind.
    std::fs::write(
        &cc,
        "#include \"db.h\"\nnamespace leveldb {\nint DB::Get(int k) {\n  return k;\n}\n}\n",
    )
    .unwrap();
    let (graph, _) = index
        .apply_changes(std::slice::from_ref(&cc))
        .unwrap()
        .unwrap();
    assert!(spans(&graph, "db.h::leveldb::DB::Put").is_empty());
    assert_eq!(
        spans(&graph, "db.h::leveldb::DB::Get"),
        [("db.cc".to_string(), [3, 5])]
    );
    assert!(
        graph.symbols().all(|s| s.file != "db.cc"),
        "no symbol of the source file is left: every definition is paired or gone"
    );

    // The source file deleted: no member keeps a span in it, and nothing panics.
    std::fs::remove_file(&cc).unwrap();
    let (graph, caught) = index.apply_changes(&[cc]).unwrap().unwrap();
    assert_eq!(caught.removed, ["db.cc"]);
    assert!(graph.find("db.cc").is_none());
    assert!(spans(&graph, "db.h::leveldb::DB::Get").is_empty());
    assert!(graph.find("db.h::leveldb::DB::Get").is_some());
}
