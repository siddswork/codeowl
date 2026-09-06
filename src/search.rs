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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SearchMatch {
    pub file: String,
    pub line: u64,
    pub text: String,
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
                out.push(SearchMatch {
                    file: rel.clone(),
                    line,
                    text: text.trim_end().to_string(),
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
}
