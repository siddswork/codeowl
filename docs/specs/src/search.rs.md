---
kind: file
source_paths: [src/search.rs]
file: { source_hash: b83d8c5ce308374c94d01ea0a3e7b554fe0e6810b999bf1153cb6cfffb91af49, deps_hash: b010edda95ab5c521b46be256d4724f0abf419ac9dbdac186bdde16fc44fd95d, spec_hash: 5dd4519e21a1f5c7d5a7762276e18dfb10b7e8ab991c8074ca840cec87881916 }
symbols:
  src/search.rs::SearchOptions: { source_hash: 1399010192964f0e5a148438a92a5f48db64af3f512535d46f363333a480415c, deps_hash: 4e1b3a6a6404b8cfa7e53da01a87ebb3862b58da1244e90c42575a3275c7037a, spec_hash: 16507891a16339fb21f6431d907c5c4a3acb0279ce0c3c4086377f3aeb8dc661 }
  src/search.rs::SearchMatch: { source_hash: a50e79c8ede0fa3329c54bbb2d06cc8af4c287ca0ab645620a060a315e31c865, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 5ce2e04bee86cfde4a9a3ed0bb0f34bdcd7d47167c809da736e625c853810d43 }
  src/search.rs::SearchResults: { source_hash: ad7429abcf29bf5a447a27b84535a41b5aee4e5754363e3348440fe5f458a014, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: d0680c6f0839f4e22bf0650192c57a247b42cd6f2512acf3cd7842e9d1ab4c36 }
  src/search.rs::truncate_match_text: { source_hash: 0140a0b0476bcc7929b66253e894118624759cb2d7defba24d50be1492b75410, deps_hash: ae97b3ec05885270be2acee996971dc6377fae91a2b56e7b997ea576431176b6, spec_hash: 624d005df58341665993a9a9c2b539e91f37c83c1c6c5a415bb235564ca5446f }
  src/search.rs::FileContext: { source_hash: 6116823c2997907ec9b31295008e421a9d19069223047bf513b8418f70fc0126, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: e6a2295f09ecd475c187ee6964c47e12df8c4cb7b38c00eb9a15f5e0d556c78d }
  src/search.rs::context_for_line: { source_hash: be9bdc64dfc2aec29cde363f55411771a3a7ab7895a0f208db39ad59a5651b7d, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 273f0301b9c6452892651421b18fe0829f773eb66d5f720e8361d6aa685d13c1 }
  src/search.rs::search_code: { source_hash: c98c30b346c30c439148f3afd45768b76b6b3ee37a8a2adec4411476d89ce860, deps_hash: 4e1b3a6a6404b8cfa7e53da01a87ebb3862b58da1244e90c42575a3275c7037a, spec_hash: a15ff8bf78065f9bc9abfa4a63df8a000b8b65145bcc0b95cfe166eb84a51b95 }
dep_targets:
  file -> src/spec.rs::cap_source_text: fb0d9470a29c
  file -> src/spec.rs::within_scope: 4dcc40036692
  src/search.rs::SearchOptions -> src/spec.rs::within_scope: 4dcc40036692
  src/search.rs::truncate_match_text -> src/spec.rs::cap_source_text: fb0d9470a29c
  src/search.rs::search_code -> src/spec.rs::within_scope: 4dcc40036692
---
# src/search.rs
## Summary
This file implements the code search tool: a plain regular-expression search over every file in the repo, using an embedded copy of ripgrep's search library instead of a real search index, since a repo this size does not need one to be fast enough. Because an unbounded search could return an enormous response, four separate limits each guard a different way a result can balloon: the number of matches (200 at most, which a caller can only lower), the width of one matched line (500 bytes, keeping the start of the line, where the match is most likely to be), the number of context lines around a match (5 at most), and a whole-response ceiling of about 100,000 bytes that covers all of those combined. When any limit cuts a search short, the result says so with a `truncated` flag instead of quietly returning something that looks complete, and a single match whose line was shortened carries its own flag. A search can be limited to a file or folder by whole path pieces, can ignore case, and can include surrounding lines, which are read lazily so a file with no matches is never read a second time. A match whose context could not be read is marked unavailable, which is different from a match near the start or end of a file that simply has little context. An invalid pattern is the only error; unreadable or binary files are skipped, and results come back in the order files are visited, not ranked.

## `SearchOptions`
`pub struct SearchOptions`
### Summary
The choices a caller can make about a code search beyond the pattern itself: where to look, whether case matters, how much surrounding text to show, and how many matches to allow.
### Behavior
`path` limits matches to one file or folder using the same whole-path-piece rule as the coverage scope, so `lib` matches `lib/foo.ts` and never `library/foo.ts`; `None` searches the whole repo. `ignore_case` turns on case-insensitive matching. `context_lines` is the number of lines of surrounding text to include on each side of a match, capped at `MAX_SEARCH_CONTEXT_LINES` whatever is asked. `max_results` can only lower the match limit below `MAX_RESULTS`; `None`, or a larger number, leaves the full ceiling. The default value reproduces the original search exactly: no path filter, case-sensitive, no context, and the full match ceiling. The struct is plain data, and the caps are applied by the search function.
### Depends on
- `src/spec.rs::within_scope` — crate::spec

## `SearchMatch`
`pub struct SearchMatch`
### Summary
One matched line from a `search_code` query, plus whatever surrounding context was asked for and any caveats about how trustworthy that context is.
### Behavior
`file` and `line` locate the match; `text` is the matched line itself, trimmed only of trailing whitespace unless it's too long, in which case `truncated` is `true` and `text` was cut short at `MAX_MATCH_TEXT_BYTES` — `false` means `text` is the complete line.

`context_before`/`context_after` hold the lines immediately surrounding the match (oldest-first before, file-order after), populated only when the caller requested `context_lines > 0`. Each context line is silently trimmed to the same `MAX_MATCH_TEXT_BYTES` cap `text` uses — deliberately silent (no separate truncation flag per context line), so a long context line can't reopen the response-size problem `text`'s own cap exists to close.

`context_unavailable` is `true` specifically when context was requested but a *second* read of the match's file — needed to pull the surrounding lines, done after the match itself was already found by an earlier, separate, successful read (via grep) of the same path — failed in between (the file was deleted, permission was denied, or it's no longer valid UTF-8). This is distinct from context legitimately being short or empty because the match sits near the very start or end of the file, which reports `context_unavailable: false` with simply fewer (or zero) lines in `context_before`/`context_after`. It's always `false` when `context_lines` wasn't requested at all.
### Depends on
- (none)

## `SearchResults`
`pub struct SearchResults`
### Summary
The full outcome of one `search_code` call: every match found, plus a flag saying whether the search stopped early before covering the whole repo.
### Behavior
`matches` is the list of `SearchMatch` results. `truncated` is a *response-level* signal, separate from any individual match's own `truncated` field (which only means that one match's `text` was cut short) — this one means the search itself stopped, either because it hit the match-count cap or because it hit `MAX_TOTAL_RESPONSE_BYTES`, before every real match in the repo had been found. This follows the same "say so, don't stop silently" pattern used elsewhere in the project wherever a response can be incomplete — `get_source`'s own `truncated` field, a spec's `stale` flag, and the marker `cap_generation_text` sets are all the same discipline applied to a different kind of response.
### Depends on
- (none)

## `truncate_match_text`
`fn truncate_match_text(line: &str) -> (String, bool)`
### Summary
Shortens one matching line of code to a fixed number of bytes so a single very long line, such as a minified file or committed prose, cannot make a search result huge. It also reports whether it cut anything.
### Behavior
Calls `spec::cap_source_text` with the line and `MAX_MATCH_TEXT_BYTES`, and returns the text and a flag. It keeps the start of the line and cuts the end, because the match itself is far more likely to be near the beginning than the tail. The cut lands on a character boundary, so a multi-byte character is never split, and no marker text is added, so what comes back is real code. The flag is true when it cut. A line within the limit is returned unchanged with false. Reusing the shared function means the boundary rule is written once. It cannot fail.
### Depends on
- `src/spec.rs::cap_source_text` — crate::spec

## `FileContext`
`enum FileContext`
### Summary
Tracks whether a file's contents have been read yet, for pulling out the context lines around a search match — read lazily, and at most once per file, no matter how many matches that file has.
### Behavior
Three states: `NotYetRead` (the starting state — this file hasn't been opened for context yet), `Read(Vec<String>)` (successfully read, holding every line of the file for slicing out context), and `ReadFailed` (the read was attempted and failed — deleted, permission-denied, or non-UTF8).

This exists so a file is read at most once per search, on first need, rather than once per match or unconditionally for every file the walk touches — reading unconditionally would cost time proportional to the whole repo's size rather than to how many matches were actually found. Caching `ReadFailed` matters just as much as caching a successful read: without it, a file that fails to read would be retried on every subsequent match in that same file instead of failing once and staying failed.
### Depends on
- (none)

## `context_for_line`
`fn context_for_line(
    state: &mut FileContext,
    path: &Path,
    line: u64,
    context_lines: usize,
) -> (Vec<String>, Vec<String>, bool)`
### Summary
Pulls out the lines immediately before and after a given match, reading the file into `state` the first time it's needed and reusing that cached read for every later match in the same file.
### Behavior
If `state` is still `NotYetRead`, reads `path` to a string, splits it into lines, and stores them as `FileContext::Read(lines)` — or, if the read fails, stores `FileContext::ReadFailed` so the failure is remembered rather than retried on the next call for this same file.

Once populated, if `state` is `Read(lines)`: converts the 1-based `line` to a 0-based index (clamped to the file's length), then slices out up to `context_lines` lines before and after that index (each clamped so it never runs past the start or end of the file), passing each context line through `truncate_match_text` the same way a matched line itself is capped. Returns `(before, after, false)` — the `false` meaning context is genuinely available, even if `before`/`after` end up short or empty because the match sits near the file's start or end.

If `state` is `ReadFailed`, returns `(empty, empty, true)` — the `true` specifically distinguishing "context was wanted but the file couldn't be read" from the legitimately-short-or-empty case above; see `SearchMatch::context_unavailable` for why a caller needs to tell these two apart. The `NotYetRead` match arm is unreachable, since the check just above always populates `state` into one of the other two variants before this match runs.
### Depends on
- externals: std

## `search_code`
`pub fn search_code(root: &Path, query: &str, opts: &SearchOptions) -> Result<SearchResults>`
### Summary
Searches every file in the repo for a regular-expression pattern and returns the matching lines, in the order the files are visited, with optional surrounding lines. It is the engine behind the code search tool and uses an embedded copy of ripgrep's search library, so it needs no separate index.
### Behavior
Builds the matcher from the pattern, case-insensitive if asked, and fails with "invalid pattern" if the pattern does not compile. It applies the caps: context lines are limited to `MAX_SEARCH_CONTEXT_LINES`, and the match count to `MAX_RESULTS` or a lower `max_results`. It then walks the repo, respecting ignore files as the indexing walk does, and stops as soon as the match count or the total response size (`MAX_TOTAL_RESPONSE_BYTES`) is reached. Only regular files are searched, and a file outside the optional `path` scope (the whole-path-piece rule) is skipped.

For each matching line it records the file path, with forward slashes and relative to the root, the line number, the line text trimmed of trailing whitespace and shortened by `truncate_match_text`, a per-match `truncated` flag, and, if requested, lines of context before and after. Context is read lazily per file, and if it cannot be read the match is marked `context_unavailable`, which is different from context legitimately being empty. The running size counts the text and the context lines. A file that is unreadable or binary is skipped silently and the search goes on.

When a limit is hit during a file, the walk stops and the result's `truncated` flag is set to true, meaning some matches may be missing. The flag is set at the moment the limit is reached, so it can be true even if there were no further matches. Results are in walk order, not ranked. Only an invalid pattern returns an error.
### Depends on
- `src/spec.rs::within_scope` — crate::spec
- externals: anyhow, grep, std
