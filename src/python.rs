//! Walks a single Python source file with tree-sitter and pulls out its
//! declarations — the Python pack's counterpart to `extract.rs` / `java.rs`
//! / `rust.rs` (M17).
//!
//! Shallow like the others: module-level classes and functions, a class's
//! methods and nested classes (recursed), and module-level `NAME = …`
//! assignments. Function bodies, comprehensions, and locals are
//! implementation detail, not declarations CodeOwl generates a spec for.
//!
//! Kind mapping onto the stack-neutral [`SymbolKind`]:
//!   * `class` → `Container` (raw `"class"`)
//!   * `def` — module-level or method → `Callable` (raw `"def"`)
//!   * a module-level `NAME = …` → `Value` (raw `"assignment"`)
//!
//! `is_exported` follows Python's leading-underscore convention: `_name` is
//! private, a `__dunder__` is public (it's API surface). Class members are
//! never independently exported (same stance as `java.rs` / `rust.rs`).
//!
//! Decorators are captured verbatim into `markers`, arguments included
//! (`@router.get("/items/{id}")`), the same way `java.rs::annotations`
//! captures `@Path("/x")` — the FastAPI feature model (M17) reads them back.

use tree_sitter::{Node, Parser};

use crate::hash::hash_text;
use crate::symbol::{ExtractedSymbol, SymbolKind};

/// Parse `source` (the contents of `rel_path`, a `.py` file) and extract
/// its module-level declarations and their members.
pub fn extract_file(source: &str, rel_path: &str) -> Vec<ExtractedSymbol> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .expect("bundled tree-sitter-python grammar should always load");

    // `tree` owns the arena every `Node<'_>` below borrows from — nothing
    // escapes this function (CLAUDE.md's "don't let Node<'a> escape" rule).
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        visit_item(child, None, &[], source, rel_path, &mut out);
    }
    out
}

/// Handle one statement. `parent_id` is the enclosing class this item is
/// nested in, or `None` at module top level. `markers` carries decorators
/// already collected off a `decorated_definition` wrapper.
fn visit_item(
    node: Node,
    parent_id: Option<&str>,
    markers: &[String],
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    match node.kind() {
        "decorated_definition" => {
            let decos = collect_decorators(node, source);
            if let Some(inner) = node.child_by_field_name("definition") {
                visit_item(inner, parent_id, &decos, source, file, out);
            }
        }
        "class_definition" => visit_container(node, parent_id, markers, source, file, out),
        "function_definition" => push_callable(node, parent_id, markers, source, file, out),
        // Module-level `NAME = …` (and `NAME: T = …`) → a `Value` symbol.
        // A class body's assignments (model fields, `Config`) stay folded
        // into the class — the same stance `rust.rs` takes for struct
        // fields.
        "expression_statement" if parent_id.is_none() => {
            visit_module_assignment(node, source, file, out);
        }
        _ => {}
    }
}

/// A `class` with a body — recurse one level for its members (fully into
/// nested classes), then Merkle-fold their `source_hash`es into the
/// container's own, matching `java.rs` / `rust.rs`.
fn visit_container(
    node: Node,
    parent_id: Option<&str>,
    markers: &[String],
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    let name = field_text(node, "name", source)
        .unwrap_or("<anon>")
        .to_string();
    let id = match parent_id {
        Some(p) => format!("{p}::{name}"),
        None => format!("{file}::{name}"),
    };
    let before = out.len();

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            visit_item(child, Some(&id), &[], source, file, out);
        }
    }

    let direct: Vec<&ExtractedSymbol> = out[before..]
        .iter()
        .filter(|s| s.parent.as_deref() == Some(id.as_str()))
        .collect();
    let member_ids: Vec<String> = direct.iter().map(|s| s.id.clone()).collect();
    let sig = signature_before_body(node, source);
    let mut rollup = sig.clone();
    for s in &direct {
        rollup.push('\n');
        rollup.push_str(&s.source_hash);
    }

    // Top-level classes are exported by name convention; a nested class is
    // a member (usually `Meta` / `Config`) and not independently reachable.
    let is_exported = parent_id.is_none() && is_public_name(&name);

    out.insert(
        before,
        ExtractedSymbol {
            id,
            kind: SymbolKind::Container,
            raw: "class".to_string(),
            file: file.to_string(),
            lines: node_lines(node),
            signature: sig.clone(),
            docstring: docstring(node, source),
            is_exported,
            source_hash: hash_text(&rollup),
            interface_hash: is_exported.then(|| hash_text(&sig)),
            markers: markers.to_vec(),
            parent: parent_id.map(str::to_string),
            children: member_ids,
        },
    );
}

/// A `def` — a module-level function or a method.
fn push_callable(
    node: Node,
    parent_id: Option<&str>,
    markers: &[String],
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    let Some(name) = field_text(node, "name", source) else {
        return;
    };
    let id = match parent_id {
        Some(p) => format!("{p}::{name}"),
        None => format!("{file}::{name}"),
    };
    let sig = signature_before_body(node, source);
    // A method is never independently imported; a module-level function is
    // exported by name convention.
    let is_exported = parent_id.is_none() && is_public_name(name);
    out.push(ExtractedSymbol {
        id,
        kind: SymbolKind::Callable,
        raw: "def".to_string(),
        file: file.to_string(),
        lines: node_lines(node),
        signature: sig.clone(),
        docstring: docstring(node, source),
        is_exported,
        source_hash: hash_text(text(node, source)),
        interface_hash: is_exported.then(|| hash_text(&sig)),
        markers: markers.to_vec(),
        parent: parent_id.map(str::to_string),
        children: Vec::new(),
    });
}

/// `NAME = …` / `NAME: T = …` at module scope → one `Value` per bound
/// name. Skips tuple targets (`a, b = …`) and attribute targets
/// (`obj.attr = …`) — neither is a top-level declaration.
fn visit_module_assignment(stmt: Node, source: &str, file: &str, out: &mut Vec<ExtractedSymbol>) {
    let mut cursor = stmt.walk();
    for child in stmt.children(&mut cursor) {
        if child.kind() != "assignment" {
            continue;
        }
        let Some(lhs) = child.child_by_field_name("left") else {
            continue;
        };
        if lhs.kind() != "identifier" {
            continue;
        }
        let name = text(lhs, source).to_string();
        let one_line = text(child, source)
            .lines()
            .next()
            .unwrap_or_default()
            .to_string();
        let is_exported = is_public_name(&name);
        out.push(ExtractedSymbol {
            id: format!("{file}::{name}"),
            kind: SymbolKind::Value,
            raw: "assignment".to_string(),
            file: file.to_string(),
            lines: node_lines(child),
            signature: one_line,
            docstring: None,
            is_exported,
            source_hash: hash_text(text(child, source)),
            interface_hash: is_exported.then(|| hash_text(text(child, source))),
            markers: Vec::new(),
            parent: None,
            children: Vec::new(),
        });
    }
}

/// The `@decorator` lines on a `decorated_definition`, verbatim and
/// whitespace-collapsed — `@staticmethod`, `@router.get("/items/{id}")`.
fn collect_decorators(node: Node, source: &str) -> Vec<String> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|n| n.kind() == "decorator")
        .map(|n| {
            text(n, source)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// Everything from the declaration's start up to its body — `def name(…) ->
/// T` / `class Name(Base, kw=…)` — with the trailing `:` trimmed. The "text
/// before the block" trick, same as `java.rs::signature_before_body`.
fn signature_before_body(node: Node, source: &str) -> String {
    let start = node.start_byte();
    let end = node
        .child_by_field_name("body")
        .map(|b| b.start_byte())
        .unwrap_or_else(|| node.end_byte())
        .max(start);
    source[start..end]
        .trim_end()
        .trim_end_matches(':')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The module/class/function docstring: a bare string literal as the first
/// statement of the body. Leading indentation and blank lines trimmed.
fn docstring(node: Node, source: &str) -> Option<String> {
    let body = node.child_by_field_name("body")?;
    let mut cursor = body.walk();
    let first = body.named_children(&mut cursor).next()?;
    if first.kind() != "expression_statement" {
        return None;
    }
    let s = first.named_child(0)?;
    if s.kind() != "string" {
        return None;
    }
    let raw = text(s, source);
    let inner = raw
        .trim_start_matches(|c| {
            c == 'r' || c == 'b' || c == 'u' || c == 'f' || c == 'R' || c == 'B'
        })
        .trim_matches('"')
        .trim_matches('\'');
    let lines: Vec<&str> = inner
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// Python's leading-underscore visibility convention: `_x` is private,
/// `__x__` (a dunder) is public API, everything else is public.
fn is_public_name(name: &str) -> bool {
    !name.starts_with('_') || (name.starts_with("__") && name.ends_with("__"))
}

fn field_text<'a>(node: Node, field: &str, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name(field).map(|n| text(n, source))
}

fn text<'a>(node: Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or_default()
}

fn node_lines(node: Node) -> [usize; 2] {
    [node.start_position().row + 1, node.end_position().row + 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_module_with_a_class_and_a_function() {
        // NB: raw string — Rust's `\`-newline continuation strips leading
        // whitespace, which silently de-indents Python and breaks the parse.
        let src = r#""""Item routes."""

router = APIRouter(prefix="/items")


class ItemService:
    """CRUD helpers for items."""

    def read_all(self, session):
        return session.query(Item).all()

    def _private(self):
        return 1


def get_db():
    yield Session()
"#;
        let syms = extract_file(src, "app/api/routes/items.py");
        let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "app/api/routes/items.py::router",
                "app/api/routes/items.py::ItemService",
                "app/api/routes/items.py::ItemService::read_all",
                "app/api/routes/items.py::ItemService::_private",
                "app/api/routes/items.py::get_db",
            ]
        );

        let router = &syms[0];
        assert_eq!(router.kind, SymbolKind::Value);
        assert_eq!(router.raw, "assignment");
        assert_eq!(router.signature, "router = APIRouter(prefix=\"/items\")");

        let svc = &syms[1];
        assert_eq!(svc.kind, SymbolKind::Container);
        assert_eq!(svc.raw, "class");
        assert!(svc.is_exported);
        assert_eq!(svc.docstring.as_deref(), Some("CRUD helpers for items."));
        assert_eq!(svc.children.len(), 2);

        let read_all = &syms[2];
        assert_eq!(read_all.kind, SymbolKind::Callable);
        assert_eq!(read_all.raw, "def");
        assert!(
            !read_all.is_exported,
            "a method is never top-level-exported"
        );
        assert_eq!(read_all.parent.as_deref(), Some(ids[1]));

        assert!(!syms[3].is_exported, "_private stays private");

        let get_db = &syms[4];
        assert!(get_db.is_exported);
        assert_eq!(get_db.signature, "def get_db()");
    }

    #[test]
    fn module_docstring_and_underscore_module_are_handled() {
        let src =
            "\"\"\"\nThe app package.\n\nDetails.\n\"\"\"\n\n_internal = 1\nVERSION = \"1.0\"\n";
        let syms = extract_file(src, "app/__init__.py");
        // module docstring isn't a symbol; the two assignments are.
        let vals: Vec<(&str, bool)> = syms
            .iter()
            .map(|s| (s.id.rsplit("::").next().unwrap(), s.is_exported))
            .collect();
        assert_eq!(vals, vec![("_internal", false), ("VERSION", true)]);
    }

    #[test]
    fn decorators_land_in_markers_with_their_arguments() {
        let src = r#"@router.get("/items/{id}")
def read_item(id: int) -> ItemPublic:
    ...
"#;
        let syms = extract_file(src, "r.py");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].markers, vec!["@router.get(\"/items/{id}\")"]);
        assert_eq!(syms[0].signature, "def read_item(id: int) -> ItemPublic");
    }

    #[test]
    fn a_decorated_class_keeps_its_decorator_and_class_kind() {
        let src = "@final\nclass Frozen:\n    pass\n";
        let syms = extract_file(src, "f.py");
        assert_eq!(syms[0].kind, SymbolKind::Container);
        assert_eq!(syms[0].markers, vec!["@final"]);
    }

    #[test]
    fn nested_class_is_a_member_not_an_export() {
        let src = r#"class Item:
    class Meta:
        ordering = ["id"]

    def m(self):
        pass
"#;
        let syms = extract_file(src, "m.py");
        let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["m.py::Item", "m.py::Item::Meta", "m.py::Item::m"]);
        let meta = syms.iter().find(|s| s.id == "m.py::Item::Meta").unwrap();
        assert_eq!(meta.kind, SymbolKind::Container);
        assert!(!meta.is_exported, "a nested class is a member");
        assert_eq!(syms[0].children.len(), 2);
    }

    #[test]
    fn method_body_edit_moves_source_hash_but_not_interface_hash() {
        let a = extract_file("class C:\n    def m(self):\n        return 1\n", "c.py");
        let b = extract_file("class C:\n    def m(self):\n        return 2\n", "c.py");
        assert_ne!(a[0].source_hash, b[0].source_hash, "Merkle rollup moves");
        assert_eq!(a[0].interface_hash, b[0].interface_hash, "signature didn't");
    }

    #[test]
    fn class_body_assignments_do_not_become_symbols() {
        // SQLModel-shaped: many field assignments in the class body. They
        // stay folded into the class, not one Value symbol each.
        let src = r#"class Item(SQLModel, table=True):
    id: int = Field(primary_key=True)
    title: str = Field(max_length=255)
    owner_id: int
"#;
        let syms = extract_file(src, "models.py");
        assert_eq!(syms.len(), 1, "just the class, no field symbols");
        assert_eq!(
            syms[0].signature, "class Item(SQLModel, table=True)",
            "the class arg list is in the signature"
        );
    }
}
