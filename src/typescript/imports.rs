//! Parses a single TypeScript file's `import`/re-export statements — the
//! input to file-to-file reference-edge resolution. Runs as its own
//! tree-sitter pass over the same source the extractor in `mod.rs` parses
//! separately: two parses of a small file is negligible cost at laptop-repo
//! scale, and keeping this independent keeps each pass single-purpose. The
//! record types it returns (`FileImports` and friends) are shared by every
//! language pack, so they live in the neutral `crate::imports`.
//!
//! Scope: NAMED imports/re-exports resolve to a target *symbol*. DEFAULT
//! imports (`import Foo from './x'`) are tracked too but resolve only to a
//! target *file*, not a symbol — enough for the rendered-component `core`
//! expansion, which is why they're tracked at all. `import * as ns from
//! './x'` (namespace) and `export * from './x'` (wildcard re-export) are
//! still untracked — a namespace import targets no single declaration, and a
//! wildcard re-export would need the source file's whole export list
//! enumerated. See `CLAUDE.md`'s pending decisions.

use super::conventions::ts_parser;
use crate::imports::{DefaultImport, FileImports, ImportRef, ReExport};
use std::collections::HashMap;
use tree_sitter::Node;

/// Parse `source` (the contents of `rel_path`) and extract its named
/// imports and named re-exports.
pub fn extract_imports(source: &str, rel_path: &str) -> FileImports {
    let mut parser = ts_parser(rel_path);

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
    visit_bare_reexports(root, source, &mut out);
    out
}

/// One name in an `export { … }` clause: the `name` it exports and, when it
/// is published under a different one, the `alias`. An alias equal to the
/// name (`export { a as a }`) is no rename and comes back as `None`.
pub(super) struct ExportSpec<'a> {
    pub name: &'a str,
    pub alias: Option<&'a str>,
}

/// The names in an `export_clause` node, in source order. The one place that
/// knows the clause's shape, shared by the re-export forms here and by
/// `mod.rs`'s clause-exported declarations.
pub(super) fn export_specs<'a>(clause: Node, source: &'a str) -> Vec<ExportSpec<'a>> {
    let mut cursor = clause.walk();
    clause
        .children(&mut cursor)
        .filter(|spec| spec.kind() == "export_specifier")
        .filter_map(|spec| {
            let name = field_text(spec, "name", source)?;
            let alias = field_text(spec, "alias", source).filter(|a| *a != name);
            Some(ExportSpec { name, alias })
        })
        .collect()
}

/// The `export_clause` of a bare export, `export { X, Y as Z }`: the one
/// with no `from`. `None` for any other statement, including
/// `export { X } from '…'` (a re-export of another module's name, which says
/// nothing about a local one) and `export function f() {}`.
pub(super) fn bare_export_clause<'t>(stmt: Node<'t>) -> Option<Node<'t>> {
    if stmt.kind() != "export_statement" || stmt.child_by_field_name("source").is_some() {
        return None;
    }
    child_of_kind(stmt, "export_clause")
}

/// Every named import's `(name in the source module, local name)`, in
/// source order.
fn named_import_specs<'a>(named: Node, source: &'a str) -> Vec<(&'a str, &'a str)> {
    let mut cursor = named.walk();
    named
        .children(&mut cursor)
        .filter(|spec| spec.kind() == "import_specifier")
        .filter_map(|spec| {
            let name = field_text(spec, "name", source)?;
            Some((name, field_text(spec, "alias", source).unwrap_or(name)))
        })
        .collect()
}

fn visit_import(node: Node, source: &str, out: &mut FileImports) {
    let Some(specifier) = string_field(node, "source", source) else {
        return;
    };
    let Some(clause) = child_of_kind(node, "import_clause") else {
        return; // side-effect-only import: `import './polyfill'`
    };

    // The default binding is an `identifier` directly under `import_clause`
    // (`import Foo from …`, or the `Foo` in `import Foo, { Bar } from …`).
    // `import * as ns` is a `namespace_import` child, deliberately skipped.
    let mut cursor = clause.walk();
    if let Some(default_id) = clause
        .children(&mut cursor)
        .find(|c| c.kind() == "identifier")
    {
        out.default_imports.push(DefaultImport {
            local_name: text(default_id, source).to_string(),
            specifier: specifier.clone(),
        });
    }

    let Some(named) = child_of_kind(clause, "named_imports") else {
        return;
    };

    for (name, _local) in named_import_specs(named, source) {
        out.imports.push(ImportRef {
            specifier: specifier.clone(),
            imported_name: name.to_string(),
        });
    }
}

/// `export { X }` / `export { X as Y }` with no `from`: when `X` is a name
/// this file imported, the file is forwarding that import, so record it as
/// the re-export `export { <original> as Y } from '<its specifier>'` --
/// the form `resolve::resolve_named` already follows. A name the file
/// declares itself is not a re-export (that is `mod.rs`'s business), and
/// default/namespace imports are not tracked.
///
/// A file with no bare clause, the large majority, returns before looking at
/// its imports at all. Otherwise the imports are read in a pass of their
/// own, because they are hoisted: `export { X }` may legally come before
/// `import { X }`.
fn visit_bare_reexports(root: Node, source: &str, out: &mut FileImports) {
    let mut cursor = root.walk();
    let clauses: Vec<Node> = root
        .children(&mut cursor)
        .filter_map(bare_export_clause)
        .collect();
    if clauses.is_empty() {
        return;
    }

    // local name -> (specifier, name in the source module)
    let mut locals: HashMap<&str, (String, &str)> = HashMap::new();
    let mut cursor = root.walk();
    for stmt in root.children(&mut cursor) {
        if stmt.kind() != "import_statement" {
            continue;
        }
        let Some(specifier) = string_field(stmt, "source", source) else {
            continue;
        };
        let Some(named) =
            child_of_kind(stmt, "import_clause").and_then(|c| child_of_kind(c, "named_imports"))
        else {
            continue;
        };
        for (name, local) in named_import_specs(named, source) {
            locals.insert(local, (specifier.clone(), name));
        }
    }

    for clause in clauses {
        for spec in export_specs(clause, source) {
            let Some((specifier, source_name)) = locals.get(spec.name) else {
                continue;
            };
            out.re_exports.push(ReExport {
                exported_as: spec.alias.unwrap_or(spec.name).to_string(),
                specifier: specifier.clone(),
                source_name: (*source_name).to_string(),
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

    for spec in export_specs(clause, source) {
        out.re_exports.push(ReExport {
            exported_as: spec.alias.unwrap_or(spec.name).to_string(),
            specifier: specifier.clone(),
            source_name: spec.name.to_string(),
        });
    }
}

fn child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).find(|c| c.kind() == kind)
}

fn text<'a>(node: Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or_default()
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
    fn default_import_is_tracked_separately_from_named() {
        let out = extract_imports("import Foo from './foo';\n", "a.ts");
        assert!(out.imports.is_empty());
        assert_eq!(
            out.default_imports,
            vec![DefaultImport {
                local_name: "Foo".into(),
                specifier: "./foo".into(),
            }]
        );
    }

    #[test]
    fn namespace_import_is_not_tracked() {
        let out = extract_imports("import * as ns from './foo';\n", "a.ts");
        assert!(out.imports.is_empty());
        assert!(out.default_imports.is_empty());
    }

    #[test]
    fn side_effect_only_import_is_not_tracked() {
        let out = extract_imports("import './polyfill';\n", "a.ts");
        assert!(out.imports.is_empty());
    }

    #[test]
    fn default_plus_named_import_splits_the_two_halves() {
        let out = extract_imports("import Foo, { Bar } from './x';\n", "a.ts");
        assert_eq!(out.imports.len(), 1);
        assert_eq!(out.imports[0].imported_name, "Bar");
        assert_eq!(out.default_imports[0].local_name, "Foo");
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

    // --- A bare `export { X }` (no `from`) of a name the file imported is a
    // re-export of that import: its origin is on the `import` line, not the
    // `export` line.

    fn reexport(exported_as: &str, specifier: &str, source_name: &str) -> ReExport {
        ReExport {
            exported_as: exported_as.into(),
            specifier: specifier.into(),
            source_name: source_name.into(),
        }
    }

    #[test]
    fn a_bare_export_of_an_imported_name_is_a_reexport_of_that_import() {
        let out = extract_imports("import { X } from './y';\nexport { X };\n", "a.ts");
        assert_eq!(out.re_exports, vec![reexport("X", "./y", "X")]);
        // The import itself is still an import.
        assert_eq!(out.imports.len(), 1);
    }

    #[test]
    fn a_bare_export_of_an_aliased_import_follows_the_original_name() {
        let out = extract_imports("import { A as B } from './y';\nexport { B };\n", "a.ts");
        assert_eq!(out.re_exports, vec![reexport("B", "./y", "A")]);
    }

    #[test]
    fn a_renamed_bare_export_of_an_imported_name_keeps_both_names() {
        let out = extract_imports("import { A } from './y';\nexport { A as C };\n", "a.ts");
        assert_eq!(out.re_exports, vec![reexport("C", "./y", "A")]);
    }

    #[test]
    fn the_import_may_come_after_the_export_clause() {
        // Imports are hoisted, so this is valid and means the same thing.
        let out = extract_imports("export { X };\nimport { X } from './y';\n", "a.ts");
        assert_eq!(out.re_exports, vec![reexport("X", "./y", "X")]);
    }

    #[test]
    fn type_only_imports_and_exports_are_followed_too() {
        let out = extract_imports(
            "import type { T } from './y';\nexport type { T };\n",
            "a.ts",
        );
        assert_eq!(out.re_exports, vec![reexport("T", "./y", "T")]);
    }

    #[test]
    fn a_multi_name_clause_re_exports_only_the_imported_names() {
        let out = extract_imports(
            "import { A, B } from './y';\nconst C = 1;\nexport {\n  A,\n  B,\n  C,\n};\n",
            "a.ts",
        );
        assert_eq!(
            out.re_exports,
            vec![reexport("A", "./y", "A"), reexport("B", "./y", "B")]
        );
    }

    #[test]
    fn a_bare_export_of_a_locally_declared_name_is_not_a_reexport() {
        let out = extract_imports("const X = 1;\nexport { X };\n", "a.ts");
        assert!(out.re_exports.is_empty());
    }

    #[test]
    fn a_bare_export_of_a_default_or_namespace_import_is_not_tracked() {
        // Neither resolves to a single named symbol (see the module docs).
        let out = extract_imports(
            "import D from './y';\nimport * as N from './z';\nexport { D, N };\n",
            "a.ts",
        );
        assert!(out.re_exports.is_empty());
    }

    #[test]
    fn an_alias_equal_to_the_name_re_exports_it_unchanged() {
        let out = extract_imports("import { A } from './y';\nexport { A as A };\n", "a.ts");
        assert_eq!(out.re_exports, vec![reexport("A", "./y", "A")]);
    }

    #[test]
    fn a_file_with_imports_but_no_export_clause_has_no_reexports() {
        let out = extract_imports("import { A } from './y';\nexport function f() {}\n", "a.ts");
        assert!(out.re_exports.is_empty());
        assert_eq!(out.imports.len(), 1);
    }

    #[test]
    fn a_reexport_with_a_source_is_unchanged_by_the_bare_form() {
        let out = extract_imports(
            "import { X } from './y';\nexport { X } from './z';\n",
            "a.ts",
        );
        assert_eq!(out.re_exports, vec![reexport("X", "./z", "X")]);
    }
}
