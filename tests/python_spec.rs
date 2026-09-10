//! Integration (M17, commit 1 — extraction): a Python file goes through
//! extraction → `Graph` → the spec `next_task`/`submit` loop → `render`,
//! and comes out with the class as one `## `Type`` section, its methods
//! folded in and never offered as their own top-level tasks. The
//! end-to-end version of `python.rs`'s unit tests, on the path
//! `/codeowl generate` drives.
//!
//! Resolution, the `is_schema_symbol` seam, and the FastAPI feature model
//! land in follow-up commits with their own coverage.

use codeowl::graph::{FileExtraction, Graph};
use codeowl::hash::hash_text;
use codeowl::spec::{SpecTask, next_task, read_file_spec, render, submit};

const PATH: &str = "app/api/routes/items.py";

// Raw string on purpose — Rust's `\`-newline continuation strips leading
// whitespace, which de-indents Python and breaks the parse.
const SRC: &str = r#""""Item CRUD routes."""

router = APIRouter(prefix="/items")


class ItemService:
    """Reads and writes items for the current user."""

    def read_all(self, session, owner_id):
        return session.exec(select(Item).where(Item.owner_id == owner_id)).all()

    def _row_count(self, session):
        return session.exec(select(func.count()).select_from(Item)).one()


def get_current_user():
    """FastAPI dependency — resolves the bearer token to a user row."""
    ...
"#;

fn py_graph(rel_path: &str, src: &str) -> Graph {
    Graph::build(vec![FileExtraction {
        rel_path: rel_path.to_string(),
        source_hash: hash_text(src),
        symbols: codeowl::python::extract_file(src, rel_path),
    }])
}

#[test]
fn a_class_renders_as_one_section_with_its_methods_folded_in() {
    let dir = std::env::temp_dir().join(format!("codeowl-py-spec-{}-cls", std::process::id()));
    std::fs::create_dir_all(dir.join("app/api/routes")).unwrap();
    std::fs::write(dir.join(PATH), SRC).unwrap();

    let graph = py_graph(PATH, SRC);
    let file_id = graph.find(PATH).unwrap();

    // A `_`-prefixed method is a member, not exported — never a
    // reference-edge target, but it stays in the graph.
    let row_count = graph
        .get_symbol(
            graph
                .find(&format!("{PATH}::ItemService::_row_count"))
                .unwrap(),
        )
        .unwrap();
    assert!(
        !row_count.is_exported,
        "a _-prefixed method is not exported"
    );

    // The generate loop offers two symbol tasks — the class and the
    // module-level function. `read_all` / `_row_count` are members; the
    // `router` assignment is a `Value`, folded into the file.
    let mut offered = Vec::new();
    for _ in 0..20 {
        let Some(task) = next_task(&graph, &dir, file_id).unwrap() else {
            break;
        };
        match task {
            SpecTask::Symbol { id, docstring, .. } => {
                if id == format!("{PATH}::ItemService") {
                    assert_eq!(
                        docstring.as_deref(),
                        Some("Reads and writes items for the current user."),
                        "the class docstring is carried into the task"
                    );
                }
                offered.push(id.clone());
                submit(
                    &graph,
                    &dir,
                    &id,
                    "### Summary\nA thin data-access helper scoped to the calling user.\n\
                     ### Behavior\nEach public method runs one SQLModel select against \
                     the item table, filtered by owner where relevant.\n",
                )
                .unwrap();
            }
            SpecTask::File { id, .. } => {
                submit(
                    &graph,
                    &dir,
                    &id,
                    "Item CRUD routes — a service class plus the current-user dependency.",
                )
                .unwrap();
            }
        }
    }

    assert_eq!(
        offered,
        vec![
            format!("{PATH}::ItemService"),
            format!("{PATH}::get_current_user"),
        ],
        "the class and the module function are offered — not the methods or the router assignment"
    );

    let spec = read_file_spec(&dir, PATH).unwrap().unwrap();
    let rendered = render(&graph, &dir, file_id, &spec);

    assert!(
        rendered.contains("## `ItemService`"),
        "class section missing:\n{rendered}"
    );
    assert!(
        !rendered.contains("## `read_all`") && !rendered.contains("## `ItemService.read_all`"),
        "a method must not render as its own top-level section:\n{rendered}"
    );
    assert_eq!(
        rendered.matches("\n## `").count(),
        2,
        "two top-level sections — the class and the function:\n{rendered}"
    );

    std::fs::remove_dir_all(&dir).ok();
}
