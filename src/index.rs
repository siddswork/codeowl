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

use crate::graph::{FileExtraction, FlowEdge, Graph, UnresolvedFlowEdge};
use crate::hash::hash_text;
use crate::imports::FileImports;
use crate::lang::SourceKind;
use crate::stack::StackPack;
use crate::symbol::ExtractedSymbol;

/// Everything `Graph::build` plus import resolution needs from one file,
/// cached so an unchanged file is never re-parsed on a rebuild.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInputs {
    pub source_hash: String,
    pub symbols: Vec<ExtractedSymbol>,
    pub imports: FileImports,
    /// Unresolved flow edges from `pack.extract_flow_edges` — resolved
    /// against the whole graph in `rebuild`.
    #[serde(default)]
    pub flow_edges: Vec<UnresolvedFlowEdge>,
}

impl FileInputs {
    /// Everything that depends on a file's contents, recomputed together
    /// whenever that file changes. A schema file gets only the table pass
    /// (folded into `pack.extract_symbols`); the pack's import and
    /// flow-edge passes are skipped for it.
    fn extract(pack: &dyn StackPack, rel_path: &str, source: &str) -> Self {
        let source_hash = hash_text(source);
        let symbols = pack.extract_symbols(rel_path, source);
        match pack.source_kind(Path::new(rel_path)) {
            Some(SourceKind::Schema) => Self {
                source_hash,
                symbols,
                imports: FileImports::default(),
                flow_edges: Vec::new(),
            },
            Some(SourceKind::Code) | None => Self {
                source_hash,
                symbols,
                imports: pack.extract_imports(rel_path, source),
                flow_edges: pack.extract_flow_edges(rel_path, source),
            },
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

#[derive(Debug, Serialize, Deserialize)]
pub struct RepoIndex {
    /// On-disk format stamp, shared with `.codeowl/graph` — see
    /// [`crate::graph::FORMAT_VERSION`]. `#[serde(default)]` so a cache
    /// written before the stamp existed deserializes to 0; `load` then sees
    /// the mismatch and forces a full rebuild rather than trusting inputs
    /// that a newer `#[serde(default)]` field would read empty from.
    #[serde(default)]
    format_version: u32,
    /// The `StackPack::name()` this cache was built with. If `detect`
    /// picks a different pack now — the repo's stack changed — the cached
    /// `FileInputs` were parsed by the wrong grammar, so `load` discards
    /// them (M13 design decision 7). `#[serde(default)]` empty on a
    /// pre-M14 cache, which `format_version` already rejects.
    #[serde(default)]
    pack_name: String,
    /// Repo-relative path → its cached inputs. A `BTreeMap` so a rebuilt
    /// graph's node order is deterministic regardless of walk or
    /// filesystem-event order.
    files: BTreeMap<String, FileInputs>,
    /// The absolute repo root. Machine-specific, so it's never serialized —
    /// `load` sets it from the path it read the cache from.
    #[serde(skip)]
    root: PathBuf,
    /// The stack pack for this repo — chosen by `lang::detect` at
    /// `build`/`open`, never serialized (it's derived from the repo, and
    /// M13 design decision 7 keys the cache on `pack.name()` so a pack
    /// change forces a rebuild anyway). `Box`ed rather than a generic
    /// param so `RepoIndex` and `watch.rs` stay concrete types.
    #[serde(skip, default = "crate::stack::typescript_next")]
    pack: Box<dyn StackPack>,
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
    /// absent or unreadable. Picks the repo's `StackPack` here (fail-fast
    /// if none recognises it).
    pub fn build(root: &Path) -> Result<Self> {
        let pack = crate::lang::detect(root)?;
        let mut files = BTreeMap::new();
        for entry in ignore::WalkBuilder::new(root).build() {
            let entry = entry.context("walking repo")?;
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            if pack.source_kind(path).is_none() {
                continue;
            }
            let source = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            let rel = rel_path(root, path);
            files.insert(
                rel.clone(),
                FileInputs::extract(pack.as_ref(), &rel, &source),
            );
        }
        Ok(Self {
            format_version: crate::graph::FORMAT_VERSION,
            pack_name: pack.name().to_string(),
            files,
            root: root.to_path_buf(),
            pack,
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
        // A cache from a different on-disk format — including one written
        // before the stamp existed, which deserializes to 0 — is not
        // partially reusable: its `FileInputs` may be missing a field a
        // newer graph build now depends on, and every hash would still
        // match, so `rescan` would re-extract nothing. Discard it; `open`
        // then does a full `build`.
        if index.format_version != crate::graph::FORMAT_VERSION {
            return None;
        }
        // `pack` and `root` are `#[serde(skip)]` — re-derive them from the
        // repo. `detect` also fail-fasts if the repo lost all its source
        // while nothing was running.
        let pack = crate::lang::detect(root).ok()?;
        // The repo's stack changed since this cache was written — its
        // `FileInputs` were parsed by the wrong grammar (design decision 7).
        if index.pack_name != pack.name() {
            return None;
        }
        index.pack = pack;
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
            if self.pack.source_kind(path).is_none() {
                continue;
            }
            let rel = rel_path(&self.root, path);
            seen.insert(rel.clone());
            let source = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            match self.files.get(&rel) {
                Some(existing) if existing.source_hash == hash_text(&source) => {}
                Some(_) => {
                    self.files.insert(
                        rel.clone(),
                        FileInputs::extract(self.pack.as_ref(), &rel, &source),
                    );
                    caught.modified.push(rel);
                }
                None => {
                    self.files.insert(
                        rel.clone(),
                        FileInputs::extract(self.pack.as_ref(), &rel, &source),
                    );
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
            if self.pack.source_kind(abs).is_none() {
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
                        self.files.insert(
                            rel.clone(),
                            FileInputs::extract(self.pack.as_ref(), &rel, &source),
                        );
                        caught.modified.push(rel);
                    }
                    None => {
                        self.files.insert(
                            rel.clone(),
                            FileInputs::extract(self.pack.as_ref(), &rel, &source),
                        );
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
        graph.set_pack_name(self.pack.name());
        let file_imports: HashMap<String, FileImports> = self
            .files
            .iter()
            .map(|(rel, f)| (rel.clone(), f.imports.clone()))
            .collect();
        let resolved = self.pack.resolve_imports(&self.root, &file_imports, &graph);
        graph.set_resolved_imports(resolved);
        // Still a free-function call: `resolved_default_imports` is
        // import-resolution plumbing for the rendered-component flow edge,
        // not a flow edge itself — folds into `pack.resolve_imports`'s
        // output in M18.
        graph.set_resolved_default_imports(crate::resolve::resolve_default_imports(
            &self.root,
            &crate::resolve::build_resolver(),
            &file_imports,
        ));

        // Resolve every file's unresolved flow edges against the whole
        // graph now that imports (and default imports) are in place.
        let flow_edges: Vec<FlowEdge> = self
            .files
            .values()
            .flat_map(|f| f.flow_edges.iter())
            .map(|e| FlowEdge {
                from_file: e.from_file.clone(),
                kind: e.kind.clone(),
                raw: e.raw.clone(),
                target: self.pack.resolve_flow_edge(&graph, e),
            })
            .collect();
        graph.set_flow_edges(flow_edges);

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
    fn open_rebuilds_when_the_cache_predates_the_current_format_version() {
        // The latent M11 bug: when a field is added to a persisted struct
        // behind `#[serde(default)]`, an older `.codeowl/` cache still
        // deserializes cleanly — the new field just reads empty — and
        // `rescan` sees every file's hash unchanged, so nothing is
        // re-extracted. The graph is then rebuilt from inputs that are
        // missing whatever the new field feeds. A `format_version` stamp is
        // the fix: a cache without the current version is not reused at all.
        let dir = tempdir("stale-format");
        let source = "export function real() {}\n";
        write(&dir, "a.ts", source);

        // A hand-written `.codeowl/index` in the *old* on-disk shape: valid
        // JSON, `source_hash` matches what's on disk (so `rescan` treats
        // a.ts as unchanged and skips re-extraction), but `symbols` is stale
        // and empty and there is no `format_version` key.
        let stale = serde_json::json!({
            "files": {
                "a.ts": {
                    "source_hash": hash_text(source),
                    "symbols": [],
                    "imports": { "imports": [], "re_exports": [] },
                    "route_literals": []
                }
            }
        });
        let index_path = RepoIndex::index_path(&dir);
        std::fs::create_dir_all(index_path.parent().unwrap()).unwrap();
        std::fs::write(&index_path, serde_json::to_string_pretty(&stale).unwrap()).unwrap();

        let (_index, graph, _caught) = RepoIndex::open(&dir).unwrap();

        assert!(
            graph.find("a.ts::real").is_some(),
            "a cache with no format_version must force a full rebuild, not \
             silent reuse of its stale inputs"
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
