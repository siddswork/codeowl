//! Ties `imports.rs` (what each file imports) to `graph.rs` (what symbols
//! exist) via `oxc_resolver` (what file a specifier actually points at),
//! producing the file-to-file reference edges M2 exists to build.
//!
//! A specifier resolves in two steps: `oxc_resolver` turns `'./foo'` or
//! `'@/lib/foo'` into an absolute file path (handling relative imports,
//! `tsconfig.json` path aliases, and `node_modules` lookups the same way
//! Node/TypeScript's own resolution does); then we look for a symbol named
//! `<name>` declared in that file. If it isn't declared there directly, the
//! target file might be a barrel forwarding it via a named re-export (see
//! `imports.rs`) — we chase through those, depth-capped, until we find a
//! real declaration or run out of re-exports to follow.

use std::collections::HashMap;
use std::path::Path;

use oxc_resolver::{ResolveOptions, Resolver, TsconfigDiscovery};
use serde::{Deserialize, Serialize};

use crate::graph::{Graph, SymbolId};
use crate::imports::FileImports;

/// Chasing `export { X } from './barrel-of-barrels'` chains shouldn't be
/// able to loop forever on a circular re-export — 5 hops is generous for
/// any real barrel structure and cheap to bound.
const MAX_REEXPORT_HOPS: u8 = 5;

/// One resolved (or unresolved) file-to-file reference edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedImport {
    pub from_file: String,
    pub specifier: String,
    pub imported_name: String,
    /// `None` when the specifier resolves outside the walked repo (an
    /// external package), doesn't resolve at all (a broken import), or
    /// resolves to a file with no declaration or re-export chain ending in
    /// `imported_name`.
    pub target: Option<SymbolId>,
}

/// Build a resolver configured for this project: TypeScript-first
/// extensions, and `tsconfig.json` path-alias discovery turned on (this is
/// what makes `@/lib/...`-style aliases resolve without CodeOwl having to
/// locate and parse `tsconfig.json` itself).
pub fn build_resolver() -> Resolver {
    Resolver::new(ResolveOptions {
        tsconfig: Some(TsconfigDiscovery::Auto),
        extensions: vec![
            ".ts".into(),
            ".tsx".into(),
            ".d.ts".into(),
            ".js".into(),
            ".jsx".into(),
            ".json".into(),
        ],
        ..ResolveOptions::default()
    })
}

/// Resolve every tracked import across the repo. `file_imports` must have
/// one entry per walked file (its `imports`/`re_exports`, from
/// `imports::extract_imports`), keyed by the same repo-relative path
/// scheme `Symbol::file` uses.
pub fn resolve_imports(
    repo_root: &Path,
    resolver: &Resolver,
    file_imports: &HashMap<String, FileImports>,
    graph: &Graph,
) -> Vec<ResolvedImport> {
    let ctx = ResolveCtx {
        repo_root,
        resolver,
        file_imports,
        graph,
    };
    let mut out = Vec::new();
    for (from_file, fi) in file_imports {
        for imp in &fi.imports {
            let target = ctx.resolve_named(
                from_file,
                &imp.specifier,
                &imp.imported_name,
                MAX_REEXPORT_HOPS,
            );
            out.push(ResolvedImport {
                from_file: from_file.clone(),
                specifier: imp.specifier.clone(),
                imported_name: imp.imported_name.clone(),
                target,
            });
        }
    }
    out
}

/// The read-only context every resolution step needs — bundled into one
/// struct (rather than four parameters threaded through each recursive
/// call) at clippy's `too_many_arguments` prompting.
struct ResolveCtx<'a> {
    repo_root: &'a Path,
    resolver: &'a Resolver,
    file_imports: &'a HashMap<String, FileImports>,
    graph: &'a Graph,
}

impl ResolveCtx<'_> {
    fn resolve_named(
        &self,
        from_file: &str,
        specifier: &str,
        name: &str,
        hops_left: u8,
    ) -> Option<SymbolId> {
        let target_file = self.resolve_specifier_to_rel_path(from_file, specifier)?;

        if let Some(id) = self.graph.find(&format!("{target_file}::{name}")) {
            return Some(id);
        }

        // Not declared there directly — maybe it's forwarded via a named
        // re-export (a barrel). Depth-capped so a circular barrel chain
        // can't recurse forever.
        let hops_left = hops_left.checked_sub(1)?;
        let re_export = self
            .file_imports
            .get(&target_file)?
            .re_exports
            .iter()
            .find(|r| r.exported_as == name)?;
        self.resolve_named(
            &target_file,
            &re_export.specifier,
            &re_export.source_name,
            hops_left,
        )
    }

    /// Resolve `specifier`, written inside `from_file`, to a repo-relative
    /// path using the same `<forward-slash-normalized>` scheme `main.rs`
    /// builds `Symbol::file` with. `None` if it resolves outside
    /// `repo_root` (external packages) or doesn't resolve at all.
    fn resolve_specifier_to_rel_path(&self, from_file: &str, specifier: &str) -> Option<String> {
        let abs_from = self.repo_root.join(from_file);
        let resolution = self.resolver.resolve_file(&abs_from, specifier).ok()?;
        let rel = resolution.path().strip_prefix(self.repo_root).ok()?;
        Some(rel.to_string_lossy().replace('\\', "/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_file;
    use crate::imports::extract_imports;

    /// Write a small fixture repo to a fresh temp dir, extract symbols +
    /// imports from every file the same way `main.rs` would, and resolve.
    /// Returns `(resolved edges, graph)` — tests look up expected targets
    /// in the graph rather than hardcoding `SymbolId` values, since those
    /// are only meaningful relative to this run's `Graph`.
    fn resolve_fixture(files: &[(&str, &str)]) -> (Vec<ResolvedImport>, Graph) {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-resolve-test-{}-{}",
            std::process::id(),
            files.len() * 7919 + files[0].0.len() // cheap per-test uniqueness
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let mut all_symbols = Vec::new();
        let mut file_imports = HashMap::new();
        for (rel, content) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            all_symbols.extend(extract_file(content, rel));
            file_imports.insert(rel.to_string(), extract_imports(content, rel));
        }

        let graph = Graph::from_symbols(all_symbols);
        let resolver = build_resolver();
        let resolved = resolve_imports(&dir, &resolver, &file_imports, &graph);

        std::fs::remove_dir_all(&dir).ok();
        (resolved, graph)
    }

    #[test]
    fn relative_import_resolves_to_target_symbol() {
        let (resolved, graph) = resolve_fixture(&[
            ("a.ts", "import { helper } from './b';\n"),
            ("b.ts", "export function helper(): void {}\n"),
        ]);
        let edge = resolved
            .iter()
            .find(|r| r.from_file == "a.ts" && r.imported_name == "helper")
            .unwrap();
        assert_eq!(edge.target, graph.find("b.ts::helper"));
        assert!(edge.target.is_some());
    }

    #[test]
    fn path_alias_resolves_via_tsconfig() {
        let (resolved, graph) = resolve_fixture(&[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "baseUrl": ".", "paths": { "@/*": ["*"] } } }"#,
            ),
            ("src/a.ts", "import { util } from '@/lib/util';\n"),
            ("lib/util.ts", "export function util(): void {}\n"),
        ]);
        let edge = resolved
            .iter()
            .find(|r| r.from_file == "src/a.ts" && r.imported_name == "util")
            .unwrap();
        assert_eq!(edge.target, graph.find("lib/util.ts::util"));
        assert!(edge.target.is_some());
    }

    #[test]
    fn barrel_reexport_chases_to_the_underlying_symbol() {
        let (resolved, graph) = resolve_fixture(&[
            ("a.ts", "import { Foo } from './barrel';\n"),
            ("barrel.ts", "export { Foo } from './real';\n"),
            ("real.ts", "export function Foo(): void {}\n"),
        ]);
        let edge = resolved
            .iter()
            .find(|r| r.from_file == "a.ts" && r.imported_name == "Foo")
            .unwrap();
        assert_eq!(edge.target, graph.find("real.ts::Foo"));
        assert!(edge.target.is_some());
    }

    #[test]
    fn external_package_import_is_unresolved_not_an_error() {
        let (resolved, _graph) =
            resolve_fixture(&[("a.ts", "import { z } from 'some-external-package';\n")]);
        let edge = &resolved[0];
        assert_eq!(edge.target, None);
    }
}
