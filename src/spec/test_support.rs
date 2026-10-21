//! Fixtures and helpers shared by the spec modules' tests.

use std::path::Path;

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
