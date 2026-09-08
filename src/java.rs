//! Walks a single Java source file with tree-sitter and pulls out its
//! declarations — the Java pack's counterpart to `extract.rs` / `rust.rs`
//! (M16).
//!
//! Deliberately shallow like the other extractors: the compilation unit's
//! top-level types, their direct members (methods, constructors, fields),
//! and nested types (recursed fully — Java nesting is common). Method
//! bodies, statements, lambdas, and anonymous classes are implementation
//! detail, not declarations CodeOwl generates a spec for.
//!
//! Kind mapping onto the stack-neutral [`SymbolKind`] (M16): every named
//! type kind -- `class` / `interface` / `enum` / `@interface` / `record` --
//! is a `Container`. A record is just a restricted `final class` (JLS), so
//! it's treated as one: no size test, so a record-per-file DTO layout still
//! produces a file spec, and CodeOwl's model never files a type declaration
//! under `Value` (which is for `const` / field / variable). `method` /
//! `constructor` / annotation `element` -> `Callable`; `field` / interface
//! `constant` -> `Value`. The concrete keyword lands in `raw`.
//!
//! Unlike `rust.rs` there is no `merge_inherent_impls` step: a Java type's
//! body already holds its methods and fields directly, so the walk
//! produces the parent-then-children shape the Rust merge has to
//! reconstruct.

use tree_sitter::{Node, Parser};

use crate::hash::hash_text;
use crate::symbol::{ExtractedSymbol, SymbolKind};

/// Parse `source` (the contents of `rel_path`, a `.java` file) and extract
/// its top-level types and their members.
pub fn extract_file(source: &str, rel_path: &str) -> Vec<ExtractedSymbol> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .expect("bundled tree-sitter-java grammar should always load");

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

/// Handle one declaration. `parent_id` is the enclosing type this item is
/// nested in, or `None` at compilation-unit top level.
fn visit_item(
    node: Node,
    parent_id: Option<&str>,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    match node.kind() {
        "class_declaration" => visit_container(node, "class", parent_id, source, file, out),
        "interface_declaration" => visit_container(node, "interface", parent_id, source, file, out),
        "enum_declaration" => visit_container(node, "enum", parent_id, source, file, out),
        "annotation_type_declaration" => {
            visit_container(node, "@interface", parent_id, source, file, out)
        }
        "record_declaration" => visit_container(node, "record", parent_id, source, file, out),
        "method_declaration" => {
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
        "constructor_declaration" | "compact_constructor_declaration" => {
            push_named_leaf(
                node,
                "<init>",
                SymbolKind::Callable,
                "constructor",
                parent_id,
                source,
                file,
                out,
            );
        }
        "annotation_type_element_declaration" => {
            push_leaf(
                node,
                SymbolKind::Callable,
                "element",
                parent_id,
                source,
                file,
                out,
            );
        }
        // `int a, b;` is one declaration binding several names — one `Value`
        // symbol per name.
        "field_declaration" | "constant_declaration" => {
            let raw = if node.kind() == "constant_declaration" {
                "constant"
            } else {
                "field"
            };
            let mut cursor = node.walk();
            for declarator in node.children(&mut cursor) {
                if declarator.kind() != "variable_declarator" {
                    continue;
                }
                let Some(name) = field_text(declarator, "name", source) else {
                    continue;
                };
                push_named_leaf(
                    node,
                    name,
                    SymbolKind::Value,
                    raw,
                    parent_id,
                    source,
                    file,
                    out,
                );
            }
        }
        // The `; <members>` tail of an enum body.
        "enum_body_declarations" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                visit_item(child, parent_id, source, file, out);
            }
        }
        _ => {}
    }
}

/// A type with a body — recurse one level for its members (fully into
/// nested types), then Merkle-fold their `source_hash`es into the
/// container's own, matching `extract.rs` / `rust.rs`.
fn visit_container(
    node: Node,
    raw: &str,
    parent_id: Option<&str>,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    let name = type_name(node, source);
    let id = match parent_id {
        Some(p) => format!("{p}::{name}"),
        None => format!("{file}::{name}"),
    };
    let before = out.len();

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            visit_item(child, Some(&id), source, file, out);
        }
    }

    // Direct children only — a nested type recursed above has already
    // pushed its own members and (below) folded their hashes into its
    // `source_hash`, so folding the nested type's own hash here covers
    // them transitively.
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

    // A top-level Java type is always at least package-visible, so it's
    // reachable (and spec-bearing); a nested type needs `public`/`protected`.
    let is_exported = parent_id.is_none() || has_public_or_protected(node);

    // Insert the container *before* its members so declaration order in
    // `out` stays parent-then-children.
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
            markers: annotations(node, source),
            parent: parent_id.map(str::to_string),
            children: member_ids,
        },
    );
}

/// A leaf symbol whose name comes from the node's own `name` field.
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
    push_named_leaf(node, name, kind, raw, parent_id, source, file, out);
}

/// A leaf symbol with an explicitly supplied name (a constructor's `<init>`,
/// one declarator of a multi-name field).
#[allow(clippy::too_many_arguments)]
fn push_named_leaf(
    node: Node,
    name: &str,
    kind: SymbolKind,
    raw: &str,
    parent_id: Option<&str>,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    let id = match parent_id {
        Some(p) => format!("{p}::{name}"),
        None => format!("{file}::{name}"),
    };
    let sig = signature_before_body(node, source);
    // A member is never independently imported — same stance as
    // `extract.rs` / `rust.rs` for a class method.
    let is_exported = parent_id.is_none() && has_public_or_protected(node);
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
        markers: annotations(node, source),
        parent: parent_id.map(str::to_string),
        children: Vec::new(),
    });
}

fn type_name(node: Node, source: &str) -> String {
    field_text(node, "name", source)
        .unwrap_or("<anon>")
        .to_string()
}

/// Everything from the declaration's start up to its `body` (or its
/// terminating `;` / end) — modifiers, keyword, name, type parameters,
/// `extends` / `implements`, params, return type. The "text before the
/// `{`" trick.
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

/// `public` or `protected` in the declaration's `modifiers` child.
fn has_public_or_protected(node: Node) -> bool {
    let Some(modifiers) = modifiers_of(node) else {
        return false;
    };
    let mut cursor = modifiers.walk();
    modifiers
        .children(&mut cursor)
        .any(|c| matches!(c.kind(), "public" | "protected"))
}

/// The `modifiers` child of a declaration, if present. `class_declaration`
/// exposes it as a field; a few nodes only have it as an ordinary child.
fn modifiers_of(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("modifiers").or_else(|| {
        let mut c = node.walk();
        node.children(&mut c).find(|n| n.kind() == "modifiers")
    })
}

/// Contiguous run of `/** … */` Javadoc block comments immediately above
/// `node`. A blank line, or any non-comment node, ends the run — so a
/// comment paragraphs above an unrelated declaration isn't misattributed.
/// (Annotations sit inside the `modifiers` child, not as prev-siblings, so
/// there's no "step over attributes" clause to carry from `rust.rs`.)
fn leading_doc(node: Node, source: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut expected_row = node.start_position().row;
    let mut cursor = node.prev_sibling();

    while let Some(n) = cursor {
        let end = n.end_position();
        let end_row_exclusive = end.row + usize::from(end.column > 0);
        if end_row_exclusive != expected_row {
            break;
        }
        match n.kind() {
            "block_comment" | "line_comment" => {
                if let Some(content) = doc_comment_body(text(n, source)) {
                    lines.extend(content.into_iter().rev());
                }
                // a plain `//` / `/* */` comment is stepped over
            }
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

/// `Some(content_lines)` for a Javadoc `/** … */`; `None` for an ordinary
/// `//` or `/* */` comment (stepped over, not a break).
fn doc_comment_body(raw: &str) -> Option<Vec<String>> {
    let inner = raw.strip_prefix("/**").and_then(|s| s.strip_suffix("*/"))?;
    Some(
        inner
            .lines()
            .map(|l| l.trim().trim_start_matches('*').trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
    )
}

/// The `@Annotation` items in the declaration's `modifiers` child,
/// verbatim — `@Deprecated`, `@Override`, `@Path("/x")`. This is what a
/// `JavaStack` reads back for its feature / schema model (M17); mostly
/// empty for commons-lang.
fn annotations(node: Node, source: &str) -> Vec<String> {
    let Some(modifiers) = modifiers_of(node) else {
        return Vec::new();
    };
    let mut cursor = modifiers.walk();
    modifiers
        .children(&mut cursor)
        .filter(|c| matches!(c.kind(), "marker_annotation" | "annotation"))
        .map(|c| {
            text(c, source)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
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
    fn extracts_a_public_class_with_static_methods() {
        let src = "\
package org.example;\n\
\n\
/**\n * Utility helpers.\n */\n\
public final class Helpers {\n\
    private Helpers() {}\n\
    public static boolean isEmpty(String s) { return s == null || s.isEmpty(); }\n\
    static int internal() { return 0; }\n\
}\n";
        let syms = extract_file(src, "src/main/java/org/example/Helpers.java");
        let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "src/main/java/org/example/Helpers.java::Helpers",
                "src/main/java/org/example/Helpers.java::Helpers::<init>",
                "src/main/java/org/example/Helpers.java::Helpers::isEmpty",
                "src/main/java/org/example/Helpers.java::Helpers::internal",
            ]
        );

        let class = &syms[0];
        assert_eq!(class.kind, SymbolKind::Container);
        assert_eq!(class.raw, "class");
        assert!(class.is_exported);
        assert_eq!(class.docstring.as_deref(), Some("Utility helpers."));
        assert_eq!(class.signature, "public final class Helpers");
        assert_eq!(class.children.len(), 3);

        let ctor = &syms[1];
        assert_eq!(ctor.raw, "constructor");
        assert_eq!(ctor.parent.as_deref(), Some(ids[0]));
        assert!(!ctor.is_exported, "a private constructor isn't exported");

        let is_empty = &syms[2];
        assert_eq!(is_empty.raw, "method");
        assert!(
            !is_empty.is_exported,
            "a member is never top-level-exported"
        );
        assert_eq!(
            is_empty.signature,
            "public static boolean isEmpty(String s)"
        );
    }

    #[test]
    fn package_private_top_level_class_is_still_exported() {
        // Top-level Java types are at least package-visible, so CodeOwl
        // treats them as reachable (spec-bearing, interface-hashed).
        let syms = extract_file("class Internal {}\n", "a.java");
        assert_eq!(syms.len(), 1);
        assert!(syms[0].is_exported);
        assert!(syms[0].interface_hash.is_some());
    }

    #[test]
    fn nested_enum_builds_a_containment_tree() {
        let src = "\
public class Processor {\n\
    public enum Arch {\n\
        BIT_32(\"32-bit\"), BIT_64(\"64-bit\");\n\
        private final String label;\n\
        Arch(String label) { this.label = label; }\n\
        public String getLabel() { return label; }\n\
    }\n\
}\n";
        let syms = extract_file(src, "P.java");
        let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "P.java::Processor",
                "P.java::Processor::Arch",
                "P.java::Processor::Arch::label",
                "P.java::Processor::Arch::<init>",
                "P.java::Processor::Arch::getLabel",
            ]
        );
        let arch = &syms[1];
        assert_eq!(arch.kind, SymbolKind::Container);
        assert_eq!(arch.raw, "enum");
        assert_eq!(arch.parent.as_deref(), Some("P.java::Processor"));
        assert!(arch.is_exported); // nested but `public`
        assert_eq!(
            syms[0].children,
            vec!["P.java::Processor::Arch".to_string()]
        );
    }

    #[test]
    fn javadoc_leading_star_is_stripped_and_a_far_comment_is_not_attached() {
        let attached = "/**\n * First line.\n * Second line.\n */\npublic class A {}\n";
        assert_eq!(
            extract_file(attached, "a.java")[0].docstring.as_deref(),
            Some("First line.\nSecond line.")
        );

        let far = "// unrelated\n\npublic class B {}\n";
        assert_eq!(extract_file(far, "b.java")[0].docstring, None);
    }

    #[test]
    fn annotations_land_in_markers() {
        let src = "@Deprecated\n@FunctionalInterface\npublic interface Callback { void call(); }\n";
        let syms = extract_file(src, "c.java");
        assert_eq!(syms[0].raw, "interface");
        assert_eq!(syms[0].markers, vec!["@Deprecated", "@FunctionalInterface"]);
    }

    #[test]
    fn annotation_type_is_a_container_with_its_elements() {
        let src = "\
public @interface JsonProperty {\n\
    String value() default \"\";\n\
    boolean required() default false;\n\
}\n";
        let syms = extract_file(src, "j.java");
        assert_eq!(syms[0].kind, SymbolKind::Container);
        assert_eq!(syms[0].raw, "@interface");
        let elems: Vec<&str> = syms[1..].iter().map(|s| s.raw.as_str()).collect();
        assert_eq!(elems, vec!["element", "element"]);
    }

    #[test]
    fn a_record_is_a_container_like_any_other_named_type() {
        // A record is a restricted `final class` (JLS) -- CodeOwl treats it
        // as one, with no size test, so a bare DTO still gets a file spec
        // and a type declaration is never filed under `Value`.
        let bare = extract_file("public record Point(int x, int y) {}\n", "p.java");
        assert_eq!(bare.len(), 1);
        assert_eq!(bare[0].kind, SymbolKind::Container);
        assert_eq!(bare[0].raw, "record");

        let with_method = extract_file(
            "public record Range(int lo, int hi) { boolean has(int n) { return lo <= n && n <= hi; } }\n",
            "r.java",
        );
        assert_eq!(with_method[0].kind, SymbolKind::Container);
        assert_eq!(with_method[0].raw, "record");
        assert!(with_method.iter().any(|s| s.id == "r.java::Range::has"));
    }

    #[test]
    fn multi_name_field_yields_one_symbol_per_name() {
        let src = "class C {\n    private int width, height;\n}\n";
        let syms = extract_file(src, "c.java");
        let fields: Vec<&str> = syms
            .iter()
            .filter(|s| s.raw == "field")
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(fields, vec!["c.java::C::width", "c.java::C::height"]);
    }

    #[test]
    fn interface_default_method_and_constant() {
        let src = "\
public interface Sized {\n\
    int UNKNOWN = -1;\n\
    int size();\n\
    default boolean isEmpty() { return size() == 0; }\n\
}\n";
        let syms = extract_file(src, "s.java");
        assert_eq!(syms[0].raw, "interface");
        let members: Vec<(&str, &str)> = syms[1..]
            .iter()
            .map(|s| (s.id.rsplit("::").next().unwrap(), s.raw.as_str()))
            .collect();
        assert_eq!(
            members,
            vec![
                ("UNKNOWN", "constant"),
                ("size", "method"),
                ("isEmpty", "method")
            ]
        );
    }

    #[test]
    fn method_body_edit_moves_source_hash_but_not_interface_hash() {
        let a = extract_file(
            "public class C { public int m() { return 1; } }\n",
            "c.java",
        );
        let b = extract_file(
            "public class C { public int m() { return 2; } }\n",
            "c.java",
        );
        // The class's Merkle rollup picks up the method-body change...
        assert_ne!(a[0].source_hash, b[0].source_hash);
        // ...but its exported signature didn't change.
        assert_eq!(a[0].interface_hash, b[0].interface_hash);
    }
}
