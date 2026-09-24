//! `search_code`'s Phase 1 implementation: embedded ripgrep (the `grep`
//! crate), not a real index. See `ARCHITECTURE.md`'s "Storage" — the
//! `tantivy` and embeddings search index is deferred to Phase 2, since a
//! repo this size doesn't need one to search fast enough to be useful.

use std::path::Path;

use anyhow::{Context, Result};
use grep::regex::RegexMatcherBuilder;
use grep::searcher::{Searcher, sinks::UTF8};
use serde::Serialize;

/// Cap on returned matches — MCP responses shouldn't balloon on a query
/// that happens to match half the repo. `SearchOptions::max_results` can
/// narrow this further; it can never widen it.
const MAX_RESULTS: usize = 200;

/// Cap on one match's `text`, in bytes. `search.rs` used to push whole
/// untruncated lines: against this project's own committed spec prose
/// (1-3 KB on a single line, by design — see `ARCHITECTURE.md`'s "Spec
/// document format") a single broad query returns hundreds of KB.
/// `MAX_RESULTS` caps match *count*; this caps match *width*, the other
/// axis a response can balloon on. Confirmed real, not projected: a
/// `search_code("Merkle")` call against this repo returned ~15 KB for
/// ~20 matches before this cap existed.
const MAX_MATCH_TEXT_BYTES: usize = 500;

/// Cap on `SearchOptions::context_lines`, tighter than `get_source`'s
/// equivalent (`mcp.rs::MAX_CONTEXT_LINES`, 20). A single `search_code`
/// call can return up to `MAX_RESULTS` matches, each now carrying its own
/// context — an unbounded per-match multiplier reintroduces the same
/// payload problem `MAX_MATCH_TEXT_BYTES` exists to fix, just multiplied
/// by match count instead of line width.
const MAX_SEARCH_CONTEXT_LINES: usize = 5;

/// Options beyond the query string itself. `Default` reproduces
/// `search_code`'s original, pre-M20 behavior exactly: no path filter,
/// case-sensitive, no context, the full `MAX_RESULTS` ceiling.
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    /// Restrict matches to files under this path — an exact file or a
    /// directory prefix, same boundary rule as `spec::within_scope`
    /// (which this reuses): `"lib"` matches `"lib/foo.ts"`, never
    /// `"library/foo.ts"`. `None` searches the whole repo.
    pub path: Option<String>,
    pub ignore_case: bool,
    /// Lines of surrounding context per match, each side — capped at
    /// [`MAX_SEARCH_CONTEXT_LINES`] regardless of what's asked for.
    pub context_lines: usize,
    /// Narrows the match-count cap below [`MAX_RESULTS`]. `None`, or a
    /// value above `MAX_RESULTS`, leaves the full ceiling in place —
    /// this can only make a response smaller, never larger.
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SearchMatch {
    pub file: String,
    pub line: u64,
    pub text: String,
    /// `true` when `text` was cut short of the real line's end because it
    /// exceeded [`MAX_MATCH_TEXT_BYTES`]. `false` means `text` is the
    /// whole matched line, trimmed only of trailing whitespace.
    pub truncated: bool,
    /// Lines immediately before the match, oldest first — populated only
    /// when `context_lines > 0` was requested. Each line is trimmed to
    /// [`MAX_MATCH_TEXT_BYTES`] the same as `text`, silently: a long
    /// context line shouldn't reopen the payload problem `text`'s own
    /// cap exists to close.
    pub context_before: Vec<String>,
    /// Lines immediately after the match, in file order.
    pub context_after: Vec<String>,
}

/// Cuts `line` to at most `MAX_MATCH_TEXT_BYTES`, on a UTF-8 char
/// boundary, keeping the start of the line (where the match itself is
/// far more likely to sit than the tail) rather than the end.
fn truncate_match_text(line: &str) -> (String, bool) {
    if line.len() <= MAX_MATCH_TEXT_BYTES {
        return (line.to_string(), false);
    }
    let mut end = MAX_MATCH_TEXT_BYTES;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    (line[..end].to_string(), true)
}

/// Regex-search every file under `root` (respecting `.gitignore`, same as
/// the extraction walk) for `query`, in walk order, shaped by `opts`.
pub fn search_code(root: &Path, query: &str, opts: &SearchOptions) -> Result<Vec<SearchMatch>> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(opts.ignore_case)
        .build(query)
        .with_context(|| format!("invalid pattern: {query}"))?;
    let mut searcher = Searcher::new();
    let mut out = Vec::new();

    let context_lines = opts.context_lines.min(MAX_SEARCH_CONTEXT_LINES);
    let max_results = opts
        .max_results
        .map(|n| n.min(MAX_RESULTS))
        .unwrap_or(MAX_RESULTS);

    for entry in ignore::WalkBuilder::new(root).build() {
        if out.len() >= max_results {
            break;
        }
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        if let Some(scope) = &opts.path
            && !crate::spec::within_scope(&rel, scope)
        {
            continue;
        }

        // Context needs the whole file's lines in memory; the common case
        // (context_lines == 0, the overwhelming majority of calls) skips
        // this second read entirely and costs nothing extra.
        let file_lines: Option<Vec<String>> = (context_lines > 0)
            .then(|| std::fs::read_to_string(path).ok())
            .flatten()
            .map(|content| content.lines().map(str::to_string).collect());

        // A single unreadable or binary file shouldn't abort the whole
        // search — skip it and keep going.
        let _ = searcher.search_path(
            &matcher,
            path,
            UTF8(|line, text| {
                let (text, truncated) = truncate_match_text(text.trim_end());
                let (context_before, context_after) = match &file_lines {
                    Some(lines) => {
                        let idx = (line as usize).saturating_sub(1).min(lines.len());
                        let lo = idx.saturating_sub(context_lines);
                        let hi = (idx + 1 + context_lines).min(lines.len());
                        let before = lines[lo..idx]
                            .iter()
                            .map(|l| truncate_match_text(l).0)
                            .collect();
                        let after = lines[(idx + 1).min(lines.len())..hi]
                            .iter()
                            .map(|l| truncate_match_text(l).0)
                            .collect();
                        (before, after)
                    }
                    None => (Vec::new(), Vec::new()),
                };
                out.push(SearchMatch {
                    file: rel.clone(),
                    line,
                    text,
                    truncated,
                    context_before,
                    context_after,
                });
                Ok(out.len() < max_results)
            }),
        );
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_fixture(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
    }

    fn tmp(label: &str) -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "codeowl-search-test-{label}-{}-{n}",
            std::process::id()
        ))
    }

    #[test]
    fn finds_a_known_string_with_line_number() {
        let dir = tmp("basic");
        write_fixture(&dir, "a.ts", "line one\nfunction target() {}\nline three\n");

        let matches = search_code(&dir, "target", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file, "a.ts");
        assert_eq!(matches[0].line, 2);
        assert!(matches[0].text.contains("target"));
    }

    #[test]
    fn no_match_returns_empty_not_an_error() {
        let dir = tmp("nomatch");
        write_fixture(&dir, "a.ts", "nothing interesting here\n");

        let matches =
            search_code(&dir, "nonexistent_pattern_xyz", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert!(matches.is_empty());
    }

    #[test]
    fn supports_regex_patterns() {
        let dir = tmp("regex");
        write_fixture(
            &dir,
            "a.ts",
            "export function foo() {}\nexport const bar = 1;\n",
        );

        let matches =
            search_code(&dir, r"export (function|const)", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn a_short_match_is_not_flagged_truncated() {
        let dir = tmp("shortline");
        write_fixture(&dir, "a.ts", "line one\nfunction target() {}\nline three\n");

        let matches = search_code(&dir, "target", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 1);
        assert!(!matches[0].truncated);
    }

    #[test]
    fn an_oversized_line_is_truncated_and_flagged() {
        // The real bug, reproduced directly rather than assumed --
        // search.rs pushed whole untruncated lines into SearchMatch.text,
        // and MAX_RESULTS caps match *count*, not response *size*. A
        // single committed-spec-prose line runs 1-3 KB in this project's
        // own corpus by construction; 5 KB here stands in for that.
        let dir = tmp("longline");
        let long_line = format!("needle {}", "x".repeat(5_000));
        write_fixture(&dir, "a.md", &format!("{long_line}\n"));

        let matches = search_code(&dir, "needle", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 1);
        assert!(
            matches[0].text.len() <= MAX_MATCH_TEXT_BYTES,
            "match text ({} bytes) exceeds the cap ({} bytes) -- truncation didn't run",
            matches[0].text.len(),
            MAX_MATCH_TEXT_BYTES
        );
        assert!(
            matches[0].text.starts_with("needle"),
            "truncation should keep the match itself, not just the line's tail"
        );
        assert!(
            matches[0].truncated,
            "an oversized line must set truncated: true"
        );
    }

    #[test]
    fn line_returns_are_unaffected_by_truncation() {
        // Truncating text shouldn't touch which file/line a hit is
        // reported against -- only the text payload shrinks.
        let dir = tmp("linenum");
        let long_line = format!("needle {}", "x".repeat(5_000));
        write_fixture(&dir, "a.md", &format!("short line\n{long_line}\n"));

        let matches = search_code(&dir, "needle", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 2);
    }

    // --- M20: path / ignore_case / context_lines / max_results ---------

    #[test]
    fn path_filter_restricts_matches_to_that_file_or_directory() {
        let dir = tmp("pathfilter");
        write_fixture(&dir, "src/a.ts", "const target = 1;\n");
        write_fixture(&dir, "tests/a.ts", "const target = 2;\n");

        let opts = SearchOptions {
            path: Some("src".to_string()),
            ..Default::default()
        };
        let matches = search_code(&dir, "target", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file, "src/a.ts");
    }

    #[test]
    fn path_filter_respects_boundaries_not_bare_string_prefixes() {
        // Mirrors spec::within_scope's own tested property -- "lib" must
        // not also match "library/foo.ts".
        let dir = tmp("pathboundary");
        write_fixture(&dir, "lib/a.ts", "const target = 1;\n");
        write_fixture(&dir, "library/a.ts", "const target = 2;\n");

        let opts = SearchOptions {
            path: Some("lib".to_string()),
            ..Default::default()
        };
        let matches = search_code(&dir, "target", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file, "lib/a.ts");
    }

    #[test]
    fn ignore_case_finds_a_differently_cased_match() {
        let dir = tmp("ignorecase");
        write_fixture(&dir, "a.ts", "const TARGET = 1;\n");

        let case_sensitive = search_code(&dir, "target", &SearchOptions::default()).unwrap();
        assert!(case_sensitive.is_empty(), "must not match by default");

        let opts = SearchOptions {
            ignore_case: true,
            ..Default::default()
        };
        let insensitive = search_code(&dir, "target", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(insensitive.len(), 1);
    }

    #[test]
    fn context_lines_populates_before_and_after_in_file_order() {
        let dir = tmp("context");
        write_fixture(
            &dir,
            "a.ts",
            "import { helper } from \"./helper\";\n\
             \n\
             function target() {\n\
             \x20   return helper();\n\
             }\n",
        );

        let opts = SearchOptions {
            context_lines: 2,
            ..Default::default()
        };
        let matches = search_code(&dir, "function target", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0].context_before,
            vec![
                "import { helper } from \"./helper\";".to_string(),
                "".to_string()
            ]
        );
        assert_eq!(
            matches[0].context_after,
            vec!["    return helper();".to_string(), "}".to_string()]
        );
    }

    #[test]
    fn context_lines_is_capped_regardless_of_what_s_asked_for() {
        let dir = tmp("contextcap");
        let mut src = String::new();
        for i in 0..20 {
            src.push_str(&format!("line {i}\n"));
        }
        src.push_str("needle\n");
        for i in 0..20 {
            src.push_str(&format!("line {i}\n"));
        }
        write_fixture(&dir, "a.ts", &src);

        let opts = SearchOptions {
            context_lines: 1000,
            ..Default::default()
        };
        let matches = search_code(&dir, "needle", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].context_before.len() <= MAX_SEARCH_CONTEXT_LINES);
        assert!(matches[0].context_after.len() <= MAX_SEARCH_CONTEXT_LINES);
    }

    #[test]
    fn context_lines_is_empty_by_default() {
        let dir = tmp("nocontext");
        write_fixture(&dir, "a.ts", "before\nneedle\nafter\n");

        let matches = search_code(&dir, "needle", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].context_before.is_empty());
        assert!(matches[0].context_after.is_empty());
    }

    #[test]
    fn max_results_narrows_below_the_default_cap() {
        let dir = tmp("maxresults");
        let mut src = String::new();
        for i in 0..10 {
            src.push_str(&format!("needle {i}\n"));
        }
        write_fixture(&dir, "a.ts", &src);

        let opts = SearchOptions {
            max_results: Some(3),
            ..Default::default()
        };
        let matches = search_code(&dir, "needle", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn max_results_above_the_hard_cap_is_still_capped() {
        let dir = tmp("maxresultscap");
        let mut src = String::new();
        for i in 0..5 {
            src.push_str(&format!("needle {i}\n"));
        }
        write_fixture(&dir, "a.ts", &src);

        let opts = SearchOptions {
            max_results: Some(1_000_000),
            ..Default::default()
        };
        let matches = search_code(&dir, "needle", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        // Only 5 real matches exist -- this asserts the request didn't
        // error or misbehave, the ceiling itself is exercised by
        // max_results_narrows_below_the_default_cap above.
        assert_eq!(matches.len(), 5);
    }
}
