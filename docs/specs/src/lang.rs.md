---
kind: file
source_paths: [src/lang.rs]
file: { source_hash: aa4a56983460aff174ca497bff6fdf10736d5ddace764e125746fb7a7c8a70ff, deps_hash: b776fb3cc6c002ace828693051e3b97eaf436e4d510c078f8de4abe18e7eee17, spec_hash: 124fdf2b869e6fa2b6450d2879a6fb4557bd8f1dbfbc3100df11a1c7400d5563 }
symbols:
  src/lang.rs::SourceKind: { source_hash: 030d6675bccb66a7f8439dc3f04f8c310d7bd2c00ab00dac67bec882b8ca1a15, deps_hash: 2a9d2ea9db4f081ce47fc07c50dfff9c0fc7567dd30571980f7e3cddf762311f, spec_hash: 08edb275f1878607938f083cc6cbb91ede4bcd674b9302beb14a5f2bb80c54b6 }
  src/lang.rs::ts_parser: { source_hash: a4658a5c6fc19c35d73f060642c96c4e029a9fb4ac78f7d14c6f8a2ab39892cf, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3ca8125f4a6f16954cd35389b931e4f5254eed0644541aa93296a4246335abee }
  src/lang.rs::extract_symbols: { source_hash: 9884210b833cf488592ee7a0904ea061eae1990853e1d2d70a2a41e5ae137746, deps_hash: 7cb07719fc60cc0fb67d5ed31b14f24f3af3ddd28259bcce38cd092cb25b0b39, spec_hash: 7ee021e582a78ee15fba7d51d4857d2b120f1c07a0cc9e544cc759d9f8a9a8c5 }
  src/lang.rs::FileRole: { source_hash: 664e61d3b1ae6a13b12f0d090d22185e85254a0dee982702438d9a1223428088, deps_hash: 3cb515d9d23dd84457b02f8b3176d442ebe0ddad3d308cecd5753c3254b4da07, spec_hash: 29e48e7f6d21214c80cfe0d6b12678d6c1808fb319707ea11b66aea6266b96d5 }
  src/lang.rs::classify: { source_hash: 908bf0c8a6f2bb97bc5feed5c0cd0e4df84717e8c481b29b9f1ac2eed35b626f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: cb5f679c4c0e908c4665e90988e4a80a1825f3fdab8173f868ad5a56ba9148c5 }
  src/lang.rs::is_test_path: { source_hash: 43bd27fc12cb63b5b6e60e8b23cc75ba2d0bc82465e1ffb6cf530d089aab30e1, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a511957bab4e03e1d307eb16d4d08b01087428c3d50e9d8673f9d4e459a0ea28 }
  src/lang.rs::is_ui_primitive: { source_hash: 209f4445fe0023de0546da9717f126bbf2a38ed1753a94a50ca314462c748675, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 367ced1d96d513b98d77c48ddb63572c0bcfb1d984742211d1bf495007b8c895 }
  src/lang.rs::detect: { source_hash: bf5a35d9c182615746de8e548978dfcb8c4f4f396d653eb226ec749eb4974eef, deps_hash: a543d37c43576bc616f39ff9a81d12bca4bcf58e78c1e31ef694af10cdd81fc7, spec_hash: e5dd24299cf31ce13571b85bb5a954e2ca8886eb25c027af91bbee5977293e2a }
---
# src/lang.rs
## Summary
This file does two jobs. First, `detect` picks which language pack a repo uses, by counting each candidate's ordinary source files (skipping test and tooling folders so a stray fixture cannot mislead it) and requiring exactly one winner, with a clear error if there are none or several. Second, it holds the TypeScript pack's own basics: which extensions the import resolver tries, how a path maps to a source kind, which tree-sitter grammar (the parser CodeOwl uses to turn source text into a syntax tree) to load, which function extracts declarations, and the rules that recognise test code and shared UI building blocks. Two small vocabularies defined here are shared by every language: `SourceKind` (ordinary code versus a dedicated schema file) and `FileRole` (product code, UI primitive, test code, or machine-generated code), which decide how a file's spec is ordered and whether it counts as shared infrastructure. Everything else in the file is specific to TypeScript, and the TypeScript pack calls into it.

## `SourceKind`
`pub enum SourceKind`
### Summary
Says what sort of source file a path is: ordinary code to parse, or a dedicated schema file that only declares database tables. Which file extension means which is each language pack's own decision.
### Behavior
Two variants. `Code` is a normal source file, parsed by the pack's grammar and put through the import and flow-edge passes. `Schema` is a dedicated schema file (the TypeScript pack's `.sql`), parsed only for table declarations, and the indexer skips the import and flow-edge passes for it. A model class written inside a code file, such as an ORM class, is not this: it stays a `Code` file whose class is retagged as a table symbol by the pack. `SourceKind::of(rel_path)` is the TypeScript mapping: a path ending `.d.ts` returns `None` (type-declaration files have grammar shapes that are not handled), a path ending `.ts` or `.tsx` returns `Some(Code)`, and anything else returns `None`. Only the TypeScript pack calls it; the others match their own extensions directly, and the TypeScript pack maps `.sql` to `Schema` before falling back to this.
### Depends on
- `src/stack.rs::TypeScriptNextStack` — crate::stack

## `ts_parser`
`pub fn ts_parser(rel_path: &str) -> Parser`
### Summary
Builds a fresh tree-sitter `Parser` with the correct TypeScript grammar for a given file — the TSX grammar (JSX-aware) for `.tsx`, the plain TypeScript grammar for everything else. Every TypeScript-pack parse pass (`extract.rs`, `imports.rs`) starts by calling this.
### Behavior
Extension check on the string path: `.tsx` selects `LANGUAGE_TSX`, any other suffix selects `LANGUAGE_TYPESCRIPT`. `set_language` is `.expect`-ed — the grammar is compiled into the binary, so a failure here is a build problem, not a runtime one. Returns a ready-to-use owned `Parser`; callers get a new one each time rather than sharing, which sidesteps tree-sitter's non-`Sync` parser state. `.sql` never reaches here — `SourceKind` routes it to `schema.rs`'s own `tree-sitter-sequel` parser.
### Depends on
- externals: tree_sitter

## `extract_symbols`
`pub fn extract_symbols(rel_path: &str, source: &str) -> Vec<ExtractedSymbol>`
### Summary
Reads a TypeScript file and returns the declarations in it. It is a thin front door so the TypeScript language pack and the test helper in the graph module both go through one function.
### Behavior
It forwards to `extract::extract_file(source, rel_path)` and returns whatever list of extracted symbols that produces. There is no logic of its own and no error case. It handles TypeScript only: `.sql` files are dispatched elsewhere, in the pack, and this function does not check the extension itself.
### Depends on
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- `src/extract.rs::extract_file` — crate::extract

## `FileRole`
`pub enum FileRole`
### Summary
Says what kind of code a file holds, which decides how its spec is ordered in a generation run and whether it counts as shared infrastructure. Each language pack decides which paths get which role.
### Behavior
Four variants. `Domain` is ordinary product code, the default: it can be in the top tier of widely used shared code, it counts in import fan-in, and it is documented in normal priority order. `Primitive` is a low-level UI building block (`components/ui/*`), which is imported almost everywhere and so would dominate a fan-in ranking, but whose summary tells a dependent nothing it could not guess; it is kept out of the shared-code tier and still gets its own file spec in the long tail. `Test` is test code (an `e2e`, `cypress` or `playwright` tree, a `__tests__` folder, or a `.test.` or `.spec.` file): it stays in the graph so callers can see which tests use something, but its specs come last and it is never a product module. `Generated` is machine-written code; spec bearing is never granted to it, since specs describe hand-written code. The doc comment on the type says no path rule produces `Generated` yet, but the generated-source report and the spec code both act on it, so packs such as the Java one do assign it. Plain data, no logic.
### Depends on
- `src/stack.rs::StackPack` — crate::stack

## `classify`
`pub fn classify(path: &str) -> FileRole`
### Summary
Maps a repo-relative path to its `FileRole` — the JS/Next-convention implementation behind `TypeScriptNextStack::classify`. The one place `is_test_path` and `is_ui_primitive` are consulted.
### Behavior
First strips a leading `rollup:` or `feature:` coverage-id prefix, so a directory-rollup id (`rollup:lib/email`) classifies by its bare directory path. Then a fixed precedence: `Test` wins over `Primitive` wins over `Domain` (the default). Precedence matters for a file that matches two rules — a test file under a `ui/` path is `Test`, not `Primitive`. Never returns `Generated` — no Phase 1 path convention produces it.
### Depends on
- (none)

## `is_test_path`
`fn is_test_path(path: &str) -> bool`
### Summary
The JavaScript/TypeScript-convention test-file detector behind `classify` — recognises the standard e2e-runner trees and unit-test naming patterns.
### Behavior
Returns `true` if the path is in or under an `e2e/`, `cypress/`, or `playwright/` directory (checked both as a leading segment and mid-path), or contains `__tests__/`, `.test.`, or `.spec.` anywhere. Pure substring matching — no filesystem access, no awareness of a project's actual test config. It is deliberately JS-shaped: a Rust `tests/` tree or a Java `src/test/java` root matches nothing here, which is why `classify` is a pack-owned seam (`RustStack` supplies its own).
### Depends on
- (none)

## `is_ui_primitive`
`fn is_ui_primitive(path: &str) -> bool`
### Summary
Detects a shadcn/ui-style presentational primitive by path — a file in a `components/ui/` directory — so `classify` can keep it out of the shared-code tier despite its high import fan-in.
### Behavior
`true` if the path starts with `components/ui/` or contains `/components/ui/`; nothing else. A pure path convention, not a content check — it assumes the shadcn/ui layout where `components/ui/` holds only design-system leaves (`button.tsx`, `dialog.tsx`). A project that puts real logic there, or names the directory differently, would be misclassified — acceptable for Phase 1, and a pack-specific rule a `StackPack` eventually owns.
### Depends on
- (none)

## `detect`
`pub fn detect(root: &Path) -> Result<Box<dyn crate::stack::StackPack>>`
### Summary
Works out which language the repo is written in and returns the matching language pack, or fails with a clear message instead of quietly serving an empty index. It runs once at startup.
### Behavior
For each of the four candidates (TypeScript/TSX, Rust, Java, Python) it counts the files the pack would parse as ordinary code, skipping folders that do not define a repo's identity. Test folders (`tests`, `test`, `benches`, or any path containing `src/test/`) are skipped so a TypeScript fixture read by a Rust test cannot make a Rust repo look like TypeScript. Tooling folders (`utility`, `scripts`, `tools`, `hack`) are skipped so build scripts cannot make a repo look like it has a second language. The walk respects ignore files. A `.sql` schema file is not counted, since it is not ordinary code, so a Rust repo with migrations stays a Rust repo.

Packs with a count above zero are the hits. With exactly one hit, a fresh instance of that pack is returned. With none, it fails, saying no extractable source files were found, though that message lists only TypeScript, Rust and Java and leaves out Python. With more than one, it fails listing each stack and its file count and suggests pointing CodeOwl at a subdirectory, since one stack per repo is a deliberate limit. Because tests are skipped only for the count, they are still parsed once the pack is chosen.
### Depends on
- `src/stack.rs::StackPack` — crate::stack
- `src/stack.rs::TypeScriptNextStack` — crate::stack
- `src/stack.rs::RustStack` — crate::stack
- `src/stack.rs::JavaStack` — crate::stack
- `src/stack.rs::PythonStack` — crate::stack
- externals: anyhow, std
