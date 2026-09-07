//! Walks a single Rust source file with tree-sitter and pulls out its
//! top-level items — the Rust pack's counterpart to `extract.rs` (M14).
//!
//! Deliberately shallow, like `extract.rs`: `source_file`'s direct
//! children, plus one level into `impl` / `trait` / inline `mod` bodies
//! for the callables they contain. Function bodies, struct fields, enum
//! variants, and nested `fn`s are implementation detail, not declarations
//! CodeOwl generates a spec for.
//!
//! Kind mapping onto the stack-neutral [`SymbolKind`] (M14 / design
//! decision 1): `fn` -> `Callable`, `struct`/`enum`/`union`/`trait`/`impl`/
//! `mod` -> `Container`, `const`/`static` -> `Value`, `macro_rules!` ->
//! `Callable`. The concrete keyword lands in `raw`.

use tree_sitter::{Node, Parser};

use crate::hash::hash_text;
use crate::symbol::{ExtractedSymbol, SymbolKind};

/// Parse `source` (the contents of `rel_path`, a `.rs` file) and extract
/// its top-level items plus one level of `impl`/`trait`/`mod` members.
pub fn extract_file(source: &str, rel_path: &str) -> Vec<ExtractedSymbol> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("bundled tree-sitter-rust grammar should always load");

    // `tree` owns the arena every `Node<'_>` below borrows from — nothing
    // escapes this function (CLAUDE.md's "don't let Node<'a> escape" rule).
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        visit_item(child, None, source, rel_path, &mut out);
    }
    out
}

/// Handle one item. `parent_id` is the container this item is nested in
/// (an `impl`/`trait`/`mod`), or `None` at file top level.
fn visit_item(
    node: Node,
    parent_id: Option<&str>,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    match node.kind() {
        "function_item" => {
            let raw = if parent_id.is_some() { "method" } else { "fn" };
            push_leaf(
                node,
                SymbolKind::Callable,
                raw,
                parent_id,
                source,
                file,
                out,
            );
        }
        "function_signature_item" => {
            // A trait method with no default body — still part of the
            // trait's surface.
            push_leaf(
                node,
                SymbolKind::Callable,
                "method",
                parent_id,
                source,
                file,
                out,
            );
        }
        "struct_item" => push_leaf(
            node,
            SymbolKind::Container,
            "struct",
            parent_id,
            source,
            file,
            out,
        ),
        "enum_item" => push_leaf(
            node,
            SymbolKind::Container,
            "enum",
            parent_id,
            source,
            file,
            out,
        ),
        "union_item" => push_leaf(
            node,
            SymbolKind::Container,
            "union",
            parent_id,
            source,
            file,
            out,
        ),
        "const_item" => push_leaf(
            node,
            SymbolKind::Value,
            "const",
            parent_id,
            source,
            file,
            out,
        ),
        "static_item" => push_leaf(
            node,
            SymbolKind::Value,
            "static",
            parent_id,
            source,
            file,
            out,
        ),
        "type_item" => push_leaf(
            node,
            SymbolKind::Value,
            "type",
            parent_id,
            source,
            file,
            out,
        ),
        "macro_definition" => push_leaf(
            node,
            SymbolKind::Callable,
            "macro_rules",
            parent_id,
            source,
            file,
            out,
        ),
        "trait_item" => visit_container(node, "trait", trait_name(node, source), source, file, out),
        "impl_item" => {
            let name = impl_header(node, source);
            visit_container(node, "impl", name, source, file, out)
        }
        "mod_item" if node.child_by_field_name("body").is_some() && !is_cfg_test(node, source) => {
            let name = field_text(node, "name", source)
                .unwrap_or("<mod>")
                .to_string();
            visit_container(node, "mod", name, source, file, out)
        }
        _ => {}
    }
}

/// A `#[cfg(test)] mod tests { … }` block — unit tests colocated in a
/// source file. Skipped entirely (the whole subtree), the same call
/// `classify` makes for a `tests/` file: test code isn't spec-bearing and
/// shouldn't pad a domain file's symbol list.
fn is_cfg_test(node: Node, source: &str) -> bool {
    attributes(node, source)
        .iter()
        .any(|a| a.replace(' ', "").contains("cfg(test)"))
}

/// A `Container` with a body — recurse one level for the callables it
/// holds, then Merkle-fold their `source_hash`es into the container's own
/// (an edit to any member is an edit to the container), matching how
/// `extract.rs` treats a class.
fn visit_container(
    node: Node,
    raw: &str,
    name: String,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    let id = format!("{file}::{name}");
    let before = out.len();

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            visit_item(child, Some(&id), source, file, out);
        }
    }

    let member_ids: Vec<String> = out[before..].iter().map(|s| s.id.clone()).collect();
    let mut rollup = signature_before_body(node, source);
    for s in &out[before..] {
        rollup.push('\n');
        rollup.push_str(&s.source_hash);
    }

    let is_exported = has_pub(node, source);
    let sig = signature_before_body(node, source);
    // Insert the container *before* its members so declaration order in
    // `out` stays parent-then-children, like `extract.rs`.
    out.insert(
        before,
        ExtractedSymbol {
            id,
            kind: SymbolKind::Container,
            raw: raw.to_string(),
            file: file.to_string(),
            lines: node_lines(node),
            signature: sig.clone(),
            docstring: leading_doc(node, source),
            is_exported,
            source_hash: hash_text(&rollup),
            interface_hash: is_exported.then(|| hash_text(&sig)),
            markers: attributes(node, source),
            parent: None,
            children: member_ids,
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn push_leaf(
    node: Node,
    kind: SymbolKind,
    raw: &str,
    parent_id: Option<&str>,
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
    // A member (inside an impl/trait) is never independently imported —
    // same stance as `extract.rs` for a class method.
    let is_exported = parent_id.is_none() && has_pub(node, source);
    out.push(ExtractedSymbol {
        id,
        kind,
        raw: raw.to_string(),
        file: file.to_string(),
        lines: node_lines(node),
        signature: sig.clone(),
        docstring: leading_doc(node, source),
        is_exported,
        source_hash: hash_text(text(node, source)),
        interface_hash: is_exported.then(|| hash_text(&sig)),
        markers: attributes(node, source),
        parent: parent_id.map(str::to_string),
        children: Vec::new(),
    });
}

/// `impl Graph` / `impl Display for Graph` / `impl<T> Foo<T>` — the source
/// text of everything before the body, `impl ` prefix kept so an `impl`
/// symbol's id can't collide with the `struct` it's for.
fn impl_header(node: Node, source: &str) -> String {
    signature_before_body(node, source)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn trait_name(node: Node, source: &str) -> String {
    field_text(node, "name", source)
        .unwrap_or("<trait>")
        .to_string()
}

/// Everything from the item's start up to its `body` (or its terminating
/// `;` / end) — modifiers, keyword, name, generics, params, return type,
/// `where` clause. The "text before the `{`" trick from `extract.rs`.
fn signature_before_body(node: Node, source: &str) -> String {
    let start = node.start_byte();
    let end = node
        .child_by_field_name("body")
        .map(|b| b.start_byte())
        .unwrap_or_else(|| node.end_byte())
        .max(start);
    source[start..end]
        .trim_end()
        .trim_end_matches(';')
        .trim_end()
        .to_string()
}

/// Contiguous run of `///` (or `//!`, or a `/** */` block) doc comments
/// immediately above `node`. `#[...]` attributes and plain `//` comments
/// between the doc and the item are stepped over (they belong to the
/// item); anything else, or a blank line, ends the run — so a comment
/// paragraphs above an unrelated item isn't misattributed.
fn leading_doc(node: Node, source: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    // The line the current run starts on; a preceding node is adjacent iff
    // it ends where this row begins (nothing, not even a blank line, in
    // between). Comment nodes include their trailing newline, so an
    // end column of 0 means "ends at the start of the next line".
    let mut expected_row = node.start_position().row;
    let mut cursor = node.prev_sibling();

    while let Some(n) = cursor {
        let end = n.end_position();
        let end_row_exclusive = end.row + usize::from(end.column > 0);
        if end_row_exclusive != expected_row {
            break;
        }
        match n.kind() {
            "line_comment" | "block_comment" => {
                if let Some(content) = doc_comment_body(text(n, source)) {
                    lines.extend(content.into_iter().rev());
                }
                // a plain `//` comment is stepped over, not collected
            }
            "attribute_item" | "inner_attribute_item" => {}
            _ => break,
        }
        expected_row = n.start_position().row;
        cursor = n.prev_sibling();
    }

    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    Some(lines.join("\n"))
}

/// `Some(content_lines)` for a doc comment (`///` / `//!` / `/** */`),
/// `None` for an ordinary `//` / `/* */` comment (skipped, not a break).
fn doc_comment_body(raw: &str) -> Option<Vec<String>> {
    if let Some(rest) = raw.strip_prefix("///").or_else(|| raw.strip_prefix("//!")) {
        return Some(vec![rest.trim().to_string()]);
    }
    if let Some(inner) = raw
        .strip_prefix("/**")
        .or_else(|| raw.strip_prefix("/*!"))
        .and_then(|s| s.strip_suffix("*/"))
    {
        return Some(
            inner
                .lines()
                .map(|l| l.trim().trim_start_matches('*').trim().to_string())
                .filter(|l| !l.is_empty())
                .collect(),
        );
    }
    None
}

/// The `#[...]` attribute items directly above `node`, verbatim — this is
/// what a `RustStack` reads back for `#[tool]` / `#[derive(...)]` / a test
/// attribute (M14 / design decision 9). Adjacency isn't required: an
/// attribute always abuts its item.
fn attributes(node: Node, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = node.prev_sibling();
    while let Some(n) = cursor {
        match n.kind() {
            "attribute_item" | "inner_attribute_item" => out.push(text(n, source).to_string()),
            "line_comment" | "block_comment" => {}
            _ => break,
        }
        cursor = n.prev_sibling();
    }
    out.reverse();
    out
}

/// A `pub` / `pub(crate)` / `pub(super)` visibility modifier on the item.
fn has_pub(node: Node, source: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| c.kind() == "visibility_modifier" && text(c, source).starts_with("pub"))
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
    fn extracts_a_free_function() {
        let src = "/// Doubles it.\npub fn double(x: i64) -> i64 {\n    x * 2\n}\n";
        let syms = extract_file(src, "src/math.rs");
        assert_eq!(syms.len(), 1);
        let s = &syms[0];
        assert_eq!(s.id, "src/math.rs::double");
        assert_eq!(s.kind, SymbolKind::Callable);
        assert_eq!(s.raw, "fn");
        assert_eq!(s.signature, "pub fn double(x: i64) -> i64");
        assert_eq!(s.docstring.as_deref(), Some("Doubles it."));
        assert!(s.is_exported);
        assert!(s.parent.is_none());
    }

    #[test]
    fn private_function_is_not_exported() {
        let syms = extract_file("fn helper() {}\n", "a.rs");
        assert_eq!(syms.len(), 1);
        assert!(!syms[0].is_exported);
        assert_eq!(syms[0].interface_hash, None);
    }

    #[test]
    fn struct_and_its_impl_methods_build_a_containment_tree() {
        let src = "\
pub struct Counter {\n    n: u64,\n}\n\n\
impl Counter {\n\
    /// Start at zero.\n\
    pub fn new() -> Self {\n        Self { n: 0 }\n    }\n\
    fn bump(&mut self) {\n        self.n += 1;\n    }\n\
}\n";
        let syms = extract_file(src, "src/counter.rs");
        let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "src/counter.rs::Counter",
                "src/counter.rs::impl Counter",
                "src/counter.rs::impl Counter::new",
                "src/counter.rs::impl Counter::bump",
            ]
        );

        let the_struct = &syms[0];
        assert_eq!(the_struct.kind, SymbolKind::Container);
        assert_eq!(the_struct.raw, "struct");

        let the_impl = &syms[1];
        assert_eq!(the_impl.kind, SymbolKind::Container);
        assert_eq!(the_impl.raw, "impl");
        assert_eq!(
            the_impl.children,
            vec![
                "src/counter.rs::impl Counter::new",
                "src/counter.rs::impl Counter::bump"
            ]
        );

        let new = &syms[2];
        assert_eq!(new.raw, "method");
        assert_eq!(new.parent.as_deref(), Some("src/counter.rs::impl Counter"));
        assert_eq!(new.docstring.as_deref(), Some("Start at zero."));
        assert!(!new.is_exported);
    }

    #[test]
    fn impl_source_hash_folds_in_its_methods() {
        let a = extract_file(
            "struct S;\nimpl S {\n    fn f(&self) { let x = 1; }\n}\n",
            "a.rs",
        );
        let b = extract_file(
            "struct S;\nimpl S {\n    fn f(&self) { let x = 2; }\n}\n",
            "a.rs",
        );
        let impl_a = a.iter().find(|s| s.id == "a.rs::impl S").unwrap();
        let impl_b = b.iter().find(|s| s.id == "a.rs::impl S").unwrap();
        assert_ne!(impl_a.source_hash, impl_b.source_hash);
    }

    #[test]
    fn trait_for_type_impl_header_is_disambiguated() {
        let src = "struct S;\nimpl std::fmt::Debug for S {\n    fn fmt(&self) {}\n}\n";
        let syms = extract_file(src, "a.rs");
        assert!(
            syms.iter()
                .any(|s| s.id == "a.rs::impl std::fmt::Debug for S")
        );
    }

    #[test]
    fn derive_and_attributes_land_in_markers() {
        let src = "#[derive(Debug, Clone)]\n#[serde(rename_all = \"snake_case\")]\npub enum Kind {\n    A,\n}\n";
        let syms = extract_file(src, "a.rs");
        let e = &syms[0];
        assert_eq!(e.raw, "enum");
        assert_eq!(
            e.markers,
            vec![
                "#[derive(Debug, Clone)]",
                "#[serde(rename_all = \"snake_case\")]"
            ]
        );
    }

    #[test]
    fn const_and_static_are_values() {
        let syms = extract_file(
            "pub const MAX: usize = 8;\nstatic NAME: &str = \"x\";\n",
            "a.rs",
        );
        assert_eq!(syms[0].kind, SymbolKind::Value);
        assert_eq!(syms[0].raw, "const");
        assert_eq!(syms[1].kind, SymbolKind::Value);
        assert_eq!(syms[1].raw, "static");
    }

    #[test]
    fn macro_rules_is_a_callable() {
        let syms = extract_file("macro_rules! shout {\n    () => {};\n}\n", "a.rs");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].id, "a.rs::shout");
        assert_eq!(syms[0].raw, "macro_rules");
        assert_eq!(syms[0].kind, SymbolKind::Callable);
    }

    #[test]
    fn inline_module_nests_its_items() {
        let src = "pub mod parse {\n    pub fn run() {}\n    struct Inner;\n}\n";
        let syms = extract_file(src, "a.rs");
        let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["a.rs::parse", "a.rs::parse::run", "a.rs::parse::Inner"]
        );
        assert_eq!(syms[0].raw, "mod");
        assert_eq!(syms[1].parent.as_deref(), Some("a.rs::parse"));
    }

    #[test]
    fn mod_declaration_without_a_body_is_skipped() {
        // `mod foo;` is an import-graph edge (commit 4), not a symbol.
        let syms = extract_file("mod foo;\npub fn g() {}\n", "a.rs");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].id, "a.rs::g");
    }

    #[test]
    fn cfg_test_module_is_skipped_whole() {
        let src = "\
pub fn real() {}\n\n\
#[cfg(test)]\n\
mod tests {\n\
    use super::*;\n\
    #[test]\n\
    fn a_test() { real(); }\n\
}\n";
        let syms = extract_file(src, "a.rs");
        assert_eq!(
            syms.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["a.rs::real"]
        );
    }

    #[test]
    fn extracts_this_crate_s_own_graph_module() {
        // The M14 dogfood check: parse a real file from this repo.
        let src = std::fs::read_to_string("src/graph.rs").unwrap();
        let syms = extract_file(&src, "src/graph.rs");
        let ids: std::collections::HashSet<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert!(
            ids.contains("src/graph.rs::Graph"),
            "missing the Graph type"
        );
        assert!(
            ids.contains("src/graph.rs::FORMAT_VERSION"),
            "missing the FORMAT_VERSION const"
        );
        assert!(
            syms.iter()
                .any(|s| s.id.starts_with("src/graph.rs::impl Graph") && s.raw == "impl"),
            "missing an impl Graph block"
        );
        assert!(
            syms.iter()
                .any(|s| s.raw == "method" && s.id.contains("::build")),
            "missing Graph::build"
        );
        // Every symbol carries a concrete keyword.
        assert!(syms.iter().all(|s| !s.raw.is_empty()));
    }
}
