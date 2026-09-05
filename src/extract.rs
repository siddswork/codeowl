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

use crate::symbol::{Symbol, SymbolKind};

/// Parse `source` (the contents of `rel_path`) and extract its symbols.
///
/// `rel_path`'s extension picks the grammar: `.tsx` gets JSX support,
/// everything else parses as plain TypeScript.
pub fn extract_file(source: &str, rel_path: &str) -> Vec<Symbol> {
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
fn visit_top_level(node: Node, source: &str, file: &str, out: &mut Vec<Symbol>) {
    let decl = if node.kind() == "export_statement" {
        match node.child_by_field_name("declaration") {
            Some(d) => d,
            // `export { X } from './y'`, `export * from './y'`, or
            // `export default <expr>` — nothing new is *declared* here for
            // M1 to extract. Barrel files are exactly this case.
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

fn visit_function(decl: Node, outer: Node, source: &str, file: &str, out: &mut Vec<Symbol>) {
    let Some(body) = decl.child_by_field_name("body") else {
        return;
    };
    let name = field_text(decl, "name", source).unwrap_or("<anonymous>");
    out.push(Symbol {
        id: format!("{file}::{name}"),
        kind: SymbolKind::Function,
        file: file.to_string(),
        lines: node_lines(decl),
        // `decl`, not `outer` — signature text skips `export`/`export
        // default` for consistency with the const/arrow-function case
        // below, where it's cheaper to just not include it. Whether a
        // symbol is exported becomes its own `isExported` field in M2.
        signature: signature_text(decl, body, source),
        docstring: leading_doc(outer, source),
        parent: None,
        children: Vec::new(),
    });
}

fn visit_class(decl: Node, outer: Node, source: &str, file: &str, out: &mut Vec<Symbol>) {
    let Some(body) = decl.child_by_field_name("body") else {
        return;
    };
    let name = field_text(decl, "name", source).unwrap_or("<anonymous>");
    let class_id = format!("{file}::{name}");

    let mut method_ids = Vec::new();
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
        method_symbols.push(Symbol {
            id: m_id.clone(),
            kind: SymbolKind::Method,
            file: file.to_string(),
            lines: node_lines(member),
            signature: signature_text(member, m_body, source),
            docstring: leading_doc(member, source),
            parent: Some(class_id.clone()),
            children: Vec::new(),
        });
        method_ids.push(m_id);
    }

    out.push(Symbol {
        id: class_id,
        kind: SymbolKind::Class,
        file: file.to_string(),
        lines: node_lines(decl),
        signature: signature_text(decl, body, source),
        docstring: leading_doc(outer, source),
        parent: None,
        children: method_ids,
    });
    out.extend(method_symbols);
}

fn visit_lexical(decl: Node, outer: Node, source: &str, file: &str, out: &mut Vec<Symbol>) {
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
            _ => (SymbolKind::Const, format!("const {name}")),
        };

        out.push(Symbol {
            id,
            kind,
            file: file.to_string(),
            lines: node_lines(declarator),
            signature,
            docstring: leading_doc(outer, source),
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

        let ctor = &symbols[1];
        assert_eq!(ctor.id, "a.ts::Foo.constructor");
        assert_eq!(ctor.kind, SymbolKind::Method);
        assert_eq!(ctor.parent.as_deref(), Some("a.ts::Foo"));
        assert_eq!(ctor.docstring.as_deref(), Some("ctor doc"));

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
    fn tsx_file_parses_with_jsx_grammar() {
        let src = "export const Widget = ({ label }: { label: string }) => {\n    return <span>{label}</span>;\n};\n";
        let symbols = extract_file(src, "a.tsx");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Function);
    }
}
