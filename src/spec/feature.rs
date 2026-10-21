//! Feature specs: the document, the participants it was written against, and the task and submit pair that generates it.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::format::{read_file_spec, with_provenance, without_provenance, write_replacing};
use super::smells::{body_smells, reject_if_smelly};
use super::staleness::diff_hash_lists;
use crate::features::{
    EntryPoint, Participants, assemble_participants, feature_model_for, feature_slug,
};
use crate::graph::Graph;
use crate::hash::hash_text;

/// A feature spec, kept a lot flatter than `FileSpec` — one document, one
/// `spec_hash` over the whole LLM-written body, since (unlike a file's
/// per-symbol sections) nothing else in the design ever reads back a
/// piece of a feature spec on its own. `body` is everything after the
/// title line onward, title included: "human-friendly titles live inside
/// the document" (`ARCHITECTURE.md`), not synthesized by CodeOwl the way
/// a file spec's `# lib/utils.ts` heading is.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FeatureSpec {
    pub slug: String,
    pub entry_point: String,
    /// Each participant's hash *as observed at generation time* — a
    /// file's `source_hash` for a core participant, a symbol's
    /// `interface_hash` for a dependency (see `Participants`'s doc
    /// comment on why the split matters).
    pub participants: Vec<(String, String)>,
    pub spec_hash: String,
    pub body: String,
}

pub fn feature_spec_path(root: &Path, slug: &str) -> PathBuf {
    root.join("docs")
        .join("specs")
        .join("_features")
        .join(format!("{slug}.md"))
}

pub fn render_feature(spec: &FeatureSpec) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("kind: feature\n");
    out.push_str(&format!("entry_point: {}\n", spec.entry_point));
    out.push_str("participants:\n");
    for (id, hash) in &spec.participants {
        out.push_str(&format!("  {id}: {hash}\n"));
    }
    out.push_str(&format!("spec_hash: {}\n", spec.spec_hash));
    out.push_str("---\n");
    out.push_str(&with_provenance(spec.body.trim()));
    out.push('\n');
    out
}

pub fn parse_feature(content: &str) -> Result<FeatureSpec> {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        bail!("feature spec missing frontmatter opening `---`");
    }

    let mut entry_point = String::new();
    let mut participants = Vec::new();
    let mut spec_hash = String::new();
    let mut in_participants = false;
    let mut consumed = 1;

    for line in lines.by_ref() {
        consumed += 1;
        if line.trim() == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("  ")
            && in_participants
        {
            // rsplit, not split: a symbol participant id contains `::`
            // (colons), but its hash value never does.
            let (id, hash) = rest
                .rsplit_once(':')
                .context("malformed participants entry in feature frontmatter")?;
            participants.push((id.trim().to_string(), hash.trim().to_string()));
            continue;
        }
        in_participants = line.trim() == "participants:";
        if in_participants {
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .context("malformed feature frontmatter line")?;
        match key.trim() {
            "entry_point" => entry_point = value.trim().to_string(),
            "spec_hash" => spec_hash = value.trim().to_string(),
            _ => {}
        }
    }

    let body = without_provenance(
        content
            .lines()
            .skip(consumed)
            .collect::<Vec<_>>()
            .join("\n")
            .trim(),
    );
    let slug = feature_slug(&entry_point);

    Ok(FeatureSpec {
        slug,
        entry_point,
        participants,
        spec_hash,
        body,
    })
}

pub fn read_feature_spec(root: &Path, slug: &str) -> Result<Option<FeatureSpec>> {
    let path = feature_spec_path(root, slug);
    if !path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    parse_feature(&content).map(Some)
}

/// Each participant's hash as the graph currently reports it — compared
/// against a persisted `FeatureSpec.participants` to decide whether a
/// feature spec is current. Also doubles as what a fresh generation
/// records.
pub fn current_participant_hashes(
    graph: &Graph,
    participants: &Participants,
) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for file in &participants.core {
        let id = graph
            .find(file)
            .with_context(|| format!("core participant {file:?} not in graph"))?;
        let f = graph
            .get_file(id)
            .with_context(|| format!("core participant {file:?} is not a file"))?;
        out.push((file.clone(), f.source_hash.clone()));
    }
    for dep in &participants.dependencies {
        let id = graph
            .find(dep)
            .with_context(|| format!("dependency participant {dep:?} not in graph"))?;
        let sym = graph
            .get_symbol(id)
            .with_context(|| format!("dependency participant {dep:?} is not a symbol"))?;
        // Falls back to source_hash on the rare case a resolved import
        // target isn't itself exported (no interface_hash) -- still a
        // meaningful staleness signal, just not the shape-only one.
        let hash = sym
            .interface_hash
            .clone()
            .unwrap_or_else(|| sym.source_hash.clone());
        out.push((dep.clone(), hash));
    }
    for table in &participants.data {
        let id = graph
            .find(table)
            .with_context(|| format!("data participant {table:?} not in graph"))?;
        let sym = graph
            .get_symbol(id)
            .with_context(|| format!("data participant {table:?} is not a symbol"))?;
        // A table has no interface_hash -- its whole definition is its
        // "shape", so source_hash (over the CREATE TABLE span) is the right
        // staleness key: add/drop/rename a column the feature touches and
        // the feature spec goes stale.
        out.push((table.clone(), sym.source_hash.clone()));
    }
    Ok(out)
}

/// A dependency participant's context for the LLM: its own already-
/// generated summary if one exists and is current, otherwise a
/// deterministic stub (signature + docstring) -- the same reference-edge
/// read path "Recursive spec generation" defines for file specs' own
/// dependency context, just reused here (never triggers generation of the
/// dependency itself).
fn dependency_context(graph: &Graph, root: &Path, dep_id: &str) -> Result<String> {
    let Some(sym_id) = graph.find(dep_id) else {
        return Ok(format!("{dep_id} (unresolved)"));
    };
    let Some(sym) = graph.get_symbol(sym_id) else {
        return Ok(format!("{dep_id} (not a symbol)"));
    };
    if let Some(file_id) = sym.parent
        && let Some(file) = graph.get_file(file_id)
        && let Some(spec) = read_file_spec(root, &file.id)?
        && spec
            .symbol_hash(&sym.id)
            .is_some_and(|h| h.source_hash == sym.source_hash)
        && let Some(prose) = spec.section(&sym.id)
    {
        return Ok(prose.summary.clone());
    }
    Ok(format!(
        "{}{}",
        sym.signature,
        sym.docstring
            .as_ref()
            .map(|d| format!(" -- {d}"))
            .unwrap_or_default()
    ))
}

/// One unit of work for a feature entry point -- unlike a file's bottom-up
/// symbol-then-file chase, a feature spec is generated in a single task:
/// there's exactly one document, one `spec_hash`, no per-participant
/// subsections needing their own LLM call.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureTask {
    pub slug: String,
    pub entry_point: String,
    /// (file id, raw source) for the feature's own code -- the entry
    /// point plus whatever it reaches via route-literal edges.
    pub core_sources: Vec<(String, String)>,
    /// (symbol id, summary-or-stub) for what that code depends on.
    pub dependencies: Vec<(String, String)>,
    /// (table symbol id, `table(col, col, ...)`) for each SQL table the
    /// core code queries via a resolved `.from("table")` -- so the "Data
    /// touched" section names real columns instead of guessing from
    /// scattered `.select()` calls (M10/M11).
    pub data: Vec<(String, String)>,
}

/// The feature task for `entry`, or `None` if its spec is already current
/// (every participant's hash matches, and the participant set itself
/// hasn't changed -- a new `fetch()` literal appearing is a real change
/// even though no existing participant moved).
pub fn next_feature_task(
    graph: &Graph,
    root: &Path,
    entry: &EntryPoint,
) -> Result<Option<FeatureTask>> {
    let Some(fm) = feature_model_for(graph) else {
        return Ok(None);
    };
    let participants = assemble_participants(graph, fm, entry);
    let current = current_participant_hashes(graph, &participants)?;

    let existing = read_feature_spec(root, &entry.id)?;
    // A quality smell is a hash-invisible reason to revisit this feature
    // even when every participant's hash still matches -- see "Quality
    // smells" in ARCHITECTURE.md.
    let is_current = existing.is_some_and(|spec| {
        diff_hash_lists(&current, &spec.participants).is_empty()
            && body_smells(&spec.body).is_empty()
    });
    if is_current {
        return Ok(None);
    }

    let mut core_sources = Vec::new();
    for file in &participants.core {
        let path = root.join(file);
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        core_sources.push((file.clone(), source));
    }

    let mut dependencies = Vec::new();
    for dep in &participants.dependencies {
        dependencies.push((dep.clone(), dependency_context(graph, root, dep)?));
    }

    let mut data = Vec::new();
    for table in &participants.data {
        let signature = graph
            .find(table)
            .and_then(|id| graph.get_symbol(id))
            .map(|s| s.signature.clone())
            .unwrap_or_default();
        data.push((table.clone(), signature));
    }

    Ok(Some(FeatureTask {
        slug: entry.id.clone(),
        entry_point: entry.file.clone(),
        core_sources,
        dependencies,
        data,
    }))
}

/// Persist `content` (the agent's LLM-written feature narrative, title
/// included) as `entry_id`'s feature spec.
///
/// **Resolves the entry by its `id` (the feature slug), never by `file`.**
/// A file-based lookup (`.find(|e| e.file == …)`) is ambiguous the moment
/// a stack's entry points can share a file — several `@router` decorators
/// in one FastAPI module (M17), where the pilot's Next.js pages/routes
/// were always one-per-file and never exposed this. The pre-M17 signature
/// took the entry's *file* and re-derived "which entry" from it, which
/// silently resolved to the wrong sibling entry once a file had several —
/// misattributing (and overwriting) a *different* feature's spec on disk.
/// `get_next_spec_task`'s `next_feature_task_response` had the identical
/// bug on the read side (fixed separately); this is the write-side twin,
/// found by the `full-stack-fastapi-template` dogfood.
pub fn submit_feature(
    graph: &Graph,
    root: &Path,
    entry_id: &str,
    content: &str,
) -> Result<FeatureSpec> {
    let body = without_provenance(content.trim());
    if body.is_empty() {
        bail!("submitted feature content is empty");
    }
    if !body.starts_with("# ") {
        bail!("submitted feature content must start with a `# Title` heading");
    }
    reject_if_smelly(
        &format!("submitted feature spec for {entry_id:?}"),
        body_smells(&body),
    )?;

    let Some(fm) = feature_model_for(graph) else {
        bail!("this stack has no feature layer — nothing to submit a feature spec against");
    };
    let entry = fm
        .enumerate_entry_points(graph)
        .into_iter()
        .find(|e| e.id == entry_id)
        .with_context(|| format!("no feature entry point with slug {entry_id:?}"))?;
    let slug = entry.id.clone();
    let entry_file = entry.file.clone();
    let participants = assemble_participants(graph, fm, &entry);
    let hashes = current_participant_hashes(graph, &participants)?;

    let spec = FeatureSpec {
        slug: slug.clone(),
        entry_point: entry_file,
        participants: hashes,
        spec_hash: hash_text(&body),
        body,
    };

    let path = feature_spec_path(root, &slug);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    write_replacing(&path, &render_feature(&spec))?;

    Ok(spec)
}

/// Each currently-enumerated feature entry point's own current
/// `spec_hash` (empty if that feature isn't itself current yet) — the
/// system spec's per-feature half of its staleness key.
pub fn current_feature_hashes(graph: &Graph, root: &Path) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let Some(fm) = feature_model_for(graph) else {
        return Ok(out); // a stack with no feature layer has no feature half
    };
    for entry in fm.enumerate_entry_points(graph) {
        let participants = assemble_participants(graph, fm, &entry);
        let current = current_participant_hashes(graph, &participants)?;
        let hash = read_feature_spec(root, &entry.id)?
            .filter(|s| diff_hash_lists(&current, &s.participants).is_empty())
            .map(|s| s.spec_hash)
            .unwrap_or_default();
        out.push((entry.id, hash));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::enumerate_entry_points;
    use crate::spec::format::PROVENANCE;

    use crate::spec::test_support::{ARTWORK_FIXTURE, build_feature_fixture};

    #[test]
    fn feature_render_then_parse_round_trips() {
        let spec = FeatureSpec {
            slug: "submit".to_string(),
            entry_point: "app/submit/page.tsx".to_string(),
            participants: vec![
                ("app/submit/page.tsx".to_string(), "filehash1".to_string()),
                (
                    "app/api/submit-artwork/route.ts".to_string(),
                    "filehash2".to_string(),
                ),
                (
                    "lib/supabase.ts::getSupabase".to_string(),
                    "ifacehash".to_string(),
                ),
            ],
            spec_hash: "spechash".to_string(),
            body: "# Artwork submission\n## Summary\nLets an artist submit artwork.".to_string(),
        };
        let rendered = render_feature(&spec);
        let parsed = parse_feature(&rendered).unwrap();
        assert_eq!(parsed, spec);
    }

    #[test]
    fn a_feature_spec_carries_the_provenance_line_and_its_body_is_unchanged() {
        let spec = FeatureSpec {
            slug: "submit".to_string(),
            entry_point: "app/page.tsx".to_string(),
            participants: vec![],
            spec_hash: "spechash".to_string(),
            body: "# Artwork submission\n## Summary\nLets an artist submit artwork.".to_string(),
        };
        let rendered = render_feature(&spec);
        let lines: Vec<&str> = rendered.lines().collect();
        let title = lines
            .iter()
            .position(|l| *l == "# Artwork submission")
            .unwrap();
        assert_eq!(lines[title + 1], PROVENANCE);
        assert_eq!(parse_feature(&rendered).unwrap().body, spec.body);
        let legacy = rendered.replace(&format!("{PROVENANCE}\n"), "");
        assert_eq!(parse_feature(&legacy).unwrap().body, spec.body);
    }

    #[test]
    fn resubmitting_a_rendered_feature_does_not_double_the_provenance_line() {
        let (graph, dir) = build_feature_fixture(ARTWORK_FIXTURE, "provenance-resubmit");
        let rendered = format!(
            "# Artwork submission\n{PROVENANCE}\n## Summary\nLets an artist submit artwork.\n"
        );
        let spec = submit_feature(&graph, &dir, "submit", &rendered).unwrap();
        assert!(!spec.body.contains(PROVENANCE), "{}", spec.body);
        let on_disk = std::fs::read_to_string(feature_spec_path(&dir, "submit")).unwrap();
        assert_eq!(on_disk.matches(PROVENANCE).count(), 1);
        // Same hash as the same text submitted without the line.
        let plain = submit_feature(
            &graph,
            &dir,
            "submit",
            "# Artwork submission\n## Summary\nLets an artist submit artwork.\n",
        )
        .unwrap();
        assert_eq!(plain.spec_hash, spec.spec_hash);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn feature_generate_loop_produces_a_current_spec_then_reports_done() {
        let (graph, dir) = build_feature_fixture(ARTWORK_FIXTURE, "1");
        let entry = enumerate_entry_points(&graph)
            .into_iter()
            .find(|e| e.id == "submit")
            .unwrap();

        let task = next_feature_task(&graph, &dir, &entry)
            .unwrap()
            .expect("first run should need generation");
        assert_eq!(task.slug, "submit");
        assert_eq!(
            task.core_sources
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>(),
            vec!["app/submit/page.tsx", "app/api/submit-artwork/route.ts"]
        );
        assert_eq!(
            task.dependencies,
            vec![(
                "lib/supabase.ts::getSupabase".to_string(),
                "function getSupabase(): void".to_string()
            )]
        );

        submit_feature(
            &graph,
            &dir,
            "submit",
            "# Artwork submission\n## Summary\nLets an artist submit artwork.\n",
        )
        .unwrap();

        assert!(feature_spec_path(&dir, "submit").exists());
        assert_eq!(next_feature_task(&graph, &dir, &entry).unwrap(), None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submit_feature_rejects_content_without_a_title() {
        let (graph, dir) = build_feature_fixture(ARTWORK_FIXTURE, "2");
        let result = submit_feature(&graph, &dir, "submit", "no title here, just prose");
        assert!(result.is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submit_feature_errors_loudly_on_an_id_matching_no_entry_point() {
        // Code-review finding: an unmatched entry_id used to fall back to a
        // fabricated EntryPoint whose `file` field was the raw id/slug
        // string itself (a leftover from entry_file -> entry_id being
        // renamed without updating this fallback's semantics) -- not a
        // real path. The dangerous case is an entry_id that happens to
        // collide with a *real* node in the graph that isn't a feature
        // entry point at all -- here, `lib/supabase.ts`, a real file in
        // the fixture, but not the fixture's one real entry point
        // ("submit"). Under the fallback, assemble_participants/
        // current_participant_hashes both succeed against that
        // coincidentally-real file, silently persisting a nonsensical
        // "feature" spec whose slug and entry_point are just a random
        // file's path. mcp.rs's sole production caller pre-validates the
        // id today, so this was unreachable there, but submit_feature is
        // `pub fn` and directly callable (as this test is) -- it must
        // fail loudly on its own, not rely on every future caller
        // re-implementing the same pre-validation.
        let (graph, dir) = build_feature_fixture(ARTWORK_FIXTURE, "no-such-entry");
        let result = submit_feature(
            &graph,
            &dir,
            "lib/supabase.ts",
            "# Not a feature\n## Summary\nThis id names a real file, not an entry point.\n",
        );
        let err = result.expect_err(
            "an id that names a real file/symbol but no feature entry point must still error",
        );
        assert!(
            err.to_string().contains("lib/supabase.ts"),
            "the error should name the id that failed to match, got: {err}"
        );
        assert!(
            !feature_spec_path(&dir, "lib/supabase.ts").exists(),
            "no spec file should be written for an id that was never a real entry point"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
