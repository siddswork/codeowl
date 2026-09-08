---
kind: file
source_paths: [src/search.rs]
file: { source_hash: 7c53defd02eba44ea01e9e7a765782c846921be4b208d487eee3a5bce7d59ec6, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 79c13977664a1e7b8901d372590c08c9d5e30c681be4fbadb9b1ef6ea188b5ed }
symbols:
  src/search.rs::SearchMatch: { source_hash: 08ed69463cb10cb85546786c62d091d65db2bd576598d1bcd19ddee49f986cd1, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: ff7ba35be5f4aeb07eb6effa7d48725003688975d9de52753a3e11ee83f74fa3 }
  src/search.rs::search_code: { source_hash: dbf36860dbce95415c0852b2b955e6cf870807331ce9e1cbf3d068b9108951f7, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 5db6d63c0a5816316a71edd61db2c25378d6af4049de19d2e7251caca28cb365 }
---
# src/search.rs
## Summary
`search_code`'s Phase-1 implementation: embedded ripgrep (the `grep` crate) over the gitignore-visible tree, no index. One public function, `search_code`, returning `SearchMatch`es (file / line / text) capped at `MAX_RESULTS` (200) so a broad query can't balloon an MCP response. Per `ARCHITECTURE.md`'s "Storage", the `tantivy` keyword index and embedding search are deferred to Phase 2 — a laptop-scale repo searches fast enough without one.

## `SearchMatch`
`pub struct SearchMatch`
### Summary
One `search_code` hit — the repo-relative file, the 1-indexed line number, and the matching line's text. What the MCP `search_code` tool returns (wrapped in a `SearchResponse` object).
### Behavior
Plain data. `text` is the whole line, not just the matched span, trimmed of its trailing newline. `line` is 1-indexed to match editor conventions.
### Depends on
- (none)

## `search_code`
`pub fn search_code(root: &Path, query: &str) -> Result<Vec<SearchMatch>>`
### Summary
Regex-searches every file under `root` for `query`, returning up to `MAX_RESULTS` `SearchMatch`es in walk order — CodeOwl's Phase-1 `search_code`, embedded ripgrep with no index.
### Behavior
Compiles `query` as a `RegexMatcher` — an invalid pattern is the one error this returns (`with_context` names it). Walks the tree with `ignore` (so `.gitignore` is respected, exactly like the extraction walk), searching each *file* (directories skipped). A per-file search error — an unreadable file, a binary file — is swallowed (`let _ =`) so one bad file doesn't abort the whole search. The `MAX_RESULTS` cap is enforced twice: the outer loop breaks once the vec is full, and the per-line callback returns `false` to stop mid-file. Paths are forward-slash normalised.
### Depends on
- externals: anyhow, grep, std
