//! One-off tools that rewrite stored specs in place after a format change.

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::format::{parse, write_replacing};
use super::layout::spec_path;
use super::staleness::{
    file_dependency_hash, hash_dependency_pairs, shorten_dep_targets, try_dependency_pairs,
    try_file_dependency_pairs,
};
use crate::graph::Graph;

/// What `backfill_dep_targets` found, counted per owner (a file's own record,
/// or one symbol's) except `unreadable_files`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BackfillReport {
    /// Spec files whose `dep_targets:` block changed (or would, on a dry run).
    pub files_changed: usize,
    /// Owners that got their pairs written.
    pub filled: usize,
    /// Owners skipped because the pairs the graph gives now no longer hash to
    /// the stored `deps_hash`, so they are not the pairs it was made from.
    pub skipped_moved: usize,
    /// Owners skipped because the graph cannot say what they depend on now:
    /// the symbol no longer exists, or its source cannot be read.
    pub unverifiable: usize,
    /// Owners that already had a record.
    pub already_recorded: usize,
    /// Owners whose dependency list is empty, so there is nothing to record.
    pub without_dependencies: usize,
    /// Spec files that could not be read or parsed; left as they are.
    pub unreadable_files: usize,
}

/// Write the per-target record into stored file specs that predate it. An
/// owner is filled only when the pairs recomputed from the graph hash to its
/// stored `deps_hash`: then they are, by construction, the pairs it was made
/// from. The only change to a spec is lines added to the `dep_targets:` block
/// of its frontmatter; every other byte, including record lines already there,
/// stays as it was. A spec that cannot be read is counted and skipped. With
/// `write` false nothing is written and the report says what would be.
pub fn backfill_dep_targets(graph: &Graph, root: &Path, write: bool) -> Result<BackfillReport> {
    let mut report = BackfillReport::default();
    for file in graph.files() {
        let Some(file_id) = graph.find(&file.id) else {
            continue;
        };
        let path = spec_path(root, &file.id);
        if !path.exists() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            report.unreadable_files += 1;
            continue;
        };
        let Ok(spec) = parse(&text) else {
            report.unreadable_files += 1;
            continue;
        };
        let owners = std::iter::once((None, &spec.file))
            .chain(spec.symbols.iter().map(|(id, h)| (Some(id.as_str()), h)));
        let mut lines = String::new();
        let mut filled = 0;
        for (symbol, stored) in owners {
            if !stored.dep_targets.is_empty() {
                report.already_recorded += 1;
                continue;
            }
            let pairs = match symbol {
                None => try_file_dependency_pairs(graph, file_id),
                Some(id) => graph
                    .find(id)
                    .and_then(|sym_id| graph.get_symbol(sym_id))
                    .and_then(|sym| try_dependency_pairs(graph, root, file_id, sym)),
            };
            let Some(pairs) = pairs else {
                report.unverifiable += 1;
                continue;
            };
            if hash_dependency_pairs(&pairs) != stored.deps_hash {
                report.skipped_moved += 1;
            } else if pairs.is_empty() {
                report.without_dependencies += 1;
            } else {
                let owner = symbol.unwrap_or("file");
                for (target, hash) in shorten_dep_targets(&pairs) {
                    lines.push_str(&format!("  {owner} -> {target}: {hash}\n"));
                }
                filled += 1;
            }
        }
        if filled == 0 {
            continue;
        }
        report.filled += filled;
        report.files_changed += 1;
        if write {
            let updated = add_dep_target_lines(&text, &lines)
                .with_context(|| format!("updating {}", path.display()))?;
            write_replacing(&path, &updated)?;
        }
    }
    Ok(report)
}

/// `text` with `lines` (each ending in `\n`) added to the `dep_targets:` block
/// of its frontmatter: at the end of the block if there is one, otherwise as a
/// new block just before the closing `---`. Uses the file's own line ending.
fn add_dep_target_lines(text: &str, lines: &str) -> Result<String> {
    let crlf = text
        .split_inclusive('\n')
        .next()
        .is_some_and(|l| l.ends_with("\r\n"));
    let eol = |s: &str| {
        if crlf {
            s.replace('\n', "\r\n")
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    let mut rest = text.split_inclusive('\n');
    let first = rest.next().context("empty spec file")?;
    if first.trim_end() != "---" {
        bail!("spec file has no frontmatter");
    }
    out.push_str(first);
    let mut block = None; // None: no block seen; Some(true): inside it
    for line in rest.by_ref() {
        let at_end = line.trim_end() == "---";
        let in_block = block == Some(true) && line.starts_with("  ");
        if (at_end || !in_block) && block == Some(true) {
            out.push_str(&eol(lines));
            block = Some(false);
        }
        if at_end {
            if block.is_none() {
                out.push_str(&eol(&format!("dep_targets:\n{lines}")));
            }
            out.push_str(line);
            out.extend(rest);
            return Ok(out);
        }
        if line.trim_end() == "dep_targets:" {
            block = Some(true);
        }
        out.push_str(line);
    }
    bail!("spec file frontmatter is never closed")
}

/// What `migrate_file_fingerprints` found, counted per spec file.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    /// Legacy specs that were current and got a `fingerprint` (or would, on
    /// a dry run).
    pub migrated: usize,
    /// Specs that already carry a fingerprint.
    pub already_migrated: usize,
    /// Legacy specs left alone because they were already stale: their text
    /// or dependencies had moved since they were written, so what they say
    /// is not known to match the code.
    pub left_stale: usize,
    /// Spec files with symbol sections but no file summary yet.
    pub no_file_entry: usize,
    /// Spec files that could not be read, parsed or edited; left as they are.
    pub unreadable_files: usize,
}

/// Give file specs written before the file fingerprint their `fingerprint`
/// key. A legacy spec is migrated only if it was current under the old
/// definition (its stored raw-text hash and dependency hash both still equal
/// the graph's), because only then is the graph's current fingerprint also
/// the value the spec was written from. The only change to a spec is the
/// inserted key on its file line; a stale spec stays stale. With `write`
/// false nothing is written and the report says what would be.
pub fn migrate_file_fingerprints(
    graph: &Graph,
    root: &Path,
    write: bool,
) -> Result<MigrationReport> {
    let mut report = MigrationReport::default();
    for file in graph.files() {
        let Some(file_id) = graph.find(&file.id) else {
            continue;
        };
        let path = spec_path(root, &file.id);
        if !path.exists() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            report.unreadable_files += 1;
            continue;
        };
        let Ok(spec) = parse(&text) else {
            report.unreadable_files += 1;
            continue;
        };
        if spec.file.spec_hash.is_empty() {
            report.no_file_entry += 1;
            continue;
        }
        if !spec.file.fingerprint.is_empty() {
            report.already_migrated += 1;
            continue;
        }
        let was_current = spec.file.source_hash == file.source_hash
            && spec.file.deps_hash == file_dependency_hash(graph, file_id);
        if !was_current {
            report.left_stale += 1;
            continue;
        }
        let Ok(updated) = add_file_fingerprint(&text, &file.fingerprint) else {
            report.unreadable_files += 1;
            continue;
        };
        report.migrated += 1;
        if write {
            write_replacing(&path, &updated)
                .with_context(|| format!("updating {}", path.display()))?;
        }
    }
    Ok(report)
}

/// `text` with `fingerprint: <value>` added to the `file:` line of its
/// frontmatter, right after `source_hash`. Nothing else changes, including
/// the file's line endings.
fn add_file_fingerprint(text: &str, fingerprint: &str) -> Result<String> {
    let mut out = String::with_capacity(text.len() + fingerprint.len() + 16);
    let mut in_frontmatter = false;
    let mut done = false;
    for (i, line) in text.split_inclusive('\n').enumerate() {
        if i == 0 {
            if line.trim_end() != "---" {
                bail!("spec file has no frontmatter");
            }
            in_frontmatter = true;
        } else if in_frontmatter && line.trim_end() == "---" {
            in_frontmatter = false;
        }
        if in_frontmatter && !done && line.starts_with("file: {") {
            let Some(at) = line.find(", deps_hash:") else {
                bail!("the file line has no deps_hash to insert before");
            };
            out.push_str(&line[..at]);
            out.push_str(&format!(", fingerprint: {fingerprint}"));
            out.push_str(&line[at..]);
            done = true;
        } else {
            out.push_str(line);
        }
    }
    if !done {
        bail!("spec file has no file line in its frontmatter");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::build_graph_from_sources;
    use crate::spec::file_task::submit;
    use crate::spec::format::{parse, read_file_spec};
    use crate::spec::layout::spec_path;
    use crate::spec::staleness::file_changes;
    use crate::spec::test_support::{greeter_people_graph, temp_dir_for};

    use std::path::Path;

    /// Write the `Polite` section and `people.rs` summary, so the spec holds
    /// one dependency record for the file and one for the symbol.
    fn generate_people(graph: &Graph, dir: &Path) -> String {
        let content = "### Summary\nGreets people politely by name.\n### Behavior\nReturns a polite greeting string for the caller.\n";
        submit(graph, dir, "src/people.rs::Polite", content).unwrap();
        submit(
            graph,
            dir,
            "src/people.rs",
            "Small people module for tests.",
        )
        .unwrap();
        std::fs::read_to_string(spec_path(dir, "src/people.rs")).unwrap()
    }

    /// The spec text as it was before the record existed: no `dep_targets:`
    /// block at all.
    fn without_dep_targets(text: &str) -> String {
        let mut out = String::new();
        let mut in_block = false;
        for line in text.split_inclusive('\n') {
            if line.trim_end() == "dep_targets:" {
                in_block = true;
                continue;
            }
            if in_block && line.starts_with("  ") {
                continue;
            }
            in_block = false;
            out.push_str(line);
        }
        out
    }

    // ---- migrate-file-fingerprint ----

    /// Put a file's spec on disk, then turn it into a legacy one by dropping
    /// its `fingerprint` key (the shape every spec had before the fingerprint).
    fn legacy_file_spec(graph: &Graph, dir: &Path, id: &str) -> String {
        submit(graph, dir, id, "A file that does one clear job.").unwrap();
        let path = spec_path(dir, id);
        let text = std::fs::read_to_string(&path).unwrap();
        let fp = graph
            .get_file(graph.find(id).unwrap())
            .unwrap()
            .fingerprint
            .clone();
        let legacy = text.replace(&format!(" fingerprint: {fp},"), "");
        assert_ne!(
            legacy, text,
            "the fixture must have had a fingerprint to drop"
        );
        std::fs::write(&path, &legacy).unwrap();
        legacy
    }

    fn migrate_fixture(tag: &str) -> (Graph, std::path::PathBuf) {
        let dir = temp_dir_for(tag);
        let mut graph = build_graph_from_sources(&[("a.ts", "export function f(): void {}\n")]);
        graph.set_file_fingerprint("a.ts", "fp-now");
        (graph, dir)
    }

    #[test]
    fn backfill_restores_a_stripped_record_byte_for_byte() {
        let (graph, dir) = greeter_people_graph("backfill-exact");
        let original = generate_people(&graph, &dir);
        assert!(original.contains("dep_targets:"));
        let path = spec_path(&dir, "src/people.rs");
        std::fs::write(&path, without_dep_targets(&original)).unwrap();

        let report = backfill_dep_targets(&graph, &dir, true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        assert_eq!(report.files_changed, 1);
        assert_eq!(report.filled, 2); // the file and `Polite`
        assert_eq!(report.skipped_moved, 0);

        // A second run finds everything recorded and changes nothing.
        let again = backfill_dep_targets(&graph, &dir, true).unwrap();
        assert_eq!(again.files_changed, 0);
        assert_eq!(again.filled, 0);
        assert_eq!(again.already_recorded, 2);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_dry_run_reports_the_same_counts_and_writes_nothing() {
        let (graph, dir) = greeter_people_graph("backfill-dry");
        let original = generate_people(&graph, &dir);
        let stripped = without_dep_targets(&original);
        let path = spec_path(&dir, "src/people.rs");
        std::fs::write(&path, &stripped).unwrap();

        let report = backfill_dep_targets(&graph, &dir, false).unwrap();
        assert_eq!((report.files_changed, report.filled), (1, 2));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), stripped);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_owner_whose_dependencies_moved_is_skipped_and_its_lines_left_alone() {
        let (graph, dir) = greeter_people_graph("backfill-moved");
        let original = generate_people(&graph, &dir);
        let spec = parse(&original).unwrap();
        let polite = spec.symbol_hash("src/people.rs::Polite").unwrap();
        // The symbol's stored dependency hash no longer matches the graph.
        // (The file shares that hash here, so change the symbol's line only.)
        let moved: String = without_dep_targets(&original)
            .split_inclusive('\n')
            .map(|line| {
                if line.starts_with("  src/people.rs::Polite: ") {
                    line.replace(&polite.deps_hash, &"0".repeat(64))
                } else {
                    line.to_string()
                }
            })
            .collect();
        let path = spec_path(&dir, "src/people.rs");
        std::fs::write(&path, &moved).unwrap();

        let report = backfill_dep_targets(&graph, &dir, true).unwrap();
        assert_eq!(report.filled, 1);
        assert_eq!(report.skipped_moved, 1);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("  file -> src/greeter.rs::Greeter: "));
        assert!(!text.contains("  src/people.rs::Polite -> "));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_spec_stale_in_its_text_alone_still_gets_its_records() {
        let (graph, dir) = greeter_people_graph("backfill-source");
        let original = generate_people(&graph, &dir);
        let spec = parse(&original).unwrap();
        // Source hashes moved (the code was edited), dependencies did not.
        let edited = without_dep_targets(&original)
            .replace(&spec.file.source_hash, &"1".repeat(64))
            .replace(
                &spec
                    .symbol_hash("src/people.rs::Polite")
                    .unwrap()
                    .source_hash,
                &"2".repeat(64),
            );
        std::fs::write(spec_path(&dir, "src/people.rs"), &edited).unwrap();
        let report = backfill_dep_targets(&graph, &dir, true).unwrap();
        assert_eq!(report.filled, 2);
        assert_eq!(report.skipped_moved, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn owners_with_no_dependencies_write_nothing_and_are_counted() {
        let (graph, dir) = greeter_people_graph("backfill-none");
        let content = "### Summary\nA trait with one greeting method.\n### Behavior\nDeclares the single greet method implementors provide.\n";
        submit(&graph, &dir, "src/greeter.rs::Greeter", content).unwrap();
        submit(
            &graph,
            &dir,
            "src/greeter.rs",
            "The greeting trait that people modules implement.",
        )
        .unwrap();
        let path = spec_path(&dir, "src/greeter.rs");
        let before = std::fs::read_to_string(&path).unwrap();
        assert!(!before.contains("dep_targets"));

        let report = backfill_dep_targets(&graph, &dir, true).unwrap();
        assert_eq!(report.files_changed, 0);
        assert_eq!(report.without_dependencies, 2);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_malformed_spec_is_counted_and_skipped_not_fatal() {
        let (graph, dir) = greeter_people_graph("backfill-bad");
        let original = generate_people(&graph, &dir);
        std::fs::write(
            spec_path(&dir, "src/people.rs"),
            without_dep_targets(&original),
        )
        .unwrap();
        // A hand-damaged spec for the other file: frontmatter `parse` rejects.
        let bad = spec_path(&dir, "src/greeter.rs");
        std::fs::create_dir_all(bad.parent().unwrap()).unwrap();
        std::fs::write(&bad, "---\nthis line has no colon\n---\n# x\n").unwrap();

        let report = backfill_dep_targets(&graph, &dir, true).unwrap();
        assert_eq!(report.unreadable_files, 1);
        assert_eq!(report.files_changed, 1);
        assert_eq!(
            std::fs::read_to_string(spec_path(&dir, "src/people.rs")).unwrap(),
            original
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_record_line_for_an_undeclared_owner_survives_a_backfill() {
        let (graph, dir) = greeter_people_graph("backfill-orphan");
        let original = generate_people(&graph, &dir);
        let orphan = "  src/people.rs::Gone -> src/x.rs::X: aaaaaaaaaaaa\n";
        let with_orphan = add_dep_target_lines(&without_dep_targets(&original), orphan).unwrap();
        let path = spec_path(&dir, "src/people.rs");
        std::fs::write(&path, &with_orphan).unwrap();

        let report = backfill_dep_targets(&graph, &dir, true).unwrap();
        assert_eq!(report.filled, 2);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(orphan), "orphan line lost:\n{text}");
        assert!(text.contains("  file -> src/greeter.rs::Greeter: "));
        assert!(text.contains("  src/people.rs::Polite -> src/greeter.rs::Greeter: "));
        // One block only.
        assert_eq!(text.matches("dep_targets:").count(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_spec_with_windows_line_endings_keeps_them() {
        let (graph, dir) = greeter_people_graph("backfill-crlf");
        let original = generate_people(&graph, &dir).replace('\n', "\r\n");
        let path = spec_path(&dir, "src/people.rs");
        std::fs::write(&path, without_dep_targets(&original)).unwrap();

        backfill_dep_targets(&graph, &dir, true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_symbol_gone_from_the_graph_is_unverifiable_not_moved() {
        let (graph, dir) = greeter_people_graph("backfill-gone");
        let original = generate_people(&graph, &dir);
        let renamed: String = without_dep_targets(&original)
            .split_inclusive('\n')
            .map(|l| {
                if l.starts_with("  src/people.rs::Polite: ") {
                    l.replace("::Polite", "::Gone")
                } else {
                    l.to_string()
                }
            })
            .collect();
        std::fs::write(spec_path(&dir, "src/people.rs"), renamed).unwrap();

        let report = backfill_dep_targets(&graph, &dir, true).unwrap();
        assert_eq!(report.unverifiable, 1);
        assert_eq!(report.skipped_moved, 0);
        assert_eq!(report.filled, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_block_splice_refuses_text_without_closed_frontmatter() {
        assert!(add_dep_target_lines("no frontmatter\n", "  a\n").is_err());
        assert!(add_dep_target_lines("---\nkind: file\n", "  a\n").is_err());
        assert!(add_dep_target_lines("", "  a\n").is_err());
    }

    #[test]
    fn a_current_legacy_spec_gets_only_its_fingerprint_added() {
        let (graph, dir) = migrate_fixture("mig-basic");
        let legacy = legacy_file_spec(&graph, &dir, "a.ts");

        let report = migrate_file_fingerprints(&graph, &dir, true).unwrap();
        assert_eq!(report.migrated, 1);

        let after = std::fs::read_to_string(spec_path(&dir, "a.ts")).unwrap();
        assert_eq!(
            after,
            legacy.replace(", deps_hash:", ", fingerprint: fp-now, deps_hash:")
        );
        let spec = read_file_spec(&dir, "a.ts").unwrap().unwrap();
        assert!(
            file_changes(&graph, graph.find("a.ts").unwrap(), &spec.file).is_empty(),
            "a migrated spec reads as current"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_dry_run_reports_but_writes_nothing() {
        let (graph, dir) = migrate_fixture("mig-dry");
        let legacy = legacy_file_spec(&graph, &dir, "a.ts");
        let report = migrate_file_fingerprints(&graph, &dir, false).unwrap();
        assert_eq!(report.migrated, 1);
        assert_eq!(
            std::fs::read_to_string(spec_path(&dir, "a.ts")).unwrap(),
            legacy
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn running_it_twice_changes_nothing_the_second_time() {
        let (graph, dir) = migrate_fixture("mig-twice");
        legacy_file_spec(&graph, &dir, "a.ts");
        migrate_file_fingerprints(&graph, &dir, true).unwrap();
        let once = std::fs::read_to_string(spec_path(&dir, "a.ts")).unwrap();

        let report = migrate_file_fingerprints(&graph, &dir, true).unwrap();
        assert_eq!((report.migrated, report.already_migrated), (0, 1));
        assert_eq!(
            std::fs::read_to_string(spec_path(&dir, "a.ts")).unwrap(),
            once
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_legacy_spec_that_was_already_stale_is_left_stale() {
        let (graph, dir) = migrate_fixture("mig-stale");
        legacy_file_spec(&graph, &dir, "a.ts");
        // The file changes after the spec was written.
        let mut changed =
            build_graph_from_sources(&[("a.ts", "export function f(): number { return 1; }\n")]);
        changed.set_file_fingerprint("a.ts", "fp-later");
        let before = std::fs::read_to_string(spec_path(&dir, "a.ts")).unwrap();

        let report = migrate_file_fingerprints(&changed, &dir, true).unwrap();
        assert_eq!((report.migrated, report.left_stale), (0, 1));
        assert_eq!(
            std::fs::read_to_string(spec_path(&dir, "a.ts")).unwrap(),
            before
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_legacy_spec_whose_dependencies_moved_is_left_stale() {
        let (graph, dir) = migrate_fixture("mig-deps");
        legacy_file_spec(&graph, &dir, "a.ts");
        let path = spec_path(&dir, "a.ts");
        let text = std::fs::read_to_string(&path).unwrap();
        let spec = parse(&text).unwrap();
        let moved = text.replace(
            &format!("deps_hash: {}", spec.file.deps_hash),
            "deps_hash: moved",
        );
        assert_ne!(moved, text);
        std::fs::write(&path, &moved).unwrap();

        let report = migrate_file_fingerprints(&graph, &dir, true).unwrap();
        assert_eq!((report.migrated, report.left_stale), (0, 1));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), moved);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn crlf_line_endings_are_kept() {
        let (graph, dir) = migrate_fixture("mig-crlf");
        let legacy = legacy_file_spec(&graph, &dir, "a.ts");
        let crlf = legacy.replace('\n', "\r\n");
        std::fs::write(spec_path(&dir, "a.ts"), &crlf).unwrap();

        migrate_file_fingerprints(&graph, &dir, true).unwrap();
        let after = std::fs::read_to_string(spec_path(&dir, "a.ts")).unwrap();
        assert_eq!(
            after,
            crlf.replace(", deps_hash:", ", fingerprint: fp-now, deps_hash:")
        );
        assert!(
            !after.replace("\r\n", "").contains('\n'),
            "no bare newline introduced"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn specs_with_no_file_entry_or_unreadable_content_are_counted_and_left_alone() {
        let (graph, dir) = migrate_fixture("mig-skip");
        // Only a symbol was ever generated: there is no file entry to migrate.
        submit(
            &graph,
            &dir,
            "a.ts::f",
            "### Summary\nDoes one specific thing.\n### Behavior\nIt runs and returns nothing at all.\n",
        )
        .unwrap();
        let report = migrate_file_fingerprints(&graph, &dir, true).unwrap();
        assert_eq!((report.migrated, report.no_file_entry), (0, 1));

        // Garbage where a spec should be.
        std::fs::write(spec_path(&dir, "a.ts"), "not a spec at all").unwrap();
        let report = migrate_file_fingerprints(&graph, &dir, true).unwrap();
        assert_eq!(report.unreadable_files, 1);
        assert_eq!(
            std::fs::read_to_string(spec_path(&dir, "a.ts")).unwrap(),
            "not a spec at all"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
