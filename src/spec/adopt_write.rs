//! The write half of adopting the specs a restructuring orphaned.
//!
//! `adopt::plan` says which orphaned prose has a successor. This module
//! turns that into actions and applies them: carry the prose to the new id,
//! drop the entries it was moved out of, and remove an orphaned document
//! once everything in it has been carried or has no successor. Planning is
//! separate from applying so the dry run prints exactly what `--write` does.
//!
//! Writes only under the explicit command; `get_spec` stays a pure read.
//! See `DECISIONS.md` question 26 for the rules this encodes.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context, Result};

use super::adopt::{AdoptionPlan, Finding, Successor, file_of};
use super::file_task::{
    file_prose_is_human_edited, store_symbol_spec, symbol_prose_hash, symbol_prose_is_human_edited,
};
use super::format::{HashPair, SymbolProse, read_file_spec, write as write_file_spec};
use super::layout::{spec_bearing_children, spec_path};
use super::smells::prose_smells;
use super::staleness::dependency_record;
use crate::graph::Graph;

/// How a carried spec is stored at the new id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Stamped with the successor's current hashes: the spec reads current.
    Current,
    /// Stored with the hashes it was written against, which no longer match,
    /// so it reads stale and the writer is handed the old prose to revise.
    Draft,
}

/// One spec to carry to a new id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adoption {
    pub old_id: String,
    pub new_id: String,
    pub mode: Mode,
    pub prose: SymbolProse,
    pub old_hashes: HashPair,
}

/// Why an orphaned symbol's prose is not carried. It stays where it is, and
/// its document is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skip {
    /// A human edited the prose; it is never moved without them.
    HandEdited,
    /// The prose fails the quality check.
    Smelly(Vec<String>),
    /// The successor is not a top-level symbol, so its file's spec has no
    /// section to hold it.
    NotTopLevel,
    /// Several orphans would be written to one id.
    Claimed,
    /// The successor already has different prose of its own.
    Documented,
    /// The prose names a file this repo no longer has.
    NamesVanishedFile,
    /// No single successor: several candidates, or several same-name ones.
    NoSingleSuccessor,
}

/// What happens to one orphaned symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {
    Adopt(Adoption),
    /// The successor already holds this very prose (an earlier run got that
    /// far): nothing to write.
    AlreadyThere,
    /// Its code is gone and nothing corresponds to it: nothing to carry.
    Gone,
    Skip(Skip),
}

/// Everything a write run would do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WritePlan {
    /// By old id, in the order of the adoption plan.
    pub dispositions: Vec<(String, Disposition)>,
    /// Orphaned file documents everything in which has been carried or has
    /// no successor.
    pub remove_docs: Vec<String>,
    /// Orphaned file documents that stay, with the reason.
    pub keep_docs: Vec<(String, String)>,
    /// Sections to drop from a file that still exists, because their prose
    /// moved to another file.
    pub remove_sections: Vec<String>,
}

/// What a write run did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplyReport {
    pub adopted_current: usize,
    pub adopted_draft: usize,
    pub removed_docs: usize,
    pub removed_sections: usize,
}

/// The write plan's counts. Every orphaned symbol has exactly one
/// disposition, so the first eleven add up to the number of orphaned symbols.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriteSummary {
    pub current: usize,
    pub draft: usize,
    pub already_there: usize,
    pub gone: usize,
    pub hand_edited: usize,
    pub smelly: usize,
    pub not_top_level: usize,
    pub claimed: usize,
    pub documented: usize,
    pub names_vanished_file: usize,
    pub no_single_successor: usize,
    pub docs_removed: usize,
    pub docs_kept: usize,
    pub sections_removed: usize,
}

impl WritePlan {
    pub fn summary(&self) -> WriteSummary {
        let mut s = WriteSummary {
            docs_removed: self.remove_docs.len(),
            docs_kept: self.keep_docs.len(),
            sections_removed: self.remove_sections.len(),
            ..WriteSummary::default()
        };
        for (_, d) in &self.dispositions {
            match d {
                Disposition::Adopt(a) if a.mode == Mode::Current => s.current += 1,
                Disposition::Adopt(_) => s.draft += 1,
                Disposition::AlreadyThere => s.already_there += 1,
                Disposition::Gone => s.gone += 1,
                Disposition::Skip(Skip::HandEdited) => s.hand_edited += 1,
                Disposition::Skip(Skip::Smelly(_)) => s.smelly += 1,
                Disposition::Skip(Skip::NotTopLevel) => s.not_top_level += 1,
                Disposition::Skip(Skip::Claimed) => s.claimed += 1,
                Disposition::Skip(Skip::Documented) => s.documented += 1,
                Disposition::Skip(Skip::NamesVanishedFile) => s.names_vanished_file += 1,
                Disposition::Skip(Skip::NoSingleSuccessor) => s.no_single_successor += 1,
            }
        }
        s
    }

    /// The text `codeowl adopt-moved-specs` prints for the write step.
    /// `done` is false for a dry run (every verb is "would"); `list` adds the
    /// documents that stay, the symbols not carried, and the drafts.
    pub fn report(&self, done: bool, list: bool) -> String {
        let s = self.summary();
        let v = |would: &str, did: &str| {
            if done {
                did.to_string()
            } else {
                would.to_string()
            }
        };
        let mut out = format!(
            "write step{}:\n\
             \x20 {} as current: {}\n\
             \x20 {} as a stale draft (the writer is handed the old prose): {}\n\
             \x20 already carried by an earlier run: {}\n\
             \x20 no successor, nothing to carry: {}\n\
             \x20 not carried: hand-edited {}, fails the quality check {}, successor not a top-level symbol {}, \
             several orphans one target {}, successor documented otherwise {}, names a vanished file {}, \
             no single successor {}\n\
             \x20 orphaned documents {}: {}, kept: {}\n\
             \x20 sections {} from files that still exist: {}\n",
            if done {
                ""
            } else {
                " (dry run: nothing written)"
            },
            v("would carry", "carried"),
            s.current,
            v("would carry", "carried"),
            s.draft,
            s.already_there,
            s.gone,
            s.hand_edited,
            s.smelly,
            s.not_top_level,
            s.claimed,
            s.documented,
            s.names_vanished_file,
            s.no_single_successor,
            v("that would be removed", "removed"),
            s.docs_removed,
            s.docs_kept,
            v("would be removed", "removed"),
            s.sections_removed,
        );
        if list {
            out.push('\n');
            for (doc, why) in &self.keep_docs {
                out.push_str(&format!("kept {doc}: {why}\n"));
            }
            for (old, d) in &self.dispositions {
                let line = match d {
                    Disposition::Adopt(a) => {
                        format!("{old} -> {} ({:?})", a.new_id, a.mode)
                    }
                    Disposition::AlreadyThere => format!("{old} already carried"),
                    Disposition::Gone => format!("{old} no successor"),
                    Disposition::Skip(why) => format!("{old} not carried: {}", why.label()),
                };
                out.push_str(&line);
                out.push('\n');
            }
        }
        out
    }
}

impl Skip {
    fn label(&self) -> String {
        match self {
            Skip::HandEdited => "hand-edited".to_string(),
            Skip::Smelly(smells) => format!("fails the quality check ({})", smells.join(", ")),
            Skip::NotTopLevel => "successor is not a top-level symbol".to_string(),
            Skip::Claimed => "several orphans share one target".to_string(),
            Skip::Documented => "successor already has other prose".to_string(),
            Skip::NamesVanishedFile => "prose names a file that is gone".to_string(),
            Skip::NoSingleSuccessor => "no single successor".to_string(),
        }
    }
}

/// Decide what a write run would do. Reads, never writes.
pub fn plan_writes(graph: &Graph, root: &Path, plan: &AdoptionPlan) -> Result<WritePlan> {
    let mut dispositions: Vec<(String, Disposition)> = Vec::new();
    for finding in &plan.findings {
        dispositions.push((finding.old_id.clone(), decide(graph, root, finding)?));
    }

    // Two orphans carried to one id would overwrite each other.
    let mut targets: HashMap<String, usize> = HashMap::new();
    for (_, d) in &dispositions {
        if let Disposition::Adopt(a) = d {
            *targets.entry(a.new_id.clone()).or_default() += 1;
        }
    }
    for (_, d) in dispositions.iter_mut() {
        if matches!(d, Disposition::Adopt(a) if targets[&a.new_id] > 1) {
            *d = Disposition::Skip(Skip::Claimed);
        }
    }

    let mut remove_docs = Vec::new();
    let mut keep_docs = Vec::new();
    for doc in &plan.orphaned_file_docs {
        let not_carried: Vec<&str> = dispositions
            .iter()
            .filter(|(old, d)| file_of(old) == doc && matches!(d, Disposition::Skip(_)))
            .map(|(old, _)| old.as_str())
            .collect();
        if !not_carried.is_empty() {
            keep_docs.push((
                doc.clone(),
                format!(
                    "{} symbol(s) not carried: {}",
                    not_carried.len(),
                    not_carried.join(", ")
                ),
            ));
        } else if has_unlisted_section(root, doc)? {
            keep_docs.push((
                doc.clone(),
                "it holds a section with no hash record".to_string(),
            ));
        } else if file_summary_was_edited(root, doc)? {
            keep_docs.push((
                doc.clone(),
                "its file summary was edited by hand".to_string(),
            ));
        } else {
            remove_docs.push(doc.clone());
        }
    }

    // A section carried out of a file that still exists goes from it too,
    // and so does one whose code is gone: with no successor there is nothing
    // to carry, and nothing else removes it until that file is next written.
    let remove_sections = dispositions
        .iter()
        .filter(|(old, d)| {
            matches!(
                d,
                Disposition::Adopt(_) | Disposition::AlreadyThere | Disposition::Gone
            ) && !plan
                .orphaned_file_docs
                .iter()
                .any(|doc| doc == file_of(old))
        })
        .map(|(old, _)| old.clone())
        .collect();

    Ok(WritePlan {
        dispositions,
        remove_docs,
        keep_docs,
        remove_sections,
    })
}

/// What to do with one orphaned symbol.
fn decide(graph: &Graph, root: &Path, finding: &Finding) -> Result<Disposition> {
    if symbol_prose_is_human_edited(&finding.old_hashes, &finding.prose) {
        return Ok(Disposition::Skip(Skip::HandEdited));
    }
    let prose = cleaned(&finding.prose);
    let (new_id, mode) = match &finding.successor {
        Successor::None => return Ok(Disposition::Gone),
        Successor::Ambiguous(_) => return Ok(Disposition::Skip(Skip::NoSingleSuccessor)),
        Successor::Contested(_) => return Ok(Disposition::Skip(Skip::Claimed)),
        Successor::Occupied(id) => {
            return Ok(match already_documented(root, id, &prose)? {
                Some(done) => done,
                None => Disposition::Skip(Skip::Documented),
            });
        }
        Successor::TextDiffers(ids) => match ids.as_slice() {
            [only] => (only.clone(), Mode::Draft),
            _ => return Ok(Disposition::Skip(Skip::NoSingleSuccessor)),
        },
        Successor::One { id, .. } if finding.adoptable() => (id.clone(), Mode::Current),
        Successor::One { .. } if finding.draft_only() => {
            return Ok(Disposition::Skip(Skip::NamesVanishedFile));
        }
        Successor::One { id, .. } => (id.clone(), Mode::Draft),
    };
    if !is_top_level_spec_bearing(graph, &new_id) {
        return Ok(Disposition::Skip(Skip::NotTopLevel));
    }
    if let Some(done) = already_documented(root, &new_id, &prose)? {
        return Ok(done);
    }
    if mode == Mode::Current {
        let smells: Vec<String> = prose_smells(&prose.summary)
            .into_iter()
            .map(|s| format!("Summary: {s}"))
            .chain(
                prose_smells(&prose.behavior)
                    .into_iter()
                    .map(|s| format!("Behavior: {s}")),
            )
            .collect();
        if !smells.is_empty() {
            return Ok(Disposition::Skip(Skip::Smelly(smells)));
        }
    }
    Ok(Disposition::Adopt(Adoption {
        old_id: finding.old_id.clone(),
        new_id,
        mode,
        prose,
        old_hashes: finding.old_hashes.clone(),
    }))
}

/// `prose` without the closing tag an earlier generator left in some of it.
fn cleaned(prose: &SymbolProse) -> SymbolProse {
    let strip = |text: &str| text.replace("</content>", "").trim().to_string();
    SymbolProse {
        summary: strip(&prose.summary),
        behavior: strip(&prose.behavior),
    }
}

/// If `id` already has written prose: `AlreadyThere` when it is this very
/// prose (an earlier run got that far), `Skip(Documented)` when it is
/// something else. `None` when `id` has none.
fn already_documented(root: &Path, id: &str, prose: &SymbolProse) -> Result<Option<Disposition>> {
    let Some(spec) = read_file_spec(root, file_of(id))? else {
        return Ok(None);
    };
    let Some((_, existing)) = spec.sections.iter().find(|(sid, _)| sid == id) else {
        return Ok(None);
    };
    if existing.summary.trim().is_empty() && existing.behavior.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(if existing == prose {
        Disposition::AlreadyThere
    } else {
        Disposition::Skip(Skip::Documented)
    }))
}

/// Whether the document has more `## \`name\`` sections than it has hash
/// records. The parser only reads a section whose id is in the front matter,
/// so the planner never saw the extra one, and removing the document would
/// delete text nobody evaluated.
fn has_unlisted_section(root: &Path, source_path: &str) -> Result<bool> {
    let path = spec_path(root, source_path);
    let Some(spec) = read_file_spec(root, source_path)? else {
        return Ok(false);
    };
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let sections = text.lines().filter(|l| l.starts_with("## `")).count();
    Ok(sections > spec.symbols.len())
}

fn file_summary_was_edited(root: &Path, source_path: &str) -> Result<bool> {
    let Some(spec) = read_file_spec(root, source_path)? else {
        return Ok(false);
    };
    Ok(!spec.file.spec_hash.is_empty()
        && file_prose_is_human_edited(&spec.file, &spec.file_summary))
}

/// Whether `id` is a top-level callable or container of its file: the only
/// symbols whose file spec has a section to hold prose.
pub(super) fn is_top_level_spec_bearing(graph: &Graph, id: &str) -> bool {
    let Some(sym_id) = graph.find(id) else {
        return false;
    };
    let Some(file_id) = graph.get_symbol(sym_id).and_then(|s| s.parent) else {
        return false;
    };
    graph.get_file(file_id).is_some() && spec_bearing_children(graph, file_id).contains(&sym_id)
}

/// Do it.
pub fn apply(graph: &Graph, root: &Path, plan: &WritePlan) -> Result<ApplyReport> {
    let mut report = ApplyReport::default();

    for (_, d) in &plan.dispositions {
        let Disposition::Adopt(a) = d else {
            continue;
        };
        let sym_id = graph
            .find(&a.new_id)
            .with_context(|| format!("successor {} is gone", a.new_id))?;
        let sym = graph
            .get_symbol(sym_id)
            .with_context(|| format!("{} is not a symbol", a.new_id))?;
        let file_id = sym.parent.context("successor has no containing file")?;
        let hash = match a.mode {
            Mode::Current => {
                let (deps_hash, dep_targets) = dependency_record(graph, root, file_id, sym);
                HashPair {
                    source_hash: sym.source_hash.clone(),
                    fingerprint: String::new(),
                    deps_hash,
                    spec_hash: symbol_prose_hash(&a.prose.summary, &a.prose.behavior),
                    dep_targets,
                }
            }
            // The record it was written against, which no longer matches.
            Mode::Draft => HashPair {
                source_hash: a.old_hashes.source_hash.clone(),
                fingerprint: String::new(),
                deps_hash: a.old_hashes.deps_hash.clone(),
                spec_hash: symbol_prose_hash(&a.prose.summary, &a.prose.behavior),
                dep_targets: a.old_hashes.dep_targets.clone(),
            },
        };
        store_symbol_spec(graph, root, sym, a.prose.clone(), hash)?;
        match a.mode {
            Mode::Current => report.adopted_current += 1,
            Mode::Draft => report.adopted_draft += 1,
        }
    }

    let mut by_file: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for old in &plan.remove_sections {
        by_file.entry(file_of(old)).or_default().push(old);
    }
    for (file, ids) in by_file {
        let (Some(file_id), Some(mut spec)) = (graph.find(file), read_file_spec(root, file)?)
        else {
            continue;
        };
        let before = spec.symbols.len();
        spec.symbols.retain(|(id, _)| !ids.contains(&id.as_str()));
        spec.sections.retain(|(id, _)| !ids.contains(&id.as_str()));
        let removed = before - spec.symbols.len();
        if removed > 0 {
            write_file_spec(root, graph, file_id, &spec)?;
            report.removed_sections += removed;
        }
    }

    for doc in &plan.remove_docs {
        let path = spec_path(root, doc);
        std::fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
        report.removed_docs += 1;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::adopt::plan as adoption_plan;
    use crate::spec::file_task::next_task;
    use crate::spec::format::read_file_spec;
    use crate::spec::layout::spec_path;
    use crate::spec::test_support::{
        PROSE, documented, plant_smelly_symbol_spec, rebuild_graph as rebuild, write_file as write,
    };
    use crate::spec::{SpecTask, symbol_changes};
    use std::path::PathBuf;

    const F: &str = "export function f(): number { return 1; }\n";
    const H_OLD: &str = "export function g(x: number): number { return x; }\n";
    const H_NEW: &str = "export function g(x: string): string { return x; }\n";
    const USES_G: &str =
        "import { g } from \"./h\";\nexport function f(): number { return g(1); }\n";

    /// Plan and apply against whatever is on disk now.
    fn run(dir: &Path) -> (Graph, WritePlan, ApplyReport) {
        let graph = rebuild(dir);
        let wp = plan_writes(&graph, dir, &adoption_plan(&graph, dir).unwrap()).unwrap();
        let report = apply(&graph, dir, &wp).unwrap();
        (graph, wp, report)
    }

    fn disposition<'a>(wp: &'a WritePlan, old: &str) -> &'a Disposition {
        &wp.dispositions
            .iter()
            .find(|(id, _)| id == old)
            .unwrap_or_else(|| panic!("no disposition for {old}: {:?}", wp.dispositions))
            .1
    }

    /// `f` documented in lib/a.ts, then moved to lib/b.ts unchanged.
    fn moved(tag: &str) -> PathBuf {
        let dir = documented(tag, &[("lib/a.ts", F)], &["lib/a.ts::f"], PROSE);
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);
        dir
    }

    fn snapshot(dir: &Path) -> Vec<(String, String)> {
        fn walk(dir: &Path, out: &mut Vec<(String, String)>) {
            let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().collect();
            entries.sort_by_key(|e| e.path());
            for e in entries {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push((
                        p.display().to_string(),
                        std::fs::read_to_string(&p).unwrap_or_default(),
                    ));
                }
            }
        }
        let mut out = Vec::new();
        walk(&dir.join("docs"), &mut out);
        out
    }

    #[test]
    fn current_adoption_stores_the_prose_at_the_new_id_with_current_hashes() {
        let dir = moved("w-current");
        let (graph, wp, report) = run(&dir);

        assert!(matches!(
            disposition(&wp, "lib/a.ts::f"),
            Disposition::Adopt(a) if a.mode == Mode::Current && a.new_id == "lib/b.ts::f"
        ));
        assert_eq!((report.adopted_current, report.adopted_draft), (1, 0));

        let spec = read_file_spec(&dir, "lib/b.ts").unwrap().unwrap();
        let (_, prose) = spec
            .sections
            .iter()
            .find(|(id, _)| id == "lib/b.ts::f")
            .expect("the section was written");
        assert_eq!(prose.summary, "Does the one thing it is named for.");
        let stored = &spec.symbols[0].1;
        let sym = graph
            .get_symbol(graph.find("lib/b.ts::f").unwrap())
            .unwrap();
        let file_id = graph.find("lib/b.ts").unwrap();
        assert!(
            symbol_changes(&graph, &dir, file_id, sym, stored).is_empty(),
            "stamped with the successor's hashes, so it reads current"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_document_whose_every_symbol_was_carried_is_removed() {
        let dir = moved("w-remove");
        assert!(spec_path(&dir, "lib/a.ts").exists());
        let (_, wp, report) = run(&dir);
        assert_eq!(wp.remove_docs, vec!["lib/a.ts".to_string()]);
        assert_eq!(report.removed_docs, 1);
        assert!(!spec_path(&dir, "lib/a.ts").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_document_with_a_symbol_that_could_not_be_carried_is_kept() {
        // f has one successor; g's text is in two files and nothing picks one.
        let g = "export function g(): number { return 9; }\n";
        let both = format!("{F}{g}");
        let dir = documented(
            "w-keep",
            &[("lib/a.ts", both.as_str())],
            &["lib/a.ts::f", "lib/a.ts::g"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);
        write(&dir, "lib/c.ts", g);
        write(&dir, "lib/d.ts", g);

        let (_, wp, report) = run(&dir);
        assert!(matches!(
            disposition(&wp, "lib/a.ts::f"),
            Disposition::Adopt(_)
        ));
        assert_eq!(
            disposition(&wp, "lib/a.ts::g"),
            &Disposition::Skip(Skip::NoSingleSuccessor)
        );
        assert!(wp.remove_docs.is_empty());
        assert_eq!(wp.keep_docs.len(), 1);
        assert_eq!(wp.keep_docs[0].0, "lib/a.ts");
        assert_eq!((report.adopted_current, report.removed_docs), (1, 0));
        assert!(
            spec_path(&dir, "lib/a.ts").exists(),
            "g's prose is still there"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hand_edited_prose_is_not_carried_and_its_document_stays() {
        let dir = moved("w-hand");
        let path = spec_path(&dir, "lib/a.ts");
        let edited = std::fs::read_to_string(&path).unwrap().replace(
            "Does the one thing it is named for.",
            "A person reworded this summary.",
        );
        std::fs::write(&path, edited).unwrap();

        let (_, wp, report) = run(&dir);
        assert_eq!(
            disposition(&wp, "lib/a.ts::f"),
            &Disposition::Skip(Skip::HandEdited)
        );
        assert_eq!(report.adopted_current + report.adopted_draft, 0);
        assert!(path.exists());
        assert!(!spec_path(&dir, "lib/b.ts").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_changed_dependency_is_carried_as_a_stale_draft_that_offers_the_old_prose() {
        let dir = documented(
            "w-dep",
            &[("lib/h.ts", H_OLD), ("lib/a.ts", USES_G)],
            &["lib/a.ts::f"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", USES_G);
        write(&dir, "lib/h.ts", H_NEW);

        let (graph, wp, report) = run(&dir);
        assert!(matches!(
            disposition(&wp, "lib/a.ts::f"),
            Disposition::Adopt(a) if a.mode == Mode::Draft
        ));
        assert_eq!((report.adopted_current, report.adopted_draft), (0, 1));

        let spec = read_file_spec(&dir, "lib/b.ts").unwrap().unwrap();
        let sym = graph
            .get_symbol(graph.find("lib/b.ts::f").unwrap())
            .unwrap();
        let file_id = graph.find("lib/b.ts").unwrap();
        assert!(
            !symbol_changes(&graph, &dir, file_id, sym, &spec.symbols[0].1).is_empty(),
            "the dependency changed, so it reads stale"
        );
        match next_task(&graph, &dir, file_id).unwrap().unwrap() {
            SpecTask::Symbol { id, previous, .. } => {
                assert_eq!(id, "lib/b.ts::f");
                assert_eq!(
                    previous.expect("the old prose is offered").summary,
                    "Does the one thing it is named for."
                );
            }
            other => panic!("expected the symbol task, got {other:?}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_same_name_with_edited_text_is_carried_as_a_stale_draft() {
        let dir = documented("w-edited", &[("lib/a.ts", F)], &["lib/a.ts::f"], PROSE);
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(
            &dir,
            "lib/b.ts",
            "export function f(): number { return 2; }\n",
        );

        let (graph, wp, report) = run(&dir);
        assert!(matches!(
            disposition(&wp, "lib/a.ts::f"),
            Disposition::Adopt(a) if a.mode == Mode::Draft && a.new_id == "lib/b.ts::f"
        ));
        assert_eq!(report.adopted_draft, 1);
        let spec = read_file_spec(&dir, "lib/b.ts").unwrap().unwrap();
        let sym = graph
            .get_symbol(graph.find("lib/b.ts::f").unwrap())
            .unwrap();
        let changes = symbol_changes(
            &graph,
            &dir,
            graph.find("lib/b.ts").unwrap(),
            sym,
            &spec.symbols[0].1,
        );
        assert!(
            changes.contains(&"changed:source".to_string()),
            "{changes:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prose_naming_a_vanished_file_is_left_for_a_person_and_keeps_its_document() {
        let prose = "### Summary\nSame idea as lib/a.ts used to hold.\n### Behavior\nReturns the number one with no side effects.\n";
        let dir = documented("w-vanished", &[("lib/a.ts", F)], &["lib/a.ts::f"], prose);
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);

        let (_, wp, report) = run(&dir);
        assert_eq!(
            disposition(&wp, "lib/a.ts::f"),
            &Disposition::Skip(Skip::NamesVanishedFile)
        );
        assert_eq!(report.adopted_current + report.adopted_draft, 0);
        assert!(spec_path(&dir, "lib/a.ts").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_section_moved_out_of_a_file_that_still_exists_is_removed_from_it() {
        let k = "export function k(): number { return 2; }\n";
        let both = format!("{F}{k}");
        let dir = documented(
            "w-section",
            &[("lib/a.ts", both.as_str())],
            &["lib/a.ts::f", "lib/a.ts::k"],
            PROSE,
        );
        write(&dir, "lib/a.ts", k);
        write(&dir, "lib/b.ts", F);

        let (_, wp, report) = run(&dir);
        assert_eq!(wp.remove_sections, vec!["lib/a.ts::f".to_string()]);
        assert_eq!((report.adopted_current, report.removed_sections), (1, 1));
        let left = read_file_spec(&dir, "lib/a.ts").unwrap().unwrap();
        assert!(left.sections.iter().all(|(id, _)| id != "lib/a.ts::f"));
        assert!(
            left.sections.iter().any(|(id, _)| id == "lib/a.ts::k"),
            "k did not move, its prose stays"
        );
        assert!(
            wp.remove_docs.is_empty(),
            "the document itself is still live"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_section_for_code_deleted_from_a_file_that_still_exists_is_removed_too() {
        // f was deleted outright, not moved: there is nothing to carry, and
        // nothing else would remove its section until the file is rewritten.
        let k = "export function k(): number { return 2; }\n";
        let both = format!("{F}{k}");
        let dir = documented(
            "w-gone-section",
            &[("lib/a.ts", both.as_str())],
            &["lib/a.ts::f", "lib/a.ts::k"],
            PROSE,
        );
        write(&dir, "lib/a.ts", k);

        let (_, wp, report) = run(&dir);
        assert_eq!(disposition(&wp, "lib/a.ts::f"), &Disposition::Gone);
        assert_eq!(wp.remove_sections, vec!["lib/a.ts::f".to_string()]);
        assert_eq!(report.removed_sections, 1);
        let left = read_file_spec(&dir, "lib/a.ts").unwrap().unwrap();
        assert!(left.sections.iter().all(|(id, _)| id != "lib/a.ts::f"));
        assert!(left.sections.iter().any(|(id, _)| id == "lib/a.ts::k"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_section_with_no_hash_entry_keeps_its_document() {
        // The planner only sees prose that has a hash record. A section
        // without one was never looked at, so deleting its document would
        // destroy text nothing evaluated.
        let g = "export function g(): number { return 9; }\n";
        let both = format!("{F}{g}");
        let dir = documented(
            "w-hashless",
            &[("lib/a.ts", both.as_str())],
            &["lib/a.ts::f", "lib/a.ts::g"],
            PROSE,
        );
        // A hand-added section the parser never reads (no hash record).
        let path = spec_path(&dir, "lib/a.ts");
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str("\n## `extra`\n### Summary\nNotes written by hand.\n### Behavior\nKept.\n");
        std::fs::write(&path, text).unwrap();
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);

        let (_, wp, _) = run(&dir);
        assert!(wp.remove_docs.is_empty(), "{:?}", wp.remove_docs);
        assert_eq!(wp.keep_docs.len(), 1);
        assert!(wp.keep_docs[0].1.contains("no hash record"));
        assert!(spec_path(&dir, "lib/a.ts").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_section_that_is_already_absent_is_not_counted_as_removed() {
        let k = "export function k(): number { return 2; }\n";
        let both = format!("{F}{k}");
        let dir = documented(
            "w-count",
            &[("lib/a.ts", both.as_str())],
            &["lib/a.ts::f", "lib/a.ts::k"],
            PROSE,
        );
        write(&dir, "lib/a.ts", k);
        let (graph, wp, first) = run(&dir);
        assert_eq!(first.removed_sections, 1);
        // The same plan again, as after an interrupted run.
        let again = apply(&graph, &dir, &wp).unwrap();
        assert_eq!(again.removed_sections, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_stray_closing_tag_is_stripped_before_the_prose_is_carried() {
        let (graph, dir) =
            crate::spec::test_support::build_feature_fixture(&[("lib/a.ts", F)], "w-markup");
        plant_smelly_symbol_spec(
            &graph,
            &dir,
            "lib/a.ts::f",
            "Does the one thing it is named for.",
            "Runs synchronously and has no side effects.</content>",
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);

        let (_, wp, _) = run(&dir);
        assert!(matches!(
            disposition(&wp, "lib/a.ts::f"),
            Disposition::Adopt(a) if a.mode == Mode::Current
        ));
        let spec = read_file_spec(&dir, "lib/b.ts").unwrap().unwrap();
        assert_eq!(
            spec.sections[0].1.behavior,
            "Runs synchronously and has no side effects."
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_successor_with_different_prose_of_its_own_is_left_alone() {
        // lib/b.ts::f was documented too, differently; then lib/a.ts is deleted.
        let (graph, dir) = crate::spec::test_support::build_feature_fixture(
            &[("lib/a.ts", F), ("lib/b.ts", F)],
            "w-documented",
        );
        crate::spec::file_task::submit(&graph, &dir, "lib/a.ts::f", PROSE).unwrap();
        crate::spec::file_task::submit(
            &graph,
            &dir,
            "lib/b.ts::f",
            "### Summary\nA different account written for this one.\n### Behavior\nIt returns one and touches nothing else.\n",
        )
        .unwrap();
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();

        let (_, wp, report) = run(&dir);
        assert_eq!(
            disposition(&wp, "lib/a.ts::f"),
            &Disposition::Skip(Skip::Documented)
        );
        assert_eq!(report.adopted_current + report.adopted_draft, 0);
        let kept = read_file_spec(&dir, "lib/b.ts").unwrap().unwrap();
        assert_eq!(
            kept.sections[0].1.summary,
            "A different account written for this one."
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn orphans_that_would_share_one_target_are_not_carried() {
        let h = "export function helper(): number { return 7; }\n";
        let dir = documented(
            "w-claimed",
            &[("lib/a.ts", h), ("lib/b.ts", h)],
            &["lib/a.ts::helper", "lib/b.ts::helper"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        std::fs::remove_file(dir.join("lib/b.ts")).unwrap();
        write(&dir, "lib/c.ts", h);

        let (_, wp, report) = run(&dir);
        for old in ["lib/a.ts::helper", "lib/b.ts::helper"] {
            assert_eq!(disposition(&wp, old), &Disposition::Skip(Skip::Claimed));
        }
        assert_eq!(report.adopted_current + report.adopted_draft, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_symbol_with_no_successor_does_not_block_removing_its_document() {
        let g = "export function gone(): number { return 5; }\n";
        let both = format!("{F}{g}");
        let dir = documented(
            "w-gone",
            &[("lib/a.ts", both.as_str())],
            &["lib/a.ts::f", "lib/a.ts::gone"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);

        let (_, wp, _) = run(&dir);
        assert_eq!(disposition(&wp, "lib/a.ts::gone"), &Disposition::Gone);
        assert_eq!(wp.remove_docs, vec!["lib/a.ts".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn running_it_twice_changes_nothing_the_second_time() {
        let dir = moved("w-twice");
        let (_, _, first) = run(&dir);
        assert_eq!(first.adopted_current, 1);
        let after_first = snapshot(&dir);

        let (_, wp, second) = run(&dir);
        assert!(wp.dispositions.is_empty(), "nothing is orphaned any more");
        assert_eq!(
            (
                second.adopted_current,
                second.removed_docs,
                second.removed_sections
            ),
            (0, 0, 0)
        );
        assert_eq!(snapshot(&dir), after_first);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_rerun_after_a_crash_before_the_prune_finishes_the_job() {
        let dir = moved("w-crash");
        let old_doc = std::fs::read_to_string(spec_path(&dir, "lib/a.ts")).unwrap();
        run(&dir);
        // The run died after writing the successor and before removing the
        // old document: put it back.
        write(&dir, "docs/specs/lib/a.ts.md", &old_doc);

        let (_, wp, report) = run(&dir);
        assert_eq!(disposition(&wp, "lib/a.ts::f"), &Disposition::AlreadyThere);
        assert_eq!(
            (
                report.adopted_current,
                report.adopted_draft,
                report.removed_docs
            ),
            (0, 0, 1)
        );
        assert!(!spec_path(&dir, "lib/a.ts").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn planning_writes_nothing() {
        let dir = moved("w-dry");
        let before = snapshot(&dir);
        let graph = rebuild(&dir);
        let wp = plan_writes(&graph, &dir, &adoption_plan(&graph, &dir).unwrap()).unwrap();
        assert!(!wp.dispositions.is_empty());
        assert_eq!(snapshot(&dir), before);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_write_summary_counts_each_disposition_once() {
        let adopt = |mode| {
            Disposition::Adopt(Adoption {
                old_id: "a::1".into(),
                new_id: "b::1".into(),
                mode,
                prose: SymbolProse::default(),
                old_hashes: HashPair::default(),
            })
        };
        let wp = WritePlan {
            dispositions: vec![
                ("a::1".into(), adopt(Mode::Current)),
                ("a::2".into(), adopt(Mode::Current)),
                ("a::3".into(), adopt(Mode::Draft)),
                ("a::4".into(), Disposition::AlreadyThere),
                ("a::5".into(), Disposition::Gone),
                ("a::6".into(), Disposition::Skip(Skip::HandEdited)),
                (
                    "a::7".into(),
                    Disposition::Skip(Skip::Smelly(vec!["x".into()])),
                ),
                ("a::8".into(), Disposition::Skip(Skip::NotTopLevel)),
                ("a::9".into(), Disposition::Skip(Skip::Claimed)),
                ("a::10".into(), Disposition::Skip(Skip::Documented)),
                ("a::11".into(), Disposition::Skip(Skip::NamesVanishedFile)),
                ("a::12".into(), Disposition::Skip(Skip::NoSingleSuccessor)),
            ],
            remove_docs: vec!["a.ts".into()],
            keep_docs: vec![("b.ts".into(), "why".into()), ("c.ts".into(), "why".into())],
            remove_sections: vec!["a::1".into()],
        };
        let s = wp.summary();
        assert_eq!((s.current, s.draft, s.already_there, s.gone), (2, 1, 1, 1));
        assert_eq!(
            (s.hand_edited, s.smelly, s.not_top_level, s.claimed),
            (1, 1, 1, 1)
        );
        assert_eq!(
            (s.documented, s.names_vanished_file, s.no_single_successor),
            (1, 1, 1)
        );
        assert_eq!((s.docs_removed, s.docs_kept, s.sections_removed), (1, 2, 1));
        assert_eq!(
            s.current
                + s.draft
                + s.already_there
                + s.gone
                + s.hand_edited
                + s.smelly
                + s.not_top_level
                + s.claimed
                + s.documented
                + s.names_vanished_file
                + s.no_single_successor,
            wp.dispositions.len(),
            "every orphaned symbol has exactly one disposition"
        );

        let dry = wp.report(false, false);
        assert!(dry.contains("would carry as current: 2"), "{dry}");
        let done = wp.report(true, false);
        assert!(done.contains("carried as current: 2"), "{done}");
        assert!(!dry.contains("a::6"));
        assert!(
            wp.report(false, true)
                .contains("a::6 not carried: hand-edited")
        );
    }

    #[test]
    fn only_a_top_level_function_or_class_can_hold_a_section() {
        let src = "export function top(): number { return 1; }\nexport class C {\n  m(): number { return 2; }\n}\n";
        let (graph, dir) =
            crate::spec::test_support::build_feature_fixture(&[("lib/c.ts", src)], "w-top");
        assert!(is_top_level_spec_bearing(&graph, "lib/c.ts::top"));
        assert!(is_top_level_spec_bearing(&graph, "lib/c.ts::C"));
        assert!(!is_top_level_spec_bearing(&graph, "lib/c.ts::C::m"));
        assert!(!is_top_level_spec_bearing(&graph, "lib/c.ts::missing"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
