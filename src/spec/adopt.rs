//! Finds the written prose a restructuring orphaned that still applies.
//!
//! A spec is keyed by the path-based id of the code it describes, so moving
//! a function leaves its spec orphaned at the old id and the new location
//! reading as missing. When the function's text did not change, the prose
//! is still true. This module is the read-only half of re-keying it: for
//! every orphaned symbol that has written prose it looks for a successor by
//! source hash, checks what the symbol depends on, and reports; it writes
//! nothing. See `experiments/exp-07-adopt-moved-specs.md`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::Result;

use super::format::{FileSpec, HashPair, SymbolProse, read_file_spec};
use super::orphans::{OrphanedSpec, find_orphaned_specs};
use super::staleness::{hash_dependency_pairs, shorten_dep_targets, try_dependency_pairs};
use crate::graph::Graph;
use crate::symbol::Symbol;

/// How the dependencies a symbol was documented against compare with its
/// successor's now. Compared by interface hash, never by id: `deps_hash`
/// hashes the target ids, so it cannot match once a dependency has moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deps {
    /// The same set of dependency hashes.
    Equal,
    /// Every recorded dependency hash is still present, and there are more.
    /// What a split produces: a call to a same-file helper was not a
    /// dependency, and once the helper lives in another module it is.
    OnlyAdditions,
    /// A recorded dependency hash is gone: something it relied on changed.
    Changed,
    /// The old spec recorded no dependency hashes and the successor has some,
    /// so there is nothing to compare.
    Unknown,
}

/// What became of one orphaned symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Successor {
    /// Exactly one current symbol has the same text (and, if several did,
    /// the same name and a path under the old file's stem, or the file where
    /// the rest of the old file went).
    One { id: String, deps: Deps },
    /// Several current symbols have the same text and nothing picks one.
    Ambiguous(Vec<String>),
    /// Several orphans resolve to this one symbol (two helpers merged into
    /// one). Only one spec can live at an id, so none is adopted.
    Contested(String),
    /// The successor already has written prose of its own; adopting would
    /// overwrite it.
    Occupied(String),
    /// No symbol has the same text, but these have the same name.
    TextDiffers(Vec<String>),
    /// Nothing in the current tree corresponds to it.
    None,
}

/// One orphaned symbol with written prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub old_id: String,
    pub successor: Successor,
    /// File-like names in the old summary or behavior that match no current
    /// file. Many are illustrations (`lib/utils.ts` as an example path).
    pub moved_paths_named: Vec<String>,
    /// The subset of those that name a file whose own spec is orphaned: a
    /// file this repo used to have. That is a stale reference, not an
    /// illustration.
    pub names_vanished_file: Vec<String>,
    /// The written prose the old spec held, and the hashes it was written
    /// against: what a write step carries over.
    pub prose: SymbolProse,
    pub old_hashes: HashPair,
}

impl Finding {
    /// One successor with unchanged dependencies, and prose that names no
    /// file this repo has lost: safe to carry over as current.
    pub fn adoptable(&self) -> bool {
        self.same_text_and_dependencies() && self.names_vanished_file.is_empty()
    }

    /// The same, except the prose names a file that is gone: it cannot be
    /// carried as current, and the write step leaves it, and its document,
    /// for a person to rewrite.
    pub fn draft_only(&self) -> bool {
        self.same_text_and_dependencies() && !self.names_vanished_file.is_empty()
    }

    fn same_text_and_dependencies(&self) -> bool {
        matches!(
            self.successor,
            Successor::One {
                deps: Deps::Equal | Deps::OnlyAdditions,
                ..
            }
        )
    }
}

/// Everything the planner found, in a stable order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdoptionPlan {
    pub findings: Vec<Finding>,
    /// Orphaned file documents (their file summaries describe a whole and
    /// are not adopted).
    pub orphaned_file_docs: Vec<String>,
    /// Orphaned rollup and feature documents: regenerated, not adopted, and
    /// counted so the report does not look like the whole orphan population.
    pub orphaned_other_docs: usize,
}

/// The plan's counts. Every orphaned symbol lands in exactly one of
/// `adoptable`, `draft_vanished_file`, `deps_changed`, `deps_unknown`,
/// `ambiguous`, `contested`, `occupied`, `text_differs` and `no_successor`;
/// `equal` and `only_additions` split `adoptable`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub adoptable: usize,
    pub equal: usize,
    pub only_additions: usize,
    /// Same text and dependencies, but the prose names a file this repo no
    /// longer has (a stale reference, as opposed to an example path).
    pub draft_vanished_file: usize,
    pub deps_changed: usize,
    pub deps_unknown: usize,
    pub ambiguous: usize,
    pub contested: usize,
    pub occupied: usize,
    pub text_differs: usize,
    pub no_successor: usize,
    pub orphaned_file_docs: usize,
    pub orphaned_other_docs: usize,
}

impl AdoptionPlan {
    pub fn summary(&self) -> Summary {
        let mut s = Summary {
            total: self.findings.len(),
            orphaned_file_docs: self.orphaned_file_docs.len(),
            orphaned_other_docs: self.orphaned_other_docs,
            ..Summary::default()
        };
        for f in &self.findings {
            match &f.successor {
                Successor::One { deps, .. } => match deps {
                    Deps::Equal | Deps::OnlyAdditions if !f.names_vanished_file.is_empty() => {
                        s.draft_vanished_file += 1;
                    }
                    Deps::Equal => s.equal += 1,
                    Deps::OnlyAdditions => s.only_additions += 1,
                    Deps::Changed => s.deps_changed += 1,
                    Deps::Unknown => s.deps_unknown += 1,
                },
                Successor::Ambiguous(_) => s.ambiguous += 1,
                Successor::Contested(_) => s.contested += 1,
                Successor::Occupied(_) => s.occupied += 1,
                Successor::TextDiffers(_) => s.text_differs += 1,
                Successor::None => s.no_successor += 1,
            }
        }
        s.adoptable = s.equal + s.only_additions;
        s
    }

    /// The plain-text report `codeowl adopt-moved-specs` prints. `list` adds
    /// one line per finding.
    pub fn report(&self, list: bool) -> String {
        let s = self.summary();
        let pct = |n: usize| {
            if s.total == 0 {
                0.0
            } else {
                100.0 * n as f64 / s.total as f64
            }
        };
        let mut out = format!(
            "orphaned symbols with written prose: {}\n\
             adoptable as current: {} ({:.0}%)  [dependencies equal {}, only additions {}]\n\
             not adopted as current:\n\
             \x20 same text, but the prose names a file this repo no longer has (left for a person): {}\n\
             \x20 a dependency changed shape: {}\n\
             \x20 dependencies not comparable (old spec recorded none): {}\n\
             \x20 several identical successors, none preferred: {}\n\
             \x20 several orphans claim the same successor: {}\n\
             \x20 the successor already has written prose: {}\n\
             \x20 same name elsewhere, text differs: {}\n\
             \x20 no successor: {}\n\
             orphaned file documents (their summaries are regenerated): {}\n\
             orphaned rollup and feature documents (regenerated): {}\n",
            s.total,
            s.adoptable,
            pct(s.adoptable),
            s.equal,
            s.only_additions,
            s.draft_vanished_file,
            s.deps_changed,
            s.deps_unknown,
            s.ambiguous,
            s.contested,
            s.occupied,
            s.text_differs,
            s.no_successor,
            s.orphaned_file_docs,
            s.orphaned_other_docs
        );
        if list {
            out.push('\n');
            for f in &self.findings {
                let what = match &f.successor {
                    Successor::One { id, deps } => format!("{} -> {id} [{deps:?}]", f.old_id),
                    Successor::Ambiguous(ids) => {
                        format!("{} ambiguous: {}", f.old_id, ids.join(", "))
                    }
                    Successor::Contested(id) => format!("{} contested: {id}", f.old_id),
                    Successor::Occupied(id) => format!("{} occupied: {id}", f.old_id),
                    Successor::TextDiffers(ids) => {
                        format!("{} text differs: {}", f.old_id, ids.join(", "))
                    }
                    Successor::None => format!("{} no successor", f.old_id),
                };
                out.push_str(&what);
                if !f.names_vanished_file.is_empty() {
                    out.push_str(&format!(
                        "  (names vanished {})",
                        f.names_vanished_file.join(", ")
                    ));
                }
                out.push('\n');
            }
        }
        out
    }
}

/// An orphaned symbol with the hashes and prose it was documented with.
type Old = BTreeMap<String, (HashPair, SymbolProse)>;

/// Plan the adoption of orphaned symbol prose against the current `graph`.
pub fn plan(graph: &Graph, root: &Path) -> Result<AdoptionPlan> {
    let orphans = find_orphaned_specs(graph, root, None)?;
    let (old, orphaned_file_docs, orphaned_other_docs) = collect_orphans(graph, root, &orphans)?;

    // The current tree, indexed the two ways a successor is looked up.
    let mut by_hash: HashMap<&str, Vec<&Symbol>> = HashMap::new();
    let mut by_name: HashMap<&str, Vec<&Symbol>> = HashMap::new();
    for sym in graph.symbols() {
        by_hash
            .entry(sym.source_hash.as_str())
            .or_default()
            .push(sym);
        by_name.entry(short_name(&sym.id)).or_default().push(sym);
    }

    let mut resolved: Vec<(String, HashPair, SymbolProse, Successor)> = old
        .into_iter()
        .map(|(old_id, (hashes, prose))| {
            let successor = successor_of(graph, root, &old_id, &hashes, &by_hash, &by_name);
            (old_id, hashes, prose, successor)
        })
        .collect();
    apply_lineage(graph, root, &mut resolved);
    mark_claimed_successors(root, &mut resolved)?;

    let files: Vec<String> = graph.files().map(|f| f.id.clone()).collect();
    let findings = resolved
        .into_iter()
        .map(|(old_id, old_hashes, prose, successor)| {
            let text = format!("{}\n{}", prose.summary, prose.behavior);
            let moved_paths_named: Vec<String> = file_like_names(&text)
                .into_iter()
                .filter(|name| !files.iter().any(|f| names_file(name, f)))
                .collect();
            let names_vanished_file = moved_paths_named
                .iter()
                .filter(|name| orphaned_file_docs.iter().any(|o| names_file(name, o)))
                .cloned()
                .collect();
            Finding {
                old_id,
                successor,
                moved_paths_named,
                names_vanished_file,
                prose,
                old_hashes,
            }
        })
        .collect();

    Ok(AdoptionPlan {
        findings,
        orphaned_file_docs,
        orphaned_other_docs,
    })
}

/// Every orphaned symbol with written prose, with the hashes it was
/// documented against. A symbol shows up once whether its file is gone (a
/// "file" orphan lists all its symbols) or still there (a "symbol" orphan
/// names just the one that left). Also the orphaned file documents, and a
/// count of the rollup and feature ones, which are regenerated.
fn collect_orphans(
    graph: &Graph,
    root: &Path,
    orphans: &[OrphanedSpec],
) -> Result<(Old, Vec<String>, usize)> {
    let mut old: Old = BTreeMap::new();
    let mut orphaned_file_docs = Vec::new();
    let mut orphaned_other_docs = 0;
    for orphan in orphans {
        let (source_path, wanted): (&str, Option<&str>) = match orphan.kind.as_str() {
            "file" => {
                orphaned_file_docs.push(orphan.id.clone());
                (orphan.id.as_str(), None)
            }
            "symbol" => (file_of(&orphan.id), Some(orphan.id.as_str())),
            _ => {
                orphaned_other_docs += 1;
                continue;
            }
        };
        let Some(spec) = read_file_spec(root, source_path)? else {
            continue;
        };
        for (id, hashes) in &spec.symbols {
            if wanted.is_some_and(|w| w != id) || graph.find(id).is_some() {
                continue;
            }
            if let Some(prose) = written_prose(&spec, id) {
                old.insert(id.clone(), (hashes.clone(), prose.clone()));
            }
        }
    }
    orphaned_file_docs.sort();
    Ok((old, orphaned_file_docs, orphaned_other_docs))
}

/// `id`'s section in `spec`, if it has any text.
fn written_prose<'a>(spec: &'a FileSpec, id: &str) -> Option<&'a SymbolProse> {
    spec.sections
        .iter()
        .find(|(sid, _)| sid == id)
        .map(|(_, p)| p)
        .filter(|p| !p.summary.trim().is_empty() || !p.behavior.trim().is_empty())
}

fn successor_of(
    graph: &Graph,
    root: &Path,
    old_id: &str,
    hashes: &HashPair,
    by_hash: &HashMap<&str, Vec<&Symbol>>,
    by_name: &HashMap<&str, Vec<&Symbol>>,
) -> Successor {
    let name = short_name(old_id);
    let twins: &[&Symbol] = if hashes.source_hash.is_empty() {
        &[]
    } else {
        by_hash
            .get(hashes.source_hash.as_str())
            .map_or(&[], Vec::as_slice)
    };
    if twins.is_empty() {
        let mut same_name: Vec<String> = by_name
            .get(name)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(|s| s.id.clone())
            .collect();
        same_name.sort();
        return if same_name.is_empty() {
            Successor::None
        } else {
            Successor::TextDiffers(same_name)
        };
    }

    // Same text. Prefer a successor that kept the name; a renamed one is a
    // candidate only when nothing kept it.
    let named: Vec<&Symbol> = twins
        .iter()
        .copied()
        .filter(|s| short_name(&s.id) == name)
        .collect();
    let pool: Vec<&Symbol> = if named.is_empty() {
        twins.to_vec()
    } else {
        named
    };
    let chosen = if pool.len() == 1 {
        Some(pool[0])
    } else {
        // `lib/a.ts` became a directory `lib/a/`, the way `java.rs` became
        // `java/mod.rs`: the successor lives under the old file's stem.
        let old_file = file_of(old_id);
        let stem = old_file.rsplit_once('.').map_or(old_file, |(s, _)| s);
        let prefix = format!("{stem}/");
        let under_stem: Vec<&Symbol> = pool
            .iter()
            .copied()
            .filter(|s| s.file.starts_with(&prefix))
            .collect();
        (under_stem.len() == 1).then(|| under_stem[0])
    };
    match chosen {
        Some(sym) => Successor::One {
            id: sym.id.clone(),
            deps: compare_deps(graph, root, sym, hashes),
        },
        None => {
            let mut ids: Vec<String> = pool.iter().map(|s| s.id.clone()).collect();
            ids.sort();
            Successor::Ambiguous(ids)
        }
    }
}

/// Where did each old file's code go? Count the files its uniquely matched
/// symbols landed in. A symbol with several identical successors (a helper
/// copied into every pack) then goes where the rest of its old file went,
/// when exactly one file leads.
fn apply_lineage(
    graph: &Graph,
    root: &Path,
    resolved: &mut [(String, HashPair, SymbolProse, Successor)],
) {
    let mut lineage: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for (old_id, _, _, successor) in resolved.iter() {
        if let Successor::One { id, .. } = successor {
            *lineage
                .entry(file_of(old_id).to_string())
                .or_default()
                .entry(file_of(id).to_string())
                .or_default() += 1;
        }
    }
    for (old_id, hashes, _, successor) in resolved.iter_mut() {
        let Successor::Ambiguous(candidates) = successor else {
            continue;
        };
        let Some(votes) = lineage.get(file_of(old_id)) else {
            continue;
        };
        let top = votes.values().copied().max();
        let leaders: Vec<&String> = votes
            .iter()
            .filter(|(_, n)| Some(**n) == top)
            .map(|(file, _)| file)
            .collect();
        let [leader] = leaders.as_slice() else {
            continue;
        };
        let in_leader: Vec<&String> = candidates
            .iter()
            .filter(|id| file_of(id) == leader.as_str())
            .collect();
        let [only] = in_leader.as_slice() else {
            continue;
        };
        if let Some(sym) = graph.find(only).and_then(|id| graph.get_symbol(id)) {
            *successor = Successor::One {
                id: sym.id.clone(),
                deps: compare_deps(graph, root, sym, hashes),
            };
        }
    }
}

/// Two orphans that resolved to one symbol (merged helpers) cannot both be
/// written there, and a successor that already has written prose must not
/// be overwritten: neither is adoptable.
fn mark_claimed_successors(
    root: &Path,
    resolved: &mut [(String, HashPair, SymbolProse, Successor)],
) -> Result<()> {
    let mut claims: HashMap<String, usize> = HashMap::new();
    for (_, _, _, successor) in resolved.iter() {
        if let Successor::One { id, .. } = successor {
            *claims.entry(id.clone()).or_default() += 1;
        }
    }
    let mut spec_cache: HashMap<String, Option<FileSpec>> = HashMap::new();
    for (_, _, _, successor) in resolved.iter_mut() {
        let Successor::One { id, .. } = successor else {
            continue;
        };
        if claims.get(id).copied().unwrap_or(0) > 1 {
            *successor = Successor::Contested(id.clone());
            continue;
        }
        let file = file_of(id).to_string();
        if !spec_cache.contains_key(&file) {
            spec_cache.insert(file.clone(), read_file_spec(root, &file)?);
        }
        let occupied = spec_cache[&file]
            .as_ref()
            .is_some_and(|spec| written_prose(spec, id).is_some());
        if occupied {
            *successor = Successor::Occupied(id.clone());
        }
    }
    Ok(())
}

/// The dependencies `old` was documented against, against `sym`'s now.
fn compare_deps(graph: &Graph, root: &Path, sym: &Symbol, old: &HashPair) -> Deps {
    let Some(file_id) = graph.find(&sym.file) else {
        return Deps::Unknown;
    };
    let Some(pairs) = try_dependency_pairs(graph, root, file_id, sym) else {
        return Deps::Unknown;
    };
    let mut now: Vec<String> = shorten_dep_targets(&pairs)
        .into_iter()
        .map(|(_, h)| h)
        .collect();
    now.sort();

    if old.dep_targets.is_empty() {
        // Either it really had none, or it predates the per-target record.
        // A spec written with none stores the hash of the empty list.
        return if old.deps_hash == hash_dependency_pairs(&[]) {
            if now.is_empty() {
                Deps::Equal
            } else {
                Deps::OnlyAdditions
            }
        } else {
            Deps::Unknown
        };
    }

    let mut before: Vec<String> = old.dep_targets.iter().map(|(_, h)| h.clone()).collect();
    before.sort();
    if before == now {
        return Deps::Equal;
    }
    // Is every recorded hash still present? Counted, so a hash recorded
    // twice must still be there twice.
    let mut remaining = now;
    for hash in &before {
        match remaining.iter().position(|h| h == hash) {
            Some(i) => {
                remaining.remove(i);
            }
            None => return Deps::Changed,
        }
    }
    Deps::OnlyAdditions
}

fn short_name(id: &str) -> &str {
    id.rsplit("::").next().unwrap_or(id)
}

/// The file part of a symbol id (`lib/a.ts::f` is in `lib/a.ts`).
pub(super) fn file_of(id: &str) -> &str {
    id.split("::").next().unwrap_or(id)
}

/// File-like names in `text`: runs of path characters that end in a source
/// extension (`lib/a.ts`, `spec.rs`). A heuristic for prose that names the
/// place its code used to live, not a parser. It knows the extensions of
/// the languages CodeOwl reads, matched in lower case, and does not tell a
/// lower-case product name (`node.js`) from a file.
fn file_like_names(text: &str) -> Vec<String> {
    const EXTENSIONS: [&str; 7] = [".rs", ".ts", ".tsx", ".java", ".py", ".js", ".jsx"];
    let mut found: BTreeSet<String> = BTreeSet::new();
    for run in text.split(|c: char| !(c.is_ascii_alphanumeric() || "_/.-".contains(c))) {
        let run = run.trim_matches(|c| c == '.' || c == '/' || c == '-');
        if !EXTENSIONS
            .iter()
            .any(|ext| run.len() > ext.len() && run.ends_with(ext))
        {
            continue;
        }
        // `Next.js`, `Node.js`: a capitalised word with no directory in it
        // is a product name.
        let is_product_name =
            !run.contains('/') && run.chars().next().is_some_and(|c| c.is_ascii_uppercase());
        if !is_product_name {
            found.insert(run.to_string());
        }
    }
    found.into_iter().collect()
}

/// Whether `name`, as prose wrote it (a full path or just a file name), is
/// `file`.
fn names_file(name: &str, file: &str) -> bool {
    file == name
        || file
            .strip_suffix(name)
            .is_some_and(|rest| rest.ends_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::test_support::{
        PROSE, documented, rebuild_graph as rebuild, write_file as write,
    };

    fn finding<'a>(plan: &'a AdoptionPlan, old: &str) -> &'a Finding {
        plan.findings
            .iter()
            .find(|f| f.old_id == old)
            .unwrap_or_else(|| panic!("no finding for {old}: {:?}", plan.findings))
    }

    const F: &str = "export function f(): number { return 1; }\n";

    #[test]
    fn a_moved_function_with_the_same_text_and_dependencies_is_adoptable() {
        let dir = documented("adopt-moved", &[("lib/a.ts", F)], &["lib/a.ts::f"], PROSE);
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        let f = finding(&plan, "lib/a.ts::f");
        assert_eq!(
            f.successor,
            Successor::One {
                id: "lib/b.ts::f".to_string(),
                deps: Deps::Equal
            }
        );
        assert!(f.adoptable());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_function_moved_out_of_a_file_that_still_exists_is_found_too() {
        let both = "export function f(): number { return 1; }\nexport function k(): number { return 2; }\n";
        let dir = documented(
            "adopt-split-out",
            &[("lib/a.ts", both)],
            &["lib/a.ts::f"],
            PROSE,
        );
        write(
            &dir,
            "lib/a.ts",
            "export function k(): number { return 2; }\n",
        );
        write(&dir, "lib/b.ts", F);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        assert_eq!(
            finding(&plan, "lib/a.ts::f").successor,
            Successor::One {
                id: "lib/b.ts::f".to_string(),
                deps: Deps::Equal
            }
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_function_whose_text_changed_only_has_a_same_name_candidate() {
        let dir = documented("adopt-differs", &[("lib/a.ts", F)], &["lib/a.ts::f"], PROSE);
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(
            &dir,
            "lib/b.ts",
            "export function f(): number { return 2; }\n",
        );

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        let f = finding(&plan, "lib/a.ts::f");
        assert_eq!(
            f.successor,
            Successor::TextDiffers(vec!["lib/b.ts::f".to_string()])
        );
        assert!(!f.adoptable());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_function_that_is_gone_has_no_successor() {
        let dir = documented("adopt-gone", &[("lib/a.ts", F)], &["lib/a.ts::f"], PROSE);
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/other.ts", "export function other(): void {}\n");

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        assert_eq!(finding(&plan, "lib/a.ts::f").successor, Successor::None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn identical_text_in_two_places_is_ambiguous_when_no_path_picks_one() {
        let dir = documented(
            "adopt-ambiguous",
            &[("lib/a.ts", F)],
            &["lib/a.ts::f"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);
        write(&dir, "lib/c.ts", F);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        let f = finding(&plan, "lib/a.ts::f");
        assert_eq!(
            f.successor,
            Successor::Ambiguous(vec!["lib/b.ts::f".to_string(), "lib/c.ts::f".to_string()])
        );
        assert!(!f.adoptable());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_path_under_the_old_files_stem_picks_between_identical_candidates() {
        // lib/a.ts became a directory lib/a/, the way java.rs became java/mod.rs.
        let dir = documented(
            "adopt-affinity",
            &[("lib/a.ts", F)],
            &["lib/a.ts::f"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/a/index.ts", F);
        write(&dir, "lib/c.ts", F);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        assert_eq!(
            finding(&plan, "lib/a.ts::f").successor,
            Successor::One {
                id: "lib/a/index.ts::f".to_string(),
                deps: Deps::Equal
            }
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_files_other_symbols_decide_between_identical_candidates() {
        // lib/a.ts held f and g. g went to lib/x/y.ts alone, so lib/a.ts's
        // lineage points there; f's text is in lib/x/y.ts and in lib/c.ts.
        let g = "export function g(): number { return 9; }\n";
        let both = format!("{F}{g}");
        let dir = documented(
            "adopt-lineage",
            &[("lib/a.ts", both.as_str())],
            &["lib/a.ts::f", "lib/a.ts::g"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/x/y.ts", &both);
        write(&dir, "lib/c.ts", F);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        assert_eq!(
            finding(&plan, "lib/a.ts::f").successor,
            Successor::One {
                id: "lib/x/y.ts::f".to_string(),
                deps: Deps::Equal
            }
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_tied_lineage_leaves_identical_candidates_ambiguous() {
        // g and h went to different files: nothing says where f went.
        let g = "export function g(): number { return 9; }\n";
        let h = "export function h(): number { return 8; }\n";
        let all = format!("{F}{g}{h}");
        let dir = documented(
            "adopt-lineage-tie",
            &[("lib/a.ts", all.as_str())],
            &["lib/a.ts::f", "lib/a.ts::g", "lib/a.ts::h"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/x.ts", &format!("{F}{g}"));
        write(&dir, "lib/y.ts", &format!("{F}{h}"));

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        assert_eq!(
            finding(&plan, "lib/a.ts::f").successor,
            Successor::Ambiguous(vec!["lib/x.ts::f".to_string(), "lib/y.ts::f".to_string()])
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    const H_OLD: &str = "export function g(x: number): number { return x; }\n";
    const H_NEW: &str = "export function g(x: string): string { return x; }\n";
    const USES_G_A: &str =
        "import { g } from \"./h\";\nexport function f(): number { return g(1); }\n";
    const USES_G_B: &str =
        "import { g } from \"./h\";\nexport function f(): number { return g(1); }\n";

    #[test]
    fn a_dependency_that_changed_shape_blocks_adoption() {
        let dir = documented(
            "adopt-dep-changed",
            &[("lib/h.ts", H_OLD), ("lib/a.ts", USES_G_A)],
            &["lib/a.ts::f"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", USES_G_B);
        write(&dir, "lib/h.ts", H_NEW);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        let f = finding(&plan, "lib/a.ts::f");
        assert_eq!(
            f.successor,
            Successor::One {
                id: "lib/b.ts::f".to_string(),
                deps: Deps::Changed
            }
        );
        assert!(!f.adoptable());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_dependency_that_only_moved_keeps_the_same_hash_and_still_adopts() {
        let dir = documented(
            "adopt-dep-moved",
            &[("lib/h.ts", H_OLD), ("lib/a.ts", USES_G_A)],
            &["lib/a.ts::f"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        std::fs::remove_file(dir.join("lib/h.ts")).unwrap();
        write(&dir, "lib/helpers.ts", H_OLD);
        write(
            &dir,
            "lib/b.ts",
            "import { g } from \"./helpers\";\nexport function f(): number { return g(1); }\n",
        );

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        assert_eq!(
            finding(&plan, "lib/a.ts::f").successor,
            Successor::One {
                id: "lib/b.ts::f".to_string(),
                deps: Deps::Equal
            },
            "g moved to another file but its text is unchanged, so its interface hash is the same"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_same_file_helper_that_became_an_import_is_only_an_addition() {
        let before = "function helper(): number { return 1; }\nexport function f(): number { return helper(); }\n";
        let dir = documented(
            "adopt-split",
            &[("lib/a.ts", before)],
            &["lib/a.ts::f"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(
            &dir,
            "lib/h.ts",
            "export function helper(): number { return 1; }\n",
        );
        write(
            &dir,
            "lib/b.ts",
            "import { helper } from \"./h\";\nexport function f(): number { return helper(); }\n",
        );

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        let f = finding(&plan, "lib/a.ts::f");
        assert_eq!(
            f.successor,
            Successor::One {
                id: "lib/b.ts::f".to_string(),
                deps: Deps::OnlyAdditions
            },
            "the old spec recorded no dependencies; the helper is a new cross-file one"
        );
        assert!(f.adoptable());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prose_that_names_a_file_which_no_longer_exists_is_flagged() {
        let prose = "### Summary\nLives in lib/a.ts next to lib/keep.ts.\n### Behavior\nReturns one; see lib/a.ts for the constant.\n";
        let dir = documented(
            "adopt-names-path",
            &[("lib/a.ts", F), ("lib/keep.ts", "export const k = 1;\n")],
            &["lib/a.ts::f"],
            prose,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        assert_eq!(
            finding(&plan, "lib/a.ts::f").moved_paths_named,
            vec!["lib/a.ts".to_string()],
            "lib/keep.ts still exists, lib/a.ts does not"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn only_a_name_that_is_an_orphaned_file_is_a_stale_reference() {
        // lib/a.ts is gone and has an orphaned spec: naming it is stale.
        // lib/example.ts never existed: prose that cites it is illustrating.
        let prose = "### Summary\nSame as lib/a.ts; for example lib/example.ts maps over.\n### Behavior\nReturns the number one and has no side effects.\n";
        let dir = documented(
            "adopt-vanished",
            &[("lib/a.ts", F)],
            &["lib/a.ts::f"],
            prose,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        let f = finding(&plan, "lib/a.ts::f");
        assert_eq!(
            f.moved_paths_named,
            vec!["lib/a.ts".to_string(), "lib/example.ts".to_string()]
        );
        assert_eq!(f.names_vanished_file, vec!["lib/a.ts".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_symbol_with_no_written_prose_is_not_reported() {
        let dir = documented("adopt-no-prose", &[("lib/a.ts", F)], &[], PROSE);
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        assert!(plan.findings.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    fn made(old: &str, successor: Successor, named: &[&str]) -> Finding {
        Finding {
            old_id: old.to_string(),
            successor,
            moved_paths_named: named.iter().map(|s| s.to_string()).collect(),
            names_vanished_file: named.iter().map(|s| s.to_string()).collect(),
            prose: SymbolProse::default(),
            old_hashes: HashPair::default(),
        }
    }

    #[test]
    fn the_summary_counts_each_outcome_once_and_splits_the_adoptable_by_dependency_relation() {
        let one = |id: &str, deps| Successor::One {
            id: id.to_string(),
            deps,
        };
        let plan = AdoptionPlan {
            findings: vec![
                made("a::1", one("b::1", Deps::Equal), &[]),
                made("a::2", one("b::2", Deps::Equal), &["a.rs"]),
                made("a::3", one("b::3", Deps::OnlyAdditions), &[]),
                made("a::4", one("b::4", Deps::Changed), &[]),
                made("a::5", one("b::5", Deps::Unknown), &[]),
                made(
                    "a::6",
                    Successor::Ambiguous(vec!["x".into(), "y".into()]),
                    &[],
                ),
                made("a::7", Successor::TextDiffers(vec!["z".into()]), &[]),
                made("a::8", Successor::None, &[]),
                made("a::9", Successor::Contested("b::9".into()), &[]),
                made("a::10", Successor::Occupied("b::10".into()), &[]),
            ],
            orphaned_file_docs: vec!["a.rs".to_string()],
            orphaned_other_docs: 2,
        };
        let s = plan.summary();
        assert_eq!(s.total, 10);
        assert_eq!((s.adoptable, s.equal, s.only_additions), (2, 1, 1));
        assert_eq!(s.draft_vanished_file, 1, "a::2 names a vanished file");
        assert_eq!((s.deps_changed, s.deps_unknown), (1, 1));
        assert_eq!((s.ambiguous, s.text_differs, s.no_successor), (1, 1, 1));
        assert_eq!((s.contested, s.occupied), (1, 1));
        assert_eq!((s.orphaned_file_docs, s.orphaned_other_docs), (1, 2));
        assert_eq!(
            s.total,
            s.adoptable
                + s.draft_vanished_file
                + s.deps_changed
                + s.deps_unknown
                + s.ambiguous
                + s.text_differs
                + s.no_successor
                + s.contested
                + s.occupied,
            "every orphaned symbol lands in exactly one bucket"
        );
    }

    #[test]
    fn the_report_names_the_buckets_and_lists_findings_only_when_asked() {
        let one = |id: &str| Successor::One {
            id: id.to_string(),
            deps: Deps::Equal,
        };
        let plan = AdoptionPlan {
            findings: vec![
                made("a::1", one("b::1"), &[]),
                Finding {
                    names_vanished_file: vec!["a.rs".to_string()],
                    ..made("a::2", one("b::2"), &[])
                },
            ],
            orphaned_file_docs: vec![],
            orphaned_other_docs: 3,
        };
        let brief = plan.report(false);
        assert!(brief.contains("adoptable as current: 1"));
        assert!(
            brief.contains("left for a person): 1"),
            "the vanished-file one is left for a person"
        );
        assert!(brief.contains("rollup and feature documents (regenerated): 3"));
        assert!(!brief.contains("a::1"));
        let long = plan.report(true);
        assert!(long.contains("a::1 -> b::1"));
        assert!(long.contains("names vanished a.rs"));
    }

    #[test]
    fn two_orphans_merged_into_one_symbol_contest_it() {
        // lib/a.ts and lib/b.ts each had helper(); now only lib/c.ts does.
        // Both specs cannot be written to one id.
        let h = "export function helper(): number { return 7; }\n";
        let dir = documented(
            "adopt-contested",
            &[("lib/a.ts", h), ("lib/b.ts", h)],
            &["lib/a.ts::helper", "lib/b.ts::helper"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        std::fs::remove_file(dir.join("lib/b.ts")).unwrap();
        write(&dir, "lib/c.ts", h);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        for old in ["lib/a.ts::helper", "lib/b.ts::helper"] {
            let f = finding(&plan, old);
            assert_eq!(
                f.successor,
                Successor::Contested("lib/c.ts::helper".to_string())
            );
            assert!(!f.adoptable(), "{old} must not be adopted over its twin");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_successor_that_already_has_written_prose_is_left_alone() {
        // lib/a.ts and lib/b.ts both define f and both were documented;
        // then lib/a.ts is deleted. lib/b.ts::f has its own spec.
        let dir = documented(
            "adopt-occupied",
            &[("lib/a.ts", F), ("lib/b.ts", F)],
            &["lib/a.ts::f", "lib/b.ts::f"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        let f = finding(&plan, "lib/a.ts::f");
        assert_eq!(f.successor, Successor::Occupied("lib/b.ts::f".to_string()));
        assert!(!f.adoptable());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prose_naming_a_vanished_file_is_a_draft_not_a_current_adoption() {
        let prose = "### Summary\nSame as lib/a.ts in spirit.\n### Behavior\nReturns the number one with no side effects.\n";
        let dir = documented("adopt-draft", &[("lib/a.ts", F)], &["lib/a.ts::f"], prose);
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", F);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        let f = finding(&plan, "lib/a.ts::f");
        assert_eq!(
            f.successor,
            Successor::One {
                id: "lib/b.ts::f".to_string(),
                deps: Deps::Equal
            }
        );
        assert!(!f.adoptable(), "it names a file that is gone");
        assert!(f.draft_only());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_framework_name_is_not_taken_for_a_file_name() {
        // "Next.js" ends in ".js"; a capitalised word with no directory in
        // it is a product name, not a file.
        assert_eq!(
            file_like_names("Built for Next.js and Node.js; see lib/a.ts and route.js."),
            vec!["lib/a.ts".to_string(), "route.js".to_string()]
        );
    }

    // Keeps the sort order the report relies on from drifting unnoticed.
    #[test]
    fn findings_come_back_sorted_by_old_id() {
        let two = "export function f(): number { return 1; }\nexport function e(): number { return 5; }\n";
        let dir = documented(
            "adopt-sorted",
            &[("lib/a.ts", two)],
            &["lib/a.ts::f", "lib/a.ts::e"],
            PROSE,
        );
        std::fs::remove_file(dir.join("lib/a.ts")).unwrap();
        write(&dir, "lib/b.ts", two);

        let plan = plan(&rebuild(&dir), &dir).unwrap();
        let ids: Vec<&str> = plan.findings.iter().map(|f| f.old_id.as_str()).collect();
        assert_eq!(ids, vec!["lib/a.ts::e", "lib/a.ts::f"]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
