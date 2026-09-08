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
//!
//! An **inherent `impl Foo` block is folded into `Foo`'s own symbol** after
//! the walk (M15 / `merge_inherent_impls`): Rust splits a type's data
//! (`struct Foo`) from its methods (`impl Foo`) as a matter of syntax, but a
//! spec reader wants one `Foo`, the same containment unit a TS/Java `class`
//! already is. Trait impls (`impl Trait for Foo`) stay their own symbols —
//! "how `Foo` satisfies `Trait`" is a distinct concern, and other languages
//! express it separately too (`class Foo implements Trait`).

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
    merge_inherent_impls(out)
}

/// The last `::`-separated segment of a stable id — `a.rs::Counter` ->
/// `Counter`, `a.rs::impl Counter::new` -> `new`. Only sound for the ids
/// `merge_inherent_impls` actually inspects (top-level types and the
/// methods of an *inherent* impl); a trait-impl id like
/// `a.rs::impl Debug for S` is never passed here.
fn short_id(id: &str) -> &str {
    id.rsplit("::").next().unwrap_or(id)
}

/// Fold every inherent `impl Foo { … }` block into the `Foo` symbol this
/// file defines: reparent its methods onto `Foo` (rewriting their ids from
/// `<file>::impl Foo::m` to `<file>::Foo::m`), Merkle-fold the block's
/// `source_hash` into `Foo`'s, union the block's attribute `markers`, then
/// drop the block. A type's several inherent impls all collapse into it, in
/// source order. Trait impls (`impl Trait for Foo` — the ` for ` gives it
/// away) and impls on a type this file doesn't define are left untouched.
fn merge_inherent_impls(syms: Vec<ExtractedSymbol>) -> Vec<ExtractedSymbol> {
    use std::collections::HashMap;

    // Short name -> index of a top-level type this file defines.
    let type_ix: HashMap<&str, usize> = syms
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.parent.is_none() && matches!(s.raw.as_str(), "struct" | "enum" | "union")
        })
        .map(|(i, s)| (short_id(&s.id), i))
        .collect();

    // Inherent-impl block index -> the index of the type it merges into.
    let merge_into: HashMap<usize, usize> = syms
        .iter()
        .enumerate()
        .filter(|(_, s)| s.parent.is_none() && s.raw == "impl")
        .filter_map(|(i, s)| {
            let target = inherent_impl_target(&s.signature)?;
            Some((i, *type_ix.get(target.as_str())?))
        })
        .collect();

    if merge_into.is_empty() {
        return syms;
    }

    // Per target type: what its inherent impl blocks contribute — the
    // reparented method symbols, the block `source_hash`es to Merkle-fold
    // in, and the attribute markers to union. Accumulated in source order
    // (we sweep `syms` front to back).
    #[derive(Default)]
    struct Folded {
        methods: Vec<ExtractedSymbol>,
        block_hashes: Vec<String>,
        markers: Vec<String>,
    }
    let mut folded: HashMap<usize, Folded> = HashMap::new();
    let mut dropped = vec![false; syms.len()];

    for i in 0..syms.len() {
        let Some(&ti) = merge_into.get(&i) else {
            continue;
        };
        dropped[i] = true;
        let block_id = syms[i].id.clone();
        let type_id = syms[ti].id.clone();
        let block_hash = syms[i].source_hash.clone();
        let block_markers = syms[i].markers.clone();

        // This block's methods are the contiguous run right after it whose
        // `parent` is the block (`visit_container` inserts the container,
        // then its members follow directly).
        let mut methods = Vec::new();
        for (j, m) in syms.iter().enumerate().skip(i + 1) {
            if m.parent.as_deref() != Some(block_id.as_str()) {
                break;
            }
            dropped[j] = true;
            let mut method = m.clone();
            method.id = format!("{type_id}::{}", short_id(&m.id));
            method.parent = Some(type_id.clone());
            methods.push(method);
        }

        let entry = folded.entry(ti).or_default();
        entry.methods.extend(methods);
        entry.block_hashes.push(block_hash);
        entry.markers.extend(block_markers);
    }

    let mut out = Vec::with_capacity(syms.len());
    for (i, s) in syms.into_iter().enumerate() {
        if dropped[i] {
            continue;
        }
        let Some(f) = folded.remove(&i) else {
            out.push(s);
            continue;
        };
        let mut ty = s;
        let mut rollup = ty.source_hash.clone();
        for h in &f.block_hashes {
            rollup.push('\n');
            rollup.push_str(h);
        }
        ty.source_hash = hash_text(&rollup);
        for marker in f.markers {
            if !ty.markers.contains(&marker) {
                ty.markers.push(marker);
            }
        }
        ty.children.extend(f.methods.iter().map(|m| m.id.clone()));
        out.push(ty);
        out.extend(f.methods);
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

/// The bare type name an *inherent* `impl` header is for, or `None` for a
/// trait impl or an unparseable header. `impl Graph` -> `Graph`,
/// `impl<T> Wrapper<T>` -> `Wrapper`, `impl ResolveCtx<'_>` -> `ResolveCtx`,
/// `impl foo::Bar` -> `Bar`. A ` for ` before any `where` clause means a
/// trait impl (`impl Debug for S`) — left for its own spec section.
fn inherent_impl_target(header: &str) -> Option<String> {
    let header: String = header.split_whitespace().collect::<Vec<_>>().join(" ");
    let rest = header.strip_prefix("impl")?;
    // Guard against an identifier that merely starts with "impl".
    if !rest.is_empty() && !rest.starts_with(|c: char| c.is_whitespace() || c == '<') {
        return None;
    }
    let rest = rest.trim_start();
    let before_where = rest.split(" where ").next().unwrap_or(rest);
    if before_where.split(' ').any(|w| w == "for") {
        return None;
    }
    let after_generics = strip_leading_generics(before_where);
    let type_path = after_generics
        .split(|c: char| c.is_whitespace() || c == '<')
        .next()
        .unwrap_or("");
    let name = type_path.rsplit("::").next().unwrap_or(type_path);
    (!name.is_empty()).then(|| name.to_string())
}

/// Drop a leading `<…>` generic-parameter list (angle-bracket balanced, so
/// `<T: Iterator<Item = u8>>` is handled), returning the rest trimmed. `s`
/// with no leading `<` is returned unchanged.
fn strip_leading_generics(s: &str) -> &str {
    if !s.starts_with('<') {
        return s;
    }
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return s[i + 1..].trim_start();
                }
            }
            _ => {}
        }
    }
    s
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

// ---------------------------------------------------------------------------
// Imports — `use` statements and their resolution against the module tree.
// The Rust pack's counterpart to `imports.rs` + `resolve.rs`. Reference
// edges only: a `use` names an *item* (type / fn / trait / const / module),
// never a method, so every target is a top-level symbol like the extractor
// produces. `mod foo;` is structural, not a symbol reference — skipped.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::path::Path;

use crate::graph::{Graph, SymbolId};
use crate::imports::{FileImports, ImportRef, ReExport};
use crate::resolve::ResolvedImport;

/// Parse a `.rs` file's `use` declarations. A `pub use` is a re-export
/// (the barrel pattern — `lib.rs` is all of these); a plain `use` is an
/// import. Glob (`use x::*`), `use x::{self, ..}`'s `self`, and nested
/// lists (`use a::{b::C}`) are skipped — a glob names no single item, and
/// the others are rare enough to defer.
pub fn extract_imports(source: &str, _rel_path: &str) -> FileImports {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("bundled tree-sitter-rust grammar should always load");
    let Some(tree) = parser.parse(source, None) else {
        return FileImports::default();
    };

    let mut fi = FileImports::default();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        if node.kind() != "use_declaration" {
            continue;
        }
        let mut dc = node.walk();
        let kids: Vec<Node> = node.children(&mut dc).collect();
        let is_reexport = kids.iter().any(|c| c.kind() == "visibility_modifier");
        let Some(arg) = kids
            .iter()
            .find(|c| !matches!(c.kind(), "use" | "visibility_modifier" | ";"))
        else {
            continue;
        };
        let arg = *arg;
        for (specifier, name) in use_targets(arg, source) {
            if is_reexport {
                fi.re_exports.push(ReExport {
                    exported_as: name.clone(),
                    specifier,
                    source_name: name,
                });
            } else {
                fi.imports.push(ImportRef {
                    specifier,
                    imported_name: name,
                });
            }
        }
    }
    fi
}

/// `(module-path, item-name)` pairs for one `use` argument node.
fn use_targets(arg: Node, source: &str) -> Vec<(String, String)> {
    match arg.kind() {
        // `use a::b::Item;`
        "scoped_identifier" => split_last(text(arg, source)).into_iter().collect(),
        // `use a::b::Item as Alias;` — the alias is a local rename, track
        // the source name like `imports.rs` does for TS.
        "use_as_clause" => arg
            .child_by_field_name("path")
            .or_else(|| arg.named_child(0))
            .and_then(|p| split_last(text(p, source)))
            .into_iter()
            .collect(),
        // `use a::b::{X, Y, Z};`
        "scoped_use_list" => {
            let mut cursor = arg.walk();
            let kids: Vec<Node> = arg.children(&mut cursor).collect();
            let prefix = kids
                .first()
                .map(|n| text(*n, source).to_string())
                .unwrap_or_default();
            let Some(list) = kids.iter().find(|c| c.kind() == "use_list") else {
                return Vec::new();
            };
            let mut lc = list.walk();
            list.children(&mut lc)
                .filter(|c| c.kind() == "identifier")
                .map(|c| (prefix.clone(), text(c, source).to_string()))
                .collect()
        }
        _ => Vec::new(), // use_wildcard, bare identifier, etc.
    }
}

/// `"crate::graph::Graph"` -> `("crate::graph", "Graph")`. `None` for a
/// single-segment path (`use foo;` — a whole-module import, no item).
fn split_last(path: &str) -> Option<(String, String)> {
    path.rsplit_once("::")
        .map(|(pre, name)| (pre.to_string(), name.to_string()))
}

/// Resolve every file's `use` imports (and `pub use` re-exports) against
/// the module tree. A module path maps to a file by Rust's filesystem
/// convention — `crate::a::b` -> `<src>/a/b.rs` or `<src>/a/b/mod.rs` —
/// not by reading `mod` declarations; then the item is looked up as a
/// top-level symbol in that file, following one hop of `pub use`.
pub fn resolve_imports(
    root: &Path,
    file_imports: &HashMap<String, FileImports>,
    graph: &Graph,
) -> Vec<ResolvedImport> {
    let src = crate_src_dir(file_imports);
    let mut sorted: Vec<_> = file_imports.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    let mut out = Vec::new();
    for (from_file, fi) in sorted {
        let each = fi
            .imports
            .iter()
            .map(|i| (&i.specifier, &i.imported_name))
            .chain(fi.re_exports.iter().map(|r| (&r.specifier, &r.source_name)));
        for (specifier, name) in each {
            out.push(ResolvedImport {
                from_file: from_file.clone(),
                specifier: specifier.clone(),
                imported_name: name.clone(),
                target: resolve_one(&src, from_file, specifier, name, file_imports, graph),
            });
        }
    }
    let _ = root; // resolution is off the walked file set, not the FS
    out
}

fn resolve_one(
    src: &str,
    from_file: &str,
    specifier: &str,
    name: &str,
    file_imports: &HashMap<String, FileImports>,
    graph: &Graph,
) -> Option<SymbolId> {
    let target_file = module_path_to_file(src, from_file, specifier, file_imports)?;
    if let Some(id) = graph.find(&format!("{target_file}::{name}")) {
        return Some(id);
    }
    // One hop through a `pub use` in the target module.
    let rx = file_imports
        .get(&target_file)?
        .re_exports
        .iter()
        .find(|r| r.exported_as == name)?;
    let hop = module_path_to_file(src, &target_file, &rx.specifier, file_imports)?;
    graph.find(&format!("{hop}::{}", rx.source_name))
}

/// The directory the crate root lives in — `"src"` for the usual layout,
/// `""` for a crate rooted at the repo root. Taken from the walked file
/// set so it needs no filesystem access.
fn crate_src_dir(file_imports: &HashMap<String, FileImports>) -> String {
    for f in file_imports.keys() {
        if let Some(dir) = f
            .strip_suffix("/lib.rs")
            .or_else(|| f.strip_suffix("/main.rs"))
        {
            return dir.to_string();
        }
        if f == "lib.rs" || f == "main.rs" {
            return String::new();
        }
    }
    "src".to_string()
}

/// The module a file *is* — `src/graph.rs` is module `crate::graph`, and a
/// `self::x` from it means `src/graph/x.rs`. `mod.rs` / crate roots are
/// the module named by their directory.
fn module_dir_of(file: &str) -> String {
    if file.ends_with("/mod.rs")
        || file.ends_with("/lib.rs")
        || file.ends_with("/main.rs")
        || file == "lib.rs"
        || file == "main.rs"
    {
        return file
            .rsplit_once('/')
            .map_or(String::new(), |(d, _)| d.to_string());
    }
    file.strip_suffix(".rs").unwrap_or(file).to_string()
}

/// A `crate::` / `self::` / `super::` module path -> the repo-relative
/// file that module's items live in, or `None` for an external crate or an
/// unresolvable path.
fn module_path_to_file(
    src: &str,
    from_file: &str,
    specifier: &str,
    file_imports: &HashMap<String, FileImports>,
) -> Option<String> {
    let segs: Vec<&str> = specifier.split("::").collect();
    let exists = |p: &str| file_imports.contains_key(p);

    let (base, rest): (String, &[&str]) = match *segs.first()? {
        "crate" => (src.to_string(), &segs[1..]),
        "self" => (module_dir_of(from_file), &segs[1..]),
        "super" => {
            let ups = segs.iter().take_while(|s| **s == "super").count();
            let mut dir = module_dir_of(from_file);
            for _ in 0..ups {
                dir = dir
                    .rsplit_once('/')
                    .map_or(String::new(), |(d, _)| d.to_string());
            }
            (dir, &segs[ups..])
        }
        // An external crate (`anyhow`, `std`, `tree_sitter`) — or, in 2018
        // style, a bare top-level module. Try the latter under `src`; if
        // no such file was walked, it's external -> unresolved.
        _ => (src.to_string(), &segs[..]),
    };

    let joined = if rest.is_empty() {
        base.clone()
    } else {
        let mut p = base.clone();
        for s in rest {
            if !p.is_empty() {
                p.push('/');
            }
            p.push_str(s);
        }
        p
    };

    let prefixed = |name: &str| {
        if joined.is_empty() {
            name.to_string()
        } else {
            format!("{joined}/{name}")
        }
    };
    [
        format!("{joined}.rs"),
        prefixed("mod.rs"),
        prefixed("lib.rs"),
        prefixed("main.rs"),
    ]
    .into_iter()
    .find(|cand| exists(cand))
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
    fn inherent_impl_folds_into_its_type() {
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
                "src/counter.rs::Counter::new",
                "src/counter.rs::Counter::bump",
            ],
            "the `impl Counter` block should be gone, its methods reparented onto Counter"
        );

        let the_struct = &syms[0];
        assert_eq!(the_struct.kind, SymbolKind::Container);
        assert_eq!(the_struct.raw, "struct");
        assert_eq!(
            the_struct.children,
            vec![
                "src/counter.rs::Counter::new",
                "src/counter.rs::Counter::bump"
            ]
        );

        let new = &syms[1];
        assert_eq!(new.raw, "method");
        assert_eq!(new.parent.as_deref(), Some("src/counter.rs::Counter"));
        assert_eq!(new.docstring.as_deref(), Some("Start at zero."));
        assert!(!new.is_exported);
    }

    #[test]
    fn several_inherent_impls_all_fold_into_the_one_type() {
        let src = "\
struct S;\n\
impl S {\n    fn a(&self) {}\n}\n\
impl S {\n    fn b(&self) {}\n}\n";
        let syms = extract_file(src, "a.rs");
        let s = syms.iter().find(|s| s.id == "a.rs::S").unwrap();
        assert_eq!(s.children, vec!["a.rs::S::a", "a.rs::S::b"]);
        assert!(
            !syms.iter().any(|s| s.raw == "impl"),
            "no impl-block symbol should survive"
        );
    }

    #[test]
    fn folded_type_source_hash_moves_with_a_method_body_edit() {
        let a = extract_file(
            "struct S;\nimpl S {\n    fn f(&self) { let x = 1; }\n}\n",
            "a.rs",
        );
        let b = extract_file(
            "struct S;\nimpl S {\n    fn f(&self) { let x = 2; }\n}\n",
            "a.rs",
        );
        let s_a = a.iter().find(|s| s.id == "a.rs::S").unwrap();
        let s_b = b.iter().find(|s| s.id == "a.rs::S").unwrap();
        assert_ne!(s_a.source_hash, s_b.source_hash);
    }

    #[test]
    fn trait_impl_is_left_as_its_own_symbol() {
        let src = "\
struct S;\n\
impl S {\n    fn own(&self) {}\n}\n\
impl std::fmt::Debug for S {\n    fn fmt(&self) {}\n}\n";
        let syms = extract_file(src, "a.rs");
        // The inherent `impl S` folded in; the trait impl did not.
        assert_eq!(
            syms.iter().find(|s| s.id == "a.rs::S").unwrap().children,
            vec!["a.rs::S::own"]
        );
        assert!(
            syms.iter()
                .any(|s| s.id == "a.rs::impl std::fmt::Debug for S" && s.raw == "impl")
        );
    }

    #[test]
    fn inherent_impl_target_parses_headers() {
        assert_eq!(inherent_impl_target("impl Graph").as_deref(), Some("Graph"));
        assert_eq!(
            inherent_impl_target("impl ResolveCtx<'_>").as_deref(),
            Some("ResolveCtx")
        );
        assert_eq!(
            inherent_impl_target("impl<T> Wrapper<T>").as_deref(),
            Some("Wrapper")
        );
        assert_eq!(
            inherent_impl_target("impl<T: Iterator<Item = u8>> Foo<T>").as_deref(),
            Some("Foo")
        );
        assert_eq!(inherent_impl_target("impl std::fmt::Debug for S"), None);
        assert_eq!(inherent_impl_target("impl<T> From<T> for S"), None);
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
            !syms.iter().any(|s| s.id == "src/graph.rs::impl Graph"),
            "the inherent `impl Graph` block should have folded into Graph"
        );
        assert!(
            syms.iter()
                .any(|s| s.raw == "method" && s.id == "src/graph.rs::Graph::build"),
            "missing Graph::build, reparented onto Graph"
        );
        // Every symbol carries a concrete keyword.
        assert!(syms.iter().all(|s| !s.raw.is_empty()));
    }

    // --- imports ---

    #[test]
    fn parses_plain_grouped_aliased_and_pub_use() {
        let src = "\
use crate::graph::Graph;\n\
use crate::graph::{Node, SymbolId};\n\
use crate::hash::hash_text as h;\n\
use anyhow::Result;\n\
use crate::features::*;\n\
pub use crate::stack::RustStack;\n";
        let fi = extract_imports(src, "src/x.rs");

        let names: Vec<(&str, &str)> = fi
            .imports
            .iter()
            .map(|i| (i.specifier.as_str(), i.imported_name.as_str()))
            .collect();
        assert_eq!(
            names,
            vec![
                ("crate::graph", "Graph"),
                ("crate::graph", "Node"),
                ("crate::graph", "SymbolId"),
                ("crate::hash", "hash_text"), // alias dropped, source name kept
                ("anyhow", "Result"),
                // the glob `use crate::features::*` is not tracked
            ]
        );
        assert_eq!(fi.re_exports.len(), 1);
        assert_eq!(fi.re_exports[0].source_name, "RustStack");
        assert_eq!(fi.re_exports[0].specifier, "crate::stack");
    }

    /// A fixture module tree, resolved end to end.
    fn resolved(files: &[(&str, &str)]) -> Vec<ResolvedImport> {
        let extractions: Vec<crate::graph::FileExtraction> = files
            .iter()
            .map(|(p, src)| crate::graph::FileExtraction {
                rel_path: (*p).to_string(),
                source_hash: crate::hash::hash_text(src),
                symbols: extract_file(src, p),
            })
            .collect();
        let graph = Graph::build(extractions);
        let file_imports: HashMap<String, FileImports> = files
            .iter()
            .map(|(p, src)| ((*p).to_string(), extract_imports(src, p)))
            .collect();
        resolve_imports(Path::new("/unused"), &file_imports, &graph)
    }

    #[test]
    fn crate_relative_use_resolves_to_a_top_level_item() {
        let edges = resolved(&[
            ("src/lib.rs", "pub mod graph;\npub mod app;\n"),
            ("src/graph.rs", "pub struct Graph;\n"),
            (
                "src/app.rs",
                "use crate::graph::Graph;\npub fn run(_g: Graph) {}\n",
            ),
        ]);
        let e = edges
            .iter()
            .find(|e| e.from_file == "src/app.rs" && e.imported_name == "Graph")
            .unwrap();
        assert!(e.target.is_some(), "crate::graph::Graph should resolve");
    }

    #[test]
    fn external_crate_and_missing_item_stay_unresolved() {
        let edges = resolved(&[(
            "src/lib.rs",
            "use anyhow::Result;\nuse crate::nope::Gone;\npub fn f() {}\n",
        )]);
        assert!(edges.iter().all(|e| e.target.is_none()));
    }

    #[test]
    fn super_and_pub_use_hop_resolve() {
        let edges = resolved(&[
            (
                "src/lib.rs",
                "pub mod inner;\npub struct Root;\npub use inner::Leaf;\n",
            ),
            (
                "src/inner.rs",
                "use super::Root;\npub struct Leaf;\npub fn g(_r: Root) {}\n",
            ),
            ("src/other.rs", "use crate::Leaf;\npub fn h(_l: Leaf) {}\n"),
        ]);
        // `super::Root` from src/inner.rs -> src/lib.rs::Root
        assert!(edges.iter().any(|e| e.from_file == "src/inner.rs"
            && e.imported_name == "Root"
            && e.target.is_some()));
        // `use crate::Leaf` -> follows lib.rs's `pub use inner::Leaf`
        assert!(edges.iter().any(|e| e.from_file == "src/other.rs"
            && e.imported_name == "Leaf"
            && e.target.is_some()));
    }

    #[test]
    fn resolve_imports_on_this_repo_produces_real_edges() {
        // The M14 dogfood: parse + resolve every src/*.rs, and expect the
        // internal `use crate::…` graph to be substantially resolved.
        let files: Vec<(String, String)> = std::fs::read_dir("src")
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("rs"))
            .map(|p| {
                let rel = format!("src/{}", p.file_name().unwrap().to_str().unwrap());
                (rel, std::fs::read_to_string(&p).unwrap())
            })
            .collect();
        let refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(p, s)| (p.as_str(), s.as_str()))
            .collect();
        let edges = resolved(&refs);

        let crate_edges: Vec<_> = edges
            .iter()
            .filter(|e| e.specifier.starts_with("crate::"))
            .collect();
        let hit = crate_edges.iter().filter(|e| e.target.is_some()).count();
        assert!(
            crate_edges.len() >= 30,
            "expected a lot of crate:: imports, got {}",
            crate_edges.len()
        );
        assert!(
            hit * 100 / crate_edges.len() >= 80,
            "expected >=80% of crate:: imports resolved, got {hit}/{}",
            crate_edges.len()
        );
        // A specific one we know exists.
        assert!(edges.iter().any(|e| e.imported_name == "Graph"
            && e.specifier == "crate::graph"
            && e.target.is_some()));
    }
}
