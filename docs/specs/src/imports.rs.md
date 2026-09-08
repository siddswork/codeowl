---
kind: file
source_paths: [src/imports.rs]
file: { source_hash: 4e1d1226c6720c19d926fede66b617af4e3850eac0a6f4fc5429d132634b7572, deps_hash: a1964b774915f48932ea74e02999d3d269ef7b4814dc96c0fd8d0cd974b40435, spec_hash: 32299632f03d94b82e32fd628aea636e8293392bdf2d2f26bb4d4550ee29160d }
symbols:
  src/imports.rs::ImportRef: { source_hash: 1b927e9c131cb384bfcb2056471bbff21be0d27d99a0b8776ad2e44426a26fb3, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 1860e729b7aa5e1e4bd5db4750f93c5261552e31bf75b0e8842e50dad5a0b2c9 }
  src/imports.rs::ReExport: { source_hash: 6e93cb94dbb7cea61c279750b51f567ef99f6b1e0335f075eeb27206603f8778, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: d100b1e8a87ac853d5fe8e5a4c887c21e47e3110022e13f2ea339c5f657983c0 }
  src/imports.rs::DefaultImport: { source_hash: af1be68cb44e0a21c69353a88ad69f449922eb0ea7fa7780120819d4dd7fd116, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a70f0c43e5df13423e65cf4a89306288e889fbb7cdc9e68e8312f8a2b290d008 }
  src/imports.rs::FileImports: { source_hash: dfdc5f0691f8f26444a4da8718e6cce982ac6c50d2ede25422c96514f20679d4, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 23a09100cd719ac416786a680dcfcdd68487f136e60b3d1c24667eeb52db4f39 }
  src/imports.rs::extract_imports: { source_hash: fc2cdab27d82c0d0f1fb8c0bd435d2ce54c4d94385c5cf9129d9727858b84af3, deps_hash: a1964b774915f48932ea74e02999d3d269ef7b4814dc96c0fd8d0cd974b40435, spec_hash: 14a4add6341926c6cc1bf32d8f037d0128b1ebb024eeba8c496c5d59d63bb582 }
  src/imports.rs::visit_import: { source_hash: 3789ebd450c51020d52120119a8f3e0fa544a4fdddc6a8b22612ea6ffd674024, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 841e1a0ec4883dd97864d6219265d7d03f3440010723c27447341c510d9065db }
  src/imports.rs::visit_maybe_reexport: { source_hash: 879ed65f55f1eebb887827501ecdd3e20f67f51756b4463b0efc596e72c98602, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 8dabfde9927059389ba79aaaaaf835b9a3d824a9ca52bbc62bef81be7c1f8d44 }
  src/imports.rs::child_of_kind: { source_hash: 1620bcf21d4d0e24f1c17a7845829036675acd2ccc2d0ed00a855156e9ab0266, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 75e9280c84eb249544dea6768ba99b9318dd8770035e82019fa968b10b42cfd2 }
  src/imports.rs::text: { source_hash: f2f836c691a8002927cc493e38650603f8fc64232d33afeb1b8f41f9279d1f2f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 9c9762e818e8d171ef84278958b8becf4196b7b183b3ad304cc3cbb01e83c148 }
  src/imports.rs::field_text: { source_hash: 49fcca597c11dccf7ad16445d0a55ff9466165dfa7161782ba4be1054a28e01f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 2b3e3ccfe6d2cbacf1bfc1245bcf6ab8f9d7d0ddbc85cd95765c43120d6fc587 }
  src/imports.rs::string_field: { source_hash: 42467b736c4d816d098e75165a41a7b59f91dd2ed1ad24b9d9b55ba12a5166c6, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: e4705e8daf7fe8f382d71cdd1b703f2937464a2d3f026e3ac54252a5fac8e437 }
---
# src/imports.rs
## Summary
The TypeScript pack's import parser — one of the two tree-sitter passes over a source file (the other being `extract.rs`'s symbol pass), kept separate so each stays single-purpose. It walks only the top-level `import`/`export ... from` statements and produces a `FileImports` holding three lists: named imports (`ImportRef`), named re-exports (`ReExport`, how a barrel forwards a name), and default imports (`DefaultImport`). Named forms resolve to a target symbol; default imports resolve only to a target file, and exist solely for M11's rendered-component matcher. Namespace imports (`import * as`) and wildcard re-exports (`export * from`) are deliberately untracked, since neither names a single declaration. The small `child_of_kind` / `text` / `field_text` / `string_field` helpers wrap tree-sitter's cursor API and quote-stripping. This whole module is TypeScript-specific — a second stack (`RustStack`) writes its own import parser rather than reusing this.

## `ImportRef`
`pub struct ImportRef`
### Summary
One named import parsed out of a file — the specifier it was imported from (`'./x'`, `'@/lib/y'`) and the name that was imported. The unit the resolver turns into a file-to-file reference edge.
### Behavior
A plain data pair, `Serialize`/`Deserialize` so it can be cached in `RepoIndex` alongside the file's other inputs. Only the *source* name is stored: `import { Foo as Bar }` records `Foo`, because the local alias `Bar` is irrelevant to resolving what the import points at. Default imports and namespace/wildcard imports are tracked separately (or not at all) — this type is exclusively for the `{ named }` form.
### Depends on
- (none)

## `ReExport`
`pub struct ReExport`
### Summary
One named re-export — `export { source_name as exported_as } from 'specifier'` — the mechanism a barrel file uses to forward a name it doesn't declare itself.
### Behavior
Keeps both names because they serve different roles in resolution: `exported_as` is what a consumer importing from the barrel asks for, `source_name` is what to look up in the file the barrel forwards from. The resolver follows these chains one hop when a direct lookup in the target file fails. Cached in `RepoIndex` with the rest of a file's import inputs.
### Depends on
- (none)

## `DefaultImport`
`pub struct DefaultImport`
### Summary
One default import — `import Local from 'specifier'`, or the default half of `import Local, { ... } from 'specifier'`. Tracked so the M11 rendered-component resolver can match a `<Component/>` tag to the file it comes from.
### Behavior
Stores the local binding name and the specifier only. Unlike a named import, this resolves at the *file* level, not to a specific exported symbol: React components are nearly always default exports and the export name inside the target file is arbitrary, so `resolve::resolve_default_imports` just records "this local name resolves to that file". Used only for the rendered-component flow edge — a default import to a non-component module carries no further meaning in the graph.
### Depends on
- (none)

## `FileImports`
`pub struct FileImports`
### Summary
Everything one file's `import`/`export ... from` statements contribute to the reference graph: its named imports, its named re-exports, and its default imports. The per-file unit `extract_imports` produces and `RepoIndex` caches.
### Behavior
Three `Vec`s, one per import shape. `default_imports` is `#[serde(default)]` because it was added in M11 after the cache format already existed, so an older cache deserializes it as empty. Derives `Default`, so a pack with no import concept (or a schema file) can return an empty `FileImports` trivially. `resolve_imports` consumes the whole map of these (path → `FileImports`) to produce resolved edges once the arena exists.
### Depends on
- (none)

## `extract_imports`
`pub fn extract_imports(source: &str, rel_path: &str) -> FileImports`
### Summary
The TypeScript pack's import extractor: parse a file and pull out its named imports, named re-exports, and default imports into a `FileImports`.
### Behavior
Runs its own tree-sitter parse (separate from `extract.rs`'s symbol pass — two parses of a small file is negligible and keeps each pass single-purpose), then walks only the top-level children of `program`, dispatching `import_statement` to `visit_import` and `export_statement` to `visit_maybe_reexport` (which ignores an `export` that isn't a re-export from another module). A parse failure returns an empty `FileImports` rather than erroring. Namespace imports (`import * as ns`) and wildcard re-exports (`export * from`) are deliberately not tracked — they name no single declaration.
### Depends on
- `src/lang.rs::ts_parser` — crate::lang

## `visit_import`
`fn visit_import(node: Node, source: &str, out: &mut FileImports)`
### Summary
Handle one `import_statement` node: pull its specifier string, then record a `DefaultImport` for any default binding and an `ImportRef` for each `{ named }` entry.
### Behavior
Returns early on two shapes that contribute nothing: an import with no source string, and a side-effect-only import (`import './polyfill'`, which has no `import_clause`). The default binding is detected as a bare `identifier` directly under the clause — covering both `import Foo from` and the `Foo` in `import Foo, { Bar } from`. `import * as ns` appears as a `namespace_import` child and is deliberately skipped. Each `import_specifier` in the `named_imports` group becomes one `ImportRef` keyed on the `name` field (the source name, not any `as` alias). All results are appended to the shared `out: &mut FileImports`.
### Depends on
- externals: tree_sitter

## `visit_maybe_reexport`
`fn visit_maybe_reexport(node: Node, source: &str, out: &mut FileImports)`
### Summary
Handle one `export_statement` node, recording a `ReExport` for each name in an `export { ... } from 'other'` — and doing nothing for any other kind of export.
### Behavior
Two early returns rule out the non-re-export cases: no `source` string means it's an ordinary local `export function`/`class`/`const` (extracted by `extract.rs`, not here), and no `export_clause` means it's `export * from '...'`, a wildcard the resolver deliberately doesn't chase. For each `export_specifier`, `source_name` is the `name` field and `exported_as` is the `alias` field when present, else the same as `source_name` (a plain `export { X } from` re-exports `X` under its own name). Results append to `out.re_exports`.
### Depends on
- externals: tree_sitter

## `child_of_kind`
`fn child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>>`
### Summary
Find the first direct child of a tree-sitter node with a given kind string.
### Behavior
A one-line helper wrapping the cursor dance `Node::children` requires. Returns `None` when no child matches. Used throughout the import walk to reach into `import_clause` / `named_imports` / `export_clause` without matching on every child.
### Depends on
- externals: tree_sitter

## `text`
`fn text<'a>(node: Node, source: &'a str) -> &'a str`
### Summary
The source slice a tree-sitter node spans, as `&str`.
### Behavior
Wraps `Node::utf8_text`, substituting an empty string for the (practically unreachable) UTF-8 error case so callers don't have to handle it. The returned slice borrows `source`, not the node's `Tree`, so it's safe to keep after the tree is dropped.
### Depends on
- externals: tree_sitter

## `field_text`
`fn field_text<'a>(node: Node, field: &str, source: &'a str) -> Option<&'a str>`
### Summary
The source text of a named field child (`node.child_by_field_name(field)`), or `None` if that field is absent.
### Behavior
Combines the field lookup and the UTF-8 text extraction into one call. Used to read `name` / `alias` off an import/export specifier without an intermediate `Node`.
### Depends on
- externals: tree_sitter

## `string_field`
`fn string_field(node: Node, field: &str, source: &str) -> Option<String>`
### Summary
Like `field_text`, but for a field whose node is a string literal — returns the content with the surrounding quotes stripped, as an owned `String`.
### Behavior
`trim_matches(['"', '\''])` removes both single and double quote characters from each end. Used for the `source` field of an import/export (the module specifier), which tree-sitter reports with its quotes included.
### Depends on
- externals: tree_sitter
