---
type: file
title: "src/lang.rs"
description: "Specification for `src/lang.rs`."
generated: { by: CodeOwl }
resource: "../../../src/lang.rs"
source_paths: [src/lang.rs]
file: { source_hash: 265ae6a607d0aeb3634623f5573365d76832769b01ae177ad47707feaafda5e6, fingerprint: a2bb150eb112ce790fb6083c73df811365b95f63f3e9d109e77f6b5a304b45f1, deps_hash: 67a48b24191b515fc09307d7ac000c448b56f790acab54e1bc729df0768a141a, spec_hash: 2dab468e0eef6bfebf0a19bb31c9f1e136ec25ceb288fd087e02e7ffe0cbf961 }
symbols:
  src/lang.rs::SourceKind: { source_hash: 030d6675bccb66a7f8439dc3f04f8c310d7bd2c00ab00dac67bec882b8ca1a15, deps_hash: 2a9d2ea9db4f081ce47fc07c50dfff9c0fc7567dd30571980f7e3cddf762311f, spec_hash: 08edb275f1878607938f083cc6cbb91ede4bcd674b9302beb14a5f2bb80c54b6 }
  src/lang.rs::ts_parser: { source_hash: a4658a5c6fc19c35d73f060642c96c4e029a9fb4ac78f7d14c6f8a2ab39892cf, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3ca8125f4a6f16954cd35389b931e4f5254eed0644541aa93296a4246335abee }
  src/lang.rs::extract_symbols: { source_hash: 9884210b833cf488592ee7a0904ea061eae1990853e1d2d70a2a41e5ae137746, deps_hash: 7cb07719fc60cc0fb67d5ed31b14f24f3af3ddd28259bcce38cd092cb25b0b39, spec_hash: 7ee021e582a78ee15fba7d51d4857d2b120f1c07a0cc9e544cc759d9f8a9a8c5 }
  src/lang.rs::FileRole: { source_hash: 5b2895883885ea62dd71226ca330aa642b970ff9dcf29ca9eae78a6e9bc45051, deps_hash: 47273209ec5140c5f8d7af7935e26ab691768b33840826bbf90a615c5a4d23f0, spec_hash: 9903e681e9dbf53390d3bfc4c201dcef9af76399532e66ff169c2947b72764a0 }
dep_targets:
  file -> src/extract.rs::extract_file: fcab56f46e52
  file -> src/stack.rs::JavaStack: 3b4c6ab553d6
  file -> src/stack.rs::PythonStack: 398bcceb2112
  file -> src/stack.rs::RustStack: b893cf055d16
  file -> src/stack.rs::StackPack: 9350ff43aeca
  file -> src/stack.rs::TypeScriptNextStack: 64056e725621
  file -> src/symbol.rs::ExtractedSymbol: 314759561cf6
  src/lang.rs::SourceKind -> src/stack.rs::TypeScriptNextStack: 64056e725621
  src/lang.rs::extract_symbols -> src/extract.rs::extract_file: fcab56f46e52
  src/lang.rs::extract_symbols -> src/symbol.rs::ExtractedSymbol: 314759561cf6
  src/lang.rs::FileRole -> src/stack.rs::StackPack: 9350ff43aeca
---
# src/lang.rs
## Summary
This file does two jobs. First, `detect` picks which language pack a repo uses, by counting each candidate's ordinary source files (skipping top-level `tests`, `test`, `benches`, any `src/test/` path, and the tooling folders `utility`, `scripts`, `tools`, `hack`, so a stray fixture or build script cannot mislead it) and requiring exactly one winner. It fails with an error naming all four supported stacks if there are none, or listing the stacks found with their file counts if there are several. Second, it holds the TypeScript pack's own basics. `RESOLVER_EXTENSIONS` is the Node-style order the import resolver tries (`.ts`, `.tsx`, `.d.ts`, `.js`, `.jsx`, `.json`). `SourceKind::of` maps a path to ordinary code only for `.ts` and `.tsx`, and deliberately not `.d.ts`. `ts_parser` loads the tree-sitter grammar (the parser CodeOwl uses to turn source text into a syntax tree): the TSX grammar for `.tsx`, plain TypeScript for everything else, and it panics if the bundled grammar fails to load. `extract_symbols` hands a file to the TypeScript extractor. `classify` turns a path into a `FileRole`, after stripping a `rollup:` or `feature:` prefix so a coverage id classifies by its path. Test paths are `e2e`, `cypress` or `playwright` folders at the root or nested, any `__tests__/`, and any `.test.` or `.spec.` name; a UI primitive is anything under `components/ui/`, at the root or nested; test is checked first, and everything else is domain code. Two small vocabularies defined here are shared by every language: `SourceKind` (ordinary code versus a dedicated schema file, which the TypeScript pack uses for `.sql`) and `FileRole` (product code, UI primitive, test code, or machine-generated code assigned by the Java and Python packs), which decide how a file's spec is ordered and whether it counts as shared infrastructure. Everything else in the file is specific to TypeScript, and the TypeScript pack calls into it. The path rules are plain substring checks, so an unusual name like `my.test.helper.ts` counts as a test.

## `SourceKind`
`pub enum SourceKind`
### Summary
Says what sort of source file a path is: ordinary code to parse, or a dedicated schema file that only declares database tables. Which file extension means which is each language pack's own decision.
### Behavior
Two variants. `Code` is a normal source file, parsed by the pack's grammar and put through the import and flow-edge passes. `Schema` is a dedicated schema file (the TypeScript pack's `.sql`), parsed only for table declarations, and the indexer skips the import and flow-edge passes for it. A model class written inside a code file, such as an ORM class, is not this: it stays a `Code` file whose class is retagged as a table symbol by the pack. `SourceKind::of(rel_path)` is the TypeScript mapping: a path ending `.d.ts` returns `None` (type-declaration files have grammar shapes that are not handled), a path ending `.ts` or `.tsx` returns `Some(Code)`, and anything else returns `None`. Only the TypeScript pack calls it; the others match their own extensions directly, and the TypeScript pack maps `.sql` to `Schema` before falling back to this.
### Depends on
- (none)

## `ts_parser`
### Summary
Builds a fresh tree-sitter `Parser` with the correct TypeScript grammar for a given file — the TSX grammar (JSX-aware) for `.tsx`, the plain TypeScript grammar for everything else. Every TypeScript-pack parse pass (`extract.rs`, `imports.rs`) starts by calling this.
### Behavior
Extension check on the string path: `.tsx` selects `LANGUAGE_TSX`, any other suffix selects `LANGUAGE_TYPESCRIPT`. `set_language` is `.expect`-ed — the grammar is compiled into the binary, so a failure here is a build problem, not a runtime one. Returns a ready-to-use owned `Parser`; callers get a new one each time rather than sharing, which sidesteps tree-sitter's non-`Sync` parser state. `.sql` never reaches here — `SourceKind` routes it to `schema.rs`'s own `tree-sitter-sequel` parser.
### Depends on
- (none)

## `extract_symbols`
### Summary
Reads a TypeScript file and returns the declarations in it. It is a thin front door so the TypeScript language pack and the test helper in the graph module both go through one function.
### Behavior
It forwards to `extract::extract_file(source, rel_path)` and returns whatever list of extracted symbols that produces. There is no logic of its own and no error case. It handles TypeScript only: `.sql` files are dispatched elsewhere, in the pack, and this function does not check the extension itself.
### Depends on
- (none)

## `FileRole`
`pub enum FileRole`
### Summary
Says what kind of code a file holds, which decides how its spec is ordered in a generation run and whether it counts as shared infrastructure. Each language pack decides which paths get which role.
### Behavior
Four variants. `Domain` is ordinary product code, the default: it can be in the top tier of widely used shared code, it counts in import fan-in, and it is documented in normal priority order. `Primitive` is a low-level UI building block (`components/ui/*`), which is imported almost everywhere and so would dominate a fan-in ranking, but whose summary tells a dependent feature spec nothing it could not guess; it is kept out of the shared-code tier and still gets its own file spec in the long tail. `Test` is test code (an `e2e`, `cypress` or `playwright` tree, a `__tests__` folder, or a `.test.` or `.spec.` file): it stays in the graph so callers can still see which tests use something, but its specs sort last and it is never treated as a product module. `Generated` is machine-written code, assigned by the pack: the Java pack uses it under `target/generated-sources` and `build/generated`, and the Python pack for Alembic migration versions. It is never spec-bearing, since specs describe hand-written code. Plain data, no logic; the classification itself lives in the packs.
### Depends on
- (none)
