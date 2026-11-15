//! The edges of a C++ graph. Step 2 of `docs/cpp-step2-plan.md`, part C.
//!
//! An include brings in a whole header, but an edge is file to *symbol*
//! (`ResolvedImport`), and staleness reads edges by the word a symbol's own
//! text contains. So the include graph is *narrowed*: for each file `f` and
//! each file `g` that `f` reaches through includes (any depth, so no cascade
//! logic is needed), every top-level symbol of `g` whose name `f` writes
//! becomes one edge. An edit to a header moves a dependant only if the
//! dependant names something that changed.

use std::collections::HashMap;

use super::resolve::IncludeGraph;
use crate::graph::{Graph, ResolvedImport};
use crate::imports::FileImports;
use crate::symbol::SymbolId;

/// What a file offers to the files that include it: each word another file
/// may write, with the symbol that word reaches.
fn targets(graph: &Graph, file: &str) -> Vec<(String, SymbolId)> {
    let Some(file_id) = graph.find(file) else {
        return Vec::new();
    };
    let prefix = format!("{file}::");
    let mut out = Vec::new();
    for &id in graph.children_ids(file_id) {
        let Some(sym) = graph.get_symbol(id) else {
            continue;
        };
        let Some(name) = sym.id.strip_prefix(&prefix) else {
            continue;
        };
        // A `DB::Open` defined outside its class has `::` in its name, and
        // no word a file writes ever does, so it is never matched: it is
        // reached through its class (pairing, a later step, folds it in).
        // Likewise a free `operator==` is never matched, because the word
        // list holds identifiers and not punctuation (plan section 9).
        out.push((name.to_string(), id));
        // An enumerator is written without its enum, so it is a word that
        // reaches the enum.
        for &child in graph.children_ids(id) {
            if let Some(c) = graph.get_symbol(child)
                && c.raw == "enumerator"
                && let Some(word) = c.id.rsplit("::").next()
            {
                out.push((word.to_string(), id));
            }
        }
    }
    out
}

/// One edge per (file, target symbol, word) for every file in
/// `file_imports`. Deterministic: files in path order, then the including
/// file's reach in path order, then the header's symbols in graph order.
pub fn resolve(file_imports: &HashMap<String, FileImports>, graph: &Graph) -> Vec<ResolvedImport> {
    let includes = IncludeGraph::build(file_imports);
    let mut files: Vec<&String> = file_imports.keys().collect();
    files.sort();

    // A header is offered to many includers; read its symbols once.
    let mut offered: HashMap<String, Vec<(String, SymbolId)>> = HashMap::new();
    let mut out = Vec::new();
    for from_file in files {
        let words = &file_imports[from_file].identifiers;
        if words.is_empty() {
            continue;
        }
        for header in includes.reach(from_file) {
            if !offered.contains_key(&header) {
                offered.insert(header.clone(), targets(graph, &header));
            }
            for (word, id) in &offered[&header] {
                if words.binary_search(word).is_ok() {
                    out.push(ResolvedImport {
                        from_file: from_file.clone(),
                        specifier: header.clone(),
                        imported_name: word.clone(),
                        target: Some(*id),
                    });
                }
            }
        }
    }
    out
}
