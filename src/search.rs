//! `search_code`'s Phase 1 implementation: embedded ripgrep (the `grep`
//! crate), not a real index. See `ARCHITECTURE.md`'s "Storage" — the
//! `tantivy` and embeddings search index is deferred to Phase 2, since a
//! repo this size doesn't need one to search fast enough to be useful.

use std::path::Path;

use anyhow::{Context, Result};
use grep::regex::RegexMatcher;
use grep::searcher::{Searcher, sinks::UTF8};
use serde::Serialize;

/// Cap on returned matches — MCP responses shouldn't balloon on a query
/// that happens to match half the repo.
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SearchMatch {
    pub file: String,
    pub line: u64,
    pub text: String,
    /// `true` when `text` was cut short of the real line's end because it
    /// exceeded [`MAX_MATCH_TEXT_BYTES`]. `false` means `text` is the
    /// whole matched line, trimmed only of trailing whitespace.
    pub truncated: bool,
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
/// the extraction walk) for `query`, returning at most `MAX_RESULTS`
/// matches in walk order.
pub fn search_code(root: &Path, query: &str) -> Result<Vec<SearchMatch>> {
    let matcher = RegexMatcher::new(query).with_context(|| format!("invalid pattern: {query}"))?;
    let mut searcher = Searcher::new();
    let mut out = Vec::new();

    for entry in ignore::WalkBuilder::new(root).build() {
        if out.len() >= MAX_RESULTS {
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

        // A single unreadable or binary file shouldn't abort the whole
        // search — skip it and keep going.
        let _ = searcher.search_path(
            &matcher,
            path,
            UTF8(|line, text| {
                let (text, truncated) = truncate_match_text(text.trim_end());
                out.push(SearchMatch {
                    file: rel.clone(),
                    line,
                    text,
                    truncated,
                });
                Ok(out.len() < MAX_RESULTS)
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

    #[test]
    fn finds_a_known_string_with_line_number() {
        let dir = std::env::temp_dir().join(format!("codeowl-search-test-{}", std::process::id()));
        write_fixture(&dir, "a.ts", "line one\nfunction target() {}\nline three\n");

        let matches = search_code(&dir, "target").unwrap();

        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file, "a.ts");
        assert_eq!(matches[0].line, 2);
        assert!(matches[0].text.contains("target"));
    }

    #[test]
    fn no_match_returns_empty_not_an_error() {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-search-test-nomatch-{}",
            std::process::id()
        ));
        write_fixture(&dir, "a.ts", "nothing interesting here\n");

        let matches = search_code(&dir, "nonexistent_pattern_xyz").unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert!(matches.is_empty());
    }

    #[test]
    fn supports_regex_patterns() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-search-test-regex-{}", std::process::id()));
        write_fixture(
            &dir,
            "a.ts",
            "export function foo() {}\nexport const bar = 1;\n",
        );

        let matches = search_code(&dir, r"export (function|const)").unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn a_short_match_is_not_flagged_truncated() {
        let dir = std::env::temp_dir().join(format!(
            "codeowl-search-test-shortline-{}",
            std::process::id()
        ));
        write_fixture(&dir, "a.ts", "line one\nfunction target() {}\nline three\n");

        let matches = search_code(&dir, "target").unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 1);
        assert!(!matches[0].truncated);
    }

    #[test]
    fn an_oversized_line_is_truncated_and_flagged() {
        // M20: the real bug, reproduced directly rather than assumed --
        // search.rs pushed whole untruncated lines into SearchMatch.text,
        // and MAX_RESULTS caps match *count*, not response *size*. A
        // single committed-spec-prose line runs 1-3 KB in this project's
        // own corpus by construction; 5 KB here stands in for that.
        let dir = std::env::temp_dir().join(format!(
            "codeowl-search-test-longline-{}",
            std::process::id()
        ));
        let long_line = format!("needle {}", "x".repeat(5_000));
        write_fixture(&dir, "a.md", &format!("{long_line}\n"));

        let matches = search_code(&dir, "needle").unwrap();

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
        let dir = std::env::temp_dir().join(format!(
            "codeowl-search-test-linenum-{}",
            std::process::id()
        ));
        let long_line = format!("needle {}", "x".repeat(5_000));
        write_fixture(&dir, "a.md", &format!("short line\n{long_line}\n"));

        let matches = search_code(&dir, "needle").unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 2);
    }
}
