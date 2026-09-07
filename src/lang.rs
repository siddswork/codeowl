//! The one module that knows CodeOwl's Phase 1 extractor is TypeScript +
//! SQL and nothing else. Everything stack-specific that used to be spread
//! across `extract.rs`, `imports.rs`, `features.rs`, `resolve.rs`,
//! `schema.rs`, and `index.rs` — which extensions count as source, which
//! tree-sitter grammar to load, which extractor a file goes to, which
//! extensions the module resolver tries — is centralized here.
//!
//! Free functions and constants, deliberately not a trait: the interface
//! is shaped against two extractor kinds ([`SourceKind`]) and — via
//! `features.rs` — a handful of convention resolvers (route literals,
//! `.from()` table refs, rendered components), which is enough to see the
//! seam but not enough to design a good abstraction. M13 promotes this to
//! a `StackPack` trait (renamed from `LanguagePack` — the Phase 1 pack
//! already spans TS + SQL + Next + Supabase, a *stack*, not a language)
//! once a genuinely different stack (Rust) is there to check it against.
//! See `ROADMAP.md`'s "Phase 2".

use std::path::Path;

use anyhow::{Result, bail};
use tree_sitter::Parser;

use crate::stack::StackPack;
use crate::symbol::ExtractedSymbol;

/// Extensions the module resolver (`resolve.rs`) tries when following an
/// import specifier — Node/TypeScript resolution order.
pub const RESOLVER_EXTENSIONS: &[&str] = &[".ts", ".tsx", ".d.ts", ".js", ".jsx", ".json"];

/// Which extractor a source file's *contents* go to. Derived from the
/// extension for now; M13 folds this and `is_extractable` into a single
/// `pack.is_source_file(path) -> Option<SourceKind>` the active stack
/// owns. See `ROADMAP.md`'s "Phase 2".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// TypeScript / TSX — `ts_parser` + `extract.rs`.
    Code,
    /// A SQL schema file — `tree-sitter-sequel` + `schema.rs`.
    Schema,
}

impl SourceKind {
    /// The kind for a repo-relative path, or `None` when CodeOwl's Phase 1
    /// extractor reads nothing from it. `.d.ts` is `None` on purpose —
    /// ambient declaration files use grammar shapes M1 doesn't handle (see
    /// `ROADMAP.md`'s M1 scope). The single source of truth behind both
    /// `is_extractable` and the extract dispatch.
    pub fn of(rel_path: &str) -> Option<SourceKind> {
        if rel_path.ends_with(".d.ts") {
            None
        } else if rel_path.ends_with(".sql") {
            Some(SourceKind::Schema)
        } else if rel_path.ends_with(".ts") || rel_path.ends_with(".tsx") {
            Some(SourceKind::Code)
        } else {
            None
        }
    }
}

/// Whether CodeOwl extracts anything from this file — `.ts`/`.tsx` (never
/// `.d.ts`) and `.sql`. The single definition `main.rs`, the catch-up
/// pass, and the file watcher all share. "What goes to which extractor"
/// is [`SourceKind::of`].
pub fn is_extractable(path: &Path) -> bool {
    path.to_str().and_then(SourceKind::of).is_some()
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

/// Extract a file's symbols with the right extractor for its kind — SQL
/// tables for a schema file, TypeScript declarations otherwise. The single
/// dispatch point `graph.rs` and `index.rs` both call. A path with no
/// [`SourceKind`] (never reached from the walk, which filters on
/// `is_extractable` first) is parsed as `Code`.
pub fn extract_symbols(rel_path: &str, source: &str) -> Vec<ExtractedSymbol> {
    match SourceKind::of(rel_path) {
        Some(SourceKind::Schema) => crate::schema::extract_tables(source, rel_path),
        Some(SourceKind::Code) | None => crate::extract::extract_file(source, rel_path),
    }
}

/// What kind of code a file holds, for the purpose of deciding how its
/// spec is prioritised (`spec::prioritize`) and whether it counts as
/// shared infrastructure. A single classification replacing the scattered
/// `is_test_path` / `is_ui_primitive` predicates — the seam a `StackPack`
/// eventually owns, since "what's a UI primitive" and "what's generated"
/// are stack conventions, not universal truths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRole {
    /// Product code — the default. Eligible for the shared-code tier,
    /// counted in import fan-in, documented in normal priority order.
    Domain,
    /// A UI primitive (`components/ui/*`): imported nearly everywhere, so
    /// it dominates a fan-in ranking, but its summary tells a dependent
    /// feature spec nothing it couldn't guess — kept out of the
    /// shared-code tier. Still gets its own file spec, in the long tail.
    Primitive,
    /// Test code — an `e2e/`/`cypress/`/`playwright/` tree, a `__tests__/`
    /// directory, or a `.test.`/`.spec.` file. Stays in the graph so
    /// `get_callers` still shows "used by these tests", but its specs sort
    /// last and it's never treated as a product module.
    Test,
    /// Machine-generated code. Reserved: no Phase 1 path convention
    /// produces this yet — a `StackPack` supplies the rule (Supabase's
    /// `database.types.ts`, protobuf `*_pb.ts`, GraphQL codegen, …).
    /// Treated like [`FileRole::Primitive`] until then.
    Generated,
}

/// Classify a repo-relative path into its [`FileRole`]. Accepts a
/// `rollup:`/`feature:` coverage id too — the prefix is stripped first, so
/// a directory-rollup id classifies by its directory path.
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

/// Pick the `StackPack` for `root`, or fail fast if no pack recognises it
/// — rather than silently building and serving an empty graph. Called once
/// at startup, from `RepoIndex::build`/`open`. Phase 1 has one pack
/// (`TypeScriptNextStack`); the walk here becomes a per-pack `recognises`
/// probe when there's a second one (M14).
pub fn detect(root: &Path) -> Result<Box<dyn crate::stack::StackPack>> {
    let pack = crate::stack::TypeScriptNextStack;
    let has_source = ignore::WalkBuilder::new(root)
        .build()
        .filter_map(|entry| entry.ok())
        .any(|entry| {
            entry.file_type().is_some_and(|t| t.is_file())
                && StackPack::source_kind(&pack, entry.path()).is_some()
        });
    if !has_source {
        bail!(
            "no .ts/.tsx or .sql files found under {} — CodeOwl's Phase 1 extractor only \
             handles TypeScript/TSX and SQL schema files (see ROADMAP.md)",
            root.display()
        );
    }
    Ok(Box::new(pack))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_extractable_accepts_ts_tsx_sql_and_rejects_dts() {
        assert!(is_extractable(Path::new("a/b.ts")));
        assert!(is_extractable(Path::new("a/b.tsx")));
        assert!(is_extractable(Path::new("supabase/schema.sql")));
        assert!(!is_extractable(Path::new("a/b.d.ts")));
        assert!(!is_extractable(Path::new("a/b.js")));
        assert!(!is_extractable(Path::new("README.md")));
    }

    #[test]
    fn source_kind_maps_extensions() {
        // Purely on extension — the directory is never consulted.
        assert_eq!(SourceKind::of("a/b.ts"), Some(SourceKind::Code));
        assert_eq!(SourceKind::of("a/b.tsx"), Some(SourceKind::Code));
        assert_eq!(SourceKind::of("a/b.sql"), Some(SourceKind::Schema));
        assert_eq!(SourceKind::of("a/b.d.ts"), None);
        assert_eq!(SourceKind::of("a/b.js"), None);
        assert_eq!(SourceKind::of("README.md"), None);
    }

    #[test]
    fn extract_symbols_dispatches_on_extension() {
        let ts = extract_symbols("a.ts", "export function f() {}\n");
        assert_eq!(ts[0].id, "a.ts::f");

        let sql = extract_symbols("s.sql", "CREATE TABLE public.t (id integer);\n");
        assert_eq!(sql[0].id, "s.sql::t");
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

    #[test]
    fn detect_bails_on_a_dir_with_no_source() {
        let dir = std::env::temp_dir().join(format!("codeowl-lang-detect-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.md"), "hi\n").unwrap();
        assert!(detect(&dir).is_err());

        std::fs::write(dir.join("a.ts"), "export const x = 1;\n").unwrap();
        assert!(detect(&dir).is_ok());

        std::fs::remove_dir_all(&dir).ok();
    }
}
