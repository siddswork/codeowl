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
    let mut graph = Graph::build(vec![FileExtraction {
        rel_path: rel_path.to_string(),
        source_hash: hash_text(src),
        symbols: codeowl::rust::extract_file(src, rel_path),
    }]);
    // Mirrors `RepoIndex::build`: imports (including M21.g's synthetic
    // same-file impl -> type edges) are resolved once the graph exists,
    // never at extraction time — `dependency_hash` reads `graph.imports()`,
    // so skipping this step would leave every dependency edge invisible.
    let file_imports = std::collections::HashMap::from([(
        rel_path.to_string(),
        codeowl::rust::extract_imports(src, rel_path),
    )]);
    let resolved =
        codeowl::rust::resolve_imports(std::path::Path::new("/unused"), &file_imports, &graph);
    graph.set_resolved_imports(resolved);
    graph
}

/// The rendered block for one symbol's `## \`name\`` section, up to (not
/// including) the next `## \`` heading or end of file — everything
/// `render` wrote for it, including its own `### Depends on` list.
fn section<'a>(rendered: &'a str, name: &str) -> &'a str {
    let heading = format!("## `{name}`");
    let start = rendered
        .find(&heading)
        .unwrap_or_else(|| panic!("missing section {heading:?} in:\n{rendered}"));
    let rest = &rendered[start..];
    let end = rest[heading.len()..]
        .find("\n## `")
        .map(|i| i + heading.len())
        .unwrap_or(rest.len());
    &rest[..end]
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

// M21.e (`ARCHITECTURE.md` open question 16, degenerate case): a zero-field,
// zero-inherent-method type whose entire contract is one trait impl folds
// into a single document too — the shape `Counter` above deliberately isn't,
// since it has a field and an inherent impl. The real-world trigger was
// `stack.rs::JavaStack`; this is the same shape in miniature.
const MARKER_SRC: &str = "\
pub trait Greeter {\n\
    fn greet(&self) -> String;\n\
}\n\
\n\
pub struct EnglishGreeter;\n\
\n\
impl Greeter for EnglishGreeter {\n\
    fn greet(&self) -> String {\n        \"Hello!\".to_string()\n    }\n\
}\n";

#[test]
fn a_zero_field_single_trait_impl_marker_type_folds_into_one_document() {
    let dir = std::env::temp_dir().join(format!("codeowl-rust-spec-{}-marker", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/greeter.rs"), MARKER_SRC).unwrap();

    let graph = rust_graph("src/greeter.rs", MARKER_SRC);
    let file_id = graph.find("src/greeter.rs").unwrap();

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
                submit(
                    &graph,
                    &dir,
                    &id,
                    "A greeter trait and its one implementor.",
                )
                .unwrap();
            }
        }
    }

    assert_eq!(
        offered,
        vec![
            "src/greeter.rs::Greeter".to_string(),
            "src/greeter.rs::EnglishGreeter".to_string(),
        ],
        "the trait impl must not be offered as its own task — it's folded \
         into EnglishGreeter, the same way an inherent impl already is"
    );

    let spec = read_file_spec(&dir, "src/greeter.rs").unwrap().unwrap();
    let rendered = render(&graph, &dir, file_id, &spec);

    assert!(
        rendered.contains("## `EnglishGreeter`"),
        "type section missing:\n{rendered}"
    );
    assert!(
        !rendered.contains("## `impl Greeter for EnglishGreeter`"),
        "the trait impl must not render as its own section once folded:\n{rendered}"
    );
    assert_eq!(
        rendered.matches("\n## `").count(),
        2,
        "exactly two top-level sections: Greeter (the trait itself, untouched) \
         and EnglishGreeter (now folded with its impl):\n{rendered}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

// M21.g (`ARCHITECTURE.md` open question 16, general case): `PersonalGreeter`
// has a real field, so M21.e's fold correctly leaves it alone — two
// documents, not one. But the trait impl's own text never mentions `use`
// for `PersonalGreeter` (same file, no import needed), so without a
// synthetic impl -> type dependency edge nothing would ever tell the impl's
// document that `PersonalGreeter`'s own shape moved underneath it.
fn personal_greeter_src(field_decl: &str) -> String {
    format!(
        "\
pub trait Greeter {{\n\
    fn greet(&self) -> String;\n\
}}\n\
\n\
pub struct PersonalGreeter {{\n\
    {field_decl}\n\
}}\n\
\n\
impl Greeter for PersonalGreeter {{\n\
    fn greet(&self) -> String {{\n        \
        format!(\"Hello, {{}}!\", self.name)\n    \
    }}\n\
}}\n"
    )
}

#[test]
fn a_types_own_field_change_invalidates_its_trait_impls_document_without_folding() {
    let dir =
        std::env::temp_dir().join(format!("codeowl-rust-spec-{}-personal", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();

    let v1 = personal_greeter_src("pub name: String,");
    std::fs::write(dir.join("src/greeter.rs"), &v1).unwrap();
    let graph1 = rust_graph("src/greeter.rs", &v1);
    let file_id1 = graph1.find("src/greeter.rs").unwrap();

    let mut offered = Vec::new();
    while let Some(task) = next_task(&graph1, &dir, file_id1).unwrap() {
        match task {
            SpecTask::Symbol { id, .. } => {
                offered.push(id.clone());
                submit(
                    &graph1,
                    &dir,
                    &id,
                    "### Summary\nA deliberate unit of behaviour.\n\
                     ### Behavior\nDoes exactly what the name says and nothing more.\n",
                )
                .unwrap();
            }
            SpecTask::File { id, .. } => {
                submit(
                    &graph1,
                    &dir,
                    &id,
                    "A greeter trait and one real implementor.",
                )
                .unwrap();
            }
        }
    }
    assert_eq!(
        offered,
        vec![
            "src/greeter.rs::Greeter".to_string(),
            "src/greeter.rs::PersonalGreeter".to_string(),
            "src/greeter.rs::impl Greeter for PersonalGreeter".to_string(),
        ],
        "a real field keeps PersonalGreeter and its trait impl as separate tasks"
    );

    let spec1 = read_file_spec(&dir, "src/greeter.rs").unwrap().unwrap();
    let rendered1 = render(&graph1, &dir, file_id1, &spec1);
    assert!(rendered1.contains("## `PersonalGreeter`"));
    assert!(rendered1.contains("## `impl Greeter for PersonalGreeter`"));
    assert_eq!(
        rendered1.matches("\n## `").count(),
        3,
        "three top-level sections: Greeter, PersonalGreeter, its trait impl:\n{rendered1}"
    );

    // Regression: the synthetic impl -> type edge must never make the
    // *type* appear to depend on itself. The edge's `imported_name` is
    // `PersonalGreeter`'s own name, and `PersonalGreeter`'s own
    // declaration necessarily contains that name too -- without an
    // explicit self-exclusion, its own dependency scan matches its own
    // synthetic edge.
    let greeter_section = section(&rendered1, "PersonalGreeter");
    assert!(
        greeter_section.contains("### Depends on\n- (none)"),
        "PersonalGreeter must not depend on anything, least of all itself:\n{greeter_section}"
    );
    // The impl's own section must have the real edge, with a real
    // (non-empty) specifier -- not the malformed `` `target` — `` an
    // empty specifier string would render.
    let impl_section = section(&rendered1, "impl Greeter for PersonalGreeter");
    assert!(
        impl_section.contains("`src/greeter.rs::PersonalGreeter` — same file"),
        "the impl must depend on PersonalGreeter with a real specifier:\n{impl_section}"
    );

    // Add a second public field. This moves PersonalGreeter's own
    // source_hash *and* interface_hash — but the trait impl block's own
    // text is byte-for-byte unchanged.
    let v2 = personal_greeter_src("pub name: String,\n    pub greeting_count: u32,");
    assert_eq!(
        v1.lines()
            .find(|l| l.contains("impl Greeter for PersonalGreeter"))
            .unwrap(),
        v2.lines()
            .find(|l| l.contains("impl Greeter for PersonalGreeter"))
            .unwrap(),
        "sanity check: the impl block's own header line is unchanged"
    );
    std::fs::write(dir.join("src/greeter.rs"), &v2).unwrap();
    let graph2 = rust_graph("src/greeter.rs", &v2);
    let file_id2 = graph2.find("src/greeter.rs").unwrap();

    let mut offered2 = Vec::new();
    while let Some(task) = next_task(&graph2, &dir, file_id2).unwrap() {
        if let SpecTask::Symbol { id, .. } = &task {
            offered2.push(id.clone());
        }
        match task {
            SpecTask::Symbol { id, .. } => {
                submit(
                    &graph2,
                    &dir,
                    &id,
                    "### Summary\nA deliberate unit of behaviour, refreshed.\n\
                     ### Behavior\nDoes exactly what the name says and nothing more.\n",
                )
                .unwrap();
            }
            SpecTask::File { id, .. } => {
                submit(
                    &graph2,
                    &dir,
                    &id,
                    "A greeter trait and one real implementor.",
                )
                .unwrap();
            }
        }
    }

    assert!(
        offered2.contains(&"src/greeter.rs::PersonalGreeter".to_string()),
        "PersonalGreeter's own text changed, so it must be regenerated: {offered2:?}"
    );
    assert!(
        offered2.contains(&"src/greeter.rs::impl Greeter for PersonalGreeter".to_string()),
        "the trait impl's document must go stale too -- PersonalGreeter's shape moved \
         underneath it even though the impl's own text didn't change at all: {offered2:?}"
    );
    assert!(
        !offered2.contains(&"src/greeter.rs::Greeter".to_string()),
        "the unrelated trait itself must NOT be regenerated -- proves this isn't just \
         regenerating everything blindly: {offered2:?}"
    );

    let spec2 = read_file_spec(&dir, "src/greeter.rs").unwrap().unwrap();
    let rendered2 = render(&graph2, &dir, file_id2, &spec2);
    assert!(rendered2.contains("## `PersonalGreeter`"));
    assert!(rendered2.contains("## `impl Greeter for PersonalGreeter`"));
    assert_eq!(
        rendered2.matches("\n## `").count(),
        3,
        "still three separate sections after the fix -- linking staleness never folds \
         them into one document:\n{rendered2}"
    );

    std::fs::remove_dir_all(&dir).ok();
}
