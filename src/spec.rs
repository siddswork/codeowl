//! The spec document format and file writer — M4 (file specs), M5 (feature
//! specs), and M6 (directory rollups). Implements `ARCHITECTURE.md`'s "Spec
//! document format": the mirrored `docs/specs/` tree, per-symbol/per-file/
//! per-directory hash frontmatter, and the granularity rules deciding which
//! documents exist at all.
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
    EntryPoint, Participants, RouteLiteral, assemble_participants, feature_slug,
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
    },
    File {
        id: String,
    },
}

/// The next thing `/codeowl generate <target_file_id>` needs written, or
/// `None` if the file isn't spec-bearing at all, or everything on it is
/// already current. Stateless and idempotent: safe to call repeatedly
/// (the client-side generate loop's own termination condition), and
/// doesn't touch disk beyond a read.
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
        let current = existing
            .as_ref()
            .and_then(|spec| spec.symbol_hash(&sym.id))
            .is_some_and(|h| symbol_changes(graph, root, target_file_id, sym, h).is_empty());
        if !current {
            return Ok(Some(SpecTask::Symbol {
                id: sym.id.clone(),
                signature: sym.signature.clone(),
                docstring: sym.docstring.clone(),
                lines: sym.lines,
            }));
        }
    }

    let file_current = existing
        .as_ref()
        .is_some_and(|spec| file_changes(graph, target_file_id, &spec.file).is_empty());
    if !file_current {
        return Ok(Some(SpecTask::File {
            id: file.id.clone(),
        }));
    }

    Ok(None)
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
    let is_current =
        existing.is_some_and(|spec| diff_hash_lists(&current, &spec.participants).is_empty());
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
    let is_current = existing.is_some_and(|spec| diff_hash_lists(&current, &spec.files).is_empty());
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
            }
        );

        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nDoes one thing.\n### Behavior\nNo-op.\n",
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
            }
        );

        submit(
            &graph,
            &dir,
            "a.ts::two",
            "### Summary\nDoes two.\n### Behavior\nNo-op.\n",
        )
        .unwrap();
        let task = next_task(&graph, &dir, file_id).unwrap().unwrap();
        assert_eq!(
            task,
            SpecTask::File {
                id: "a.ts".to_string()
            }
        );

        submit(&graph, &dir, "a.ts", "Two no-op helpers.").unwrap();
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
            "### Summary\nS.\n### Behavior\nB.\n",
        )
        .unwrap();
        // Re-running generate against the same, unchanged graph should
        // skip straight past the symbol (already current) to the file.
        let task = next_task(&graph, &dir, file_id).unwrap().unwrap();
        assert_eq!(
            task,
            SpecTask::File {
                id: "a.ts".to_string()
            }
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
            }
        );
        assert_eq!(next_rollup_task(&graph, &dir, "lib").unwrap(), None);

        submit(
            &graph,
            &dir,
            "lib/one.ts::one",
            "### Summary\nS1.\n### Behavior\nB1.\n",
        )
        .unwrap();
        submit(&graph, &dir, "lib/one.ts", "Does one thing.").unwrap();
        submit(
            &graph,
            &dir,
            "lib/two.ts::two",
            "### Summary\nS2.\n### Behavior\nB2.\n",
        )
        .unwrap();
        submit(&graph, &dir, "lib/two.ts", "Does two things.").unwrap();

        // Both files are now current -- the directory chase is exhausted,
        // and the rollup task itself becomes available.
        assert_eq!(next_task_for_directory(&graph, &dir, "lib").unwrap(), None);
        let rollup_task = next_rollup_task(&graph, &dir, "lib")
            .unwrap()
            .expect("both files current, rollup itself still missing");
        assert_eq!(
            rollup_task.files,
            vec![
                ("lib/one.ts".to_string(), "Does one thing.".to_string()),
                ("lib/two.ts".to_string(), "Does two things.".to_string()),
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
            ("lib/one.ts::one", "### Summary\nS1.\n### Behavior\nB1.\n"),
            ("lib/two.ts::two", "### Summary\nS2.\n### Behavior\nB2.\n"),
        ] {
            submit(&graph, &dir, id, content).unwrap();
        }
        submit(&graph, &dir, "lib/one.ts", "Does one thing.").unwrap();
        submit(&graph, &dir, "lib/two.ts", "Does two things.").unwrap();

        let spec = submit_rollup(&graph, &dir, "lib", "Small shared helpers.").unwrap();
        assert_eq!(spec.files.len(), 2);
        assert!(spec.files.iter().all(|(_, h)| !h.is_empty()));

        assert!(rollup_spec_path(&dir, "lib").exists());
        assert_eq!(
            read_rollup_spec(&dir, "lib").unwrap().unwrap().body,
            "Small shared helpers."
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
}
