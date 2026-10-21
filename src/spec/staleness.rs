//! Dependency hashing and change detection for file and symbol specs: what a spec was written against, what is current now, and which parts differ.

use std::path::Path;

use super::format::HashPair;
use super::source_text::{contains_identifier, symbol_span_text};
use crate::graph::{Graph, Node, SymbolId};
use crate::hash::hash_text;
use crate::symbol::Symbol;

/// A reference target's contribution to a consumer's staleness key: its
/// `interfaceHash` if it has one (an exported symbol), falling back to a
/// file's own `source_hash` if the target is a whole file rather than a
/// symbol — never a spec/summary's text (see "Caching and invalidation":
/// rewording a dependency's prose must never invalidate its consumers).
fn interface_or_source_hash(graph: &Graph, id: SymbolId) -> String {
    graph
        .get_symbol(id)
        .map(|s| {
            s.interface_hash
                .clone()
                .unwrap_or_else(|| s.source_hash.clone())
        })
        .or_else(|| graph.get_file(id).map(|f| f.source_hash.clone()))
        .unwrap_or_default()
}

/// The `(target id, hash)` pairs behind a deps-hash: each reference-edge
/// target with the hash an importer depends on (its `interface_hash`, or its
/// own source hash when it has none -- see `interface_or_source_hash`).
/// Order-independent (sorted first) since import declaration order isn't a
/// real dependency, and deduplicated since two different local names can
/// resolve to the same target. Kept as its own step, rather than folded
/// straight into the hash, so a caller can see *which* target a deps-hash
/// is made of.
fn dependency_target_pairs(
    graph: &Graph,
    targets: impl Iterator<Item = SymbolId>,
) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = targets
        .map(|id| {
            (
                graph.string_id(id).to_string(),
                interface_or_source_hash(graph, id),
            )
        })
        .collect();
    pairs.sort();
    pairs.dedup();
    pairs
}

/// One deps-hash value over already-sorted, de-duplicated target pairs. The
/// formula (`"id:hash"` lines joined by `"\n"`, then `hash_text`) is what
/// every stored `deps_hash` was made with, so it must not change.
pub(crate) fn hash_dependency_pairs(pairs: &[(String, String)]) -> String {
    hash_text(
        &pairs
            .iter()
            .map(|(id, h)| format!("{id}:{h}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// `sym`'s reference-edge staleness contribution — the same per-symbol
/// scoping `dependency_lines` uses for the human-readable "Depends on"
/// list, but hashing only the *resolved* targets' current interface
/// hashes (an external/unresolved import has no interface CodeOwl can
/// observe, so it can't contribute to staleness). Persisted as
/// `HashPair.deps_hash` at generation time, recomputed here again at
/// every currency check to detect drift.
fn dependency_hash(graph: &Graph, root: &Path, file_id: SymbolId, sym: &Symbol) -> String {
    try_dependency_pairs(graph, root, file_id, sym)
        .map_or_else(String::new, |pairs| hash_dependency_pairs(&pairs))
}

/// `None` when the symbol's file node or source text can't be read -- the
/// case `dependency_hash` has always answered with an empty string rather
/// than the hash of an empty list, which a stored `deps_hash` still reflects.
pub(crate) fn try_dependency_pairs(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    sym: &Symbol,
) -> Option<Vec<(String, String)>> {
    let Node::File(file) = graph.get(file_id) else {
        return None;
    };
    let Ok(symbol_text) = symbol_span_text(root, graph, &file.id, sym) else {
        return None;
    };
    // Excludes a resolved import whose target is `sym` itself -- see the
    // identical exclusion in `scoped_symbol_deps`'s own doc comment for
    // why this can happen and why it's never a real dependency.
    let self_id = graph.find(&sym.id);
    let targets = graph
        .imports()
        .iter()
        .filter(|imp| {
            imp.from_file == file.id
                && contains_identifier(&symbol_text, &imp.imported_name)
                && imp.target != self_id
        })
        .filter_map(|imp| imp.target);
    Some(dependency_target_pairs(graph, targets))
}

/// What `submit` stores for a symbol: its combined `deps_hash` and the
/// shortened per-target pairs it was made from, computed together so the
/// two cannot disagree.
pub(super) fn dependency_record(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    sym: &Symbol,
) -> (String, Vec<(String, String)>) {
    match try_dependency_pairs(graph, root, file_id, sym) {
        Some(pairs) => (hash_dependency_pairs(&pairs), shorten_dep_targets(&pairs)),
        None => (String::new(), Vec::new()),
    }
}

/// How many hex characters of each target's hash `dep_targets` keeps. Enough
/// to tell two versions of one target apart; the full values are never
/// needed once the combined `deps_hash` is stored beside them.
const DEP_TARGET_HASH_LEN: usize = 12;

pub(crate) fn shorten_dep_targets(pairs: &[(String, String)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(id, h)| (id.clone(), h.chars().take(DEP_TARGET_HASH_LEN).collect()))
        .collect()
}

/// A file's own reference-edge staleness contribution — every one of its
/// resolved imports, regardless of which specific symbol in the file uses
/// each one (a file has no per-symbol text scoping the way a `### Depends
/// on` section does; `dependency_hash` above is the narrower, per-symbol
/// version of this same idea).
pub(super) fn file_dependency_hash(graph: &Graph, file_id: SymbolId) -> String {
    try_file_dependency_pairs(graph, file_id)
        .map_or_else(String::new, |pairs| hash_dependency_pairs(&pairs))
}

/// `None` when `file_id` is not a file node (answered with an empty string
/// by `file_dependency_hash`, as it always has been).
pub(crate) fn try_file_dependency_pairs(
    graph: &Graph,
    file_id: SymbolId,
) -> Option<Vec<(String, String)>> {
    let Node::File(file) = graph.get(file_id) else {
        return None;
    };
    let targets = graph
        .imports()
        .iter()
        .filter(|imp| imp.from_file == file.id)
        .filter_map(|imp| imp.target);
    Some(dependency_target_pairs(graph, targets))
}

/// The file-level counterpart of `dependency_record`.
pub(super) fn file_dependency_record(
    graph: &Graph,
    file_id: SymbolId,
) -> (String, Vec<(String, String)>) {
    match try_file_dependency_pairs(graph, file_id) {
        Some(pairs) => (hash_dependency_pairs(&pairs), shorten_dep_targets(&pairs)),
        None => (String::new(), Vec::new()),
    }
}

/// Diff two `(id, hash)` lists — shared between a feature's `participants`
/// map and a rollup's `files` map, since the staleness semantics are
/// identical for both: an existing entry whose hash moved, a new entry
/// that appeared, or an old entry that's gone are all real changes (the
/// "or the participant set itself changes" half of "Caching and
/// invalidation", applied uniformly). Empty means current; each entry in
/// the result names one thing that moved, deterministically, off the
/// graph — no LLM needed to say *that* something changed (see
/// `ARCHITECTURE.md`'s "Ordering").
pub fn diff_hash_lists(current: &[(String, String)], stored: &[(String, String)]) -> Vec<String> {
    let mut changed = Vec::new();
    for (id, hash) in current {
        match stored.iter().find(|(sid, _)| sid == id) {
            Some((_, h)) if h == hash => {}
            Some(_) => changed.push(format!("changed:{id}")),
            None => changed.push(format!("added:{id}")),
        }
    }
    for (id, _) in stored {
        if !current.iter().any(|(cid, _)| cid == id) {
            changed.push(format!("removed:{id}"));
        }
    }
    changed.sort();
    changed
}

/// What's changed for `sym` since `stored` was recorded — empty means
/// current. Expressed as a `diff_hash_lists` comparison over exactly two
/// fixed keys (`"source"`, `"dependencies"`) so a caller (`get_spec`) can
/// report *which* of the two moved, not just that something did.
pub fn symbol_changes(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    sym: &Symbol,
    stored: &HashPair,
) -> Vec<String> {
    diff_hash_lists(
        &[
            ("source".to_string(), sym.source_hash.clone()),
            (
                "dependencies".to_string(),
                dependency_hash(graph, root, file_id, sym),
            ),
        ],
        &[
            ("source".to_string(), stored.source_hash.clone()),
            ("dependencies".to_string(), stored.deps_hash.clone()),
        ],
    )
}

/// The file-level equivalent of `symbol_changes` — what's changed about
/// `file_id`'s own spec since `stored` was recorded.
pub fn file_changes(graph: &Graph, file_id: SymbolId, stored: &HashPair) -> Vec<String> {
    // The fingerprint, not the raw text: edits a file summary would not
    // mention leave it unchanged. A legacy spec stored none, so it differs.
    let fingerprint = graph
        .get_file(file_id)
        .map(|f| f.fingerprint.clone())
        .unwrap_or_default();
    diff_hash_lists(
        &[
            ("source".to_string(), fingerprint),
            (
                "dependencies".to_string(),
                file_dependency_hash(graph, file_id),
            ),
        ],
        &[
            ("source".to_string(), stored.fingerprint.clone()),
            ("dependencies".to_string(), stored.deps_hash.clone()),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::build_graph_from_sources;
    use crate::spec::format::{HashPair, dependency_lines};

    fn stored_for(graph: &Graph, file: &str, fingerprint: &str, raw: &str) -> HashPair {
        HashPair {
            source_hash: raw.to_string(),
            fingerprint: fingerprint.to_string(),
            deps_hash: file_dependency_hash(graph, graph.find(file).unwrap()),
            ..HashPair::default()
        }
    }

    #[test]
    fn a_degenerate_fold_type_depends_on_the_trait_its_impl_header_names() {
        // Open question 19, end to end: the resolved `Polite -> Greeter`
        // import exists, but before the header span was carried it never
        // scoped onto `Polite`, so `Greeter`'s contract changing could not
        // invalidate `Polite`'s document.
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-q19", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let greeter = "pub trait Greeter {\n    fn greet(&self) -> String;\n}\n";
        let people = "\
use crate::greeter::Greeter;\n\n\
pub struct Polite;\n\n\
impl Greeter for Polite {\n    fn greet(&self) -> String {\n        String::new()\n    }\n}\n";
        std::fs::write(dir.join("src/greeter.rs"), greeter).unwrap();
        std::fs::write(dir.join("src/people.rs"), people).unwrap();

        let mut graph = Graph::build(vec![
            crate::graph::FileExtraction {
                rel_path: "src/greeter.rs".to_string(),
                source_hash: hash_text(greeter),
                symbols: crate::rust::extract_file(greeter, "src/greeter.rs"),
            },
            crate::graph::FileExtraction {
                rel_path: "src/people.rs".to_string(),
                source_hash: hash_text(people),
                symbols: crate::rust::extract_file(people, "src/people.rs"),
            },
        ]);
        graph.set_resolved_imports(vec![crate::graph::ResolvedImport {
            from_file: "src/people.rs".to_string(),
            specifier: "crate::greeter".to_string(),
            imported_name: "Greeter".to_string(),
            target: graph.find("src/greeter.rs::Greeter"),
        }]);

        let file_id = graph.find("src/people.rs").unwrap();
        let polite = graph
            .get_symbol(graph.find("src/people.rs::Polite").unwrap())
            .unwrap();
        let deps = dependency_lines(&graph, &dir, file_id, polite);
        assert!(
            deps.iter().any(|d| d.contains("greeter.rs::Greeter")),
            "Polite must depend on Greeter, got {deps:?}"
        );
        assert!(
            !dependency_hash(&graph, &dir, file_id, polite).is_empty(),
            "dependency_hash must fold in Greeter's interface"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn deps_hash_is_a_hash_of_the_sorted_target_pairs_and_stays_byte_identical() {
        // The per-target `(id, hash)` pairs behind a `deps_hash` must be
        // available on their own, and hashing them must give exactly the
        // value a `deps_hash` has always had -- stored `deps_hash` values in
        // existing specs depend on that.
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-deppairs", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let greeter =
            "pub trait Greeter {\n    fn greet(&self) -> String;\n}\n\npub struct Name;\n";
        std::fs::write(dir.join("src/greeter.rs"), greeter).unwrap();
        let graph = Graph::build(vec![crate::graph::FileExtraction {
            rel_path: "src/greeter.rs".to_string(),
            source_hash: hash_text(greeter),
            symbols: crate::rust::extract_file(greeter, "src/greeter.rs"),
        }]);
        let greeter_id = graph.find("src/greeter.rs::Greeter").unwrap();
        let name_id = graph.find("src/greeter.rs::Name").unwrap();

        // Reverse order and a duplicate: the pairs come back sorted by id and
        // de-duplicated, because import order is not a real dependency.
        let pairs = dependency_target_pairs(&graph, [name_id, greeter_id, name_id].into_iter());
        let ids: Vec<&str> = pairs.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["src/greeter.rs::Greeter", "src/greeter.rs::Name"]);

        // The combined hash is the original formula over those pairs...
        let original_formula = hash_text(&format!(
            "{}:{}\n{}:{}",
            pairs[0].0, pairs[0].1, pairs[1].0, pairs[1].1
        ));
        assert_eq!(hash_dependency_pairs(&pairs), original_formula);
        // Each pair's hash is the target's interface hash, or its own source
        // hash when it has none -- the existing fallback, unchanged.
        let greeter_sym = graph.get_symbol(greeter_id).unwrap();
        assert_eq!(
            pairs[0].1,
            greeter_sym
                .interface_hash
                .clone()
                .unwrap_or_else(|| greeter_sym.source_hash.clone())
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn diff_hash_lists_reports_changed_added_and_removed() {
        let current = vec![
            ("a".to_string(), "h1".to_string()),
            ("b".to_string(), "h2-new".to_string()),
            ("c".to_string(), "h3".to_string()),
        ];
        let stored = vec![
            ("a".to_string(), "h1".to_string()),
            ("b".to_string(), "h2-old".to_string()),
            ("d".to_string(), "h4".to_string()),
        ];
        assert_eq!(
            diff_hash_lists(&current, &stored),
            vec!["added:c", "changed:b", "removed:d"]
        );
    }

    #[test]
    fn diff_hash_lists_is_empty_when_nothing_moved() {
        let list = vec![("a".to_string(), "h1".to_string())];
        assert!(diff_hash_lists(&list, &list).is_empty());
    }

    #[test]
    fn file_changes_compares_the_fingerprint_not_the_raw_text() {
        let mut graph = build_graph_from_sources(&[("a.ts", "export function f(): void {}\n")]);
        graph.set_file_fingerprint("a.ts", "fp-now");
        let file_id = graph.find("a.ts").unwrap();

        // Same fingerprint, a raw hash that no longer matches: not stale.
        let stored = stored_for(&graph, "a.ts", "fp-now", "an-old-raw-hash");
        assert!(file_changes(&graph, file_id, &stored).is_empty());

        // A different fingerprint is a source change, even if the raw hash matches.
        let raw_now = graph.get_file(file_id).unwrap().source_hash.clone();
        let stored = stored_for(&graph, "a.ts", "fp-then", &raw_now);
        assert_eq!(
            file_changes(&graph, file_id, &stored),
            vec!["changed:source"]
        );
    }

    #[test]
    fn a_legacy_file_spec_with_no_fingerprint_reads_as_changed() {
        let graph = build_graph_from_sources(&[("a.ts", "export function f(): void {}\n")]);
        let file_id = graph.find("a.ts").unwrap();
        let raw_now = graph.get_file(file_id).unwrap().source_hash.clone();
        let legacy = stored_for(&graph, "a.ts", "", &raw_now);
        assert_eq!(
            file_changes(&graph, file_id, &legacy),
            vec!["changed:source"]
        );
    }
}
