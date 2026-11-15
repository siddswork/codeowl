//! Fixtures and helpers shared by the spec modules' tests.

use std::path::{Path, PathBuf};

use super::file_task::submit;
use super::layout::spec_path;
use crate::graph::Graph;
use crate::hash::hash_text;

/// Test helper: put a hash-*current* symbol spec on disk whose prose would
/// fail `prose_smells` — the shape a pre-fix cache carries, or a hand-edit
/// whose frontmatter `spec_hash` was fixed up to match. `submit` now
/// refuses to write such prose, so the tests that exercise the
/// smell-driven `next_task` re-offer plant it directly. Seeds a clean spec
/// via `submit`, then swaps in the smelly prose and its matching hash.
pub(crate) fn plant_smelly_symbol_spec(
    graph: &Graph,
    root: &Path,
    sym_id: &str,
    summary: &str,
    behavior: &str,
) {
    const OK_S: &str = "A placeholder summary comfortably past the four word floor.";
    const OK_B: &str = "A placeholder behavior sentence comfortably past the four word floor.";
    submit(
        graph,
        root,
        sym_id,
        &format!("### Summary\n{OK_S}\n### Behavior\n{OK_B}\n"),
    )
    .expect("placeholder prose is clean");

    let file_id = graph
        .get_symbol(graph.find(sym_id).expect("symbol exists"))
        .and_then(|s| s.parent)
        .expect("symbol has a containing file");
    let file_string_id = graph
        .get_file(file_id)
        .expect("parent is a file")
        .id
        .clone();
    let path = spec_path(root, &file_string_id);

    let patched = std::fs::read_to_string(&path)
        .expect("spec was written")
        .replace(OK_S, summary)
        .replace(OK_B, behavior)
        .replace(
            &hash_text(&format!("{OK_S}\n{OK_B}")),
            &hash_text(&format!("{summary}\n{behavior}")),
        );
    std::fs::write(&path, patched).expect("rewrite spec");
}

/// A two-file Rust fixture: `people.rs` imports the trait `Greeter` from
/// `greeter.rs` and implements it on `Polite`, with the import resolved,
/// so both the file and the `Polite` symbol have one real dependency.
pub(crate) fn greeter_people_graph(suffix: &str) -> (Graph, std::path::PathBuf) {
    let dir =
        std::env::temp_dir().join(format!("codeowl-spec-test-{}-{suffix}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let greeter = "pub trait Greeter {\n    fn greet(&self) -> String;\n}\n";
    let people = "\
use crate::greeter::Greeter;\n\n\
pub struct Polite;\n\n\
impl Greeter for Polite {\n    fn greet(&self) -> String {\n        String::new()\n    }\n}\n";
    std::fs::write(dir.join("src/greeter.rs"), greeter).unwrap();
    std::fs::write(dir.join("src/people.rs"), people).unwrap();
    let mut graph = Graph::build(vec![
        crate::graph::FileExtraction {
            rel_path: "src/greeter.rs".to_string(),
            source_hash: hash_text(greeter),
            symbols: crate::rust::extract_file(greeter, "src/greeter.rs"),
        },
        crate::graph::FileExtraction {
            rel_path: "src/people.rs".to_string(),
            source_hash: hash_text(people),
            symbols: crate::rust::extract_file(people, "src/people.rs"),
        },
    ]);
    graph.set_resolved_imports(vec![crate::graph::ResolvedImport {
        from_file: "src/people.rs".to_string(),
        specifier: "crate::greeter".to_string(),
        imported_name: "Greeter".to_string(),
        target: graph.find("src/greeter.rs::Greeter"),
    }]);
    (graph, dir)
}

/// A symbol whose source is in two files, built by hand (no C++ involved):
/// the class `A` in `a.h`, whose members `f` and `g` have their bodies in
/// `a.cc`. `a.h` names `Foo` (an edge to `foo.h::Foo`), `a.cc` names `Bar`
/// (an edge to `bar.h::Bar`), and two decoy edges must never count for `A`:
/// one from `a.h` whose word appears only in `a.cc`'s text, one from `a.cc`
/// whose word appears only in `a.h`'s. The decoys resolve to *other* symbols
/// (`other.h::Bar`, `other.h::Foo`) so that counting one is visible in the
/// targets, not hidden by a real edge to the same symbol.
pub(crate) fn paired_graph(suffix: &str) -> (Graph, PathBuf) {
    use crate::graph::{FileExtraction, ResolvedImport};
    use crate::symbol::{ExtractedSymbol, SymbolKind};

    let dir = std::env::temp_dir().join(format!(
        "codeowl-paired-test-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let a_h = "class A {\n  int f(Foo x);\n  int g();\n};\n";
    let a_cc = "#include \"a.h\"\nint A::f(int x) {\n  return Bar() + 1;\n}\n\nint A::g() {\n  return 2;\n}\n";
    let foo_h = "int Foo();\n";
    let bar_h = "int Bar();\n";
    let other_h = "int Foo();\nint Bar();\n";
    for (rel, text) in [
        ("a.h", a_h),
        ("a.cc", a_cc),
        ("foo.h", foo_h),
        ("bar.h", bar_h),
        ("other.h", other_h),
    ] {
        std::fs::write(dir.join(rel), text).unwrap();
    }
    let sym = |id: &str,
               file: &str,
               kind: SymbolKind,
               lines: [usize; 2],
               parent: Option<&str>,
               children: &[&str],
               other: &[(&str, [usize; 2])]| ExtractedSymbol {
        id: id.to_string(),
        kind,
        raw: "x".to_string(),
        file: file.to_string(),
        lines,
        signature: id.to_string(),
        docstring: None,
        is_exported: true,
        source_hash: hash_text(id),
        interface_hash: Some(hash_text(&format!("{id} interface"))),
        markers: Vec::new(),
        extra_spans: Vec::new(),
        other_file_spans: other.iter().map(|(f, l)| (f.to_string(), *l)).collect(),
        parent: parent.map(str::to_string),
        children: children.iter().map(|c| c.to_string()).collect(),
    };
    let mut graph = Graph::build(vec![
        FileExtraction {
            rel_path: "a.h".to_string(),
            source_hash: hash_text(a_h),
            symbols: vec![
                sym(
                    "a.h::A",
                    "a.h",
                    SymbolKind::Container,
                    [1, 4],
                    None,
                    &["a.h::A::f", "a.h::A::g"],
                    &[],
                ),
                sym(
                    "a.h::A::f",
                    "a.h",
                    SymbolKind::Callable,
                    [2, 2],
                    Some("a.h::A"),
                    &[],
                    &[("a.cc", [2, 4])],
                ),
                sym(
                    "a.h::A::g",
                    "a.h",
                    SymbolKind::Callable,
                    [3, 3],
                    Some("a.h::A"),
                    &[],
                    &[("a.cc", [6, 8])],
                ),
            ],
        },
        FileExtraction {
            rel_path: "a.cc".to_string(),
            source_hash: hash_text(a_cc),
            symbols: Vec::new(),
        },
        FileExtraction {
            rel_path: "foo.h".to_string(),
            source_hash: hash_text(foo_h),
            symbols: vec![sym(
                "foo.h::Foo",
                "foo.h",
                SymbolKind::Callable,
                [1, 1],
                None,
                &[],
                &[],
            )],
        },
        FileExtraction {
            rel_path: "bar.h".to_string(),
            source_hash: hash_text(bar_h),
            symbols: vec![sym(
                "bar.h::Bar",
                "bar.h",
                SymbolKind::Callable,
                [1, 1],
                None,
                &[],
                &[],
            )],
        },
        FileExtraction {
            rel_path: "other.h".to_string(),
            source_hash: hash_text(other_h),
            symbols: vec![
                sym(
                    "other.h::Foo",
                    "other.h",
                    SymbolKind::Callable,
                    [1, 1],
                    None,
                    &[],
                    &[],
                ),
                sym(
                    "other.h::Bar",
                    "other.h",
                    SymbolKind::Callable,
                    [2, 2],
                    None,
                    &[],
                    &[],
                ),
            ],
        },
    ]);
    let edge = |from: &str, spec: &str, name: &str, target: &str| ResolvedImport {
        from_file: from.to_string(),
        specifier: spec.to_string(),
        imported_name: name.to_string(),
        target: graph.find(target),
    };
    let edges = vec![
        edge("a.h", "foo.h", "Foo", "foo.h::Foo"),
        edge("a.cc", "bar.h", "Bar", "bar.h::Bar"),
        // Decoys: each word is written only in the other file's text.
        edge("a.h", "other.h", "Bar", "other.h::Bar"),
        edge("a.cc", "other.h", "Foo", "other.h::Foo"),
    ];
    graph.set_resolved_imports(edges);
    (graph, dir)
}

/// Write a small fixture repo to a fresh temp dir and build the *whole*
/// graph off it — imports resolved, route literals / table refs /
/// rendered components extracted and set — exactly the pipeline
/// `RepoIndex::rebuild` runs. Feature-spec tests need the real thing
/// because they read files off disk and traverse the flow edges.
pub(crate) fn build_feature_fixture(
    files: &[(&str, &str)],
    suffix: &str,
) -> (Graph, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "codeowl-feature-spec-test-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for (rel, content) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
    }
    let graph = crate::index::RepoIndex::build(&dir)
        .unwrap()
        .rebuild()
        .unwrap();
    (graph, dir)
}

pub(crate) const ARTWORK_FIXTURE: &[(&str, &str)] = &[
    (
        "app/submit/page.tsx",
        "import { getSupabase } from '../../lib/supabase';\nexport default function Page() {\n  fetch(\"/api/submit-artwork\");\n  getSupabase();\n  return null;\n}\n",
    ),
    (
        "app/api/submit-artwork/route.ts",
        "import { getSupabase } from '../../../lib/supabase';\nexport async function POST(): Promise<void> {\n  getSupabase();\n}\n",
    ),
    (
        "lib/supabase.ts",
        "export function getSupabase(): void {}\n",
    ),
];

pub(crate) const SYSTEM_FIXTURE: &[(&str, &str)] = &[
    (
        "app/submit/page.tsx",
        "import { getSupabase } from '../../lib/supabase';\nexport default function Page() {\n  fetch(\"/api/submit-artwork\");\n  getSupabase();\n  return null;\n}\n",
    ),
    (
        "app/api/submit-artwork/route.ts",
        "import { getSupabase } from '../../../lib/supabase';\nexport async function POST(): Promise<void> {\n  getSupabase();\n}\n",
    ),
    (
        "lib/supabase.ts",
        "export function getSupabase(): void {}\n",
    ),
    ("lib/one.ts", "export function one(): void {}\n"),
    ("lib/two.ts", "export function two(): void {}\n"),
];

pub(crate) const BIG_JAVA: &str = "package a;\n\n/** The big class. */\npublic class Big {\n    /** Adds one. */\n    public int bump(int n) {\n        return n + 1; // BODY-ONE\n    }\n\n    /** Doubles it. */\n    public int twice(int n) {\n        return n * 2; // BODY-TWO\n    }\n}\n";

pub(crate) fn java_graph(src: &str) -> Graph {
    Graph::build(vec![crate::graph::FileExtraction {
        rel_path: "a/Big.java".to_string(),
        source_hash: hash_text(src),
        symbols: crate::java::extract_file(src, "a/Big.java"),
    }])
}

// ---- the file fingerprint in a file's own spec ----

pub(crate) fn temp_dir_for(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("codeowl-spec-test-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Prose that passes the quality check, for a symbol spec.
pub(crate) const PROSE: &str = "### Summary\nDoes the one thing it is named for.\n### Behavior\nRuns synchronously and has no side effects.\n";

/// Write `text` to `rel` under `dir`, creating its directories.
pub(crate) fn write_file(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The graph of whatever is on disk under `dir` now.
pub(crate) fn rebuild_graph(dir: &Path) -> Graph {
    crate::index::RepoIndex::build(dir)
        .unwrap()
        .rebuild()
        .unwrap()
}

/// Specs written for `files`, with each of `symbols` given `prose`; returns
/// the directory so a test can then move code around and rebuild.
pub(crate) fn documented(
    tag: &str,
    files: &[(&str, &str)],
    symbols: &[&str],
    prose: &str,
) -> PathBuf {
    let (graph, dir) = build_feature_fixture(files, tag);
    for id in symbols {
        submit(&graph, &dir, id, prose).unwrap();
    }
    dir
}
