//! Parses a single file's `import`/re-export statements — the input to
//! M2's file-to-file reference-edge resolution. Runs as its own tree-sitter
//! pass over the same source `extract_file` parses separately: two parses
//! of a small file is negligible cost at laptop-repo scale, and keeping
//! this independent of `extract.rs` keeps each pass single-purpose.
//!
//! Scope is deliberately narrow: only NAMED imports/re-exports are
//! resolved. `import Foo from './x'` (default) and `import * as ns from
//! './x'` (namespace) don't target one named declaration the way a named
//! import does, and `export * from './x'` (wildcard re-export) would need
//! enumerating the source file's whole export list to chase through — all
//! three are left untracked here rather than guessed at. See `CLAUDE.md`'s
//! pending decisions.

use tree_sitter::{Node, Parser};

/// One named import: `import { <imported_name> } from '<specifier>'`. A
/// local alias (`import { Foo as Bar }`), if any, is purely a local rename
/// — irrelevant to resolving what it points at, so it isn't tracked here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRef {
    pub specifier: String,
    pub imported_name: String,
}

/// One named re-export: `export { <source_name> as <exported_as> } from
/// '<specifier>'` — how a barrel forwards a name it doesn't declare itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReExport {
    pub exported_as: String,
    pub specifier: String,
    pub source_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileImports {
    pub imports: Vec<ImportRef>,
    pub re_exports: Vec<ReExport>,
}

/// Parse `source` (the contents of `rel_path`) and extract its named
/// imports and named re-exports.
pub fn extract_imports(source: &str, rel_path: &str) -> FileImports {
    let language = if rel_path.ends_with(".tsx") {
        tree_sitter_typescript::LANGUAGE_TSX
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT
    };

    let mut parser = Parser::new();
    parser
        .set_language(&language.into())
        .expect("bundled tree-sitter-typescript grammar should always load");

    let Some(tree) = parser.parse(source, None) else {
        return FileImports::default();
    };

    let mut out = FileImports::default();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "import_statement" => visit_import(child, source, &mut out),
            "export_statement" => visit_maybe_reexport(child, source, &mut out),
            _ => {}
        }
    }
    out
}

fn visit_import(node: Node, source: &str, out: &mut FileImports) {
    let Some(specifier) = string_field(node, "source", source) else {
        return;
    };
    let Some(clause) = child_of_kind(node, "import_clause") else {
        return; // side-effect-only import: `import './polyfill'`
    };
    let Some(named) = child_of_kind(clause, "named_imports") else {
        return; // default/namespace import — not resolved, see module docs
    };

    let mut cursor = named.walk();
    for spec in named.children(&mut cursor) {
        if spec.kind() != "import_specifier" {
            continue;
        }
        if let Some(name) = field_text(spec, "name", source) {
            out.imports.push(ImportRef {
                specifier: specifier.clone(),
                imported_name: name.to_string(),
            });
        }
    }
}

fn visit_maybe_reexport(node: Node, source: &str, out: &mut FileImports) {
    let Some(specifier) = string_field(node, "source", source) else {
        return; // a normal `export function/class/const`, not a re-export
    };
    let Some(clause) = child_of_kind(node, "export_clause") else {
        return; // `export * from '...'` — wildcard, not chased (see module docs)
    };

    let mut cursor = clause.walk();
    for spec in clause.children(&mut cursor) {
        if spec.kind() != "export_specifier" {
            continue;
        }
        let Some(source_name) = field_text(spec, "name", source) else {
            continue;
        };
        let exported_as = field_text(spec, "alias", source).unwrap_or(source_name);
        out.re_exports.push(ReExport {
            exported_as: exported_as.to_string(),
            specifier: specifier.clone(),
            source_name: source_name.to_string(),
        });
    }
}

fn child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).find(|c| c.kind() == kind)
}

fn field_text<'a>(node: Node, field: &str, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name(field)
        .map(|n| n.utf8_text(source.as_bytes()).unwrap_or_default())
}

/// A `source`/`name`/`alias` field that's a `string` node has its text
/// include the surrounding quotes — strip them.
fn string_field(node: Node, field: &str, source: &str) -> Option<String> {
    let text = field_text(node, field, source)?;
    Some(text.trim_matches(['"', '\'']).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_import_captures_specifier_and_name() {
        let out = extract_imports("import { Foo } from './foo';\n", "a.ts");
        assert_eq!(
            out.imports,
            vec![ImportRef {
                specifier: "./foo".into(),
                imported_name: "Foo".into(),
            }]
        );
    }

    #[test]
    fn aliased_import_tracks_the_source_name_not_the_local_alias() {
        let out = extract_imports("import { Foo as Bar } from './foo';\n", "a.ts");
        assert_eq!(out.imports[0].imported_name, "Foo");
    }

    #[test]
    fn multiple_named_imports_in_one_statement() {
        let out = extract_imports("import { A, B } from './x';\n", "a.ts");
        assert_eq!(out.imports.len(), 2);
        assert_eq!(out.imports[0].imported_name, "A");
        assert_eq!(out.imports[1].imported_name, "B");
    }

    #[test]
    fn default_import_is_not_tracked() {
        let out = extract_imports("import Foo from './foo';\n", "a.ts");
        assert!(out.imports.is_empty());
    }

    #[test]
    fn namespace_import_is_not_tracked() {
        let out = extract_imports("import * as ns from './foo';\n", "a.ts");
        assert!(out.imports.is_empty());
    }

    #[test]
    fn side_effect_only_import_is_not_tracked() {
        let out = extract_imports("import './polyfill';\n", "a.ts");
        assert!(out.imports.is_empty());
    }

    #[test]
    fn default_plus_named_import_tracks_only_the_named_part() {
        let out = extract_imports("import Foo, { Bar } from './x';\n", "a.ts");
        assert_eq!(out.imports.len(), 1);
        assert_eq!(out.imports[0].imported_name, "Bar");
    }

    #[test]
    fn named_reexport_captures_source_and_exported_name() {
        let out = extract_imports("export { Foo } from './foo';\n", "a.ts");
        assert_eq!(
            out.re_exports,
            vec![ReExport {
                exported_as: "Foo".into(),
                specifier: "./foo".into(),
                source_name: "Foo".into(),
            }]
        );
    }

    #[test]
    fn aliased_reexport_tracks_both_names() {
        let out = extract_imports("export { Foo as Bar } from './foo';\n", "a.ts");
        assert_eq!(out.re_exports[0].exported_as, "Bar");
        assert_eq!(out.re_exports[0].source_name, "Foo");
    }

    #[test]
    fn wildcard_reexport_is_not_tracked() {
        let out = extract_imports("export * from './foo';\n", "a.ts");
        assert!(out.re_exports.is_empty());
    }

    #[test]
    fn plain_export_is_not_a_reexport() {
        let out = extract_imports("export function foo() {}\n", "a.ts");
        assert!(out.imports.is_empty());
        assert!(out.re_exports.is_empty());
    }
}
