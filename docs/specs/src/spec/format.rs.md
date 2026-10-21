---
kind: file
source_paths: [src/spec/format.rs]
file: { source_hash: , deps_hash: , spec_hash:  }
symbols:
  src/spec/format.rs::HashPair: { source_hash: 350e2e86a1e6ef631cac1d7225d5e5bfbd899f4efac317babd9e21ad3216a697, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c25b1458e08c3de25cf5011dd67e1bcc646222324e22e2b3e86e71cede7c3332 }
  src/spec/format.rs::SymbolProse: { source_hash: 0301037c6c6d90b416a855f49d23120dcd897baba2d3b2a52ad2c2a635e2640b, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 98c5598ecdf992708e4df4dcdb3a64fa69499fa12f7d8e0689f1aa2d79cbcc70 }
  src/spec/format.rs::FileSpec: { source_hash: 59577f31c85d49571b1bf9ae73fd4e51dfed05a8497cf601c80144f773ee1680, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 2c146e081ca47566567131298d7fd7fd745bfe91dad6f3d03d6a35116fdb916d }
  src/spec/format.rs::render: { source_hash: 0cdd420f8174ecf40cc1ce194f5c30213ce0fbd0a2ca38b61b8660a9373be3ad, deps_hash: 1905c59a809cb146e2d62da82589212debb7a142f8d907564aa5b0da29373eae, spec_hash: f2892f79dfe0d1ac3353f6df9f6f375477fe235720079354eaebc1b223cada89 }
  src/spec/format.rs::dependency_lines: { source_hash: dabf28e5aae746a62e15257ad05be04315d2b17bcc7fa1eaf0961cb042a8ac83, deps_hash: 43f1c124e9646bc90f5c80d49d1555450df5957531f3ebcb7bac0dac0dab7275, spec_hash: bd7b2b7db6238a3f393c256fab85cc69af16dbb735f0f992c0183ab19dc622a3 }
  src/spec/format.rs::parse: { source_hash: 7b03666ab07be2bfd1a7b41c42af5d7e4bd2986bc099b79931191478b9a7dc09, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: bb88686684920137bfbe120b8c451135a518cecfde3162b1f2eff33c8b7a75bf }
  src/spec/format.rs::parse_hash_pair: { source_hash: 7570e0ec732222b6706d5c0c73fcf90b50d9df7aedd2cbcd5af7143e7ce93733, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: be2d56cd4b5a9d3442d39567b7d9a2abf8b53d1caa62acf7b2ab53f81fe10c1b }
  src/spec/format.rs::extract_section: { source_hash: c5312439f0d2375f94db65ad86471bbd739b8ba3bc4e9c7c2cbc5b21b6b9708a, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 11a6146ee9e5c8484a016bb090a900d1c52850b1a8fc1031934eb7dd7463571d }
  src/spec/format.rs::read_file_spec: { source_hash: 94ddd07c09c53d9a4e06279f20eb5da6ed687c3f47daba7ab3849053b927f7b8, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 04accf5385d9023f9d21cc8cfe08aff711eed064c5bf726c2b2d8e5e1bd98211 }
  src/spec/format.rs::read_existing: { source_hash: 2f01ef9e6863931910cfc5812104518ce5a05603ee8898c1af55d0f0b3b03bcd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: cf74d8d13f92d66cc4935217b72987d9b1893e8b39c511f0b3333be827dbd1fe }
  src/spec/format.rs::write: { source_hash: 89cf9806e951ba2d4809a640929606435a8ef4ab024ccc726a75f47596acdcb2, deps_hash: 1905c59a809cb146e2d62da82589212debb7a142f8d907564aa5b0da29373eae, spec_hash: e91a23dd2fe942a1e91d9c6873b85c71d706d587af3e330213e8308ae73a6279 }
dep_targets:
  src/spec/format.rs::render -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/format.rs::render -> src/symbol.rs::SymbolId: 6733b5c62e75
  src/spec/format.rs::dependency_lines -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/format.rs::dependency_lines -> src/graph.rs::Node: 1e1e50b8ba0e
  src/spec/format.rs::dependency_lines -> src/symbol.rs::Symbol: 2bab71782e46
  src/spec/format.rs::dependency_lines -> src/symbol.rs::SymbolId: 6733b5c62e75
  src/spec/format.rs::write -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/format.rs::write -> src/symbol.rs::SymbolId: 6733b5c62e75
---
# src/spec/format.rs
## Summary


## `HashPair`
`pub struct HashPair`
### Summary
The three fingerprints stored with each written spec, used later to tell whether the spec still matches the code. Despite the name it holds three values, not two.
### Behavior
`source_hash` fingerprints the item's own code at the time the spec was written. `deps_hash` fingerprints the public shape (the `interface_hash`, see `GLOSSARY.md`) of everything the item depends on, as observed at that moment. It catches the case where the item's own text is unchanged but something it imports changed shape. `spec_hash` fingerprints the prose the agent wrote, so hand edits to a spec can be detected. Staleness compares the first two against freshly computed values and never uses the third for invalidation. The struct is plain data with public fields and no logic.
### Depends on
- (none)

## `SymbolProse`
`pub struct SymbolProse`
### Summary
The two pieces of prose an agent writes for one function or type: a short summary and a longer description of what it does.
### Behavior
Holds a `summary` string and a `behavior` string, matching the two headings of a symbol task. Everything else in a symbol's section, such as the signature and dependency list, is filled in by CodeOwl from the graph rather than stored here. The struct is plain data with no validation of its own; the quality check on submission is what rejects text that is too short or that dodges the question.
### Depends on
- (none)

## `FileSpec`
`pub struct FileSpec`
### Summary
The in-memory form of one file's spec document: the file's own fingerprints, a fingerprint per function or type, and all the prose. It is kept separate from the markdown text so the code that saves and merges specs never has to re-read what it just wrote.
### Behavior
Fields: `source_path` (the file's repo-relative path), `file` (a `HashPair` for the whole file), `symbols` (an id plus `HashPair` for each top-level function or type, in declaration order), `file_summary` (the prose under the file's `## Summary`), and `sections` (an id plus `SymbolProse` per symbol). Lists of pairs are used rather than maps to keep declaration order. Three private helpers exist: `blank` makes an empty spec for a path with every other field defaulted, and `symbol_hash` and `section` find one symbol's hashes or prose by id with a linear scan, returning `None` when the id is absent. Scans are fine because a file has few top-level symbols.
### Depends on
- (none)

## `render`
`pub fn render(graph: &Graph, root: &Path, file_id: SymbolId, spec: &FileSpec) -> String`
### Summary
Turns a file's stored spec data into the markdown document saved under `docs/specs/`. It mixes the prose an agent wrote with facts CodeOwl recomputes each time, such as signatures and dependency lists, so those facts never go stale or get invented.
### Behavior
Output is built in this order. First a frontmatter block (the `---` header with machine-readable fields) with `kind: file`, `source_paths`, the file's three hashes on one line, and, when there are symbols, one hash line per symbol id. Then a `# <path>` title and the `## Summary` with the trimmed file prose. Then, for each symbol in `spec.symbols` order, a `## \`name\`` heading (from `short_name`), the current signature looked up in the graph (omitted if the id no longer resolves), `### Summary` and `### Behavior` from the matching section (an empty default if none was stored), and a `### Depends on` list from `dependency_lines`, or `- (none)` when empty. `root` is only needed so `dependency_lines` can read the symbol's own source off disk and list just what that symbol uses, not every import in the file. All prose is trimmed and ends with a newline. The function never fails and has no side effects.
### Depends on
- `src/spec/layout.rs::short_name` — super::layout
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph
- externals: std

## `dependency_lines`
`pub(super) fn dependency_lines(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    sym: &Symbol,
) -> Vec<String>`
### Summary
Works out the "Depends on" list for one function or type: which other places in this repo it leans on, plus a single line naming outside libraries. CodeOwl writes this itself so the agent never has to guess it.
### Behavior
Imports are resolved per file, so listing all of a file's imports against every symbol would claim dependencies a symbol never uses. To narrow it, the function reads the symbol's own source text from disk (`symbol_span_text`) and calls `scoped_symbol_deps`, which does a whole-word text search for each imported name. That is a heuristic, not real analysis, but much closer to the truth. Each resolved in-repo dependency becomes one line of the form `` `target` — specifier ``. All unresolved imports (standard library, third-party crates, npm packages) are merged into one trailing `externals: a, b` line, so the many standard-library names Rust code mentions don't bury the in-repo ones. It returns an empty list if the id is not a file node or if the source cannot be read, so a missing file yields no dependency lines rather than an error.
### Depends on
- `src/spec/source_text.rs::scoped_symbol_deps` — super::source_text
- `src/spec/source_text.rs::symbol_span_text` — super::source_text
- `src/graph.rs::Graph` — crate::graph
- `src/graph.rs::Node` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph
- `src/symbol.rs::Symbol` — crate::symbol
- externals: std

## `parse`
`pub fn parse(content: &str) -> Result<FileSpec>`
### Summary
Parses a file spec document's own rendered text back into a structured `FileSpec` — the inverse of `render`. Only needs to handle exactly what `render` itself produces; tolerating arbitrary human reformatting or reordering of a hand-edited spec is a separate, not-yet-built concern (per `ARCHITECTURE.md`'s "Human corrections").
### Behavior
Requires the document to start with a `---` line opening the YAML frontmatter, erroring otherwise. Walks the frontmatter line by line: a `symbols:` line switches into a nested-block mode where each following two-space-indented line is one symbol entry — split at the flow map's opening `{` rather than the first `:`, since a symbol's own id contains `::` (like `file.ts::name`) and splitting on the first colon would cut the id apart. Every other top-level frontmatter line is split on the first `:` into a key/value pair; `source_paths` is unwrapped from its `[...]` bracket syntax, and `file` is parsed into a `HashPair` via `parse_hash_pair`. Parsing stops at the closing `---`.

The remaining body text (everything after the closing `---`) is scanned for the file's own `## Summary` section (via `extract_section`, bounded by the next `## ` heading). Then, for each symbol recorded in the frontmatter, it locates that symbol's own `## \`name\`` heading in the body, extracts the block of text up to the next `## ` heading, and within that block extracts the `### Summary` and `### Behavior` sections (each bounded by the next `### ` heading) into a `SymbolProse`. A symbol whose heading can't be found in the body is silently skipped rather than erroring — this happens if the frontmatter and body have somehow gone out of sync, and skipping rather than failing keeps the rest of a still-parseable document usable.
### Depends on
- `src/spec/layout.rs::short_name` — super::layout
- externals: anyhow

## `parse_hash_pair`
`fn parse_hash_pair(flow_map: &str) -> HashPair`
### Summary
Parses a YAML flow-map string like `{ source_hash: abc, deps_hash: def, spec_hash: ghi }` back into a `HashPair`.
### Behavior
Strips the surrounding `{`/`}` braces, splits the inside on commas, and for each `key: value` pair, assigns the trimmed value to the matching `HashPair` field by name — any key that isn't one of the three recognized names is silently ignored. Fields not present in the input keep their default (empty string) value.
### Depends on
- (none)

## `extract_section`
`pub(super) fn extract_section(text: &str, heading: &str, next_marker: &str) -> Option<String>`
### Summary
A small text-extraction helper: finds a named heading and pulls out everything between it and the next section marker, trimmed — the primitive `parse` uses repeatedly to pull individual sections (`## Summary`, `### Behavior`, etc.) back out of rendered markdown.
### Behavior
`, etc.) back out of rendered markdown.
### Depends on
- (none)

## `read_file_spec`
`pub fn read_file_spec(root: &Path, source_path: &str) -> Result<Option<FileSpec>>`
### Summary
The public entry point for reading a file's already-generated spec off disk, if one exists — the function `get_spec`'s MCP handler calls directly, and one this file's own generation/submission logic reuses internally too. Always a pure read that never triggers generation.
### Behavior
A thin wrapper delegating to `read_existing`, returning `Ok(None)` if no spec file exists yet for this source path rather than an error — "no spec generated yet" is an expected, normal state, not a failure.
### Depends on
- externals: anyhow, std

## `read_existing`
`pub(super) fn read_existing(root: &Path, source_path: &str) -> Result<Option<FileSpec>>`
### Summary
Reads and parses a file's spec document off disk, if it exists.
### Behavior
Computes the spec's expected path via `spec_path`; if nothing exists there, returns `Ok(None)` rather than an error, since a file simply not having a spec yet is a normal, expected state. If the file does exist, reads its contents and parses them via `parse`, propagating any read or parse error.
### Depends on
- `src/spec/layout.rs::spec_path` — super::layout
- externals: anyhow, std

## `write`
`pub(super) fn write(root: &Path, graph: &Graph, file_id: SymbolId, spec: &FileSpec) -> Result<()>`
### Summary
Saves a file's spec to disk as a markdown document under `docs/specs/`, creating folders as needed.
### Behavior
Works out the output path from the file's source path with `spec_path`, creates any missing parent directories, and writes the string produced by `render`, overwriting whatever was there. Errors from creating the directory or writing the file are returned with context naming the path ("creating ..." or "writing ..."). It does a plain overwrite with no temp file or atomic rename, so a crash mid-write could leave a partial document. Anything calling it has already merged the new prose with existing prose, so nothing is read back here.
### Depends on
- `src/spec/layout.rs::spec_path` — super::layout
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph
- externals: anyhow, std
