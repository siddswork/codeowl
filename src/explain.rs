//! Why a spec is stale: the cause behind each `changed` entry, named.
//!
//! Staleness itself is decided in `spec.rs` by comparing stored hashes with
//! freshly computed ones. That says *that* something moved. This module says
//! *what*, as a short chain: a symbol's own text, a named dependency, or a
//! child document that is itself stale (with its own cause nested inside).
//! It only reads: it never writes a spec and never asks for one to be
//! written.

use std::path::Path;

use anyhow::{Context, Result};

use crate::graph::{Graph, SymbolId};
use crate::spec::{self, HashPair};

/// How many levels a chain may nest: system, folder, file. A document's own
/// causes are always listed; the causes of its stale children are listed
/// only while the document is above this depth.
pub const MAX_DEPTH: usize = 3;

/// One reason a document is stale.
///
/// `kind` is one of `source`, `dependency`, `added`, `removed`, `child`,
/// `rewritten`, `participant` or `unknown`. `target` names the thing that
/// moved when there is one. `because` holds the cause of a stale child.
#[derive(Debug, Clone, PartialEq)]
pub struct StaleCause {
    pub kind: String,
    pub target: Option<String>,
    pub detail: String,
    pub because: Vec<StaleCause>,
}

impl StaleCause {
    fn new(kind: &str, target: Option<&str>, detail: &str) -> Self {
        Self {
            kind: kind.to_string(),
            target: target.map(str::to_string),
            detail: detail.to_string(),
            because: Vec::new(),
        }
    }
}

/// The state of one document, and why, if it is stale.
///
/// `status` is `missing`, `current` or `stale`. `causes` is empty unless the
/// document is stale.
#[derive(Debug, Clone, PartialEq)]
pub struct Explanation {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub causes: Vec<StaleCause>,
}

impl Explanation {
    fn new(id: &str, kind: &str, status: &str, causes: Vec<StaleCause>) -> Self {
        Self {
            id: id.to_string(),
            kind: kind.to_string(),
            status: status.to_string(),
            causes,
        }
    }

    fn missing(id: &str, kind: &str) -> Self {
        Self::new(id, kind, "missing", Vec::new())
    }

    fn current(id: &str, kind: &str) -> Self {
        Self::new(id, kind, "current", Vec::new())
    }
}

/// Explain the document `id` names: a symbol id, a file path, `rollup:<dir>`,
/// `feature:<slug>` or `system`. A document with no spec is `missing`, and
/// an id that names nothing is an error.
pub fn explain(graph: &Graph, root: &Path, id: &str) -> Result<Explanation> {
    if id == "system" {
        return explain_system(graph, root, 1);
    }
    if let Some(dir) = id.strip_prefix("rollup:") {
        return explain_rollup(graph, root, dir, 1);
    }
    if let Some(slug) = id.strip_prefix("feature:") {
        return explain_feature(graph, root, slug);
    }
    let node = graph
        .find(id)
        .with_context(|| format!("unknown id {id:?}"))?;
    if graph.get_file(node).is_some() {
        explain_file(graph, root, node)
    } else {
        explain_symbol(graph, root, node)
    }
}

/// Split a `changed:<id>`, `added:<id>` or `removed:<id>` entry at its first
/// colon. The id may contain colons of its own.
fn split_change(entry: &str) -> Option<(&str, &str)> {
    entry.split_once(':')
}

fn explain_symbol(graph: &Graph, root: &Path, sym_id: SymbolId) -> Result<Explanation> {
    let sym = graph.get_symbol(sym_id).context("not a symbol")?;
    let id = sym.id.as_str();
    // A member of a class has the class as its parent; its spec lives in the
    // file, so ask for the owning file, not the parent.
    let file_id = graph
        .find(graph.owning_file_id(sym_id))
        .context("symbol's file is not in the graph")?;
    let file = graph.get_file(file_id).context("not a file")?;
    let Some(spec) = spec::read_file_spec(root, &file.id)? else {
        return Ok(Explanation::missing(id, "symbol"));
    };
    let Some((_, stored)) = spec.symbols.iter().find(|(sid, _)| sid == id) else {
        return Ok(Explanation::missing(id, "symbol"));
    };
    let changes = spec::symbol_changes(graph, root, file_id, sym, stored);
    if changes.is_empty() {
        return Ok(Explanation::current(id, "symbol"));
    }
    let mut causes = Vec::new();
    if changes.iter().any(|c| c == "changed:source") {
        causes.push(StaleCause::new("source", None, "its own text changed"));
    }
    if changes.iter().any(|c| c == "changed:dependencies") {
        match spec::try_dependency_pairs(graph, root, file_id, sym) {
            Some(now) => causes.extend(dependency_causes(graph, &now, stored)),
            None => causes.push(unreadable_cause()),
        }
    }
    Ok(Explanation::new(id, "symbol", "stale", causes))
}

fn explain_file(graph: &Graph, root: &Path, file_id: SymbolId) -> Result<Explanation> {
    let file = graph.get_file(file_id).context("not a file")?;
    let id = file.id.as_str();
    let spec = spec::read_file_spec(root, id)?;
    // A file spec with symbol sections but no file summary yet has nothing
    // to be stale about.
    let Some(spec) = spec.filter(|s| !s.file.spec_hash.is_empty()) else {
        return Ok(Explanation::missing(id, "file"));
    };
    let changes = spec::file_changes(graph, file_id, &spec.file);
    let mut causes = Vec::new();
    if changes.iter().any(|c| c == "changed:source") {
        causes.push(StaleCause::new("source", None, "the file's text changed"));
    }
    if changes.iter().any(|c| c == "changed:dependencies") {
        match spec::try_file_dependency_pairs(graph, file_id) {
            Some(now) => causes.extend(dependency_causes(graph, &now, &spec.file)),
            None => causes.push(unreadable_cause()),
        }
    }
    if changes.is_empty() {
        // The file's own text and imports match its summary. It still counts
        // as stale (as `get_spec_coverage` counts it) while a symbol section
        // is missing or stale, so name those.
        causes.extend(stale_symbol_causes(graph, root, file_id, &spec));
    }
    if causes.is_empty() {
        return Ok(Explanation::current(id, "file"));
    }
    Ok(Explanation::new(id, "file", "stale", causes))
}

/// The dependency hash moved but the current targets cannot be worked out
/// (the file or its text is unreadable), so no single one can be blamed.
fn unreadable_cause() -> StaleCause {
    StaleCause::new(
        "unknown",
        None,
        "a dependency changed, but the source could not be read to say which",
    )
}

/// One `child` cause for each symbol of the file whose section is missing
/// from the spec or no longer matches the code.
fn stale_symbol_causes(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    spec: &spec::FileSpec,
) -> Vec<StaleCause> {
    spec::spec_bearing_children(graph, file_id)
        .into_iter()
        .filter_map(|sym_id| {
            let sym = graph.get_symbol(sym_id)?;
            let detail = match spec.symbol_hash(&sym.id) {
                None => "its section has not been written yet".to_string(),
                Some(stored)
                    if spec::symbol_changes(graph, root, file_id, sym, stored).is_empty() =>
                {
                    return None;
                }
                Some(_) => "its section is out of date".to_string(),
            };
            Some(StaleCause::new("child", Some(&sym.id), &detail))
        })
        .collect()
}

/// Which dependencies moved, from the targets recorded when the spec was
/// written and the targets as they are now (`now` carries full hashes, the
/// record carries shortened ones).
fn dependency_causes(
    graph: &Graph,
    now: &[(String, String)],
    stored: &HashPair,
) -> Vec<StaleCause> {
    let now_short = spec::shorten_dep_targets(now);
    if stored.dep_targets.is_empty() {
        // Either the spec recorded that it had no dependencies, so each
        // current one is new, or it predates the record and nothing is known.
        if stored.deps_hash == spec::hash_dependency_pairs(&[]) && !now_short.is_empty() {
            return now_short
                .iter()
                .map(|(id, _)| StaleCause::new("added", Some(id), "a new dependency"))
                .collect();
        }
        return vec![StaleCause::new(
            "unknown",
            None,
            "a dependency changed, but this spec has no record of which one",
        )];
    }
    let mut causes = Vec::new();
    for (id, hash) in &now_short {
        match stored.dep_targets.iter().find(|(sid, _)| sid == id) {
            Some((_, old)) if old == hash => {}
            Some(_) => causes.push(StaleCause::new(
                "dependency",
                Some(id),
                &target_detail(graph, id),
            )),
            None => causes.push(StaleCause::new("added", Some(id), "a new dependency")),
        }
    }
    for (id, _) in &stored.dep_targets {
        if !now_short.iter().any(|(nid, _)| nid == id) {
            causes.push(StaleCause::new(
                "removed",
                Some(id),
                "no longer a dependency",
            ));
        }
    }
    if causes.is_empty() {
        causes.push(StaleCause::new(
            "unknown",
            None,
            "the dependencies changed, but no single one explains it",
        ));
    }
    causes
}

/// What it means that `id` moved: a symbol's interface, a symbol with no
/// interface hash (so its whole text counts), or a file's text.
fn target_detail(graph: &Graph, id: &str) -> String {
    match graph.find(id) {
        Some(t) => match graph.get_symbol(t) {
            Some(sym) if sym.interface_hash.is_some() => "its interface changed".to_string(),
            Some(_) => "its source text changed, and it has no interface hash".to_string(),
            None => "its file text changed".to_string(),
        },
        None => "it changed".to_string(),
    }
}

/// A child of a folder or system spec: stale or missing now (so its own
/// cause is nested), or current but written again since.
fn child_cause(
    target: &str,
    now_hash: Option<&str>,
    depth: usize,
    child: Explanation,
) -> StaleCause {
    if now_hash.is_none_or(str::is_empty) {
        let detail = if child.status == "missing" {
            "its summary is missing"
        } else {
            "its summary is stale"
        };
        let mut cause = StaleCause::new("child", Some(target), detail);
        if depth < MAX_DEPTH {
            cause.because = child.causes;
        }
        cause
    } else {
        StaleCause::new(
            "rewritten",
            Some(target),
            "its summary was written again after this one",
        )
    }
}

fn explain_rollup(graph: &Graph, root: &Path, dir: &str, depth: usize) -> Result<Explanation> {
    let id = format!("rollup:{dir}");
    let Some(spec) = spec::read_rollup_spec(root, dir)? else {
        return Ok(Explanation::missing(&id, "rollup"));
    };
    let current = spec::current_file_hashes(graph, root, dir)?;
    let changed = spec::diff_hash_lists(&current, &spec.files);
    if changed.is_empty() {
        return Ok(Explanation::current(&id, "rollup"));
    }
    let mut causes = Vec::new();
    for entry in &changed {
        let Some((action, target)) = split_change(entry) else {
            continue;
        };
        causes.push(match action {
            "added" => StaleCause::new("added", Some(target), "a new file in this folder"),
            "removed" => StaleCause::new("removed", Some(target), "no longer in this folder"),
            _ => {
                let now = current
                    .iter()
                    .find(|(fid, _)| fid == target)
                    .map(|(_, h)| h.as_str());
                let child = match graph.find(target) {
                    Some(fid) if graph.get_file(fid).is_some() => explain_file(graph, root, fid)?,
                    _ => Explanation::missing(target, "file"),
                };
                child_cause(target, now, depth, child)
            }
        });
    }
    Ok(Explanation::new(&id, "rollup", "stale", causes))
}

fn explain_feature(graph: &Graph, root: &Path, slug: &str) -> Result<Explanation> {
    let id = format!("feature:{slug}");
    let fm = crate::features::feature_model_for(graph)
        .with_context(|| format!("unknown id {id:?}: this stack has no features"))?;
    let entry = fm
        .enumerate_entry_points(graph)
        .into_iter()
        .find(|e| e.id == slug)
        .with_context(|| format!("unknown id {id:?}"))?;
    let Some(spec) = spec::read_feature_spec(root, slug)? else {
        return Ok(Explanation::missing(&id, "feature"));
    };
    let participants = crate::features::assemble_participants(graph, fm, &entry);
    let current = spec::current_participant_hashes(graph, &participants)?;
    let changed = spec::diff_hash_lists(&current, &spec.participants);
    if changed.is_empty() {
        return Ok(Explanation::current(&id, "feature"));
    }
    let causes = changed
        .iter()
        .filter_map(|entry| split_change(entry))
        .map(|(action, target)| match action {
            "added" => StaleCause::new("added", Some(target), "a new part of this feature"),
            "removed" => StaleCause::new("removed", Some(target), "no longer part of this feature"),
            _ => StaleCause::new("participant", Some(target), &target_detail(graph, target)),
        })
        .collect();
    Ok(Explanation::new(&id, "feature", "stale", causes))
}

fn explain_system(graph: &Graph, root: &Path, depth: usize) -> Result<Explanation> {
    let Some(spec) = spec::read_system_spec(root)? else {
        return Ok(Explanation::missing("system", "system"));
    };
    let mut current = spec::current_module_hashes(graph, root)?;
    current.extend(spec::current_feature_hashes(graph, root)?);
    let mut stored = spec.modules.clone();
    stored.extend(spec.features.clone());
    let changed = spec::diff_hash_lists(&current, &stored);
    if changed.is_empty() {
        return Ok(Explanation::current("system", "system"));
    }
    let modules = spec::enumerate_modules(graph);
    let mut causes = Vec::new();
    for entry in &changed {
        let Some((action, target)) = split_change(entry) else {
            continue;
        };
        causes.push(match action {
            "added" => StaleCause::new("added", Some(target), "a new folder or feature"),
            "removed" => StaleCause::new("removed", Some(target), "this folder or feature is gone"),
            _ => {
                let now = current
                    .iter()
                    .find(|(cid, _)| cid == target)
                    .map(|(_, h)| h.as_str());
                let child = if modules.iter().any(|m| m == target) {
                    explain_rollup(graph, root, target, depth + 1)?
                } else {
                    explain_feature(graph, root, target)
                        .unwrap_or_else(|_| Explanation::missing(target, "feature"))
                };
                child_cause(target, now, depth, child)
            }
        });
    }
    Ok(Explanation::new("system", "system", "stale", causes))
}

/// How many dependency targets `explain_corpus` lists as the busiest.
const TOP_TARGETS: usize = 10;

/// The causes behind every stale spec in the repo, counted.
///
/// `by_cause` counts stale documents by the set of causes they have, for
/// example `source`, `dependency`, `source + dependency` or `child`.
/// `top_targets` lists the dependencies that stale the most symbol and file
/// specs. `coarse_dependants` counts importers resting on a target that has
/// no interface hash, whether or not anything is stale now.
#[derive(Debug, Clone, PartialEq)]
pub struct CorpusSummary {
    pub documents: usize,
    pub stale: usize,
    pub by_cause: Vec<(String, usize)>,
    pub top_targets: Vec<(String, usize)>,
    pub coarse_dependants: usize,
}

/// Explain every spec that exists, and count the causes.
pub fn explain_corpus(graph: &Graph, root: &Path) -> Result<CorpusSummary> {
    let mut ids: Vec<String> = Vec::new();
    for item in spec::coverage(graph, root, None)? {
        if item.kind == "file"
            && let Some(file_spec) = spec::read_file_spec(root, &item.id)?
        {
            ids.extend(file_spec.symbols.into_iter().map(|(id, _)| id));
        }
        ids.push(item.id);
    }

    let mut documents = 0;
    let mut stale = 0;
    let mut by_cause: std::collections::BTreeMap<String, usize> = Default::default();
    let mut targets: std::collections::BTreeMap<String, usize> = Default::default();
    for id in ids {
        // One document that cannot be explained should not hide the rest.
        let Ok(e) = explain(graph, root, &id) else {
            continue;
        };
        if e.status == "missing" {
            continue;
        }
        documents += 1;
        if e.status != "stale" {
            continue;
        }
        stale += 1;
        *by_cause.entry(cause_label(&e)).or_default() += 1;
        if e.kind == "symbol" || e.kind == "file" {
            for cause in e.causes.iter().filter(|c| c.kind == "dependency") {
                if let Some(target) = &cause.target {
                    *targets.entry(target.clone()).or_default() += 1;
                }
            }
        }
    }

    let mut by_cause: Vec<(String, usize)> = by_cause.into_iter().collect();
    by_cause.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut top_targets: Vec<(String, usize)> = targets.into_iter().collect();
    top_targets.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top_targets.truncate(TOP_TARGETS);
    Ok(CorpusSummary {
        documents,
        stale,
        by_cause,
        top_targets,
        coarse_dependants: coarse_dependants(graph),
    })
}

/// A stale document's causes as one label: its distinct kinds, joined by
/// ` + `. For a symbol or file, a new, removed or moved dependency all read
/// as `dependency`.
fn cause_label(e: &Explanation) -> String {
    let leaf = e.kind == "symbol" || e.kind == "file";
    let labels: std::collections::BTreeSet<&str> = e
        .causes
        .iter()
        .map(|c| match c.kind.as_str() {
            "dependency" | "added" | "removed" if leaf => "dependency",
            "added" | "removed" => "added or removed",
            "rewritten" => "rewritten child",
            other => other,
        })
        .collect();
    labels.into_iter().collect::<Vec<_>>().join(" + ")
}

/// Importers resting on a target with no interface hash: one per importing
/// file and target, counting a target in another file only. Their
/// dependency hash uses the target's whole text, so any edit to it stales
/// them.
pub fn coarse_dependants(graph: &Graph) -> usize {
    let mut pairs: std::collections::BTreeSet<(&str, &str)> = Default::default();
    for imp in graph.imports() {
        let Some(target) = imp.target else {
            continue;
        };
        let coarse = match graph.get_symbol(target) {
            Some(sym) => {
                sym.interface_hash.is_none() && graph.owning_file_id(target) != imp.from_file
            }
            None => true,
        };
        if coarse {
            pairs.insert((imp.from_file.as_str(), graph.string_id(target)));
        }
    }
    pairs.len()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::graph::{FileExtraction, Graph};
    use crate::hash::hash_text;
    use crate::spec;

    const SYM: &str = "### Summary\nDoes one specific thing well.\n### Behavior\nIt runs and returns a plain value.\n";
    const FILE: &str = "A small module that does one clear job.";
    const FILE_REWRITTEN: &str = "A different but equally clear module summary.";

    const GREETER_V1: &str =
        "pub trait Greeter {\n    fn greet(&self) -> String {\n        String::new()\n    }\n}\n";
    const GREETER_BODY: &str = "pub trait Greeter {\n    fn greet(&self) -> String {\n        String::from(\"hi\")\n    }\n}\n";
    const GREETER_SIG: &str = "pub trait Greeter {\n    fn greet(&self, name: &str) -> String {\n        String::new()\n    }\n}\n";
    const GREETER_PRIVATE_V1: &str =
        "trait Greeter {\n    fn greet(&self) -> String {\n        String::new()\n    }\n}\n";
    const GREETER_PRIVATE_BODY: &str = "trait Greeter {\n    fn greet(&self) -> String {\n        String::from(\"hi\")\n    }\n}\n";
    const PEOPLE_V1: &str = "use crate::greeter::Greeter;\n\npub struct Polite;\n\nimpl Greeter for Polite {\n    fn greet(&self) -> String {\n        String::new()\n    }\n}\n";
    const PEOPLE_BODY: &str = "use crate::greeter::Greeter;\n\npub struct Polite;\n\nimpl Greeter for Polite {\n    fn greet(&self) -> String {\n        String::from(\"hello\")\n    }\n}\n";
    const PEOPLE_NODEP: &str = "pub struct Polite;\n";

    fn fixture_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-explain-test-{}-{suffix}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        dir
    }

    /// Write the two files and index them; `import` says whether
    /// `people.rs`'s `use` of `Greeter` is a resolved dependency.
    fn build(dir: &Path, greeter: &str, people: &str, import: bool) -> Graph {
        std::fs::write(dir.join("src/greeter.rs"), greeter).unwrap();
        std::fs::write(dir.join("src/people.rs"), people).unwrap();
        let mut graph = Graph::build(vec![
            FileExtraction {
                rel_path: "src/greeter.rs".to_string(),
                source_hash: hash_text(greeter),
                symbols: crate::rust::extract_file(greeter, "src/greeter.rs"),
            },
            FileExtraction {
                rel_path: "src/people.rs".to_string(),
                source_hash: hash_text(people),
                symbols: crate::rust::extract_file(people, "src/people.rs"),
            },
        ]);
        if import {
            graph.set_resolved_imports(vec![crate::resolve::ResolvedImport {
                from_file: "src/people.rs".to_string(),
                specifier: "crate::greeter".to_string(),
                imported_name: "Greeter".to_string(),
                target: graph.find("src/greeter.rs::Greeter"),
            }]);
        }
        graph
    }

    /// Every document the fixture has: its two symbols, two files, the
    /// folder and the system spec.
    fn generate_all(graph: &Graph, dir: &Path) {
        spec::submit(graph, dir, "src/greeter.rs::Greeter", SYM).unwrap();
        spec::submit(graph, dir, "src/people.rs::Polite", SYM).unwrap();
        spec::submit(graph, dir, "src/greeter.rs", FILE).unwrap();
        spec::submit(graph, dir, "src/people.rs", FILE).unwrap();
        spec::submit_rollup(graph, dir, "src", "A folder of two small greeting modules.").unwrap();
        spec::submit_system(
            graph,
            dir,
            "# Tiny library\nA tiny library used only in tests.",
        )
        .unwrap();
    }

    fn kinds(e: &Explanation) -> Vec<&str> {
        e.causes.iter().map(|c| c.kind.as_str()).collect()
    }

    #[test]
    fn a_body_edit_of_a_symbol_is_a_source_cause_only() {
        let dir = fixture_dir("body");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir);
        let after = build(&dir, GREETER_V1, PEOPLE_BODY, true);
        let e = explain(&after, &dir, "src/people.rs::Polite").unwrap();
        assert_eq!(e.kind, "symbol");
        assert_eq!(e.status, "stale");
        assert_eq!(kinds(&e), ["source"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_changed_signature_of_an_imported_symbol_is_a_named_dependency_cause() {
        let dir = fixture_dir("sig");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir);
        let after = build(&dir, GREETER_SIG, PEOPLE_V1, true);
        let e = explain(&after, &dir, "src/people.rs::Polite").unwrap();
        assert_eq!(e.status, "stale");
        assert_eq!(kinds(&e), ["dependency"]);
        assert_eq!(
            e.causes[0].target.as_deref(),
            Some("src/greeter.rs::Greeter")
        );
        assert!(
            e.causes[0].detail.contains("interface"),
            "{}",
            e.causes[0].detail
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_body_only_edit_of_an_imported_symbol_leaves_the_importer_current() {
        let dir = fixture_dir("bodydep");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir);
        let after = build(&dir, GREETER_BODY, PEOPLE_V1, true);
        let e = explain(&after, &dir, "src/people.rs::Polite").unwrap();
        assert_eq!(e.status, "current");
        assert!(e.causes.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_target_without_an_interface_hash_is_named_as_coarse() {
        let dir = fixture_dir("coarse");
        generate_all(&build(&dir, GREETER_PRIVATE_V1, PEOPLE_V1, true), &dir);
        let after = build(&dir, GREETER_PRIVATE_BODY, PEOPLE_V1, true);
        let e = explain(&after, &dir, "src/people.rs::Polite").unwrap();
        assert_eq!(kinds(&e), ["dependency"]);
        assert!(
            e.causes[0].detail.contains("no interface hash"),
            "{}",
            e.causes[0].detail
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_spec_written_before_the_record_existed_gets_an_unknown_cause() {
        let dir = fixture_dir("legacy");
        let before = build(&dir, GREETER_V1, PEOPLE_V1, true);
        generate_all(&before, &dir);
        // Strip the recorded targets, as a spec from before the record has.
        let mut s = spec::read_file_spec(&dir, "src/people.rs")
            .unwrap()
            .unwrap();
        s.file.dep_targets.clear();
        for (_, h) in s.symbols.iter_mut() {
            h.dep_targets.clear();
        }
        let file_id = before.find("src/people.rs").unwrap();
        std::fs::write(
            spec::spec_path(&dir, "src/people.rs"),
            spec::render(&before, &dir, file_id, &s),
        )
        .unwrap();

        let after = build(&dir, GREETER_SIG, PEOPLE_V1, true);
        let e = explain(&after, &dir, "src/people.rs::Polite").unwrap();
        assert_eq!(kinds(&e), ["unknown"]);
        assert!(e.causes[0].target.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_dependency_that_appears_after_a_spec_recorded_none_is_added() {
        let dir = fixture_dir("added");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_NODEP, false), &dir);
        let after = build(&dir, GREETER_V1, PEOPLE_V1, true);
        let e = explain(&after, &dir, "src/people.rs::Polite").unwrap();
        assert_eq!(kinds(&e), ["source", "added"]);
        assert_eq!(
            e.causes[1].target.as_deref(),
            Some("src/greeter.rs::Greeter")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn source_and_dependency_moving_together_give_two_causes() {
        let dir = fixture_dir("both");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir);
        let after = build(&dir, GREETER_SIG, PEOPLE_BODY, true);
        let e = explain(&after, &dir, "src/people.rs::Polite").unwrap();
        assert_eq!(kinds(&e), ["source", "dependency"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_file_is_explained_like_a_symbol() {
        let dir = fixture_dir("file");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir);
        let text = build(&dir, GREETER_V1, PEOPLE_BODY, true);
        let e = explain(&text, &dir, "src/people.rs").unwrap();
        assert_eq!(e.kind, "file");
        assert_eq!(kinds(&e), ["source"]);

        let dep = build(&dir, GREETER_SIG, PEOPLE_V1, true);
        let e = explain(&dep, &dir, "src/people.rs").unwrap();
        assert_eq!(kinds(&e), ["dependency"]);
        assert_eq!(
            e.causes[0].target.as_deref(),
            Some("src/greeter.rs::Greeter")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_folder_names_its_stale_child_and_nests_the_childs_cause() {
        let dir = fixture_dir("rollup");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir);
        let after = build(&dir, GREETER_V1, PEOPLE_BODY, true);
        let e = explain(&after, &dir, "rollup:src").unwrap();
        assert_eq!(e.kind, "rollup");
        assert_eq!(e.status, "stale");
        assert_eq!(kinds(&e), ["child"]);
        assert_eq!(e.causes[0].target.as_deref(), Some("src/people.rs"));
        let inner: Vec<&str> = e.causes[0]
            .because
            .iter()
            .map(|c| c.kind.as_str())
            .collect();
        assert_eq!(inner, ["source"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_regenerated_child_is_reported_as_rewritten() {
        let dir = fixture_dir("rewritten");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir);
        let after = build(&dir, GREETER_V1, PEOPLE_BODY, true);
        // The file summary is written again, with new prose: it is current
        // now, but no longer the one the folder summary was written from.
        spec::submit(&after, &dir, "src/people.rs", FILE_REWRITTEN).unwrap();
        let e = explain(&after, &dir, "rollup:src").unwrap();
        assert_eq!(kinds(&e), ["rewritten"]);
        assert_eq!(e.causes[0].target.as_deref(), Some("src/people.rs"));
        assert!(e.causes[0].because.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_system_spec_nests_folder_then_file_causes() {
        let dir = fixture_dir("system");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir);
        let after = build(&dir, GREETER_V1, PEOPLE_BODY, true);
        let e = explain(&after, &dir, "system").unwrap();
        assert_eq!(e.kind, "system");
        assert_eq!(kinds(&e), ["child"]);
        let folder = &e.causes[0];
        assert_eq!(folder.target.as_deref(), Some("src"));
        assert_eq!(folder.because.len(), 1);
        let file = &folder.because[0];
        assert_eq!(file.target.as_deref(), Some("src/people.rs"));
        assert_eq!(file.because.len(), 1);
        assert_eq!(file.because[0].kind, "source");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nesting_stops_at_the_depth_limit() {
        let dir = fixture_dir("depth");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir);
        let after = build(&dir, GREETER_V1, PEOPLE_BODY, true);
        let at_limit = explain_rollup(&after, &dir, "src", MAX_DEPTH).unwrap();
        assert_eq!(kinds(&at_limit), ["child"]);
        assert!(at_limit.causes[0].because.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_spec_is_missing_and_a_current_one_has_no_causes() {
        let dir = fixture_dir("status");
        let graph = build(&dir, GREETER_V1, PEOPLE_V1, true);
        for id in [
            "src/people.rs::Polite",
            "src/people.rs",
            "rollup:src",
            "system",
        ] {
            let e = explain(&graph, &dir, id).unwrap();
            assert_eq!(e.status, "missing", "{id}");
            assert!(e.causes.is_empty(), "{id}");
        }
        generate_all(&graph, &dir);
        for id in [
            "src/people.rs::Polite",
            "src/people.rs",
            "rollup:src",
            "system",
        ] {
            let e = explain(&graph, &dir, id).unwrap();
            assert_eq!(e.status, "current", "{id}");
            assert!(e.causes.is_empty(), "{id}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_unknown_id_is_an_error() {
        let dir = fixture_dir("unknown");
        let graph = build(&dir, GREETER_V1, PEOPLE_V1, true);
        assert!(explain(&graph, &dir, "src/nope.rs").is_err());
        assert!(explain(&graph, &dir, "feature:nope").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_feature_names_the_participant_that_changed() {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-explain-test-{}-feature",
            std::process::id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        let files = [
            (
                "app/submit/page.tsx",
                "import { getSupabase } from '../../lib/supabase';\nexport default function Page() {\n  fetch(\"/api/submit-artwork\");\n  getSupabase();\n  return null;\n}\n",
            ),
            (
                "app/api/submit-artwork/route.ts",
                "import { getSupabase } from '../../../lib/supabase';\nexport async function POST(): Promise<void> {\n  getSupabase();\n}\n",
            ),
            (
                "lib/supabase.ts",
                "export function getSupabase(): void {}\n",
            ),
        ];
        for (rel, content) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        let graph = crate::index::RepoIndex::build(&dir)
            .unwrap()
            .rebuild()
            .unwrap();
        spec::submit_feature(
            &graph,
            &dir,
            "submit",
            "# Artwork submission\n## Summary\nLets an artist submit artwork.\n",
        )
        .unwrap();
        let current = explain(&graph, &dir, "feature:submit").unwrap();
        assert_eq!(current.status, "current");

        std::fs::write(
            dir.join("app/submit/page.tsx"),
            "import { getSupabase } from '../../lib/supabase';\nexport default function Page() {\n  fetch(\"/api/submit-artwork\");\n  getSupabase();\n  return 1;\n}\n",
        )
        .unwrap();
        let after = crate::index::RepoIndex::build(&dir)
            .unwrap()
            .rebuild()
            .unwrap();
        let e = explain(&after, &dir, "feature:submit").unwrap();
        assert_eq!(e.kind, "feature");
        assert_eq!(e.status, "stale");
        assert_eq!(kinds(&e), ["participant"]);
        assert_eq!(e.causes[0].target.as_deref(), Some("app/submit/page.tsx"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_corpus_summary_counts_causes_and_names_the_busiest_target() {
        let dir = fixture_dir("corpus");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir);

        // Nothing moved: documents are counted, none is stale.
        let quiet = explain_corpus(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir).unwrap();
        assert_eq!(quiet.stale, 0);
        assert!(quiet.documents >= 6, "{quiet:?}");
        assert!(quiet.by_cause.is_empty());

        // The trait's signature changes: the trait's own spec and file go
        // stale on their text, the importer's symbol and file go stale on
        // the trait, and the folder and system specs on their stale child.
        let after = build(&dir, GREETER_SIG, PEOPLE_V1, true);
        let s = explain_corpus(&after, &dir).unwrap();
        let count = |label: &str| {
            s.by_cause
                .iter()
                .find(|(l, _)| l == label)
                .map_or(0, |(_, n)| *n)
        };
        assert_eq!(count("source"), 2, "{s:?}");
        assert_eq!(count("dependency"), 2, "{s:?}");
        assert_eq!(count("child"), 2, "{s:?}");
        assert_eq!(s.stale, 6, "{s:?}");
        // Two documents rest on the trait that moved.
        assert_eq!(
            s.top_targets,
            vec![("src/greeter.rs::Greeter".to_string(), 2)]
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_corpus_summary_counts_dependants_resting_on_a_coarse_target() {
        let dir = fixture_dir("coarse-count");
        let graph = build(&dir, GREETER_PRIVATE_V1, PEOPLE_V1, true);
        // `Greeter` is private here, so it has no interface hash and the
        // importer's dependency hash uses its whole text.
        assert_eq!(coarse_dependants(&graph), 1);
        let graph = build(&dir, GREETER_V1, PEOPLE_V1, true);
        assert_eq!(coarse_dependants(&graph), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_unreadable_source_is_unknown_not_a_pile_of_removed_dependencies() {
        let dir = fixture_dir("unreadable");
        generate_all(&build(&dir, GREETER_V1, PEOPLE_V1, true), &dir);
        let graph = build(&dir, GREETER_V1, PEOPLE_V1, true);
        std::fs::remove_file(dir.join("src/people.rs")).unwrap();
        let e = explain(&graph, &dir, "src/people.rs::Polite").unwrap();
        assert_eq!(e.status, "stale");
        assert!(
            e.causes.iter().all(|c| c.kind != "removed"),
            "{:?}",
            kinds(&e)
        );
        assert!(e.causes.iter().any(|c| c.kind == "unknown"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_file_with_a_symbol_section_still_to_write_is_stale_with_a_child_cause() {
        let dir = fixture_dir("child-missing");
        let graph = build(&dir, GREETER_V1, PEOPLE_V1, true);
        spec::submit(&graph, &dir, "src/greeter.rs::Greeter", SYM).unwrap();
        spec::submit(&graph, &dir, "src/greeter.rs", FILE).unwrap();
        spec::submit(&graph, &dir, "src/people.rs", FILE).unwrap();
        // The file summary matches the code, but `Polite` has no section.
        let e = explain(&graph, &dir, "src/people.rs").unwrap();
        assert_eq!(e.status, "stale");
        assert_eq!(kinds(&e), ["child"]);
        assert_eq!(e.causes[0].target.as_deref(), Some("src/people.rs::Polite"));
        // A file whose own text moved is explained by that alone.
        let after = build(&dir, GREETER_V1, PEOPLE_BODY, true);
        let e = explain(&after, &dir, "src/people.rs").unwrap();
        assert_eq!(kinds(&e), ["source"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_member_of_a_container_is_missing_not_an_error() {
        let dir = fixture_dir("member");
        let graph = build(&dir, GREETER_V1, PEOPLE_V1, true);
        generate_all(&graph, &dir);
        let member = [
            "src/greeter.rs::Greeter::greet",
            "src/people.rs::Polite::greet",
        ]
        .into_iter()
        .find(|id| graph.find(id).is_some())
        .expect("the fixture has a member symbol");
        let e = explain(&graph, &dir, member).unwrap();
        assert_eq!(e.status, "missing");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_change_entry_splits_at_its_first_colon_only() {
        assert_eq!(
            split_change("changed:src/a.rs"),
            Some(("changed", "src/a.rs"))
        );
        assert_eq!(
            split_change("added:src/a.rs::one"),
            Some(("added", "src/a.rs::one"))
        );
        assert_eq!(
            split_change("removed:rollup:src"),
            Some(("removed", "rollup:src"))
        );
        assert_eq!(split_change("nonsense"), None);
    }
}
