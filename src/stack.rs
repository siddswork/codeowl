//! `trait StackPack` — the single interface every stack-specific decision
//! passes through (M13).
//!
//! Phase 1's extractor is TypeScript + SQL + Next.js + Supabase, spread
//! across `extract.rs` / `imports.rs` / `resolve.rs` / `schema.rs` /
//! `features.rs` and reached directly from the generic core (`index.rs`,
//! `graph.rs`, `spec.rs`). M13 forces every one of those coupling points
//! through this trait so M14 (`RustStack`) has an interface to implement
//! against rather than a scatter of free functions to shadow.
//!
//! The trait grows milestone by milestone: M13 covers source
//! classification, symbol + import extraction, import resolution, flow
//! edges (`extract_flow_edges` / `resolve_flow_edge`), and the optional
//! `feature_model()` (the Next-specific "a feature is a page plus what it
//! reaches" concept — `None` for a stack with no runtime entry surface).
//! See `ROADMAP.md`'s "Phase 2" and `experiments/exp-02-feature-layer.md`.
//!
//! **Still one pack.** `TypeScriptNextStack` here just delegates to the
//! existing free functions, which stay `pub` as shims so the ~90 test call
//! sites compile unchanged. The point is the seam, not new behaviour.

use std::collections::HashMap;
use std::path::Path;

use crate::graph::{FlowTarget, Graph, UnresolvedFlowEdge};
use crate::imports::FileImports;
use crate::lang::{FileRole, SourceKind};
use crate::resolve::ResolvedImport;
use crate::symbol::ExtractedSymbol;

/// Everything the generic core needs a stack to decide. One `StackPack`
/// per repo, chosen by [`crate::lang::detect`]. `Send + Sync` because the
/// file watcher carries the active pack to its background thread; `Debug`
/// so the structs that hold one (`RepoIndex`) still derive it.
pub trait StackPack: Send + Sync + std::fmt::Debug {
    /// A stable identifier for this pack — persisted alongside the cache so
    /// switching packs forces a rebuild (M13 design decision 7).
    fn name(&self) -> &str;

    /// Which extractor a file's *contents* go to, or `None` when the pack
    /// reads nothing from it. Replaces `lang::is_extractable` +
    /// `SourceKind::of` — a file is walkable iff this returns `Some`.
    fn source_kind(&self, path: &Path) -> Option<SourceKind>;

    /// The role a repo-relative path plays for spec prioritisation.
    /// Replaces the scattered `is_test_path` / `is_ui_primitive`.
    fn classify(&self, rel_path: &str) -> FileRole;

    /// Parse one file into its declarations. The pack picks the grammar
    /// and maps its node kinds onto `SymbolKind`.
    fn extract_symbols(&self, rel_path: &str, source: &str) -> Vec<ExtractedSymbol>;

    /// Parse one file's import / re-export statements.
    fn extract_imports(&self, rel_path: &str, source: &str) -> FileImports;

    /// Resolve every tracked import across the repo to a target `SymbolId`
    /// (or `None` for externals / unresolved). The pack owns resolver
    /// construction end to end — `oxc_resolver` for TS, a module-tree walk
    /// for Rust (M13 design decision 5).
    fn resolve_imports(
        &self,
        root: &Path,
        file_imports: &HashMap<String, FileImports>,
        graph: &Graph,
    ) -> Vec<ResolvedImport>;

    /// The "this file reaches that thing" edges the import graph can't see
    /// — for `TypeScriptNextStack`, `fetch("/api/…")`, `.from("table")`,
    /// `<Component/>`. One raw string per edge; resolution is a separate
    /// step. A pack with no such conventions returns an empty vec.
    fn extract_flow_edges(&self, rel_path: &str, source: &str) -> Vec<UnresolvedFlowEdge>;

    /// Resolve one flow edge against the built graph. Called per edge at
    /// graph-build time, after imports are resolved. `FlowTarget::Unresolved`
    /// for an edge whose raw string points nowhere (external URL, DB view,
    /// dynamic path, typo).
    fn resolve_flow_edge(&self, graph: &Graph, edge: &UnresolvedFlowEdge) -> FlowTarget;

    /// The stack's model of "what is a feature", or `None` if it has no
    /// runtime entry surface — a CLI or a utility library gets symbol /
    /// file / rollup / system specs and no feature layer (M13-pre;
    /// `ROADMAP.md` M13 piece 3, design decision 3). M14 (`RustStack`) and
    /// M16 (commons-lang) both return `None`; M17 (Quarkus) is where this
    /// gets its second implementation.
    fn feature_model(&self) -> Option<&dyn crate::features::FeatureModel> {
        None
    }
}

/// The Phase 1 stack: TypeScript / TSX + SQL schema files, Next.js App
/// Router routing, Supabase `.from()`. Every method delegates to the
/// free functions in `extract.rs` / `imports.rs` / `resolve.rs` /
/// `lang.rs`; M13 is a re-wiring, not a rewrite.
#[derive(Debug, Default, Clone, Copy)]
pub struct TypeScriptNextStack;

impl StackPack for TypeScriptNextStack {
    fn name(&self) -> &str {
        "typescript-next"
    }

    fn source_kind(&self, path: &Path) -> Option<SourceKind> {
        path.to_str().and_then(SourceKind::of)
    }

    fn classify(&self, rel_path: &str) -> FileRole {
        crate::lang::classify(rel_path)
    }

    fn extract_symbols(&self, rel_path: &str, source: &str) -> Vec<ExtractedSymbol> {
        crate::lang::extract_symbols(rel_path, source)
    }

    fn extract_imports(&self, rel_path: &str, source: &str) -> FileImports {
        crate::imports::extract_imports(source, rel_path)
    }

    fn resolve_imports(
        &self,
        root: &Path,
        file_imports: &HashMap<String, FileImports>,
        graph: &Graph,
    ) -> Vec<ResolvedImport> {
        let resolver = crate::resolve::build_resolver();
        crate::resolve::resolve_imports(root, &resolver, file_imports, graph)
    }

    fn extract_flow_edges(&self, rel_path: &str, source: &str) -> Vec<UnresolvedFlowEdge> {
        let mut out = Vec::new();
        for rl in crate::features::extract_route_literals(source, rel_path) {
            out.push(UnresolvedFlowEdge {
                from_file: rl.from_file,
                kind: "route-literal".to_string(),
                raw: rl.static_path,
            });
        }
        for tr in crate::features::extract_table_refs(source, rel_path) {
            out.push(UnresolvedFlowEdge {
                from_file: tr.from_file,
                kind: "table-ref".to_string(),
                raw: tr.table,
            });
        }
        for rc in crate::features::extract_rendered_components(source, rel_path) {
            out.push(UnresolvedFlowEdge {
                from_file: rc.from_file,
                kind: "rendered-component".to_string(),
                raw: rc.name,
            });
        }
        out
    }

    fn resolve_flow_edge(&self, graph: &Graph, edge: &UnresolvedFlowEdge) -> FlowTarget {
        let resolved = match edge.kind.as_str() {
            "route-literal" => crate::features::resolve_route_literal(graph, &edge.raw),
            "table-ref" => crate::features::resolve_table_ref(graph, &edge.raw),
            "rendered-component" => {
                crate::features::resolve_rendered_component(graph, &edge.from_file, &edge.raw)
                    .and_then(|file| graph.find(&file))
            }
            _ => None,
        };
        resolved.map_or(FlowTarget::Unresolved, FlowTarget::Node)
    }

    fn feature_model(&self) -> Option<&dyn crate::features::FeatureModel> {
        Some(crate::features::default_feature_model())
    }
}

/// The default pack as a trait object — the shape `RepoIndex` and the test
/// fixtures want. Phase 1 has exactly one pack, so this is unconditional;
/// M13's `detect()` is where the real choice will live.
pub fn typescript_next() -> Box<dyn StackPack> {
    Box::new(TypeScriptNextStack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn typescript_next_delegates_transparently() {
        let pack = TypeScriptNextStack;
        let src = "import { helper } from './b';\nexport function f() { helper(); }\n";

        // Each method must return exactly what the underlying free function
        // does — M13 is a re-wiring, so the seam has to be invisible.
        assert_eq!(
            pack.extract_symbols("a.ts", src),
            crate::lang::extract_symbols("a.ts", src)
        );
        assert_eq!(
            pack.extract_imports("a.ts", src),
            crate::imports::extract_imports(src, "a.ts")
        );
        assert_eq!(pack.classify("lib/x.ts"), crate::lang::classify("lib/x.ts"));
        assert_eq!(pack.classify("x.test.ts"), FileRole::Test);
        assert_eq!(
            pack.source_kind(Path::new("a/b.tsx")),
            Some(SourceKind::Code)
        );
        assert_eq!(
            pack.source_kind(Path::new("a/b.sql")),
            Some(SourceKind::Schema)
        );
        assert_eq!(pack.source_kind(Path::new("a/b.md")), None);
        assert_eq!(pack.name(), "typescript-next");
    }

    #[test]
    fn ts_pack_exposes_a_feature_model() {
        // The TS+Next stack has a feature layer; a `None`-returning pack
        // (M14/M16) exercises the trait default instead.
        assert!(TypeScriptNextStack.feature_model().is_some());
    }

    #[test]
    fn detect_returns_the_ts_pack_for_a_repo_with_source() {
        let dir = std::env::temp_dir().join(format!("codeowl-detect-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.ts"), "export const x = 1;\n").unwrap();
        assert_eq!(crate::lang::detect(&dir).unwrap().name(), "typescript-next");

        std::fs::remove_file(dir.join("a.ts")).unwrap();
        std::fs::write(dir.join("notes.md"), "hi\n").unwrap();
        assert!(crate::lang::detect(&dir).is_err());

        std::fs::remove_dir_all(&dir).ok();
    }
}
