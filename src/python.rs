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

use std::collections::HashMap;
use std::path::Path;

use tree_sitter::{Node, Parser};

use crate::graph::{FlowTarget, Graph, SymbolId, UnresolvedFlowEdge};
use crate::hash::hash_text;
use crate::imports::{FileImports, ImportRef};
use crate::resolve::ResolvedImport;
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

// ---------------------------------------------------------------------------
// Imports — `import` / `from … import` statements and their resolution
// against the walked source tree. The Python pack's counterpart to
// `imports.rs` + `resolve.rs`.
//
// Resolution is a **path match against the walk root**, the same idea as
// `java.rs` but keyed on the dotted module name rather than an FQN: CodeOwl
// is pointed at the project root (the dir holding the top package), so
// `from app.models import Item` → `app/models.py::Item`. No `pyproject.toml`
// parse, no virtualenv. Relative imports (`from . import x`,
// `from ..pkg import y`) resolve against the importing file's package.
// `import *` and single-segment `import os` (stdlib / third-party) resolve
// to nothing, like a Java star import.
//
// Python has no `export` keyword; an `__init__.py` that re-exports a name
// (`from .models import Item`) is followed **one hop** so `from app import
// Item` still lands on the real declaration.
// ---------------------------------------------------------------------------

/// One extra hop through an `__init__.py` re-export — enough for a barrel
/// `__init__`, bounded so a circular re-export can't loop.
const MAX_REEXPORT_HOPS: u8 = 1;

/// Parse a `.py` file's `import` / `from … import` statements. `import
/// a.b.c` → `{ specifier: "a.b", imported_name: "c" }`; `from a.b import x`
/// → `{ specifier: "a.b", imported_name: "x" }`; a relative `from ..pkg
/// import x` keeps the dotted prefix in the specifier (`"..pkg"`), resolved
/// later against the importing file. `from x import *` records `"*"`.
pub fn extract_imports(source: &str, _rel_path: &str) -> FileImports {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .expect("bundled tree-sitter-python grammar should always load");
    let Some(tree) = parser.parse(source, None) else {
        return FileImports::default();
    };

    let mut fi = FileImports::default();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        match node.kind() {
            "import_statement" => {
                let mut nc = node.walk();
                for name_node in node.children_by_field_name("name", &mut nc) {
                    let Some(dotted) = dotted_of(name_node) else {
                        continue;
                    };
                    let fqn = text(dotted, source);
                    let (specifier, name) = match fqn.rsplit_once('.') {
                        Some((pre, last)) => (pre.to_string(), last.to_string()),
                        None => (String::new(), fqn.to_string()),
                    };
                    fi.imports.push(ImportRef {
                        specifier,
                        imported_name: name,
                    });
                }
            }
            "import_from_statement" => {
                let Some(module) = node.child_by_field_name("module_name") else {
                    continue;
                };
                let specifier = module_specifier(module, source);
                let mut c = node.walk();
                let is_star = node
                    .children(&mut c)
                    .any(|ch| ch.kind() == "wildcard_import");
                if is_star {
                    fi.imports.push(ImportRef {
                        specifier,
                        imported_name: "*".to_string(),
                    });
                    continue;
                }
                let mut nc = node.walk();
                for name_node in node.children_by_field_name("name", &mut nc) {
                    let Some(dotted) = dotted_of(name_node) else {
                        continue;
                    };
                    fi.imports.push(ImportRef {
                        specifier: specifier.clone(),
                        imported_name: text(dotted, source).to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    fi
}

/// Unwrap an `aliased_import` (`x as y` — the alias is a local rename,
/// irrelevant to what it points at) down to its `dotted_name`.
fn dotted_of(node: Node) -> Option<Node> {
    match node.kind() {
        "dotted_name" => Some(node),
        "aliased_import" => node.child_by_field_name("name"),
        _ => None,
    }
}

/// The specifier string for a `from … import` module: a plain
/// `dotted_name` verbatim (`"app.models"`), or a `relative_import` as its
/// dot prefix plus any trailing path (`"."`, `".deps"`, `"..core"`).
fn module_specifier(module: Node, source: &str) -> String {
    if module.kind() == "relative_import" {
        let mut prefix = String::new();
        let mut rest = String::new();
        let mut c = module.walk();
        for child in module.children(&mut c) {
            match child.kind() {
                "import_prefix" => prefix = text(child, source).to_string(),
                "dotted_name" => rest = text(child, source).to_string(),
                _ => {}
            }
        }
        format!("{prefix}{rest}")
    } else {
        text(module, source).to_string()
    }
}

/// Resolve every file's imports to a target `SymbolId` (a declaration in
/// the imported module, or the module's own file node for `import a.b.c` /
/// a submodule import), or `None` (stdlib / third-party / `*` / broken).
/// Iterates files path-sorted so the persisted edge list is diffable
/// (same reasoning as `resolve::resolve_imports` / `java::resolve_imports`).
pub fn resolve_imports(
    _root: &Path,
    file_imports: &HashMap<String, FileImports>,
    graph: &Graph,
) -> Vec<ResolvedImport> {
    let mut sorted: Vec<(&String, &FileImports)> = file_imports.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    let mut out = Vec::new();
    for (from_file, fi) in &sorted {
        for imp in &fi.imports {
            out.push(ResolvedImport {
                from_file: (*from_file).clone(),
                specifier: imp.specifier.clone(),
                imported_name: imp.imported_name.clone(),
                target: resolve_one(
                    from_file,
                    &imp.specifier,
                    &imp.imported_name,
                    file_imports,
                    graph,
                    0,
                ),
            });
        }
    }
    out
}

#[allow(clippy::only_used_in_recursion)]
fn resolve_one(
    from_file: &str,
    specifier: &str,
    name: &str,
    file_imports: &HashMap<String, FileImports>,
    graph: &Graph,
    hop: u8,
) -> Option<SymbolId> {
    if name == "*" {
        return None;
    }
    let mod_path = module_path(from_file, specifier)?;

    // The module the specifier names — `a/b.py` or `a/b/__init__.py`.
    let mod_file = pick_file(file_imports, &[format!("{mod_path}.py")])
        .or_else(|| pick_file(file_imports, &[format!("{mod_path}/__init__.py")]));

    // (1) a declaration inside that module.
    if let Some(mf) = mod_file
        && let Some(id) = graph.find(&format!("{mf}::{name}"))
    {
        return Some(id);
    }
    // (2) `name` is a submodule / subpackage of the specifier.
    for cand in [
        format!("{mod_path}/{name}.py"),
        format!("{mod_path}/{name}/__init__.py"),
    ] {
        if let Some(sub) = pick_file(file_imports, &[cand])
            && let Some(id) = graph.find(sub)
        {
            return Some(id);
        }
    }
    // (3) one hop through an `__init__.py` that re-exports `name`.
    if hop < MAX_REEXPORT_HOPS
        && let Some(mf) = mod_file
        && mf.ends_with("/__init__.py")
        && let Some(fi) = file_imports.get(mf)
    {
        for reexp in &fi.imports {
            if reexp.imported_name == name {
                return resolve_one(
                    mf,
                    &reexp.specifier,
                    &reexp.imported_name,
                    file_imports,
                    graph,
                    hop + 1,
                );
            }
        }
    }
    None
}

/// The repo-relative directory-ish path a specifier names, minus any
/// extension. Absolute (`app.api.deps` → `app/api/deps`) or relative
/// (`.deps` / `..core` resolved against `from_file`'s package).
fn module_path(from_file: &str, specifier: &str) -> Option<String> {
    if !specifier.starts_with('.') {
        return Some(specifier.replace('.', "/"));
    }
    let dots = specifier.chars().take_while(|c| *c == '.').count();
    let rest = specifier[dots..].replace('.', "/");
    // 1 dot → the importing file's own package (its directory); each extra
    // dot climbs one more.
    let mut base: &str = from_file.rsplit_once('/').map_or("", |(d, _)| d);
    for _ in 1..dots {
        base = base.rsplit_once('/').map_or("", |(d, _)| d);
    }
    Some(match (base.is_empty(), rest.is_empty()) {
        (true, _) => rest,
        (false, true) => base.to_string(),
        (false, false) => format!("{base}/{rest}"),
    })
}

/// First of `candidates` that's a walked file — an exact key, else a
/// path-suffix match (covers CodeOwl being pointed above the project root).
fn pick_file<'a>(
    file_imports: &'a HashMap<String, FileImports>,
    candidates: &[String],
) -> Option<&'a str> {
    for cand in candidates {
        if file_imports.contains_key(cand) {
            return file_imports
                .keys()
                .find(|k| k.as_str() == cand)
                .map(String::as_str);
        }
        let suffix = format!("/{cand}");
        let mut hits: Vec<&str> = file_imports
            .keys()
            .map(String::as_str)
            .filter(|k| k.ends_with(&suffix))
            .collect();
        hits.sort_unstable();
        if let Some(hit) = hits.into_iter().next() {
            return Some(hit);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Flow edges — a FastAPI route's dependencies (M17). The import graph sees
// `from app.api.deps import SessionDep`, but not that a route *uses*
// `SessionDep` (which chains through `Annotated[Session, Depends(get_db)]`
// to touch a table) or `Depends(get_current_user)`. So for each route-
// decorated function, emit one edge per:
//   * `Depends(<name>)` anywhere in its params or decorator, and
//   * bare-identifier param type annotation (`session: SessionDep`).
// `resolve_flow_edge` maps the name to the *file* that declares it; the
// FastAPI feature model's `admits_to_core` then decides whether that file
// (a CRUD module, a schema-touching `deps.py`) belongs in the feature.
// ---------------------------------------------------------------------------

const ROUTE_DECORATOR_VERBS: &[&str] = &[
    ".get(",
    ".post(",
    ".put(",
    ".patch(",
    ".delete(",
    ".head(",
    ".options(",
    ".websocket(",
];

/// Type names that never name a project symbol worth pulling into a
/// feature's `core`.
const PRIMITIVE_TYPES: &[&str] = &[
    "int",
    "str",
    "bool",
    "float",
    "bytes",
    "None",
    "Any",
    "dict",
    "list",
    "set",
    "tuple",
    "object",
    "bytearray",
    "complex",
];

/// Emit a `depends` / `dep-type` edge per dependency of each route-
/// decorated function in the file.
pub fn extract_flow_edges(source: &str, rel_path: &str) -> Vec<UnresolvedFlowEdge> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .expect("bundled tree-sitter-python grammar should always load");
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut out: Vec<UnresolvedFlowEdge> = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.kind() == "decorated_definition" && is_route_decorated(node, source) {
            collect_route_deps(node, source, rel_path, &mut out);
        }
        let mut c = node.walk();
        for child in node.children(&mut c) {
            stack.push(child);
        }
    }
    out.sort_by(|a, b| (a.kind.as_str(), a.raw.as_str()).cmp(&(b.kind.as_str(), b.raw.as_str())));
    out.dedup_by(|a, b| a.kind == b.kind && a.raw == b.raw);
    out
}

fn is_route_decorated(node: Node, source: &str) -> bool {
    let mut c = node.walk();
    node.children(&mut c)
        .filter(|n| n.kind() == "decorator")
        .any(|d| {
            let t = text(d, source);
            ROUTE_DECORATOR_VERBS.iter().any(|v| t.contains(v))
        })
}

fn collect_route_deps(node: Node, source: &str, file: &str, out: &mut Vec<UnresolvedFlowEdge>) {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        match n.kind() {
            // `Depends(<name>)` — a call whose callee is the bare name
            // `Depends` and whose first argument is an identifier.
            "call" => {
                let callee = n.child_by_field_name("function").map(|f| text(f, source));
                if callee == Some("Depends")
                    && let Some(args) = n.child_by_field_name("arguments")
                {
                    let mut ac = args.walk();
                    if let Some(id) = args.children(&mut ac).find(|c| c.kind() == "identifier") {
                        out.push(UnresolvedFlowEdge {
                            from_file: file.to_string(),
                            kind: "depends".to_string(),
                            raw: text(id, source).to_string(),
                        });
                    }
                }
            }
            // A bare-identifier param type: `session: SessionDep`.
            "type" => {
                let mut tc = n.walk();
                let kids: Vec<Node> = n.children(&mut tc).collect();
                if let [only] = kids.as_slice()
                    && only.kind() == "identifier"
                {
                    let name = text(*only, source);
                    if !PRIMITIVE_TYPES.contains(&name) {
                        out.push(UnresolvedFlowEdge {
                            from_file: file.to_string(),
                            kind: "dep-type".to_string(),
                            raw: name.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push(child);
        }
    }
}

/// A `depends` / `dep-type` edge → the file node that declares `raw`, or
/// `Unresolved` (a stdlib type, an inline `Depends()` with no name, a typo).
pub fn resolve_flow_edge(graph: &Graph, edge: &UnresolvedFlowEdge) -> FlowTarget {
    if !matches!(edge.kind.as_str(), "depends" | "dep-type") {
        return FlowTarget::Unresolved;
    }
    let mut files: Vec<&str> = graph
        .symbols()
        .filter(|s| s.id.rsplit("::").next() == Some(edge.raw.as_str()))
        .map(|s| s.file.as_str())
        .collect();
    files.sort_unstable();
    files
        .into_iter()
        .next()
        .and_then(|f| graph.find(f))
        .map_or(FlowTarget::Unresolved, FlowTarget::Node)
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

    // --- imports + resolution ------------------------------------------

    #[test]
    fn parses_the_import_forms() {
        let src = r#"import os
import app.core.config
import app.core.db as database
from app.models import Item, User
from app.utils import send_email as mailer
from app.api.routes import items
from . import crud
from .deps import SessionDep
from ..core import config
from legacy import *
"#;
        let fi = extract_imports(src, "app/api/x.py");
        let got: Vec<(&str, &str)> = fi
            .imports
            .iter()
            .map(|i| (i.specifier.as_str(), i.imported_name.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("", "os"),
                ("app.core", "config"),
                ("app.core", "db"), // alias `database` dropped
                ("app.models", "Item"),
                ("app.models", "User"),
                ("app.utils", "send_email"), // alias `mailer` dropped
                ("app.api.routes", "items"),
                (".", "crud"),
                (".deps", "SessionDep"),
                ("..core", "config"),
                ("legacy", "*"),
            ]
        );
    }

    /// Extract + resolve a small fixture tree end to end.
    fn resolved(files: &[(&str, &str)]) -> (Graph, Vec<ResolvedImport>) {
        let extractions: Vec<crate::graph::FileExtraction> = files
            .iter()
            .map(|(p, src)| crate::graph::FileExtraction {
                rel_path: (*p).to_string(),
                source_hash: hash_text(src),
                symbols: extract_file(src, p),
            })
            .collect();
        let graph = Graph::build(extractions);
        let file_imports: HashMap<String, FileImports> = files
            .iter()
            .map(|(p, src)| ((*p).to_string(), extract_imports(src, p)))
            .collect();
        let edges = resolve_imports(Path::new("/unused"), &file_imports, &graph);
        (graph, edges)
    }

    #[test]
    fn from_import_resolves_to_a_declaration_in_the_module() {
        let (g, edges) = resolved(&[
            (
                "app/api/routes/items.py",
                "from app.models import Item\ndef read():\n    return Item\n",
            ),
            (
                "app/models.py",
                "class Item(SQLModel, table=True):\n    id: int\n",
            ),
        ]);
        let e = edges.iter().find(|e| e.imported_name == "Item").unwrap();
        assert_eq!(
            g.string_id(e.target.expect("Item resolves")),
            "app/models.py::Item"
        );
    }

    #[test]
    fn relative_and_parent_imports_resolve_against_the_package() {
        let (g, edges) = resolved(&[
            (
                "app/api/routes/items.py",
                "from ..deps import SessionDep\nfrom . import crud\n",
            ),
            ("app/api/deps.py", "SessionDep = object()\n"),
            ("app/api/routes/crud.py", "def create():\n    ...\n"),
        ]);
        let dep = edges
            .iter()
            .find(|e| e.imported_name == "SessionDep")
            .unwrap();
        assert_eq!(
            g.string_id(dep.target.expect("SessionDep resolves")),
            "app/api/deps.py::SessionDep"
        );
        // `from . import crud` — crud is a submodule, so the target is its
        // file node.
        let crud = edges.iter().find(|e| e.imported_name == "crud").unwrap();
        assert_eq!(
            g.string_id(crud.target.expect("crud submodule resolves")),
            "app/api/routes/crud.py"
        );
    }

    #[test]
    fn init_py_reexport_is_followed_one_hop() {
        let (g, edges) = resolved(&[
            ("app/main.py", "from app import Item\n"),
            ("app/__init__.py", "from app.models import Item\n"),
            ("app/models.py", "class Item:\n    pass\n"),
        ]);
        let e = edges
            .iter()
            .find(|e| e.from_file == "app/main.py" && e.imported_name == "Item")
            .unwrap();
        assert_eq!(
            g.string_id(e.target.expect("Item resolves through __init__")),
            "app/models.py::Item"
        );
    }

    #[test]
    fn stdlib_and_star_and_broken_imports_stay_unresolved() {
        let (_, edges) = resolved(&[(
            "app/x.py",
            "import os\nfrom typing import Any\nfrom app.gone import Thing\nfrom app.models import *\n",
        )]);
        assert_eq!(edges.len(), 4);
        assert!(edges.iter().all(|e| e.target.is_none()));
    }
}
