//! One-off tools that rewrite stored specs in place after a format change.

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::feature::{feature_okf_frontmatter, parse_feature};
use super::format::{
    file_okf_frontmatter, file_title_line, has_provenance, linked_dependency_line, parse,
    with_provenance, write_replacing, yaml_flow_item,
};
use super::layout::spec_path;
use super::rollup::{parse_rollup, rollup_okf_frontmatter, rollup_title_line};
use super::staleness::{
    file_dependency_hash, hash_dependency_pairs, shorten_dep_targets, try_dependency_pairs,
    try_file_dependency_pairs,
};
use super::system::{parse_system, system_okf_frontmatter};
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

/// What `migrate_to_okf` found, counted per spec document.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OkfReport {
    /// Documents that got the OKF keys (or would, on a dry run); the four
    /// counts below say of which kind.
    pub migrated: usize,
    pub files: usize,
    pub rollups: usize,
    pub features: usize,
    pub systems: usize,
    /// Documents that got the provenance line under their title (or would):
    /// a spec written before the line existed has none until it is rewritten.
    /// Counted apart from `migrated`; a document can be in both.
    pub provenance_added: usize,
    /// Dependency lines in file specs that were plain text and are now links
    /// (or would be): `links_added` counts the lines, `linked_documents` the
    /// specs they were in. A spec is regenerated only when it is stale, so
    /// without this a current spec would keep plain text indefinitely.
    pub links_added: usize,
    pub linked_documents: usize,
    /// File and rollup specs whose plain title line became a link to the
    /// source file or directory (or would).
    pub titles_linked: usize,
    /// Documents that needed no change: the OKF keys, the provenance line and
    /// the links are there (or there is nothing to put them on), or the page
    /// has a `type:` key that CodeOwl did not write.
    pub unchanged: usize,
    /// Markdown files under `docs/specs/` with no frontmatter (a style guide,
    /// say); left as they are.
    pub no_frontmatter: usize,
    /// Documents left as they are because they could not be read, are of a
    /// kind CodeOwl does not write, or do not parse: each as the path under
    /// `docs/specs/`, a colon, and the reason.
    pub unreadable: Vec<String>,
    /// `docs/specs/STYLE.md` exists: the style guide now lives at
    /// `docs/STYLE.md`, and a file with no frontmatter inside the folder keeps
    /// it from being a conformant bundle.
    pub style_guide_in_bundle: bool,
}

/// Give every spec document under `docs/specs/` the keys Open Knowledge
/// Format asks of it: `kind:` becomes `type:` and the title, description,
/// `generated`, and resource or sources are added, exactly as a fresh render
/// writes them, the provenance line goes under the title of a spec that lacks
/// it, and each plain-text line in a file spec's `### Depends on` list becomes
/// a link to the spec of the file it names where such a spec can exist (the
/// graph says which; see `format::linked_dependency_line`). Nothing else
/// changes: the line and the dependency lists are in no hash and no parser
/// reads them back, so no hash and no prose moves and no spec turns stale. A
/// document that needs none of it is skipped, which makes a second run change
/// nothing, and so is a page with a `type:` key that CodeOwl did not write.
/// With `write` false nothing is written and the report says what would be.
pub fn migrate_to_okf(graph: &Graph, root: &Path, write: bool) -> Result<OkfReport> {
    let specs = root.join("docs").join("specs");
    let mut report = OkfReport {
        style_guide_in_bundle: specs.join("STYLE.md").exists(),
        ..OkfReport::default()
    };
    for path in markdown_files(&specs) {
        let shown = path
            .strip_prefix(&specs)
            .unwrap_or(&path)
            .display()
            .to_string();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                report.unreadable.push(format!("{shown}: {err}"));
                continue;
            }
        };
        if text.lines().next().map(str::trim) != Some("---") {
            report.no_frontmatter += 1;
            continue;
        }
        match with_okf_keys(graph, &text) {
            Ok(None) => report.unchanged += 1,
            Ok(Some(change)) => {
                if let Some(kind) = &change.keys_added_to {
                    report.migrated += 1;
                    match kind.as_str() {
                        "file" => report.files += 1,
                        "rollup" => report.rollups += 1,
                        "feature" => report.features += 1,
                        _ => report.systems += 1,
                    }
                }
                if change.line_added {
                    report.provenance_added += 1;
                }
                if change.links_added > 0 {
                    report.links_added += change.links_added;
                    report.linked_documents += 1;
                }
                if change.title_linked {
                    report.titles_linked += 1;
                }
                if write {
                    write_replacing(&path, &change.text)
                        .with_context(|| format!("updating {}", path.display()))?;
                }
            }
            Err(err) => report.unreadable.push(format!("{shown}: {err:#}")),
        }
    }
    Ok(report)
}

/// What `with_okf_keys` did to a document.
struct Change {
    /// The kind of document, when its frontmatter got the OKF keys.
    keys_added_to: Option<String>,
    /// Whether the provenance line was put under its title.
    line_added: bool,
    /// How many dependency entries were rewritten as the link a fresh render
    /// writes (a plain entry, or a link without its section anchor).
    links_added: usize,
    /// Whether the title line became the link a fresh render writes.
    title_linked: bool,
    text: String,
}

/// `body` with its first line, the title, replaced by the link `expected`
/// when it is the plain `# {text}`. Any other title is left alone: one already
/// linked (it is never removed, or a graph that lacks a file would strip it),
/// and one someone wrote, even if it starts like ours. Returns whether it
/// changed.
fn with_title_link(body: &str, text: &str, expected: &str) -> (String, bool) {
    let (first, rest) = body.split_once('\n').unwrap_or((body, ""));
    let plain = format!("# {text}");
    if first != plain || expected == plain {
        return (body.to_string(), false);
    }
    let separator = if body.contains('\n') { "\n" } else { "" };
    (format!("{expected}{separator}{rest}"), true)
}

/// The target and specifier of a dependency entry, plain (``- `target` —
/// specifier``) or linked (``- [`target`](destination) — specifier``).
fn dependency_entry(line: &str) -> Option<(&str, &str)> {
    if let Some(entry) = line.strip_prefix("- `") {
        let (target, rest) = entry.split_once('`')?;
        return Some((target, rest.strip_prefix(" — ")?));
    }
    let entry = line.strip_prefix("- [`")?;
    let (target, rest) = entry.split_once("`](")?;
    let (_, specifier) = rest.split_once(") — ")?;
    Some((target, specifier))
}

/// `body` with each entry of a `### Depends on` list rewritten as the line a
/// fresh render writes, and how many were changed: a plain entry becomes a
/// link, and a link without the anchor of its section (or with a wrong one) is
/// put right. It adds or corrects a link and never removes one. The
/// `externals:` line, `- (none)`, an entry with nothing to link to (the same
/// file, a file that never gets a spec, a file the graph does not know) that is
/// already plain, and any such text outside a `Depends on` list are left
/// exactly as they are.
fn with_dependency_links(graph: &Graph, from_file: &str, body: &str) -> (String, usize) {
    let mut out = String::with_capacity(body.len());
    let mut in_list = false;
    let mut in_fence = false;
    let mut linked = 0;
    for line in body.split_inclusive('\n') {
        let bare = line.trim_end();
        // A fenced block is example text: a heading or a bullet inside one is
        // not part of the list.
        if bare.trim_start().starts_with("```") {
            in_fence = !in_fence;
        } else if in_fence {
            // keep the line as it is
        } else if bare.starts_with('#') {
            in_list = bare == "### Depends on";
        } else if in_list && let Some((target, specifier)) = dependency_entry(bare) {
            let new = format!(
                "- {}",
                linked_dependency_line(graph, from_file, target, specifier)
            );
            // Add a link, or correct one; never take one away. A graph that is
            // missing a file (it failed to parse, an ignore rule changed) would
            // otherwise un-link every dependency on it.
            let would_unlink = bare.starts_with("- [`") && !new.starts_with("- [`");
            if new != bare && !would_unlink {
                out.push_str(&new);
                out.push_str(&line[bare.len()..]);
                linked += 1;
                continue;
            }
        }
        out.push_str(line);
    }
    (out, linked)
}

/// `body` with the provenance line under its title, and whether it was added.
/// A body that already has the line, or that does not open with a `# ` title,
/// is returned as it is.
fn with_missing_provenance(body: &str) -> (String, bool) {
    if has_provenance(body) {
        return (body.to_string(), false);
    }
    let with = with_provenance(body);
    let added = with != body;
    (with, added)
}

/// `text` brought up to date: its frontmatter converted, the provenance line
/// under its title, and the dependency lines of a file spec as links; `None`
/// when it needs none of that. In the frontmatter the `kind:` line is dropped
/// and the OKF keys go first, and a file spec's `source_paths:` line is
/// written as a fresh render writes it, so a path with brackets becomes valid
/// YAML. The line and the links are in no hash and no parser reads them back.
/// Every other byte stays as it was.
fn with_okf_keys(graph: &Graph, text: &str) -> Result<Option<Change>> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let close = lines
        .iter()
        .skip(1)
        .position(|l| l.trim_end() == "---")
        .map(|i| i + 1)
        .context("the frontmatter is never closed")?;
    let frontmatter = &lines[1..close];
    let doc_type = frontmatter
        .iter()
        .find_map(|l| l.strip_prefix("type:"))
        .map(str::trim);
    if let Some(doc_type) = doc_type {
        // Already has the keys. Touch it further only if CodeOwl wrote it: a
        // hand-written or foreign page with a `type:` key is not "Generated
        // by CodeOwl", and has no dependency list of ours to link.
        let ours = matches!(doc_type, "file" | "rollup" | "feature" | "system")
            && frontmatter
                .iter()
                .any(|l| l.trim_end() == "generated: { by: CodeOwl }");
        if !ours {
            return Ok(None);
        }
    }
    let kind = doc_type
        .or_else(|| {
            frontmatter
                .iter()
                .find_map(|l| l.strip_prefix("kind:"))
                .map(str::trim)
        })
        .context("the frontmatter has no kind")?;
    // A file spec knows which file it is the spec of; its dependency list is
    // linked from there. Links are relative to the spec's own place, so one
    // that does not say which file it is for cannot be linked and is reported.
    let source_path = if kind == "file" {
        let path = parse(text)?.source_path;
        if path.is_empty() {
            bail!("the file spec has no `source_paths`, so its links cannot be worked out");
        }
        Some(path)
    } else {
        None
    };
    // The OKF keys, for a spec that has not got them yet.
    let converted: Option<String> = if doc_type.is_some() {
        None
    } else {
        Some(match kind {
            "file" => file_okf_frontmatter(source_path.as_deref().unwrap_or_default()),
            "rollup" => rollup_okf_frontmatter(&parse_rollup(text)?.dir_path),
            "feature" => feature_okf_frontmatter(&parse_feature(text)?),
            "system" => system_okf_frontmatter(&parse_system(text)?),
            other => bail!("a spec of kind `{other}` is not one CodeOwl writes"),
        })
    };
    // The title line of a file or rollup spec, as a fresh render writes it.
    // A render only ever writes a spec for code that exists, so it always
    // links; an orphaned spec (its code moved or was deleted) has nothing to
    // link to, so `expected` is the plain title and nothing changes.
    let title = match kind {
        "file" => source_path.as_deref().map(|path| {
            let line = if graph.find(path).is_some() {
                file_title_line(path)
            } else {
                format!("# {path}")
            };
            (path.to_string(), line)
        }),
        "rollup" => {
            let dir = parse_rollup(text)?.dir_path;
            let prefix = format!("{dir}/");
            let line = if graph.files().any(|file| file.id.starts_with(&prefix)) {
                rollup_title_line(&dir)
            } else {
                format!("# {dir}")
            };
            Some((dir, line))
        }
        _ => None,
    };
    let (body, line_added) = with_missing_provenance(&lines[close + 1..].concat());
    let (body, title_linked) = match &title {
        Some((text, expected)) => with_title_link(&body, text, expected),
        None => (body, false),
    };
    let (body, links_added) = match &source_path {
        Some(path) => with_dependency_links(graph, path, &body),
        None => (body, 0),
    };
    if converted.is_none() && !line_added && !title_linked && links_added == 0 {
        return Ok(None);
    }
    let mut out = String::with_capacity(text.len());
    out.push_str(lines[0]);
    if let Some(header) = &converted {
        out.push_str(header);
    }
    for line in frontmatter
        .iter()
        .filter(|l| converted.is_none() || !l.starts_with("kind:"))
    {
        match &source_path {
            Some(path) if converted.is_some() && line.starts_with("source_paths:") => {
                out.push_str(&format!("source_paths: [{}]\n", yaml_flow_item(path)));
            }
            _ => out.push_str(line),
        }
    }
    out.push_str(lines[close]);
    out.push_str(&body);
    Ok(Some(Change {
        keys_added_to: converted.map(|_| kind.to_string()),
        line_added,
        links_added,
        title_linked,
        text: out,
    }))
}

/// Every `.md` file under `dir`, in path order. Symbolic links are not followed.
fn markdown_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            found.extend(markdown_files(&path));
        } else if file_type.is_file() && path.extension().is_some_and(|e| e == "md") {
            found.push(path);
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::feature_slug;
    use crate::fixtures::build_graph_from_sources;
    use crate::spec::feature::{FeatureSpec, feature_okf_frontmatter, render_feature};
    use crate::spec::file_task::submit;
    use crate::spec::format::{
        FileSpec, PROVENANCE, file_okf_frontmatter, file_title_line, parse, read_file_spec, render,
    };
    use crate::spec::layout::spec_path;
    use crate::spec::rollup::{RollupSpec, render_rollup, rollup_okf_frontmatter};
    use crate::spec::staleness::file_changes;
    use crate::spec::system::{SystemSpec, render_system, system_okf_frontmatter};
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
    fn a_spec_written_with_dependency_links_still_reads_as_current_and_not_smelly() {
        // The links are only in the rendered text; nothing reads them back,
        // so they must not move a hash or trip the quality check.
        let (graph, dir) = greeter_people_graph("linked-current");
        let text = generate_people(&graph, &dir);
        assert!(text.contains("](greeter.rs.md#greeter)"), "{text}");
        let spec = read_file_spec(&dir, "src/people.rs").unwrap().unwrap();
        let file_id = graph.find("src/people.rs").unwrap();
        assert!(file_changes(&graph, file_id, &spec.file).is_empty());
        assert!(crate::spec::smells::file_spec_smells(&graph, &dir, file_id, &spec).is_empty());
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

    /// One document of each kind as a fresh render writes it, with the same
    /// document as it was written before the OKF keys (`kind:` instead of
    /// the header). Returns `(path under docs/specs, fresh, legacy)`.
    fn okf_documents(dir: &Path) -> Vec<(&'static str, String, String)> {
        let graph = build_graph_from_sources(&[("lib/a.ts", "export function one() {}\n")]);
        let file_id = graph.find("lib/a.ts").unwrap();
        let file_spec = FileSpec {
            source_path: "lib/a.ts".to_string(),
            file_summary: "One thing.".to_string(),
            ..FileSpec::blank("lib/a.ts")
        };
        let rollup = RollupSpec {
            dir_path: "lib".to_string(),
            files: vec![("lib/a.ts".to_string(), "onehash".to_string())],
            spec_hash: "rolluphash".to_string(),
            body: "Small shared helpers.".to_string(),
        };
        let feature = FeatureSpec {
            slug: feature_slug("app/page.tsx"),
            entry_point: "app/page.tsx".to_string(),
            participants: vec![("lib/a.ts".to_string(), "h1".to_string())],
            spec_hash: "spechash".to_string(),
            body: "# Artwork submission\n## Summary\nLets an artist submit.".to_string(),
        };
        let system = SystemSpec {
            modules: vec![("lib".to_string(), "rolluphash".to_string())],
            features: vec![("home".to_string(), "featurehash".to_string())],
            spec_hash: "systemhash".to_string(),
            body: "# Acme\n## Summary\nA platform.".to_string(),
        };
        let file_doc = render(&graph, dir, file_id, &file_spec);
        let rollup_doc = render_rollup(&graph, dir, &rollup);
        let feature_doc = render_feature(&feature);
        let system_doc = render_system(dir, &system);
        // The same document with its OKF header put back as the old `kind:`.
        let as_legacy = |fresh: &str, header: String, kind: &str| {
            assert!(fresh.contains(&header), "{fresh}");
            fresh.replacen(&header, &format!("kind: {kind}\n"), 1)
        };
        vec![
            (
                "lib/a.ts.md",
                file_doc.clone(),
                as_legacy(&file_doc, file_okf_frontmatter("lib/a.ts"), "file"),
            ),
            (
                "lib/_index.md",
                rollup_doc.clone(),
                as_legacy(&rollup_doc, rollup_okf_frontmatter("lib"), "rollup"),
            ),
            (
                "_features/home.md",
                feature_doc.clone(),
                as_legacy(&feature_doc, feature_okf_frontmatter(&feature), "feature"),
            ),
            (
                "_index.md",
                system_doc.clone(),
                as_legacy(&system_doc, system_okf_frontmatter(&system), "system"),
            ),
        ]
    }

    /// `migrate_to_okf` with a graph of the one file (`lib/a.ts`) that
    /// `okf_documents` describes, for the tests that have no dependency list
    /// to link. The ones that link pass `greeter_people_graph`'s graph.
    fn migrate_without_graph(dir: &Path, write: bool) -> Result<OkfReport> {
        let graph = build_graph_from_sources(&[("lib/a.ts", "export function one() {}\n")]);
        migrate_to_okf(&graph, dir, write)
    }

    fn put_spec(dir: &Path, rel: &str, text: &str) -> std::path::PathBuf {
        let path = dir.join("docs").join("specs").join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn every_kind_of_legacy_document_becomes_what_a_fresh_render_writes() {
        let dir = temp_dir_for("okf-all-kinds");
        let documents = okf_documents(&dir);
        let paths: Vec<_> = documents
            .iter()
            .map(|(rel, _, legacy)| put_spec(&dir, rel, legacy))
            .collect();

        // A dry run reports and writes nothing.
        let report = migrate_without_graph(&dir, false).unwrap();
        assert_eq!(report.migrated, 4);
        assert_eq!(
            (
                report.files,
                report.rollups,
                report.features,
                report.systems
            ),
            (1, 1, 1, 1)
        );
        for (path, (_, _, legacy)) in paths.iter().zip(&documents) {
            assert_eq!(&std::fs::read_to_string(path).unwrap(), legacy);
        }

        // Writing changes only the header: each document is now exactly what
        // a fresh render of it writes.
        let report = migrate_without_graph(&dir, true).unwrap();
        assert_eq!(report.migrated, 4);
        for (path, (_, fresh, _)) in paths.iter().zip(&documents) {
            assert_eq!(&std::fs::read_to_string(path).unwrap(), fresh);
        }

        // A second run finds them all converted and changes nothing.
        let again = migrate_without_graph(&dir, true).unwrap();
        assert_eq!((again.migrated, again.unchanged), (0, 4));
        for (path, (_, fresh, _)) in paths.iter().zip(&documents) {
            assert_eq!(&std::fs::read_to_string(path).unwrap(), fresh);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn migrating_to_okf_moves_no_hash_and_no_prose() {
        let dir = temp_dir_for("okf-hash-neutral");
        let documents = okf_documents(&dir);
        let (rel, fresh, legacy) = &documents[0];
        put_spec(&dir, rel, legacy);
        let before = parse(legacy).unwrap();
        migrate_without_graph(&dir, true).unwrap();
        let after = read_file_spec(&dir, "lib/a.ts").unwrap().unwrap();
        assert_eq!(
            after, before,
            "the parsed spec, hashes and prose, is the same"
        );
        // Everything below the frontmatter is byte for byte the same.
        let body = |t: &str| t.split_once("\n---\n").unwrap().1.to_string();
        assert_eq!(body(fresh), body(legacy));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_document_without_frontmatter_or_of_an_unknown_kind_is_left_as_it_is() {
        let dir = temp_dir_for("okf-skips");
        let style = put_spec(&dir, "STYLE.md", "# Style\nPlain notes, no frontmatter.\n");
        let odd = put_spec(&dir, "lib/odd.md", "---\nkind: custom\n---\n# odd\n");
        let broken = put_spec(
            &dir,
            "lib/b.ts.md",
            "---\nkind: file\nno colon here\n---\n# b\n",
        );

        let report = migrate_without_graph(&dir, true).unwrap();
        assert_eq!(report.migrated, 0);
        assert_eq!(report.no_frontmatter, 1);
        // Each file that was left alone is named, with the reason.
        assert_eq!(report.unreadable.len(), 2, "{:?}", report.unreadable);
        assert!(
            report.unreadable[0].starts_with("lib/b.ts.md: ")
                && report.unreadable[0].contains("malformed frontmatter line"),
            "{:?}",
            report.unreadable
        );
        assert!(
            report.unreadable[1].starts_with("lib/odd.md: ")
                && report.unreadable[1].contains("custom"),
            "{:?}",
            report.unreadable
        );
        // And a style guide left inside the bundle is flagged.
        assert!(report.style_guide_in_bundle);
        assert!(
            std::fs::read_to_string(&style)
                .unwrap()
                .starts_with("# Style")
        );
        assert!(
            std::fs::read_to_string(&odd)
                .unwrap()
                .contains("kind: custom")
        );
        assert!(
            std::fs::read_to_string(&broken)
                .unwrap()
                .contains("no colon here")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_legacy_file_spec_with_a_bracketed_path_gets_a_valid_source_paths_line() {
        let dir = temp_dir_for("okf-bracket-path");
        let path = "app/api/[id]/route.ts";
        let legacy = format!(
            "---\nkind: file\nsource_paths: [{path}]\n\
             file: {{ source_hash: s, deps_hash: d, spec_hash: p }}\n---\n# {path}\n## Summary\nA route.\n"
        );
        let doc = put_spec(&dir, &format!("{path}.md"), &legacy);
        let before = parse(&legacy).unwrap();

        migrate_without_graph(&dir, true).unwrap();
        let migrated = std::fs::read_to_string(&doc).unwrap();
        assert!(
            migrated.contains("source_paths: [\"app/api/[id]/route.ts\"]\n"),
            "{migrated}"
        );
        assert!(!migrated.contains("source_paths: [app/"), "{migrated}");
        assert_eq!(parse(&migrated).unwrap(), before, "still the same spec");
        // A plain path keeps its line as it was.
        let plain = put_spec(
            &dir,
            "lib/a.ts.md",
            "---\nkind: file\nsource_paths: [lib/a.ts]\nfile: { source_hash: s, deps_hash: d, spec_hash: p }\n---\n# lib/a.ts\n## Summary\nA.\n",
        );
        migrate_without_graph(&dir, true).unwrap();
        assert!(
            std::fs::read_to_string(&plain)
                .unwrap()
                .contains("source_paths: [lib/a.ts]\n")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_style_guide_beside_the_bundle_is_not_flagged() {
        let dir = temp_dir_for("okf-style-outside");
        let style = dir.join("docs").join("STYLE.md");
        std::fs::create_dir_all(style.parent().unwrap()).unwrap();
        std::fs::write(&style, "# Style\n").unwrap();
        put_spec(
            &dir,
            "lib/a.ts.md",
            &format!("---\ntype: file\n---\n# a\n{PROVENANCE}\n"),
        );
        let report = migrate_without_graph(&dir, true).unwrap();
        assert!(!report.style_guide_in_bundle);
        assert_eq!(report.unchanged, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// `text` without the provenance line, as a spec written before it existed.
    fn without_the_line(text: &str) -> String {
        let stripped = text.replacen(&format!("{PROVENANCE}\n"), "", 1);
        assert_ne!(stripped, text, "the document had no line to remove");
        stripped
    }

    #[test]
    fn a_spec_that_already_has_the_okf_keys_still_gets_a_missing_provenance_line() {
        let dir = temp_dir_for("okf-line-only");
        let documents = okf_documents(&dir);
        let paths: Vec<_> = documents
            .iter()
            .map(|(rel, fresh, _)| put_spec(&dir, rel, &without_the_line(fresh)))
            .collect();

        let dry = migrate_without_graph(&dir, false).unwrap();
        assert_eq!((dry.migrated, dry.provenance_added), (0, 4));
        for (path, (_, fresh, _)) in paths.iter().zip(&documents) {
            assert_eq!(
                std::fs::read_to_string(path).unwrap(),
                without_the_line(fresh),
                "a dry run writes nothing"
            );
        }

        let report = migrate_without_graph(&dir, true).unwrap();
        assert_eq!(
            (report.migrated, report.provenance_added, report.unchanged),
            (0, 4, 0)
        );
        for (path, (_, fresh, _)) in paths.iter().zip(&documents) {
            assert_eq!(&std::fs::read_to_string(path).unwrap(), fresh);
        }
        let again = migrate_without_graph(&dir, true).unwrap();
        assert_eq!(
            (again.migrated, again.provenance_added, again.unchanged),
            (0, 0, 4)
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_legacy_spec_with_no_provenance_line_gets_both_in_one_pass() {
        let dir = temp_dir_for("okf-keys-and-line");
        let documents = okf_documents(&dir);
        let paths: Vec<_> = documents
            .iter()
            .map(|(rel, _, legacy)| put_spec(&dir, rel, &without_the_line(legacy)))
            .collect();

        let report = migrate_without_graph(&dir, true).unwrap();
        assert_eq!((report.migrated, report.provenance_added), (4, 4));
        for (path, (_, fresh, _)) in paths.iter().zip(&documents) {
            assert_eq!(&std::fs::read_to_string(path).unwrap(), fresh);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn adding_the_provenance_line_moves_no_hash_and_no_prose() {
        let dir = temp_dir_for("okf-line-hash-neutral");
        let documents = okf_documents(&dir);
        let (rel, fresh, _) = &documents[0];
        let without = without_the_line(fresh);
        put_spec(&dir, rel, &without);
        let before = parse(&without).unwrap();
        migrate_without_graph(&dir, true).unwrap();
        assert_eq!(read_file_spec(&dir, "lib/a.ts").unwrap().unwrap(), before);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_document_with_a_type_key_that_codeowl_did_not_write_is_left_alone() {
        // Only a spec CodeOwl wrote is given the line: it says "Generated
        // using CodeOwl", which would be false of a hand-written or foreign
        // page.
        let dir = temp_dir_for("okf-foreign-type");
        let article = "---\ntype: article\n---\n# My notes\nText.\n";
        let unattributed = "---\ntype: file\ntitle: \"x\"\n---\n# x\nText.\n";
        let a = put_spec(&dir, "notes.md", article);
        let b = put_spec(&dir, "lib/x.ts.md", unattributed);

        let report = migrate_without_graph(&dir, true).unwrap();
        assert_eq!((report.migrated, report.provenance_added), (0, 0));
        assert_eq!(report.unchanged, 2);
        assert_eq!(std::fs::read_to_string(&a).unwrap(), article);
        assert_eq!(std::fs::read_to_string(&b).unwrap(), unattributed);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_body_that_does_not_open_with_a_title_gets_no_line() {
        let dir = temp_dir_for("okf-line-no-title");
        let text = "---\ntype: system\ntitle: \"X\"\n---\n## Summary\nNo title line.\n";
        let path = put_spec(&dir, "_index.md", text);
        let report = migrate_without_graph(&dir, true).unwrap();
        assert_eq!((report.provenance_added, report.unchanged), (0, 1));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        std::fs::remove_dir_all(&dir).ok();
    }

    const LINKED_DEP: &str =
        "- [`src/greeter.rs::Greeter`](greeter.rs.md#greeter) — crate::greeter";
    const PLAIN_DEP: &str = "- `src/greeter.rs::Greeter` — crate::greeter";

    #[test]
    fn existing_dependency_lines_become_the_links_a_fresh_render_writes() {
        let (graph, dir) = greeter_people_graph("okf-links");
        let fresh = generate_people(&graph, &dir);
        assert!(fresh.contains(LINKED_DEP), "{fresh}");
        let path = spec_path(&dir, "src/people.rs");
        let legacy = fresh.replace(LINKED_DEP, PLAIN_DEP);
        std::fs::write(&path, &legacy).unwrap();
        let before = parse(&legacy).unwrap();

        // A dry run reports and writes nothing.
        let dry = migrate_to_okf(&graph, &dir, false).unwrap();
        assert_eq!(
            (
                dry.links_added,
                dry.linked_documents,
                dry.migrated,
                dry.provenance_added
            ),
            (1, 1, 0, 0)
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), legacy);

        let report = migrate_to_okf(&graph, &dir, true).unwrap();
        assert_eq!((report.links_added, report.linked_documents), (1, 1));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), fresh);

        // No hash moves, the spec still reads as current, and nothing smells.
        let spec = read_file_spec(&dir, "src/people.rs").unwrap().unwrap();
        assert_eq!(spec, before);
        let file_id = graph.find("src/people.rs").unwrap();
        assert!(file_changes(&graph, file_id, &spec.file).is_empty());
        assert!(crate::spec::smells::file_spec_smells(&graph, &dir, file_id, &spec).is_empty());

        // A second run finds nothing left to link.
        let again = migrate_to_okf(&graph, &dir, true).unwrap();
        assert_eq!((again.links_added, again.unchanged), (0, 1));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), fresh);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn only_a_dependency_with_a_spec_to_point_at_is_linked_and_only_in_the_list() {
        let (graph, dir) = greeter_people_graph("okf-links-plain");
        let header = file_okf_frontmatter("src/people.rs");
        let title = file_title_line("src/people.rs");
        let doc = format!(
            "---\n{header}source_paths: [src/people.rs]\n---\n{title}\n{PROVENANCE}\n\
             ## Summary\nS.\n\n## `Polite`\n`pub struct Polite;`\n### Summary\nA.\n\
             ### Behavior\n{PLAIN_DEP}\n\
             ### Depends on\n{PLAIN_DEP}\n\
             - `src/people.rs::Polite` — same file\n\
             - `src/gone.rs::Old` — crate::gone\n\
             externals: anyhow\n"
        );
        let path = put_spec(&dir, "src/people.rs.md", &doc);

        let report = migrate_to_okf(&graph, &dir, true).unwrap();
        assert_eq!(report.links_added, 1, "{report:?}");
        // Only the one line in the Depends on list changes: the same text in
        // the prose, a same-file line, a file that is gone, and the externals
        // line all stay as they were.
        let (before_list, list) = doc.split_once("### Depends on").unwrap();
        let expected = format!(
            "{before_list}### Depends on{}",
            list.replacen(PLAIN_DEP, LINKED_DEP, 1)
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_older_spec_gets_the_linked_title_and_links_that_reach_the_section() {
        // Specs written before the title was a link, and before a dependency
        // link named a section, are brought to what a fresh render writes.
        let (graph, dir) = greeter_people_graph("okf-title-anchor");
        let fresh = generate_people(&graph, &dir);
        assert!(
            fresh.contains("\n# [`src/people.rs`](../../../src/people.rs)\n"),
            "{fresh}"
        );
        assert!(fresh.contains("(greeter.rs.md#greeter)"), "{fresh}");
        let older = fresh
            .replace(
                "# [`src/people.rs`](../../../src/people.rs)",
                "# src/people.rs",
            )
            .replace("(greeter.rs.md#greeter)", "(greeter.rs.md)");
        let path = spec_path(&dir, "src/people.rs");
        std::fs::write(&path, &older).unwrap();
        let before = parse(&older).unwrap();

        let dry = migrate_to_okf(&graph, &dir, false).unwrap();
        assert_eq!((dry.titles_linked, dry.links_added), (1, 1));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), older);

        let report = migrate_to_okf(&graph, &dir, true).unwrap();
        assert_eq!((report.titles_linked, report.links_added), (1, 1));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), fresh);
        // Nothing but a title and a link moved: same spec, still current.
        let spec = read_file_spec(&dir, "src/people.rs").unwrap().unwrap();
        assert_eq!(spec, before);
        let file_id = graph.find("src/people.rs").unwrap();
        assert!(file_changes(&graph, file_id, &spec.file).is_empty());

        let again = migrate_to_okf(&graph, &dir, true).unwrap();
        assert_eq!(
            (again.titles_linked, again.links_added, again.unchanged),
            (0, 0, 1)
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_title_is_left_alone_when_the_graph_does_not_know_the_source_file() {
        // An orphaned spec (its code moved or was deleted) has nothing for its
        // title to link to, so a plain title is not given a link that would not
        // open. And a link already there is never removed: a graph that is
        // missing a file it should have (it failed to parse, say) must not
        // strip the links of every spec that file has.
        let (graph, dir) = greeter_people_graph("okf-title-gone");
        let header = file_okf_frontmatter("src/gone.rs");
        let doc = |title: &str| {
            format!(
                "---\n{header}source_paths: [src/gone.rs]\n---\n{title}\n{PROVENANCE}\n\
                 ## Summary\nS.\n"
            )
        };
        let path = put_spec(&dir, "src/gone.rs.md", &doc("# src/gone.rs"));
        let report = migrate_to_okf(&graph, &dir, true).unwrap();
        assert_eq!(report.titles_linked, 0);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            doc("# src/gone.rs")
        );

        // One that is already linked stays linked.
        let linked = doc(&file_title_line("src/gone.rs"));
        std::fs::write(&path, &linked).unwrap();
        let report = migrate_to_okf(&graph, &dir, true).unwrap();
        assert_eq!(report.titles_linked, 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), linked);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_existing_dependency_link_is_never_turned_back_into_plain_text() {
        // The graph here does not contain greeter.rs, as if it had failed to
        // parse. The migration may add or correct a link; it must not remove
        // one, or a partial graph would un-link every dependency on that file.
        let (full, dir) = greeter_people_graph("okf-link-kept");
        let fresh = generate_people(&full, &dir);
        assert!(fresh.contains(LINKED_DEP), "{fresh}");
        let partial = build_graph_from_sources(&[("src/people.rs", "pub struct Polite;\n")]);
        let path = spec_path(&dir, "src/people.rs");
        let report = migrate_to_okf(&partial, &dir, true).unwrap();
        assert_eq!(report.links_added, 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), fresh);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_title_someone_edited_is_left_alone_even_if_it_starts_like_ours() {
        let (graph, dir) = greeter_people_graph("okf-title-edited");
        let fresh = generate_people(&graph, &dir);
        let ours = file_title_line("src/people.rs");
        let path = spec_path(&dir, "src/people.rs");
        for edited in [
            "# [`src/people.rs`](https://example.com/design) overview",
            "# [`src/people.rs`](https://example.com/design)",
            "# [`src/people.rs`](../../../src/people.rs) (people)",
        ] {
            let text = fresh.replace(&ours, edited);
            std::fs::write(&path, &text).unwrap();
            let report = migrate_to_okf(&graph, &dir, true).unwrap();
            assert_eq!(report.titles_linked, 0, "{edited}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "{edited}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_dependency_link_with_the_wrong_anchor_is_put_right() {
        let (graph, dir) = greeter_people_graph("okf-anchor-wrong");
        let fresh = generate_people(&graph, &dir);
        let wrong = fresh.replace("(greeter.rs.md#greeter)", "(greeter.rs.md#nothing-here)");
        let path = spec_path(&dir, "src/people.rs");
        std::fs::write(&path, &wrong).unwrap();
        let report = migrate_to_okf(&graph, &dir, true).unwrap();
        assert_eq!(report.links_added, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), fresh);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_rollups_plain_title_is_linked_and_a_title_someone_wrote_is_left_alone() {
        let dir = temp_dir_for("okf-rollup-title");
        let documents = okf_documents(&dir);
        let (rel, fresh, _) = &documents[1];
        assert!(fresh.contains("\n# [`lib`](../../../lib)\n"), "{fresh}");
        let path = put_spec(
            &dir,
            rel,
            &fresh.replace("# [`lib`](../../../lib)", "# lib"),
        );
        let report = migrate_without_graph(&dir, true).unwrap();
        assert_eq!(report.titles_linked, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), *fresh);

        // A title that is neither the plain path nor ours is not touched.
        let custom = fresh.replace("# [`lib`](../../../lib)", "# Our shared helpers");
        std::fs::write(&path, &custom).unwrap();
        let report = migrate_without_graph(&dir, true).unwrap();
        assert_eq!(report.titles_linked, 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), custom);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_file_spec_with_no_source_path_is_reported_and_not_linked_from_nowhere() {
        // Links are relative to the spec's own place, so a spec that does not
        // say which file it is for cannot be linked: it is listed, untouched,
        // not given links that point nowhere.
        let (graph, dir) = greeter_people_graph("okf-no-source-path");
        let header = file_okf_frontmatter("src/people.rs");
        let title = file_title_line("src/people.rs");
        let doc = format!(
            "---\n{header}---\n{title}\n{PROVENANCE}\n## Summary\nS.\n\n## `Polite`\n\
             ### Summary\nA.\n### Behavior\nB.\n### Depends on\n{PLAIN_DEP}\n"
        );
        let path = put_spec(&dir, "src/people.rs.md", &doc);
        let report = migrate_to_okf(&graph, &dir, true).unwrap();
        assert_eq!(report.links_added, 0);
        assert_eq!(report.unreadable.len(), 1, "{:?}", report.unreadable);
        assert!(report.unreadable[0].starts_with("src/people.rs.md: "));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), doc);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn text_inside_a_fenced_block_is_never_taken_for_a_dependency_list() {
        let (graph, dir) = greeter_people_graph("okf-links-fence");
        let header = file_okf_frontmatter("src/people.rs");
        let title = file_title_line("src/people.rs");
        let doc = format!(
            "---\n{header}source_paths: [src/people.rs]\n---\n{title}\n{PROVENANCE}\n\
             ## Summary\nS.\n\n## `Polite`\n### Summary\nA.\n### Behavior\n\
             An example of the layout:\n```\n### Depends on\n{PLAIN_DEP}\n```\n\
             ### Depends on\n- (none)\n"
        );
        let path = put_spec(&dir, "src/people.rs.md", &doc);
        let report = migrate_to_okf(&graph, &dir, true).unwrap();
        assert_eq!(report.links_added, 0, "{report:?}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), doc);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn one_run_gives_an_old_spec_the_keys_the_line_and_the_links() {
        let (graph, dir) = greeter_people_graph("okf-links-all");
        let fresh = generate_people(&graph, &dir);
        let old = fresh
            .replacen(&file_okf_frontmatter("src/people.rs"), "kind: file\n", 1)
            .replacen(&format!("{PROVENANCE}\n"), "", 1)
            .replace(LINKED_DEP, PLAIN_DEP);
        assert_ne!(old, fresh);
        let path = spec_path(&dir, "src/people.rs");
        std::fs::write(&path, &old).unwrap();

        let report = migrate_to_okf(&graph, &dir, true).unwrap();
        assert_eq!(
            (report.migrated, report.provenance_added, report.links_added),
            (1, 1, 1)
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), fresh);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_repo_with_no_specs_reports_nothing() {
        let dir = temp_dir_for("okf-empty");
        let report = migrate_without_graph(&dir, true).unwrap();
        assert_eq!(report, OkfReport::default());
        std::fs::remove_dir_all(&dir).ok();
    }
}
