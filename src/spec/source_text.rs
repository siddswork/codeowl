//! The source text a generation task shows: a symbol's true span, the size caps with their defaults, and the reduced view of a large container. Pure functions of the graph; nothing here reads or writes a spec.

use std::path::Path;

use anyhow::{Context, Result};

use crate::graph::{Graph, SymbolId};
use crate::symbol::{Symbol, SymbolKind};

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

/// `self_id`, when `Some`, excludes any resolved import whose `target` is
/// that very symbol — real for M21.g's synthetic same-file impl -> type
/// edge (`rust.rs::same_file_trait_impl_edges`): that edge's
/// `imported_name` is the *type's* own name, and a type's own declaration
/// always contains its own name, so without this exclusion the type's own
/// scan would match its own synthetic edge and appear to depend on
/// itself. No resolved import can ever legitimately target the symbol
/// whose text is being scoped, so this exclusion is always safe, not
/// specific to Rust or to this one edge kind.
pub(crate) fn scoped_symbol_deps(
    graph: &Graph,
    from_file: &str,
    symbol_text: &str,
    self_id: Option<SymbolId>,
) -> ScopedDeps {
    let mut resolved = Vec::new();
    let mut externals: Vec<String> = Vec::new();
    for imp in graph.imports().iter().filter(|imp| {
        imp.from_file == from_file
            && contains_identifier(symbol_text, &imp.imported_name)
            && imp.target != self_id
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

/// The line spans that scope a symbol's dependencies and feed its
/// generation task: `sym`'s own line span, plus the span of any direct
/// child that lies *outside* it. For a TS class every method already sits
/// inside the class braces, so this is just the class body; for a Rust
/// type whose inherent `impl` blocks were folded in (M15,
/// `rust::merge_inherent_impls`), the method spans live elsewhere in the
/// file and are appended. Adjacent/overlapping spans are merged so the
/// common case (a `struct` immediately followed by its `impl`) still reads
/// as one contiguous block. Sorted by start line, so callers can rely on
/// `.first()`/`.last()` for the outer envelope.
///
/// Split out from `symbol_span_text` (M20) so `get_source` (`mcp.rs`) can
/// share the same span computation — and widen just the first/last span
/// by its own `context_lines` — without duplicating this logic or its
/// gap-merging rule.
pub(crate) fn merged_symbol_spans(graph: &Graph, sym: &Symbol) -> Vec<[usize; 2]> {
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
    spans.extend(sym.extra_spans.iter().copied());
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
    merged
}

/// Read `spans` (1-based, inclusive line ranges) out of already-read file
/// `content`, joining non-adjacent blocks with a blank line — the actual
/// slicing logic `symbol_span_text` and `get_source` (M20) both need once
/// they have the file's current text in hand. Split out so a caller that
/// also needs the raw content for another reason (`get_source`'s
/// `graph_in_sync` hash check) reads the file once, not twice.
pub(crate) fn render_spans(content: &str, spans: &[[usize; 2]]) -> String {
    let all: Vec<&str> = content.lines().collect();
    let mut out = String::new();
    for (i, [start, end]) in spans.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        let lo = start.saturating_sub(1).min(all.len());
        let hi = (*end).min(all.len());
        out.push_str(&all[lo..hi].join("\n"));
    }
    out
}

/// The raw source text that scopes a symbol's dependencies and feeds its
/// generation task — `merged_symbol_spans` read off disk and joined. See
/// that function's doc comment for what "scope" means here.
pub(crate) fn symbol_span_text(
    root: &Path,
    graph: &Graph,
    rel_path: &str,
    sym: &Symbol,
) -> Result<String> {
    let merged = merged_symbol_spans(graph, sym);
    let content = std::fs::read_to_string(root.join(rel_path))
        .with_context(|| format!("reading {rel_path}"))?;
    Ok(render_spans(&content, &merged))
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

/// `get_source`'s cap — deliberately larger than
/// [`MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT`], since the two serve
/// different purposes. That cap exists to keep a *generation task* small
/// enough for an LLM to write prose against; this one exists so a
/// *verification read* — an agent checking a claim about real behavior,
/// `get_source`'s whole reason to exist (see `ARCHITECTURE.md` §7) —
/// gets the real text rather than a prompt-sized fragment of it.
/// Overridable the same way as the generation-task caps (`codeowl
/// serve`'s `--max-source-bytes` flag, `mcp.rs::with_source_limit`).
pub const MAX_SOURCE_TEXT_BYTES_DEFAULT: usize = 32_000;

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
    container_outline(sym, graph)
}

/// A container's signature and docstring, then each member's signature and
/// docstring in declaration order, with no bodies: the reduced display of a
/// god-class task, and the stand-in for a file that is mostly one class.
pub(crate) fn container_outline(sym: &Symbol, graph: &Graph) -> String {
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
pub(super) fn contains_identifier(text: &str, name: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::spec::test_support::{BIG_JAVA, java_graph};

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
    fn container_outline_is_signature_docstring_then_each_member_without_bodies() {
        let graph = java_graph(BIG_JAVA);
        let big = graph
            .get_symbol(graph.find("a/Big.java::Big").unwrap())
            .unwrap();
        let outline = container_outline(big, &graph);
        assert!(outline.starts_with("public class Big"), "{outline}");
        assert!(outline.contains("The big class."));
        let bump = outline.find("int bump").expect("bump member");
        let twice = outline.find("int twice").expect("twice member");
        assert!(bump < twice, "member order: {outline}");
        assert!(outline.contains("Adds one.") && outline.contains("Doubles it."));
        assert!(!outline.contains("BODY-ONE") && !outline.contains("BODY-TWO"));
    }
}
