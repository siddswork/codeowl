//! The arena that turns a flat `Vec<Symbol>` (what `extract_file` produces,
//! one file at a time) into something indexable and, once the resolver
//! lands, linkable across files.
//!
//! `extract.rs` doesn't change: it still produces `Symbol`s with string
//! `id`/`parent`/`children`, exactly as in M1. `Graph` is a thin index over
//! the concatenated result — the arena itself *is* the `Vec<Symbol>`
//! (`SymbolId` is just a position in it), plus a `HashMap` from each
//! symbol's stable string id to that position, built once after every file
//! has been walked. This is deliberately the *only* place a string id gets
//! turned into a `SymbolId`: per `CLAUDE.md`'s Rust conventions, everything
//! downstream (resolution, hash propagation) should reference symbols by
//! `SymbolId`, never by re-deriving or comparing id strings.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::resolve::ResolvedImport;
use crate::symbol::Symbol;

/// A `Symbol`'s position in a `Graph`'s arena. Only meaningful relative to
/// the `Graph` that produced it — not stable across a re-extraction, since
/// nothing pins a given symbol to the same index next time (that stability
/// is what `Symbol::id`, the string, is for).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SymbolId(u32);

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Graph {
    symbols: Vec<Symbol>,
    by_id: HashMap<String, SymbolId>,
    /// File-to-file reference edges — see `resolve.rs`. Empty until
    /// `set_resolved_imports` is called; resolving them needs a `Graph` to
    /// look symbols up in, so they can't be known at `from_symbols` time.
    imports: Vec<ResolvedImport>,
}

impl Graph {
    /// Build a `Graph` from every symbol extracted across a whole repo walk.
    pub fn from_symbols(symbols: Vec<Symbol>) -> Self {
        let by_id = symbols
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.clone(), SymbolId(i as u32)))
            .collect();
        Self {
            symbols,
            by_id,
            imports: Vec::new(),
        }
    }

    pub fn get(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0 as usize]
    }

    /// Look up a symbol by its stable string id (`"<file>::<name>"`, etc).
    pub fn find(&self, string_id: &str) -> Option<SymbolId> {
        self.by_id.get(string_id).copied()
    }

    /// The arena id of `id`'s containment parent, if it has one.
    pub fn parent_id(&self, id: SymbolId) -> Option<SymbolId> {
        let parent_string_id = self.get(id).parent.as_deref()?;
        self.find(parent_string_id)
    }

    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    pub fn imports(&self) -> &[ResolvedImport] {
        &self.imports
    }

    pub fn set_resolved_imports(&mut self, imports: Vec<ResolvedImport>) {
        self.imports = imports;
    }

    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_file;

    #[test]
    fn find_resolves_a_known_symbol_to_its_id() {
        let symbols = extract_file("export function double(x: number) {}\n", "a.ts");
        let graph = Graph::from_symbols(symbols);
        let id = graph.find("a.ts::double").expect("should be found");
        assert_eq!(graph.get(id).id, "a.ts::double");
    }

    #[test]
    fn find_returns_none_for_an_unknown_id() {
        let graph = Graph::from_symbols(Vec::new());
        assert_eq!(graph.find("a.ts::nope"), None);
    }

    #[test]
    fn parent_id_walks_a_method_up_to_its_class() {
        let symbols = extract_file("export class Foo {\n    bar(): void {}\n}\n", "a.ts");
        let graph = Graph::from_symbols(symbols);
        let class_id = graph.find("a.ts::Foo").unwrap();
        let method_id = graph.find("a.ts::Foo.bar").unwrap();

        assert_eq!(graph.parent_id(method_id), Some(class_id));
        assert_eq!(graph.parent_id(class_id), None);
    }

    #[test]
    fn save_then_load_round_trips_the_whole_graph() {
        let symbols = extract_file("export function double(x: number) {}\n", "a.ts");
        let graph = Graph::from_symbols(symbols);

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
            loaded.get(id),
            graph.get(graph.find("a.ts::double").unwrap())
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn from_symbols_across_multiple_files_indexes_all_of_them() {
        let mut all = extract_file("export const a = 1;\n", "a.ts");
        all.extend(extract_file("export const b = 2;\n", "b.ts"));
        let graph = Graph::from_symbols(all);

        assert_eq!(graph.len(), 2);
        assert!(graph.find("a.ts::a").is_some());
        assert!(graph.find("b.ts::b").is_some());
    }
}
