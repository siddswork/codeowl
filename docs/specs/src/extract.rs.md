---
kind: file
source_paths: [src/extract.rs]
file: { source_hash: 1d6514a4bcd2f3ec3762c653552be2c41ad78971149d2907956c3e306215066a, deps_hash: 0f10f465419d7111a155630ce566adae74b00a3cc44b63273460f5043a64d027, spec_hash: ea3bf4f8f58b0b2352e3190bbd49937d8e4d4f446b6e11e79ac0e92652813f13 }
symbols:
  src/extract.rs::extract_file: { source_hash: 0f80395f2d0895f891b54861868cd690539d694d6d078d92f5a259097e41129a, deps_hash: 4949265e5240911cae93183bd11990ee502d1e11aecc647e857b0fbedd82bcaf, spec_hash: e574a141f311f0ecdb45e14da2cc16ae9c5095e942c9884b51f3727e9af088fd }
  src/extract.rs::visit_top_level: { source_hash: b3494f71abb74cf7a8026cae0f59dba426184803ab1f71dcbc82e43f6275f6e8, deps_hash: ce7d9d7fc16420378d45b890370f2eb5a11a122aee5b39ac6d807d291364fb0f, spec_hash: 4879ec7c6af12343e2fb5e38cbf5ac53ef5c68e0e2e96e1f626b07e8633f7cdb }
  src/extract.rs::visit_function: { source_hash: 9ad2d57f1d77239149dba7a198136947d6450d6d458235f2b4ee71bf1344f292, deps_hash: d4034a9cbee7900a1fbe5e4401fc040618db110c2395b587568f12985ac649df, spec_hash: 66ff4314875df3ae8b5fd127633c816a73fa99d7ee7f84df5d8ed49e6f420ddb }
  src/extract.rs::visit_class: { source_hash: 2ef8a36d935b9dbe1708ff6272ef9e91c7087d20cba796b2f84b10cde2bcfca2, deps_hash: d4034a9cbee7900a1fbe5e4401fc040618db110c2395b587568f12985ac649df, spec_hash: b9636abe9f62fa633e0d2a0971fc463a7ee6f7a88ed4bf2463f2999f07560faa }
  src/extract.rs::visit_lexical: { source_hash: e7406937d062fe58270abf2f9af8753d60b92117c086b4f91015b0ccf13ddbd4, deps_hash: d4034a9cbee7900a1fbe5e4401fc040618db110c2395b587568f12985ac649df, spec_hash: c4c8cb249e3ab4180b0f01d197d0d67face556f26a86f26dfbf80f109ac23c5b }
  src/extract.rs::signature_text: { source_hash: 9596e4dcd49fc9e8ea493b1420cff5c22b200725f73ff1b6a7d2b2d82ecd097b, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b0cae5ed6ae3bd5f730fac83830eef5216066e4a6cf4707566b12e75523e96b4 }
  src/extract.rs::leading_doc: { source_hash: 7f4bd6766ea1eab0417ea2e54f3e1b922a1d6b297bc223ec3aacd4a3079cf6fa, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a4791775f2e9d962c3992e11ea46cfa74fb8a8f2678229608c2a7fec065f8c83 }
  src/extract.rs::clean_comment: { source_hash: 1df018000c9a0de5ddab711125a125cedff410e550fe2b2f1d2652163b214e10, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 5a04d28635d7efb76eae5dd28073f74822a0eb2c5d74b44d5114bc9a1f5c4eb0 }
  src/extract.rs::field_text: { source_hash: 2a5140d871a8f94a462c9ede1e35d0cc3fc4e40157ee85e7866651beb6747233, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a88de7ed3d89b62b60a1a1e7c0db24ba134aae3752fa1bff14472ef6924b4f0f }
  src/extract.rs::text: { source_hash: f2f836c691a8002927cc493e38650603f8fc64232d33afeb1b8f41f9279d1f2f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 824d5565c791d27394ca7293daf4cb7fc831cda79c77534aa5ddb36294da989d }
  src/extract.rs::node_lines: { source_hash: 30cdd2645c25a8d4ce9ab107ed17c80f226a84814ba9d5294ac6d3666e6cd0f9, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: e6e0a0ed01636bab112b2154dad013d2c1ae8a09337dcbcf5725d1d31573af1a }
---
# src/extract.rs
## Summary
Walks a single TypeScript/TSX file with tree-sitter and pulls out its top-level declarations — the TypeScript pack's counterpart to `rust.rs`. Deliberately shallow: only the syntax root's direct children (unwrapping `export`), plus one level into a class body for its methods. Nested closures and callbacks aren't declarations CodeOwl would ever spec. `extract_file` is the entry; `visit_top_level` dispatches to `visit_function` (a `Callable`), `visit_class` (a `Container` + method children, with a Merkle-rollup `source_hash`), or `visit_lexical` (a `const` — `Callable` if it holds an arrow function, else `Value`; `let`/`var` and destructures are skipped). The rest are small tree-sitter helpers (`signature_text`'s "text before the `{`" trick, `leading_doc`'s adjacency-checked comment run, `clean_comment`, `node_lines`). Every `node.kind()` string here is TypeScript-specific — a Phase-2 seam.

## `extract_file`
`pub fn extract_file(source: &str, rel_path: &str) -> Vec<ExtractedSymbol>`
### Summary
The TypeScript pack's symbol extractor — parses one file and returns its top-level declarations as `ExtractedSymbol`s with string parent/child links. The counterpart to `rust::extract_file`.
### Behavior
Builds a parser via `ts_parser` (grammar chosen by extension), parses, and returns an empty vec on parse failure. Then walks only the *direct children* of the syntax root, handing each to `visit_top_level` — a deliberately shallow walk (top-level items plus one level into a class body), not a full descent. Every value pushed is owned (`String`/`usize`), extracted while the `tree` is still alive; no `Node` escapes the function — per CLAUDE.md, storing one would tie its holder to the `Tree`'s lifetime.
### Depends on
- `src/lang.rs::ts_parser` — crate::lang
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- externals: tree_sitter

## `visit_top_level`
`fn visit_top_level(node: Node, source: &str, file: &str, out: &mut Vec<ExtractedSymbol>)`
### Summary
Handles one direct child of the syntax root: unwraps an `export` wrapper, then dispatches on declaration kind to the right `visit_*` helper.
### Behavior
If the node is an `export_statement`, it takes the `declaration` field — and returns early if there isn't one. That early return is the barrel-file case: `export { X } from './y'`, `export * from './y'`, and an *anonymous* `export default <expr>` declare nothing new. A *named* default export (`export default function Page()`, `export default class Foo`) does have a `declaration` and falls through — Next.js pages and route handlers take that shape and the feature layer depends on them being extracted. Then a `match` on kind: `function_declaration` → `visit_function`, `class_declaration` → `visit_class`, `lexical_declaration` (a `const`/`let`) → `visit_lexical`; anything else (a type alias, interface, enum) is ignored — M1 scope.
### Depends on
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- externals: tree_sitter

## `visit_function`
`fn visit_function(
    decl: Node,
    outer: Node,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
)`
### Summary
Turns a `function_declaration` node into one `Callable` `ExtractedSymbol`.
### Behavior
Bails if the node has no `body` (a `declare function` ambient signature). Name falls back to `"<anonymous>"`. `is_exported` is whether the *outer* node was an `export_statement`. `interface_hash` is set only when exported (a private function is never a reference-edge invalidation key). Two deliberate choices: `source_hash` and `signature` are taken from `decl`, not `outer`, so the `export` / `export default` prefix is excluded — consistency with the arrow-function case where including it would be awkward — and export-ness is carried in its own `is_exported` field instead. `parent` is `None` (top-level; `Graph::build` later sets it to the file). `markers` is empty — the TS pack doesn't use them.
### Depends on
- `src/hash.rs::hash_text` — crate::hash
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- `src/symbol.rs::SymbolKind` — crate::symbol
- externals: tree_sitter

## `visit_class`
`fn visit_class(decl: Node, outer: Node, source: &str, file: &str, out: &mut Vec<ExtractedSymbol>)`
### Summary
Turns a `class_declaration` into one `Container` symbol plus a `Callable` child per method — the containment unit `spec.rs` treats as one section (and the shape `rust::merge_inherent_impls` later makes a folded Rust type match).
### Behavior
Bails without a `body`. Walks `method_definition` members (skipping any without a body — an overload signature), each becoming a `Callable` with id `<class>.<method>`, `parent` set to the class, `is_exported: false` (a method is never independently imported), and `interface_hash: None`. The class's own `source_hash` is a **Merkle rollup**: the signature followed by every method's `source_hash` in declaration order — so editing one method body (or reordering methods) moves both that method's hash and the class's. The class `interface_hash` deliberately does *not* fold in method signatures — M2 only resolves file-to-file edges, nothing watches a class's members. Methods are pushed to `out` after the class, so declaration order in the vec is parent-then-children.
### Depends on
- `src/hash.rs::hash_text` — crate::hash
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- `src/symbol.rs::SymbolKind` — crate::symbol
- externals: tree_sitter

## `visit_lexical`
`fn visit_lexical(
    decl: Node,
    outer: Node,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
)`
### Summary
Handles a `const` declaration — emitting a `Callable` symbol for `const f = () => {}` (an arrow/function expression) or a `Value` symbol for a plain data const.
### Behavior
Returns immediately unless the declaration keyword is `const` — `let` and `var` aren't spec-worthy. Iterates `variable_declarator`s, skipping any whose `name` isn't a plain `identifier` (a `const { a, b } = …` destructure has no single name to attach a symbol to). For each: if the value is an `arrow_function` / `function_expression`, it's a `Callable` (`raw: "function"`) with signature `const <name> = <params/return>`; otherwise a `Value` (`raw: "const"`) whose signature is `const <name><: type>` — the *declared type annotation* if present, never the literal value, so a type change moves `interface_hash` but a value-only edit doesn't. `is_exported` from the outer node; `interface_hash` set only when exported.
### Depends on
- `src/hash.rs::hash_text` — crate::hash
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- `src/symbol.rs::SymbolKind` — crate::symbol
- externals: tree_sitter

## `signature_text`
`fn signature_text(node: Node, body: Node, source: &str) -> String`
### Summary
Extracts a declaration's signature as the source text from its start up to (not including) its body — the "everything before the `{`" trick.
### Behavior
Slices `source[node.start .. body.start]` and trims trailing whitespace. Picks up modifiers, name, type parameters, params, and return type without naming each field, and works identically for `function_declaration`, `method_definition`, `class_declaration`, and arrow functions (where the "body" may be a braceless expression). `end` is `.max(start)` as a guard against a body that somehow starts before the node — the result is then an empty string rather than a panic.
### Depends on
- externals: tree_sitter

## `leading_doc`
`fn leading_doc(node: Node, source: &str) -> Option<String>`
### Summary
The docstring for a declaration — the contiguous run of comment lines immediately above it, with `//` / `/* */` / `/** */` markers stripped.
### Behavior
Walks backward over preceding siblings while each is a `comment` whose end row is exactly one above the running "expected end" row — an **adjacency** check. That check is the point: `prev_sibling()` skips blank lines, so without it a comment several paragraphs above an unrelated declaration would be misattributed. Once the run is collected it's reversed into source order, each comment passed through `clean_comment` to strip markers, and joined with newlines. `None` if the nearest preceding sibling isn't an adjacent comment.
### Depends on
- externals: tree_sitter

## `clean_comment`
`fn clean_comment(raw: &str) -> Vec<String>`
### Summary
Strips comment syntax from one raw comment node down to its content lines, dropping any that are blank after stripping.
### Behavior
Recognises `/** … */`, `/* … */`, and `//` — in that order, so a JSDoc block is matched before a plain block comment. A raw string matching none of these (an unterminated block) yields an empty vec. Each inner line is trimmed, then has a leading `*` stripped (JSDoc continuation), then trimmed again; empty results are filtered out. So a three-line JSDoc block returns up to three content strings, marker-free.
### Depends on
- (none)

## `field_text`
`fn field_text<'a>(node: Node, field: &str, source: &'a str) -> Option<&'a str>`
### Summary
The source text of a named tree-sitter field (`"name"`, `"kind"`, `"type"`, …), or `None` if the node has no such field.
### Behavior
`child_by_field_name(field)` then `text(...)`. A one-liner that keeps the callers (`visit_function`, `visit_lexical`, …) from repeating the `map` on every field access.
### Depends on
- externals: tree_sitter

## `text`
`fn text<'a>(node: Node, source: &'a str) -> &'a str`
### Summary
A local helper: the source substring a tree-sitter node spans, as a borrowed `&str`.
### Behavior
`node.utf8_text` against the source bytes, `""` on error (unreachable for a tree parsed from a valid UTF-8 `&str`). The same convenience wrapper `features.rs` and `rust.rs` define privately.
### Depends on
- externals: tree_sitter

## `node_lines`
`fn node_lines(node: Node) -> [usize; 2]`
### Summary
A node's line range as `[start, end]`, 1-indexed and inclusive — the `lines` field on every extracted symbol.
### Behavior
Tree-sitter rows are 0-indexed, so both are `+ 1`. Inclusive of the end line. This is the range `spec.rs`'s `symbol_span_text` slices the file by to build a symbol's generation context.
### Depends on
- externals: tree_sitter
