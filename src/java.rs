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

use std::collections::HashMap;
use std::path::Path;

use tree_sitter::{Node, Parser};

use crate::graph::{Graph, SymbolId};
use crate::hash::hash_text;
use crate::imports::{FileImports, ImportRef};
use crate::resolve::ResolvedImport;
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

// ---------------------------------------------------------------------------
// Imports — `import` declarations and their resolution against the source
// tree. The Java pack's counterpart to `imports.rs` + `resolve.rs`. Two
// kinds of reference edge feed the graph:
//   * explicit `import a.b.C;` / `import static a.b.C.m;` — the FQN maps to
//     the walked `.java` file whose path ends `a/b/C.java` (a path-suffix
//     match, so Maven / Gradle / bare layouts all work with no source-root
//     discovery and — for M17 — cross-module resolution is free).
//   * same-package implicit references — a Java class names a sibling in
//     its own package with no `import` at all (`StringUtils` -> `ObjectUtils`).
//     Detected by scanning a file's source for each sibling's simple name.
// Java has no re-export (`pub use`) or default-import concept, so
// `FileImports.re_exports` / `default_imports` stay empty.
// ---------------------------------------------------------------------------

/// Parse a `.java` file's `import` declarations. `import a.b.C;` -> `{
/// specifier: "a.b", imported_name: "C" }`; `import static a.b.C.m;` keeps
/// the class in the specifier (`"a.b.C"` / `"m"`); `import a.b.*;` records
/// `"*"` as the name (it names no single declaration, so it resolves to
/// nothing — most Java projects, commons-lang included, forbid star imports
/// via checkstyle anyway).
pub fn extract_imports(source: &str, _rel_path: &str) -> FileImports {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .expect("bundled tree-sitter-java grammar should always load");
    let Some(tree) = parser.parse(source, None) else {
        return FileImports::default();
    };

    let mut fi = FileImports::default();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        if node.kind() != "import_declaration" {
            continue;
        }
        let mut dc = node.walk();
        let kids: Vec<Node> = node.children(&mut dc).collect();
        let is_star = kids.iter().any(|c| c.kind() == "asterisk");
        // The dotted path is a `scoped_identifier` (`a.b.C`), or a bare
        // `identifier` for a single-segment `import C;` / `import a.*;`.
        let Some(path) = kids
            .iter()
            .find(|c| matches!(c.kind(), "scoped_identifier" | "identifier"))
        else {
            continue;
        };
        let fqn = text(*path, source);
        // `static` vs not doesn't change extraction — the FQN split lands
        // the class in `specifier` either way (`a.b.C.m` -> `a.b.C` / `m`;
        // `a.b.C` -> `a.b` / `C`). It matters only for resolution.
        let (specifier, name) = if is_star {
            (fqn.to_string(), "*".to_string())
        } else if let Some((pre, last)) = fqn.rsplit_once('.') {
            (pre.to_string(), last.to_string())
        } else {
            continue; // `import C;` from the default package — nothing to key on
        };
        fi.imports.push(ImportRef {
            specifier,
            imported_name: name,
        });
    }
    fi
}

/// Resolve every file's `import`s to a target `SymbolId` and append the
/// same-package implicit reference edges. Iterates files in path-sorted
/// order so the persisted edge list is diffable (same reasoning as
/// `resolve::resolve_imports`). `root` *is* read here — unlike the Rust
/// pack — because the same-package scan needs each file's source text, and
/// the graph doesn't retain bodies.
pub fn resolve_imports(
    root: &Path,
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
                target: resolve_explicit(&imp.specifier, &imp.imported_name, file_imports, graph),
            });
        }
    }
    out.extend(same_package_edges(root, &sorted, graph));
    out
}

/// `a.b.C` (or a longer `a.b.C.member`) -> the walked `.java` file whose
/// path ends `a/b/C.java`. Path-suffix match: layout-agnostic, no
/// source-root discovery. On the rare tie (the same FQN under two source
/// roots — e.g. main and test) the path-sorted first wins, for determinism.
fn fqn_to_file<'a>(fqn: &str, file_imports: &'a HashMap<String, FileImports>) -> Option<&'a str> {
    let rel = format!("{}.java", fqn.replace('.', "/"));
    let suffix = format!("/{rel}");
    let mut hits: Vec<&str> = file_imports
        .keys()
        .map(String::as_str)
        .filter(|k| *k == rel || k.ends_with(&suffix))
        .collect();
    hits.sort_unstable();
    hits.into_iter().next()
}

/// One explicit import -> the symbol it names, or `None` (star import,
/// external package, or a target file with no matching declaration).
fn resolve_explicit(
    specifier: &str,
    name: &str,
    file_imports: &HashMap<String, FileImports>,
    graph: &Graph,
) -> Option<SymbolId> {
    if name == "*" {
        return None; // names no single declaration
    }
    // `import a.b.C;` — the FQN `a.b.C` is itself a top-level type.
    if let Some(id) = fqn_to_file(&format!("{specifier}.{name}"), file_imports)
        .and_then(|file| graph.find(&format!("{file}::{name}")))
    {
        return Some(id);
    }
    // `import static a.b.C.m;` or `import a.b.Outer.Inner;` — here the
    // *specifier* is the type FQN. Resolve to the nested member / static
    // method if the graph has it, otherwise to the enclosing type (a
    // method-level edge is call-graph granularity, deferred).
    if let Some(file) = fqn_to_file(specifier, file_imports) {
        let ty = specifier.rsplit('.').next().unwrap_or(specifier);
        for cand in [
            format!("{file}::{ty}::{name}"),
            format!("{file}::{name}"),
            format!("{file}::{ty}"),
        ] {
            if let Some(id) = graph.find(&cand) {
                return Some(id);
            }
        }
    }
    None
}

/// The same-package implicit edges: a Java class refers to another class in
/// its own package with no `import`. For each file, scan its source for the
/// simple name of every *other* top-level container declared in the same
/// directory; a whole-word hit is a reference edge. `specifier` is set to
/// the package name so the edge is visibly not a written import.
fn same_package_edges(
    root: &Path,
    sorted_files: &[(&String, &FileImports)],
    graph: &Graph,
) -> Vec<ResolvedImport> {
    // (simple name, id, file) for every top-level container in the graph.
    let containers: Vec<(&str, SymbolId, &str)> = graph
        .symbols()
        .filter(|s| matches!(s.kind, SymbolKind::Container))
        .filter_map(|s| {
            let rest = s.id.strip_prefix(&format!("{}::", s.file))?;
            if rest.contains("::") {
                return None; // a nested type, not top-level
            }
            Some((rest, graph.find(&s.id)?, s.file.as_str()))
        })
        .collect();

    let mut out = Vec::new();
    for (from_file, _) in sorted_files {
        let dir = dir_of(from_file);
        let siblings: Vec<&(&str, SymbolId, &str)> = containers
            .iter()
            .filter(|(_, _, f)| *f != from_file.as_str() && dir_of(f) == dir)
            .collect();
        if siblings.is_empty() {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(root.join(from_file.as_str())) else {
            continue; // file gone (mid-watch delete) or unreadable — skip
        };
        for (name, id, _) in siblings {
            if mentions_identifier(&src, name) {
                out.push(ResolvedImport {
                    from_file: (*from_file).clone(),
                    specifier: package_of(dir),
                    imported_name: (*name).to_string(),
                    target: Some(*id),
                });
            }
        }
    }
    out
}

/// The directory portion of a repo-relative path (`""` for a root file).
fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// A package directory -> its dotted name, stripping the source-root prefix
/// for the standard layouts. Cosmetic — it's only the `specifier` string on
/// a synthetic edge.
fn package_of(dir: &str) -> String {
    let pkg = ["src/main/java/", "src/test/java/", "src/java/", "java/"]
        .iter()
        .find_map(|p| dir.strip_prefix(p))
        .unwrap_or(dir);
    pkg.replace('/', ".")
}

/// Whole-word substring search — `true` iff `name` occurs in `text` not
/// flanked by identifier characters. Mirrors `spec::contains_identifier`
/// (kept local so a pack doesn't depend on the spec renderer).
fn mentions_identifier(text: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let bytes = text.as_bytes();
    let mut start = 0;
    while let Some(pos) = text[start..].find(name) {
        let idx = start + pos;
        let before_ok = idx == 0 || !is_ident(bytes[idx - 1]);
        let after = idx + name.len();
        let after_ok = after >= bytes.len() || !is_ident(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
        start = idx + 1;
    }
    false
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

    // --- imports + resolution -------------------------------------------

    /// A fixture source tree, extracted + resolved end to end. `root` is
    /// only read for the same-package scan — pass a bogus path to suppress
    /// those edges, or a real dir whose files match `files` to exercise them.
    fn resolved(root: &Path, files: &[(&str, &str)]) -> (Graph, Vec<ResolvedImport>) {
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
        let edges = resolve_imports(root, &file_imports, &graph);
        (graph, edges)
    }

    #[test]
    fn parses_plain_static_and_star_imports() {
        let fi = extract_imports(
            "package p;\n\
             import java.util.List;\n\
             import static org.junit.Assert.assertTrue;\n\
             import com.example.util.*;\n\
             public class A {}\n",
            "src/main/java/p/A.java",
        );
        let got: Vec<(&str, &str)> = fi
            .imports
            .iter()
            .map(|i| (i.specifier.as_str(), i.imported_name.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("java.util", "List"),
                ("org.junit.Assert", "assertTrue"), // static: class stays in the specifier
                ("com.example.util", "*"),          // star: names nothing
            ]
        );
        assert!(fi.re_exports.is_empty() && fi.default_imports.is_empty());
    }

    #[test]
    fn fqn_resolves_to_the_top_level_type() {
        let (g, edges) = resolved(
            Path::new("/unused"),
            &[
                (
                    "src/main/java/com/ex/App.java",
                    "package com.ex;\nimport com.ex.util.Strings;\npublic class App {}\n",
                ),
                (
                    "src/main/java/com/ex/util/Strings.java",
                    "package com.ex.util;\npublic class Strings {}\n",
                ),
            ],
        );
        let e = edges
            .iter()
            .find(|e| e.imported_name == "Strings")
            .expect("the import edge exists");
        assert_eq!(
            g.string_id(e.target.expect("Strings should resolve")),
            "src/main/java/com/ex/util/Strings.java::Strings"
        );
    }

    #[test]
    fn static_import_resolves_to_the_member_then_falls_back_to_the_class() {
        let (g, edges) = resolved(
            Path::new("/unused"),
            &[
                (
                    "src/main/java/com/ex/Tests.java",
                    "package com.ex;\nimport static com.ex.Check.ok;\npublic class Tests {}\n",
                ),
                (
                    "src/main/java/com/ex/Check.java",
                    "package com.ex;\npublic class Check { public static void ok(boolean b) {} }\n",
                ),
            ],
        );
        let e = edges.iter().find(|e| e.imported_name == "ok").unwrap();
        assert_eq!(
            g.string_id(e.target.unwrap()),
            "src/main/java/com/ex/Check.java::Check::ok"
        );
    }

    #[test]
    fn same_package_sibling_referenced_in_source_resolves() {
        let dir = std::env::temp_dir().join(format!("codeowl-java-pkg-{}", std::process::id()));
        let pkg = dir.join("src/main/java/com/ex");
        std::fs::create_dir_all(&pkg).unwrap();
        let su = "package com.ex;\n\
                  public class StringUtils {\n\
                  static boolean blank(CharSequence cs) { return ObjectUtils.isNull(cs); }\n\
                  }\n";
        let ou = "package com.ex;\n\
                  public class ObjectUtils {\n\
                  static boolean isNull(Object o) { return o == null; }\n\
                  }\n";
        std::fs::write(pkg.join("StringUtils.java"), su).unwrap();
        std::fs::write(pkg.join("ObjectUtils.java"), ou).unwrap();

        let (_, edges) = resolved(
            &dir,
            &[
                ("src/main/java/com/ex/StringUtils.java", su),
                ("src/main/java/com/ex/ObjectUtils.java", ou),
            ],
        );

        // StringUtils names ObjectUtils with no import -> a synthetic edge.
        assert!(edges.iter().any(|e| {
            e.from_file == "src/main/java/com/ex/StringUtils.java"
                && e.imported_name == "ObjectUtils"
                && e.specifier == "com.ex"
                && e.target.is_some()
        }));
        // ObjectUtils never mentions StringUtils -> no reverse edge.
        assert!(
            !edges
                .iter()
                .any(|e| e.from_file.ends_with("ObjectUtils.java")
                    && e.imported_name == "StringUtils")
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn external_import_stays_unresolved() {
        let (_, edges) = resolved(
            Path::new("/unused"),
            &[(
                "src/main/java/com/ex/A.java",
                "package com.ex;\n\
                 import java.util.List;\n\
                 import com.google.common.base.Strings;\n\
                 public class A {}\n",
            )],
        );
        assert_eq!(edges.len(), 2);
        assert!(edges.iter().all(|e| e.target.is_none()));
    }

    #[test]
    fn resolve_on_a_small_commons_lang_slice() {
        // A lang3 shape: FQN imports across sub-packages, all internal.
        let files: &[(&str, &str)] = &[
            (
                "src/main/java/org/apache/commons/lang3/StringUtils.java",
                "package org.apache.commons.lang3;\n\
                 import org.apache.commons.lang3.math.NumberUtils;\n\
                 public class StringUtils {}\n",
            ),
            (
                "src/main/java/org/apache/commons/lang3/ObjectUtils.java",
                "package org.apache.commons.lang3;\n\
                 import org.apache.commons.lang3.StringUtils;\n\
                 public class ObjectUtils {}\n",
            ),
            (
                "src/main/java/org/apache/commons/lang3/math/NumberUtils.java",
                "package org.apache.commons.lang3.math;\n\
                 import org.apache.commons.lang3.Validate;\n\
                 public class NumberUtils {}\n",
            ),
            (
                "src/main/java/org/apache/commons/lang3/Validate.java",
                "package org.apache.commons.lang3;\npublic class Validate {}\n",
            ),
        ];
        let (_, edges) = resolved(Path::new("/unused"), files);
        let explicit: Vec<_> = edges
            .iter()
            .filter(|e| e.specifier.starts_with("org.apache"))
            .collect();
        assert_eq!(explicit.len(), 3, "three FQN imports in the slice");
        let hit = explicit.iter().filter(|e| e.target.is_some()).count();
        assert!(
            hit as f64 / explicit.len() as f64 >= 0.8,
            "{hit}/3 internal imports resolved"
        );
    }
}
