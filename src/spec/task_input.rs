//! What a file task hands the writer: the file's own text in pages, or a stand-in when it is too big, and the cursor that ties the pages of one read together.

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::format::read_existing;
use super::layout::spec_bearing_children;
use super::source_text::{container_outline, symbol_span_text};
use crate::graph::{Graph, SymbolId};
use crate::hash::hash_text;
use crate::symbol::SymbolKind;
use crate::text::leading_comment_block;

/// How many pages of raw file text a writer reads before the file task
/// switches to a stand-in for the file. Starting value, not measured past
/// 12 pages (`experiments/exp-05-file-summary-input.md`); overridable per
/// server (`codeowl serve --max-file-pages`), like the byte limits above,
/// since the right cutoff depends on the MCP client and the model.
pub const FILE_RAW_MAX_PAGES_DEFAULT: usize = 12;

/// The fewest spec-bearing symbols a too-big file needs before its file
/// task is built from the symbols' written summaries instead of raw text
/// (`codeowl serve --summaries-min-symbols`). A guess, to be revisited
/// after a first real run.
pub const FILE_SUMMARIES_MIN_SYMBOLS_DEFAULT: usize = 8;

/// The share of a too-big file, in percent of its bytes, that one
/// container must cover for the file task to be a member outline of that
/// container (`codeowl serve --outline-container-percent`). A guess, to be
/// revisited after a first real run.
pub const FILE_OUTLINE_CONTAINER_PERCENT_DEFAULT: usize = 70;

/// The three numbers that decide what a file task hands the writer. One
/// struct so the choice can be a pure function of its inputs and tests can
/// pass any values; `mcp.rs::CodeOwlServer::with_file_ladder` sets it from
/// `codeowl serve`'s flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileLadder {
    pub max_raw_pages: usize,
    pub summaries_min_symbols: usize,
    pub outline_container_percent: usize,
}

impl Default for FileLadder {
    fn default() -> Self {
        Self {
            max_raw_pages: FILE_RAW_MAX_PAGES_DEFAULT,
            summaries_min_symbols: FILE_SUMMARIES_MIN_SYMBOLS_DEFAULT,
            outline_container_percent: FILE_OUTLINE_CONTAINER_PERCENT_DEFAULT,
        }
    }
}

/// Split `text` into pages of at most `page_bytes` bytes that concatenate
/// back to exactly `text`. A page ends after a newline when one fits in the
/// budget; a single line longer than the budget is cut on a `char`
/// boundary. Always makes progress: a budget smaller than one character
/// still yields one character per page. Empty text is one empty page, so a
/// caller can always say "page 1 of N".
pub(crate) fn page_text(text: &str, page_bytes: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut pages = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if rest.len() <= page_bytes {
            pages.push(rest.to_string());
            break;
        }
        let mut cut = page_bytes;
        while cut > 0 && !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        if let Some(newline) = rest[..cut].rfind('\n') {
            cut = newline + 1;
        } else if cut == 0 {
            // Budget smaller than the first character: take that one char.
            cut = rest.chars().next().map_or(rest.len(), char::len_utf8);
        }
        pages.push(rest[..cut].to_string());
        rest = &rest[cut..];
    }
    pages
}

/// First 12 hex characters of the blake3 hash of the file id and the full
/// paged input; the same shortening `dep_targets` uses. The id is part of it
/// so two files with identical text never honour each other's cursors.
fn input_fingerprint(file_id: &str, input: &str) -> String {
    hash_text(&format!("{file_id}\n{input}"))[..12].to_string()
}

/// The cursor for page `page` (1-based) of `input`, the paged text of
/// `file_id`: `"<page>:<hash12>"`. Stateless: the hash names the file and
/// the input the pages were cut from, so a cursor issued before the input
/// changed is refused instead of stitching pages from two versions of a file.
pub(crate) fn encode_page_cursor(page: usize, file_id: &str, input: &str) -> String {
    format!("{page}:{}", input_fingerprint(file_id, input))
}

/// The page number a cursor names, checked against the current `input` and
/// its page count. Errors say what to do, since the reader is an agent.
pub(crate) fn decode_page_cursor(
    cursor: &str,
    file_id: &str,
    input: &str,
    pages: usize,
) -> Result<usize> {
    let malformed =
        || anyhow::anyhow!("malformed cursor {cursor:?}: pass back a next_cursor exactly as given");
    let (page, hash) = cursor.split_once(':').ok_or_else(malformed)?;
    let page: usize = page.parse().map_err(|_| malformed())?;
    if page == 0 {
        return Err(malformed());
    }
    if hash != input_fingerprint(file_id, input) {
        bail!(
            "this cursor does not match the file's current text (it changed, or the cursor is for another file): restart from page 1 by calling again without a cursor"
        );
    }
    if page > pages {
        bail!(
            "cursor names page {page} but the file has {pages} page(s): restart from page 1 by calling again without a cursor"
        );
    }
    Ok(page)
}

/// What a file task's text is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRung {
    /// The file's own text.
    Raw,
    /// Each spec-bearing symbol's signature and written summary.
    Summaries,
    /// One dominant container's signature, docstring and member signatures.
    Outline,
}

/// Which input a file task is built from. A pure function of the facts and
/// the configured numbers, so the boundaries can be tested directly.
/// `pages` is the raw text's page count; `container_percent` is the largest
/// container's share of the file's bytes (0 when it has none). A file
/// within the page ceiling is always raw; beyond it, a dominant container
/// gets the outline (checked first, since a class-sized file has one
/// top-level symbol), then enough symbols get their summaries, and
/// anything else stays raw, paged but complete.
/// Whether a file of `pages` raw pages is read raw: the one place the page
/// ceiling is compared, so `choose_file_rung` and the cheap early exit in
/// `file_task_input` cannot drift apart.
fn within_raw_ceiling(pages: usize, ladder: &FileLadder) -> bool {
    pages <= ladder.max_raw_pages
}

pub fn choose_file_rung(
    pages: usize,
    spec_bearing_symbols: usize,
    container_percent: usize,
    ladder: &FileLadder,
) -> FileRung {
    if within_raw_ceiling(pages, ladder) {
        FileRung::Raw
    } else if container_percent >= ladder.outline_container_percent {
        FileRung::Outline
    } else if spec_bearing_symbols >= ladder.summaries_min_symbols {
        FileRung::Summaries
    } else {
        FileRung::Raw
    }
}

/// The file's largest container (class, struct, …) and its share of
/// `file_bytes`, in percent. `None` when the file has no container. The
/// span includes folded-in `impl` blocks, the same text a symbol task shows.
pub(crate) fn largest_container_percent(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    file_bytes: usize,
) -> Result<Option<(SymbolId, usize)>> {
    let file = graph
        .get_file(file_id)
        .context("container share asked of a non-file id")?;
    let mut best: Option<(SymbolId, usize)> = None;
    for sym_id in spec_bearing_children(graph, file_id) {
        let sym = graph.get_symbol(sym_id).expect("filtered to symbol ids");
        if sym.kind != SymbolKind::Container {
            continue;
        }
        let bytes = symbol_span_text(root, graph, &file.id, sym)?.len();
        if best.is_none_or(|(_, b)| bytes > b) {
            best = Some((sym_id, bytes));
        }
    }
    Ok(best.map(|(id, bytes)| {
        // An empty file has no share (checked_div is None on a zero divisor).
        let percent = (bytes * 100).checked_div(file_bytes).unwrap_or(0).min(100);
        (id, percent)
    }))
}

/// `FILE:` and `MODULE DOC:` lines that open both stand-in inputs.
fn stand_in_header(file_id: &str, text: &str) -> String {
    let doc = leading_comment_block(text);
    let doc = if doc.is_empty() {
        "(none)"
    } else {
        doc.as_str()
    };
    format!("FILE: {file_id}\nMODULE DOC:\n{doc}\n")
}

/// The summaries input: the file's top comment, then each spec-bearing
/// symbol in file order as its signature followed by its written
/// `### Summary`. Never the file's own summary (the writer would copy it)
/// and never the `### Behavior` text. `None` when any symbol has no written
/// summary, so a partial input is never presented as the whole file.
pub(crate) fn summaries_input(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    text: &str,
) -> Result<Option<String>> {
    let file = graph
        .get_file(file_id)
        .context("summaries input asked of a non-file id")?;
    let Some(existing) = read_existing(root, &file.id)? else {
        return Ok(None);
    };
    let symbols = spec_bearing_children(graph, file_id);
    let mut entries = Vec::with_capacity(symbols.len());
    for sym_id in &symbols {
        let sym = graph.get_symbol(*sym_id).expect("filtered to symbol ids");
        let Some(prose) = existing
            .section(&sym.id)
            .filter(|p| !p.summary.trim().is_empty())
        else {
            return Ok(None);
        };
        let summary = prose
            .summary
            .trim()
            .lines()
            .map(|l| format!("  {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        entries.push(format!("{}\n{summary}", sym.signature));
    }
    Ok(Some(format!(
        "{}SYMBOLS ({}), in file order, each as: signature, then its written summary:\n{}\n{}",
        stand_in_header(&file.id, text),
        entries.len(),
        entries.join("\n"),
        other_declarations(graph, file_id, &symbols)
    )))
}

/// The file's top-level declarations that a stand-in input does not already
/// cover (types, enums, constants, or everything but the outlined class),
/// as bare signatures, so the writer still sees the whole file's shape.
/// Empty when there are none.
fn other_declarations(graph: &Graph, file_id: SymbolId, covered: &[SymbolId]) -> String {
    let signatures: Vec<&str> = graph
        .children_ids(file_id)
        .iter()
        .filter(|id| !covered.contains(id))
        .filter_map(|&id| graph.get_symbol(id))
        .map(|sym| sym.signature.as_str())
        .collect();
    if signatures.is_empty() {
        return String::new();
    }
    format!(
        "\nOTHER TOP-LEVEL DECLARATIONS (signatures only):\n{}\n",
        signatures.join("\n")
    )
}

/// The outline input: the file's top comment, then the dominant container's
/// signature, docstring and member signatures and docstrings, no bodies,
/// then every other top-level declaration by signature.
pub(crate) fn outline_input(
    graph: &Graph,
    file_id: SymbolId,
    container: SymbolId,
    text: &str,
) -> String {
    let file = graph.get_file(file_id).expect("outline asked of a file id");
    let sym = graph
        .get_symbol(container)
        .expect("outline asked of a container symbol");
    format!(
        "{}CLASS OUTLINE, in file order, signatures and doc comments only:\n{}\n{}",
        stand_in_header(&file.id, text),
        container_outline(sym, graph),
        other_declarations(graph, file_id, &[container])
    )
}

/// What a file task hands the writer: the chosen rung, the input text, and
/// that text already cut into pages.
pub(crate) struct FileTaskInput {
    pub rung: FileRung,
    pub text: String,
    pub pages: Vec<String>,
}

/// What a file task hands the writer for `text` (the file's text as the
/// writer would read it, test code already left out): the chosen rung and
/// its input, paged. A summaries input that cannot be built falls back to
/// raw. A file within the raw ceiling returns before any span is read.
pub(crate) fn file_task_input(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    text: &str,
    page_bytes: usize,
    ladder: &FileLadder,
) -> Result<FileTaskInput> {
    let raw_pages = page_text(text, page_bytes);
    let raw = |pages: Vec<String>| FileTaskInput {
        rung: FileRung::Raw,
        text: text.to_string(),
        pages,
    };
    if within_raw_ceiling(raw_pages.len(), ladder) {
        return Ok(raw(raw_pages));
    }
    let container = largest_container_percent(graph, root, file_id, text.len())?;
    let rung = choose_file_rung(
        raw_pages.len(),
        spec_bearing_children(graph, file_id).len(),
        container.map_or(0, |(_, percent)| percent),
        ladder,
    );
    let stand_in = match (rung, container) {
        (FileRung::Outline, Some((sym_id, _))) => Some((
            FileRung::Outline,
            outline_input(graph, file_id, sym_id, text),
        )),
        (FileRung::Summaries, _) => {
            summaries_input(graph, root, file_id, text)?.map(|input| (FileRung::Summaries, input))
        }
        _ => None,
    };
    Ok(match stand_in {
        Some((rung, input)) => FileTaskInput {
            rung,
            pages: page_text(&input, page_bytes),
            text: input,
        },
        None => raw(raw_pages),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_graph_from_sources;

    use crate::spec::test_support::{BIG_JAVA, java_graph};

    // ---- the file-task ladder: which input, and how each is built ----

    fn ladder(pages: usize, symbols: usize, percent: usize) -> FileLadder {
        FileLadder {
            max_raw_pages: pages,
            summaries_min_symbols: symbols,
            outline_container_percent: percent,
        }
    }

    // ---- the file-task ladder: paging, cursor, configuration ----

    #[test]
    fn page_text_pages_concatenate_back_to_the_exact_text() {
        let text = "line one\nline two\nline three\nline four\nline five\n";
        let pages = page_text(text, 20);
        assert!(pages.len() > 1, "fixture must need more than one page");
        assert_eq!(pages.concat(), text);
        for page in &pages {
            assert!(page.len() <= 20, "page over the byte budget: {page:?}");
        }
    }

    #[test]
    fn page_text_cuts_on_a_line_boundary_when_one_exists() {
        let text = "aaaa\nbbbb\ncccc\ndddd\n";
        // 12 bytes fits two whole lines (10 bytes) but not a third.
        let pages = page_text(text, 12);
        assert_eq!(pages, vec!["aaaa\nbbbb\n", "cccc\ndddd\n"]);
    }

    #[test]
    fn page_text_cuts_an_over_long_line_on_a_char_boundary() {
        let text = "x".repeat(25);
        let pages = page_text(&text, 10);
        assert_eq!(
            pages.iter().map(String::len).collect::<Vec<_>>(),
            [10, 10, 5]
        );
        assert_eq!(pages.concat(), text);
    }

    #[test]
    fn page_text_never_splits_a_multi_byte_char() {
        // Each snowman is 3 bytes; a 10-byte page can hold 3 of them.
        let text = "☃".repeat(10);
        let pages = page_text(&text, 10);
        assert_eq!(pages.concat(), text);
        for page in &pages {
            assert!(page.len() <= 10);
            assert!(page.chars().all(|c| c == '☃'));
        }
    }

    #[test]
    fn page_text_always_makes_progress_on_a_tiny_budget() {
        // A budget smaller than one char must still terminate, one char per page.
        let pages = page_text("☃☃", 1);
        assert_eq!(pages, vec!["☃", "☃"]);
    }

    #[test]
    fn page_text_of_empty_text_is_one_empty_page() {
        assert_eq!(page_text("", 100), vec![String::new()]);
    }

    #[test]
    fn a_page_cursor_round_trips_and_names_the_input_it_belongs_to() {
        let cursor = encode_page_cursor(3, "a.ts", "some full input text");
        assert_eq!(
            decode_page_cursor(&cursor, "a.ts", "some full input text", 5).unwrap(),
            3
        );
    }

    #[test]
    fn a_page_cursor_for_changed_input_asks_for_a_restart() {
        let cursor = encode_page_cursor(2, "a.ts", "the text as first read");
        let err = decode_page_cursor(&cursor, "a.ts", "the text after an edit", 5).unwrap_err();
        assert!(
            err.to_string().contains("page 1"),
            "must tell the writer to restart from page 1: {err}"
        );
    }

    #[test]
    fn a_page_cursor_is_bound_to_the_file_it_was_issued_for() {
        // Two files with identical text must not honour each other's cursors.
        let cursor = encode_page_cursor(2, "a.ts", "identical boilerplate");
        let err = decode_page_cursor(&cursor, "b.ts", "identical boilerplate", 5).unwrap_err();
        assert!(err.to_string().contains("page 1"), "{err}");
    }

    #[test]
    fn a_malformed_or_out_of_range_page_cursor_is_rejected() {
        let good = encode_page_cursor(2, "a.ts", "text");
        for bad in ["", "nonsense", "2", "x:abc", ":abc", "0:abc"] {
            assert!(
                decode_page_cursor(bad, "a.ts", "text", 5).is_err(),
                "{bad:?}"
            );
        }
        // Valid shape, but past the last page.
        assert!(decode_page_cursor(&good, "a.ts", "text", 1).is_err());
    }

    #[test]
    fn file_ladder_defaults_are_the_named_constants() {
        let ladder = FileLadder::default();
        assert_eq!(ladder.max_raw_pages, FILE_RAW_MAX_PAGES_DEFAULT);
        assert_eq!(
            ladder.summaries_min_symbols,
            FILE_SUMMARIES_MIN_SYMBOLS_DEFAULT
        );
        assert_eq!(
            ladder.outline_container_percent,
            FILE_OUTLINE_CONTAINER_PERCENT_DEFAULT
        );
        // The owner-approved starting values.
        assert_eq!(
            (
                FILE_RAW_MAX_PAGES_DEFAULT,
                FILE_SUMMARIES_MIN_SYMBOLS_DEFAULT,
                FILE_OUTLINE_CONTAINER_PERCENT_DEFAULT
            ),
            (12, 8, 70)
        );
    }

    #[test]
    fn a_file_within_the_page_ceiling_is_always_raw() {
        let l = ladder(12, 8, 70);
        // Even with plenty of symbols and a dominant container.
        assert_eq!(choose_file_rung(12, 50, 100, &l), FileRung::Raw);
        assert_eq!(choose_file_rung(1, 50, 100, &l), FileRung::Raw);
    }

    #[test]
    fn the_summaries_threshold_is_exactly_the_configured_symbol_count() {
        let l = ladder(12, 8, 70);
        assert_eq!(choose_file_rung(13, 7, 10, &l), FileRung::Raw);
        assert_eq!(choose_file_rung(13, 8, 10, &l), FileRung::Summaries);
    }

    #[test]
    fn the_outline_threshold_is_exactly_the_configured_container_share() {
        let l = ladder(12, 8, 70);
        assert_eq!(choose_file_rung(13, 1, 69, &l), FileRung::Raw);
        assert_eq!(choose_file_rung(13, 1, 70, &l), FileRung::Outline);
    }

    #[test]
    fn the_outline_wins_when_both_summaries_and_outline_qualify() {
        let l = ladder(12, 8, 70);
        assert_eq!(choose_file_rung(13, 20, 90, &l), FileRung::Outline);
    }

    #[test]
    fn a_non_default_ladder_moves_every_boundary() {
        let l = ladder(3, 2, 50);
        assert_eq!(choose_file_rung(3, 5, 80, &l), FileRung::Raw);
        assert_eq!(choose_file_rung(4, 1, 49, &l), FileRung::Raw);
        assert_eq!(choose_file_rung(4, 2, 49, &l), FileRung::Summaries);
        assert_eq!(choose_file_rung(4, 1, 50, &l), FileRung::Outline);
    }

    #[test]
    fn largest_container_percent_is_the_class_share_of_the_file() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-pct", std::process::id()));
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::write(dir.join("a/Big.java"), BIG_JAVA).unwrap();
        let graph = java_graph(BIG_JAVA);
        let file_id = graph.find("a/Big.java").unwrap();

        let (container, percent) = largest_container_percent(&graph, &dir, file_id, BIG_JAVA.len())
            .unwrap()
            .expect("the file has a container");
        assert_eq!(graph.get_symbol(container).unwrap().id, "a/Big.java::Big");
        assert!(
            (70..=100).contains(&percent),
            "the class is nearly the whole file, got {percent}"
        );
        // A file with no container has no share at all.
        let ts = build_graph_from_sources(&[("f.ts", "export function f(): void {}\n")]);
        let ts_id = ts.find("f.ts").unwrap();
        assert_eq!(
            largest_container_percent(&ts, &dir, ts_id, 30).unwrap(),
            None
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn outline_input_also_lists_every_other_top_level_declaration() {
        let src = "export class Big {\n  a(): void {}\n}\nexport function helper(): void {}\nexport const LIMIT_VALUE = 5;\n";
        let graph = build_graph_from_sources(&[("big.ts", src)]);
        let file_id = graph.find("big.ts").unwrap();
        let container = graph.find("big.ts::Big").unwrap();

        let input = outline_input(&graph, file_id, container, src);
        let (outline, others) = input
            .split_once("OTHER TOP-LEVEL DECLARATIONS")
            .unwrap_or_else(|| panic!("missing the other-declarations section: {input}"));
        assert!(outline.contains("class Big"));
        assert!(
            others.contains("helper") && others.contains("LIMIT_VALUE"),
            "{others}"
        );
        assert!(
            !others.contains("class Big"),
            "the outlined class is not repeated"
        );
    }

    #[test]
    fn a_file_with_nothing_else_has_no_other_declarations_section() {
        let src = "export class Big {\n  a(): void {}\n}\n";
        let graph = build_graph_from_sources(&[("big.ts", src)]);
        let input = outline_input(
            &graph,
            graph.find("big.ts").unwrap(),
            graph.find("big.ts::Big").unwrap(),
            src,
        );
        assert!(!input.contains("OTHER TOP-LEVEL DECLARATIONS"), "{input}");
    }
}
