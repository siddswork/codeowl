//! Integration: a Rust file goes through extraction → `Graph` → the spec
//! `next_task`/`submit` loop → `render`, and comes out as the *folded*
//! shape M15 introduced — an inherent `impl Foo` is part of `## Foo`, not
//! its own `## impl Foo` section, while a trait impl keeps its own
//! section. This is the end-to-end version of `rust.rs`'s unit tests,
//! exercising the same path `/codeowl generate` drives on CodeOwl's own
//! repo.

use codeowl::graph::{FileExtraction, Graph};
use codeowl::hash::hash_text;
use codeowl::spec::{SpecTask, next_task, read_file_spec, render, submit};

const SRC: &str = "\
use std::fmt;\n\
\n\
/// A bounded counter.\n\
pub struct Counter {\n    n: u64,\n}\n\
\n\
impl Counter {\n\
    pub fn new() -> Self {\n        Self { n: 0 }\n    }\n\
    pub fn bump(&mut self) {\n        self.n += 1;\n    }\n\
}\n\
\n\
impl fmt::Display for Counter {\n\
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n\
        write!(f, \"{}\", self.n)\n    }\n\
}\n\
\n\
/// Free helper, unrelated to the type.\n\
pub fn reset(c: &mut Counter) {\n    *c = Counter::new();\n}\n";

fn rust_graph(rel_path: &str, src: &str) -> Graph {
    Graph::build(vec![FileExtraction {
        rel_path: rel_path.to_string(),
        source_hash: hash_text(src),
        symbols: codeowl::rust::extract_file(src, rel_path),
    }])
}

#[test]
fn inherent_impl_is_folded_but_trait_impl_stays_its_own_section() {
    let dir = std::env::temp_dir().join(format!("codeowl-rust-spec-{}-fold", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/counter.rs"), SRC).unwrap();

    let graph = rust_graph("src/counter.rs", SRC);
    let file_id = graph.find("src/counter.rs").unwrap();

    // The generate loop only ever offers the type, the trait impl, and the
    // free fn — never a bare `impl Counter`.
    let mut offered = Vec::new();
    while let Some(task) = next_task(&graph, &dir, file_id).unwrap() {
        match task {
            SpecTask::Symbol { id, .. } => {
                offered.push(id.clone());
                submit(
                    &graph,
                    &dir,
                    &id,
                    "### Summary\nA deliberate unit of behaviour.\n\
                     ### Behavior\nDoes exactly what the name says and nothing more.\n",
                )
                .unwrap();
            }
            SpecTask::File { id, .. } => {
                submit(&graph, &dir, &id, "A counter type and a reset helper.").unwrap();
            }
        }
    }

    assert_eq!(
        offered,
        vec![
            "src/counter.rs::Counter".to_string(),
            "src/counter.rs::impl fmt::Display for Counter".to_string(),
            "src/counter.rs::reset".to_string(),
        ],
        "the inherent `impl Counter` must not be offered as its own task"
    );

    let spec = read_file_spec(&dir, "src/counter.rs").unwrap().unwrap();
    let rendered = render(&graph, &dir, file_id, &spec);

    assert!(rendered.contains("## `Counter`"), "type section missing");
    assert!(
        !rendered.contains("## `impl Counter`"),
        "inherent impl must not render as its own section:\n{rendered}"
    );
    // The trait impl keeps its own section — its `impl` signature line is
    // present, and it's a distinct `##` heading from the type's.
    assert!(
        rendered.contains("`impl fmt::Display for Counter`"),
        "trait impl section missing:\n{rendered}"
    );
    assert_eq!(
        rendered.matches("\n## `").count(),
        3,
        "exactly three top-level sections: Counter, the trait impl, reset:\n{rendered}"
    );
    assert!(rendered.contains("## `reset`"), "free fn section missing");

    std::fs::remove_dir_all(&dir).ok();
}
