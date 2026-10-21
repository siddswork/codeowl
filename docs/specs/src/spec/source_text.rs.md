---
kind: file
source_paths: [src/spec/source_text.rs]
file: { source_hash: , deps_hash: , spec_hash:  }
symbols:
  src/spec/source_text.rs::ScopedDeps: { source_hash: 806bd54f5edebbd0cbdc653116c333121243559da051ec9fef111f64f906fd9b, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b8c136fcd6e9f4aa49a82c01d7595537869d8501b9aacb0d74c1f6457bcf2651 }
  src/spec/source_text.rs::scoped_symbol_deps: { source_hash: d0426bef62f7a49cd89dc4af7577d9aae4ae5bd75c3d3f4191bd214ec68eb3a5, deps_hash: 1905c59a809cb146e2d62da82589212debb7a142f8d907564aa5b0da29373eae, spec_hash: 5130210173d807cc66a6e7488b8ed98f487c596d8007cc8810e542df2f39e747 }
  src/spec/source_text.rs::merged_symbol_spans: { source_hash: 4d88a2156d24a1497565ea8bf3a2026fdd5826c4f7e2e3325a13b13bcd9a40af, deps_hash: 1e04ce86c45077cb522c55b1079bcc10ab61ba0cccfcd6900ef4fc1da907fe0e, spec_hash: f3cadc70b04dd0feca69ba2710809d76b3c153a7ff28f963d2323a98b8a60c13 }
  src/spec/source_text.rs::render_spans: { source_hash: 5134907334415d13a4e5ba9d4cfe5029e0ed6ed81b669c446284765b3af4c548, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 8d7d7445367bf47450358d3151e4b9d110d3dc0afe9a47d288eb5652229e3f0e }
  src/spec/source_text.rs::symbol_span_text: { source_hash: 8ba73b5794e3ee22dcd5a01c115bf33d6c4e67228efe552143ddc64906b406c4, deps_hash: 1e04ce86c45077cb522c55b1079bcc10ab61ba0cccfcd6900ef4fc1da907fe0e, spec_hash: ffd8862cef99f985c3e45ef4c0b9f24716a732937c083d71cebcdb962438d9bd }
  src/spec/source_text.rs::cap_generation_text: { source_hash: 66dae69f538d2d65c463905382ab32740f94cff9851b94e14af17962c871dd20, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 7e91837006c03537e5595b9e5bac3ae2c3063d72ba8c815ca38e3f17e709023b }
  src/spec/source_text.rs::maybe_reduce_container_source: { source_hash: 3fddb76ec01128bc47bbb1b90c5ad5a175868750e906340b449ef51bc495a975, deps_hash: 129a981f1d95d913458f9ade0ff28290c18be7bf60eba324aeb8479f28c61a3f, spec_hash: 6581dece472c922951df0bfd55eb188c697eacab34e306aa666469f211c639d3 }
  src/spec/source_text.rs::contains_identifier: { source_hash: d61f41110581dbd5c1a70313414a625c41779c7fc86e4a67135ce9ab46f7fadb, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: d06574f14b14ea8694940d4944a104a15bd9ac7c7aefe26498cdff4fa1ea54ab }
dep_targets:
  src/spec/source_text.rs::scoped_symbol_deps -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/source_text.rs::scoped_symbol_deps -> src/symbol.rs::SymbolId: 6733b5c62e75
  src/spec/source_text.rs::merged_symbol_spans -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/source_text.rs::merged_symbol_spans -> src/symbol.rs::Symbol: 2bab71782e46
  src/spec/source_text.rs::symbol_span_text -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/source_text.rs::symbol_span_text -> src/symbol.rs::Symbol: 2bab71782e46
  src/spec/source_text.rs::maybe_reduce_container_source -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/source_text.rs::maybe_reduce_container_source -> src/symbol.rs::Symbol: 2bab71782e46
  src/spec/source_text.rs::maybe_reduce_container_source -> src/symbol.rs::SymbolKind: 264efecb34de
---
# src/spec/source_text.rs
## Summary


## `ScopedDeps`
`pub(crate) struct ScopedDeps`
### Summary
The result of working out what one function or type depends on: the places in this repo it uses, and the outside packages it uses. The same data feeds both the written "Depends on" section and the context handed to the agent when it writes a spec.
### Behavior
`resolved` holds pairs of the target's symbol id and the import specifier that named it, for imports that point inside the repo. `externals` holds package names for imports that did not resolve, sorted and de-duplicated, and shortened to the package: `std::collections` becomes `std` and `next/navigation` becomes `next`. A relative import that resolved to nothing is kept verbatim, because a broken local import is worth seeing. The struct is plain data, visible within the crate only, and built by `scoped_symbol_deps`.
### Depends on
- (none)

## `scoped_symbol_deps`
`pub(crate) fn scoped_symbol_deps(
    graph: &Graph,
    from_file: &str,
    symbol_text: &str,
    self_id: Option<SymbolId>,
) -> ScopedDeps`
### Summary
Given one symbol's source text, finds which of its file's imports it actually mentions and sorts them into in-repo targets and outside packages.
### Behavior
Goes through every import in the graph and keeps those that come from `from_file`, whose imported name appears as a whole word in `symbol_text` (`contains_identifier`), and whose target is not `self_id`. The last check matters for the synthetic edge CodeOwl adds from a trait impl to its type in the same file: that edge's imported name is the type's own name, which the type's own text always contains, so without the exclusion a type would appear to depend on itself. No real import can target the symbol being scanned, so the check is always safe. A kept import with a resolved target adds the target's id and specifier to `resolved`. An unresolved one adds a package name to `externals`: a specifier starting with `.` is kept whole, anything else is cut at the first `/` or `:` (`std::fs` gives `std`, `next/navigation` gives `next`). Duplicates are skipped and `externals` is sorted at the end; `resolved` keeps graph order and is not de-duplicated. The text match is a heuristic: a name inside a comment or string would count too.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph

## `merged_symbol_spans`
`pub(crate) fn merged_symbol_spans(graph: &Graph, sym: &Symbol) -> Vec<[usize; 2]>`
### Summary
Finds all the line ranges of a file that together make up one symbol. Usually that is one block, but a Rust type whose methods sit in separate `impl` blocks further down the file has several. The ranges are what dependency scanning and generation tasks read.
### Behavior
Starts with the symbol's own line range. It then adds the range of every direct child that falls outside it (a folded-in method living in another `impl` block), and every entry in `extra_spans` (the full ranges of folded `impl` blocks, so the `impl Trait for Type {` header is included). A TypeScript class needs nothing extra because its methods are already inside its braces. The ranges are sorted by start line and merged when one starts within two lines of the previous end, so a `struct` followed by a blank line and its `impl` reads as one block. Callers can therefore rely on the first and last entries as the outer envelope. `symbol_span_text` and `get_source` share this function so their ranges cannot drift apart. It cannot fail; a child id that does not resolve is skipped.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::Symbol` — crate::symbol

## `render_spans`
`pub(crate) fn render_spans(content: &str, spans: &[[usize; 2]]) -> String`
### Summary
Cuts the requested line ranges out of a file's text and joins them into one string, with a blank line between ranges that are not next to each other.
### Behavior
Takes the file content already read into memory and a list of 1-based, inclusive `[start, end]` line ranges. It splits the content into lines once, then for each range slices from `start - 1` up to `end` and joins those lines with newlines. Between consecutive ranges it inserts a blank line (`\n\n`). Out-of-range values are clamped rather than rejected: a start of 0 is treated as line 1 and an end past the last line stops at the last line, so a stale range gives a short or empty piece instead of a panic. An empty span list returns an empty string. Taking the content as an argument lets callers that need it for something else, such as a hash check, read the file only once.
### Depends on
- (none)

## `symbol_span_text`
`pub(crate) fn symbol_span_text(
    root: &Path,
    graph: &Graph,
    rel_path: &str,
    sym: &Symbol,
) -> Result<String>`
### Summary
Reads the actual source code of one symbol from disk and returns it as a string. This text is both what dependency scanning searches and what an agent reads when writing that symbol's spec.
### Behavior
Computes the symbol's line ranges with `merged_symbol_spans`, reads the whole file at `root` joined with `rel_path`, and passes both to `render_spans`. If the file cannot be read (deleted, permissions, not UTF-8) it returns an error with the context "reading <path>", and callers decide how to react; `dependency_lines` turns it into an empty list. The text always comes from disk, not the index, so it reflects the current file even if the graph is a moment behind.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::Symbol` — crate::symbol
- externals: anyhow, std

## `cap_generation_text`
`pub(crate) fn cap_generation_text(text: String, max_bytes: usize) -> String`
### Summary
Shortens a piece of text so it fits a byte limit, and adds a visible note when it had to cut. This keeps a very large class from producing an agent task bigger than the MCP client can accept, without the agent being unaware that text is missing.
### Behavior
If the text is already within `max_bytes` it is returned unchanged. Otherwise the limit includes the truncation marker. The marker's length is reserved first, the text is cut to what remains, and the cut point is moved back until it lands on a character boundary, so a multi-byte UTF-8 character is never split. The marker is then appended into whatever room is left. If `max_bytes` is smaller than the marker itself, the marker is cut too (again on a boundary), so the result never exceeds the limit, even though the notice becomes partial. An earlier version cut the text to the limit and then added the marker on top, which overshot the limit by about 145 bytes; reserving the marker first fixed that. All subtractions are saturating or bounded, so tiny limits, including 0, cannot underflow or panic.
### Depends on
- (none)

## `maybe_reduce_container_source`
`pub(crate) fn maybe_reduce_container_source(
    sym: &Symbol,
    graph: &Graph,
    full_source: String,
    threshold: usize,
) -> String`
### Summary
For a very large class or similar type, replaces its full code with just an outline (each member's signature and doc comment) before it is shown to the agent writing the spec. A summary of the class needs to know what each member is for, not every line of every body.
### Behavior
Returns `full_source` unchanged unless the symbol is a `Container`, has at least one child, and its source is longer than `threshold` bytes. When all three hold, it builds the outline: the container's own signature, then its docstring on the next line if there is one, then for each child (separated by a blank line) the child's signature and docstring. A child id that does not resolve is skipped. There is deliberately no exception for private members, because the case that motivated this, a class with 240 public methods and 15 private ones, would gain little from it, and visibility rules differ per language. The reduced text is only for the task shown to the agent. Dependency scanning must keep using the full source, because a dependency used only inside an omitted body still has to be found.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::Symbol` — crate::symbol
- `src/symbol.rs::SymbolKind` — crate::symbol

## `contains_identifier`
`pub(super) fn contains_identifier(text: &str, name: &str) -> bool`
### Summary
Checks whether a name appears as a genuine, standalone identifier somewhere in a piece of text — not just as a substring buried inside a longer word.
### Behavior
Searches for every occurrence of `name` within `text`, and for each one checks the characters immediately before and after: neither may be an identifier character (a letter, digit, underscore, or `$`). Only when both sides pass — or the match sits at the very start/end of the text — does it count as a real whole-word match, avoiding a false positive like an import named `Label` wrongly matching inside `useLabel`. Returns `true` on the first genuine match found, `false` if the name is empty or never appears as a standalone identifier.
### Depends on
- (none)
