//! Spec documents and symbol sections whose target no longer exists in the graph.

use std::path::Path;

use anyhow::{Context, Result};

use super::format::{parse, read_file_spec};
use super::layout::spec_bearing_children;
use super::rollup::enumerate_modules;
use crate::features::enumerate_entry_points;
use crate::graph::{Graph, SymbolId};
use crate::text::within_scope;

/// One spec document on disk that no longer corresponds to anything in the
/// current graph — a deleted file, a directory that dropped below the
/// rollup threshold, or a removed feature entry point whose spec still
/// sits under `docs/specs/`. Distinct from "stale": stale means the target
/// still exists and its inputs moved, so there's something to regenerate
/// against; an orphan's target is simply gone, so there's nothing left to
/// regenerate — it's dead weight to flag for deletion, not a generation
/// task (unless the code only moved: `adopt-moved-specs` carries its prose
/// over first). See `find_orphaned_specs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanedSpec {
    /// The same id vocabulary `coverage()` uses (`kind == "feature"` ids
    /// carry the `feature:` prefix, `kind == "rollup"` the `rollup:`
    /// prefix) — this is what no longer exists, not what to pass to
    /// `get_next_spec_task` (there's nothing left for it to generate).
    pub id: String,
    /// One of `"file"` | `"rollup"` | `"feature"` | `"symbol"`. The system
    /// spec is never orphanable — it always has a root to summarize.
    pub kind: String,
    /// For `"file"`/`"rollup"`/`"feature"`, the orphaned `.md` document's
    /// own path, relative to `root` — what a caller would delete outright.
    /// For `"symbol"`, the *containing file's* spec path instead — there's
    /// no separate file to delete, only a section inside a still-valid
    /// document to prune (which happens automatically on that file's next
    /// write, via `reorder_symbols`).
    pub path: String,
}

/// Walk `docs/specs/` looking for documents whose target no longer exists
/// in `graph` — the opposite direction from `coverage()`, which only ever
/// walks the graph outward looking for specs. Nothing else in this module
/// walks the spec tree inward toward the graph, so a spec whose target was
/// deleted *entirely* (not just edited) is otherwise invisible: `coverage`
/// can only report on ids the graph still knows about.
///
/// Each document kind's target is derived the same way its own
/// `*_spec_path` function derives where to *write* it — a file's implied
/// source path is its `.md` path relative to `docs/specs/` with the
/// trailing `.md` stripped (see `spec_path`), a rollup's implied directory
/// is its `_index.md`'s parent path, a feature's implied slug is its
/// `_features/<slug>.md` filename — so this needs no frontmatter parsing,
/// only the mirrored tree's own naming convention.
///
/// `scope` narrows the file/rollup portion the same way `coverage`'s does
/// (a directory prefix), and — also matching `coverage` — excludes feature
/// orphans entirely whenever a scope is given at all, since a feature is a
/// repo-wide concept with no directory of its own to be "in scope."
pub fn find_orphaned_specs(
    graph: &Graph,
    root: &Path,
    scope: Option<&str>,
) -> Result<Vec<OrphanedSpec>> {
    let specs_dir = root.join("docs").join("specs");
    if !specs_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut orphans = Vec::new();

    let features_dir = specs_dir.join("_features");
    if scope.is_none() && features_dir.is_dir() {
        let live_slugs: std::collections::HashSet<String> = enumerate_entry_points(graph)
            .into_iter()
            .map(|e| e.id)
            .collect();
        for entry in std::fs::read_dir(&features_dir)
            .with_context(|| format!("reading {}", features_dir.display()))?
        {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let slug = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !live_slugs.contains(&slug) {
                orphans.push(OrphanedSpec {
                    id: format!("feature:{slug}"),
                    kind: "feature".to_string(),
                    path: format!("docs/specs/_features/{slug}.md"),
                });
            }
        }
    }

    let live_files: std::collections::HashSet<String> =
        graph.files().map(|f| f.id.clone()).collect();
    let live_modules: std::collections::HashSet<String> =
        enumerate_modules(graph).into_iter().collect();
    find_orphaned_files_and_rollups(
        &specs_dir,
        &specs_dir,
        &features_dir,
        &live_files,
        &live_modules,
        scope,
        &mut orphans,
    )?;

    find_orphaned_symbol_sections(graph, root, scope, &mut orphans)?;

    Ok(orphans)
}

/// The one case `find_orphaned_files_and_rollups` can't see: a symbol
/// deleted from a file that *itself* still exists. `file_status` only ever
/// walks a file's *current* symbols when deciding hash-currency (see its
/// own doc comment), so a stored `FileSpec.symbols` entry whose id no
/// longer appears among `spec_bearing_children` never surfaces any other
/// way — the file can read `"current"` while quietly carrying a dead
/// section for a function that no longer exists, until the next unrelated
/// write to that file prunes it via `reorder_symbols`. Scoped to files
/// still live in the graph (a deleted file's sections are already covered
/// by its own whole-file orphan entry, above — no need to double-report).
fn find_orphaned_symbol_sections(
    graph: &Graph,
    root: &Path,
    scope: Option<&str>,
    orphans: &mut Vec<OrphanedSpec>,
) -> Result<()> {
    let mut file_ids: Vec<SymbolId> = graph.files().filter_map(|f| graph.find(&f.id)).collect();
    file_ids.sort_by_key(|&id| graph.string_id(id).to_string());

    for file_id in file_ids {
        let path = graph.string_id(file_id).to_string();
        if scope.is_some_and(|s| !within_scope(&path, s)) {
            continue;
        }
        let Some(spec) = read_file_spec(root, &path)? else {
            continue;
        };
        let current_ids: std::collections::HashSet<&str> = spec_bearing_children(graph, file_id)
            .iter()
            .filter_map(|&id| graph.get_symbol(id))
            .map(|s| s.id.as_str())
            .collect();
        for (stored_id, _) in &spec.symbols {
            if !current_ids.contains(stored_id.as_str()) {
                orphans.push(OrphanedSpec {
                    id: stored_id.clone(),
                    kind: "symbol".to_string(),
                    path: format!("docs/specs/{path}.md"),
                });
            }
        }
    }
    Ok(())
}

/// The file/rollup half of `find_orphaned_specs` — recurses through
/// `docs/specs/` skipping `_features/` (handled separately, above, since
/// its documents key on a slug, not a mirrored path) and the root
/// `_index.md` (the system spec, never orphanable).
#[allow(clippy::too_many_arguments)]
fn find_orphaned_files_and_rollups(
    dir: &Path,
    specs_root: &Path,
    features_dir: &Path,
    live_files: &std::collections::HashSet<String>,
    live_modules: &std::collections::HashSet<String>,
    scope: Option<&str>,
    orphans: &mut Vec<OrphanedSpec>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path == *features_dir {
            continue;
        }
        if path.is_dir() {
            find_orphaned_files_and_rollups(
                &path,
                specs_root,
                features_dir,
                live_files,
                live_modules,
                scope,
                orphans,
            )?;
            continue;
        }
        let rel = path.strip_prefix(specs_root).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        if rel.file_name().and_then(|n| n.to_str()) == Some("_index.md") {
            let Some(dir_path) = rel_str.strip_suffix("/_index.md") else {
                continue; // the root system spec -- never orphanable
            };
            if scope.is_some_and(|s| !within_scope(dir_path, s)) {
                continue;
            }
            if !live_modules.contains(dir_path) {
                orphans.push(OrphanedSpec {
                    id: format!("rollup:{dir_path}"),
                    kind: "rollup".to_string(),
                    path: format!("docs/specs/{rel_str}"),
                });
            }
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(source_path) = rel_str.strip_suffix(".md") else {
            continue;
        };
        if scope.is_some_and(|s| !within_scope(source_path, s)) {
            continue;
        }
        // Only treat this as a generated file spec if it actually parses
        // as one. A hand-authored doc that happens to live directly under
        // docs/specs/ (e.g. STYLE.md, a convention guide with no
        // frontmatter at all) is not a spec CodeOwl ever wrote, so its
        // "absence" from the graph means nothing — flagging it as orphaned
        // is a real false positive, not a conservative-but-harmless one:
        // it tells a human to delete a document they authored by hand.
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        if parse(&content).is_err() {
            continue;
        }
        if !live_files.contains(source_path) {
            orphans.push(OrphanedSpec {
                id: source_path.to_string(),
                kind: "file".to_string(),
                path: format!("docs/specs/{rel_str}"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::spec::feature::submit_feature;
    use crate::spec::file_task::submit;

    use crate::spec::rollup::submit_rollup;
    use crate::spec::test_support::{ARTWORK_FIXTURE, build_feature_fixture};

    #[test]
    fn find_orphaned_specs_reports_nothing_in_a_fully_live_repo() {
        let (graph, dir) = build_feature_fixture(
            &[
                ("lib/db.ts", "export function query(): void {}\n"),
                ("lib/other.ts", "export function helper(): void {}\n"),
            ],
            "orphan-none",
        );
        submit(
            &graph,
            &dir,
            "lib/db.ts::query",
            "### Summary\nRuns a database query.\n### Behavior\nReturns the query results.\n",
        )
        .unwrap();
        submit(&graph, &dir, "lib/db.ts", "A small database helper module.").unwrap();
        submit(
            &graph,
            &dir,
            "lib/other.ts::helper",
            "### Summary\nDoes a small helper task.\n### Behavior\nRuns without side effects.\n",
        )
        .unwrap();
        submit(&graph, &dir, "lib/other.ts", "A small helper module.").unwrap();

        assert!(find_orphaned_specs(&graph, &dir, None).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_orphaned_specs_reports_a_file_spec_whose_source_was_deleted() {
        let (graph, dir) = build_feature_fixture(
            &[
                ("lib/db.ts", "export function query(): void {}\n"),
                ("lib/other.ts", "export function helper(): void {}\n"),
            ],
            "orphan-file",
        );
        submit(
            &graph,
            &dir,
            "lib/db.ts::query",
            "### Summary\nRuns a database query.\n### Behavior\nReturns the query results.\n",
        )
        .unwrap();
        submit(&graph, &dir, "lib/db.ts", "A small database helper module.").unwrap();
        submit(
            &graph,
            &dir,
            "lib/other.ts::helper",
            "### Summary\nDoes a small helper task.\n### Behavior\nRuns without side effects.\n",
        )
        .unwrap();
        submit(&graph, &dir, "lib/other.ts", "A small helper module.").unwrap();

        std::fs::remove_file(dir.join("lib/db.ts")).unwrap();
        let graph_after = crate::index::RepoIndex::build(&dir)
            .unwrap()
            .rebuild()
            .unwrap();

        let orphans = find_orphaned_specs(&graph_after, &dir, None).unwrap();
        assert_eq!(
            orphans,
            vec![OrphanedSpec {
                id: "lib/db.ts".to_string(),
                kind: "file".to_string(),
                path: "docs/specs/lib/db.ts.md".to_string(),
            }],
            "only the deleted file's spec is orphaned -- lib/other.ts is still live"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_orphaned_specs_reports_a_rollup_whose_directory_no_longer_qualifies() {
        let (graph, dir) = build_feature_fixture(
            &[
                ("lib/email/send.ts", "export function send(): void {}\n"),
                ("lib/email/queue.ts", "export function queue(): void {}\n"),
            ],
            "orphan-rollup",
        );
        submit(
            &graph,
            &dir,
            "lib/email/send.ts::send",
            "### Summary\nSends a single email.\n### Behavior\nDispatches it immediately, without queuing.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "lib/email/send.ts",
            "The module responsible for sending email.",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "lib/email/queue.ts::queue",
            "### Summary\nQueues an email for later.\n### Behavior\nAdds it to the send queue.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "lib/email/queue.ts",
            "The module responsible for queueing email.",
        )
        .unwrap();
        submit_rollup(&graph, &dir, "lib/email", "Email sending and queueing.").unwrap();

        // Drop to one spec-bearing file -- lib/email no longer qualifies
        // for a rollup at all (needs >= 2), so it's now orphaned dead
        // weight, not merely stale.
        std::fs::remove_file(dir.join("lib/email/queue.ts")).unwrap();
        let graph_after = crate::index::RepoIndex::build(&dir)
            .unwrap()
            .rebuild()
            .unwrap();

        let orphans = find_orphaned_specs(&graph_after, &dir, None).unwrap();
        assert!(
            orphans.contains(&OrphanedSpec {
                id: "rollup:lib/email".to_string(),
                kind: "rollup".to_string(),
                path: "docs/specs/lib/email/_index.md".to_string(),
            }),
            "the rollup no longer has a qualifying directory: {orphans:?}"
        );
        assert!(
            orphans.iter().any(|o| o.id == "lib/email/queue.ts"),
            "the deleted file's own spec is orphaned too: {orphans:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_orphaned_specs_reports_a_feature_whose_entry_point_was_deleted() {
        let (graph, dir) = build_feature_fixture(ARTWORK_FIXTURE, "orphan-feature");
        submit(
            &graph,
            &dir,
            "app/submit/page.tsx",
            "The artwork submission page.",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "app/api/submit-artwork/route.ts::POST",
            "### Summary\nHandles an artwork submission request.\n### Behavior\nPersists the submitted artwork.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "app/api/submit-artwork/route.ts",
            "The artwork submission API route.",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "lib/supabase.ts::getSupabase",
            "### Summary\nGets the shared Supabase client.\n### Behavior\nReturns a cached instance.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "lib/supabase.ts",
            "The Supabase client module.",
        )
        .unwrap();
        submit_feature(
            &graph,
            &dir,
            "submit",
            "# Artwork submission\n## Summary\nLets a user submit artwork for judging.\n",
        )
        .unwrap();

        std::fs::remove_file(dir.join("app/submit/page.tsx")).unwrap();
        let graph_after = crate::index::RepoIndex::build(&dir)
            .unwrap()
            .rebuild()
            .unwrap();

        let orphans = find_orphaned_specs(&graph_after, &dir, None).unwrap();
        assert!(
            orphans
                .iter()
                .any(|o| o.id == "feature:submit" && o.kind == "feature"),
            "the entry point is gone, so the feature spec has nothing left to describe: {orphans:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_orphaned_specs_scope_narrows_files_and_drops_features_entirely() {
        let (graph, dir) = build_feature_fixture(
            &[
                ("lib/db.ts", "export function query(): void {}\n"),
                (
                    "app/page.tsx",
                    "export default function Page() {\n  return null;\n}\n",
                ),
                ("lib/keep.ts", "export function keep(): void {}\n"),
            ],
            "orphan-scope",
        );
        submit(
            &graph,
            &dir,
            "lib/db.ts::query",
            "### Summary\nRuns a database query.\n### Behavior\nReturns the query results.\n",
        )
        .unwrap();
        submit(&graph, &dir, "lib/db.ts", "A small database helper module.").unwrap();
        submit_feature(
            &graph,
            &dir,
            "home",
            "# Home page\n## Summary\nRenders the application home page.\n",
        )
        .unwrap();

        std::fs::remove_file(dir.join("lib/db.ts")).unwrap();
        std::fs::remove_file(dir.join("app/page.tsx")).unwrap();
        let graph_after = crate::index::RepoIndex::build(&dir)
            .unwrap()
            .rebuild()
            .unwrap();

        // Unscoped: both the orphaned file and the orphaned feature show up.
        let all = find_orphaned_specs(&graph_after, &dir, None).unwrap();
        assert!(all.iter().any(|o| o.kind == "file"));
        assert!(all.iter().any(|o| o.kind == "feature"));

        // Scoped to "lib": the file orphan survives, the feature orphan is
        // dropped entirely -- same exclusion coverage() itself applies.
        let scoped = find_orphaned_specs(&graph_after, &dir, Some("lib")).unwrap();
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].kind, "file");
        assert_eq!(scoped[0].id, "lib/db.ts");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_orphaned_specs_reports_a_deleted_symbols_lingering_section() {
        let (graph, dir) = build_feature_fixture(
            &[(
                "lib/math.ts",
                "export function add(a: number, b: number): number {\n  return a + b;\n}\nexport function sub(a: number, b: number): number {\n  return a - b;\n}\n",
            )],
            "orphan-symbol",
        );
        submit(
            &graph,
            &dir,
            "lib/math.ts::add",
            "### Summary\nAdds two numbers together.\n### Behavior\nReturns the sum of the two arguments.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "lib/math.ts::sub",
            "### Summary\nSubtracts one number from another.\n### Behavior\nReturns the difference of the two arguments.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "lib/math.ts",
            "Small arithmetic helper functions.",
        )
        .unwrap();

        // Remove `sub` from the source but keep the file itself -- the
        // file is still current for `add`, so it must not be reported as
        // an orphaned *file*; only `sub`'s now-dangling section should be
        // flagged.
        std::fs::write(
            dir.join("lib/math.ts"),
            "export function add(a: number, b: number): number {\n  return a + b;\n}\n",
        )
        .unwrap();
        let graph_after = crate::index::RepoIndex::build(&dir)
            .unwrap()
            .rebuild()
            .unwrap();

        let orphans = find_orphaned_specs(&graph_after, &dir, None).unwrap();
        assert!(
            orphans.contains(&OrphanedSpec {
                id: "lib/math.ts::sub".to_string(),
                kind: "symbol".to_string(),
                path: "docs/specs/lib/math.ts.md".to_string(),
            }),
            "sub's dangling section should be flagged: {orphans:?}"
        );
        assert!(
            !orphans.iter().any(|o| o.id == "lib/math.ts"),
            "the file itself still exists and is current for `add` -- it must not be reported as a whole-file orphan: {orphans:?}"
        );
        assert!(
            !orphans.iter().any(|o| o.id == "lib/math.ts::add"),
            "add is still live and should never be reported as orphaned: {orphans:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_orphaned_specs_ignores_a_hand_authored_doc_living_under_docs_specs() {
        // Real bug found running this against CodeOwl's own repo, when its
        // style guide still lived at docs/specs/STYLE.md (it is docs/STYLE.md
        // now, but a repo that has not moved its own can still have one
        // there): a hand-written style guide, not a
        // generated file spec -- it has no frontmatter at all, and there
        // is no source file literally named "STYLE". The old
        // implementation treated *any* .md file under docs/specs/ (other
        // than _index.md/_features/) as an implied file spec purely from
        // its path, so it wrongly flagged STYLE as an orphan of a
        // nonexistent "STYLE" source file.
        let (graph, dir) = build_feature_fixture(
            &[("lib/db.ts", "export function query(): void {}\n")],
            "orphan-hand-authored-doc",
        );
        submit(
            &graph,
            &dir,
            "lib/db.ts::query",
            "### Summary\nRuns a database query.\n### Behavior\nReturns the query results.\n",
        )
        .unwrap();
        submit(&graph, &dir, "lib/db.ts", "A small database helper module.").unwrap();

        let style_path = dir.join("docs/specs/STYLE.md");
        std::fs::create_dir_all(style_path.parent().unwrap()).unwrap();
        std::fs::write(
            &style_path,
            "# Spec style for this repo\n\nNo frontmatter here -- this is a hand-written convention doc, not a generated spec.\n",
        )
        .unwrap();

        let orphans = find_orphaned_specs(&graph, &dir, None).unwrap();
        assert!(
            orphans.is_empty(),
            "a hand-authored doc with no spec frontmatter must never be treated as an orphaned file spec: {orphans:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
