//! Directory rollup specs, and the module enumeration the system spec and the coverage report both use.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::file_task::{SpecTask, next_task};
use super::format::{
    PROVENANCE, extract_section, linked_title, okf_frontmatter, path_from_doc, read_file_spec,
    write_replacing,
};
use super::layout::{
    directory_is_spec_bearing, file_is_spec_bearing, files_in, is_test_path, spec_bearing_files_in,
};
use super::smells::{body_smells, reject_if_smelly};
use super::staleness::{diff_hash_lists, file_changes};
use crate::graph::Graph;
use crate::hash::hash_text;

/// A directory rollup's persisted shape (M6). Deliberately closer to
/// `FeatureSpec` than `FileSpec`: one document, one `spec_hash`, no
/// per-entry subsections — a directory has no source of its own to
/// section, only containment children (its files) to compose from. See
/// `ARCHITECTURE.md`'s "Spec document format" for the decided template.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RollupSpec {
    pub dir_path: String,
    /// Each of the directory's spec-bearing files, keyed on that file's own
    /// current `spec_hash` — not `source_hash`. Containment children
    /// contribute the hash of their own *spec* (see "Caching and
    /// invalidation"), so this only moves once a file's spec is actually
    /// rewritten, never merely because its source changed but hasn't been
    /// regenerated yet.
    pub files: Vec<(String, String)>,
    pub spec_hash: String,
    /// The LLM-written `## Summary` prose only — the `## Contents` listing
    /// is CodeOwl-written and recomputed fresh on every render, the same
    /// way a file spec's signature/dependency lines are (see
    /// `render_rollup`).
    pub body: String,
}

pub fn rollup_spec_path(root: &Path, dir_path: &str) -> PathBuf {
    root.join("docs")
        .join("specs")
        .join(dir_path)
        .join("_index.md")
}

/// The title line of a rollup: its directory, linked to that directory (the
/// path `resource:` holds).
pub(super) fn rollup_title_line(dir_path: &str) -> String {
    linked_title(dir_path, &path_from_doc(dir_path, dir_path))
}

/// The OKF keys of a rollup: the same text a fresh render writes and the
/// migration adds to a rollup written before them.
pub(super) fn rollup_okf_frontmatter(dir_path: &str) -> String {
    okf_frontmatter(
        "rollup",
        dir_path,
        &format!("Overview of the `{dir_path}` directory."),
        Some(&path_from_doc(dir_path, dir_path)),
        &[],
    )
}

/// Render a `RollupSpec` to markdown+frontmatter. `root` is needed to pull
/// each file's own current summary fresh for the `## Contents` list, the
/// same "recompute, don't store" pattern `render`'s dependency lines use.
pub fn render_rollup(graph: &Graph, root: &Path, spec: &RollupSpec) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&rollup_okf_frontmatter(&spec.dir_path));
    out.push_str(&format!("dir: {}\n", spec.dir_path));
    out.push_str("files:\n");
    for (path, hash) in &spec.files {
        out.push_str(&format!("  {path}: {hash}\n"));
    }
    out.push_str(&format!("spec_hash: {}\n", spec.spec_hash));
    out.push_str("---\n");
    out.push_str(&format!(
        "{}\n{PROVENANCE}\n",
        rollup_title_line(&spec.dir_path)
    ));
    out.push_str("## Summary\n");
    out.push_str(spec.body.trim());
    out.push('\n');
    out.push_str("\n## Contents\n");
    for id in files_in(graph, &spec.dir_path) {
        let file = graph.get_file(id).expect("filtered to file ids");
        if file_is_spec_bearing(graph, id) {
            let summary = read_file_spec(root, &file.id)
                .ok()
                .flatten()
                .map(|s| s.file_summary)
                .unwrap_or_default();
            let blurb = summary.lines().next().unwrap_or("").trim();
            out.push_str(&format!("- `{}` — {blurb}\n", file.id));
        } else {
            out.push_str(&format!(
                "- `{}` — (no document; not spec-bearing)\n",
                file.id
            ));
        }
    }
    out
}

pub fn parse_rollup(content: &str) -> Result<RollupSpec> {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        bail!("rollup spec missing frontmatter opening `---`");
    }

    let mut dir_path = String::new();
    let mut files = Vec::new();
    let mut spec_hash = String::new();
    let mut in_files = false;
    let mut consumed = 1;

    for line in lines.by_ref() {
        consumed += 1;
        if line.trim() == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("  ")
            && in_files
        {
            let (path, hash) = rest
                .rsplit_once(':')
                .context("malformed files entry in rollup frontmatter")?;
            files.push((path.trim().to_string(), hash.trim().to_string()));
            continue;
        }
        in_files = line.trim() == "files:";
        if in_files {
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .context("malformed rollup frontmatter line")?;
        match key.trim() {
            "dir" => dir_path = value.trim().to_string(),
            "spec_hash" => spec_hash = value.trim().to_string(),
            _ => {}
        }
    }

    let body_all: String = content
        .lines()
        .skip(consumed)
        .collect::<Vec<_>>()
        .join("\n");
    let body = extract_section(&body_all, "## Summary", "\n## ").unwrap_or_default();

    Ok(RollupSpec {
        dir_path,
        files,
        spec_hash,
        body,
    })
}

pub fn read_rollup_spec(root: &Path, dir_path: &str) -> Result<Option<RollupSpec>> {
    let path = rollup_spec_path(root, dir_path);
    if !path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    parse_rollup(&content).map(Some)
}

/// Each of `dir_path`'s spec-bearing files paired with its own current
/// `spec_hash` (empty if that file has no current spec yet) — what a
/// persisted `RollupSpec.files` is compared against to decide staleness,
/// and what a fresh generation records.
pub fn current_file_hashes(
    graph: &Graph,
    root: &Path,
    dir_path: &str,
) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for id in spec_bearing_files_in(graph, dir_path) {
        let file = graph.get_file(id).expect("filtered to file ids");
        let hash = read_file_spec(root, &file.id)?
            .filter(|s| file_changes(graph, id, &s.file).is_empty())
            .map(|s| s.file.spec_hash)
            .unwrap_or_default();
        out.push((file.id.clone(), hash));
    }
    Ok(out)
}

/// One unit of work for a directory rollup — like a feature spec, a single
/// document generated in one task, not a bottom-up ladder: composed
/// entirely from its files' own already-generated summaries (containment
/// children contribute their own spec, never their raw source — see
/// "Bottom-up composition"). Only ever produced once every file under the
/// directory is itself current (see `next_task_for_directory`).
#[derive(Debug, Clone, PartialEq)]
pub struct RollupTask {
    pub dir_path: String,
    /// (file path, that file's own current `## Summary` prose).
    pub files: Vec<(String, String)>,
}

/// Walks `dir_path`'s own spec-bearing files' bottom-up ladders (`next_task`,
/// in path-sorted order), returning the first uncovered symbol or file
/// task — the same "symbols before their file" recursion, one level up:
/// files before their directory's rollup. `None` once every file under
/// `dir_path` is fully current, at which point `next_rollup_task` decides
/// whether the rollup document itself still needs writing.
pub fn next_task_for_directory(
    graph: &Graph,
    root: &Path,
    dir_path: &str,
) -> Result<Option<SpecTask>> {
    for file_id in spec_bearing_files_in(graph, dir_path) {
        if let Some(task) = next_task(graph, root, file_id)? {
            return Ok(Some(task));
        }
    }
    Ok(None)
}

/// The rollup task for `dir_path`, or `None` if it isn't spec-bearing (per
/// `directory_is_spec_bearing`), one or more of its files aren't
/// themselves current yet (call `next_task_for_directory` first — this
/// mirrors `next_task` never returning a `File` task while a symbol is
/// still uncovered), or the rollup is already current (every spec-bearing
/// file's `spec_hash` matches what's on record, and the file set itself
/// hasn't changed). Stateless and safe to call standalone at any point.
pub fn next_rollup_task(graph: &Graph, root: &Path, dir_path: &str) -> Result<Option<RollupTask>> {
    if dir_path.is_empty() {
        bail!(
            "the repo root's rollup would collide with the reserved system-spec path \
             (docs/specs/_index.md) -- not supported until a system spec milestone exists"
        );
    }
    if !directory_is_spec_bearing(graph, dir_path) {
        return Ok(None);
    }

    let current = current_file_hashes(graph, root, dir_path)?;
    if current.iter().any(|(_, h)| h.is_empty()) {
        return Ok(None);
    }
    let existing = read_rollup_spec(root, dir_path)?;
    let is_current = existing.is_some_and(|spec| {
        diff_hash_lists(&current, &spec.files).is_empty() && body_smells(&spec.body).is_empty()
    });
    if is_current {
        return Ok(None);
    }

    let mut files = Vec::new();
    for id in spec_bearing_files_in(graph, dir_path) {
        let file = graph.get_file(id).expect("filtered to file ids");
        let summary = read_file_spec(root, &file.id)?
            .filter(|s| file_changes(graph, id, &s.file).is_empty())
            .map(|s| s.file_summary)
            .with_context(|| {
                format!(
                    "{} has no current spec yet -- exhaust next_task_for_directory first",
                    file.id
                )
            })?;
        files.push((file.id.clone(), summary));
    }

    Ok(Some(RollupTask {
        dir_path: dir_path.to_string(),
        files,
    }))
}

/// Persist `content` (the agent's LLM-written directory summary — plain
/// prose, becomes the rollup's `## Summary`) for `dir_path`.
pub fn submit_rollup(
    graph: &Graph,
    root: &Path,
    dir_path: &str,
    content: &str,
) -> Result<RollupSpec> {
    if !directory_is_spec_bearing(graph, dir_path) {
        bail!("{dir_path:?} is not a spec-bearing directory (needs >= 2 spec-bearing files)");
    }
    let body = content.trim().to_string();
    if body.is_empty() {
        bail!("submitted rollup content is empty");
    }
    reject_if_smelly(
        &format!("submitted rollup for {dir_path:?}"),
        body_smells(&body),
    )?;

    let files = current_file_hashes(graph, root, dir_path)?;
    if let Some((missing, _)) = files.iter().find(|(_, h)| h.is_empty()) {
        bail!("{missing} has no current spec yet -- generate its file spec first");
    }

    let spec = RollupSpec {
        dir_path: dir_path.to_string(),
        files,
        spec_hash: hash_text(&body),
        body,
    };

    let path = rollup_spec_path(root, dir_path);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    write_replacing(&path, &render_rollup(graph, root, &spec))?;

    Ok(spec)
}

/// Every directory anywhere in the repo that currently qualifies for a
/// rollup (`directory_is_spec_bearing`, at any nesting depth) — the
/// system spec's flat "modules" list (M8). Deliberately flat, not a
/// nested module tree: rollups don't recursively aggregate a
/// subdirectory's rollup into its parent's (a real, explicit scope cut —
/// see `ROADMAP.md`'s M6 note), so the system spec doesn't pretend
/// otherwise by inventing a hierarchy on top of documents that aren't
/// actually structured that way. Excludes the repo root (`""`) even if it
/// has >=2 spec-bearing files directly in it — that path is reserved for
/// a future system-spec document (see `next_rollup_task`'s own guard), so
/// it can never actually be generated as a rollup.
pub fn enumerate_modules(graph: &Graph) -> Vec<String> {
    let mut dirs: Vec<String> = graph
        .files()
        .filter_map(|f| {
            Path::new(&f.id)
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs.retain(|d| {
        !d.is_empty()
            && !is_test_path(graph, &format!("{d}/"))
            && directory_is_spec_bearing(graph, d)
    });
    dirs
}

/// Each module's own current rollup `spec_hash` (empty if that rollup
/// isn't itself current yet) — the system spec's per-module half of its
/// staleness key, in the same "current vs. stored, empty means not ready"
/// shape `current_file_hashes` already uses one level down.
pub fn current_module_hashes(graph: &Graph, root: &Path) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for dir in enumerate_modules(graph) {
        let files = current_file_hashes(graph, root, &dir)?;
        let hash = read_rollup_spec(root, &dir)?
            .filter(|s| diff_hash_lists(&files, &s.files).is_empty())
            .map(|s| s.spec_hash)
            .unwrap_or_default();
        out.push((dir, hash));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::build_graph_from_sources;
    use crate::spec::file_task::{SpecTask, submit};
    use crate::spec::layout::directory_is_spec_bearing;
    use crate::spec::test_support::temp_dir_for;

    use std::path::Path;

    fn rollup_fixture_dir(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "codeowl-rollup-spec-test-{}-{suffix}",
            std::process::id()
        ))
    }

    #[test]
    fn enumerate_modules_excludes_the_repo_root_even_when_spec_bearing() {
        let graph = build_graph_from_sources(&[
            ("one.ts", "export function one(): void {}\n"),
            ("two.ts", "export function two(): void {}\n"),
        ]);
        // The repo root itself has 2 spec-bearing files, so
        // directory_is_spec_bearing("") is true -- but "" is reserved for
        // a future system-spec path (see next_rollup_task's guard), so it
        // must never show up as a generatable module.
        assert!(directory_is_spec_bearing(&graph, ""));
        assert_eq!(enumerate_modules(&graph), Vec::<String>::new());
    }

    #[test]
    fn a_rollup_write_that_cannot_complete_leaves_the_previous_rollup_untouched() {
        let dir = rollup_fixture_dir("atomic2");
        std::fs::create_dir_all(&dir).unwrap();

        let graph = build_graph_from_sources(&[
            ("lib/one.ts", "export function one(): void {}\n"),
            ("lib/two.ts", "export function two(): void {}\n"),
        ]);
        for (id, content) in [
            (
                "lib/one.ts::one",
                "### Summary\nDoes the first specific thing.\n### Behavior\nRuns synchronously with no side effects.\n",
            ),
            (
                "lib/two.ts::two",
                "### Summary\nDoes a second, different thing.\n### Behavior\nAlso runs synchronously with no side effects.\n",
            ),
            ("lib/one.ts", "Performs one specific, deliberate task."),
            ("lib/two.ts", "Performs a couple of related tasks."),
        ] {
            submit(&graph, &dir, id, content).unwrap();
        }
        submit_rollup(
            &graph,
            &dir,
            "lib",
            "Small shared helper functions for this module.",
        )
        .unwrap();
        let path = rollup_spec_path(&dir, "lib");
        let before = std::fs::read_to_string(&path).unwrap();

        std::fs::create_dir_all(format!("{}.tmp", path.display())).unwrap();
        let result = submit_rollup(
            &graph,
            &dir,
            "lib",
            "A rewritten description of this module.",
        );

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn render_rollup_then_parse_rollup_round_trips() {
        let graph = build_graph_from_sources(&[("lib/one.ts", "export function one(): void {}\n")]);
        let spec = RollupSpec {
            dir_path: "lib".to_string(),
            files: vec![("lib/one.ts".to_string(), "onehash".to_string())],
            spec_hash: "rolluphash".to_string(),
            body: "Small shared helpers.".to_string(),
        };
        let rendered = render_rollup(&graph, Path::new("/nonexistent"), &spec);
        assert!(rendered.contains("dir: lib"));
        assert!(rendered.contains("lib/one.ts: onehash"));
        let parsed = parse_rollup(&rendered).expect("should parse what we just rendered");
        assert_eq!(parsed, spec);
    }

    #[test]
    fn a_rollup_carries_the_provenance_line_and_parses_without_it() {
        let graph = build_graph_from_sources(&[("lib/one.ts", "export function one(): void {}\n")]);
        let spec = RollupSpec {
            dir_path: "lib".to_string(),
            files: vec![("lib/one.ts".to_string(), "onehash".to_string())],
            spec_hash: "rolluphash".to_string(),
            body: "Small shared helpers.".to_string(),
        };
        let rendered = render_rollup(&graph, Path::new("/nonexistent"), &spec);
        let lines: Vec<&str> = rendered.lines().collect();
        let title = lines
            .iter()
            .position(|l| *l == "# [`lib`](../../../lib)")
            .unwrap();
        assert_eq!(lines[title + 1], PROVENANCE);
        let legacy = rendered.replace(&format!("{PROVENANCE}\n"), "");
        assert_eq!(parse_rollup(&legacy).unwrap(), spec);
    }

    #[test]
    fn a_rollup_leads_with_the_okf_keys_and_no_longer_writes_kind() {
        let graph = build_graph_from_sources(&[("lib/one.ts", "export function one(): void {}\n")]);
        let spec = RollupSpec {
            dir_path: "lib".to_string(),
            files: vec![("lib/one.ts".to_string(), "onehash".to_string())],
            spec_hash: "rolluphash".to_string(),
            body: "Small shared helpers.".to_string(),
        };
        let rendered = render_rollup(&graph, Path::new("/nonexistent"), &spec);
        let head: Vec<&str> = rendered.lines().take(6).collect();
        assert_eq!(
            head,
            vec![
                "---",
                "type: rollup",
                "title: \"lib\"",
                "description: \"Overview of the `lib` directory.\"",
                "generated: { by: CodeOwl }",
                "resource: \"../../../lib\"",
            ]
        );
        assert!(!rendered.contains("kind:"), "{rendered}");
        assert_eq!(parse_rollup(&rendered).unwrap(), spec);
        let nested = RollupSpec {
            dir_path: "src/spec".to_string(),
            ..spec
        };
        let rendered = render_rollup(&graph, Path::new("/nonexistent"), &nested);
        assert!(
            rendered.contains("resource: \"../../../../src/spec\"\n"),
            "{rendered}"
        );
    }

    #[test]
    fn next_task_for_directory_walks_files_before_the_rollup() {
        let dir = rollup_fixture_dir("1");
        std::fs::create_dir_all(&dir).unwrap();

        let graph = build_graph_from_sources(&[
            ("lib/one.ts", "export function one(): void {}\n"),
            ("lib/two.ts", "export function two(): void {}\n"),
        ]);

        // Neither file has a spec yet -- the first task is one of their
        // own symbols, not the rollup.
        let task = next_task_for_directory(&graph, &dir, "lib")
            .unwrap()
            .expect("files aren't current yet");
        assert_eq!(
            task,
            SpecTask::Symbol {
                id: "lib/one.ts::one".to_string(),
                signature: "function one(): void".to_string(),
                docstring: None,
                prior: None,
                previous: None,
            }
        );
        assert_eq!(next_rollup_task(&graph, &dir, "lib").unwrap(), None);

        submit(
            &graph,
            &dir,
            "lib/one.ts::one",
            "### Summary\nDoes the first specific thing.\n### Behavior\nRuns synchronously with no side effects.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "lib/one.ts",
            "Performs one specific, deliberate task.",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "lib/two.ts::two",
            "### Summary\nDoes a second, different thing.\n### Behavior\nAlso runs synchronously with no side effects.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "lib/two.ts",
            "Performs a couple of related tasks.",
        )
        .unwrap();

        // Both files are now current -- the directory chase is exhausted,
        // and the rollup task itself becomes available.
        assert_eq!(next_task_for_directory(&graph, &dir, "lib").unwrap(), None);
        let rollup_task = next_rollup_task(&graph, &dir, "lib")
            .unwrap()
            .expect("both files current, rollup itself still missing");
        assert_eq!(
            rollup_task.files,
            vec![
                (
                    "lib/one.ts".to_string(),
                    "Performs one specific, deliberate task.".to_string()
                ),
                (
                    "lib/two.ts".to_string(),
                    "Performs a couple of related tasks.".to_string()
                ),
            ]
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submit_rollup_writes_the_document_and_records_file_hashes() {
        let dir = rollup_fixture_dir("2");
        std::fs::create_dir_all(&dir).unwrap();

        let graph = build_graph_from_sources(&[
            ("lib/one.ts", "export function one(): void {}\n"),
            ("lib/two.ts", "export function two(): void {}\n"),
        ]);
        for (id, content) in [
            (
                "lib/one.ts::one",
                "### Summary\nDoes the first specific thing.\n### Behavior\nRuns synchronously with no side effects.\n",
            ),
            (
                "lib/two.ts::two",
                "### Summary\nDoes a second, different thing.\n### Behavior\nAlso runs synchronously with no side effects.\n",
            ),
        ] {
            submit(&graph, &dir, id, content).unwrap();
        }
        submit(
            &graph,
            &dir,
            "lib/one.ts",
            "Performs one specific, deliberate task.",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "lib/two.ts",
            "Performs a couple of related tasks.",
        )
        .unwrap();

        let spec = submit_rollup(
            &graph,
            &dir,
            "lib",
            "Small shared helper functions for this module.",
        )
        .unwrap();
        assert_eq!(spec.files.len(), 2);
        assert!(spec.files.iter().all(|(_, h)| !h.is_empty()));

        assert!(rollup_spec_path(&dir, "lib").exists());
        assert_eq!(
            read_rollup_spec(&dir, "lib").unwrap().unwrap().body,
            "Small shared helper functions for this module."
        );
        assert_eq!(next_rollup_task(&graph, &dir, "lib").unwrap(), None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submit_rollup_rejects_a_directory_that_is_not_spec_bearing() {
        let dir = rollup_fixture_dir("3");
        std::fs::create_dir_all(&dir).unwrap();
        let graph = build_graph_from_sources(&[(
            "app/api/submit/route.ts",
            "export function GET(): void {}\n",
        )]);
        let result = submit_rollup(&graph, &dir, "app/api/submit", "A route.");
        assert!(result.is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_rollup_stays_buildable_when_a_files_text_changed_but_its_fingerprint_did_not() {
        let dir = temp_dir_for("fp-rollup");
        let v1 = "export function f(): number { return 1; }\n";
        let v2 = "export function f(): number { return 1; }\n// a trailing comment\n";
        let other = "export function g(): number { return 2; }\n";

        let mut g1 = build_graph_from_sources(&[("lib/a.ts", v1), ("lib/b.ts", other)]);
        g1.set_file_fingerprint("lib/a.ts", "fp-a");
        g1.set_file_fingerprint("lib/b.ts", "fp-b");
        for (id, prose) in [
            (
                "lib/a.ts::f",
                "### Summary\nReturns the number one.\n### Behavior\nIt always evaluates to one.\n",
            ),
            (
                "lib/b.ts::g",
                "### Summary\nReturns the number two.\n### Behavior\nIt always evaluates to two.\n",
            ),
            ("lib/a.ts", "File a returns the number one."),
            ("lib/b.ts", "File b returns the number two."),
        ] {
            submit(&g1, &dir, id, prose).unwrap();
        }

        // a.ts gains a comment (raw text moves) but keeps its fingerprint.
        let mut g2 = build_graph_from_sources(&[("lib/a.ts", v2), ("lib/b.ts", other)]);
        g2.set_file_fingerprint("lib/a.ts", "fp-a");
        g2.set_file_fingerprint("lib/b.ts", "fp-b");
        assert_ne!(
            g1.get_file(g1.find("lib/a.ts").unwrap())
                .unwrap()
                .source_hash,
            g2.get_file(g2.find("lib/a.ts").unwrap())
                .unwrap()
                .source_hash,
            "the raw text really did change"
        );

        assert_eq!(next_task_for_directory(&g2, &dir, "lib").unwrap(), None);
        let rollup = next_rollup_task(&g2, &dir, "lib")
            .expect("both file specs are still current, so the rollup can be built")
            .expect("the rollup was never written");
        assert_eq!(rollup.files.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
