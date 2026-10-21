//! Test-only helpers that build a `Graph` straight from source text. They
//! live here, not in `graph.rs`, so the graph never depends on a language
//! pack even in its tests: `graph.rs` knows only the arena and the edge
//! types, and this module is the one place a test fixture reaches into the
//! TypeScript pack on its behalf.

use crate::graph::{FileExtraction, Graph};
use crate::hash::hash_text;

/// Extract `source` (already read from `rel_path`) and hash it in one
/// step — the shape every multi-file test fixture in this codebase wants.
/// It mirrors `TypeScriptNextStack::extract_symbols` (TS via
/// `typescript::extract_file`, `.sql` via `typescript::extract_tables`), not
/// the repo's detected `StackPack`, so it only produces symbols for TS/SQL
/// fixtures. Production extraction goes through `RepoIndex::build` →
/// `pack.extract_symbols`.
pub(crate) fn extract_and_hash(rel_path: &str, source: &str) -> FileExtraction {
    let symbols = if rel_path.ends_with(".sql") {
        crate::typescript::extract_tables(source, rel_path)
    } else {
        crate::typescript::extract_file(source, rel_path)
    };
    FileExtraction {
        rel_path: rel_path.to_string(),
        source_hash: hash_text(source),
        symbols,
    }
}

/// Build a `Graph` straight from `(rel_path, source)` pairs — the shape
/// every test fixture in this codebase starts from. TS/SQL only (see
/// `extract_and_hash`).
pub(crate) fn build_graph_from_sources(files: &[(&str, &str)]) -> Graph {
    let extractions = files
        .iter()
        .map(|(rel, src)| extract_and_hash(rel, src))
        .collect();
    Graph::build(extractions)
}
