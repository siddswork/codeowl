//! Stack selection ([`detect`]) plus the TypeScript + SQL pack's own
//! internals — which extensions count as source, which tree-sitter
//! grammar to load, which extractor a file goes to, which extensions the
//! module resolver tries, the JS test-path / UI-primitive heuristics.
//! `TypeScriptNextStack` (in `stack.rs`) delegates to the free functions
//! here; `RustStack` has its own in `rust.rs`.
//!
//! [`SourceKind`] and [`FileRole`] are stack-neutral (the `StackPack`
//! trait returns them); everything else in this module is TS-specific and
//! moves fully behind `TypeScriptNextStack` in a later M14 commit, at
//! which point `detect` is all that's left here worth keeping generic.
//! See `ROADMAP.md`'s "Phase 2".

use std::path::Path;

use anyhow::{Result, bail};
use tree_sitter::Parser;

use crate::stack::StackPack;
use crate::symbol::ExtractedSymbol;

/// Extensions the module resolver (`resolve.rs`) tries when following an
/// import specifier — Node/TypeScript resolution order.
pub const RESOLVER_EXTENSIONS: &[&str] = &[".ts", ".tsx", ".d.ts", ".js", ".jsx", ".json"];

/// What a `StackPack::source_kind` says about a file it reads. Stack-
/// neutral vocabulary (like `SymbolKind`), but which *extension* maps to
/// which kind is each pack's decision, not this module's — as of M17
/// `lang.rs` no longer names `.sql` at all; `TypeScriptNextStack` owns
/// that mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// A normal source file — parsed by the pack's grammar, and put
    /// through the import / flow-edge passes.
    Code,
    /// A dedicated schema file (the TS pack's `.sql`) — parsed for table
    /// declarations only; `index.rs` skips the import / flow-edge passes.
    /// An *in-language* ORM model is not this — it's a `Code` file whose
    /// model class `is_schema_symbol` retags to `SymbolKind::Schema` (M17).
    Schema,
}

impl SourceKind {
    /// The kind for a TypeScript path — `.ts`/`.tsx` (never `.d.ts`, whose
    /// grammar shapes M1 doesn't handle). Only `TypeScriptNextStack` calls
    /// this; every other pack matches its own extension directly, and the
    /// TS pack maps `.sql` → `Schema` before falling back here.
    pub fn of(rel_path: &str) -> Option<SourceKind> {
        if rel_path.ends_with(".d.ts") {
            None
        } else if rel_path.ends_with(".ts") || rel_path.ends_with(".tsx") {
            Some(SourceKind::Code)
        } else {
            None
        }
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

/// Parse a TypeScript file into its declarations — the TS pack's symbol
/// extractor, kept as a free function so `graph.rs`'s test helper and
/// `TypeScriptNextStack` share one entry point. `.sql` dispatch moved into
/// the pack at M17; this is TS-only now.
pub fn extract_symbols(rel_path: &str, source: &str) -> Vec<ExtractedSymbol> {
    crate::extract::extract_file(source, rel_path)
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

/// Pick the one `StackPack` for `root`, or fail fast — rather than
/// silently building and serving an empty graph. Called once at startup,
/// from `RepoIndex::build`/`open`.
///
/// A pack "recognises" a repo by the count of files whose `source_kind`
/// is [`SourceKind::Code`] — its *primary* language (`.ts`/`.tsx` for TS,
/// `.rs` for Rust, `.java` for Java). A `.sql` schema file is
/// `SourceKind::Schema`, not `Code`, so it never makes a repo "TypeScript"
/// on its own (a Rust repo with migrations is still a Rust repo — its
/// schema story is M18's).
///
/// Exactly one pack may claim the repo (M13 design decision 6 — a repo
/// that's genuinely two stacks is out of scope for Phase 2; multi-pack
/// per repo is deferred). Zero → error naming the supported stacks.
pub fn detect(root: &Path) -> Result<Box<dyn crate::stack::StackPack>> {
    // A repo's stack identity is its *source*, not its test trees — a
    // `tests/fixtures/foo.tsx` read as a string by a Rust test mustn't
    // make CodeOwl's own repo look like TypeScript. Skip the conventional
    // test / bench directories for the detection count only (they're still
    // walked and extracted once a pack is chosen).
    fn is_non_identity_tree(rel: &str) -> bool {
        let seg = |d: &str| rel == d || rel.starts_with(&format!("{d}/"));
        // Test trees: JS/Rust keep tests in a top-level `tests` / `test` /
        // `benches`; Maven/Gradle keep them under `src/test/...`.
        let is_test = seg("tests") || seg("test") || seg("benches") || rel.contains("src/test/");
        // Tooling trees: build / dev / CI scripts don't define a repo's
        // stack identity — CodeOwl's own `utility/*.py` mustn't make this
        // Rust repo look ambiguously Python.
        let is_tooling = seg("utility") || seg("scripts") || seg("tools") || seg("hack");
        is_test || is_tooling
    }

    fn primary_count(root: &Path, pack: &dyn StackPack) -> usize {
        ignore::WalkBuilder::new(root)
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_some_and(|t| t.is_file()))
            .filter(|entry| {
                entry
                    .path()
                    .strip_prefix(root)
                    .ok()
                    .and_then(|p| p.to_str())
                    .is_none_or(|rel| !is_non_identity_tree(rel))
            })
            .filter(|entry| pack.source_kind(entry.path()) == Some(SourceKind::Code))
            .count()
    }

    type Ctor = fn() -> Box<dyn crate::stack::StackPack>;
    let candidates: [(&str, Ctor); 4] = [
        ("TypeScript/TSX", || {
            Box::new(crate::stack::TypeScriptNextStack)
        }),
        ("Rust", || Box::new(crate::stack::RustStack)),
        ("Java", || Box::new(crate::stack::JavaStack)),
        ("Python", || Box::new(crate::stack::PythonStack)),
    ];

    let hits: Vec<(&str, usize, Ctor)> = candidates
        .iter()
        .map(|(label, ctor)| (*label, primary_count(root, ctor().as_ref()), *ctor))
        .filter(|(_, n, _)| *n > 0)
        .collect();

    match hits.as_slice() {
        [] => bail!(
            "no source files CodeOwl can extract were found under {} — it handles \
             TypeScript/TSX (+ SQL schema), Rust, and Java (see ROADMAP.md)",
            root.display()
        ),
        [(_, _, ctor)] => Ok(ctor()),
        multiple => {
            let listed = multiple
                .iter()
                .map(|(label, n, _)| format!("{label} ({n} files)"))
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "{} contains more than one stack CodeOwl recognises — {listed}. CodeOwl \
                 serves one stack per repo (ROADMAP.md design decision 6). Point it at a \
                 subdirectory that's a single stack.",
                root.display()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_kind_of_is_typescript_only_now() {
        // `.sql` left `lang.rs` at M17 — `TypeScriptNextStack::source_kind`
        // maps it; this free function is TS extensions only.
        assert_eq!(SourceKind::of("a/b.ts"), Some(SourceKind::Code));
        assert_eq!(SourceKind::of("a/b.tsx"), Some(SourceKind::Code));
        assert_eq!(SourceKind::of("a/b.sql"), None);
        assert_eq!(SourceKind::of("a/b.d.ts"), None);
        assert_eq!(SourceKind::of("a/b.js"), None);
        assert_eq!(SourceKind::of("README.md"), None);
    }

    #[test]
    fn extract_symbols_parses_typescript() {
        let ts = extract_symbols("a.ts", "export function f() {}\n");
        assert_eq!(ts[0].id, "a.ts::f");
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
