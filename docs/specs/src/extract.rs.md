---
kind: file
source_paths: [src/extract.rs]
file: { source_hash: 1d6514a4bcd2f3ec3762c653552be2c41ad78971149d2907956c3e306215066a, deps_hash: 0f10f465419d7111a155630ce566adae74b00a3cc44b63273460f5043a64d027, spec_hash: ea3bf4f8f58b0b2352e3190bbd49937d8e4d4f446b6e11e79ac0e92652813f13 }
symbols:
  src/extract.rs::extract_file: { source_hash: 0f80395f2d0895f891b54861868cd690539d694d6d078d92f5a259097e41129a, deps_hash: 7a249fa56fb8f71895f3311c45ae988dd0d5426fe72cd08f212c8d33029b1253, spec_hash: 279969477ef8721fea255ee14212e68963dc06b4a240e69dbe147b94a9bba0bb }
  src/extract.rs::visit_top_level: { source_hash: b3494f71abb74cf7a8026cae0f59dba426184803ab1f71dcbc82e43f6275f6e8, deps_hash: 57b36ccf02abf241d59cf027b8af996e4e37cdf048813be00047a453efba7ce6, spec_hash: 1dd94bea92fd0545b5f38902f2e10ffd1b1caa3be50f1d90dede5779a7826f70 }
  src/extract.rs::visit_function: { source_hash: 9ad2d57f1d77239149dba7a198136947d6450d6d458235f2b4ee71bf1344f292, deps_hash: 05b2db27555b24c9f8b2464bb5722a3dae6651742c01cb85432c3f1b12383bb5, spec_hash: 3ed706af3b63d5c82ecaad80ec7bed1da6c783fbf1b319572cabceb26ad4d82e }
  src/extract.rs::visit_class: { source_hash: e51d518d683ec6a2c7313b3f5c560726a401ef0dd2ff81aa8baa686848512947, deps_hash: 05b2db27555b24c9f8b2464bb5722a3dae6651742c01cb85432c3f1b12383bb5, spec_hash: 0aa92348644fca449bef76f23e19b29417bb15d148f936d7cbecc203cf009690 }
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
The TypeScript extractor's entry point: parses one file's source text and returns its list of top-level declarations.
### Behavior
Picks a parser grammar based on `rel_path`'s extension (`.tsx` gets JSX support, everything else parses as plain TypeScript), then parses `source` into a syntax tree (the structure `tree-sitter` produces from source text). If parsing fails outright, it returns an empty list rather than erroring. Otherwise it walks each of the tree's top-level children and hands them to `visit_top_level` to build up the symbol list.

The parsed tree itself never escapes this function — every value pushed into the result is an owned `String`/`usize` extracted while the tree is still alive, not a reference into it. That's deliberate: the tree's node type only exists validly as long as the tree that produced it is alive, so returning one (or a struct holding one) would tie the caller's lifetime to this function's internal parse tree, which extraction across a whole repo can't afford.
### Depends on
- `src/lang.rs::ts_parser` — crate::lang
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- externals: tree_sitter

## `visit_top_level`
`fn visit_top_level(node: Node, source: &str, file: &str, out: &mut Vec<ExtractedSymbol>)`
### Summary
Handles one direct child of the file's top level: unwraps an `export` if present, works out what kind of declaration it actually is, and extracts whatever symbols it produces.
### Behavior
If the node is an `export_statement`, it looks for the inner declaration it wraps. Some export forms have no declaration to find at all — `export { X } from './y'`, `export * from './y'`, or an anonymous `export default <expr>` (an arrow function, a bare identifier, a literal) — since nothing new is actually declared there; a barrel file (a file whose only job is forwarding names from elsewhere) is made up entirely of this shape, so this function simply returns without extracting anything for it. A *named* default export (`export default function Page() {}`, `export default class Foo {}`) does have an inner declaration and falls through to ordinary handling like any other declaration — this matters because a Next.js page or route component is routinely written exactly this way, so it needs to be extracted, not skipped.

Once it has the actual declaration node (whether or not it was wrapped in an export), it dispatches on its kind: a function declaration, class declaration, or lexical declaration (`let`/`const`/`var`) each go to their own dedicated visitor. Any other declaration kind is silently ignored.
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
Turns a `function` declaration node into an `ExtractedSymbol`.
### Behavior
If the declaration has no body at all, it's skipped (nothing is pushed) — this guards against a malformed or incomplete parse rather than a real code shape. Otherwise it reads the function's name (falling back to `"<anonymous>"` if none), builds its `id` as `"<file>::<name>"`, and computes its signature text from the declaration node up to (but not including) the body.

Whether the symbol is exported is read from the *outer* node (whether this declaration sits inside an `export_statement`), but the signature text itself is taken from the declaration node alone, deliberately excluding any `export`/`export default` prefix — this keeps it consistent with how a const/arrow-function declaration's signature is computed, where leaving the prefix off is simply the cheaper choice. `source_hash` covers the whole declaration's text; `interface_hash` is only computed (as a hash of just the signature) when the function is exported — an unexported function isn't a valid target for cross-file resolution, so it has no public shape worth tracking separately from its full source. The docstring is read from whatever comment immediately precedes the *outer* node (so a comment above `export function foo()` is still picked up, not just one directly above `function foo()`).
### Depends on
- `src/hash.rs::hash_text` — crate::hash
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- `src/symbol.rs::SymbolKind` — crate::symbol
- externals: tree_sitter

## `visit_class`
`fn visit_class(decl: Node, outer: Node, source: &str, file: &str, out: &mut Vec<ExtractedSymbol>)`
### Summary
Turns a `class` declaration into an `ExtractedSymbol` container, plus one member symbol for each of its methods and fields, folding those members' hashes into the class's own — this is the piece that makes editing a method or field register as a real change to the class that contains it.
### Behavior
If the class has no body, it's skipped entirely. Otherwise it reads the class name (falling back to `"<anonymous>"`), builds the class's own id as `"<file>::<name>"`, and walks the class body once, producing one member symbol per method (`method_definition`) and per field (`public_field_definition` — this grammar node covers plain fields, JS-private `#`-prefixed fields, and `private`-modified fields alike; a member's own visibility never makes it independently exported). Each member's id is `"<class_id>.<member_name>"`, deduplicated against sibling members that would otherwise collide on the same name. A method with no body is skipped the same way the class itself would be. Every member is `is_exported: false` — a method or field is never independently importable on its own, only reached through an already-imported class — and every member's `parent` points back at the class's id.

A field's `signature` is computed by a dedicated helper that strips its initializer value off, keeping only its name and type; a method's signature is its declaration text up to its body, the same as a top-level function's.

Once every member is collected, two separate rollups are built. `source_hash` folds the class's own signature together with every member's `source_hash`, in declaration order — reordering members is itself treated as a real change, not just editing one. That's what makes hash propagation up the containment chain work: editing one member moves both that member's own `source_hash` and the class's. `interface_hash` — computed only when the class itself is exported, since an unexported class has no public shape to track — folds in only each *public* field's own signature (name and type, never the docstring or the initializer value), skipping every method and skipping any field that isn't public; this rollup is built lazily, only once the class is known to be exported, since building it unconditionally would waste an allocation and a full member scan on every reparse of every non-exported class, and the file watcher reparses on every save.

The class symbol itself is pushed first, with `children` set to the ordered list of member ids, and its own members follow immediately after in the output list.
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
