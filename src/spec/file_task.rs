//! The symbol-then-file generation loop for one file: which unit needs a spec next, and persisting what the writer submitted.

use std::path::Path;

use anyhow::{Context, Result};

use super::format::{FileSpec, HashPair, SymbolProse, extract_section, read_existing, write};
use super::layout::{file_is_spec_bearing, spec_bearing_children};
use super::smells::{prose_smells, reject_if_smelly};
use super::staleness::{dependency_record, file_changes, file_dependency_record, symbol_changes};
use crate::graph::{Graph, Node, SymbolId};
use crate::hash::hash_text;
use crate::symbol::Symbol;

/// One unit of work `/codeowl generate <id>` still needs a spec for,
/// bottom-up: a file's top-level symbols before the file itself (see
/// `ARCHITECTURE.md`'s "Recursive spec generation" — containment recurses,
/// reference edges don't).
#[derive(Debug, Clone, PartialEq)]
pub enum SpecTask {
    Symbol {
        id: String,
        signature: String,
        docstring: Option<String>,
        /// `Some` only for a reconciliation regeneration (M8's "Human
        /// corrections" case 4: source moved *and* a human had edited this
        /// symbol's prose) — the human's prior text, to preserve whatever
        /// correction is still accurate rather than silently discard it.
        /// `None` for a first-ever generation or a plain case-2
        /// regeneration (no human edit on record).
        prior: Option<SymbolProse>,
        /// `Some` when the source moved and no human had edited the prose:
        /// the last machine-written text, for the writer to revise rather
        /// than redraft from nothing. Never set together with `prior` (a
        /// human's text is the one to preserve), never set for a
        /// first-ever generation, and never set for prose the quality
        /// check flagged (that text is the thing being replaced).
        previous: Option<SymbolProse>,
    },
    File {
        id: String,
        /// Same case-4 meaning as `Symbol::prior`, for the file's own
        /// `## Summary`.
        prior: Option<String>,
        /// Same meaning as `Symbol::previous`, for the file's own
        /// `## Summary`.
        previous: Option<String>,
    },
}

/// Whether `spec`'s currently-parsed prose for `id` no longer matches the
/// `spec_hash` last recorded for it — i.e. a human edited the `.md` file
/// directly (never through `submit_spec`) since it was last machine-
/// written. `spec_hash` is defined as `hash_text` of exactly this prose at
/// generation time (see `submit`), so any drift between the two can only
/// come from an edit that didn't also fix up the hash — not something a
/// human would do by hand. See `ARCHITECTURE.md`'s "Human corrections".
/// The `spec_hash` of a symbol's prose: the one formula `submit` stores and
/// every human-edit check compares against.
pub(super) fn symbol_prose_hash(summary: &str, behavior: &str) -> String {
    hash_text(&format!("{summary}\n{behavior}"))
}

pub(super) fn symbol_prose_is_human_edited(hash: &HashPair, prose: &SymbolProse) -> bool {
    symbol_prose_hash(&prose.summary, &prose.behavior) != hash.spec_hash
}

pub(super) fn file_prose_is_human_edited(hash: &HashPair, file_summary: &str) -> bool {
    hash_text(file_summary) != hash.spec_hash
}

/// Case 3 of "Human corrections": source unchanged, but a human edited the
/// prose — refresh just this symbol's `spec_hash` to match their edit (no
/// LLM call, prose left exactly as the human wrote it) and persist it.
fn reconcile_symbol_hash(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    sym_id: &str,
    hash: &HashPair,
    prose: &SymbolProse,
) -> Result<()> {
    let mut new_hash = hash.clone();
    new_hash.spec_hash = hash_text(&format!("{}\n{}", prose.summary, prose.behavior));
    let Node::File(file) = graph.get(file_id) else {
        return Ok(());
    };
    let mut spec = read_existing(root, &file.id)?.unwrap_or_else(|| FileSpec::blank(&file.id));
    upsert(&mut spec.symbols, sym_id.to_string(), new_hash);
    reorder_symbols(graph, file_id, &mut spec);
    write(root, graph, file_id, &spec)
}

/// The file-level equivalent of `reconcile_symbol_hash`.
fn reconcile_file_hash(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    spec: &FileSpec,
) -> Result<()> {
    let mut fixed = spec.clone();
    fixed.file.spec_hash = hash_text(&spec.file_summary);
    write(root, graph, file_id, &fixed)
}

/// The next thing `/codeowl generate <target_file_id>` needs written, or
/// `None` if the file isn't spec-bearing at all, or everything on it is
/// already current. Stateless and idempotent from a caller's perspective
/// (safe to call repeatedly — the client-side generate loop's own
/// termination condition), though it may itself write a small housekeeping
/// fix to disk: case 3 of "Human corrections" (source unchanged, prose
/// hand-edited) is reconciled silently here, with no LLM call and no task
/// returned for it, rather than surfaced as something needing generation.
pub fn next_task(graph: &Graph, root: &Path, target_file_id: SymbolId) -> Result<Option<SpecTask>> {
    if !file_is_spec_bearing(graph, target_file_id) {
        return Ok(None);
    }
    let file = graph
        .get_file(target_file_id)
        .context("generate target is not a file id")?;
    let existing = read_existing(root, &file.id)?;

    for sym_id in spec_bearing_children(graph, target_file_id) {
        let sym = graph.get_symbol(sym_id).expect("filtered to symbol ids");
        let Some(hash) = existing.as_ref().and_then(|spec| spec.symbol_hash(&sym.id)) else {
            // Never generated at all -- not a reconciliation case, just a
            // first-ever generation.
            return Ok(Some(SpecTask::Symbol {
                id: sym.id.clone(),
                signature: sym.signature.clone(),
                docstring: sym.docstring.clone(),
                prior: None,
                previous: None,
            }));
        };
        let source_changed = !symbol_changes(graph, root, target_file_id, sym, hash).is_empty();
        let prose = existing
            .as_ref()
            .and_then(|spec| spec.section(&sym.id).cloned())
            .unwrap_or_default();
        let human_edited = symbol_prose_is_human_edited(hash, &prose);

        match (source_changed, human_edited) {
            (false, false) => {
                // Case 1 by the four-case reconciliation rules, but a
                // quality smell is a real, hash-invisible reason to
                // revisit this symbol anyway -- see "Quality smells" in
                // ARCHITECTURE.md. No `prior` here: there's nothing to
                // reconcile against, just a plain "please rewrite this."
                let smelly = !prose_smells(&prose.summary).is_empty()
                    || !prose_smells(&prose.behavior).is_empty();
                if smelly {
                    return Ok(Some(SpecTask::Symbol {
                        id: sym.id.clone(),
                        signature: sym.signature.clone(),
                        docstring: sym.docstring.clone(),
                        prior: None,
                        previous: None,
                    }));
                }
                continue;
            }
            (true, false) => {
                // case 2: regeneration with no human edit to preserve. The
                // last machine draft still goes along, so the writer
                // revises it instead of redrafting from the source alone
                // (and quietly dropping a caveat the old text had). Not
                // when the old text is itself flagged (a cop-out stub, or
                // an empty half, which counts as too short): that is the
                // thing being replaced, not something to keep.
                let clean = prose_smells(&prose.summary).is_empty()
                    && prose_smells(&prose.behavior).is_empty();
                return Ok(Some(SpecTask::Symbol {
                    id: sym.id.clone(),
                    signature: sym.signature.clone(),
                    docstring: sym.docstring.clone(),
                    prior: None,
                    previous: clean.then_some(prose),
                }));
            }
            (false, true) => {
                // case 3: reconcile silently, no task
                reconcile_symbol_hash(graph, root, target_file_id, &sym.id, hash, &prose)?;
            }
            (true, true) => {
                // case 4: reconciliation regeneration, preserve the prior
                return Ok(Some(SpecTask::Symbol {
                    id: sym.id.clone(),
                    signature: sym.signature.clone(),
                    docstring: sym.docstring.clone(),
                    prior: Some(prose),
                    previous: None,
                }));
            }
        }
    }

    // `existing` may already exist purely because a symbol was submitted
    // (which lazily creates the `FileSpec` via `FileSpec::blank`) even
    // though the file's own entry -- a single `HashPair`, not a map keyed
    // lookup -- has never itself been written. An empty `spec_hash` is
    // that "never generated" signal, the file-level analogue of a
    // symbol's `symbol_hash` returning `None`.
    let file_generated = existing
        .as_ref()
        .is_some_and(|spec| !spec.file.spec_hash.is_empty());
    if let Some(spec) = existing.as_ref().filter(|_| file_generated) {
        let source_changed = !file_changes(graph, target_file_id, &spec.file).is_empty();
        let human_edited = file_prose_is_human_edited(&spec.file, &spec.file_summary);
        match (source_changed, human_edited) {
            (false, false) => {
                if !prose_smells(&spec.file_summary).is_empty() {
                    return Ok(Some(SpecTask::File {
                        id: file.id.clone(),
                        prior: None,
                        previous: None,
                    }));
                }
                return Ok(None);
            }
            (true, false) => {
                return Ok(Some(SpecTask::File {
                    id: file.id.clone(),
                    prior: None,
                    // Same rule as for a symbol: a flagged summary is not
                    // handed over as text to keep.
                    previous: prose_smells(&spec.file_summary)
                        .is_empty()
                        .then(|| spec.file_summary.clone()),
                }));
            }
            (false, true) => {
                reconcile_file_hash(graph, root, target_file_id, spec)?;
                return Ok(None);
            }
            (true, true) => {
                return Ok(Some(SpecTask::File {
                    id: file.id.clone(),
                    prior: Some(spec.file_summary.clone()),
                    previous: None,
                }));
            }
        }
    }

    Ok(Some(SpecTask::File {
        id: file.id.clone(),
        prior: None,
        previous: None,
    }))
}

/// Persist `content` (the agent's LLM-written prose) for `id` — a symbol
/// id (expects `### Summary` and `### Behavior` headings in `content`) or
/// a file id (expects plain prose, becomes `## Summary`). Returns the
/// hashes the persisted entry now carries.
pub fn submit(graph: &Graph, root: &Path, id: &str, content: &str) -> Result<HashPair> {
    let node_id = graph
        .find(id)
        .with_context(|| format!("unknown id {id:?}"))?;
    match graph.get(node_id) {
        Node::File(file) => {
            let file_id = node_id;
            let mut spec =
                read_existing(root, &file.id)?.unwrap_or_else(|| FileSpec::blank(&file.id));
            let summary = content.trim().to_string();
            reject_if_smelly(
                &format!("submitted summary for {id:?}"),
                prose_smells(&summary),
            )?;
            let (deps_hash, dep_targets) = file_dependency_record(graph, file_id);
            let hash = HashPair {
                source_hash: file.source_hash.clone(),
                fingerprint: file.fingerprint.clone(),
                deps_hash,
                spec_hash: hash_text(&summary),
                dep_targets,
            };
            spec.file_summary = summary;
            spec.file = hash.clone();
            reorder_symbols(graph, file_id, &mut spec);
            write(root, graph, file_id, &spec)?;
            Ok(hash)
        }
        Node::Symbol(sym) => {
            let file_id = sym
                .parent
                .context("symbol has no containing file to attach its spec to")?;
            let summary = extract_section(content, "### Summary", "\n### ")
                .filter(|s| !s.is_empty())
                .context("submitted content missing a non-empty `### Summary` section")?;
            let behavior = extract_section(content, "### Behavior", "\n### ")
                .filter(|s| !s.is_empty())
                .context("submitted content missing a non-empty `### Behavior` section")?;
            reject_if_smelly(
                &format!("submitted prose for {id:?}"),
                prose_smells(&summary)
                    .into_iter()
                    .map(|s| format!("Summary: {s}"))
                    .chain(
                        prose_smells(&behavior)
                            .into_iter()
                            .map(|s| format!("Behavior: {s}")),
                    )
                    .collect(),
            )?;

            let (deps_hash, dep_targets) = dependency_record(graph, root, file_id, sym);
            let hash = HashPair {
                source_hash: sym.source_hash.clone(),
                fingerprint: String::new(),
                deps_hash,
                spec_hash: symbol_prose_hash(&summary, &behavior),
                dep_targets,
            };
            store_symbol_spec(
                graph,
                root,
                sym,
                SymbolProse { summary, behavior },
                hash.clone(),
            )?;
            Ok(hash)
        }
    }
}

/// Put `prose` and its `hash` in `sym`'s section of its file's spec, creating
/// the file spec (with a blank file summary) if there is none, and write it.
/// The one place a symbol's entry reaches disk, so `submit` and the adoption
/// of moved specs cannot store it differently.
pub(super) fn store_symbol_spec(
    graph: &Graph,
    root: &Path,
    sym: &Symbol,
    prose: SymbolProse,
    hash: HashPair,
) -> Result<()> {
    let file_id = sym
        .parent
        .context("symbol has no containing file to attach its spec to")?;
    let file = graph
        .get_file(file_id)
        .context("symbol's parent is not a file")?;
    let mut spec = read_existing(root, &file.id)?.unwrap_or_else(|| FileSpec::blank(&file.id));
    upsert(&mut spec.symbols, sym.id.clone(), hash);
    upsert_section(&mut spec.sections, sym.id.clone(), prose);
    reorder_symbols(graph, file_id, &mut spec);
    write(root, graph, file_id, &spec)
}

fn upsert(entries: &mut Vec<(String, HashPair)>, id: String, value: HashPair) {
    match entries.iter_mut().find(|(eid, _)| *eid == id) {
        Some((_, v)) => *v = value,
        None => entries.push((id, value)),
    }
}

fn upsert_section(entries: &mut Vec<(String, SymbolProse)>, id: String, value: SymbolProse) {
    match entries.iter_mut().find(|(eid, _)| *eid == id) {
        Some((_, v)) => *v = value,
        None => entries.push((id, value)),
    }
}

/// Re-sort `spec.symbols`/`spec.sections` to match the file's current
/// declaration order, dropping entries for symbols no longer present.
/// Runs on every write so reordering or removing a top-level symbol in
/// source never leaves a stale entry behind.
fn reorder_symbols(graph: &Graph, file_id: SymbolId, spec: &mut FileSpec) {
    let order = spec_bearing_children(graph, file_id);
    let mut symbols = Vec::with_capacity(order.len());
    let mut sections = Vec::with_capacity(order.len());
    for id in order {
        let sym = graph.get_symbol(id).expect("filtered to symbol ids");
        if let Some(h) = spec.symbol_hash(&sym.id) {
            symbols.push((sym.id.clone(), h.clone()));
        }
        if let Some(p) = spec.section(&sym.id) {
            sections.push((sym.id.clone(), p.clone()));
        }
    }
    spec.symbols = symbols;
    spec.sections = sections;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::build_graph_from_sources;
    use crate::spec::format::{HashPair, SymbolProse, read_existing, read_file_spec};
    use crate::spec::layout::spec_path;
    use crate::spec::staleness::{
        hash_dependency_pairs, try_dependency_pairs, try_file_dependency_pairs,
    };
    use crate::spec::task_input::summaries_input;
    use crate::spec::test_support::{greeter_people_graph, plant_smelly_symbol_spec, temp_dir_for};

    const MIXED_TS: &str = "export interface Shape { sides: number }\nexport const LIMIT_VALUE = 5;\nexport function first(): void {}\nexport function second(): void {}\n";

    #[test]
    fn submitting_a_symbol_records_its_dependency_targets() {
        let (graph, dir) = greeter_people_graph("rec-sym");
        let file_id = graph.find("src/people.rs").unwrap();
        let polite = graph
            .get_symbol(graph.find("src/people.rs::Polite").unwrap())
            .unwrap();
        let content = "### Summary\nGreets people politely by name.\n### Behavior\nReturns a polite greeting string for the caller.\n";
        let returned = submit(&graph, &dir, "src/people.rs::Polite", content).unwrap();

        // The pairs are the symbol's real dependency, hash shortened to 12 hex.
        let full = try_dependency_pairs(&graph, &dir, file_id, polite).unwrap();
        assert_eq!(full.len(), 1);
        assert_eq!(full[0].0, "src/greeter.rs::Greeter");
        let expected: Vec<(String, String)> = full
            .iter()
            .map(|(id, h)| (id.clone(), h[..12].to_string()))
            .collect();
        assert_eq!(returned.dep_targets, expected);

        // And it was persisted, and still agrees with the combined hash.
        let spec = read_existing(&dir, "src/people.rs").unwrap().unwrap();
        let stored = spec.symbol_hash("src/people.rs::Polite").unwrap();
        assert_eq!(stored.dep_targets, expected);
        assert_eq!(hash_dependency_pairs(&full), stored.deps_hash);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submitting_a_file_records_its_dependency_targets() {
        let (graph, dir) = greeter_people_graph("rec-file");
        let file_id = graph.find("src/people.rs").unwrap();
        let returned = submit(
            &graph,
            &dir,
            "src/people.rs",
            "A small module defining a polite greeting person.",
        )
        .unwrap();

        let full = try_file_dependency_pairs(&graph, file_id).unwrap();
        assert_eq!(full.len(), 1);
        let expected: Vec<(String, String)> = full
            .iter()
            .map(|(id, h)| (id.clone(), h[..12].to_string()))
            .collect();
        assert_eq!(returned.dep_targets, expected);
        let spec = read_existing(&dir, "src/people.rs").unwrap().unwrap();
        assert_eq!(spec.file.dep_targets, expected);
        assert_eq!(hash_dependency_pairs(&full), spec.file.deps_hash);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn adopting_a_human_edit_keeps_the_dependency_targets() {
        // Case 3 of "Human corrections" refreshes only `spec_hash`; the
        // recorded targets are about what the spec was *written from*, and
        // a hand-edit of its prose does not change that.
        let (graph, dir) = greeter_people_graph("keep-deps");
        let file_id = graph.find("src/people.rs").unwrap();
        let hash = HashPair {
            source_hash: "sh".to_string(),
            fingerprint: String::new(),
            deps_hash: "sd".to_string(),
            spec_hash: "old".to_string(),
            dep_targets: vec![(
                "src/greeter.rs::Greeter".to_string(),
                "a1b2c3d4e5f6".to_string(),
            )],
        };
        let prose = SymbolProse {
            summary: "Hand-edited summary text.".to_string(),
            behavior: "Hand-edited behavior text here.".to_string(),
        };
        reconcile_symbol_hash(
            &graph,
            &dir,
            file_id,
            "src/people.rs::Polite",
            &hash,
            &prose,
        )
        .unwrap();
        let spec = read_existing(&dir, "src/people.rs").unwrap().unwrap();
        let stored = spec.symbol_hash("src/people.rs::Polite").unwrap();
        assert_eq!(stored.dep_targets, hash.dep_targets);
        assert_ne!(stored.spec_hash, "old");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn next_task_returns_first_uncovered_symbol_then_the_file() {
        let dir = std::env::temp_dir().join(format!("codeowl-spec-test-{}-1", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let graph = build_graph_from_sources(&[(
            "a.ts",
            "export function one(): void {}\nexport function two(): void {}\n",
        )]);
        let file_id = graph.find("a.ts").unwrap();

        let task = next_task(&graph, &dir, file_id).unwrap().unwrap();
        assert_eq!(
            task,
            SpecTask::Symbol {
                id: "a.ts::one".to_string(),
                signature: "function one(): void".to_string(),
                docstring: None,
                prior: None,
                previous: None,
            }
        );

        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nDoes one specific, deliberate thing.\n### Behavior\nIntentionally performs no operation.\n",
        )
        .unwrap();
        let task = next_task(&graph, &dir, file_id).unwrap().unwrap();
        assert_eq!(
            task,
            SpecTask::Symbol {
                id: "a.ts::two".to_string(),
                signature: "function two(): void".to_string(),
                docstring: None,
                prior: None,
                previous: None,
            }
        );

        submit(
            &graph,
            &dir,
            "a.ts::two",
            "### Summary\nDoes a second, related thing.\n### Behavior\nIntentionally performs no operation.\n",
        )
        .unwrap();
        let task = next_task(&graph, &dir, file_id).unwrap().unwrap();
        assert_eq!(
            task,
            SpecTask::File {
                id: "a.ts".to_string(),
                prior: None,
                previous: None,
            }
        );

        submit(
            &graph,
            &dir,
            "a.ts",
            "A file with two deliberate no-op helpers.",
        )
        .unwrap();
        assert_eq!(next_task(&graph, &dir, file_id).unwrap(), None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn next_task_is_none_for_a_barrel_file() {
        let dir = std::env::temp_dir().join(format!("codeowl-spec-test-{}-2", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let graph = build_graph_from_sources(&[("a.ts", "export { Foo } from './foo';\n")]);
        let file_id = graph.find("a.ts").unwrap();

        assert_eq!(next_task(&graph, &dir, file_id).unwrap(), None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn next_task_is_none_for_a_generated_file_with_a_real_uncovered_symbol() {
        // M19: get_next_spec_task targeting a generated file directly
        // must report nothing to do, even though it has a genuinely
        // uncovered exported symbol -- the same "kind: done" outcome a
        // barrel file gets, for a different reason (reference-only, not
        // "nothing to say").
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-gen", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let path = "target/generated-sources/foo/HeroesResource.java";
        let mut graph =
            build_graph_from_sources(&[(path, "export function getAllHeroes(): void {}\n")]);
        graph.set_pack_name("java");
        let file_id = graph.find(path).unwrap();

        assert_eq!(next_task(&graph, &dir, file_id).unwrap(), None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_file_spec_write_that_cannot_complete_leaves_the_previous_spec_untouched() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-atomic1", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let graph = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nDoes one specific thing.\n### Behavior\nRuns synchronously with no side effects.\n",
        )
        .unwrap();
        submit(&graph, &dir, "a.ts", "Holds the first specific helper.").unwrap();
        let path = spec_path(&dir, "a.ts");
        let before = std::fs::read_to_string(&path).unwrap();

        // The writer stages into `<path>.tmp` and renames over the spec. A
        // directory squatting on that name makes the staging step fail,
        // standing in for any failure before the rename (disk full, a
        // crash): the old spec must still be the whole old spec.
        std::fs::create_dir_all(format!("{}.tmp", path.display())).unwrap();
        let result = submit(&graph, &dir, "a.ts", "Holds a different, rewritten helper.");

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submit_rejects_a_symbol_missing_required_sections() {
        let dir = std::env::temp_dir().join(format!("codeowl-spec-test-{}-3", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let graph = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        let result = submit(&graph, &dir, "a.ts::one", "just some prose, no headings");
        assert!(result.is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submit_rejects_prose_that_next_task_would_re_offer_forever() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-smelly", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let graph = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        let file_id = graph.find("a.ts").unwrap();

        // A Summary under the 4-word `prose_smells` floor — storing it would
        // make the next `next_task` call re-offer `a.ts::one` indefinitely.
        let err = submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nToo terse.\n\
             ### Behavior\nThis behavior sentence is comfortably long enough to pass.\n",
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("quality check"), "{msg}");
        assert!(msg.contains("Summary: suspiciously_short"), "{msg}");

        // A "see the source" cop-out is rejected even at full length.
        let err = submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nReturns the one canonical value for this module.\n\
             ### Behavior\nIt does the obvious thing; see the source for the exact steps.\n",
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("Behavior: cop_out_phrase"),
            "{err}"
        );

        // Nothing was persisted: it's still a first-ever generation task.
        assert!(matches!(
            next_task(&graph, &dir, file_id).unwrap(),
            Some(SpecTask::Symbol { ref id, prior: None, .. }) if id == "a.ts::one"
        ));

        // Real prose is accepted and the loop then terminates.
        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nReturns the one canonical value this module exposes.\n\
             ### Behavior\nReads a cached slot, initialising it on first call, never mutating after.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "a.ts",
            "The single-value helper module used across the app.",
        )
        .unwrap();
        assert_eq!(next_task(&graph, &dir, file_id).unwrap(), None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submit_rejects_a_smelly_file_summary() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-smellyf", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let graph = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nReturns the one canonical value this module exposes.\n\
             ### Behavior\nReads a cached slot, initialising it on first call, never mutating after.\n",
        )
        .unwrap();
        let err = submit(&graph, &dir, "a.ts", "Three short words.").unwrap_err();
        assert!(err.to_string().contains("suspiciously_short"), "{err}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resubmitting_the_same_symbol_after_no_source_change_leaves_next_task_past_it() {
        let dir = std::env::temp_dir().join(format!("codeowl-spec-test-{}-4", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let graph = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        let file_id = graph.find("a.ts").unwrap();

        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nDoes one small, specific job.\n### Behavior\nRuns without any side effects.\n",
        )
        .unwrap();
        // Re-running generate against the same, unchanged graph should
        // skip straight past the symbol (already current) to the file.
        let task = next_task(&graph, &dir, file_id).unwrap().unwrap();
        assert_eq!(
            task,
            SpecTask::File {
                id: "a.ts".to_string(),
                prior: None,
                previous: None,
            }
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn human_edited_prose_with_source_unchanged_reconciles_silently() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-human1", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let graph = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        let file_id = graph.find("a.ts").unwrap();
        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nThis is the original summary.\n### Behavior\nThis is the original behavior.\n",
        )
        .unwrap();

        // A human hand-edits the spec file's prose directly -- never
        // through submit_spec -- leaving the frontmatter hashes as they
        // were (a human wouldn't hand-update an opaque blake3 hash).
        let path = spec_path(&dir, "a.ts");
        let content = std::fs::read_to_string(&path).unwrap();
        let edited = content.replace(
            "This is the original summary.",
            "A careful human-corrected summary here.",
        );
        assert_ne!(edited, content);
        std::fs::write(&path, &edited).unwrap();

        // Source hasn't changed -- case 3: reconcile silently (no task for
        // the symbol; spec_hash refreshed to match the human's edit) and
        // move on to the file, which was never generated at all.
        let task = next_task(&graph, &dir, file_id).unwrap();
        assert_eq!(
            task,
            Some(SpecTask::File {
                id: "a.ts".to_string(),
                prior: None,
                previous: None,
            })
        );

        let reread = read_file_spec(&dir, "a.ts").unwrap().unwrap();
        assert_eq!(
            reread.sections[0].1.summary,
            "A careful human-corrected summary here."
        );
        let expected_hash =
            hash_text("A careful human-corrected summary here.\nThis is the original behavior.");
        assert_eq!(reread.symbols[0].1.spec_hash, expected_hash);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn human_edited_prose_with_source_changed_returns_a_reconciliation_task() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-human2", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let source_v1 = "export function one(): void {}\n";
        let graph_v1 = build_graph_from_sources(&[("a.ts", source_v1)]);
        submit(
            &graph_v1,
            &dir,
            "a.ts::one",
            "### Summary\nThis is the original summary.\n### Behavior\nThis is the original behavior.\n",
        )
        .unwrap();

        let path = spec_path(&dir, "a.ts");
        let content = std::fs::read_to_string(&path).unwrap();
        let edited = content.replace(
            "This is the original summary.",
            "A careful human-corrected summary here.",
        );
        std::fs::write(&path, &edited).unwrap();

        // The underlying source ALSO changes -- case 4: a reconciliation
        // regeneration, carrying the human's prior text forward rather
        // than silently discarding it.
        let source_v2 = "export function one(): void {\n  console.log('changed');\n}\n";
        let graph_v2 = build_graph_from_sources(&[("a.ts", source_v2)]);
        let file_id_v2 = graph_v2.find("a.ts").unwrap();

        let task = next_task(&graph_v2, &dir, file_id_v2).unwrap();
        assert_eq!(
            task,
            Some(SpecTask::Symbol {
                id: "a.ts::one".to_string(),
                signature: "function one(): void".to_string(),
                docstring: None,
                prior: Some(SymbolProse {
                    summary: "A careful human-corrected summary here.".to_string(),
                    behavior: "This is the original behavior.".to_string(),
                }),
                previous: None,
            })
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_source_change_with_no_human_edit_carries_the_last_machine_draft_as_previous() {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-previous-symbol",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let graph_v1 = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        submit(
            &graph_v1,
            &dir,
            "a.ts::one",
            "### Summary\nThis is the original summary.\n### Behavior\nThis is the original behavior.\n",
        )
        .unwrap();

        // The source moves and nobody touched the prose: the writer is
        // handed the last machine draft to revise, in `previous`, and
        // `prior` stays empty because there is no human edit to preserve.
        let source_v2 = "export function one(): void {\n  console.log('changed');\n}\n";
        let graph_v2 = build_graph_from_sources(&[("a.ts", source_v2)]);
        let file_id_v2 = graph_v2.find("a.ts").unwrap();

        let task = next_task(&graph_v2, &dir, file_id_v2).unwrap();
        assert_eq!(
            task,
            Some(SpecTask::Symbol {
                id: "a.ts::one".to_string(),
                signature: "function one(): void".to_string(),
                docstring: None,
                prior: None,
                previous: Some(SymbolProse {
                    summary: "This is the original summary.".to_string(),
                    behavior: "This is the original behavior.".to_string(),
                }),
            })
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_source_change_with_no_human_edit_carries_the_last_file_summary_as_previous() {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-previous-file",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let graph_v1 = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        submit(
            &graph_v1,
            &dir,
            "a.ts::one",
            "### Summary\nThis is the original summary.\n### Behavior\nThis is the original behavior.\n",
        )
        .unwrap();
        submit(
            &graph_v1,
            &dir,
            "a.ts",
            "A file with one small helper function.",
        )
        .unwrap();

        let source_v2 = "export function one(): void {\n  console.log('changed');\n}\n";
        let graph_v2 = build_graph_from_sources(&[("a.ts", source_v2)]);
        let file_id_v2 = graph_v2.find("a.ts").unwrap();
        // The changed symbol comes first; write it so the file is next.
        submit(
            &graph_v2,
            &dir,
            "a.ts::one",
            "### Summary\nThis is the revised summary.\n### Behavior\nThis is the revised behavior.\n",
        )
        .unwrap();

        let task = next_task(&graph_v2, &dir, file_id_v2).unwrap();
        assert_eq!(
            task,
            Some(SpecTask::File {
                id: "a.ts".to_string(),
                prior: None,
                previous: Some("A file with one small helper function.".to_string()),
            })
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_source_change_does_not_carry_a_flagged_symbol_draft_as_previous() {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-previous-smelly-symbol",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let graph_v1 = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        // The stored draft is a cop-out stub: it is the thing being
        // replaced, so handing it to the writer as text to keep would ask
        // it to preserve a defect.
        plant_smelly_symbol_spec(
            &graph_v1,
            &dir,
            "a.ts::one",
            "One does its one small job.",
            "See the source for details.",
        );

        let source_v2 = "export function one(): void {\n  console.log('changed');\n}\n";
        let graph_v2 = build_graph_from_sources(&[("a.ts", source_v2)]);
        let file_id_v2 = graph_v2.find("a.ts").unwrap();

        let task = next_task(&graph_v2, &dir, file_id_v2).unwrap();
        assert_eq!(
            task,
            Some(SpecTask::Symbol {
                id: "a.ts::one".to_string(),
                signature: "function one(): void".to_string(),
                docstring: None,
                prior: None,
                previous: None,
            })
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_source_change_does_not_carry_a_flagged_file_summary_as_previous() {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-previous-smelly-file",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let graph_v1 = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        submit(
            &graph_v1,
            &dir,
            "a.ts::one",
            "### Summary\nThis is the original summary.\n### Behavior\nThis is the original behavior.\n",
        )
        .unwrap();
        let clean = "A file with one small helper function.";
        let stub = "See the source for details.";
        submit(&graph_v1, &dir, "a.ts", clean).unwrap();
        // Turn the stored file summary into a hash-current cop-out stub.
        let path = spec_path(&dir, "a.ts");
        let patched = std::fs::read_to_string(&path)
            .unwrap()
            .replace(clean, stub)
            .replace(&hash_text(clean), &hash_text(stub));
        std::fs::write(&path, patched).unwrap();

        let source_v2 = "export function one(): void {\n  console.log('changed');\n}\n";
        let graph_v2 = build_graph_from_sources(&[("a.ts", source_v2)]);
        let file_id_v2 = graph_v2.find("a.ts").unwrap();
        submit(
            &graph_v2,
            &dir,
            "a.ts::one",
            "### Summary\nThis is the revised summary.\n### Behavior\nThis is the revised behavior.\n",
        )
        .unwrap();

        let task = next_task(&graph_v2, &dir, file_id_v2).unwrap();
        assert_eq!(
            task,
            Some(SpecTask::File {
                id: "a.ts".to_string(),
                prior: None,
                previous: None,
            })
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_smelly_but_hash_current_symbol_is_offered_again_by_next_task() {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-smelly-retrigger",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let graph = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        let file_id = graph.find("a.ts").unwrap();
        // A hash-current spec whose Behavior is a "see the source" cop-out
        // (the shape a pre-fix cache carries — `submit` rejects it now).
        plant_smelly_symbol_spec(
            &graph,
            &dir,
            "a.ts::one",
            "One does its one small job.",
            "See the source for details.",
        );

        // Nothing about the source changed and nobody hand-edited the
        // file -- by hash alone this is "case 1: current" -- but the
        // prose itself is a cop-out stub. next_task must still offer it
        // again (with no `prior`: there's nothing to reconcile against,
        // just a plain "please rewrite this").
        let task = next_task(&graph, &dir, file_id).unwrap();
        assert_eq!(
            task,
            Some(SpecTask::Symbol {
                id: "a.ts::one".to_string(),
                signature: "function one(): void".to_string(),
                docstring: None,
                prior: None,
                previous: None,
            })
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn summaries_input_lists_each_symbol_signature_with_its_written_summary_in_order() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-sumin", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = "// Top comment of the file.\nexport function first(): void {}\nexport function second(): void {}\n";
        let graph = build_graph_from_sources(&[("a.ts", src)]);
        let file_id = graph.find("a.ts").unwrap();
        submit(
            &graph,
            &dir,
            "a.ts::first",
            "### Summary\nDoes the first thing.\n### Behavior\nBEHAVIOR-ONE runs quietly and returns nothing.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "a.ts::second",
            "### Summary\nDoes the second thing.\n### Behavior\nBEHAVIOR-TWO runs quietly and returns nothing.\n",
        )
        .unwrap();
        submit(&graph, &dir, "a.ts", "FILE-LEVEL-SUMMARY must not leak in.").unwrap();

        let input = summaries_input(&graph, &dir, file_id, src)
            .unwrap()
            .expect("every symbol has a summary");
        let first = input.find("function first(").expect("first signature");
        let second = input.find("function second(").expect("second signature");
        assert!(first < second, "file order: {input}");
        assert!(input.contains("Does the first thing."));
        assert!(input.contains("Does the second thing."));
        assert!(
            input.contains("// Top comment of the file."),
            "module doc: {input}"
        );
        assert!(
            !input.contains("BEHAVIOR-ONE"),
            "summary only, not behavior"
        );
        assert!(
            !input.contains("FILE-LEVEL-SUMMARY"),
            "never the file's own summary"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn summaries_input_is_none_when_a_symbol_has_no_written_summary() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-sumnone", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = "export function first(): void {}\nexport function second(): void {}\n";
        let graph = build_graph_from_sources(&[("a.ts", src)]);
        let file_id = graph.find("a.ts").unwrap();
        submit(
            &graph,
            &dir,
            "a.ts::first",
            "### Summary\nDoes the first thing.\n### Behavior\nIt runs and returns nothing.\n",
        )
        .unwrap();
        // `second` was never generated: never present a partial input as whole.
        assert_eq!(summaries_input(&graph, &dir, file_id, src).unwrap(), None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn summaries_input_also_lists_the_declarations_that_have_no_summary() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-sumother", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let graph = build_graph_from_sources(&[("a.ts", MIXED_TS)]);
        let file_id = graph.find("a.ts").unwrap();
        for id in ["a.ts::first", "a.ts::second"] {
            submit(
                &graph,
                &dir,
                id,
                "### Summary\nDoes one specific thing.\n### Behavior\nIt runs and returns nothing at all.\n",
            )
            .unwrap();
        }

        let input = summaries_input(&graph, &dir, file_id, MIXED_TS)
            .unwrap()
            .expect("every spec-bearing symbol has a summary");
        let (_, others) = input
            .split_once("OTHER TOP-LEVEL DECLARATIONS")
            .unwrap_or_else(|| panic!("missing the other-declarations section: {input}"));
        assert!(others.contains("Shape"), "interface missing: {others}");
        assert!(others.contains("LIMIT_VALUE"), "constant missing: {others}");
        assert!(
            !others.contains("function first"),
            "summarised symbols are not repeated"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submitting_a_file_records_the_graphs_fingerprint_beside_the_raw_hash() {
        let dir = temp_dir_for("fp-submit");
        let mut graph = build_graph_from_sources(&[("a.ts", "export function f(): void {}\n")]);
        graph.set_file_fingerprint("a.ts", "fp-custom");
        let hash = submit(&graph, &dir, "a.ts", "A file that does one clear job.").unwrap();
        let raw = graph
            .get_file(graph.find("a.ts").unwrap())
            .unwrap()
            .source_hash
            .clone();
        assert_eq!(hash.fingerprint, "fp-custom");
        assert_eq!(hash.source_hash, raw, "the raw text hash is still recorded");

        let stored = read_file_spec(&dir, "a.ts").unwrap().unwrap();
        assert_eq!(stored.file.fingerprint, "fp-custom");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_file_symbol_submission_leaves_the_files_fingerprint_unset() {
        // Symbol entries never carry a fingerprint; only a file's own entry does.
        let dir = temp_dir_for("fp-symbol");
        let graph = build_graph_from_sources(&[("a.ts", "export function f(): void {}\n")]);
        let hash = submit(
            &graph,
            &dir,
            "a.ts::f",
            "### Summary\nDoes one specific thing.\n### Behavior\nIt runs and returns nothing at all.\n",
        )
        .unwrap();
        assert_eq!(hash.fingerprint, "");
        std::fs::remove_dir_all(&dir).ok();
    }
}
