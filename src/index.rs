//! Incremental repo indexing (M9).
//!
//! `RepoIndex` keeps the *per-file inputs* the graph is built from — one
//! file's extracted symbols, its named imports/re-exports, its route
//! literals, and a hash of its raw text — so rebuilding the graph after an
//! edit re-parses only the files that actually changed, never the whole
//! tree. It's the backbone of both moments `ARCHITECTURE.md`'s "Incremental
//! indexing" calls out: the fresh-spawn catch-up pass ([`RepoIndex::open`])
//! and the in-session file watcher ([`RepoIndex::apply_changes`], driven by
//! `watch.rs`).
//!
//! It's persisted next to the graph at `.codeowl/index`. The graph is fully
//! derivable from it — but persisting the raw inputs is exactly what lets a
//! fresh process skip re-parsing files that didn't change while nothing was
//! running.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::features::{RouteLiteral, extract_route_literals};
use crate::graph::{FileExtraction, Graph};
use crate::hash::hash_text;
use crate::imports::{FileImports, extract_imports};
use crate::resolve::{build_resolver, resolve_imports};
use crate::symbol::ExtractedSymbol;

/// Everything `Graph::build` plus import resolution needs from one file,
/// cached so an unchanged file is never re-parsed on a rebuild.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInputs {
    pub source_hash: String,
    pub symbols: Vec<ExtractedSymbol>,
    pub imports: FileImports,
    pub route_literals: Vec<RouteLiteral>,
}

impl FileInputs {
    /// The three tree-sitter passes (symbols, imports, route literals) plus
    /// the raw-text hash — everything that depends on a file's contents,
    /// recomputed together whenever that file changes.
    fn extract(rel_path: &str, source: &str) -> Self {
        Self {
            source_hash: hash_text(source),
            symbols: crate::extract::extract_file(source, rel_path),
            imports: extract_imports(source, rel_path),
            route_literals: extract_route_literals(source, rel_path),
        }
    }
}

/// What a rescan or an incremental update actually touched — the evidence
/// the M9 validation asks for ("reindexes exactly the changed files").
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CatchUp {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub removed: Vec<String>,
}

impl CatchUp {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.modified.is_empty() && self.removed.is_empty()
    }

    pub fn total(&self) -> usize {
        self.added.len() + self.modified.len() + self.removed.len()
    }

    fn sorted(mut self) -> Self {
        self.added.sort();
        self.modified.sort();
        self.removed.sort();
        self
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RepoIndex {
    /// Repo-relative path → its cached inputs. A `BTreeMap` so a rebuilt
    /// graph's node order is deterministic regardless of walk or
    /// filesystem-event order.
    files: BTreeMap<String, FileInputs>,
    /// The absolute repo root. Machine-specific, so it's never serialized —
    /// `load` sets it from the path it read the cache from.
    #[serde(skip)]
    root: PathBuf,
}

impl RepoIndex {
    fn index_path(root: &Path) -> PathBuf {
        root.join(".codeowl").join("index")
    }

    fn graph_path(root: &Path) -> PathBuf {
        root.join(".codeowl").join("graph")
    }

    /// Full walk and parse of every extractable file under `root` — the
    /// cold-start path, and the fallback whenever `.codeowl/index` is
    /// absent or unreadable.
    pub fn build(root: &Path) -> Result<Self> {
        let mut files = BTreeMap::new();
        for entry in ignore::WalkBuilder::new(root).build() {
            let entry = entry.context("walking repo")?;
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            if !is_extractable(path) {
                continue;
            }
            let source = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            let rel = rel_path(root, path);
            files.insert(rel.clone(), FileInputs::extract(&rel, &source));
        }
        Ok(Self {
            files,
            root: root.to_path_buf(),
        })
    }

    /// The fresh-spawn path (`ARCHITECTURE.md`, "Incremental indexing"):
    /// load the cached index, hash-check every file on disk, re-parse only
    /// what changed while no process was running, then rebuild and persist
    /// the graph. Falls back to a full [`build`](Self::build) when there's
    /// no usable cache — reported as an empty [`CatchUp`], since "nothing
    /// changed since last run" is the honest answer when there was no last
    /// run to diff against.
    pub fn open(root: &Path) -> Result<(Self, Graph, CatchUp)> {
        match Self::load(root) {
            Some(mut index) => {
                let caught = index.rescan()?;
                let graph = index.rebuild()?;
                Ok((index, graph, caught))
            }
            None => {
                let index = Self::build(root)?;
                let graph = index.rebuild()?;
                Ok((index, graph, CatchUp::default()))
            }
        }
    }

    fn load(root: &Path) -> Option<Self> {
        let file = std::fs::File::open(Self::index_path(root)).ok()?;
        let mut index: Self = serde_json::from_reader(std::io::BufReader::new(file)).ok()?;
        index.root = root.to_path_buf();
        Some(index)
    }

    /// Walk the tree, diff every extractable file against the cached hash,
    /// re-extract the ones that moved, drop the ones that are gone.
    fn rescan(&mut self) -> Result<CatchUp> {
        let mut seen = HashSet::new();
        let mut caught = CatchUp::default();
        for entry in ignore::WalkBuilder::new(&self.root).build() {
            let entry = entry.context("walking repo")?;
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            if !is_extractable(path) {
                continue;
            }
            let rel = rel_path(&self.root, path);
            seen.insert(rel.clone());
            let source = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            match self.files.get(&rel) {
                Some(existing) if existing.source_hash == hash_text(&source) => {}
                Some(_) => {
                    self.files
                        .insert(rel.clone(), FileInputs::extract(&rel, &source));
                    caught.modified.push(rel);
                }
                None => {
                    self.files
                        .insert(rel.clone(), FileInputs::extract(&rel, &source));
                    caught.added.push(rel);
                }
            }
        }
        let removed: Vec<String> = self
            .files
            .keys()
            .filter(|k| !seen.contains(*k))
            .cloned()
            .collect();
        for r in &removed {
            self.files.remove(r);
        }
        caught.removed = removed;
        Ok(caught.sorted())
    }

    /// Watcher-driven incremental update: `paths` are absolute paths the
    /// file watcher reported touched (created / modified / deleted). Returns
    /// `Some` only if at least one of them actually changed an input the
    /// graph is built from — an editor rewriting an identical buffer, or a
    /// touch of a non-source file, is a no-op that never rebuilds.
    pub fn apply_changes(&mut self, paths: &[PathBuf]) -> Result<Option<(Graph, CatchUp)>> {
        let mut caught = CatchUp::default();
        for abs in paths {
            if !is_extractable(abs) {
                continue;
            }
            let Ok(rel) = abs.strip_prefix(&self.root) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/");
            match std::fs::read_to_string(abs) {
                Ok(source) => match self.files.get(&rel) {
                    Some(existing) if existing.source_hash == hash_text(&source) => {}
                    Some(_) => {
                        self.files
                            .insert(rel.clone(), FileInputs::extract(&rel, &source));
                        caught.modified.push(rel);
                    }
                    None => {
                        self.files
                            .insert(rel.clone(), FileInputs::extract(&rel, &source));
                        caught.added.push(rel);
                    }
                },
                // Unreadable almost always means deleted (or renamed away).
                Err(_) => {
                    if self.files.remove(&rel).is_some() {
                        caught.removed.push(rel);
                    }
                }
            }
        }
        if caught.is_empty() {
            return Ok(None);
        }
        let graph = self.rebuild()?;
        Ok(Some((graph, caught.sorted())))
    }

    /// Rebuild the whole `Graph` from the current cached inputs and
    /// re-persist both `.codeowl/graph` and `.codeowl/index`. Cheap: arena
    /// construction plus import resolution, no parsing — every file's
    /// symbols and imports are already in hand.
    pub fn rebuild(&self) -> Result<Graph> {
        let extractions: Vec<FileExtraction> = self
            .files
            .iter()
            .map(|(rel, f)| FileExtraction {
                rel_path: rel.clone(),
                source_hash: f.source_hash.clone(),
                symbols: f.symbols.clone(),
            })
            .collect();

        let mut graph = Graph::build(extractions);
        let resolver = build_resolver();
        let file_imports: HashMap<String, FileImports> = self
            .files
            .iter()
            .map(|(rel, f)| (rel.clone(), f.imports.clone()))
            .collect();
        let resolved = resolve_imports(&self.root, &resolver, &file_imports, &graph);
        graph.set_resolved_imports(resolved);
        graph.set_route_literals(
            self.files
                .values()
                .flat_map(|f| f.route_literals.iter().cloned())
                .collect(),
        );

        graph.save(&Self::graph_path(&self.root))?;
        self.save()?;
        Ok(graph)
    }

    fn save(&self) -> Result<()> {
        let path = Self::index_path(&self.root);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let file =
            std::fs::File::create(&path).with_context(|| format!("creating {}", path.display()))?;
        serde_json::to_writer_pretty(std::io::BufWriter::new(file), self)
            .with_context(|| format!("writing {}", path.display()))
    }

    /// Every directory under the repo root that the file watcher should
    /// register a watch on — the gitignore-visible tree only, so a real
    /// Next.js repo's `node_modules` never gets watched. Kept here rather
    /// than in `watch.rs` so it shares one `ignore` walk configuration with
    /// everything else that traverses the repo.
    pub fn watchable_dirs(root: &Path) -> Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        for entry in ignore::WalkBuilder::new(root).build() {
            let entry = entry.context("walking repo to register watches")?;
            if entry.file_type().is_some_and(|t| t.is_dir()) {
                dirs.push(entry.path().to_path_buf());
            }
        }
        Ok(dirs)
    }
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// `.ts`/`.tsx` only, and never `.d.ts` — ambient declaration files use
/// different grammar shapes (`declare function`, etc.) that M1 doesn't
/// handle; see `ROADMAP.md`'s M1 scope. Public so `main.rs`, the catch-up
/// pass, and the file watcher all share exactly one definition of what
/// counts as source.
pub fn is_extractable(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if name.ends_with(".d.ts") {
        return false;
    }
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts") | Some("tsx")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, contents: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn tempdir(tag: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("codeowl-index-{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn hashes(graph: &Graph) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = graph
            .symbols()
            .map(|s| (s.id.clone(), s.source_hash.clone()))
            .chain(graph.files().map(|f| (f.id.clone(), f.source_hash.clone())))
            .collect();
        out.sort();
        out
    }

    #[test]
    fn open_with_no_cache_does_a_full_build() {
        let dir = tempdir("cold");
        write(&dir, "a.ts", "export const a = 1;\n");
        write(&dir, "b.ts", "export const b = 2;\n");

        let (_index, graph, caught) = RepoIndex::open(&dir).unwrap();

        assert!(caught.is_empty(), "no prior run to diff against");
        assert!(graph.find("a.ts::a").is_some());
        assert!(graph.find("b.ts::b").is_some());
        assert!(
            RepoIndex::index_path(&dir).exists(),
            "cache persisted for next spawn"
        );
    }

    #[test]
    fn catch_up_reindexes_exactly_the_changed_files() {
        let dir = tempdir("catchup");
        write(&dir, "a.ts", "export const a = 1;\n");
        write(&dir, "b.ts", "export const b = 2;\n");
        write(&dir, "c.ts", "export const c = 3;\n");

        // First spawn: establishes the cache.
        RepoIndex::open(&dir).unwrap();

        // Edits while "no process is running": modify b, add d, delete c.
        write(&dir, "b.ts", "export const b = 22;\n");
        write(&dir, "d.ts", "export const d = 4;\n");
        std::fs::remove_file(dir.join("c.ts")).unwrap();

        let (_index, graph, caught) = RepoIndex::open(&dir).unwrap();

        assert_eq!(caught.modified, vec!["b.ts"]);
        assert_eq!(caught.added, vec!["d.ts"]);
        assert_eq!(caught.removed, vec!["c.ts"]);

        assert!(graph.find("d.ts::d").is_some());
        assert!(graph.find("c.ts::c").is_none());

        // The incrementally-rebuilt graph is byte-for-byte what a full
        // cold build of the same on-disk state produces.
        let fresh = RepoIndex::build(&dir).unwrap().rebuild().unwrap();
        assert_eq!(hashes(&graph), hashes(&fresh));
    }

    #[test]
    fn catch_up_with_no_edits_touches_nothing() {
        let dir = tempdir("noop");
        write(&dir, "a.ts", "export const a = 1;\n");
        RepoIndex::open(&dir).unwrap();

        let (_index, _graph, caught) = RepoIndex::open(&dir).unwrap();
        assert!(caught.is_empty());
    }

    #[test]
    fn apply_changes_ignores_a_write_that_did_not_change_content() {
        let dir = tempdir("apply-noop");
        write(&dir, "a.ts", "export const a = 1;\n");
        let (mut index, _graph, _) = RepoIndex::open(&dir).unwrap();

        // Rewrite identical bytes — what an editor's save-on-no-change does.
        write(&dir, "a.ts", "export const a = 1;\n");
        let result = index.apply_changes(&[dir.join("a.ts")]).unwrap();
        assert!(result.is_none(), "identical content must not rebuild");
    }

    #[test]
    fn apply_changes_rebuilds_and_reflects_the_edit() {
        let dir = tempdir("apply-edit");
        write(&dir, "a.ts", "export function f() { return 1; }\n");
        let (mut index, graph, _) = RepoIndex::open(&dir).unwrap();
        let before = graph
            .get_symbol(graph.find("a.ts::f").unwrap())
            .unwrap()
            .source_hash
            .clone();

        write(&dir, "a.ts", "export function f() { return 2; }\n");
        let (graph, caught) = index
            .apply_changes(&[dir.join("a.ts")])
            .unwrap()
            .expect("a real edit rebuilds");

        assert_eq!(caught.modified, vec!["a.ts"]);
        let after = graph
            .get_symbol(graph.find("a.ts::f").unwrap())
            .unwrap()
            .source_hash
            .clone();
        assert_ne!(before, after);
    }

    #[test]
    fn apply_changes_resolves_a_newly_added_importer() {
        let dir = tempdir("apply-add");
        write(&dir, "lib.ts", "export function helper() {}\n");
        let (mut index, _graph, _) = RepoIndex::open(&dir).unwrap();

        write(&dir, "app.ts", "import { helper } from './lib';\n");
        let (graph, _) = index
            .apply_changes(&[dir.join("app.ts")])
            .unwrap()
            .expect("a new file rebuilds");

        let target = graph.find("lib.ts::helper").unwrap();
        assert!(
            graph.imports().iter().any(|i| i.target == Some(target)),
            "the new importer's edge resolves against the existing graph"
        );
    }
}
