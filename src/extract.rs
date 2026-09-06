//! Walks a single TypeScript/TSX source file with tree-sitter and pulls out
//! its top-level `Symbol`s.
//!
//! Deliberately *not* a recursive walk of every node in the tree: we only
//! look at `program`'s direct children (unwrapping `export` statements) and,
//! for classes, one level into `class_body`. Nested closures, callbacks
//! passed to `useEffect`, etc. are implementation detail, not declarations
//! CodeOwl would ever generate a spec for — see `CLAUDE.md`'s note that
//! generation only recurses containment edges between *symbols*, not every
//! syntax node.

use tree_sitter::{Node, Parser};

use crate::hash::hash_text;
use crate::symbol::{ExtractedSymbol, SymbolKind};

/// Parse `source` (the contents of `rel_path`) and extract its symbols.
///
/// `rel_path`'s extension picks the grammar: `.tsx` gets JSX support,
/// everything else parses as plain TypeScript.
pub fn extract_file(source: &str, rel_path: &str) -> Vec<ExtractedSymbol> {
    let language = if rel_path.ends_with(".tsx") {
        tree_sitter_typescript::LANGUAGE_TSX
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT
    };

    let mut parser = Parser::new();
    parser
        .set_language(&language.into())
        .expect("bundled tree-sitter-typescript grammar should always load");

    // `tree` owns the arena the whole `Node<'_>` chain below borrows from.
    // We never let a `Node` outlive this function — every value we push
    // onto `out` is an owned `String`/`usize`, extracted while `tree` is
    // still alive. That's the "don't let Node<'a> escape the parse
    // function" rule from CLAUDE.md: it's not that Node is unsafe to use,
    // it's that storing one ties whatever holds it to this Tree's lifetime.
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        visit_top_level(child, source, rel_path, &mut out);
    }
    out
}

/// Handle one direct child of `program`: unwrap an `export` wrapper if
/// present, dispatch on the declaration kind, and push whatever symbols it
/// produces onto `out`.
fn visit_top_level(node: Node, source: &str, file: &str, out: &mut Vec<ExtractedSymbol>) {
    let decl = if node.kind() == "export_statement" {
        match node.child_by_field_name("declaration") {
            Some(d) => d,
            // `export { X } from './y'`, `export * from './y'`, or an
            // *anonymous* `export default <expr>` (an arrow function, an
            // identifier, a literal) — nothing new is *declared* here for
            // M1 to extract. Barrel files are exactly this case. A
            // *named* default export (`export default function Page()
            // {}`, `export default class Foo {}`) does have a
            // `declaration` field and falls through to the match below
            // like any other declaration — Next.js page/route components
            // routinely take this shape, so M5's feature layer relies on
            // it being extracted, not skipped.
            None => return,
        }
    } else {
        node
    };

    match decl.kind() {
        "function_declaration" => visit_function(decl, node, source, file, out),
        "class_declaration" => visit_class(decl, node, source, file, out),
        "lexical_declaration" => visit_lexical(decl, node, source, file, out),
        _ => {}
    }
}

fn visit_function(
    decl: Node,
    outer: Node,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    let Some(body) = decl.child_by_field_name("body") else {
        return;
    };
    let name = field_text(decl, "name", source).unwrap_or("<anonymous>");
    let signature = signature_text(decl, body, source);
    let is_exported = outer.kind() == "export_statement";
    out.push(ExtractedSymbol {
        id: format!("{file}::{name}"),
        kind: SymbolKind::Function,
        file: file.to_string(),
        lines: node_lines(decl),
        // `decl`, not `outer` — signature text skips `export`/`export
        // default` for consistency with the const/arrow-function case
        // below, where it's cheaper to just not include it. Whether a
        // symbol is exported is now its own `is_exported` field instead.
        source_hash: hash_text(text(decl, source)),
        interface_hash: is_exported.then(|| hash_text(&signature)),
        signature,
        docstring: leading_doc(outer, source),
        is_exported,
        parent: None,
        children: Vec::new(),
    });
}

fn visit_class(decl: Node, outer: Node, source: &str, file: &str, out: &mut Vec<ExtractedSymbol>) {
    let Some(body) = decl.child_by_field_name("body") else {
        return;
    };
    let name = field_text(decl, "name", source).unwrap_or("<anonymous>");
    let class_id = format!("{file}::{name}");

    let mut method_ids = Vec::new();
    let mut method_source_hashes = Vec::new();
    let mut method_symbols = Vec::new();
    let mut cursor = body.walk();
    for member in body.children(&mut cursor) {
        if member.kind() != "method_definition" {
            continue;
        }
        let Some(m_body) = member.child_by_field_name("body") else {
            continue;
        };
        let m_name = field_text(member, "name", source).unwrap_or("<anonymous>");
        let m_id = format!("{class_id}.{m_name}");
        let m_source_hash = hash_text(text(member, source));
        method_symbols.push(ExtractedSymbol {
            id: m_id.clone(),
            kind: SymbolKind::Method,
            file: file.to_string(),
            lines: node_lines(member),
            signature: signature_text(member, m_body, source),
            docstring: leading_doc(member, source),
            // A method isn't independently exported/imported — see the
            // doc comment on Symbol::is_exported.
            is_exported: false,
            source_hash: m_source_hash.clone(),
            interface_hash: None,
            parent: Some(class_id.clone()),
            children: Vec::new(),
        });
        method_ids.push(m_id);
        method_source_hashes.push(m_source_hash);
    }

    let signature = signature_text(decl, body, source);
    let is_exported = outer.kind() == "export_statement";

    // Merkle rollup: the class's own source_hash folds in each method's
    // source_hash, in declaration order (reordering methods is a real
    // change too). This is what makes the ancestor-chain hash-propagation
    // validation in ROADMAP.md's M2 entry hold: edit one method's body,
    // and both that method's and the class's source_hash move.
    //
    // interface_hash deliberately does NOT fold in method signatures —
    // M2 only resolves file-to-file import edges (a consumer imports the
    // class itself), not method-level call resolution, so nothing yet
    // watches a class's members for invalidation purposes. Revisit once a
    // later milestone adds call-level resolution.
    let mut rollup_input = signature.clone();
    for h in &method_source_hashes {
        rollup_input.push('\n');
        rollup_input.push_str(h);
    }

    out.push(ExtractedSymbol {
        id: class_id,
        kind: SymbolKind::Class,
        file: file.to_string(),
        lines: node_lines(decl),
        source_hash: hash_text(&rollup_input),
        interface_hash: is_exported.then(|| hash_text(&signature)),
        signature,
        docstring: leading_doc(outer, source),
        is_exported,
        parent: None,
        children: method_ids,
    });
    out.extend(method_symbols);
}

fn visit_lexical(
    decl: Node,
    outer: Node,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    // Only `const` is in scope — `let` (and `var`, a different node kind
    // entirely) aren't declarations CodeOwl generates specs for.
    if field_text(decl, "kind", source) != Some("const") {
        return;
    }

    let mut cursor = decl.walk();
    for declarator in decl.children(&mut cursor) {
        if declarator.kind() != "variable_declarator" {
            continue;
        }
        // `const { a, b } = ...` / `const [a, b] = ...` destructure into
        // patterns, not a single named symbol — skip rather than guess.
        let Some(name_node) = declarator
            .child_by_field_name("name")
            .filter(|n| n.kind() == "identifier")
        else {
            continue;
        };
        let name = text(name_node, source);
        let id = format!("{file}::{name}");
        let value = declarator.child_by_field_name("value");

        let (kind, signature) = match value {
            Some(v) if v.kind() == "arrow_function" || v.kind() == "function_expression" => {
                let body = v.child_by_field_name("body").unwrap_or(v);
                (
                    SymbolKind::Function,
                    format!("const {name} = {}", signature_text(v, body, source)),
                )
            }
            // A plain const's "shape" is its declared type, if any — e.g.
            // `const PI: number = 3.14` — not its literal value, which
            // interface_hash should ignore (below) exactly like a
            // function's body. Include the type annotation's own text
            // (which already carries a leading ": "), so a type change is
            // a real interface_hash change and a value-only edit isn't.
            _ => {
                let type_text = declarator
                    .child_by_field_name("type")
                    .map(|t| text(t, source))
                    .unwrap_or("");
                (SymbolKind::Const, format!("const {name}{type_text}"))
            }
        };

        let is_exported = outer.kind() == "export_statement";
        out.push(ExtractedSymbol {
            id,
            kind,
            file: file.to_string(),
            lines: node_lines(declarator),
            source_hash: hash_text(text(declarator, source)),
            interface_hash: is_exported.then(|| hash_text(&signature)),
            signature,
            docstring: leading_doc(outer, source),
            is_exported,
            parent: None,
            children: Vec::new(),
        });
    }
}

/// Slice `source` from `node`'s start up to (not including) `body`'s start,
/// trimmed. This is the "everything before the `{`" trick: it picks up
/// modifiers, name, type parameters, params, and return type without having
/// to name each of those fields individually — and it works identically for
/// `function_declaration`, `method_definition`, `class_declaration`, and
/// arrow functions (whose "body" is an expression when there are no braces).
fn signature_text(node: Node, body: Node, source: &str) -> String {
    let start = node.start_byte();
    let end = body.start_byte().max(start);
    source[start..end].trim_end().to_string()
}

/// Walk backward over `node`'s immediately preceding siblings, collecting a
/// contiguous run of `comment` nodes with no blank line between them (or
/// between the last comment and `node` itself). Returns `None` if the
/// nearest preceding sibling isn't a comment, or isn't adjacent.
///
/// The adjacency check matters: `prev_sibling()` finds the nearest sibling
/// regardless of blank lines in between, so without it a comment several
/// paragraphs above an unrelated declaration would get misattributed as
/// its docstring.
fn leading_doc(node: Node, source: &str) -> Option<String> {
    let mut comments = Vec::new();
    let mut expected_end_row = node.start_position().row;
    let mut cursor = node.prev_sibling();

    while let Some(n) = cursor {
        if n.kind() == "comment" && n.end_position().row + 1 == expected_end_row {
            expected_end_row = n.start_position().row;
            cursor = n.prev_sibling();
            comments.push(n);
        } else {
            break;
        }
    }

    if comments.is_empty() {
        return None;
    }
    comments.reverse();
    let lines: Vec<String> = comments
        .iter()
        .flat_map(|n| clean_comment(text(*n, source)))
        .collect();
    Some(lines.join("\n"))
}

/// Strip comment syntax (`//`, `/* */`, JSDoc's leading `*`) down to content
/// lines, dropping ones that are blank once stripped.
fn clean_comment(raw: &str) -> Vec<String> {
    let inner = raw
        .strip_prefix("/**")
        .or_else(|| raw.strip_prefix("/*"))
        .and_then(|s| s.strip_suffix("*/"))
        .or_else(|| raw.strip_prefix("//"));

    let Some(inner) = inner else {
        return Vec::new();
    };

    inner
        .lines()
        .map(|line| line.trim().trim_start_matches('*').trim())
        .filter(|line| !line.is_empty())
        .map(str::to_string)
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
    fn function_declaration_basic() {
        let src = "export function double(x: number): number {\n    return x * 2;\n}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        let s = &symbols[0];
        assert_eq!(s.id, "a.ts::double");
        assert_eq!(s.kind, SymbolKind::Function);
        assert_eq!(s.file, "a.ts");
        assert_eq!(s.lines, [1, 3]);
        assert_eq!(s.signature, "function double(x: number): number");
        assert_eq!(s.docstring, None);
        assert_eq!(s.parent, None);
        assert!(s.children.is_empty());
        assert!(s.is_exported);
        assert!(s.interface_hash.is_some());
        assert!(!s.source_hash.is_empty());
    }

    #[test]
    fn non_exported_declarations_have_no_interface_hash() {
        let src = "function helper(): void {}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        assert!(!symbols[0].is_exported);
        assert_eq!(symbols[0].interface_hash, None);
        // source_hash is still computed — non-exported code still needs a
        // staleness signal, it just can't be a reference-edge target.
        assert!(!symbols[0].source_hash.is_empty());
    }

    #[test]
    fn body_only_edit_changes_source_hash_but_not_interface_hash() {
        // This is the direct regression test for gap 2: rewriting a
        // function's implementation must never invalidate its importers.
        let before = extract_file(
            "export function add(a: number, b: number): number {\n    return a + b;\n}\n",
            "a.ts",
        );
        let after = extract_file(
            "export function add(a: number, b: number): number {\n    let sum = a + b;\n    return sum;\n}\n",
            "a.ts",
        );
        assert_ne!(before[0].source_hash, after[0].source_hash);
        assert_eq!(before[0].interface_hash, after[0].interface_hash);
    }

    #[test]
    fn signature_edit_changes_interface_hash() {
        let before = extract_file(
            "export function add(a: number, b: number): number {\n    return a + b;\n}\n",
            "a.ts",
        );
        let after = extract_file(
            "export function add(a: number, b: number, c: number): number {\n    return a + b;\n}\n",
            "a.ts",
        );
        assert_ne!(before[0].interface_hash, after[0].interface_hash);
        assert_ne!(before[0].source_hash, after[0].source_hash);
    }

    #[test]
    fn jsdoc_block_comment_is_docstring() {
        let src = "/**\n * Doubles a number.\n */\nexport function double(x: number): number {\n    return x * 2;\n}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].docstring.as_deref(), Some("Doubles a number."));
    }

    #[test]
    fn consecutive_line_comments_join_into_docstring() {
        let src = "// Adds two numbers.\n// Simple helper.\nexport function add(a: number, b: number): number {\n  return a + b;\n}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(
            symbols[0].docstring.as_deref(),
            Some("Adds two numbers.\nSimple helper.")
        );
    }

    #[test]
    fn comment_separated_by_blank_line_is_not_attached() {
        let src = "// unrelated\n\nexport function noop(): void {}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].docstring, None);
    }

    #[test]
    fn arrow_function_const_is_function_kind() {
        let src = "export const double = (x: number): number => x * 2;\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        let s = &symbols[0];
        assert_eq!(s.id, "a.ts::double");
        assert_eq!(s.kind, SymbolKind::Function);
        assert_eq!(s.signature, "const double = (x: number): number =>");
    }

    #[test]
    fn plain_const_is_const_kind() {
        let src = "export const PI = 3.14;\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        let s = &symbols[0];
        assert_eq!(s.id, "a.ts::PI");
        assert_eq!(s.kind, SymbolKind::Const);
        assert_eq!(s.signature, "const PI");
    }

    #[test]
    fn let_declaration_is_ignored() {
        let src = "export let counter = 0;\n";
        let symbols = extract_file(src, "a.ts");
        assert!(symbols.is_empty());
    }

    #[test]
    fn destructured_const_is_skipped() {
        let src = "export const { a, b } = getStuff();\n";
        let symbols = extract_file(src, "a.ts");
        assert!(symbols.is_empty());
    }

    #[test]
    fn barrel_file_yields_no_symbols() {
        let src = "export { Foo } from './foo';\nexport * from './bar';\n";
        let symbols = extract_file(src, "a.ts");
        assert!(symbols.is_empty());
    }

    #[test]
    fn class_with_methods_builds_containment_tree() {
        let src = "export class Foo<T> extends Bar implements Baz {\n    /** ctor doc */\n    constructor(private x: number) {}\n\n    // plain method\n    async doThing(y: T): Promise<void> {\n        console.log(y);\n    }\n}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 3);

        let class = &symbols[0];
        assert_eq!(class.id, "a.ts::Foo");
        assert_eq!(class.kind, SymbolKind::Class);
        assert_eq!(class.signature, "class Foo<T> extends Bar implements Baz");
        assert_eq!(class.parent, None);
        assert_eq!(
            class.children,
            vec!["a.ts::Foo.constructor", "a.ts::Foo.doThing"]
        );
        assert!(class.is_exported);
        assert!(class.interface_hash.is_some());

        let ctor = &symbols[1];
        assert_eq!(ctor.id, "a.ts::Foo.constructor");
        assert_eq!(ctor.kind, SymbolKind::Method);
        assert_eq!(ctor.parent.as_deref(), Some("a.ts::Foo"));
        assert_eq!(ctor.docstring.as_deref(), Some("ctor doc"));
        // Methods are never independently exported — see the doc comment
        // on Symbol::is_exported.
        assert!(!ctor.is_exported);
        assert_eq!(ctor.interface_hash, None);

        let method = &symbols[2];
        assert_eq!(method.id, "a.ts::Foo.doThing");
        assert_eq!(method.kind, SymbolKind::Method);
        assert_eq!(method.parent.as_deref(), Some("a.ts::Foo"));
        assert_eq!(method.docstring.as_deref(), Some("plain method"));
        assert!(
            method
                .signature
                .starts_with("async doThing(y: T): Promise<void>")
        );
    }

    #[test]
    fn method_body_edit_propagates_source_hash_to_class_but_not_interface_hash() {
        let before = extract_file(
            "export class Foo {\n    bar(): void {\n        console.log('a');\n    }\n}\n",
            "a.ts",
        );
        let after = extract_file(
            "export class Foo {\n    bar(): void {\n        console.log('b');\n    }\n}\n",
            "a.ts",
        );
        let (before_method, before_class) = (&before[1], &before[0]);
        let (after_method, after_class) = (&after[1], &after[0]);

        // The ancestor chain: a leaf method's source_hash changes...
        assert_ne!(before_method.source_hash, after_method.source_hash);
        // ...and that propagates up to the containing class's source_hash...
        assert_ne!(before_class.source_hash, after_class.source_hash);
        // ...but the class's *interface* didn't change (M2 doesn't yet
        // fold method signatures into a class's interface_hash — see the
        // doc comment on visit_class's interface_hash computation).
        assert_eq!(before_class.interface_hash, after_class.interface_hash);
    }

    #[test]
    fn tsx_file_parses_with_jsx_grammar() {
        let src = "export const Widget = ({ label }: { label: string }) => {\n    return <span>{label}</span>;\n};\n";
        let symbols = extract_file(src, "a.tsx");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Function);
    }
}
