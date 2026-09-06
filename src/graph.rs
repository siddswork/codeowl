//! The arena that turns per-file extraction output into something
//! indexable and cross-referenceable.
//!
//! `extract.rs` doesn't know about any of this: it only ever sees one file
//! at a time and produces `ExtractedSymbol`s with string `parent`/
//! `children` (see `symbol.rs`). `Graph::build` is what turns a whole
//! repo's worth of those, plus one content hash per file, into the real
//! arena: every file becomes a `FileNode`, every symbol becomes a `Symbol`
//! with `SymbolId`-typed containment, and a top-level symbol's parent
//! becomes the file it lives in. This is deliberately the *only* place a
//! string id gets turned into a `SymbolId` for containment — per
//! `CLAUDE.md`'s Rust conventions, everything downstream (resolution, hash
//! propagation, MCP responses) references nodes by `SymbolId`, never by
//! re-deriving or comparing id strings, and never leaks a `SymbolId`
//! outside the process that produced it (see `SymbolId`'s own doc comment).

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::extract::extract_file;
use crate::hash::hash_text;
use crate::resolve::ResolvedImport;
pub use crate::symbol::SymbolId;
use crate::symbol::{ExtractedSymbol, Symbol, SymbolKind};

/// The external-safe shape of a `Symbol` — same fields, but `parent`/
/// `children` are translated back to stable string ids rather than the
/// internal `SymbolId`s `Symbol` itself carries. `SymbolId` is only valid
/// for the `Graph` that produced it (see its own doc comment), so it must
/// never appear in output a caller might hold onto past this process's
/// lifetime — every place a `Symbol` crosses that boundary (an MCP
/// response, `codeowl extract`'s JSON, a spec's frontmatter) should build
/// one of these instead of serializing a `Symbol` directly.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct SymbolView {
    pub id: String,
    pub kind: SymbolKind,
    pub file: String,
    pub lines: [usize; 2],
    pub signature: String,
    pub docstring: Option<String>,
    pub is_exported: bool,
    pub source_hash: String,
    pub interface_hash: Option<String>,
    pub parent: Option<String>,
    pub children: Vec<String>,
}

impl SymbolView {
    pub fn from_graph(graph: &Graph, id: SymbolId) -> Option<Self> {
        let s = graph.get_symbol(id)?;
        Some(Self {
            id: s.id.clone(),
            kind: s.kind,
            file: s.file.clone(),
            lines: s.lines,
            signature: s.signature.clone(),
            docstring: s.docstring.clone(),
            is_exported: s.is_exported,
            source_hash: s.source_hash.clone(),
            interface_hash: s.interface_hash.clone(),
            parent: s.parent.map(|p| graph.string_id(p).to_string()),
            children: s
                .children
                .iter()
                .map(|c| graph.string_id(*c).to_string())
                .collect(),
        })
    }
}

/// A file's own arena node. `id` is its repo-relative path (the same
/// string scheme `Symbol::file` uses); `source_hash` is a hash of the
/// file's raw text — deliberately *not* a rollup of children's hashes like
/// a `Class`'s is, since a file's spec-relevant content (imports, types,
/// comments) isn't fully captured by its declared symbols alone, and a
/// whole-file hash is simpler and strictly more sensitive. `children` are
/// its top-level symbols, in declaration order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileNode {
    pub id: String,
    pub source_hash: String,
    pub children: Vec<SymbolId>,
}

/// One arena entry — either kind of node the containment tree can hold.
/// `Graph::build` is the only place these get constructed; everything else
/// reads them back out via `get_symbol`/`get_file` (or `get` when the kind
/// genuinely doesn't matter, e.g. hashing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Node {
    Symbol(Symbol),
    File(FileNode),
}

impl Node {
    fn string_id(&self) -> &str {
        match self {
            Node::Symbol(s) => &s.id,
            Node::File(f) => &f.id,
        }
    }
}

/// One walked file's extraction output, plus the hash of its raw text —
/// everything `Graph::build` needs to turn a file into arena nodes.
pub struct FileExtraction {
    pub rel_path: String,
    pub source_hash: String,
    pub symbols: Vec<ExtractedSymbol>,
}

/// Extract `source` (already read from `rel_path`) and hash it in one
/// step — the shape `main.rs`'s repo walk and every multi-file test fixture
/// in this codebase wants, so it's a real (non-test-only) helper rather
/// than duplicated per call site.
pub fn extract_and_hash(rel_path: &str, source: &str) -> FileExtraction {
    FileExtraction {
        rel_path: rel_path.to_string(),
        source_hash: hash_text(source),
        symbols: extract_file(source, rel_path),
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Graph {
    nodes: Vec<Node>,
    by_id: HashMap<String, SymbolId>,
    /// File-to-file reference edges — see `resolve.rs`. Empty until
    /// `set_resolved_imports` is called; resolving them needs a `Graph` to
    /// look symbols up in, so they can't be known at `build` time.
    imports: Vec<ResolvedImport>,
    /// Every `fetch("/api/...")` call site found across the repo — see
    /// `features.rs`. Stored here (rather than re-walked on every
    /// `get_next_spec_task` call) the same way `imports` is: computed once
    /// at graph-build time, persisted with everything else.
    route_literals: Vec<crate::features::RouteLiteral>,
}

impl Graph {
    /// Build a `Graph` from every file walked across a repo: each file
    /// becomes a `FileNode`, each of its symbols becomes a `Symbol`, and
    /// every string `parent`/`children` reference — including a top-level
    /// symbol's implicit membership in its file — is resolved to a
    /// `SymbolId`. Two passes: first reserve one arena slot per node (a
    /// file, then each of its symbols, in order) so every string id has a
    /// `SymbolId` to resolve to; then build the actual nodes now that any
    /// node may need to reference any other by id.
    pub fn build(files: Vec<FileExtraction>) -> Self {
        let mut by_id = HashMap::new();
        let mut next = 0u32;
        for file in &files {
            by_id.insert(file.rel_path.clone(), SymbolId::new(next));
            next += 1;
            for sym in &file.symbols {
                by_id.insert(sym.id.clone(), SymbolId::new(next));
                next += 1;
            }
        }

        let mut nodes = Vec::with_capacity(next as usize);
        for file in files {
            let file_id = by_id[&file.rel_path];
            let mut children = Vec::new();
            let mut symbol_nodes = Vec::with_capacity(file.symbols.len());
            for sym in file.symbols {
                let id = by_id[&sym.id];
                let parent = match &sym.parent {
                    // Top-level in its file (no containing symbol) — its
                    // parent is the file itself.
                    None => {
                        children.push(id);
                        Some(file_id)
                    }
                    Some(parent_str) => by_id.get(parent_str).copied(),
                };
                symbol_nodes.push(Node::Symbol(Symbol {
                    id: sym.id,
                    kind: sym.kind,
                    file: sym.file,
                    lines: sym.lines,
                    signature: sym.signature,
                    docstring: sym.docstring,
                    is_exported: sym.is_exported,
                    source_hash: sym.source_hash,
                    interface_hash: sym.interface_hash,
                    parent,
                    children: sym
                        .children
                        .iter()
                        .filter_map(|c| by_id.get(c).copied())
                        .collect(),
                }));
            }
            nodes.push(Node::File(FileNode {
                id: file.rel_path,
                source_hash: file.source_hash,
                children,
            }));
            nodes.extend(symbol_nodes);
        }

        Self {
            nodes,
            by_id,
            imports: Vec::new(),
            route_literals: Vec::new(),
        }
    }

    pub fn get(&self, id: SymbolId) -> &Node {
        &self.nodes[id.index()]
    }

    pub fn get_symbol(&self, id: SymbolId) -> Option<&Symbol> {
        match self.get(id) {
            Node::Symbol(s) => Some(s),
            Node::File(_) => None,
        }
    }

    pub fn get_file(&self, id: SymbolId) -> Option<&FileNode> {
        match self.get(id) {
            Node::File(f) => Some(f),
            Node::Symbol(_) => None,
        }
    }

    /// The stable string id a `SymbolId` resolves to — how internal
    /// `SymbolId`-typed containment gets translated back to something safe
    /// to hand to an MCP caller (see `SymbolId`'s doc comment: it must
    /// never leak outside the process that produced it).
    pub fn string_id(&self, id: SymbolId) -> &str {
        self.get(id).string_id()
    }

    /// Look up a node by its stable string id (`"<file>::<name>"`, or a
    /// bare repo-relative path for a file).
    pub fn find(&self, string_id: &str) -> Option<SymbolId> {
        self.by_id.get(string_id).copied()
    }

    /// The arena id of `id`'s containment parent, if it has one. Only
    /// symbols carry a parent for now — a `FileNode`'s parent would be a
    /// directory node, which doesn't exist yet (see `ROADMAP.md`'s M4
    /// scope note on directories).
    pub fn parent_id(&self, id: SymbolId) -> Option<SymbolId> {
        self.get_symbol(id).and_then(|s| s.parent)
    }

    /// `id`'s containment children — a symbol's nested members, or a
    /// file's top-level symbols.
    pub fn children_ids(&self, id: SymbolId) -> &[SymbolId] {
        match self.get(id) {
            Node::Symbol(s) => &s.children,
            Node::File(f) => &f.children,
        }
    }

    /// Every extracted symbol in the arena (not files) — what `codeowl
    /// extract` prints and what M1–M3's tests were written against.
    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.nodes.iter().filter_map(|n| match n {
            Node::Symbol(s) => Some(s),
            Node::File(_) => None,
        })
    }

    /// Every file in the arena.
    pub fn files(&self) -> impl Iterator<Item = &FileNode> {
        self.nodes.iter().filter_map(|n| match n {
            Node::File(f) => Some(f),
            Node::Symbol(_) => None,
        })
    }

    pub fn imports(&self) -> &[ResolvedImport] {
        &self.imports
    }

    pub fn set_resolved_imports(&mut self, imports: Vec<ResolvedImport>) {
        self.imports = imports;
    }

    pub fn route_literals(&self) -> &[crate::features::RouteLiteral] {
        &self.route_literals
    }

    pub fn set_route_literals(&mut self, route_literals: Vec<crate::features::RouteLiteral>) {
        self.route_literals = route_literals;
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Write the whole arena to `path` (`.codeowl/graph` in the target
    /// repo — see `ARCHITECTURE.md`'s "Storage") as JSON. Plain JSON, not
    /// bincode: Phase 1 is personal/local-only, and being able to `cat`/
    /// `jq` the cache while learning Rust is worth more than bincode's
    /// speed at this scale.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let file =
            std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
        serde_json::to_writer_pretty(file, self)
            .with_context(|| format!("writing {}", path.display()))
    }

    pub fn load(path: &Path) -> Result<Self> {
        let file =
            std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
        serde_json::from_reader(file).with_context(|| format!("parsing {}", path.display()))
    }
}

/// Build a `Graph` straight from `(rel_path, source)` pairs — the shape
/// every test fixture in this codebase (and `main.rs`'s repo walk) starts
/// from. Not `#[cfg(test)]`: `main.rs` uses it too.
pub fn build_graph_from_sources(files: &[(&str, &str)]) -> Graph {
    let extractions = files
        .iter()
        .map(|(rel, src)| extract_and_hash(rel, src))
        .collect();
    Graph::build(extractions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_resolves_a_known_symbol_to_its_id() {
        let graph = build_graph_from_sources(&[("a.ts", "export function double(x: number) {}\n")]);
        let id = graph.find("a.ts::double").expect("should be found");
        assert_eq!(graph.get_symbol(id).unwrap().id, "a.ts::double");
    }

    #[test]
    fn find_returns_none_for_an_unknown_id() {
        let graph = Graph::build(Vec::new());
        assert_eq!(graph.find("a.ts::nope"), None);
    }

    #[test]
    fn top_level_symbol_parent_is_its_file() {
        let graph = build_graph_from_sources(&[("a.ts", "export function double(x: number) {}\n")]);
        let file_id = graph.find("a.ts").expect("file node should exist");
        let sym_id = graph.find("a.ts::double").unwrap();

        assert_eq!(graph.parent_id(sym_id), Some(file_id));
        assert_eq!(graph.children_ids(file_id), &[sym_id]);
    }

    #[test]
    fn parent_id_walks_a_method_up_to_its_class() {
        let graph =
            build_graph_from_sources(&[("a.ts", "export class Foo {\n    bar(): void {}\n}\n")]);
        let class_id = graph.find("a.ts::Foo").unwrap();
        let method_id = graph.find("a.ts::Foo.bar").unwrap();
        let file_id = graph.find("a.ts").unwrap();

        assert_eq!(graph.parent_id(method_id), Some(class_id));
        // The class itself is top-level -- its parent is the file, not None.
        assert_eq!(graph.parent_id(class_id), Some(file_id));
    }

    #[test]
    fn save_then_load_round_trips_the_whole_graph() {
        let graph = build_graph_from_sources(&[("a.ts", "export function double(x: number) {}\n")]);

        let dir = std::env::temp_dir().join(format!("codeowl-graph-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("graph");

        graph.save(&path).expect("save should succeed");
        let loaded = Graph::load(&path).expect("load should succeed");

        assert_eq!(loaded.len(), graph.len());
        let id = loaded
            .find("a.ts::double")
            .expect("id should survive round trip");
        assert_eq!(
            loaded.get_symbol(id),
            graph.get_symbol(graph.find("a.ts::double").unwrap())
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn from_symbols_across_multiple_files_indexes_all_of_them() {
        let graph = build_graph_from_sources(&[
            ("a.ts", "export const a = 1;\n"),
            ("b.ts", "export const b = 2;\n"),
        ]);

        assert_eq!(graph.symbols().count(), 2);
        assert!(graph.find("a.ts::a").is_some());
        assert!(graph.find("b.ts::b").is_some());
        assert!(graph.find("a.ts").is_some());
        assert!(graph.find("b.ts").is_some());
    }
}
