//! The one module that knows CodeOwl's Phase 1 extractor is TypeScript +
//! SQL and nothing else. Everything stack-specific that used to be spread
//! across `extract.rs`, `imports.rs`, `features.rs`, `resolve.rs`,
//! `schema.rs`, and `index.rs` — which extensions count as source, which
//! tree-sitter grammar to load, which extractor a file goes to, which
//! extensions the module resolver tries — is centralized here.
//!
//! Free functions and constants, deliberately not a trait: the interface
//! is shaped against two extractor kinds (TS symbols, SQL tables) and — via
//! `features.rs` — two convention resolvers (route literals, `.from()`
//! table refs), which is enough to see the seam but not enough to design a
//! good abstraction. Phase 2 promotes this to a `LanguagePack` trait once a
//! genuinely different language (Rust, C++) is there to check it against.
//! See `ROADMAP.md`'s "Stack modularization".

use std::path::Path;

use anyhow::{Result, bail};
use tree_sitter::Parser;

use crate::symbol::ExtractedSymbol;

/// Extensions the module resolver (`resolve.rs`) tries when following an
/// import specifier — Node/TypeScript resolution order.
pub const RESOLVER_EXTENSIONS: &[&str] = &[".ts", ".tsx", ".d.ts", ".js", ".jsx", ".json"];

/// Whether CodeOwl extracts anything from this file: `.ts`/`.tsx` (never
/// `.d.ts` — ambient declaration files use grammar shapes M1 doesn't
/// handle, see `ROADMAP.md`'s M1 scope) and `.sql` (M10 schema files).
/// The single definition `main.rs`, the catch-up pass, and the file
/// watcher all share.
pub fn is_extractable(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if name.ends_with(".d.ts") {
        return false;
    }
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts") | Some("tsx") | Some("sql")
    )
}

/// Whether a file goes to the SQL schema extractor rather than the
/// TypeScript one. Kept as its own predicate (not an inline `.ends_with`)
/// so the "SQL files are schema files" assumption has one name.
pub fn is_schema_file(rel_path: &str) -> bool {
    rel_path.ends_with(".sql")
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
/// dispatch point `graph.rs` and `index.rs` both call.
pub fn extract_symbols(rel_path: &str, source: &str) -> Vec<ExtractedSymbol> {
    if is_schema_file(rel_path) {
        crate::schema::extract_tables(source, rel_path)
    } else {
        crate::extract::extract_file(source, rel_path)
    }
}

/// Fail fast if `root` has no files the Phase 1 extractor can read, rather
/// than silently building — and serving — an empty graph. Called once at
/// startup (`main.rs`), before the full walk.
pub fn detect(root: &Path) -> Result<()> {
    let has_source = ignore::WalkBuilder::new(root)
        .build()
        .filter_map(|entry| entry.ok())
        .any(|entry| {
            entry.file_type().is_some_and(|t| t.is_file()) && is_extractable(entry.path())
        });
    if !has_source {
        bail!(
            "no .ts/.tsx or .sql files found under {} — CodeOwl's Phase 1 extractor only \
             handles TypeScript/TSX and SQL schema files (see ROADMAP.md)",
            root.display()
        );
    }
    Ok(())
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
    fn extract_symbols_dispatches_on_extension() {
        let ts = extract_symbols("a.ts", "export function f() {}\n");
        assert_eq!(ts[0].id, "a.ts::f");

        let sql = extract_symbols("s.sql", "CREATE TABLE public.t (id integer);\n");
        assert_eq!(sql[0].id, "s.sql::t");
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
