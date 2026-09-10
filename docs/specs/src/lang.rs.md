---
kind: file
source_paths: [src/lang.rs]
file: { source_hash: fffdeb3bbc079db8c800ebf3f48b2cd2fa8f66a1b1182f400ae6d2f27015f655, deps_hash: 8327ad3224849edf0d1a55c3a19add2d918ce0f21cd578b0bfc847eb4064e594, spec_hash: 2b189edb963a705adab19eb9e9147995a9cfe0b6e4d18edaf3f5027f71e64b29 }
symbols:
  src/lang.rs::SourceKind: { source_hash: 71f79d3a8773c4e818baa090f23294a54504f8a0d201f005e1bef8efcb48801f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a9c5de3c32b7f2c9af3acf91d5cf587768ea607a6183ec4a6d7a1caac4ca4f8a }
  src/lang.rs::is_extractable: { source_hash: 832112a79779a048177b44c06d6b3e5e65c840538b69f5a3e30b1a618cd6ebef, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 4224b3a3f47674e4c25cd37c32d2feba06b60b52b31e5c5e1b25486b78bd2f0d }
  src/lang.rs::ts_parser: { source_hash: a4658a5c6fc19c35d73f060642c96c4e029a9fb4ac78f7d14c6f8a2ab39892cf, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3ca8125f4a6f16954cd35389b931e4f5254eed0644541aa93296a4246335abee }
  src/lang.rs::extract_symbols: { source_hash: adc245db587f476386bdf9f366d3114d17cc70733d0006744a8853abeef9dab3, deps_hash: ce7d9d7fc16420378d45b890370f2eb5a11a122aee5b39ac6d807d291364fb0f, spec_hash: 7df04949e7e52807257648b948e7ecca6e8e3f0bb531ed1ee3ee22d339a9a379 }
  src/lang.rs::FileRole: { source_hash: 664e61d3b1ae6a13b12f0d090d22185e85254a0dee982702438d9a1223428088, deps_hash: f5afa2f13a5fae7ce9579309271426d63ca953f7caf6f86202a89f7f91e5a00f, spec_hash: 99e9dec2af1c03a82d76c0e3fb72e3bfbaea90093c1abd1edd1599458fcae911 }
  src/lang.rs::classify: { source_hash: 908bf0c8a6f2bb97bc5feed5c0cd0e4df84717e8c481b29b9f1ac2eed35b626f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: cb5f679c4c0e908c4665e90988e4a80a1825f3fdab8173f868ad5a56ba9148c5 }
  src/lang.rs::is_test_path: { source_hash: 43bd27fc12cb63b5b6e60e8b23cc75ba2d0bc82465e1ffb6cf530d089aab30e1, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a511957bab4e03e1d307eb16d4d08b01087428c3d50e9d8673f9d4e459a0ea28 }
  src/lang.rs::is_ui_primitive: { source_hash: 209f4445fe0023de0546da9717f126bbf2a38ed1753a94a50ca314462c748675, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 367ced1d96d513b98d77c48ddb63572c0bcfb1d984742211d1bf495007b8c895 }
  src/lang.rs::detect: { source_hash: ea74de7e49f739d02194f31b7eb7af7cbac589845823439d39f59d900f24d8fa, deps_hash: f5afa2f13a5fae7ce9579309271426d63ca953f7caf6f86202a89f7f91e5a00f, spec_hash: 9382f4c5b3d0303d9f1384a85dd5e6d49a7078c0df4fe318819fd7d53e949209 }
---
# src/lang.rs
## Summary
This file does two jobs. First, `detect` — the startup step that picks which language pack (TypeScript/Next, Rust, or Java) CodeOwl uses for a repo, by counting each stack's source files and refusing to guess when zero or several match. Second, it holds the TypeScript + SQL pack's own building blocks: which extensions count as source (`SourceKind`, `is_extractable`), how to set up the parser for a `.ts` vs a `.tsx` file (`ts_parser`), which extractor a file's contents go to (`extract_symbols` — SQL tables for `.sql`, TypeScript declarations otherwise), the extension list the import resolver tries, and `classify`, which tags each file as product code, a UI primitive, test code, or generated so spec generation can prioritise them. Only `detect` is language-neutral; the rest is TypeScript-specific and slated to move inside `TypeScriptNextStack` in a later refactor.

## `SourceKind`
`pub enum SourceKind`
### Summary
Which of the TypeScript pack's two extractors a source file's contents belong to — `Code` (`.ts`/`.tsx` → `ts_parser` + `extract.rs`) or `Schema` (`.sql` → `tree-sitter-sequel` + `schema.rs`). Extension-derived for now; M13's plan is to fold this into a `pack.is_source_file` hook the active stack owns.
### Behavior
`SourceKind::of(rel_path)` is a pure extension match with one deliberate exclusion: a `.d.ts` declaration file returns `None` (it has no runtime symbols worth extracting), checked before the `.ts` case so it isn't misrouted to `Code`. `.sql` → `Schema`, `.ts`/`.tsx` → `Code`, anything else → `None`. A `None` result means "the walker skips this file entirely" — it is how a Rust file, a JSON config, or a Markdown doc is left out of the TypeScript pack's graph.
### Depends on
- (none)

## `is_extractable`
`pub fn is_extractable(path: &Path) -> bool`
### Summary
The one predicate for "does CodeOwl's TypeScript pack read this file at all" — shared by `main.rs`'s walk, the fresh-spawn catch-up pass, and the in-session file watcher, so all three agree on the file set.
### Behavior
A thin wrapper over `SourceKind::of`: converts the path to `&str` (non-UTF-8 paths fall through to `false`) and returns whether that yields any `SourceKind`. So it is `true` for `.ts` / `.tsx` / `.sql` and `false` for everything else, including `.d.ts`. Kept separate from `SourceKind::of` because most callers only need the yes/no, not which extractor — routing to the right one is `of`'s job.
### Depends on
- externals: std

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
The TypeScript pack's single symbol-extraction entry point — hands a file to the SQL table extractor if it's a schema file, or the TypeScript declaration extractor otherwise. `graph.rs` and `index.rs` both route through here rather than calling `schema.rs` / `extract.rs` directly.
### Behavior
Matches on `SourceKind::of(rel_path)`: `Schema` → `schema::extract_tables`, `Code` → `extract::extract_file`. A path with no `SourceKind` (`None`) also goes to `extract_file` — a deliberate fallback that never actually triggers from the file walk, since the walk filters on `is_extractable` first, but keeps the function total. In the current codebase this is the TypeScript pack's implementation of `StackPack::extract_symbols`; the Rust pack has its own (`rust::extract_file`).
### Depends on
- `src/symbol.rs::ExtractedSymbol` — crate::symbol

## `FileRole`
`pub enum FileRole`
### Summary
A single classification of what kind of code a file holds — `Domain`, `Primitive`, `Test`, or `Generated` — used by `spec::prioritize` to order generation and to decide whether a file counts as shared infrastructure. Replaces the scattered `is_test_path` / `is_ui_primitive` predicates with one enum, and is the seam a `StackPack` owns since "what's a UI primitive" and "what's generated" are stack conventions.
### Behavior
Each variant carries a distinct priority meaning: `Domain` (the default) is fan-in-counted and eligible for the shared-code tier; `Primitive` (a `components/ui/*` file) is deliberately excluded from that tier — it dominates any fan-in ranking but its summary tells a dependent spec nothing useful — yet still gets a long-tail file spec; `Test` code stays in the graph so `get_callers` still shows test usage but sorts last and is never a product module; `Generated` is reserved — no Phase 1 convention emits it, and it's treated like `Primitive` until a pack supplies the rule (e.g. Supabase's `database.types.ts`, protobuf `*_pb.ts`).
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
Picks the one language pack CodeOwl uses for a repo — TypeScript/Next, Rust, or Java — by counting what source files are actually present. Runs once at startup. If nothing matches, or more than one stack does, it stops with an error instead of serving a wrong or empty model of the code.
### Behavior
Walks the whole file tree (honoring `.gitignore`) and, for each candidate pack, counts the files that pack treats as *primary source*: `.ts`/`.tsx` for TypeScript, `.rs` for Rust, `.java` for Java. A `.sql` file is schema, not code, so a migrations folder on its own never makes a Rust repo look like TypeScript.

Test trees are left out of that count — top-level `tests/`, `test/`, `benches/`, and anything under `src/test/` (the Maven/Gradle layout) — so a `.tsx` fixture that a Rust test reads as a string can't swing the result. Those files are still parsed once a pack is chosen; they just don't get a vote.

Outcome: exactly one pack with a non-zero count wins and is returned. Zero counts → an error naming the three supported stacks. Two or more → an error listing which stacks were found and their file counts, telling the caller to point CodeOwl at a subdirectory that's a single stack. One stack per repo is a deliberate limit (M13 design decision 6).
### Depends on
- `src/stack.rs::StackPack` — crate::stack
- externals: anyhow, std
