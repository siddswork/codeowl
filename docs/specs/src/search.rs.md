---
kind: file
source_paths: [src/search.rs]
file: { source_hash: b83d8c5ce308374c94d01ea0a3e7b554fe0e6810b999bf1153cb6cfffb91af49, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: d88f6c9d3f03ffe65481abeb1f5576e265672927943698de7b03bd346aa859fa }
symbols:
  src/search.rs::SearchOptions: { source_hash: 1399010192964f0e5a148438a92a5f48db64af3f512535d46f363333a480415c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 82ebdb58f8bc462738f9ec5ae49b121ba8a9549256a23f1c534eef44fe790464 }
  src/search.rs::SearchMatch: { source_hash: a50e79c8ede0fa3329c54bbb2d06cc8af4c287ca0ab645620a060a315e31c865, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 5ce2e04bee86cfde4a9a3ed0bb0f34bdcd7d47167c809da736e625c853810d43 }
  src/search.rs::SearchResults: { source_hash: ad7429abcf29bf5a447a27b84535a41b5aee4e5754363e3348440fe5f458a014, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: d0680c6f0839f4e22bf0650192c57a247b42cd6f2512acf3cd7842e9d1ab4c36 }
  src/search.rs::truncate_match_text: { source_hash: 0140a0b0476bcc7929b66253e894118624759cb2d7defba24d50be1492b75410, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a755b01f323fa18a030212f489a5fd75a75a19deb7823ae8657c5e784ea6fc9a }
  src/search.rs::FileContext: { source_hash: 6116823c2997907ec9b31295008e421a9d19069223047bf513b8418f70fc0126, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: e6a2295f09ecd475c187ee6964c47e12df8c4cb7b38c00eb9a15f5e0d556c78d }
  src/search.rs::context_for_line: { source_hash: be9bdc64dfc2aec29cde363f55411771a3a7ab7895a0f208db39ad59a5651b7d, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 273f0301b9c6452892651421b18fe0829f773eb66d5f720e8361d6aa685d13c1 }
  src/search.rs::search_code: { source_hash: c98c30b346c30c439148f3afd45768b76b6b3ee37a8a2adec4411476d89ce860, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 27003a6b02dae86346d7235cb565636f44ef48107a2af079b28e22a97584c57f }
---
# src/search.rs
## Summary
This file implements the `search_code` tool's plain regex search over the repo, using an embedded copy of ripgrep (the `grep` crate) rather than a real search index — a repo this size doesn't need one to search fast enough to be useful, so the real index (keyword plus semantic search) is deferred to a later phase. Because an unbounded search could return an enormous response — either from too many matches, from lines that are individually huge, or from context lines multiplying that further — this file defines four separate caps that each guard a different way a response can balloon: total match count, one match's own text width, how many context lines can surround a match, and a coarser total-response-byte ceiling covering all of that combined. Whenever any of those caps cuts a search short, the result says so explicitly (`truncated: true`) rather than silently returning a partial answer that looks complete.

## `SearchOptions`
`pub struct SearchOptions`
### Summary
Groups every optional knob `search_code` accepts besides the query text itself — where to look, case sensitivity, how much surrounding context to include, and how many matches to cap at.
### Behavior
Four fields, all optional in effect:
- `path`: restricts matches to files under this path. It's a true boundary match, not a bare string prefix — `"lib"` matches `"lib/foo.ts"` but never `"library/foo.ts"` — reusing the same boundary rule `spec::within_scope` uses elsewhere. `None` searches the whole repo.
- `ignore_case`: whether matching is case-insensitive.
- `context_lines`: how many lines of surrounding context to include on each side of a match, capped at `MAX_SEARCH_CONTEXT_LINES` no matter what value is requested.
- `max_results`: narrows the match-count cap below `MAX_RESULTS`. `None`, or any value above `MAX_RESULTS`, leaves the default ceiling in place — this field can only shrink a response, never grow it past the built-in cap.

`SearchOptions::default()` reproduces exactly what `search_code` did before these options existed: no path filter, case-sensitive, no context lines, and the full `MAX_RESULTS` ceiling — so a caller that never sets any of these fields gets identical behavior to before this struct was introduced.
### Depends on
- (none)

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
Cuts an overly long matched line down to a manageable size, keeping the beginning of the line rather than the end — since the actual match is far more likely to be near the start than buried at the tail.
### Behavior
Delegates entirely to `crate::spec::cap_source_text`, passing `MAX_MATCH_TEXT_BYTES` as the cap. It reuses that function's exact contract rather than reimplementing it: the cut happens on a `char` boundary (never splitting a multi-byte character in half), no truncation marker text is appended to the result, and the returned `bool` reports whether a cut actually happened. Sharing this logic instead of duplicating it was a deliberate fix — an earlier version of this function had its own separate copy of the same boundary-safe truncation loop.
### Depends on
- (none)

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
The actual implementation behind the `search_code` MCP tool — a plain regex search across every file in the repo, respecting `.gitignore` the same way extraction does, capped so neither a broad query nor a huge repo can blow up the response size.
### Behavior
Builds a regex matcher from `query` (case-insensitive if `opts.ignore_case`), returning an error if the pattern itself is invalid. Clamps `opts.context_lines` to `MAX_SEARCH_CONTEXT_LINES` and `opts.max_results` to `MAX_RESULTS` (both can only shrink these limits, never raise them, mirroring `SearchOptions`'s own contract).

Walks every file under `root` via the same gitignore-respecting walk extraction uses. For each file:
- Stops the whole walk early once `max_results` matches have been collected or `MAX_TOTAL_RESPONSE_BYTES` has been reached.
- Skips anything that isn't a plain file, and — if `opts.path` narrows the search — skips any file outside that scope (`crate::spec::within_scope`, the same boundary-aware check `SearchOptions::path` documents).
- Searches the file's content line by line for the pattern. A single unreadable or binary file is skipped rather than aborting the entire search (the search result is discarded with `let _ =`, deliberately swallowing that one file's error).
- For each matched line: truncates it via `truncate_match_text`, then — only if `context_lines > 0` — fetches surrounding context via `context_for_line`, lazily reading and caching that file's lines the first time any of its matches needs them (via a fresh `FileContext::NotYetRead` created per file). Running total byte count (`total_bytes`) accumulates the matched text plus all context text, since that's the real payload-size driver, not just the match count.
- If either cap is hit partway through a file's matches, the whole walk stops immediately (not just that file) and the response is marked `truncated: true`.

Returns a `SearchResults` with every match found and the accumulated `truncated` flag.
### Depends on
- externals: anyhow, grep, std
