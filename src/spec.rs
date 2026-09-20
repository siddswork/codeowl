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
    EntryPoint, Participants, assemble_participants, enumerate_entry_points, feature_model_for,
    feature_slug,
};
use crate::graph::{Graph, Node, SymbolId};
use crate::hash::hash_text;
use crate::lang::FileRole;
use crate::symbol::{Symbol, SymbolKind};

/// A repo-relative path's [`FileRole`] under the stack that built `graph`
/// — `RustStack` calls `tests/` a test root, `TypeScriptNextStack` uses
/// the JS conventions. Replaces the direct `lang::classify` call so
/// prioritisation isn't hard-wired to one stack.
fn classify_in(graph: &Graph, path: &str) -> FileRole {
    graph.file_role(path)
}

/// Where a file's spec lives, mirrored under `docs/specs/` — never strips
/// the source extension: `lib/utils.ts` -> `docs/specs/lib/utils.ts.md`.
pub fn spec_path(root: &Path, source_path: &str) -> PathBuf {
    root.join("docs")
        .join("specs")
        .join(format!("{source_path}.md"))
}

/// Whether a repo-relative path is test code — an `e2e/` tree, a
/// `__tests__/` directory, a `cypress/`/`playwright/` tree, or a
/// `.test.`/`.spec.` file. Test code stays in the graph (so `get_callers`
/// still shows "used by these tests"), but its specs sort to the very
/// bottom of a `--all` run and it's never treated as a product module —
/// documenting the test harness is rarely the point of a spec corpus, and
/// whenever it is, it's safe to leave for last. Also strips a
/// `rollup:`/`feature:` id prefix so it can be asked of a coverage id.
pub fn is_test_path(graph: &Graph, path: &str) -> bool {
    matches!(classify_in(graph, path), FileRole::Test)
}

/// A file is spec-bearing iff it declares at least one exported `Callable`
/// or `Container` among its top-level symbols — barrel files, const-only
/// route config, and metadata-only boilerplate get no document (see
/// `ARCHITECTURE.md`'s granularity rules).
///
/// **`FileRole::Generated` is never spec-bearing, regardless of what it
/// exports (M19).** A build-generated interface (an OpenAPI-codegen'd
/// JAX-RS resource, a `.proto` stub) can export plenty of real methods —
/// confirmed against a real `mvn generate-sources` run, `HeroesResource`
/// alone has 8 — but it's read-only structural reference, never the
/// hand-written code a spec describes, per the "only spec the
/// hand-written code" principle `ARCHITECTURE.md` open question 11
/// states. Checked first so it short-circuits before the exported-symbol
/// scan below runs at all.
pub fn file_is_spec_bearing(graph: &Graph, file_id: SymbolId) -> bool {
    if matches!(
        classify_in(graph, graph.string_id(file_id)),
        FileRole::Generated
    ) {
        return false;
    }
    graph.children_ids(file_id).iter().any(|&id| {
        graph.get_symbol(id).is_some_and(|s| {
            s.is_exported && matches!(s.kind, SymbolKind::Callable | SymbolKind::Container)
        })
    })
}

/// How many of `graph`'s files sit under the active pack's declared
/// generated-source directories (M19), for `get_spec_coverage` to surface
/// to the driving LLM. `None` for any pack whose `generated_source_dirs()`
/// is empty (every pack but Java today) — the concept doesn't apply, so
/// there's nothing honest to report, not a zero. `found: 0` for a pack
/// that *does* declare them is the actionable signal: this could be a
/// Quarkus-shaped service with build-generated entry points, and none
/// were found — has `mvn generate-sources` (or the Gradle equivalent)
/// been run locally? A full build isn't needed, just that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedSourcesSummary {
    pub checked_dirs: Vec<String>,
    pub found: usize,
}

pub fn generated_sources_summary(graph: &Graph) -> Option<GeneratedSourcesSummary> {
    let pack = crate::stack::for_name(graph.pack_name());
    let dirs = pack.generated_source_dirs();
    if dirs.is_empty() {
        return None;
    }
    let found = graph
        .files()
        .filter(|f| matches!(classify_in(graph, &f.id), FileRole::Generated))
        .count();
    Some(GeneratedSourcesSummary {
        checked_dirs: dirs.iter().map(|d| d.to_string()).collect(),
        found,
    })
}

/// The top-level symbols a file spec gives their own section — both
/// exported and unexported `Callable`s/`Container`s (the point is
/// describing how the file works, not only its public API). Top-level
/// `Value`s (a `const`) get no subsection of their own; a container's
/// members are covered inside the container's own section, not separately,
/// matching how M2 already treats a class as one containment unit.
fn spec_bearing_children(graph: &Graph, file_id: SymbolId) -> Vec<SymbolId> {
    graph
        .children_ids(file_id)
        .iter()
        .copied()
        .filter(|&id| {
            graph
                .get_symbol(id)
                .is_some_and(|s| matches!(s.kind, SymbolKind::Callable | SymbolKind::Container))
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
    /// Per top-level `Callable`/`Container` symbol, in declaration order.
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
///
/// Resolved intra-repo dependencies get one line each; unresolved
/// imports (a stdlib type, a third-party crate, an npm package) are
/// collapsed into a single trailing `externals: …` line by package name.
/// Rust code names `Vec` / `BTreeMap` / `Result` constantly and every one
/// is an unresolved external, so a line apiece would bury the signal
/// that this section exists for — what *else in this repo* the symbol
/// leans on (M14).
fn dependency_lines(graph: &Graph, root: &Path, file_id: SymbolId, sym: &Symbol) -> Vec<String> {
    let Node::File(file) = graph.get(file_id) else {
        return Vec::new();
    };
    let Ok(symbol_text) = symbol_span_text(root, graph, &file.id, sym) else {
        return Vec::new();
    };
    let scoped = scoped_symbol_deps(graph, &file.id, &symbol_text);

    let mut lines: Vec<String> = scoped
        .resolved
        .iter()
        .map(|(target, specifier)| format!("`{target}` — {specifier}"))
        .collect();
    if !scoped.externals.is_empty() {
        lines.push(format!("externals: {}", scoped.externals.join(", ")));
    }
    lines
}

/// One symbol's imports, scoped by whole-word text search over its own
/// source span (see `dependency_lines`): the resolved intra-repo ones as
/// `(target string-id, specifier)` pairs, and the unresolved ones folded
/// to a sorted, de-duplicated list of package names (`std::collections`
/// -> `std`, `next/navigation` -> `next`; a relative specifier that
/// resolved to nothing is kept verbatim — a broken import is a real
/// signal). Shared by the rendered `### Depends on` section and the
/// generation-task context `mcp.rs` hands the agent.
pub(crate) struct ScopedDeps {
    pub resolved: Vec<(String, String)>,
    pub externals: Vec<String>,
}

pub(crate) fn scoped_symbol_deps(graph: &Graph, from_file: &str, symbol_text: &str) -> ScopedDeps {
    let mut resolved = Vec::new();
    let mut externals: Vec<String> = Vec::new();
    for imp in graph.imports().iter().filter(|imp| {
        imp.from_file == from_file && contains_identifier(symbol_text, &imp.imported_name)
    }) {
        match imp.target {
            Some(target) => {
                resolved.push((graph.string_id(target).to_string(), imp.specifier.clone()));
            }
            None => {
                let pkg = if imp.specifier.starts_with('.') {
                    imp.specifier.clone()
                } else {
                    imp.specifier
                        .split(['/', ':'])
                        .next()
                        .unwrap_or(&imp.specifier)
                        .to_string()
                };
                if !externals.contains(&pkg) {
                    externals.push(pkg);
                }
            }
        }
    }
    externals.sort();
    ScopedDeps {
        resolved,
        externals,
    }
}

/// The raw source text that scopes a symbol's dependencies and feeds its
/// generation task: `sym`'s own line span, plus the span of any direct
/// child that lies *outside* it. For a TS class every method already sits
/// inside the class braces, so this is just the class body; for a Rust
/// type whose inherent `impl` blocks were folded in (M15,
/// `rust::merge_inherent_impls`), the method spans live elsewhere in the
/// file and are appended. Adjacent/overlapping spans are merged so the
/// common case (a `struct` immediately followed by its `impl`) still reads
/// as one contiguous block.
pub(crate) fn symbol_span_text(
    root: &Path,
    graph: &Graph,
    rel_path: &str,
    sym: &Symbol,
) -> Result<String> {
    let mut spans: Vec<[usize; 2]> = vec![sym.lines];
    let [own_start, own_end] = sym.lines;
    for &child in &sym.children {
        if let Some(c) = graph
            .get_symbol(child)
            .filter(|c| c.lines[0] < own_start || c.lines[1] > own_end)
        {
            spans.push(c.lines);
        }
    }
    spans.sort_by_key(|s| s[0]);
    // Merge spans that touch or overlap (a blank line or two between a
    // `struct` and its `impl` shouldn't split the block).
    let mut merged: Vec<[usize; 2]> = Vec::new();
    for s in spans {
        match merged.last_mut() {
            Some(last) if s[0] <= last[1] + 2 => last[1] = last[1].max(s[1]),
            _ => merged.push(s),
        }
    }

    let content = std::fs::read_to_string(root.join(rel_path))
        .with_context(|| format!("reading {rel_path}"))?;
    let all: Vec<&str> = content.lines().collect();
    let mut out = String::new();
    for (i, [start, end]) in merged.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        let lo = start.saturating_sub(1).min(all.len());
        let hi = (*end).min(all.len());
        out.push_str(&all[lo..hi].join("\n"));
    }
    Ok(out)
}

/// Above this many bytes, a Container's generation-task `source` is
/// reduced to signatures + docstrings instead of full bodies (M16's
/// headline finding, fixed at M18 since Java's classes are where it bites
/// hardest — `StringUtils` on commons-lang is 9,421 lines / ~420 KB;
/// CodeOwl's own `mcp.rs`/`spec.rs` hit ~95 KB).
///
/// **Recalibrated 2026-09-16, after this constant's first value (30,000)
/// shipped without ever being validated against a real MCP client.** A
/// real user's Python class hit VS Code Copilot Chat's inline-context
/// overflow (it spills an oversized tool result to a `content.json` temp
/// file and asks the agent to `read_file` it back — a documented Copilot
/// limitation, not an `rmcp`/transport bug) at just **11,498 bytes** —
/// under a third of the old threshold, so that class sailed through
/// unreduced. The original number was anchored to "when does this get
/// absurdly large" (the two cases above), not "what's actually safe for
/// a real client" — those are different questions, and only the second
/// one matters here. Lowered well under the observed failure; see
/// [`MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT`] below for the hard backstop that
/// doesn't depend on this number being right, since Copilot's exact
/// ceiling still isn't known — only that it's ≤ 11,498.
///
/// Byte-based, not line-based: token count tracks text volume more
/// directly than line count, and this codebase has no precise tokenizer
/// to reach for (`ARCHITECTURE.md` open question 2 is exactly that gap,
/// still open) — a laptop-scale heuristic constant, in the same spirit
/// as `SHARED_CODE_MAX_FILES`, not a measured limit. Overridable per
/// server instance (`mcp.rs::CodeOwlServer::with_generation_limits`,
/// wired to `codeowl serve`'s `--large-class-bytes` flag) since no
/// single number is right for every MCP client, and guessing wrong once
/// already shipped a bug — an explicit parameter, not an env var read,
/// so the value is visible in one place (the CLI invocation) and every
/// call site stays a pure function with no hidden global state.
pub const LARGE_CONTAINER_BYTES_DEFAULT: usize = 4_000;

/// A hard ceiling on any single generation-task text field
/// (`SpecTaskResponse::Symbol::source` or `::File::source`), applied
/// *after* whatever reduction already happened — the backstop for the
/// real bug this constant's sibling above was recalibrated from: we
/// don't actually know VS Code Copilot Chat's overflow threshold, only
/// that it's ≤ 11,498 bytes, so a heuristic reduction trigger alone is a
/// guess, never a guarantee. This is the guarantee: nothing this crate
/// hands back as a generation-task `source` can ever exceed this many
/// bytes, full stop, regardless of which code path produced it. Set well
/// under the observed failure for real margin. `File` tasks had **no**
/// size handling at all before this — a large flat file with no single
/// big class was completely unprotected even after the Container fix.
/// Overridable the same way as `LARGE_CONTAINER_BYTES_DEFAULT` (the
/// `--max-spec-task-bytes` flag) — different MCP clients have different
/// real ceilings, and this codebase can't know all of them.
pub const MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT: usize = 8_000;

/// The visible truncation notice [`cap_generation_text`] appends — pulled
/// out to a constant so its own byte length can be reserved against
/// `max_bytes` rather than added on top of it.
const TRUNCATION_MARKER: &str = "\n\n[... truncated: this content exceeded the generation-task \
    size limit. Use get_symbol / search_code / get_callers for the parts not shown here.]";

/// Truncate `text` to at most `max_bytes` *total, marker included* —
/// cutting at a `char` boundary (never splitting a multi-byte UTF-8
/// sequence) and appending a visible marker so truncation is never silent
/// (an agent that gets a suspiciously neat cutoff with no note would have
/// no way to know the class continues past what it can see). Returns
/// `text` unchanged if it's already within the cap.
///
/// Code-review finding: this used to truncate the *text* to `max_bytes`
/// and then append the marker on top, so the real returned length was
/// `max_bytes` plus the marker's own ~145 bytes — silently breaking the
/// "nothing this crate hands back can ever exceed this many bytes, full
/// stop" guarantee `MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT`'s doc comment
/// makes, in exactly the scenario this whole mechanism exists for (a real
/// MCP client's true ceiling sitting close to the configured cap). Fixed
/// by reserving the marker's length out of the budget *before* cutting
/// the text, then fitting the marker itself into whatever's left — if
/// `max_bytes` is smaller than the marker alone (an unreasonably tight
/// cap), the marker gets truncated too rather than the guarantee being
/// broken; an honest, cut-off notice is worth more than exact wording
/// once the budget is that tight.
pub(crate) fn cap_generation_text(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    let text_budget = max_bytes.saturating_sub(TRUNCATION_MARKER.len());
    let mut cut = text_budget.min(text.len());
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = text[..cut].to_string();

    let marker_budget = max_bytes - out.len();
    let mut marker_cut = marker_budget.min(TRUNCATION_MARKER.len());
    while marker_cut > 0 && !TRUNCATION_MARKER.is_char_boundary(marker_cut) {
        marker_cut -= 1;
    }
    out.push_str(&TRUNCATION_MARKER[..marker_cut]);
    out
}

/// `full_source` (the true full span from [`symbol_span_text`]) unless
/// `sym` is a `Container` over `threshold` bytes with members to
/// reduce, in which case each child's `signature` + `docstring` replaces
/// its body — a class-level `### Summary`/`### Behavior` prompt needs to
/// know a member exists and what it's for, not reproduce every
/// implementation. Deliberately **no** visibility-aware carve-out (a
/// private helper keeping its full body, as `ROADMAP.md`'s original
/// wording sketched): the real forcing case (`StringUtils`) is 240 public
/// methods against 15 private ones, so the exception would buy little for
/// real added complexity (and language-specific visibility rules
/// awkwardly fit this generic, stack-neutral function) — revisit only if
/// a real dogfood pass shows those bodies were actually load-bearing.
///
/// Correctness-critical callers (`### Depends on`, `scoped_symbol_deps`)
/// must keep scanning `full_source` itself, never this reduced text — a
/// dependency used only inside a now-omitted body must still be found.
/// This function is for the *generation-task display* only.
pub(crate) fn maybe_reduce_container_source(
    sym: &Symbol,
    graph: &Graph,
    full_source: String,
    threshold: usize,
) -> String {
    if sym.kind != SymbolKind::Container
        || sym.children.is_empty()
        || full_source.len() <= threshold
    {
        return full_source;
    }
    let mut out = sym.signature.clone();
    if let Some(doc) = &sym.docstring {
        out.push('\n');
        out.push_str(doc);
    }
    for &child_id in &sym.children {
        let Some(child) = graph.get_symbol(child_id) else {
            continue;
        };
        out.push_str("\n\n");
        out.push_str(&child.signature);
        if let Some(doc) = &child.docstring {
            out.push('\n');
            out.push_str(doc);
        }
    }
    out
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
    let Ok(symbol_text) = symbol_span_text(root, graph, &file.id, sym) else {
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
/// Refuse to persist prose that trips the deterministic quality check
/// (`prose_smells`) `get_next_spec_task` also runs. Storing smelly prose
/// isn't harmless: a smell in the "source unchanged, not human-edited"
/// case is exactly what makes `next_task` re-offer the document, so a
/// generate loop that keeps resubmitting equally-thin prose never
/// terminates. Erroring at submit time turns that silent non-termination
/// into a message naming what to fix — the same stance `submit` already
/// takes on content missing a `### Summary`.
fn reject_if_smelly(what: &str, smells: Vec<String>) -> Result<()> {
    if smells.is_empty() {
        return Ok(());
    }
    bail!(
        "{what} fails a quality check ({}) — expand it; `get_next_spec_task` \
         re-offers a spec until its prose clears the check",
        smells.join(", ")
    );
}

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
            reject_if_smelly(
                &format!("submitted summary for {id:?}"),
                prose_smells(&summary),
            )?;
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
            reject_if_smelly(
                &format!("submitted prose for {id:?}"),
                prose_smells(&summary)
                    .into_iter()
                    .map(|s| format!("Summary: {s}"))
                    .chain(
                        prose_smells(&behavior)
                            .into_iter()
                            .map(|s| format!("Behavior: {s}")),
                    )
                    .collect(),
            )?;

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

/// Test helper: put a hash-*current* symbol spec on disk whose prose would
/// fail `prose_smells` — the shape a pre-fix cache carries, or a hand-edit
/// whose frontmatter `spec_hash` was fixed up to match. `submit` now
/// refuses to write such prose, so the tests that exercise the
/// smell-driven `next_task` re-offer plant it directly. Seeds a clean spec
/// via `submit`, then swaps in the smelly prose and its matching hash.
#[cfg(test)]
pub(crate) fn plant_smelly_symbol_spec(
    graph: &Graph,
    root: &Path,
    sym_id: &str,
    summary: &str,
    behavior: &str,
) {
    const OK_S: &str = "A placeholder summary comfortably past the four word floor.";
    const OK_B: &str = "A placeholder behavior sentence comfortably past the four word floor.";
    submit(
        graph,
        root,
        sym_id,
        &format!("### Summary\n{OK_S}\n### Behavior\n{OK_B}\n"),
    )
    .expect("placeholder prose is clean");

    let file_id = graph
        .get_symbol(graph.find(sym_id).expect("symbol exists"))
        .and_then(|s| s.parent)
        .expect("symbol has a containing file");
    let file_string_id = graph
        .get_file(file_id)
        .expect("parent is a file")
        .id
        .clone();
    let path = spec_path(root, &file_string_id);

    let patched = std::fs::read_to_string(&path)
        .expect("spec was written")
        .replace(OK_S, summary)
        .replace(OK_B, behavior)
        .replace(
            &hash_text(&format!("{OK_S}\n{OK_B}")),
            &hash_text(&format!("{summary}\n{behavior}")),
        );
    std::fs::write(&path, patched).expect("rewrite spec");
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
    let body = content.trim().to_string();
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
    /// How many `get_next_spec_task` → `submit_spec` cycles this document
    /// still costs — i.e. what one unit of `/codeowl generate --budget=N`
    /// buys. For a file: its uncovered/stale/smelly top-level symbols,
    /// plus 1 for the file's own `## Summary` if that needs (re)writing.
    /// For a rollup/feature/the system spec: 1 while non-current, else 0.
    /// `0` for a genuinely clean, current document. Summed across the
    /// report this is the total budget a full run needs (see
    /// `CoverageSummary::generations_remaining`).
    pub generations: usize,
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
    // Only smell-check the file summary once it's actually been written.
    // `submit_spec` on a symbol lazily creates a `FileSpec` with a blank
    // summary (`FileSpec::blank`); an empty `spec_hash` is that "never
    // generated" signal (the same one `next_task` reads), and an empty
    // summary in that state is a mid-generation gap, not a short cop-out.
    let mut smells = if spec.file.spec_hash.is_empty() {
        Vec::new()
    } else {
        prose_smells(&spec.file_summary)
    };
    for (_, prose) in &spec.sections {
        smells.extend(prose_smells(&prose.summary));
        smells.extend(prose_smells(&prose.behavior));
    }
    if spec.symbols.len() >= 2 {
        // Compare only the *resolved* intra-repo lines — a shared
        // `externals: …` line (every symbol in a small module returning
        // `anyhow::Result`, say) is normal, not a scoping failure.
        let dep_lists: Vec<Vec<String>> = spec
            .symbols
            .iter()
            .filter_map(|(id, _)| {
                let sym_id = graph.find(id)?;
                let sym = graph.get_symbol(sym_id)?;
                Some(
                    dependency_lines(graph, root, file_id, sym)
                        .into_iter()
                        .filter(|line| !line.starts_with("externals:"))
                        .collect(),
                )
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

/// How many *non-test* files import something from this file — the "shared
/// infrastructure" signal `prioritize` tiers on. Imports from test code
/// don't count: a `lib/` helper pulled in by ten e2e specs and two real
/// callers is depended on by two things, not twelve.
fn file_fan_in(graph: &Graph, file_id: SymbolId) -> usize {
    graph
        .imports()
        .iter()
        .filter(|imp| !is_test_path(graph, &imp.from_file))
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
/// How many generation cycles `next_task` would still yield for `file_id`
/// if drained — a read-only mirror of `next_task`'s per-symbol decision
/// (never generated / source moved / smelly → a task; source unchanged +
/// human-edited → a silent reconcile, no task). No side effects: unlike
/// `next_task` it doesn't reconcile, it just counts.
fn file_pending_generations(graph: &Graph, root: &Path, file_id: SymbolId) -> Result<usize> {
    let file = graph.get_file(file_id).context("not a file id")?;
    let existing = read_existing(root, &file.id)?;
    let mut n = 0;

    for sym_id in spec_bearing_children(graph, file_id) {
        let sym = graph.get_symbol(sym_id).expect("filtered to symbol ids");
        match existing.as_ref().and_then(|spec| spec.symbol_hash(&sym.id)) {
            None => n += 1, // never generated
            Some(hash) => {
                let source_changed = !symbol_changes(graph, root, file_id, sym, hash).is_empty();
                let prose = existing
                    .as_ref()
                    .and_then(|spec| spec.section(&sym.id).cloned())
                    .unwrap_or_default();
                let smelly = !prose_smells(&prose.summary).is_empty()
                    || !prose_smells(&prose.behavior).is_empty();
                // source unchanged + human-edited reconciles silently
                // (no task); everything else offered is a real generation.
                if source_changed || smelly {
                    n += 1;
                }
            }
        }
    }

    let file_needs_writing = match existing
        .as_ref()
        .filter(|spec| !spec.file.spec_hash.is_empty())
    {
        None => true, // the file's own `## Summary` was never generated
        Some(spec) => {
            !file_changes(graph, file_id, &spec.file).is_empty()
                || !prose_smells(&spec.file_summary).is_empty()
        }
    };
    if file_needs_writing {
        n += 1;
    }
    Ok(n)
}

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

fn feature_status(graph: &Graph, root: &Path, entry: &EntryPoint) -> Result<(String, Vec<String>)> {
    let Some(spec) = read_feature_spec(root, &entry.id)? else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let Some(fm) = feature_model_for(graph) else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let participants = assemble_participants(graph, fm, entry);
    let current = current_participant_hashes(graph, &participants)?;
    let status = if diff_hash_lists(&current, &spec.participants).is_empty() {
        "current"
    } else {
        "stale"
    }
    .to_string();
    Ok((status, body_smells(&spec.body)))
}

fn system_status(graph: &Graph, root: &Path) -> Result<(String, Vec<String>)> {
    let Some(spec) = read_system_spec(root)? else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let smells = body_smells(&spec.body);
    let mut current_all = current_module_hashes(graph, root)?;
    current_all.extend(current_feature_hashes(graph, root)?);
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
pub fn coverage(graph: &Graph, root: &Path, scope: Option<&str>) -> Result<Vec<CoverageItem>> {
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
            generations: file_pending_generations(graph, root, id)?,
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
            generations: usize::from(status != "current"),
            status,
            fan_in: 0,
            smells,
            id: format!("rollup:{dir}"),
            kind: "rollup".to_string(),
        });
    }

    if scope.is_none() {
        for entry in enumerate_entry_points(graph) {
            let (status, smells) = feature_status(graph, root, &entry)?;
            items.push(CoverageItem {
                generations: usize::from(status != "current"),
                status,
                fan_in: 0,
                smells,
                id: format!("feature:{}", entry.id),
                kind: "feature".to_string(),
            });
        }
        let (status, smells) = system_status(graph, root)?;
        items.push(CoverageItem {
            generations: usize::from(status != "current"),
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
    /// Total `get_next_spec_task` → `submit_spec` cycles a full
    /// `/codeowl generate --all` run against this scope would spend — the
    /// sum of every item's [`CoverageItem::generations`]. This is the
    /// `--budget=N` a complete generation needs (a file with 20 uncovered
    /// symbols is 21, not 1).
    pub generations_remaining: usize,
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
        summary.generations_remaining += item.generations;
    }
    summary
}

impl CoverageSummary {
    /// `current + stale + missing` — every item this summary covers.
    /// Deliberately not a stored field: it's trivially derived, and storing
    /// it would just be one more thing `summarize` could get out of sync.
    pub fn total(&self) -> usize {
        self.current + self.stale + self.missing
    }

    /// Of the specs that *exist* (current or stale), what fraction still
    /// match the code — deliberately excludes `missing`: "no spec yet" and
    /// "spec exists but is wrong" are different failure modes needing
    /// different fixes (write one vs. regenerate one), and collapsing them
    /// into a single score hides which one a repo actually has. `1.0` when
    /// nothing has ever been generated — vacuously fresh, since there's
    /// nothing stale to report yet (see `coverage_ratio` for the other
    /// half of that same repo's story).
    pub fn freshness(&self) -> f64 {
        let documented = self.current + self.stale;
        if documented == 0 {
            1.0
        } else {
            self.current as f64 / documented as f64
        }
    }

    /// What fraction of eligible nodes have *any* spec at all, current or
    /// stale — the other axis from `freshness`. A repo can be 100% covered
    /// and 60% fresh (everything's been written once, a lot of it needs
    /// re-running) or 60% covered and 100% fresh (nothing documented is
    /// wrong, there's just more left to write) — two different problems
    /// with two different fixes, which is why this is a separate number
    /// rather than folded into `freshness`.
    pub fn coverage_ratio(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            1.0
        } else {
            (self.current + self.stale) as f64 / total as f64
        }
    }
}

/// `freshness`, but weighted by blast radius (import fan-in) instead of by
/// item count — a stale leaf utility matters far less than a stale file
/// forty others import. Scoped to `kind == "file"` items with a spec that
/// actually exists (`status != "missing"`), matching `freshness`'s own
/// "missing is coverage's problem, not freshness's" exclusion — a
/// high-fan-in file that simply has no spec yet shouldn't drag this number
/// down, that's what `coverage_ratio` is for. Falls back to the plain,
/// unweighted freshness over that same subset when every item's `fan_in` is
/// 0 (nothing to weight by, and 0/0 would otherwise need its own case).
pub fn weighted_freshness(items: &[CoverageItem]) -> f64 {
    let documented: Vec<&CoverageItem> = items
        .iter()
        .filter(|i| i.kind == "file" && i.status != "missing")
        .collect();
    let total_fan_in: usize = documented.iter().map(|i| i.fan_in).sum();
    if total_fan_in == 0 {
        if documented.is_empty() {
            return 1.0;
        }
        let current = documented.iter().filter(|i| i.status == "current").count();
        return current as f64 / documented.len() as f64;
    }
    let weighted_current: usize = documented
        .iter()
        .filter(|i| i.status == "current")
        .map(|i| i.fan_in)
        .sum();
    weighted_current as f64 / total_fan_in as f64
}

/// One spec document on disk that no longer corresponds to anything in the
/// current graph — a deleted file, a directory that dropped below the
/// rollup threshold, or a removed feature entry point whose spec still
/// sits under `docs/specs/`. Distinct from "stale": stale means the target
/// still exists and its inputs moved, so there's something to regenerate
/// against; an orphan's target is simply gone, so there's nothing left to
/// regenerate — it's dead weight to flag for deletion, not a generation
/// task. See `find_orphaned_specs`.
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

/// The `n` file-kind items most worth regenerating, ranked by blast radius
/// (`fan_in`) rather than by `prioritize`'s generate-order tiering — "which
/// stale documents would hurt the most if left wrong" instead of "which
/// order should a budgeted run spend on." Same "needs attention" criterion
/// as `pending` (non-current, or current but smelly); only `kind == "file"`
/// carries real fan-in (see `CoverageItem::fan_in`), so rollups/features/
/// system never appear here even if stale.
pub fn top_stale_by_impact(items: &[CoverageItem], n: usize) -> Vec<CoverageItem> {
    let mut candidates: Vec<CoverageItem> = items
        .iter()
        .filter(|i| i.kind == "file" && (i.status != "current" || !i.smells.is_empty()))
        .cloned()
        .collect();
    candidates.sort_by_key(|i| std::cmp::Reverse(i.fan_in));
    candidates.truncate(n);
    candidates
}

/// The canonical kind order a caller should render `by_kind` in — matches
/// `prioritize`'s own file/rollup/feature/system tiering, so "coverage
/// broken down by kind" reads in the same order a generate run would work
/// through them.
const KIND_ORDER: [&str; 4] = ["file", "rollup", "feature", "system"];

/// `coverage()`'s items, grouped by `kind` and each group summarized with
/// the same `summarize` every other rollup uses — so "how many feature
/// specs total" is `by_kind` finding `"feature"` and reading its `total()`,
/// not a separate count computed a second way. Kinds with no items are
/// omitted rather than reported as an all-zero row (a scoped `coverage()`
/// call never produces `"feature"`/`"system"` items at all).
pub fn by_kind(items: &[CoverageItem]) -> Vec<(String, CoverageSummary)> {
    KIND_ORDER
        .iter()
        .filter_map(|kind| {
            let matching: Vec<CoverageItem> =
                items.iter().filter(|i| i.kind == *kind).cloned().collect();
            if matching.is_empty() {
                None
            } else {
                Some((kind.to_string(), summarize(&matching)))
            }
        })
        .collect()
}

/// The directory `item` should be bucketed under for `by_module`, or `None`
/// if it doesn't belong to one (a feature or the system spec is a repo-wide
/// concept, not a directory's). A file's module is its own containing
/// directory; a rollup's module is the directory it *summarizes* (not that
/// directory's parent) — so `rollup:lib/email` lands in the same bucket as
/// `lib/email/foo.ts`, which is what "what's left in lib/email" actually
/// means. The repo root is `"."`, never an empty string.
fn module_of(item: &CoverageItem) -> Option<String> {
    let dir = match item.kind.as_str() {
        "file" => Path::new(&item.id).parent()?.to_string_lossy().into_owned(),
        "rollup" => item.id.strip_prefix("rollup:")?.to_string(),
        _ => return None,
    };
    Some(if dir.is_empty() { ".".to_string() } else { dir })
}

/// `coverage()`'s items, grouped by directory (see `module_of`) and each
/// group summarized with `summarize` — the per-directory breakdown
/// `ARCHITECTURE.md` already (prematurely) claimed `get_spec_coverage` had.
/// Sorted by path so the result is deterministic and reads top-down like a
/// file tree.
pub fn by_module(items: &[CoverageItem]) -> Vec<(String, CoverageSummary)> {
    let mut buckets: Vec<(String, Vec<CoverageItem>)> = Vec::new();
    for item in items {
        let Some(module) = module_of(item) else {
            continue;
        };
        match buckets.iter_mut().find(|(m, _)| *m == module) {
            Some((_, v)) => v.push(item.clone()),
            None => buckets.push((module, vec![item.clone()])),
        }
    }
    buckets.sort_by(|a, b| a.0.cmp(&b.0));
    buckets
        .into_iter()
        .map(|(m, v)| (m, summarize(&v)))
        .collect()
}

/// The "shared infrastructure" tier that gets documented before features
/// is the `SHARED_CODE_MAX_FILES` files with the highest import fan-in
/// across the whole repo (only counting those imported at least
/// `SHARED_CODE_FAN_IN` times, and only [`FileRole::Domain`] files — a UI
/// primitive or generated file is excluded however high its fan-in). It's
/// an *absolute* set anchored to the whole repo, not the top-N of
/// whatever's left to generate — otherwise a budgeted `--all` run keeps
/// promoting the next batch into tier 0 and never reaches a feature.
/// Laptop-scale heuristics, not tuned values.
const SHARED_CODE_FAN_IN: usize = 3;
const SHARED_CODE_MAX_FILES: usize = 8;

/// `coverage`'s items that still need attention, ordered the way
/// `/codeowl generate --all`/`--budget=N` should spend a limited budget
/// (see `ARCHITECTURE.md`'s "Generation priority"):
///
/// 1. the shared-code tier — the top `SHARED_CODE_MAX_FILES` files by
///    import fan-in (`fan_in >= SHARED_CODE_FAN_IN`), whose real summaries
///    upgrade every dependent spec's context for free
/// 2. feature specs — the BA-facing payoff, now written with real
///    dependency summaries rather than stubs
/// 3. the long tail of files (everything not in tier 1)
/// 4. directory rollups (need their files first)
/// 5. test code (`is_test_path`) — file specs only, always after the
///    product, whatever its fan-in
/// 6. the system spec — the capstone, only writable once everything it
///    composes from is current, so always last
///
/// Within a tier, an honestly `"missing"` or `"stale"` document outranks a
/// merely smelly-but-`"current"` one, then descending fan-in, then `id`
/// for a stable, reproducible order.
///
/// Includes a `"current"` item when it has a quality smell — hash-based
/// staleness only verifies a spec's *inputs* haven't moved, never that
/// its prose was ever meaningful (a "see the source" stub, once written,
/// stays hash-`"current"` forever unless something else flags it). Never
/// filtering on smells too would mean a `--all --budget=N` run could run
/// to completion while silently leaving known-bad content in place.
pub fn prioritize(items: Vec<CoverageItem>, graph: &Graph) -> Vec<CoverageItem> {
    // The fan-in floor for the shared-code tier: the higher of
    // SHARED_CODE_FAN_IN and the Nth-highest fan-in among *all* candidate
    // files in the repo — computed from `items` before the `pending`
    // filter, on purpose. Using `pending` instead would recompute the
    // cutoff every run as the top files get generated, quietly promoting
    // the next batch into tier 0 forever; anchoring it to the whole repo
    // means once the real top-N are current, tier 0 is empty and features
    // lead. (Ties at the cutoff can nudge the count slightly over N.)
    let shared_cutoff = {
        let mut fan_ins: Vec<usize> = items
            .iter()
            .filter(|i| i.kind == "file" && classify_in(graph, &i.id) == FileRole::Domain)
            .map(|i| i.fan_in)
            .filter(|&f| f >= SHARED_CODE_FAN_IN)
            .collect();
        fan_ins.sort_unstable_by(|a, b| b.cmp(a));
        fan_ins
            .get(SHARED_CODE_MAX_FILES - 1)
            .copied()
            .unwrap_or(SHARED_CODE_FAN_IN)
            .max(SHARED_CODE_FAN_IN)
    };

    let mut pending: Vec<CoverageItem> = items
        .into_iter()
        .filter(|i| i.status != "current" || !i.smells.is_empty())
        .collect();

    pending.sort_by(|a, b| {
        let tier = |item: &CoverageItem| -> u8 {
            if item.kind == "file" {
                return match classify_in(graph, &item.id) {
                    FileRole::Test => 5,
                    FileRole::Domain if item.fan_in >= shared_cutoff => 0,
                    // Below the fan-in cutoff, or a primitive file at any
                    // fan-in: the long tail, after features.
                    FileRole::Domain | FileRole::Primitive => 2,
                    // Unreachable in practice, kept as its own arm rather
                    // than folded into the tier above (code review,
                    // 2026-09-20): `file_is_spec_bearing` (M19 commit 2)
                    // excludes every `FileRole::Generated` file before it
                    // can ever reach the coverage inventory this sort
                    // runs over, so a reader must not infer from this
                    // match that a generated file is still prioritized.
                    FileRole::Generated => 2,
                };
            }
            match item.kind.as_str() {
                "feature" => 1,
                "rollup" => 3,
                "system" => 6,
                _ => 7,
            }
        };
        fn urgency(status: &str) -> u8 {
            match status {
                "missing" => 0,
                "stale" => 1,
                _ => 2, // "current" but smelly -- the only other way in
            }
        }
        tier(a)
            .cmp(&tier(b))
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
    fn a_generated_file_is_never_spec_bearing_even_with_exported_symbols() {
        // M19: FileRole::Generated must exclude a file from spec
        // generation regardless of what it exports -- a real
        // build-generated JAX-RS interface (quarkus-openapi-generator-
        // server output) has plenty of exported methods but must stay
        // reference-only, per the "only spec the hand-written code"
        // principle. `classify_in` reads the graph's own `pack_name`, so
        // this sets it to "java" directly rather than re-deriving it
        // through `lang::detect` on a throwaway fixture repo.
        let path = "target/generated-sources/foo/HeroesResource.java";
        let mut graph =
            build_graph_from_sources(&[(path, "export function getAllHeroes(): void {}\n")]);
        graph.set_pack_name("java");
        let file_id = graph.find(path).unwrap();
        assert!(!file_is_spec_bearing(&graph, file_id));
    }

    #[test]
    fn generated_sources_summary_counts_java_generated_files_only() {
        let mut graph = build_graph_from_sources(&[
            (
                "src/main/java/org/acme/App.java",
                "export function ignored(): void {}\n",
            ),
            (
                "target/generated-sources/foo/HeroesResource.java",
                "export function getAllHeroes(): void {}\n",
            ),
        ]);
        graph.set_pack_name("java");

        let summary = generated_sources_summary(&graph).expect("Java declares generated dirs");
        assert_eq!(
            summary.checked_dirs,
            vec!["target/generated-sources", "build/generated"]
        );
        assert_eq!(
            summary.found, 1,
            "only the file under a generated dir counts"
        );
    }

    #[test]
    fn generated_sources_summary_is_none_for_a_pack_with_no_generated_dir_convention() {
        let graph = build_graph_from_sources(&[("a.ts", "export function f(): void {}\n")]);
        assert_eq!(generated_sources_summary(&graph), None);
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
    fn depends_on_collapses_unresolved_externals_into_one_line() {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-externals",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // Two unresolved externals (`react`, `next/navigation`) plus one
        // resolved sibling — the external ones must fold into a single
        // `externals: next, react` line, not a line each.
        let a = "import { useState } from 'react';\nimport { redirect } from 'next/navigation';\nimport { helper } from './helper';\n\nexport function widget() {\n  useState();\n  redirect('/x');\n  return helper();\n}\n";
        std::fs::write(dir.join("a.ts"), a).unwrap();
        std::fs::write(
            dir.join("helper.ts"),
            "export function helper(): number { return 1; }\n",
        )
        .unwrap();

        let mut graph = Graph::build(vec![
            crate::graph::extract_and_hash("a.ts", a),
            crate::graph::extract_and_hash(
                "helper.ts",
                "export function helper(): number { return 1; }\n",
            ),
        ]);
        graph.set_resolved_imports(
            crate::imports::extract_imports(a, "a.ts")
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
                .collect(),
        );

        let file_id = graph.find("a.ts").unwrap();
        let widget = graph
            .get_symbol(graph.find("a.ts::widget").unwrap())
            .unwrap();
        let deps = dependency_lines(&graph, &dir, file_id, widget);

        assert_eq!(
            deps.len(),
            2,
            "one resolved line + one externals line, got {deps:?}"
        );
        assert!(deps[0].contains("helper.ts::helper"));
        assert_eq!(deps[1], "externals: next, react");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn symbol_span_text_includes_a_folded_impl_s_method_bodies() {
        // M15: a Rust type's inherent `impl` is folded into the type
        // symbol, but the method bodies sit outside the `struct`'s own line
        // span. `symbol_span_text` must still surface them — otherwise a
        // dep in a method (here `anyhow::Context`) is invisible to
        // `### Depends on` and the generation task never sees the method.
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-span", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let src = "\
use anyhow::Context;\n\n\
pub struct Counter {\n    n: u64,\n}\n\n\
impl Counter {\n\
    pub fn bump(&mut self) -> anyhow::Result<()> {\n\
        self.n += 1;\n        Ok(()).context(\"never\")\n    }\n\
}\n";
        std::fs::write(dir.join("src/counter.rs"), src).unwrap();

        let graph = Graph::build(vec![crate::graph::FileExtraction {
            rel_path: "src/counter.rs".to_string(),
            source_hash: hash_text(src),
            symbols: crate::rust::extract_file(src, "src/counter.rs"),
        }]);
        let counter = graph
            .get_symbol(graph.find("src/counter.rs::Counter").unwrap())
            .unwrap();

        let text = symbol_span_text(&dir, &graph, "src/counter.rs", counter).unwrap();
        assert!(
            text.contains("self.n += 1"),
            "folded method body missing from span text:\n{text}"
        );
        assert!(text.contains("pub struct Counter"), "struct decl missing");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn maybe_reduce_container_source_replaces_bodies_with_signatures_over_the_threshold() {
        // M18: the God-class fix. A synthetic large class -- one
        // documented method padded well past `LARGE_CONTAINER_BYTES_DEFAULT`
        // -- must come back as signature + docstring only, its body gone.
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-godclass", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let padding = "x".repeat(LARGE_CONTAINER_BYTES_DEFAULT + 1000);
        let src = format!(
            "public class Big {{\n\
             \x20   /** Adds one to a running total. */\n\
             \x20   public int bump(int n) {{\n\
             \x20       // {padding}\n\
             \x20       return n + 1;\n\
             \x20   }}\n\
             }}\n"
        );
        std::fs::write(dir.join("src/Big.java"), &src).unwrap();

        let graph = Graph::build(vec![crate::graph::FileExtraction {
            rel_path: "src/Big.java".to_string(),
            source_hash: hash_text(&src),
            symbols: crate::java::extract_file(&src, "src/Big.java"),
        }]);
        let big = graph
            .get_symbol(graph.find("src/Big.java::Big").unwrap())
            .unwrap();

        let full = symbol_span_text(&dir, &graph, "src/Big.java", big).unwrap();
        assert!(
            full.len() > LARGE_CONTAINER_BYTES_DEFAULT,
            "fixture must actually clear the threshold"
        );

        let reduced =
            maybe_reduce_container_source(big, &graph, full.clone(), LARGE_CONTAINER_BYTES_DEFAULT);
        assert!(
            reduced.contains("Adds one to a running total"),
            "docstring must survive:\n{reduced}"
        );
        assert!(
            reduced.contains("public int bump(int n)"),
            "signature must survive:\n{reduced}"
        );
        assert!(
            !reduced.contains("return n + 1"),
            "body must be dropped:\n{reduced}"
        );
        assert!(
            reduced.len() < full.len(),
            "reduced text must actually be smaller"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn maybe_reduce_container_source_leaves_an_ordinary_class_untouched() {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-smallclass",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let src = "public class Small {\n    public int bump(int n) { return n + 1; }\n}\n";
        std::fs::write(dir.join("src/Small.java"), src).unwrap();

        let graph = Graph::build(vec![crate::graph::FileExtraction {
            rel_path: "src/Small.java".to_string(),
            source_hash: hash_text(src),
            symbols: crate::java::extract_file(src, "src/Small.java"),
        }]);
        let small = graph
            .get_symbol(graph.find("src/Small.java::Small").unwrap())
            .unwrap();

        let full = symbol_span_text(&dir, &graph, "src/Small.java", small).unwrap();
        let reduced = maybe_reduce_container_source(
            small,
            &graph,
            full.clone(),
            LARGE_CONTAINER_BYTES_DEFAULT,
        );
        assert_eq!(
            reduced, full,
            "an ordinary-sized class is returned unchanged"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cap_generation_text_leaves_short_text_untouched() {
        let text = "hello world".to_string();
        assert_eq!(
            cap_generation_text(text.clone(), MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT),
            text
        );
    }

    #[test]
    fn cap_generation_text_truncates_and_marks_it_visibly() {
        let text = "x".repeat(MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT + 500);
        let capped = cap_generation_text(text, MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT);
        assert!(
            capped.len() < MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT + 500,
            "must actually be shorter"
        );
        assert!(
            capped.contains("truncated"),
            "must visibly mark that it was cut, not silently drop content: {capped}"
        );
    }

    #[test]
    fn cap_generation_text_never_splits_a_multi_byte_char() {
        // A run of 3-byte UTF-8 characters (☃, U+2603) straddling the cut
        // point -- a naive byte-index slice would panic mid-character.
        let text = "☃".repeat(MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT); // well over the byte cap
        let capped = cap_generation_text(text, MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT); // must not panic
        assert!(capped.len() <= MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT);
    }

    #[test]
    fn cap_generation_text_honors_max_bytes_including_the_marker_itself() {
        // Code-review finding: the function truncated the *text* to
        // max_bytes, then unconditionally appended the truncation marker
        // on top -- so the real returned length was max_bytes + the
        // marker's own ~145 bytes, contradicting the doc comment's "full
        // stop" guarantee. The exact scenario this whole fix exists for
        // (a real MCP client's ceiling sitting close to the configured
        // cap) means that overrun matters, not just in theory.
        let text = "x".repeat(1000);
        let capped = cap_generation_text(text, 50);
        assert!(
            capped.len() <= 50,
            "the total returned length, marker included, must never exceed max_bytes: {} bytes",
            capped.len()
        );
        assert!(
            capped.contains("truncated"),
            "still must visibly mark truncation even under a tight budget: {capped}"
        );
    }

    #[test]
    fn cap_generation_text_survives_a_budget_smaller_than_the_marker_itself() {
        // An unreasonably tight cap (smaller than the ~145-byte marker
        // text) -- must not panic or underflow, and the guarantee still
        // holds: whatever comes back never exceeds max_bytes.
        let text = "x".repeat(1000);
        let capped = cap_generation_text(text, 10);
        assert!(capped.len() <= 10, "must still fit: {} bytes", capped.len());
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
    fn next_task_is_none_for_a_generated_file_with_a_real_uncovered_symbol() {
        // M19: get_next_spec_task targeting a generated file directly
        // must report nothing to do, even though it has a genuinely
        // uncovered exported symbol -- the same "kind: done" outcome a
        // barrel file gets, for a different reason (reference-only, not
        // "nothing to say").
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-gen", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let path = "target/generated-sources/foo/HeroesResource.java";
        let mut graph =
            build_graph_from_sources(&[(path, "export function getAllHeroes(): void {}\n")]);
        graph.set_pack_name("java");
        let file_id = graph.find(path).unwrap();

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
    fn submit_rejects_prose_that_next_task_would_re_offer_forever() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-smelly", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let graph = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        let file_id = graph.find("a.ts").unwrap();

        // A Summary under the 4-word `prose_smells` floor — storing it would
        // make the next `next_task` call re-offer `a.ts::one` indefinitely.
        let err = submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nToo terse.\n\
             ### Behavior\nThis behavior sentence is comfortably long enough to pass.\n",
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("quality check"), "{msg}");
        assert!(msg.contains("Summary: suspiciously_short"), "{msg}");

        // A "see the source" cop-out is rejected even at full length.
        let err = submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nReturns the one canonical value for this module.\n\
             ### Behavior\nIt does the obvious thing; see the source for the exact steps.\n",
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("Behavior: cop_out_phrase"),
            "{err}"
        );

        // Nothing was persisted: it's still a first-ever generation task.
        assert!(matches!(
            next_task(&graph, &dir, file_id).unwrap(),
            Some(SpecTask::Symbol { ref id, prior: None, .. }) if id == "a.ts::one"
        ));

        // Real prose is accepted and the loop then terminates.
        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nReturns the one canonical value this module exposes.\n\
             ### Behavior\nReads a cached slot, initialising it on first call, never mutating after.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "a.ts",
            "The single-value helper module used across the app.",
        )
        .unwrap();
        assert_eq!(next_task(&graph, &dir, file_id).unwrap(), None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn submit_rejects_a_smelly_file_summary() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-smellyf", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let graph = build_graph_from_sources(&[("a.ts", "export function one(): void {}\n")]);
        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nReturns the one canonical value this module exposes.\n\
             ### Behavior\nReads a cached slot, initialising it on first call, never mutating after.\n",
        )
        .unwrap();
        let err = submit(&graph, &dir, "a.ts", "Three short words.").unwrap_err();
        assert!(err.to_string().contains("suspiciously_short"), "{err}");
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
            "### Summary\nThis is the original summary.\n### Behavior\nThis is the original behavior.\n",
        )
        .unwrap();

        // A human hand-edits the spec file's prose directly -- never
        // through submit_spec -- leaving the frontmatter hashes as they
        // were (a human wouldn't hand-update an opaque blake3 hash).
        let path = spec_path(&dir, "a.ts");
        let content = std::fs::read_to_string(&path).unwrap();
        let edited = content.replace(
            "This is the original summary.",
            "A careful human-corrected summary here.",
        );
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
        assert_eq!(
            reread.sections[0].1.summary,
            "A careful human-corrected summary here."
        );
        let expected_hash =
            hash_text("A careful human-corrected summary here.\nThis is the original behavior.");
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
            "### Summary\nThis is the original summary.\n### Behavior\nThis is the original behavior.\n",
        )
        .unwrap();

        let path = spec_path(&dir, "a.ts");
        let content = std::fs::read_to_string(&path).unwrap();
        let edited = content.replace(
            "This is the original summary.",
            "A careful human-corrected summary here.",
        );
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
                prior: Some(SymbolProse {
                    summary: "A careful human-corrected summary here.".to_string(),
                    behavior: "This is the original behavior.".to_string(),
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
        // A hash-current spec whose Behavior is a "see the source" cop-out
        // (the shape a pre-fix cache carries — `submit` rejects it now).
        plant_smelly_symbol_spec(
            &graph,
            &dir,
            "a.ts::one",
            "One does its one small job.",
            "See the source for details.",
        );

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
                prior: None,
            })
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Write a small fixture repo to a fresh temp dir and build the *whole*
    /// graph off it — imports resolved, route literals / table refs /
    /// rendered components extracted and set — exactly the pipeline
    /// `RepoIndex::rebuild` runs. Feature-spec tests need the real thing
    /// because they read files off disk and traverse the flow edges.
    fn build_feature_fixture(files: &[(&str, &str)], suffix: &str) -> (Graph, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-feature-spec-test-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for (rel, content) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
        }
        let graph = crate::index::RepoIndex::build(&dir)
            .unwrap()
            .rebuild()
            .unwrap();
        (graph, dir)
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
        // Real bug found running this against CodeOwl's own repo:
        // docs/specs/STYLE.md is a hand-written style guide, not a
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
            body: "# Acme\n## Summary\nA competition platform.".to_string(),
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

    #[test]
    fn coverage_reports_everything_missing_then_everything_current() {
        let (graph, dir) = build_feature_fixture(SYSTEM_FIXTURE, "coverage1");

        let items = coverage(&graph, &dir, None).unwrap();
        let summary = summarize(&items);
        assert_eq!(
            summary.missing, 8,
            "5 files + 1 rollup + 1 feature + 1 system"
        );
        assert_eq!(summary.current, 0);
        assert_eq!(summary.stale, 0);

        let pending = prioritize(items, &graph);
        let ids: Vec<&str> = pending.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "feature:submit",
                "lib/supabase.ts",
                "app/api/submit-artwork/route.ts",
                "app/submit/page.tsx",
                "lib/one.ts",
                "lib/two.ts",
                "rollup:lib",
                "system",
            ],
            "no file here clears the shared-code fan-in bar, so: features, \
             then files by descending fan-in (lib/supabase.ts is imported \
             by both page.tsx and route.ts), then rollups, then the system \
             spec last"
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
            "submit",
            "# Artwork submission\n## Summary\nLets an artist submit artwork for judging.\n",
        )
        .unwrap();
        submit_system(
            &graph,
            &dir,
            "# Acme\n## Summary\nA platform for running art competitions.\n",
        )
        .unwrap();

        let items = coverage(&graph, &dir, None).unwrap();
        let summary = summarize(&items);
        assert_eq!(summary.current, 8);
        assert_eq!(summary.stale, 0);
        assert_eq!(summary.missing, 0);
        assert_eq!(summary.generations_remaining, 0);
        assert!(prioritize(items, &graph).is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn coverage_never_counts_a_generated_file_as_missing_or_pending() {
        // M19: a generated interface with real exported methods must not
        // inflate get_spec_coverage's missing count or appear in its
        // pending worklist -- it's reference-only, never spec-bearing,
        // regardless of how many methods it exports.
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-gen2", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let path = "target/generated-sources/foo/HeroesResource.java";
        let mut graph = build_graph_from_sources(&[(
            path,
            "export function getAllHeroes(): void {}\n\
             export function getRandomHero(): void {}\n",
        )]);
        graph.set_pack_name("java");

        let items = coverage(&graph, &dir, None).unwrap();
        let pending = prioritize(items, &graph);
        assert!(
            !pending.iter().any(|i| i.kind == "file"),
            "the generated file must never appear as a pending file document: {pending:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn coverage_counts_generations_not_just_documents() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-gens", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = "export function one(): void {}\n\
                   export function two(): void {}\n\
                   export function three(): void {}\n";
        std::fs::write(dir.join("a.ts"), src).unwrap();
        let graph = build_graph_from_sources(&[("a.ts", src)]);

        // Nothing generated: one `pending` document (a.ts), but four
        // generations — three symbols + the file's own summary.
        let a = |items: &[CoverageItem]| items.iter().find(|i| i.id == "a.ts").unwrap().generations;
        assert_eq!(a(&coverage(&graph, &dir, None).unwrap()), 4);

        // Each symbol spec drops the count by one.
        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nDoes the first job.\n### Behavior\nRuns with no side effects.\n",
        )
        .unwrap();
        assert_eq!(a(&coverage(&graph, &dir, None).unwrap()), 3);

        for id in ["a.ts::two", "a.ts::three"] {
            submit(
                &graph,
                &dir,
                id,
                "### Summary\nDoes a second deliberate thing.\n### Behavior\nRuns with no side effects.\n",
            )
            .unwrap();
        }
        // Symbols done, only the file summary left.
        assert_eq!(a(&coverage(&graph, &dir, None).unwrap()), 1);

        submit(
            &graph,
            &dir,
            "a.ts",
            "Three small deliberate no-op helpers.",
        )
        .unwrap();
        let items = coverage(&graph, &dir, None).unwrap();
        assert_eq!(a(&items), 0);
        // The system spec is still missing, so the repo-wide total is 1,
        // and it equals the sum of every item's own share.
        let summary = summarize(&items);
        assert_eq!(summary.generations_remaining, 1);
        assert_eq!(
            summary.generations_remaining,
            items.iter().map(|i| i.generations).sum::<usize>()
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_stack_with_no_feature_model_composes_a_system_spec_from_modules_alone() {
        // M14 / design decision 3: `RustStack` has no feature layer, so
        // `coverage` must report zero features and the system-spec
        // composition must not panic or emit an empty `## Features`.
        let (graph, dir) = build_feature_fixture(
            &[
                ("src/lib.rs", "pub mod parse;\npub mod store;\n"),
                (
                    "src/parse.rs",
                    "//! Parsing.\npub fn tokenize(_s: &str) -> Vec<String> { Vec::new() }\npub fn lex() {}\n",
                ),
                (
                    "src/store.rs",
                    "//! Storage.\nuse crate::parse::tokenize;\npub fn save(_s: &str) { tokenize(_s); }\npub fn load() {}\n",
                ),
            ],
            "rust-nofeat",
        );
        assert_eq!(graph.pack_name(), "rust");
        assert!(feature_model_for(&graph).is_none());
        assert!(current_feature_hashes(&graph, &dir).unwrap().is_empty());

        let items = coverage(&graph, &dir, None).unwrap();
        assert!(
            !items.iter().any(|i| i.kind == "feature"),
            "a no-feature stack must produce no feature coverage items"
        );
        assert!(items.iter().any(|i| i.id == "system"));

        // The system task is generable and its `features` list is empty.
        // (Modules/rollups come first; drain them, then ask for system.)
        let sys = next_system_task(&graph, &dir).unwrap();
        if let Some(task) = sys {
            assert!(task.features.is_empty(), "no features to compose over");
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn coverage_scope_narrows_to_files_and_rollups_under_that_prefix() {
        let (graph, dir) = build_feature_fixture(SYSTEM_FIXTURE, "coverage2");
        let items = coverage(&graph, &dir, Some("lib")).unwrap();
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
    fn a_file_whose_summary_was_never_written_is_not_smelly_for_being_short() {
        // Mid-generation state: `submit_spec` on a symbol lazily creates a
        // `FileSpec` with a blank summary (via `FileSpec::blank`). An empty
        // summary that was never generated must not read as a
        // `suspiciously_short` cop-out — nothing was written to smell.
        let graph = build_graph_from_sources(&[(
            "a.ts",
            "export function one(): void {}\nexport function two(): void {}\n",
        )]);
        let file_id = graph.find("a.ts").unwrap();
        let spec = FileSpec {
            source_path: "a.ts".to_string(),
            file: HashPair::default(), // spec_hash empty -> file summary never written
            symbols: vec![("a.ts::one".to_string(), HashPair::default())],
            file_summary: String::new(),
            sections: vec![(
                "a.ts::one".to_string(),
                SymbolProse {
                    summary: "Does the first specific thing this file needs.".to_string(),
                    behavior: "Runs synchronously with no side effects at all.".to_string(),
                },
            )],
        };
        assert!(file_spec_smells(&graph, Path::new("/nonexistent"), file_id, &spec).is_empty());

        // But once the summary *has* been written (spec_hash set), a short
        // one is still flagged.
        let written = FileSpec {
            file: HashPair {
                spec_hash: hash_text(""),
                ..HashPair::default()
            },
            file_summary: "Two helpers.".to_string(),
            ..spec
        };
        assert_eq!(
            file_spec_smells(&graph, Path::new("/nonexistent"), file_id, &written),
            vec!["suspiciously_short"]
        );
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
        let (graph, dir) = build_feature_fixture(SYSTEM_FIXTURE, "coverage3");
        // A hash-current symbol spec with a cop-out Behavior — makes the
        // whole file spec smelly without anything having moved.
        plant_smelly_symbol_spec(
            &graph,
            &dir,
            "lib/supabase.ts::getSupabase",
            "Builds and returns the shared Supabase client.",
            "See the source for details.",
        );
        submit(
            &graph,
            &dir,
            "lib/supabase.ts",
            "A thin wrapper module around the Supabase browser SDK.",
        )
        .unwrap();

        let items = coverage(&graph, &dir, Some("lib/supabase.ts")).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "current");
        assert!(!items[0].smells.is_empty());

        let summary = summarize(&items);
        assert_eq!(summary.current, 1);
        assert_eq!(summary.smelly, 1);

        let pending = prioritize(items, &graph);
        assert_eq!(
            pending.len(),
            1,
            "a current-but-smelly document must still show up as pending"
        );
        assert_eq!(pending[0].status, "current");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prioritize_puts_shared_code_first_features_next_system_last() {
        fn item(id: &str, kind: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: "missing".into(),
                fan_in,
                smells: Vec::new(),
                generations: 0,
            }
        }
        let items = vec![
            item("system", "system", 0),
            item("feature:checkout", "feature", 0),
            item("lib/db.ts", "file", 12), // shared infra — many importers
            item("app/page.tsx", "file", 0), // a leaf
            item("rollup:lib", "rollup", 0),
        ];
        let graph = crate::graph::build_graph_from_sources(&[]);
        let ids: Vec<String> = prioritize(items, &graph)
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(
            ids,
            vec![
                "lib/db.ts",        // high-fan-in files first: their real
                "feature:checkout", // summaries upgrade downstream specs
                "app/page.tsx",     // then the long tail of leaf files
                "rollup:lib",       // then rollups
                "system",           // the system spec is always last
            ]
        );
    }

    #[test]
    fn prioritize_sinks_test_code_below_the_product_whatever_its_fan_in() {
        fn item(id: &str, kind: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: "missing".into(),
                fan_in,
                smells: Vec::new(),
                generations: 0,
            }
        }
        let items = vec![
            item("e2e/helpers/api-client.ts", "file", 71), // huge fan-in, but test code
            item("lib/db.ts", "file", 4),
            item("feature:checkout", "feature", 0),
            item("app/dashboard/card.test.tsx", "file", 0),
            item("system", "system", 0),
        ];
        let graph = crate::graph::build_graph_from_sources(&[]);
        let ids: Vec<String> = prioritize(items, &graph)
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(
            ids,
            vec![
                "lib/db.ts",                   // real shared code
                "feature:checkout",            // features
                "e2e/helpers/api-client.ts",   // test code — after the product,
                "app/dashboard/card.test.tsx", // fan-in only orders within the tier
                "system",                      // system still dead last
            ]
        );
    }

    #[test]
    fn by_kind_groups_and_summarizes_per_kind_in_canonical_order() {
        fn item(id: &str, kind: &str, status: &str) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: status.into(),
                fan_in: 0,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        let items = vec![
            item("system", "system", "stale"),
            item("feature:checkout", "feature", "missing"),
            item("rollup:lib", "rollup", "current"),
            item("lib/db.ts", "file", "current"),
            item("lib/utils.ts", "file", "stale"),
        ];
        let buckets = by_kind(&items);
        let kinds: Vec<&str> = buckets.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            kinds,
            vec!["file", "rollup", "feature", "system"],
            "canonical order regardless of input order"
        );

        let file_summary = &buckets.iter().find(|(k, _)| k == "file").unwrap().1;
        assert_eq!(file_summary.current, 1);
        assert_eq!(file_summary.stale, 1);
        assert_eq!(file_summary.total(), 2);
    }

    #[test]
    fn by_kind_omits_kinds_with_no_items() {
        fn item(id: &str, kind: &str) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: "missing".into(),
                fan_in: 0,
                smells: Vec::new(),
                generations: 1,
            }
        }
        // A scoped coverage() call never produces feature/system items.
        let items = vec![item("lib/db.ts", "file")];
        let kinds: Vec<String> = by_kind(&items).into_iter().map(|(k, _)| k).collect();
        assert_eq!(kinds, vec!["file".to_string()]);
    }

    #[test]
    fn by_module_groups_files_and_their_rollup_by_directory() {
        fn item(id: &str, kind: &str, status: &str) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: status.into(),
                fan_in: 0,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        let items = vec![
            item("lib/email/foo.ts", "file", "current"),
            item("lib/email/bar.ts", "file", "stale"),
            item("rollup:lib/email", "rollup", "missing"),
            item("index.ts", "file", "current"),
            item("feature:checkout", "feature", "missing"),
            item("system", "system", "missing"),
        ];
        let buckets = by_module(&items);
        let paths: Vec<&str> = buckets.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            vec![".", "lib/email"],
            "sorted by path; features/system have no module and are excluded"
        );

        let email = &buckets.iter().find(|(p, _)| p == "lib/email").unwrap().1;
        assert_eq!(
            email.total(),
            3,
            "the two files plus their own directory's rollup, bucketed together"
        );
        assert_eq!(email.current, 1);
        assert_eq!(email.stale, 1);
        assert_eq!(email.missing, 1);
    }

    #[test]
    fn freshness_ignores_missing_entirely() {
        // 1 current, 1 stale, 5 missing -- freshness is about the specs
        // that exist, not about how much of the repo is documented yet.
        let summary = CoverageSummary {
            current: 1,
            stale: 1,
            missing: 5,
            ..CoverageSummary::default()
        };
        assert_eq!(summary.freshness(), 0.5);
    }

    #[test]
    fn freshness_is_vacuously_full_when_nothing_documented_yet() {
        let summary = CoverageSummary {
            missing: 10,
            ..CoverageSummary::default()
        };
        assert_eq!(
            summary.freshness(),
            1.0,
            "nothing stale to report -- undocumented is coverage's problem, not freshness's"
        );
    }

    #[test]
    fn coverage_ratio_counts_stale_as_covered() {
        // A stale spec still means "something was generated for this" --
        // coverage and freshness are deliberately different axes.
        let summary = CoverageSummary {
            current: 1,
            stale: 1,
            missing: 2,
            ..CoverageSummary::default()
        };
        assert_eq!(summary.coverage_ratio(), 0.5);
    }

    #[test]
    fn coverage_ratio_is_vacuously_full_when_nothing_is_eligible() {
        assert_eq!(CoverageSummary::default().coverage_ratio(), 1.0);
    }

    #[test]
    fn weighted_freshness_weights_by_fan_in_not_item_count() {
        fn item(id: &str, status: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: "file".into(),
                status: status.into(),
                fan_in,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        // One stale file with huge fan-in, nine current leaf files with
        // none -- by item count this reads 90% fresh; weighted by blast
        // radius, the busy stale file dominates.
        let mut items = vec![item("lib/db.ts", "stale", 40)];
        for i in 0..9 {
            items.push(item(&format!("leaf{i}.ts"), "current", 0));
        }
        assert_eq!(weighted_freshness(&items), 0.0);
    }

    #[test]
    fn weighted_freshness_falls_back_to_unweighted_when_nothing_has_fan_in() {
        fn item(id: &str, status: &str) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: "file".into(),
                status: status.into(),
                fan_in: 0,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        let items = vec![
            item("a.ts", "current"),
            item("b.ts", "current"),
            item("c.ts", "stale"),
        ];
        assert_eq!(weighted_freshness(&items), 2.0 / 3.0);
    }

    #[test]
    fn weighted_freshness_ignores_missing_and_non_file_items() {
        fn item(id: &str, kind: &str, status: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: status.into(),
                fan_in,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        let items = vec![
            item("a.ts", "file", "current", 5),
            item("b.ts", "file", "missing", 99), // no spec yet -- excluded
            item("rollup:lib", "rollup", "stale", 0), // not a file -- excluded
        ];
        assert_eq!(
            weighted_freshness(&items),
            1.0,
            "only a.ts is an eligible (file, non-missing) item, and it's current"
        );
    }

    #[test]
    fn top_stale_by_impact_ranks_file_items_by_fan_in_descending() {
        fn item(id: &str, status: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: "file".into(),
                status: status.into(),
                fan_in,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        let items = vec![
            item("quiet.ts", "stale", 1),
            item("db.ts", "stale", 40),
            item("mid.ts", "missing", 10),
            item("clean.ts", "current", 99), // clean -- not "needing attention"
        ];
        let top = top_stale_by_impact(&items, 5);
        let ids: Vec<&str> = top.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["db.ts", "mid.ts", "quiet.ts"],
            "highest fan-in first; a clean current item never qualifies regardless of fan-in"
        );
    }

    #[test]
    fn top_stale_by_impact_truncates_to_n() {
        fn item(id: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: "file".into(),
                status: "stale".into(),
                fan_in,
                smells: Vec::new(),
                generations: 1,
            }
        }
        let items: Vec<CoverageItem> = (0..10).map(|i| item(&format!("f{i}.ts"), i)).collect();
        assert_eq!(top_stale_by_impact(&items, 3).len(), 3);
        assert_eq!(top_stale_by_impact(&items, 3)[0].id, "f9.ts");
    }

    #[test]
    fn top_stale_by_impact_includes_current_but_smelly_items() {
        let smelly_current = CoverageItem {
            id: "smelly.ts".into(),
            kind: "file".into(),
            status: "current".into(),
            fan_in: 7,
            smells: vec!["cop_out_phrase".to_string()],
            generations: 1,
        };
        let top = top_stale_by_impact(&[smelly_current], 5);
        let ids: Vec<&str> = top.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["smelly.ts"],
            "current but smelly still needs attention, same as pending's own criterion"
        );
    }

    #[test]
    fn top_stale_by_impact_excludes_non_file_kinds() {
        let feature = CoverageItem {
            id: "feature:checkout".into(),
            kind: "feature".into(),
            status: "stale".into(),
            fan_in: 0,
            smells: Vec::new(),
            generations: 1,
        };
        assert!(top_stale_by_impact(&[feature], 5).is_empty());
    }

    #[test]
    fn prioritize_caps_the_shared_code_tier_so_features_are_reachable() {
        fn item(id: &str, kind: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: "missing".into(),
                fan_in,
                smells: Vec::new(),
                generations: 0,
            }
        }
        // 12 files, fan-in 20 down to 9 — all well over SHARED_CODE_FAN_IN.
        let mut items = vec![item("feature:x", "feature", 0)];
        for fan in (9..=20).rev() {
            items.push(item(&format!("lib/f{fan:02}.ts"), "file", fan));
        }
        let graph = crate::graph::build_graph_from_sources(&[]);
        let ids: Vec<String> = prioritize(items, &graph)
            .into_iter()
            .map(|i| i.id)
            .collect();

        // Only the top SHARED_CODE_MAX_FILES (fan-in 20..13) come before
        // the feature; the rest (12..9) fall to the long tail after it.
        assert_eq!(ids[0], "lib/f20.ts");
        assert_eq!(ids[SHARED_CODE_MAX_FILES - 1], "lib/f13.ts");
        assert_eq!(ids[SHARED_CODE_MAX_FILES], "feature:x");
        assert_eq!(ids[SHARED_CODE_MAX_FILES + 1], "lib/f12.ts");
        assert_eq!(ids.last().unwrap(), "lib/f09.ts");
    }

    #[test]
    fn shared_cutoff_is_anchored_to_the_whole_repo_not_whats_left() {
        fn item(id: &str, kind: &str, status: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: status.into(),
                fan_in,
                smells: Vec::new(),
                generations: 0,
            }
        }
        // The repo's 8 highest-fan-in files are already current. What's
        // left is a mid-fan-in file and a feature. The mid file must NOT be
        // treated as shared code just because it's now the top of pending.
        let mut items = vec![
            item("lib/mid.ts", "file", "missing", 9),
            item("feature:x", "feature", "missing", 0),
        ];
        for fan in (12..=19).rev() {
            items.push(item(&format!("lib/hot{fan}.ts"), "file", "current", fan));
        }
        let graph = crate::graph::build_graph_from_sources(&[]);
        let ids: Vec<String> = prioritize(items, &graph)
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(
            ids,
            vec!["feature:x", "lib/mid.ts"],
            "cutoff is set by the 8 current files (fan-in 12), so lib/mid.ts (9) is long tail"
        );
    }

    #[test]
    fn ui_primitives_stay_out_of_the_shared_tier_even_at_high_fan_in() {
        fn item(id: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: "file".into(),
                status: "missing".into(),
                fan_in,
                smells: Vec::new(),
                generations: 0,
            }
        }
        let items = vec![
            item("components/ui/button.tsx", 40), // imported everywhere
            item("lib/db.ts", 12),
            CoverageItem {
                id: "feature:x".into(),
                kind: "feature".into(),
                status: "missing".into(),
                fan_in: 0,
                smells: Vec::new(),
                generations: 0,
            },
        ];
        let graph = crate::graph::build_graph_from_sources(&[]);
        let ids: Vec<String> = prioritize(items, &graph)
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(
            ids,
            vec!["lib/db.ts", "feature:x", "components/ui/button.tsx"]
        );
    }

    #[test]
    fn is_test_path_examples() {
        // An empty graph -> pack_name "" -> the TypeScript pack's classify.
        let g = crate::graph::build_graph_from_sources(&[]);
        assert!(is_test_path(&g, "e2e/helpers/api-client.ts"));
        assert!(is_test_path(&g, "src/components/__tests__/button.ts"));
        assert!(is_test_path(&g, "lib/utils.test.ts"));
        assert!(is_test_path(&g, "app/page.spec.tsx"));
        assert!(is_test_path(&g, "rollup:e2e/helpers"));
        assert!(!is_test_path(&g, "lib/utils.ts"));
        assert!(!is_test_path(&g, "app/api/attest/route.ts")); // "test" substring, not a test file
    }
}
