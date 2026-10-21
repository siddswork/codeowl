//! The TypeScript + SQL pack's own internals — which extensions count as
//! source, which tree-sitter grammar to load, which extensions the module
//! resolver tries, the JS test-path / UI-primitive heuristics.
//! `TypeScriptNextStack` (in `stack.rs`) delegates to the free functions
//! here, the way `RustStack` delegates to `rust/mod.rs`. The stack-neutral
//! vocabulary (`SourceKind`, `FileRole`) stays in `lang.rs`.

use tree_sitter::Parser;

use crate::lang::{FileRole, SourceKind};

/// Extensions the module resolver (`resolve.rs`) tries when following an
/// import specifier — Node/TypeScript resolution order.
pub(super) const RESOLVER_EXTENSIONS: &[&str] = &[".ts", ".tsx", ".d.ts", ".js", ".jsx", ".json"];

/// The kind for a TypeScript path — `.ts`/`.tsx` (never `.d.ts`, whose
/// grammar shapes aren't handled). Only `TypeScriptNextStack` calls this;
/// it maps `.sql` → `Schema` before falling back here.
pub fn source_kind(rel_path: &str) -> Option<SourceKind> {
    if rel_path.ends_with(".d.ts") {
        None
    } else if rel_path.ends_with(".ts") || rel_path.ends_with(".tsx") {
        Some(SourceKind::Code)
    } else {
        None
    }
}

/// A tree-sitter `Parser` loaded with the right grammar for `rel_path`:
/// `.tsx` gets JSX support, every other extension parses as plain
/// TypeScript. (A `.sql` file never reaches here — it goes to
/// `schema.rs`'s own `tree-sitter-sequel` parser.)
pub fn ts_parser(rel_path: &str) -> Parser {
    let language = if rel_path.ends_with(".tsx") {
        tree_sitter_typescript::LANGUAGE_TSX
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT
    };
    let mut parser = Parser::new();
    parser
        .set_language(&language.into())
        .expect("bundled tree-sitter-typescript grammar should always load");
    parser
}

/// Classify a repo-relative path into its [`FileRole`] under the JS
/// conventions. Accepts a `rollup:`/`feature:` coverage id too — the
/// prefix is stripped first, so a directory-rollup id classifies by its
/// directory path.
pub fn classify(path: &str) -> FileRole {
    let path = path
        .strip_prefix("rollup:")
        .or_else(|| path.strip_prefix("feature:"))
        .unwrap_or(path);
    if is_test_path(path) {
        FileRole::Test
    } else if is_ui_primitive(path) {
        FileRole::Primitive
    } else {
        FileRole::Domain
    }
}

fn is_test_path(path: &str) -> bool {
    path.starts_with("e2e/")
        || path.contains("/e2e/")
        || path.starts_with("cypress/")
        || path.contains("/cypress/")
        || path.starts_with("playwright/")
        || path.contains("/playwright/")
        || path.contains("__tests__/")
        || path.contains(".test.")
        || path.contains(".spec.")
}

fn is_ui_primitive(path: &str) -> bool {
    path.starts_with("components/ui/") || path.contains("/components/ui/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_kind_is_typescript_extensions_only() {
        // `.sql` is mapped by `TypeScriptNextStack::source_kind`; this
        // free function is TS extensions only.
        assert_eq!(source_kind("a/b.ts"), Some(SourceKind::Code));
        assert_eq!(source_kind("a/b.tsx"), Some(SourceKind::Code));
        assert_eq!(source_kind("a/b.sql"), None);
        assert_eq!(source_kind("a/b.d.ts"), None);
        assert_eq!(source_kind("a/b.js"), None);
        assert_eq!(source_kind("README.md"), None);
    }

    #[test]
    fn classify_partitions_paths_by_role() {
        use FileRole::*;
        assert_eq!(classify("lib/utils.ts"), Domain);
        assert_eq!(classify("app/submit/page.tsx"), Domain);

        assert_eq!(classify("components/ui/button.tsx"), Primitive);
        assert_eq!(classify("src/components/ui/dialog.tsx"), Primitive);

        assert_eq!(classify("e2e/helpers/api-client.ts"), Test);
        assert_eq!(classify("src/components/__tests__/button.ts"), Test);
        assert_eq!(classify("lib/utils.test.ts"), Test);
        assert_eq!(classify("app/page.spec.tsx"), Test);

        // Coverage-id prefixes are stripped before matching.
        assert_eq!(classify("rollup:e2e/helpers"), Test);
        assert_eq!(classify("feature:app/submit"), Domain);

        // A "test" substring that isn't a test-file convention stays Domain.
        assert_eq!(classify("app/api/attest/route.ts"), Domain);
    }
}
