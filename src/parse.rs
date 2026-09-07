//! Shared tree-sitter parser setup for the TypeScript + Next.js pack.
//!
//! The `.tsx` vs `.ts` grammar pick was copy-pasted across `extract.rs`,
//! `imports.rs`, and `features.rs`; it lives here so exactly one place knows
//! that CodeOwl currently only parses TypeScript. This is the seam M11's
//! `src/lang.rs` will formalize (grammar pick + `is_extractable` + the
//! resolver extension list behind one `detect(root)`) and Phase 2's
//! `LanguagePack` trait will make pluggable — see `ROADMAP.md`'s "Stack
//! modularization".

use tree_sitter::Parser;

/// A tree-sitter `Parser` loaded with the right grammar for `rel_path`:
/// `.tsx` gets JSX support, every other extension parses as plain
/// TypeScript.
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
