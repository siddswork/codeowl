//! The one system spec: its document, and the task and submit pair that generates it once every module and feature is current.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::feature::{current_feature_hashes, read_feature_spec};
use super::format::{extract_section, write_replacing};
use super::rollup::{current_module_hashes, read_rollup_spec};
use super::smells::{body_smells, reject_if_smelly};
use super::staleness::diff_hash_lists;
use crate::graph::Graph;
use crate::hash::hash_text;

/// The whole-repo document (M8) — see `ARCHITECTURE.md`'s "System spec
/// shape". Exactly one per repo; addressed by the fixed pseudo-id
/// `"system"`, the same way a feature is addressed by `"feature:<slug>"`.
/// `body` is the LLM-written title-plus-summary, title included (like
/// `FeatureSpec.body`) — the `## Modules`/`## Features` listings
/// underneath are CodeOwl-written and never stored, recomputed fresh on
/// every render the same way a rollup's `## Contents` is.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SystemSpec {
    pub modules: Vec<(String, String)>,
    pub features: Vec<(String, String)>,
    pub spec_hash: String,
    pub body: String,
}

pub fn system_spec_path(root: &Path) -> PathBuf {
    root.join("docs").join("specs").join("_index.md")
}

pub fn render_system(root: &Path, spec: &SystemSpec) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("kind: system\n");
    out.push_str("modules:\n");
    for (dir, hash) in &spec.modules {
        out.push_str(&format!("  {dir}: {hash}\n"));
    }
    out.push_str("features:\n");
    for (slug, hash) in &spec.features {
        out.push_str(&format!("  {slug}: {hash}\n"));
    }
    out.push_str(&format!("spec_hash: {}\n", spec.spec_hash));
    out.push_str("---\n");
    out.push_str(spec.body.trim());
    out.push('\n');

    out.push_str("\n## Modules\n");
    for (dir, _) in &spec.modules {
        let summary = read_rollup_spec(root, dir)
            .ok()
            .flatten()
            .map(|s| s.body)
            .unwrap_or_default();
        let blurb = summary.lines().next().unwrap_or("").trim();
        out.push_str(&format!("- `{dir}` — {blurb}\n"));
    }

    out.push_str("\n## Features\n");
    for (slug, _) in &spec.features {
        let feature = read_feature_spec(root, slug).ok().flatten();
        let title = feature
            .as_ref()
            .and_then(|f| f.body.lines().next())
            .map(|l| l.trim_start_matches('#').trim().to_string())
            .unwrap_or_else(|| slug.clone());
        let summary = feature
            .as_ref()
            .and_then(|f| extract_section(&f.body, "## Summary", "\n## "))
            .and_then(|s| s.lines().next().map(str::to_string))
            .unwrap_or_default();
        out.push_str(&format!("- [{title}](_features/{slug}.md) — {summary}\n"));
    }
    out
}

pub fn parse_system(content: &str) -> Result<SystemSpec> {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        bail!("system spec missing frontmatter opening `---`");
    }

    let mut modules = Vec::new();
    let mut features = Vec::new();
    let mut spec_hash = String::new();
    let mut section = "";
    let mut consumed = 1;

    for line in lines.by_ref() {
        consumed += 1;
        if line.trim() == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("  ") {
            match section {
                "modules" => {
                    let (id, hash) = rest
                        .rsplit_once(':')
                        .context("malformed modules entry in system frontmatter")?;
                    modules.push((id.trim().to_string(), hash.trim().to_string()));
                    continue;
                }
                "features" => {
                    let (id, hash) = rest
                        .rsplit_once(':')
                        .context("malformed features entry in system frontmatter")?;
                    features.push((id.trim().to_string(), hash.trim().to_string()));
                    continue;
                }
                _ => {}
            }
        }
        match line.trim() {
            "modules:" => {
                section = "modules";
                continue;
            }
            "features:" => {
                section = "features";
                continue;
            }
            _ => {}
        }
        let (key, value) = line
            .split_once(':')
            .context("malformed system frontmatter line")?;
        if key.trim() == "spec_hash" {
            spec_hash = value.trim().to_string();
        }
    }

    // `body` is everything up through the LLM-written `## Summary` --
    // title included, the CodeOwl-written `## Modules`/`## Features`
    // listings after it excluded, the same way `render_system` never
    // stores them either.
    let full_body: String = content
        .lines()
        .skip(consumed)
        .collect::<Vec<_>>()
        .join("\n");
    let body = full_body
        .split("\n## Modules")
        .next()
        .unwrap_or(&full_body)
        .trim()
        .to_string();

    Ok(SystemSpec {
        modules,
        features,
        spec_hash,
        body,
    })
}

pub fn read_system_spec(root: &Path) -> Result<Option<SystemSpec>> {
    let path = system_spec_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    parse_system(&content).map(Some)
}

/// One unit of work for the system spec — a single document, single
/// `spec_hash`, generated in one task like a feature or rollup: composed
/// purely from every module's and every feature's own already-generated
/// summary, never raw source.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemTask {
    /// (module dir, that module's own rollup summary)
    pub modules: Vec<(String, String)>,
    /// (feature slug, "<title> -- <summary>")
    pub features: Vec<(String, String)>,
}

/// The system task, or `None` if it's already current, or if any module's
/// rollup or any feature's spec isn't itself current yet — callers should
/// exhaust each module's and each feature's own chase first, the same
/// "children before parent" order every other document kind uses.
pub fn next_system_task(graph: &Graph, root: &Path) -> Result<Option<SystemTask>> {
    let current_modules = current_module_hashes(graph, root)?;
    let current_features = current_feature_hashes(graph, root)?;
    if current_modules.iter().any(|(_, h)| h.is_empty())
        || current_features.iter().any(|(_, h)| h.is_empty())
    {
        return Ok(None);
    }

    let existing = read_system_spec(root)?;
    let is_current = existing.is_some_and(|spec| {
        let smells_clean = body_smells(&spec.body).is_empty();
        let mut current_all = current_modules.clone();
        current_all.extend(current_features.clone());
        let mut stored_all = spec.modules;
        stored_all.extend(spec.features);
        diff_hash_lists(&current_all, &stored_all).is_empty() && smells_clean
    });
    if is_current {
        return Ok(None);
    }

    let mut modules = Vec::new();
    for (dir, _) in &current_modules {
        let summary = read_rollup_spec(root, dir)?
            .map(|s| s.body)
            .with_context(|| format!("{dir} has no current rollup yet"))?;
        modules.push((dir.clone(), summary));
    }
    let mut features = Vec::new();
    for (slug, _) in &current_features {
        let feature = read_feature_spec(root, slug)?
            .with_context(|| format!("{slug} has no current feature spec yet"))?;
        let title = feature
            .body
            .lines()
            .next()
            .map(|l| l.trim_start_matches('#').trim().to_string())
            .unwrap_or_else(|| slug.clone());
        let summary = extract_section(&feature.body, "## Summary", "\n## ").unwrap_or_default();
        features.push((slug.clone(), format!("{title} -- {summary}")));
    }

    Ok(Some(SystemTask { modules, features }))
}

/// Persist `content` (the agent's LLM-written product narrative, title
/// included) as the system spec.
pub fn submit_system(graph: &Graph, root: &Path, content: &str) -> Result<SystemSpec> {
    let body = content.trim().to_string();
    if body.is_empty() {
        bail!("submitted system content is empty");
    }
    if !body.starts_with("# ") {
        bail!("submitted system content must start with a `# Title` heading");
    }
    reject_if_smelly("submitted system spec", body_smells(&body))?;

    let spec = SystemSpec {
        modules: current_module_hashes(graph, root)?,
        features: current_feature_hashes(graph, root)?,
        spec_hash: hash_text(&body),
        body,
    };

    let path = system_spec_path(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    write_replacing(&path, &render_system(root, &spec))?;

    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::spec::feature::submit_feature;
    use crate::spec::file_task::submit;
    use crate::spec::layout::directory_is_spec_bearing;
    use crate::spec::rollup::submit_rollup;
    use crate::spec::test_support::{SYSTEM_FIXTURE, build_feature_fixture};

    use std::path::Path;

    #[test]
    fn render_system_then_parse_system_round_trips() {
        let spec = SystemSpec {
            modules: vec![("lib/email".to_string(), "rolluphash".to_string())],
            features: vec![("submit".to_string(), "featurehash".to_string())],
            spec_hash: "systemhash".to_string(),
            body: "# Acme\n## Summary\nA competition platform.".to_string(),
        };
        let rendered = render_system(Path::new("/nonexistent"), &spec);
        assert!(rendered.contains("lib/email: rolluphash"));
        assert!(rendered.contains("submit: featurehash"));
        let parsed = parse_system(&rendered).expect("should parse what we just rendered");
        assert_eq!(parsed, spec);
    }

    #[test]
    fn next_system_task_is_none_until_every_module_and_feature_is_current() {
        let (graph, dir) = build_feature_fixture(SYSTEM_FIXTURE, "system1");

        assert_eq!(
            next_system_task(&graph, &dir).unwrap(),
            None,
            "nothing generated yet"
        );

        for (sym_id, file_id) in [
            ("lib/supabase.ts::getSupabase", "lib/supabase.ts"),
            ("lib/one.ts::one", "lib/one.ts"),
            ("lib/two.ts::two", "lib/two.ts"),
        ] {
            submit(
                &graph,
                &dir,
                sym_id,
                "### Summary\nDoes one small, specific job.\n### Behavior\nRuns without any side effects.\n",
            )
            .unwrap();
            submit(&graph, &dir, file_id, "A small file of helper functions.").unwrap();
        }
        assert!(directory_is_spec_bearing(&graph, "lib"));
        assert_eq!(
            next_system_task(&graph, &dir).unwrap(),
            None,
            "lib's own rollup isn't generated yet"
        );
        submit_rollup(
            &graph,
            &dir,
            "lib",
            "Small shared helper functions for this module.",
        )
        .unwrap();

        assert_eq!(
            next_system_task(&graph, &dir).unwrap(),
            None,
            "the feature isn't generated yet"
        );

        submit(
            &graph,
            &dir,
            "app/submit/page.tsx::Page",
            "### Summary\nRenders the artwork submission form.\n### Behavior\nSubmits data to the API on save.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "app/submit/page.tsx",
            "The artwork submission page for competitors.",
        )
        .unwrap();
        submit_feature(
            &graph,
            &dir,
            "submit",
            "# Artwork submission\n## Summary\nLets an artist submit artwork.\n",
        )
        .unwrap();

        let task = next_system_task(&graph, &dir)
            .unwrap()
            .expect("everything current, system task should now appear");
        assert_eq!(
            task.modules,
            vec![(
                "lib".to_string(),
                "Small shared helper functions for this module.".to_string()
            )]
        );
        assert_eq!(task.features.len(), 1);
        assert_eq!(task.features[0].0, "submit");
        assert!(task.features[0].1.contains("Artwork submission"));
        assert!(
            task.features[0]
                .1
                .contains("Lets an artist submit artwork.")
        );

        submit_system(
            &graph,
            &dir,
            "# Acme\n## Summary\nA platform for running art competitions.\n",
        )
        .unwrap();
        assert_eq!(next_system_task(&graph, &dir).unwrap(), None);
        assert!(read_system_spec(&dir).unwrap().is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submit_system_rejects_content_without_a_title() {
        let (graph, dir) = build_feature_fixture(SYSTEM_FIXTURE, "system2");
        let result = submit_system(&graph, &dir, "no title here, just prose");
        assert!(result.is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
