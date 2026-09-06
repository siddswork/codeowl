//! The spec document format and file writer — M4. Implements
//! `ARCHITECTURE.md`'s "Spec document format" for file specs: the mirrored
//! `docs/specs/` tree, per-symbol/per-file hash frontmatter, and the
//! granularity rule deciding whether a file gets a document at all.
//!
//! Frontmatter here is hand-parsed rather than run through a general YAML
//! library: it's one fixed, CodeOwl-owned shape (see the `render`/`parse`
//! pair below), not arbitrary human-authored YAML, so a parser scoped
//! exactly to that shape is simpler — and easier to reason about — than a
//! full grammar for something we control both ends of.
//!
//! Directory rollups, feature specs, and the token-budget recursion
//! threshold (open question 2) are out of scope here — this module only
//! ever produces the two-level (file, its top-level function/class
//! symbols) document M4 scopes in.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::graph::{Graph, Node, SymbolId};
use crate::hash::hash_text;
use crate::symbol::SymbolKind;

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
/// root).
///
/// This only implements the *rule*, not the rollup document itself —
/// unlike the file and feature spec shapes, `ARCHITECTURE.md`'s "Spec
/// document format" never templated a directory rollup's frontmatter/body,
/// so writing one now would mean inventing an unreviewed format rather
/// than implementing a decided one. Callers can use this to decide
/// *whether* a directory would get a document without CodeOwl yet being
/// able to produce it.
pub fn directory_is_spec_bearing(graph: &Graph, dir_path: &str) -> bool {
    graph
        .files()
        .filter(|f| {
            Path::new(&f.id)
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                == Some(dir_path.to_string())
        })
        .filter(|f| {
            graph
                .find(&f.id)
                .is_some_and(|id| file_is_spec_bearing(graph, id))
        })
        .count()
        >= 2
}

/// The short name a section heading uses — the part of a top-level
/// symbol's stable id after `file::`.
fn short_name(id: &str) -> &str {
    id.rsplit("::").next().unwrap_or(id)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HashPair {
    pub source_hash: String,
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
/// doc comment on what CodeOwl writes vs. what the LLM writes).
pub fn render(graph: &Graph, file_id: SymbolId, spec: &FileSpec) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("kind: file\n");
    out.push_str(&format!("source_paths: [{}]\n", spec.source_path));
    out.push_str(&format!(
        "file: {{ source_hash: {}, spec_hash: {} }}\n",
        spec.file.source_hash, spec.file.spec_hash
    ));
    if !spec.symbols.is_empty() {
        out.push_str("symbols:\n");
        for (id, h) in &spec.symbols {
            out.push_str(&format!(
                "  {id}: {{ source_hash: {}, spec_hash: {} }}\n",
                h.source_hash, h.spec_hash
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
        let deps = dependency_lines(graph, file_id);
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

/// The file's resolved imports, one bullet per import — CodeOwl-written,
/// never the LLM's job (see `ARCHITECTURE.md`'s "What CodeOwl writes vs.
/// what the LLM writes"). File-level granularity, same limitation
/// `get_callees` already documents: M2 resolves file-to-file edges, not
/// per-symbol ones, so every symbol section in a file currently repeats
/// the same file-wide dependency list.
fn dependency_lines(graph: &Graph, file_id: SymbolId) -> Vec<String> {
    let Node::File(file) = graph.get(file_id) else {
        return Vec::new();
    };
    graph
        .imports()
        .iter()
        .filter(|imp| imp.from_file == file.id)
        .map(|imp| match imp.target {
            Some(target) => format!("`{}` — {}", graph.string_id(target), imp.specifier),
            None => format!(
                "{} — {} (external or unresolved)",
                imp.imported_name, imp.specifier
            ),
        })
        .collect()
}

/// Parse a spec file's own rendered output back into a `FileSpec`. Only
/// needs to tolerate what `render` produces — human-edit tolerance
/// (arbitrary reordering, reformatting) is M6's "Human corrections"
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
    std::fs::write(&path, render(graph, file_id, spec))
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
            .is_some_and(|h| h.source_hash == sym.source_hash);
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
        .is_some_and(|spec| spec.file.source_hash == file.source_hash);
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
                spec_hash: "filespechash".to_string(),
            },
            symbols: vec![(
                "a.ts::double".to_string(),
                HashPair {
                    source_hash: sym.source_hash.clone(),
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

        let rendered = render(&graph, file_id, &spec);
        assert!(rendered.contains("function double(x: number): number"));
        let parsed = parse(&rendered).expect("should parse what we just rendered");
        assert_eq!(parsed, spec);
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
}
