//! The spec document format and file writer — M4 (file specs), M5 (feature
//! specs), M6 (directory rollups), and M8's system spec. Implements
//! `ARCHITECTURE.md`'s "Spec document format": the mirrored `docs/specs/`
//! tree, per-symbol/per-file/per-directory/per-repo hash frontmatter, and
//! the granularity rules deciding which documents exist at all.
//!
//! Frontmatter here is hand-parsed rather than run through a general YAML
//! library: it's one fixed, CodeOwl-owned shape (see the `render`/`parse`
//! pair below), not arbitrary human-authored YAML, so a parser scoped
//! exactly to that shape is simpler — and easier to reason about — than a
//! full grammar for something we control both ends of.
//!
//! The token-budget recursion threshold (open question 2) is still out of
//! scope here.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::features::{
    EntryPoint, Participants, RouteLiteral, assemble_participants, enumerate_entry_points,
    feature_slug,
};
use crate::graph::{Graph, Node, SymbolId};
use crate::hash::hash_text;
use crate::symbol::{Symbol, SymbolKind};

/// Where a file's spec lives, mirrored under `docs/specs/` — never strips
/// the source extension: `lib/utils.ts` -> `docs/specs/lib/utils.ts.md`.
pub fn spec_path(root: &Path, source_path: &str) -> PathBuf {
    root.join("docs")
        .join("specs")
        .join(format!("{source_path}.md"))
}

/// A file is spec-bearing iff it declares at least one exported function or
/// class among its top-level symbols — barrel files, const-only route
/// config, and metadata-only boilerplate get no document (see
/// `ARCHITECTURE.md`'s granularity rules).
pub fn file_is_spec_bearing(graph: &Graph, file_id: SymbolId) -> bool {
    graph.children_ids(file_id).iter().any(|&id| {
        graph.get_symbol(id).is_some_and(|s| {
            s.is_exported && matches!(s.kind, SymbolKind::Function | SymbolKind::Class)
        })
    })
}

/// The top-level symbols a file spec gives their own section — both
/// exported and unexported functions/classes (the point is describing how
/// the file works, not only its public API). Top-level `const`s get no
/// subsection of their own; a class's methods are covered inside the
/// class's own section, not separately, matching how M2 already treats a
/// class as one containment unit.
fn spec_bearing_children(graph: &Graph, file_id: SymbolId) -> Vec<SymbolId> {
    graph
        .children_ids(file_id)
        .iter()
        .copied()
        .filter(|&id| {
            graph
                .get_symbol(id)
                .is_some_and(|s| matches!(s.kind, SymbolKind::Function | SymbolKind::Class))
        })
        .collect()
}

/// A directory gets a rollup (`_index.md`) iff at least two of its
/// *immediate* files are themselves spec-bearing — Next.js route trees are
/// full of single-file directories (`app/api/<route>/route.ts`), and an
/// `_index.md` for each of those would be pure noise. `dir_path` is a
/// repo-relative directory path with no trailing slash (`""` for the repo
/// root, though see `next_rollup_task`'s guard on that case).
pub fn directory_is_spec_bearing(graph: &Graph, dir_path: &str) -> bool {
    spec_bearing_files_in(graph, dir_path).len() >= 2
}

/// Every file directly in `dir_path` (repo-relative, no trailing slash) —
/// spec-bearing or not. `dir_path` compares against each file's own parent
/// path, so this is one directory level, not a recursive subtree walk;
/// nested subdirectories get their own rollup instead of being folded into
/// this one.
fn files_in(graph: &Graph, dir_path: &str) -> Vec<SymbolId> {
    let mut files: Vec<SymbolId> = graph
        .files()
        .filter(|f| {
            Path::new(&f.id)
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                == Some(dir_path.to_string())
        })
        .filter_map(|f| graph.find(&f.id))
        .collect();
    files.sort_by_key(|&id| graph.string_id(id).to_string());
    files
}

/// `dir_path`'s own spec-bearing files, in a stable (path-sorted) order —
/// unlike a file's symbols, a directory's files have no declaration order
/// to preserve.
fn spec_bearing_files_in(graph: &Graph, dir_path: &str) -> Vec<SymbolId> {
    files_in(graph, dir_path)
        .into_iter()
        .filter(|&id| file_is_spec_bearing(graph, id))
        .collect()
}

/// The short name a section heading uses — the part of a top-level
/// symbol's stable id after `file::`.
fn short_name(id: &str) -> &str {
    id.rsplit("::").next().unwrap_or(id)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HashPair {
    pub source_hash: String,
    /// The reference-edge contribution to this entry's staleness key: a
    /// hash of every resolved dependency's current `interfaceHash` (see
    /// `dependency_hash`/`file_dependency_hash`) as observed *at generation
    /// time*. Compared against a freshly recomputed value to detect the
    /// case `source_hash` alone can't: nothing in this symbol/file's own
    /// text changed, but something it imports changed shape (M7 — see
    /// `ARCHITECTURE.md`'s "Caching and invalidation").
    pub deps_hash: String,
    pub spec_hash: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SymbolProse {
    pub summary: String,
    pub behavior: String,
}

/// The full, parsed shape of a file spec — frontmatter plus body, kept
/// apart from their markdown rendering so submit/merge logic never has to
/// re-parse what it just wrote.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FileSpec {
    pub source_path: String,
    pub file: HashPair,
    /// Per top-level function/class symbol, in declaration order.
    pub symbols: Vec<(String, HashPair)>,
    /// The file's own `## Summary` prose (LLM-written).
    pub file_summary: String,
    /// Per symbol id: its `### Summary` / `### Behavior` prose.
    pub sections: Vec<(String, SymbolProse)>,
}

impl FileSpec {
    fn blank(source_path: &str) -> Self {
        Self {
            source_path: source_path.to_string(),
            ..Default::default()
        }
    }

    fn symbol_hash(&self, id: &str) -> Option<&HashPair> {
        self.symbols
            .iter()
            .find(|(sid, _)| sid == id)
            .map(|(_, h)| h)
    }

    fn section(&self, id: &str) -> Option<&SymbolProse> {
        self.sections
            .iter()
            .find(|(sid, _)| sid == id)
            .map(|(_, p)| p)
    }
}

/// Render a `FileSpec` to the markdown+frontmatter document
/// `ARCHITECTURE.md`'s "File spec shape" describes. `graph`/`file_id` are
/// needed to pull each symbol's current signature and the file's
/// dependency list — both CodeOwl-written, so they're recomputed fresh on
/// every render rather than stored in `FileSpec` itself (see the module
/// doc comment on what CodeOwl writes vs. what the LLM writes). `root` is
/// needed to read each symbol's own source text back off disk, so its
/// "Depends on" section can be scoped to what *that symbol* actually
/// references (see `dependency_lines`) rather than every import the whole
/// file happens to have.
pub fn render(graph: &Graph, root: &Path, file_id: SymbolId, spec: &FileSpec) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("kind: file\n");
    out.push_str(&format!("source_paths: [{}]\n", spec.source_path));
    out.push_str(&format!(
        "file: {{ source_hash: {}, deps_hash: {}, spec_hash: {} }}\n",
        spec.file.source_hash, spec.file.deps_hash, spec.file.spec_hash
    ));
    if !spec.symbols.is_empty() {
        out.push_str("symbols:\n");
        for (id, h) in &spec.symbols {
            out.push_str(&format!(
                "  {id}: {{ source_hash: {}, deps_hash: {}, spec_hash: {} }}\n",
                h.source_hash, h.deps_hash, h.spec_hash
            ));
        }
    }
    out.push_str("---\n");
    out.push_str(&format!("# {}\n", spec.source_path));
    out.push_str("## Summary\n");
    out.push_str(spec.file_summary.trim());
    out.push('\n');

    for (id, _) in &spec.symbols {
        let name = short_name(id);
        out.push_str(&format!("\n## `{name}`\n"));
        if let Some(sym_id) = graph.find(id)
            && let Some(sym) = graph.get_symbol(sym_id)
        {
            out.push_str(&format!("`{}`\n", sym.signature));
        }
        let prose = spec.section(id).cloned().unwrap_or_default();
        out.push_str("### Summary\n");
        out.push_str(prose.summary.trim());
        out.push('\n');
        out.push_str("### Behavior\n");
        out.push_str(prose.behavior.trim());
        out.push('\n');
        out.push_str("### Depends on\n");
        let deps = graph
            .find(id)
            .and_then(|sym_id| graph.get_symbol(sym_id))
            .map(|sym| dependency_lines(graph, root, file_id, sym))
            .unwrap_or_default();
        if deps.is_empty() {
            out.push_str("- (none)\n");
        } else {
            for dep in deps {
                out.push_str(&format!("- {dep}\n"));
            }
        }
    }
    out
}

/// `sym`'s dependency list — CodeOwl-written, never the LLM's job (see
/// `ARCHITECTURE.md`'s "What CodeOwl writes vs. what the LLM writes").
/// Scoped to `sym` specifically: M2 only resolves imports at file
/// granularity, but listing every one of the *file's* imports against
/// every symbol in it is actively misleading, not just imprecise (a
/// symbol that never touches half the file's imports would still claim
/// to depend on them). Narrowed with a whole-word text search over the
/// symbol's own source span instead — a heuristic, not semantic analysis,
/// but far closer to the truth than file-wide attribution, and needs no
/// new resolution machinery.
fn dependency_lines(graph: &Graph, root: &Path, file_id: SymbolId, sym: &Symbol) -> Vec<String> {
    let Node::File(file) = graph.get(file_id) else {
        return Vec::new();
    };
    let Ok(symbol_text) = read_symbol_text(root, &file.id, sym.lines) else {
        return Vec::new();
    };
    graph
        .imports()
        .iter()
        .filter(|imp| {
            imp.from_file == file.id && contains_identifier(&symbol_text, &imp.imported_name)
        })
        .map(|imp| match imp.target {
            Some(target) => format!("`{}` — {}", graph.string_id(target), imp.specifier),
            None => format!(
                "{} — {} (external or unresolved)",
                imp.imported_name, imp.specifier
            ),
        })
        .collect()
}

/// Slice `rel_path`'s raw text down to `lines` (1-indexed, inclusive) —
/// the same scheme `mcp.rs`'s `read_lines` uses for a symbol task's own
/// source.
fn read_symbol_text(root: &Path, rel_path: &str, lines: [usize; 2]) -> Result<String> {
    let content = std::fs::read_to_string(root.join(rel_path))
        .with_context(|| format!("reading {rel_path}"))?;
    let [start, end] = lines;
    Ok(content
        .lines()
        .skip(start.saturating_sub(1))
        .take(end + 1 - start)
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Whole-word substring search: `name` must not be immediately preceded
/// or followed by another identifier character, so `useLabel` doesn't
/// count as a use of the import `Label`.
fn contains_identifier(text: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let is_ident_char = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let bytes = text.as_bytes();
    let mut start = 0;
    while let Some(pos) = text[start..].find(name) {
        let idx = start + pos;
        let before_ok = idx == 0 || !is_ident_char(bytes[idx - 1]);
        let after = idx + name.len();
        let after_ok = after >= bytes.len() || !is_ident_char(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
        start = idx + 1;
    }
    false
}

/// A reference target's contribution to a consumer's staleness key: its
/// `interfaceHash` if it has one (an exported symbol), falling back to a
/// file's own `source_hash` if the target is a whole file rather than a
/// symbol — never a spec/summary's text (see "Caching and invalidation":
/// rewording a dependency's prose must never invalidate its consumers).
fn interface_or_source_hash(graph: &Graph, id: SymbolId) -> String {
    graph
        .get_symbol(id)
        .map(|s| {
            s.interface_hash
                .clone()
                .unwrap_or_else(|| s.source_hash.clone())
        })
        .or_else(|| graph.get_file(id).map(|f| f.source_hash.clone()))
        .unwrap_or_default()
}

/// Hash a set of reference-edge targets into one deps-hash value — order-
/// independent (sorted first) since import declaration order isn't a real
/// dependency, and deduplicated since two different local names can
/// resolve to the same target.
fn hash_dependency_targets(graph: &Graph, targets: impl Iterator<Item = SymbolId>) -> String {
    let mut pairs: Vec<(String, String)> = targets
        .map(|id| {
            (
                graph.string_id(id).to_string(),
                interface_or_source_hash(graph, id),
            )
        })
        .collect();
    pairs.sort();
    pairs.dedup();
    hash_text(
        &pairs
            .iter()
            .map(|(id, h)| format!("{id}:{h}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// `sym`'s reference-edge staleness contribution — the same per-symbol
/// scoping `dependency_lines` uses for the human-readable "Depends on"
/// list, but hashing only the *resolved* targets' current interface
/// hashes (an external/unresolved import has no interface CodeOwl can
/// observe, so it can't contribute to staleness). Persisted as
/// `HashPair.deps_hash` at generation time, recomputed here again at
/// every currency check to detect drift.
fn dependency_hash(graph: &Graph, root: &Path, file_id: SymbolId, sym: &Symbol) -> String {
    let Node::File(file) = graph.get(file_id) else {
        return String::new();
    };
    let Ok(symbol_text) = read_symbol_text(root, &file.id, sym.lines) else {
        return String::new();
    };
    let targets = graph
        .imports()
        .iter()
        .filter(|imp| {
            imp.from_file == file.id && contains_identifier(&symbol_text, &imp.imported_name)
        })
        .filter_map(|imp| imp.target);
    hash_dependency_targets(graph, targets)
}

/// A file's own reference-edge staleness contribution — every one of its
/// resolved imports, regardless of which specific symbol in the file uses
/// each one (a file has no per-symbol text scoping the way a `### Depends
/// on` section does; `dependency_hash` above is the narrower, per-symbol
/// version of this same idea).
fn file_dependency_hash(graph: &Graph, file_id: SymbolId) -> String {
    let Node::File(file) = graph.get(file_id) else {
        return String::new();
    };
    let targets = graph
        .imports()
        .iter()
        .filter(|imp| imp.from_file == file.id)
        .filter_map(|imp| imp.target);
    hash_dependency_targets(graph, targets)
}

/// Diff two `(id, hash)` lists — shared between a feature's `participants`
/// map and a rollup's `files` map, since the staleness semantics are
/// identical for both: an existing entry whose hash moved, a new entry
/// that appeared, or an old entry that's gone are all real changes (the
/// "or the participant set itself changes" half of "Caching and
/// invalidation", applied uniformly). Empty means current; each entry in
/// the result names one thing that moved, deterministically, off the
/// graph — no LLM needed to say *that* something changed (see
/// `ARCHITECTURE.md`'s "Ordering").
pub fn diff_hash_lists(current: &[(String, String)], stored: &[(String, String)]) -> Vec<String> {
    let mut changed = Vec::new();
    for (id, hash) in current {
        match stored.iter().find(|(sid, _)| sid == id) {
            Some((_, h)) if h == hash => {}
            Some(_) => changed.push(format!("changed:{id}")),
            None => changed.push(format!("added:{id}")),
        }
    }
    for (id, _) in stored {
        if !current.iter().any(|(cid, _)| cid == id) {
            changed.push(format!("removed:{id}"));
        }
    }
    changed.sort();
    changed
}

/// What's changed for `sym` since `stored` was recorded — empty means
/// current. Expressed as a `diff_hash_lists` comparison over exactly two
/// fixed keys (`"source"`, `"dependencies"`) so a caller (`get_spec`) can
/// report *which* of the two moved, not just that something did.
pub fn symbol_changes(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    sym: &Symbol,
    stored: &HashPair,
) -> Vec<String> {
    diff_hash_lists(
        &[
            ("source".to_string(), sym.source_hash.clone()),
            (
                "dependencies".to_string(),
                dependency_hash(graph, root, file_id, sym),
            ),
        ],
        &[
            ("source".to_string(), stored.source_hash.clone()),
            ("dependencies".to_string(), stored.deps_hash.clone()),
        ],
    )
}

/// The file-level equivalent of `symbol_changes` — what's changed about
/// `file_id`'s own spec since `stored` was recorded.
pub fn file_changes(graph: &Graph, file_id: SymbolId, stored: &HashPair) -> Vec<String> {
    let source_hash = graph
        .get_file(file_id)
        .map(|f| f.source_hash.clone())
        .unwrap_or_default();
    diff_hash_lists(
        &[
            ("source".to_string(), source_hash),
            (
                "dependencies".to_string(),
                file_dependency_hash(graph, file_id),
            ),
        ],
        &[
            ("source".to_string(), stored.source_hash.clone()),
            ("dependencies".to_string(), stored.deps_hash.clone()),
        ],
    )
}

/// Parse a spec file's own rendered output back into a `FileSpec`. Only
/// needs to tolerate what `render` produces — human-edit tolerance
/// (arbitrary reordering, reformatting) is M8's "Human corrections"
/// mechanics, not in scope yet.
pub fn parse(content: &str) -> Result<FileSpec> {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        bail!("spec file missing frontmatter opening `---`");
    }

    let mut source_path = String::new();
    let mut file = HashPair::default();
    let mut symbols = Vec::new();
    let mut in_symbols_block = false;

    let mut consumed = 1; // the opening "---"
    for line in lines.by_ref() {
        consumed += 1;
        if line.trim() == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("  ")
            && in_symbols_block
        {
            // Split on the flow map's opening `{`, not the first `:` — a
            // symbol id itself contains `::` (`file.ts::name`), so
            // `split_once(':')` would cut the id in half.
            let brace = rest
                .find('{')
                .context("malformed symbols entry in spec frontmatter")?;
            let id = rest[..brace].trim().trim_end_matches(':').trim();
            symbols.push((id.to_string(), parse_hash_pair(&rest[brace..])));
            continue;
        }
        in_symbols_block = line.trim() == "symbols:";
        if in_symbols_block {
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .context("malformed frontmatter line in spec file")?;
        match key.trim() {
            "source_paths" => {
                source_path = value
                    .trim()
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .trim()
                    .to_string();
            }
            "file" => file = parse_hash_pair(value),
            _ => {}
        }
    }

    let body: String = content
        .lines()
        .skip(consumed)
        .collect::<Vec<_>>()
        .join("\n");
    let file_summary = extract_section(&body, "## Summary", "\n## ").unwrap_or_default();

    let mut sections = Vec::new();
    for (id, _) in &symbols {
        let heading = format!("## `{}`\n", short_name(id));
        let Some(start) = body.find(&heading) else {
            continue;
        };
        let rest = &body[start..];
        let end = rest[heading.len()..]
            .find("\n## ")
            .map(|p| p + heading.len())
            .unwrap_or(rest.len());
        let block = &rest[..end];
        sections.push((
            id.clone(),
            SymbolProse {
                summary: extract_section(block, "### Summary", "\n### ").unwrap_or_default(),
                behavior: extract_section(block, "### Behavior", "\n### ").unwrap_or_default(),
            },
        ));
    }

    Ok(FileSpec {
        source_path,
        file,
        symbols,
        file_summary,
        sections,
    })
}

fn parse_hash_pair(flow_map: &str) -> HashPair {
    let inner = flow_map
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}');
    let mut pair = HashPair::default();
    for kv in inner.split(',') {
        if let Some((k, v)) = kv.split_once(':') {
            match k.trim() {
                "source_hash" => pair.source_hash = v.trim().to_string(),
                "deps_hash" => pair.deps_hash = v.trim().to_string(),
                "spec_hash" => pair.spec_hash = v.trim().to_string(),
                _ => {}
            }
        }
    }
    pair
}

/// Find `heading` in `text`, return the trimmed text between it and the
/// next `next_marker` (or end of `text`).
fn extract_section(text: &str, heading: &str, next_marker: &str) -> Option<String> {
    let idx = text.find(heading)?;
    let after = &text[idx + heading.len()..];
    let after = after.strip_prefix('\n').unwrap_or(after);
    let end = after.find(next_marker).unwrap_or(after.len());
    Some(after[..end].trim().to_string())
}

/// Read back a file's persisted spec, if one has ever been generated —
/// `mcp.rs`'s `get_spec` uses this directly (a pure read, never triggers
/// generation); `next_task`/`submit` use it internally too.
pub fn read_file_spec(root: &Path, source_path: &str) -> Result<Option<FileSpec>> {
    read_existing(root, source_path)
}

fn read_existing(root: &Path, source_path: &str) -> Result<Option<FileSpec>> {
    let path = spec_path(root, source_path);
    if !path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    parse(&content).map(Some)
}

fn write(root: &Path, graph: &Graph, file_id: SymbolId, spec: &FileSpec) -> Result<()> {
    let path = spec_path(root, &spec.source_path);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(&path, render(graph, root, file_id, spec))
        .with_context(|| format!("writing {}", path.display()))
}

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
        lines: [usize; 2],
        /// `Some` only for a reconciliation regeneration (M8's "Human
        /// corrections" case 4: source moved *and* a human had edited this
        /// symbol's prose) — the human's prior text, to preserve whatever
        /// correction is still accurate rather than silently discard it.
        /// `None` for a first-ever generation or a plain case-2
        /// regeneration (no human edit on record).
        prior: Option<SymbolProse>,
    },
    File {
        id: String,
        /// Same case-4 meaning as `Symbol::prior`, for the file's own
        /// `## Summary`.
        prior: Option<String>,
    },
}

/// Whether `spec`'s currently-parsed prose for `id` no longer matches the
/// `spec_hash` last recorded for it — i.e. a human edited the `.md` file
/// directly (never through `submit_spec`) since it was last machine-
/// written. `spec_hash` is defined as `hash_text` of exactly this prose at
/// generation time (see `submit`), so any drift between the two can only
/// come from an edit that didn't also fix up the hash — not something a
/// human would do by hand. See `ARCHITECTURE.md`'s "Human corrections".
fn symbol_prose_is_human_edited(hash: &HashPair, prose: &SymbolProse) -> bool {
    hash_text(&format!("{}\n{}", prose.summary, prose.behavior)) != hash.spec_hash
}

fn file_prose_is_human_edited(hash: &HashPair, file_summary: &str) -> bool {
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
                lines: sym.lines,
                prior: None,
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
                        lines: sym.lines,
                        prior: None,
                    }));
                }
                continue;
            }
            (true, false) => {
                // case 2: plain regeneration, nothing to preserve
                return Ok(Some(SpecTask::Symbol {
                    id: sym.id.clone(),
                    signature: sym.signature.clone(),
                    docstring: sym.docstring.clone(),
                    lines: sym.lines,
                    prior: None,
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
                    lines: sym.lines,
                    prior: Some(prose),
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
                    }));
                }
                return Ok(None);
            }
            (true, false) => {
                return Ok(Some(SpecTask::File {
                    id: file.id.clone(),
                    prior: None,
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
                }));
            }
        }
    }

    Ok(Some(SpecTask::File {
        id: file.id.clone(),
        prior: None,
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
            let hash = HashPair {
                source_hash: file.source_hash.clone(),
                deps_hash: file_dependency_hash(graph, file_id),
                spec_hash: hash_text(&summary),
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
            let file = graph
                .get_file(file_id)
                .context("symbol's parent is not a file")?;
            let summary = extract_section(content, "### Summary", "\n### ")
                .filter(|s| !s.is_empty())
                .context("submitted content missing a non-empty `### Summary` section")?;
            let behavior = extract_section(content, "### Behavior", "\n### ")
                .filter(|s| !s.is_empty())
                .context("submitted content missing a non-empty `### Behavior` section")?;

            let mut spec =
                read_existing(root, &file.id)?.unwrap_or_else(|| FileSpec::blank(&file.id));
            let hash = HashPair {
                source_hash: sym.source_hash.clone(),
                deps_hash: dependency_hash(graph, root, file_id, sym),
                spec_hash: hash_text(&format!("{summary}\n{behavior}")),
            };
            upsert(&mut spec.symbols, sym.id.clone(), hash.clone());
            upsert_section(
                &mut spec.sections,
                sym.id.clone(),
                SymbolProse { summary, behavior },
            );
            reorder_symbols(graph, file_id, &mut spec);
            write(root, graph, file_id, &spec)?;
            Ok(hash)
        }
    }
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
    out.push_str(spec.body.trim());
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

    let body = content
        .lines()
        .skip(consumed)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
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
}

/// The feature task for `entry`, or `None` if its spec is already current
/// (every participant's hash matches, and the participant set itself
/// hasn't changed -- a new `fetch()` literal appearing is a real change
/// even though no existing participant moved).
pub fn next_feature_task(
    graph: &Graph,
    root: &Path,
    entry: &EntryPoint,
    route_literals: &[RouteLiteral],
) -> Result<Option<FeatureTask>> {
    let participants = assemble_participants(graph, route_literals, &entry.file);
    let current = current_participant_hashes(graph, &participants)?;

    let existing = read_feature_spec(root, &entry.slug)?;
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

    Ok(Some(FeatureTask {
        slug: entry.slug.clone(),
        entry_point: entry.file.clone(),
        core_sources,
        dependencies,
    }))
}

/// Persist `content` (the agent's LLM-written feature narrative, title
/// included) as `entry_file`'s feature spec.
pub fn submit_feature(
    graph: &Graph,
    root: &Path,
    route_literals: &[RouteLiteral],
    entry_file: &str,
    content: &str,
) -> Result<FeatureSpec> {
    let body = content.trim().to_string();
    if body.is_empty() {
        bail!("submitted feature content is empty");
    }
    if !body.starts_with("# ") {
        bail!("submitted feature content must start with a `# Title` heading");
    }

    let slug = feature_slug(entry_file);
    let participants = assemble_participants(graph, route_literals, entry_file);
    let hashes = current_participant_hashes(graph, &participants)?;

    let spec = FeatureSpec {
        slug: slug.clone(),
        entry_point: entry_file.to_string(),
        participants: hashes,
        spec_hash: hash_text(&body),
        body,
    };

    let path = feature_spec_path(root, &slug);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(&path, render_feature(&spec))
        .with_context(|| format!("writing {}", path.display()))?;

    Ok(spec)
}

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

/// Render a `RollupSpec` to markdown+frontmatter. `root` is needed to pull
/// each file's own current summary fresh for the `## Contents` list, the
/// same "recompute, don't store" pattern `render`'s dependency lines use.
pub fn render_rollup(graph: &Graph, root: &Path, spec: &RollupSpec) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("kind: rollup\n");
    out.push_str(&format!("dir: {}\n", spec.dir_path));
    out.push_str("files:\n");
    for (path, hash) in &spec.files {
        out.push_str(&format!("  {path}: {hash}\n"));
    }
    out.push_str(&format!("spec_hash: {}\n", spec.spec_hash));
    out.push_str("---\n");
    out.push_str(&format!("# {}\n", spec.dir_path));
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
            .filter(|s| s.file.source_hash == file.source_hash)
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
    std::fs::write(&path, render_rollup(graph, root, &spec))
        .with_context(|| format!("writing {}", path.display()))?;

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
    dirs.retain(|d| !d.is_empty() && directory_is_spec_bearing(graph, d));
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

/// Each currently-enumerated feature entry point's own current
/// `spec_hash` (empty if that feature isn't itself current yet) — the
/// system spec's per-feature half of its staleness key.
pub fn current_feature_hashes(
    graph: &Graph,
    root: &Path,
    route_literals: &[RouteLiteral],
) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for entry in enumerate_entry_points(graph, route_literals) {
        let participants = assemble_participants(graph, route_literals, &entry.file);
        let current = current_participant_hashes(graph, &participants)?;
        let hash = read_feature_spec(root, &entry.slug)?
            .filter(|s| diff_hash_lists(&current, &s.participants).is_empty())
            .map(|s| s.spec_hash)
            .unwrap_or_default();
        out.push((entry.slug, hash));
    }
    Ok(out)
}

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
pub fn next_system_task(
    graph: &Graph,
    root: &Path,
    route_literals: &[RouteLiteral],
) -> Result<Option<SystemTask>> {
    let current_modules = current_module_hashes(graph, root)?;
    let current_features = current_feature_hashes(graph, root, route_literals)?;
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
pub fn submit_system(
    graph: &Graph,
    root: &Path,
    route_literals: &[RouteLiteral],
    content: &str,
) -> Result<SystemSpec> {
    let body = content.trim().to_string();
    if body.is_empty() {
        bail!("submitted system content is empty");
    }
    if !body.starts_with("# ") {
        bail!("submitted system content must start with a `# Title` heading");
    }

    let spec = SystemSpec {
        modules: current_module_hashes(graph, root)?,
        features: current_feature_hashes(graph, root, route_literals)?,
        spec_hash: hash_text(&body),
        body,
    };

    let path = system_spec_path(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(&path, render_system(root, &spec))
        .with_context(|| format!("writing {}", path.display()))?;

    Ok(spec)
}

/// One document the granularity rules say should exist, and where it
/// currently stands (M8) — the unit `get_spec_coverage` reports against.
/// `id` is exactly the id `get_next_spec_task`/`get_spec` expect for that
/// document: a repo-relative path for a file, `"rollup:<dir>"`,
/// `"feature:<slug>"`, or the fixed `"system"`.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageItem {
    pub id: String,
    pub kind: String,
    pub status: String,
    /// Import fan-in — how many other files import something from this
    /// one. Always 0 for non-file kinds; this is what "high-fan-in files"
    /// in `ARCHITECTURE.md`'s "Generation priority" is computed from.
    pub fan_in: usize,
    /// Deterministic quality smells found in this document's own current
    /// content — never empty just because `status` is `"missing"`
    /// (nothing to smell-check) but can be non-empty even when `status`
    /// is `"current"`: hash-based staleness only verifies a spec's
    /// *inputs* haven't moved, never that its prose was ever meaningful.
    /// See `prose_smells`/`ARCHITECTURE.md`'s "Quality smells".
    pub smells: Vec<String>,
}

/// A deterministic, non-LLM check for prose that looks like a stub or a
/// cop-out rather than real content — independent of hash-based
/// staleness. Catches the two failure modes `CLAUDE.md`/the generate
/// prompt already call out by name: a "see the source" cop-out that
/// defeats the whole point of the document, and prose so short it can
/// only be a template placeholder (found, concretely, in two real
/// pre-M5 specs during a quality audit: `"<name> does its job."` /
/// `"See source."`). Deliberately not exhaustive or clever — a small,
/// named denylist plus a word-count floor, both easy to reason about and
/// cheap to extend if a new failure mode turns up.
pub fn prose_smells(text: &str) -> Vec<String> {
    const COP_OUT_PHRASES: &[&str] = &[
        "see the source",
        "see source",
        "see the route handler",
        "see the handler",
        "see the file for details",
        "see the code for details",
        "refer to the source",
        "check the implementation",
        "check the source",
    ];
    let mut smells = Vec::new();
    let lower = text.to_lowercase();
    if COP_OUT_PHRASES.iter().any(|phrase| lower.contains(phrase)) {
        smells.push("cop_out_phrase".to_string());
    }
    if text.split_whitespace().count() < 4 {
        smells.push("suspiciously_short".to_string());
    }
    smells
}

/// The union of every quality smell anywhere in a file spec — the file's
/// own summary, every symbol's summary/behavior, and a whole-file check
/// for the pre-M5 file-wide-dependency-attribution bug's signature (every
/// symbol sharing one identical, non-empty dependency list). That bug is
/// structurally impossible in a freshly-generated spec after M5's
/// per-symbol scoping fix, but this stays as a detector for specs that
/// predate it and were never regenerated since. This is the coarse,
/// whole-document signal `get_spec_coverage` needs ("does this document
/// need another look at all"); `get_spec` on one specific symbol id
/// checks `prose_smells` directly against just that symbol's own prose
/// instead, for a narrower answer.
pub fn file_spec_smells(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    spec: &FileSpec,
) -> Vec<String> {
    let mut smells = prose_smells(&spec.file_summary);
    for (_, prose) in &spec.sections {
        smells.extend(prose_smells(&prose.summary));
        smells.extend(prose_smells(&prose.behavior));
    }
    if spec.symbols.len() >= 2 {
        let dep_lists: Vec<Vec<String>> = spec
            .symbols
            .iter()
            .filter_map(|(id, _)| {
                let sym_id = graph.find(id)?;
                let sym = graph.get_symbol(sym_id)?;
                Some(dependency_lines(graph, root, file_id, sym))
            })
            .collect();
        if dep_lists.len() == spec.symbols.len()
            && !dep_lists[0].is_empty()
            && dep_lists.iter().all(|d| *d == dep_lists[0])
        {
            smells.push("identical_dependencies_across_symbols".to_string());
        }
    }
    smells.sort();
    smells.dedup();
    smells
}

/// The same coarse signal as `file_spec_smells`, for a feature/rollup/
/// system spec's single-blob body — checked against just the LLM-written
/// `## Summary` section (a `RollupSpec`/`SystemSpec`'s `body` already
/// *is* just that; a `FeatureSpec`'s `body` is the whole document, title
/// included, so this extracts the `## Summary` section out of it first).
pub fn body_smells(body: &str) -> Vec<String> {
    let summary = extract_section(body, "## Summary", "\n## ").unwrap_or_else(|| body.to_string());
    prose_smells(&summary)
}

fn file_fan_in(graph: &Graph, file_id: SymbolId) -> usize {
    graph
        .imports()
        .iter()
        .filter(|imp| {
            imp.target
                .is_some_and(|t| graph.parent_id(t) == Some(file_id))
        })
        .count()
}

/// A file document's status: "missing" if nothing at all has ever been
/// generated for it (neither any symbol nor its own summary — the same
/// `HashPair`-is-empty/`symbols`-is-empty signal `next_task` itself uses
/// to distinguish a first-ever generation from a reconciliation), else
/// "current" iff every symbol's and the file's own hashes still match,
/// else "stale". Deliberately *not* implemented via `next_task` (which
/// also treats a quality smell as a reason to offer a task, so the
/// generate loop can act on one) — `status` here stays purely hash-based,
/// so it never contradicts `smells` being reported as a *separate* signal
/// on a document that's otherwise fully current. See "Quality smells" in
/// `ARCHITECTURE.md`.
fn file_status(graph: &Graph, root: &Path, file_id: SymbolId) -> Result<(String, Vec<String>)> {
    let file = graph.get_file(file_id).context("not a file id")?;
    let Some(spec) = read_file_spec(root, &file.id)? else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let has_any_content = !spec.file.spec_hash.is_empty() || !spec.symbols.is_empty();
    if !has_any_content {
        return Ok(("missing".to_string(), Vec::new()));
    }

    let mut hash_current = true;
    for sym_id in spec_bearing_children(graph, file_id) {
        let sym = graph.get_symbol(sym_id).expect("filtered to symbol ids");
        let up_to_date = spec
            .symbol_hash(&sym.id)
            .is_some_and(|hash| symbol_changes(graph, root, file_id, sym, hash).is_empty());
        if !up_to_date {
            hash_current = false;
            break;
        }
    }
    if hash_current {
        hash_current = file_changes(graph, file_id, &spec.file).is_empty();
    }
    let status = if hash_current { "current" } else { "stale" }.to_string();
    Ok((status, file_spec_smells(graph, root, file_id, &spec)))
}

fn rollup_status(graph: &Graph, root: &Path, dir: &str) -> Result<(String, Vec<String>)> {
    let Some(spec) = read_rollup_spec(root, dir)? else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let current = current_file_hashes(graph, root, dir)?;
    let status = if diff_hash_lists(&current, &spec.files).is_empty() {
        "current"
    } else {
        "stale"
    }
    .to_string();
    Ok((status, body_smells(&spec.body)))
}

fn feature_status(
    graph: &Graph,
    root: &Path,
    route_literals: &[RouteLiteral],
    entry: &EntryPoint,
) -> Result<(String, Vec<String>)> {
    let Some(spec) = read_feature_spec(root, &entry.slug)? else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let participants = assemble_participants(graph, route_literals, &entry.file);
    let current = current_participant_hashes(graph, &participants)?;
    let status = if diff_hash_lists(&current, &spec.participants).is_empty() {
        "current"
    } else {
        "stale"
    }
    .to_string();
    Ok((status, body_smells(&spec.body)))
}

fn system_status(
    graph: &Graph,
    root: &Path,
    route_literals: &[RouteLiteral],
) -> Result<(String, Vec<String>)> {
    let Some(spec) = read_system_spec(root)? else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let smells = body_smells(&spec.body);
    let mut current_all = current_module_hashes(graph, root)?;
    current_all.extend(current_feature_hashes(graph, root, route_literals)?);
    let mut stored_all = spec.modules;
    stored_all.extend(spec.features);
    let status = if diff_hash_lists(&current_all, &stored_all).is_empty() {
        "current"
    } else {
        "stale"
    }
    .to_string();
    Ok((status, smells))
}

/// Whether `path` (a file or directory) falls under `scope` — an exact
/// match, or a true subdirectory (`scope` plus a `/` boundary), never a
/// bare string-prefix match: `"lib/email"` must not also catch
/// `"lib/email.ts"` or `"lib/email-utils/"`. An empty `scope` matches
/// everything (files/rollups only — see `coverage`'s own doc comment on
/// why an empty-but-`Some` scope still excludes features/system).
fn within_scope(path: &str, scope: &str) -> bool {
    scope.is_empty() || path == scope || path.starts_with(&format!("{scope}/"))
}

/// Every document the granularity rules say should exist, each with its
/// current status — the whole-repo inventory `get_spec_coverage` reports
/// (M8), optionally narrowed to files/rollups under `scope` (a directory
/// prefix — see `within_scope`). Features and the system spec are
/// unaffected by `scope` — both are repo-wide concepts, and `scope`
/// itself excludes the system spec entirely (a system spec scoped to one
/// directory isn't a coherent thing to ask for).
pub fn coverage(
    graph: &Graph,
    root: &Path,
    route_literals: &[RouteLiteral],
    scope: Option<&str>,
) -> Result<Vec<CoverageItem>> {
    let mut items = Vec::new();

    let mut file_ids: Vec<SymbolId> = graph.files().filter_map(|f| graph.find(&f.id)).collect();
    file_ids.sort_by_key(|&id| graph.string_id(id).to_string());
    for id in file_ids {
        if !file_is_spec_bearing(graph, id) {
            continue;
        }
        let path = graph.string_id(id).to_string();
        if scope.is_some_and(|s| !within_scope(&path, s)) {
            continue;
        }
        let (status, smells) = file_status(graph, root, id)?;
        items.push(CoverageItem {
            status,
            fan_in: file_fan_in(graph, id),
            smells,
            id: path,
            kind: "file".to_string(),
        });
    }

    for dir in enumerate_modules(graph) {
        if scope.is_some_and(|s| !within_scope(&dir, s)) {
            continue;
        }
        let (status, smells) = rollup_status(graph, root, &dir)?;
        items.push(CoverageItem {
            status,
            fan_in: 0,
            smells,
            id: format!("rollup:{dir}"),
            kind: "rollup".to_string(),
        });
    }

    if scope.is_none() {
        for entry in enumerate_entry_points(graph, route_literals) {
            let (status, smells) = feature_status(graph, root, route_literals, &entry)?;
            items.push(CoverageItem {
                status,
                fan_in: 0,
                smells,
                id: format!("feature:{}", entry.slug),
                kind: "feature".to_string(),
            });
        }
        let (status, smells) = system_status(graph, root, route_literals)?;
        items.push(CoverageItem {
            status,
            fan_in: 0,
            smells,
            id: "system".to_string(),
            kind: "system".to_string(),
        });
    }

    Ok(items)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoverageSummary {
    pub current: usize,
    pub stale: usize,
    pub missing: usize,
    /// Count of items with at least one quality smell, of *any* status —
    /// deliberately not exclusive with `current`: a document can be
    /// hash-current and still carry a smell, which is exactly the case
    /// this field exists to surface (see `CoverageItem.smells`).
    pub smelly: usize,
}

pub fn summarize(items: &[CoverageItem]) -> CoverageSummary {
    let mut summary = CoverageSummary::default();
    for item in items {
        match item.status.as_str() {
            "current" => summary.current += 1,
            "stale" => summary.stale += 1,
            "missing" => summary.missing += 1,
            _ => {}
        }
        if !item.smells.is_empty() {
            summary.smelly += 1;
        }
    }
    summary
}

/// `coverage`'s items that still need attention, ordered the way
/// `/codeowl generate --all`/`--budget=N` should spend a limited budget:
/// the system spec first, then feature specs, then files by descending
/// import fan-in, then everything else (rollups) — see
/// `ARCHITECTURE.md`'s "Generation priority". Within a tier, an honestly
/// `"missing"` or `"stale"` document outranks a merely smelly-but-
/// `"current"` one; ties beyond that break on `id` for a stable,
/// reproducible order.
///
/// Includes a `"current"` item when it has a quality smell — hash-based
/// staleness only verifies a spec's *inputs* haven't moved, never that
/// its prose was ever meaningful (a "see the source" stub, once written,
/// stays hash-`"current"` forever unless something else flags it). Never
/// filtering on smells too would mean a `--all --budget=N` run could run
/// to completion while silently leaving known-bad content in place.
pub fn prioritize(items: Vec<CoverageItem>) -> Vec<CoverageItem> {
    let mut pending: Vec<CoverageItem> = items
        .into_iter()
        .filter(|i| i.status != "current" || !i.smells.is_empty())
        .collect();
    pending.sort_by(|a, b| {
        fn tier(kind: &str) -> u8 {
            match kind {
                "system" => 0,
                "feature" => 1,
                "file" => 2,
                _ => 3,
            }
        }
        fn urgency(status: &str) -> u8 {
            match status {
                "missing" => 0,
                "stale" => 1,
                _ => 2, // "current" but smelly -- the only other way in
            }
        }
        tier(&a.kind)
            .cmp(&tier(&b.kind))
            .then(urgency(&a.status).cmp(&urgency(&b.status)))
            .then(b.fan_in.cmp(&a.fan_in))
            .then(a.id.cmp(&b.id))
    });
    pending
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_graph_from_sources;

    #[test]
    fn barrel_file_is_not_spec_bearing() {
        let graph = build_graph_from_sources(&[(
            "a.ts",
            "export { Foo } from './foo';\nexport * from './bar';\n",
        )]);
        let file_id = graph.find("a.ts").unwrap();
        assert!(!file_is_spec_bearing(&graph, file_id));
    }

    #[test]
    fn file_with_exported_function_is_spec_bearing() {
        let graph = build_graph_from_sources(&[("a.ts", "export function double(x: number) {}\n")]);
        let file_id = graph.find("a.ts").unwrap();
        assert!(file_is_spec_bearing(&graph, file_id));
    }

    #[test]
    fn non_exported_only_file_is_not_spec_bearing() {
        let graph = build_graph_from_sources(&[("a.ts", "const helper = 1;\n")]);
        let file_id = graph.find("a.ts").unwrap();
        assert!(!file_is_spec_bearing(&graph, file_id));
    }

    #[test]
    fn single_file_directory_is_not_spec_bearing() {
        let graph = build_graph_from_sources(&[(
            "app/api/submit/route.ts",
            "export function GET(): void {}\n",
        )]);
        assert!(!directory_is_spec_bearing(&graph, "app/api/submit"));
    }

    #[test]
    fn directory_with_two_spec_bearing_files_is_spec_bearing() {
        let graph = build_graph_from_sources(&[
            ("lib/one.ts", "export function one(): void {}\n"),
            ("lib/two.ts", "export function two(): void {}\n"),
        ]);
        assert!(directory_is_spec_bearing(&graph, "lib"));
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
    fn directory_with_one_spec_bearing_and_one_barrel_file_is_not_spec_bearing() {
        let graph = build_graph_from_sources(&[
            ("lib/one.ts", "export function one(): void {}\n"),
            ("lib/index.ts", "export { one } from './one';\n"),
        ]);
        assert!(!directory_is_spec_bearing(&graph, "lib"));
    }

    #[test]
    fn render_then_parse_round_trips() {
        let graph = build_graph_from_sources(&[(
            "a.ts",
            "/** Doubles a number. */\nexport function double(x: number): number { return x * 2; }\n",
        )]);
        let file_id = graph.find("a.ts").unwrap();
        let sym = graph
            .get_symbol(graph.find("a.ts::double").unwrap())
            .unwrap();

        let spec = FileSpec {
            source_path: "a.ts".to_string(),
            file: HashPair {
                source_hash: "filehash".to_string(),
                deps_hash: "filedepshash".to_string(),
                spec_hash: "filespechash".to_string(),
            },
            symbols: vec![(
                "a.ts::double".to_string(),
                HashPair {
                    source_hash: sym.source_hash.clone(),
                    deps_hash: "symdepshash".to_string(),
                    spec_hash: "symspechash".to_string(),
                },
            )],
            file_summary: "Doubles numbers.".to_string(),
            sections: vec![(
                "a.ts::double".to_string(),
                SymbolProse {
                    summary: "Doubles its input.".to_string(),
                    behavior: "Multiplies by two and returns.".to_string(),
                },
            )],
        };

        let rendered = render(&graph, Path::new("/nonexistent"), file_id, &spec);
        assert!(rendered.contains("function double(x: number): number"));
        let parsed = parse(&rendered).expect("should parse what we just rendered");
        assert_eq!(parsed, spec);
    }

    #[test]
    fn depends_on_is_scoped_to_what_each_symbol_actually_uses() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-depends", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let a_content = "import { clsx } from 'clsx';\nimport { twMerge } from 'tailwind-merge';\n\nexport function usesClsx(x: string) {\n  return clsx(x);\n}\n\nexport function usesNeither() {\n  return 1;\n}\n";
        let b_content = "export function clsx(x: string): string { return x; }\n";
        let c_content = "export function twMerge(x: string): string { return x; }\n";
        for (rel, content) in [
            ("a.ts", a_content),
            ("clsx.ts", b_content),
            ("tailwind-merge.ts", c_content),
        ] {
            let path = dir.join(rel);
            std::fs::write(&path, content).unwrap();
        }
        // Only need a.ts's own imports resolved against real sibling
        // files here (module resolution isn't the point of this test),
        // so build a minimal graph + resolved imports by hand rather than
        // pulling in a real tsconfig/node_modules fixture.
        let extractions = vec![
            crate::graph::extract_and_hash("a.ts", a_content),
            crate::graph::extract_and_hash("clsx.ts", b_content),
            crate::graph::extract_and_hash("tailwind-merge.ts", c_content),
        ];
        let mut graph = Graph::build(extractions);
        let file_imports = crate::imports::extract_imports(a_content, "a.ts");
        let resolved = file_imports
            .imports
            .iter()
            .map(|imp| crate::resolve::ResolvedImport {
                from_file: "a.ts".to_string(),
                specifier: imp.specifier.clone(),
                imported_name: imp.imported_name.clone(),
                target: graph.find(&format!(
                    "{}.ts::{}",
                    imp.specifier.trim_start_matches("./"),
                    imp.imported_name
                )),
            })
            .collect();
        graph.set_resolved_imports(resolved);

        let file_id = graph.find("a.ts").unwrap();
        let uses_clsx = graph
            .get_symbol(graph.find("a.ts::usesClsx").unwrap())
            .unwrap();
        let uses_neither = graph
            .get_symbol(graph.find("a.ts::usesNeither").unwrap())
            .unwrap();

        let clsx_deps = dependency_lines(&graph, &dir, file_id, uses_clsx);
        assert_eq!(clsx_deps.len(), 1);
        assert!(clsx_deps[0].contains("clsx.ts::clsx"));

        let neither_deps = dependency_lines(&graph, &dir, file_id, uses_neither);
        assert!(neither_deps.is_empty());

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
                lines: [1, 1],
                prior: None,
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
                lines: [2, 2],
                prior: None,
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
    fn submit_rejects_a_symbol_missing_required_sections() {
        let dir = std::env::temp_dir().join(format!("codeowl-spec-test-{}-3", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let graph = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        let result = submit(&graph, &dir, "a.ts::one", "just some prose, no headings");
        assert!(result.is_err());

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
            "### Summary\nOriginal summary.\n### Behavior\nOriginal behavior.\n",
        )
        .unwrap();

        // A human hand-edits the spec file's prose directly -- never
        // through submit_spec -- leaving the frontmatter hashes as they
        // were (a human wouldn't hand-update an opaque blake3 hash).
        let path = spec_path(&dir, "a.ts");
        let content = std::fs::read_to_string(&path).unwrap();
        let edited = content.replace("Original summary.", "A human-corrected summary.");
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
            })
        );

        let reread = read_file_spec(&dir, "a.ts").unwrap().unwrap();
        assert_eq!(reread.sections[0].1.summary, "A human-corrected summary.");
        let expected_hash = hash_text("A human-corrected summary.\nOriginal behavior.");
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
            "### Summary\nOriginal summary.\n### Behavior\nOriginal behavior.\n",
        )
        .unwrap();

        let path = spec_path(&dir, "a.ts");
        let content = std::fs::read_to_string(&path).unwrap();
        let edited = content.replace("Original summary.", "A human-corrected summary.");
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
                lines: [1, 3],
                prior: Some(SymbolProse {
                    summary: "A human-corrected summary.".to_string(),
                    behavior: "Original behavior.".to_string(),
                }),
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
        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\none does its job.\n### Behavior\nSee the source for details.\n",
        )
        .unwrap();

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
                lines: [1, 1],
                prior: None,
            })
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Write a small fixture repo to a fresh temp dir with imports
    /// resolved and route literals extracted -- the full pipeline
    /// `main.rs`'s `build_graph` runs, needed for feature-spec tests since
    /// they read real files off disk and need real import resolution.
    fn build_feature_fixture(
        files: &[(&str, &str)],
        suffix: &str,
    ) -> (Graph, std::path::PathBuf, Vec<RouteLiteral>) {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-feature-spec-test-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let mut extractions = Vec::new();
        let mut file_imports = std::collections::HashMap::new();
        let mut route_literals = Vec::new();
        for (rel, content) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            extractions.push(crate::graph::extract_and_hash(rel, content));
            file_imports.insert(
                rel.to_string(),
                crate::imports::extract_imports(content, rel),
            );
            route_literals.extend(crate::features::extract_route_literals(content, rel));
        }
        let mut graph = Graph::build(extractions);
        let resolver = crate::resolve::build_resolver();
        let resolved = crate::resolve::resolve_imports(&dir, &resolver, &file_imports, &graph);
        graph.set_resolved_imports(resolved);

        (graph, dir, route_literals)
    }

    const ARTWORK_FIXTURE: &[(&str, &str)] = &[
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
    fn feature_generate_loop_produces_a_current_spec_then_reports_done() {
        let (graph, dir, route_literals) = build_feature_fixture(ARTWORK_FIXTURE, "1");
        let entry = EntryPoint {
            file: "app/submit/page.tsx".to_string(),
            slug: feature_slug("app/submit/page.tsx"),
        };

        let task = next_feature_task(&graph, &dir, &entry, &route_literals)
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
            &route_literals,
            "app/submit/page.tsx",
            "# Artwork submission\n## Summary\nLets an artist submit artwork.\n",
        )
        .unwrap();

        assert!(feature_spec_path(&dir, "submit").exists());
        assert_eq!(
            next_feature_task(&graph, &dir, &entry, &route_literals).unwrap(),
            None
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submit_feature_rejects_content_without_a_title() {
        let (graph, dir, route_literals) = build_feature_fixture(ARTWORK_FIXTURE, "2");
        let result = submit_feature(
            &graph,
            &dir,
            &route_literals,
            "app/submit/page.tsx",
            "no title here, just prose",
        );
        assert!(result.is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    fn rollup_fixture_dir(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "codeowl-rollup-spec-test-{}-{suffix}",
            std::process::id()
        ))
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
                lines: [1, 1],
                prior: None,
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
    fn diff_hash_lists_reports_changed_added_and_removed() {
        let current = vec![
            ("a".to_string(), "h1".to_string()),
            ("b".to_string(), "h2-new".to_string()),
            ("c".to_string(), "h3".to_string()),
        ];
        let stored = vec![
            ("a".to_string(), "h1".to_string()),
            ("b".to_string(), "h2-old".to_string()),
            ("d".to_string(), "h4".to_string()),
        ];
        assert_eq!(
            diff_hash_lists(&current, &stored),
            vec!["added:c", "changed:b", "removed:d"]
        );
    }

    #[test]
    fn diff_hash_lists_is_empty_when_nothing_moved() {
        let list = vec![("a".to_string(), "h1".to_string())];
        assert!(diff_hash_lists(&list, &list).is_empty());
    }

    #[test]
    fn render_system_then_parse_system_round_trips() {
        let spec = SystemSpec {
            modules: vec![("lib/email".to_string(), "rolluphash".to_string())],
            features: vec![("submit".to_string(), "featurehash".to_string())],
            spec_hash: "systemhash".to_string(),
            body: "# TalentTrail\n## Summary\nA competition platform.".to_string(),
        };
        let rendered = render_system(Path::new("/nonexistent"), &spec);
        assert!(rendered.contains("lib/email: rolluphash"));
        assert!(rendered.contains("submit: featurehash"));
        let parsed = parse_system(&rendered).expect("should parse what we just rendered");
        assert_eq!(parsed, spec);
    }

    const SYSTEM_FIXTURE: &[(&str, &str)] = &[
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
        ("lib/one.ts", "export function one(): void {}\n"),
        ("lib/two.ts", "export function two(): void {}\n"),
    ];

    #[test]
    fn next_system_task_is_none_until_every_module_and_feature_is_current() {
        let (graph, dir, route_literals) = build_feature_fixture(SYSTEM_FIXTURE, "system1");

        assert_eq!(
            next_system_task(&graph, &dir, &route_literals).unwrap(),
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
            next_system_task(&graph, &dir, &route_literals).unwrap(),
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
            next_system_task(&graph, &dir, &route_literals).unwrap(),
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
            &route_literals,
            "app/submit/page.tsx",
            "# Artwork submission\n## Summary\nLets an artist submit artwork.\n",
        )
        .unwrap();

        let task = next_system_task(&graph, &dir, &route_literals)
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
            &route_literals,
            "# TalentTrail\n## Summary\nA platform for running art competitions.\n",
        )
        .unwrap();
        assert_eq!(
            next_system_task(&graph, &dir, &route_literals).unwrap(),
            None
        );
        assert!(read_system_spec(&dir).unwrap().is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submit_system_rejects_content_without_a_title() {
        let (graph, dir, route_literals) = build_feature_fixture(SYSTEM_FIXTURE, "system2");
        let result = submit_system(&graph, &dir, &route_literals, "no title here, just prose");
        assert!(result.is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn coverage_reports_everything_missing_then_everything_current() {
        let (graph, dir, route_literals) = build_feature_fixture(SYSTEM_FIXTURE, "coverage1");

        let items = coverage(&graph, &dir, &route_literals, None).unwrap();
        let summary = summarize(&items);
        assert_eq!(
            summary.missing, 8,
            "5 files + 1 rollup + 1 feature + 1 system"
        );
        assert_eq!(summary.current, 0);
        assert_eq!(summary.stale, 0);

        let pending = prioritize(items);
        let ids: Vec<&str> = pending.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "system",
                "feature:submit",
                "lib/supabase.ts",
                "app/api/submit-artwork/route.ts",
                "app/submit/page.tsx",
                "lib/one.ts",
                "lib/two.ts",
                "rollup:lib",
            ],
            "system first, then features, then files by descending fan-in \
             (lib/supabase.ts is imported by both page.tsx and route.ts), \
             then rollups"
        );
        assert_eq!(
            pending
                .iter()
                .find(|i| i.id == "lib/supabase.ts")
                .unwrap()
                .fan_in,
            2
        );

        // Generate everything, then confirm coverage agrees nothing is
        // pending.
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
        submit_rollup(
            &graph,
            &dir,
            "lib",
            "Small shared helper functions for this module.",
        )
        .unwrap();
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
        submit(
            &graph,
            &dir,
            "app/api/submit-artwork/route.ts::POST",
            "### Summary\nAccepts an artwork submission request.\n### Behavior\nPersists the submission to storage.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "app/api/submit-artwork/route.ts",
            "The API route that accepts artwork submissions.",
        )
        .unwrap();
        submit_feature(
            &graph,
            &dir,
            &route_literals,
            "app/submit/page.tsx",
            "# Artwork submission\n## Summary\nLets an artist submit artwork for judging.\n",
        )
        .unwrap();
        submit_system(
            &graph,
            &dir,
            &route_literals,
            "# TalentTrail\n## Summary\nA platform for running art competitions.\n",
        )
        .unwrap();

        let items = coverage(&graph, &dir, &route_literals, None).unwrap();
        let summary = summarize(&items);
        assert_eq!(summary.current, 8);
        assert_eq!(summary.stale, 0);
        assert_eq!(summary.missing, 0);
        assert!(prioritize(items).is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn coverage_scope_narrows_to_files_and_rollups_under_that_prefix() {
        let (graph, dir, route_literals) = build_feature_fixture(SYSTEM_FIXTURE, "coverage2");
        let items = coverage(&graph, &dir, &route_literals, Some("lib")).unwrap();
        let ids: std::collections::HashSet<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            std::collections::HashSet::from([
                "lib/supabase.ts",
                "lib/one.ts",
                "lib/two.ts",
                "rollup:lib",
            ]),
            "scope excludes app/* files, the feature, and the system spec"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn within_scope_respects_path_boundaries_not_bare_string_prefixes() {
        // A real bug caught dogfooding against the pilot repo: "lib/email"
        // as a bare string prefix also matches "lib/email.ts", an
        // unrelated sibling file that merely shares the prefix.
        assert!(within_scope("lib/email", "lib/email"));
        assert!(within_scope("lib/email/config.ts", "lib/email"));
        assert!(!within_scope("lib/email.ts", "lib/email"));
        assert!(!within_scope("lib/email-utils/x.ts", "lib/email"));
        assert!(within_scope("anything", ""));
    }

    #[test]
    fn prose_smells_flags_cop_out_phrases_and_short_text_but_not_real_prose() {
        assert_eq!(
            prose_smells("Returns the cached client. See the source for details."),
            vec!["cop_out_phrase"]
        );
        assert_eq!(prose_smells("Does one thing."), vec!["suspiciously_short"]);
        assert!(
            prose_smells(
                "Formats a date string into a human-readable form, returning \
                 a fallback for invalid input."
            )
            .is_empty()
        );
    }

    #[test]
    fn file_spec_smells_catches_the_real_pre_m5_stub_pattern() {
        // The exact shape found auditing two real pre-M5 specs: every
        // symbol's prose is a template stub ("<name> does its job." /
        // "See source."), and every symbol shares an identical,
        // non-empty dependency list (the file-wide-attribution bug).
        // dependency_lines needs each symbol's own source text on disk
        // (see read_symbol_text), so this needs a real temp dir, the same
        // pattern depends_on_is_scoped_to_what_each_symbol_actually_uses
        // uses.
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-smell-fixture",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let a_content = "import { helper } from './util';\n\nexport function one() {\n  helper();\n}\n\nexport function two() {\n  helper();\n}\n";
        let util_content = "export function helper(): void {}\n";
        for (rel, content) in [("a.ts", a_content), ("util.ts", util_content)] {
            std::fs::write(dir.join(rel), content).unwrap();
        }
        let extractions = vec![
            crate::graph::extract_and_hash("a.ts", a_content),
            crate::graph::extract_and_hash("util.ts", util_content),
        ];
        let mut graph = Graph::build(extractions);
        let file_imports = crate::imports::extract_imports(a_content, "a.ts");
        let resolved = file_imports
            .imports
            .iter()
            .map(|imp| crate::resolve::ResolvedImport {
                from_file: "a.ts".to_string(),
                specifier: imp.specifier.clone(),
                imported_name: imp.imported_name.clone(),
                target: graph.find(&format!(
                    "{}.ts::{}",
                    imp.specifier.trim_start_matches("./"),
                    imp.imported_name
                )),
            })
            .collect();
        graph.set_resolved_imports(resolved);
        let file_id = graph.find("a.ts").unwrap();

        let spec = FileSpec {
            source_path: "a.ts".to_string(),
            file: HashPair::default(),
            symbols: vec![
                ("a.ts::one".to_string(), HashPair::default()),
                ("a.ts::two".to_string(), HashPair::default()),
            ],
            file_summary: "a.ts summary.".to_string(),
            sections: vec![
                (
                    "a.ts::one".to_string(),
                    SymbolProse {
                        summary: "one does its job.".to_string(),
                        behavior: "See source.".to_string(),
                    },
                ),
                (
                    "a.ts::two".to_string(),
                    SymbolProse {
                        summary: "two does its job.".to_string(),
                        behavior: "See source.".to_string(),
                    },
                ),
            ],
        };

        let mut smells = file_spec_smells(&graph, &dir, file_id, &spec);
        smells.sort();
        assert_eq!(
            smells,
            vec![
                "cop_out_phrase",
                "identical_dependencies_across_symbols",
                "suspiciously_short"
            ]
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_spec_smells_is_empty_for_real_looking_prose_with_distinct_dependencies() {
        let graph = build_graph_from_sources(&[(
            "a.ts",
            "export function one(): void {}\nexport function two(): void {}\n",
        )]);
        let file_id = graph.find("a.ts").unwrap();
        let spec = FileSpec {
            source_path: "a.ts".to_string(),
            file: HashPair::default(),
            symbols: vec![
                ("a.ts::one".to_string(), HashPair::default()),
                ("a.ts::two".to_string(), HashPair::default()),
            ],
            file_summary: "Two small, unrelated utility functions used across the app.".to_string(),
            sections: vec![
                (
                    "a.ts::one".to_string(),
                    SymbolProse {
                        summary: "Does the first specific thing this file needs.".to_string(),
                        behavior: "Runs synchronously with no side effects at all.".to_string(),
                    },
                ),
                (
                    "a.ts::two".to_string(),
                    SymbolProse {
                        summary: "Does a second, unrelated specific thing.".to_string(),
                        behavior: "Also runs synchronously with no side effects.".to_string(),
                    },
                ),
            ],
        };
        assert!(file_spec_smells(&graph, Path::new("/nonexistent"), file_id, &spec).is_empty());
    }

    #[test]
    fn body_smells_checks_only_the_summary_section_of_a_feature_or_rollup_body() {
        assert_eq!(
            body_smells(
                "# Title\n## Summary\nSee the route handler for details.\n## How it works\n1. Does something else entirely, at length, in this other section.\n"
            ),
            vec!["cop_out_phrase"]
        );
        assert!(
            body_smells(
                "# Title\n## Summary\nLets a registered competitor submit their artwork \
                 for judging in a competition.\n## How it works\n1. Step one.\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn coverage_includes_a_current_but_smelly_file_in_pending() {
        let (graph, dir, route_literals) = build_feature_fixture(SYSTEM_FIXTURE, "coverage3");
        submit(
            &graph,
            &dir,
            "lib/supabase.ts::getSupabase",
            "### Summary\nGetSupabase does its job.\n### Behavior\nSee the source for details.\n",
        )
        .unwrap();
        submit(&graph, &dir, "lib/supabase.ts", "lib/supabase.ts summary.").unwrap();

        let items = coverage(&graph, &dir, &route_literals, Some("lib/supabase.ts")).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "current");
        assert!(!items[0].smells.is_empty());

        let summary = summarize(&items);
        assert_eq!(summary.current, 1);
        assert_eq!(summary.smelly, 1);

        let pending = prioritize(items);
        assert_eq!(
            pending.len(),
            1,
            "a current-but-smelly document must still show up as pending"
        );
        assert_eq!(pending[0].status, "current");

        std::fs::remove_dir_all(&dir).ok();
    }
}
