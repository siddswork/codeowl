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

/// A coarser, whole-response cap, on top of the three per-field caps
/// above. Code-review finding: those three bound one match's *width*
/// (text, context) and the response's *count*, but nothing bounded the
/// *product* — worst case, `MAX_MATCH_TEXT_BYTES` (500) + 2 ×
/// `MAX_SEARCH_CONTEXT_LINES` × 500 (5,500 bytes/match) × `MAX_RESULTS`
/// (200) ≈ 1.1 MB, once `context_lines` is in play. This project has
/// already hit a real client failure at a fraction of that size — see
/// `spec::MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT`'s doc comment (VS Code
/// Copilot Chat overflowing at 11,498 bytes) — so 1.1 MB is a real risk,
/// not a theoretical one. Checked cumulatively as matches accumulate;
/// the walk stops (and `SearchResults::truncated` is set) once crossed,
/// the same "stop, don't silently drop" discipline `MAX_RESULTS` itself
/// already uses.
const MAX_TOTAL_RESPONSE_BYTES: usize = 100_000;

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
    /// `true` when `context_lines > 0` was requested but a second read of
    /// this match's file failed (deleted, permission-denied, non-UTF8) in
    /// the window after the match itself was already found by grep's own,
    /// separate, earlier-succeeding read of the same path. Distinct from
    /// context legitimately being short or empty (a match near the file's
    /// start/end), which is `false` with `context_before`/`context_after`
    /// simply short. Always `false` when `context_lines` wasn't requested.
    pub context_unavailable: bool,
}

/// The whole result of one `search_code` call. `truncated` is a
/// **response-level** signal, distinct from any single match's own
/// `truncated` field (which only means that one match's `text` was cut):
/// `true` here means the walk stopped — on the match-count cap or
/// [`MAX_TOTAL_RESPONSE_BYTES`] — before every real match in the repo was
/// found, the same "say so, don't just stop silently" discipline this
/// project applies everywhere else a response can be incomplete
/// (`get_source`'s `truncated`, a spec's `stale` flag, `cap_generation_text`'s
/// marker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResults {
    pub matches: Vec<SearchMatch>,
    pub truncated: bool,
}

/// Cuts `line` to at most `MAX_MATCH_TEXT_BYTES`, keeping the start of
/// the line (where the match itself is far more likely to sit than the
/// tail) rather than the end. Delegates to `spec::cap_source_text` --
/// same contract (cut on a `char` boundary, no marker appended, `bool`
/// says whether it cut), so this doesn't re-walk the boundary logic a
/// second time (code review finding, M20: this used to be its own
/// duplicate of that loop).
fn truncate_match_text(line: &str) -> (String, bool) {
    crate::spec::cap_source_text(line.to_string(), MAX_MATCH_TEXT_BYTES)
}

/// One file's context-lines state within one `search_code` call, lazily
/// populated on first need (code review finding, M20: this used to read
/// every walked file unconditionally, before knowing whether it even had
/// a match — cost proportional to repo size, not match count). A file
/// with several matches is read at most once, not once per match; a
/// failed read is cached too, so it isn't retried per match either.
enum FileContext {
    NotYetRead,
    Read(Vec<String>),
    ReadFailed,
}

/// The context lines around `line` (1-based) in whatever file `state`
/// refers to, reading and caching that file's content into `state` on
/// first need. Returns `(before, after, unavailable)` — `unavailable:
/// true` means context was wanted but the read failed, distinct from
/// context legitimately being empty (a match at the file's start/end),
/// which reports `unavailable: false` with short or empty `before`/`after`.
/// See `SearchMatch::context_unavailable`'s doc comment for why that
/// distinction matters to a caller.
fn context_for_line(
    state: &mut FileContext,
    path: &Path,
    line: u64,
    context_lines: usize,
) -> (Vec<String>, Vec<String>, bool) {
    if matches!(state, FileContext::NotYetRead) {
        *state = match std::fs::read_to_string(path) {
            Ok(content) => FileContext::Read(content.lines().map(str::to_string).collect()),
            Err(_) => FileContext::ReadFailed,
        };
    }
    match state {
        FileContext::Read(lines) => {
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
            (before, after, false)
        }
        FileContext::ReadFailed => (Vec::new(), Vec::new(), true),
        FileContext::NotYetRead => unreachable!("populated just above"),
    }
}

/// Regex-search every file under `root` (respecting `.gitignore`, same as
/// the extraction walk) for `query`, in walk order, shaped by `opts`.
pub fn search_code(root: &Path, query: &str, opts: &SearchOptions) -> Result<SearchResults> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(opts.ignore_case)
        .build(query)
        .with_context(|| format!("invalid pattern: {query}"))?;
    let mut searcher = Searcher::new();
    let mut out = Vec::new();
    let mut total_bytes: usize = 0;
    let mut truncated = false;

    let context_lines = opts.context_lines.min(MAX_SEARCH_CONTEXT_LINES);
    let max_results = opts
        .max_results
        .map(|n| n.min(MAX_RESULTS))
        .unwrap_or(MAX_RESULTS);

    for entry in ignore::WalkBuilder::new(root).build() {
        if out.len() >= max_results || total_bytes >= MAX_TOTAL_RESPONSE_BYTES {
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

        let mut file_context = FileContext::NotYetRead;
        let mut stopped_mid_file = false;

        // A single unreadable or binary file shouldn't abort the whole
        // search — skip it and keep going.
        let _ = searcher.search_path(
            &matcher,
            path,
            UTF8(|line, text| {
                let (text, match_truncated) = truncate_match_text(text.trim_end());
                let (context_before, context_after, context_unavailable) = if context_lines > 0 {
                    context_for_line(&mut file_context, path, line, context_lines)
                } else {
                    (Vec::new(), Vec::new(), false)
                };
                total_bytes += text.len()
                    + context_before.iter().map(String::len).sum::<usize>()
                    + context_after.iter().map(String::len).sum::<usize>();
                out.push(SearchMatch {
                    file: rel.clone(),
                    line,
                    text,
                    truncated: match_truncated,
                    context_before,
                    context_after,
                    context_unavailable,
                });
                let keep_going = out.len() < max_results && total_bytes < MAX_TOTAL_RESPONSE_BYTES;
                if !keep_going {
                    stopped_mid_file = true;
                }
                Ok(keep_going)
            }),
        );

        if stopped_mid_file {
            truncated = true;
            break;
        }
    }

    Ok(SearchResults {
        matches: out,
        truncated,
    })
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

        let result = search_code(&dir, "target", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].file, "a.ts");
        assert_eq!(result.matches[0].line, 2);
        assert!(result.matches[0].text.contains("target"));
        assert!(!result.truncated);
    }

    #[test]
    fn no_match_returns_empty_not_an_error() {
        let dir = tmp("nomatch");
        write_fixture(&dir, "a.ts", "nothing interesting here\n");

        let result =
            search_code(&dir, "nonexistent_pattern_xyz", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert!(result.matches.is_empty());
        assert!(!result.truncated);
    }

    #[test]
    fn supports_regex_patterns() {
        let dir = tmp("regex");
        write_fixture(
            &dir,
            "a.ts",
            "export function foo() {}\nexport const bar = 1;\n",
        );

        let result =
            search_code(&dir, r"export (function|const)", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 2);
    }

    #[test]
    fn a_short_match_is_not_flagged_truncated() {
        let dir = tmp("shortline");
        write_fixture(&dir, "a.ts", "line one\nfunction target() {}\nline three\n");

        let result = search_code(&dir, "target", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 1);
        assert!(!result.matches[0].truncated);
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

        let result = search_code(&dir, "needle", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 1);
        assert!(
            result.matches[0].text.len() <= MAX_MATCH_TEXT_BYTES,
            "match text ({} bytes) exceeds the cap ({} bytes) -- truncation didn't run",
            result.matches[0].text.len(),
            MAX_MATCH_TEXT_BYTES
        );
        assert!(
            result.matches[0].text.starts_with("needle"),
            "truncation should keep the match itself, not just the line's tail"
        );
        assert!(
            result.matches[0].truncated,
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

        let result = search_code(&dir, "needle", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].line, 2);
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
        let result = search_code(&dir, "target", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].file, "src/a.ts");
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
        let result = search_code(&dir, "target", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].file, "lib/a.ts");
    }

    #[test]
    fn ignore_case_finds_a_differently_cased_match() {
        let dir = tmp("ignorecase");
        write_fixture(&dir, "a.ts", "const TARGET = 1;\n");

        let case_sensitive = search_code(&dir, "target", &SearchOptions::default()).unwrap();
        assert!(
            case_sensitive.matches.is_empty(),
            "must not match by default"
        );

        let opts = SearchOptions {
            ignore_case: true,
            ..Default::default()
        };
        let insensitive = search_code(&dir, "target", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(insensitive.matches.len(), 1);
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
        let result = search_code(&dir, "function target", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(
            result.matches[0].context_before,
            vec![
                "import { helper } from \"./helper\";".to_string(),
                "".to_string()
            ]
        );
        assert_eq!(
            result.matches[0].context_after,
            vec!["    return helper();".to_string(), "}".to_string()]
        );
        assert!(!result.matches[0].context_unavailable);
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
        let result = search_code(&dir, "needle", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 1);
        assert!(result.matches[0].context_before.len() <= MAX_SEARCH_CONTEXT_LINES);
        assert!(result.matches[0].context_after.len() <= MAX_SEARCH_CONTEXT_LINES);
    }

    #[test]
    fn context_lines_is_empty_by_default() {
        let dir = tmp("nocontext");
        write_fixture(&dir, "a.ts", "before\nneedle\nafter\n");

        let result = search_code(&dir, "needle", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 1);
        assert!(result.matches[0].context_before.is_empty());
        assert!(result.matches[0].context_after.is_empty());
        assert!(!result.matches[0].context_unavailable);
    }

    #[test]
    fn context_lines_finds_correct_context_for_a_second_match_in_the_same_file() {
        // Indirectly validates FileContext's per-file caching: the
        // second match's context must be just as correct as the
        // first's, not stale or empty from a read that only happened
        // (or was expected to happen) once.
        let dir = tmp("contextcache");
        write_fixture(&dir, "a.ts", "one\nneedle-a\nthree\nneedle-b\nfive\n");

        let opts = SearchOptions {
            context_lines: 1,
            ..Default::default()
        };
        let result = search_code(&dir, "needle", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.matches[0].context_before, vec!["one".to_string()]);
        assert_eq!(result.matches[0].context_after, vec!["three".to_string()]);
        assert_eq!(result.matches[1].context_before, vec!["three".to_string()]);
        assert_eq!(result.matches[1].context_after, vec!["five".to_string()]);
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
        let result = search_code(&dir, "needle", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 3);
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
        let result = search_code(&dir, "needle", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        // Only 5 real matches exist -- this asserts the request didn't
        // error or misbehave, the ceiling itself is exercised by
        // max_results_narrows_below_the_default_cap above.
        assert_eq!(result.matches.len(), 5);
    }

    // --- M20 code-review fixes: response-level truncated, cumulative
    // byte budget, lazy + failure-aware context reads -------------------

    #[test]
    fn truncated_is_false_when_every_real_match_is_returned() {
        let dir = tmp("nottrunc");
        write_fixture(&dir, "a.ts", "needle\n");

        let result = search_code(&dir, "needle", &SearchOptions::default()).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 1);
        assert!(!result.truncated);
    }

    #[test]
    fn truncated_is_true_when_max_results_cuts_off_real_matches() {
        let dir = tmp("trunccount");
        let mut src = String::new();
        for i in 0..10 {
            src.push_str(&format!("needle {i}\n"));
        }
        write_fixture(&dir, "a.ts", &src);

        let opts = SearchOptions {
            max_results: Some(3),
            ..Default::default()
        };
        let result = search_code(&dir, "needle", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result.matches.len(), 3);
        assert!(result.truncated, "10 real matches exist, only 3 returned");
    }

    #[test]
    fn a_cumulative_byte_budget_caps_the_whole_response_even_under_max_results() {
        // Code-review finding: MAX_RESULTS / MAX_MATCH_TEXT_BYTES /
        // MAX_SEARCH_CONTEXT_LINES are three independent per-axis caps
        // that never composed into a total bound -- worst case with
        // context_lines set was ~1.1 MB for one call, against a real
        // MCP client already observed overflowing at 11,498 bytes (see
        // MAX_TOTAL_RESPONSE_BYTES's own doc comment). Many matches here,
        // each near max-width with context, well under MAX_RESULTS (200)
        // but enough to cross the byte budget well before the count cap
        // would ever fire.
        let dir = tmp("budget");
        let mut src = String::new();
        for i in 0..100 {
            src.push_str(&format!("padding {i} {}\n", "x".repeat(400)));
            src.push_str("needle\n");
        }
        write_fixture(&dir, "a.ts", &src);

        let opts = SearchOptions {
            context_lines: 5,
            ..Default::default()
        };
        let result = search_code(&dir, "needle", &opts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert!(
            result.matches.len() < 100,
            "the byte budget should have stopped the walk well before all 100 real matches; got {}",
            result.matches.len()
        );
        assert!(result.truncated, "response must say it stopped early");

        let total: usize = result
            .matches
            .iter()
            .map(|m| {
                m.text.len()
                    + m.context_before.iter().map(String::len).sum::<usize>()
                    + m.context_after.iter().map(String::len).sum::<usize>()
            })
            .sum();
        let worst_case_overshoot = MAX_MATCH_TEXT_BYTES * (1 + 2 * MAX_SEARCH_CONTEXT_LINES);
        assert!(
            total <= MAX_TOTAL_RESPONSE_BYTES + worst_case_overshoot,
            "response ({total} bytes) exceeded the budget by more than one match's worth"
        );
    }

    // --- context_for_line: the isolated, directly-testable failure path.
    // The full grep-finds-a-match-but-our-second-read-fails race isn't
    // deterministically reproducible at the search_code level (grep's
    // own read of a path and this function's separate read can't be
    // made to disagree without mocking the filesystem), so the
    // failure-signaling contract is verified here in isolation instead.

    #[test]
    fn context_for_line_reads_and_caches_a_real_file() {
        let dir = tmp("ctxfn-ok");
        write_fixture(&dir, "a.ts", "one\ntwo\nthree\n");
        let path = dir.join("a.ts");

        let mut state = FileContext::NotYetRead;
        let (before, after, unavailable) = context_for_line(&mut state, &path, 2, 1);

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(before, vec!["one".to_string()]);
        assert_eq!(after, vec!["three".to_string()]);
        assert!(!unavailable);
        assert!(matches!(state, FileContext::Read(_)));
    }

    #[test]
    fn context_for_line_reports_unavailable_not_empty_on_a_failed_read() {
        let dir = tmp("ctxfn-fail");
        // Deliberately never created -- read_to_string must fail.
        let path = dir.join("does-not-exist.ts");

        let mut state = FileContext::NotYetRead;
        let (before, after, unavailable) = context_for_line(&mut state, &path, 1, 2);

        assert!(before.is_empty());
        assert!(after.is_empty());
        assert!(
            unavailable,
            "a failed read must be signaled, not silently reported as empty context"
        );
        assert!(matches!(state, FileContext::ReadFailed));
    }

    #[test]
    fn context_for_line_does_not_retry_a_cached_failed_read() {
        let mut state = FileContext::ReadFailed;
        let (before, after, unavailable) =
            context_for_line(&mut state, Path::new("/nonexistent/whatever"), 1, 2);
        assert!(before.is_empty());
        assert!(after.is_empty());
        assert!(unavailable);
    }
}
