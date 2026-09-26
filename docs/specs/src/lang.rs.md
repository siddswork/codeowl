---
kind: file
source_paths: [src/lang.rs]
file: { source_hash: aa4a56983460aff174ca497bff6fdf10736d5ddace764e125746fb7a7c8a70ff, deps_hash: 364c7530c881d070e301df9a230178f7f0986c7e1e49e67ccd1841f1cf6836f7, spec_hash: baae5727196f7ae6f0ca4968a651d5e6fd17789ad83709499ad009c19546e2f9 }
symbols:
  src/lang.rs::SourceKind: { source_hash: 030d6675bccb66a7f8439dc3f04f8c310d7bd2c00ab00dac67bec882b8ca1a15, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: bc02ee0a2ff92e0a20aef9446cf846aabfc7d9387530f38f83a01e24f787cb14 }
  src/lang.rs::ts_parser: { source_hash: a4658a5c6fc19c35d73f060642c96c4e029a9fb4ac78f7d14c6f8a2ab39892cf, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3ca8125f4a6f16954cd35389b931e4f5254eed0644541aa93296a4246335abee }
  src/lang.rs::extract_symbols: { source_hash: 9884210b833cf488592ee7a0904ea061eae1990853e1d2d70a2a41e5ae137746, deps_hash: 57b36ccf02abf241d59cf027b8af996e4e37cdf048813be00047a453efba7ce6, spec_hash: e6a6e8985503d7997920c38d2ad3594bfa63de572dbb3fe40f7c562649b05c6e }
  src/lang.rs::FileRole: { source_hash: 664e61d3b1ae6a13b12f0d090d22185e85254a0dee982702438d9a1223428088, deps_hash: f5afa2f13a5fae7ce9579309271426d63ca953f7caf6f86202a89f7f91e5a00f, spec_hash: 99e9dec2af1c03a82d76c0e3fb72e3bfbaea90093c1abd1edd1599458fcae911 }
  src/lang.rs::classify: { source_hash: 908bf0c8a6f2bb97bc5feed5c0cd0e4df84717e8c481b29b9f1ac2eed35b626f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: cb5f679c4c0e908c4665e90988e4a80a1825f3fdab8173f868ad5a56ba9148c5 }
  src/lang.rs::is_test_path: { source_hash: 43bd27fc12cb63b5b6e60e8b23cc75ba2d0bc82465e1ffb6cf530d089aab30e1, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a511957bab4e03e1d307eb16d4d08b01087428c3d50e9d8673f9d4e459a0ea28 }
  src/lang.rs::is_ui_primitive: { source_hash: 209f4445fe0023de0546da9717f126bbf2a38ed1753a94a50ca314462c748675, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 367ced1d96d513b98d77c48ddb63572c0bcfb1d984742211d1bf495007b8c895 }
  src/lang.rs::detect: { source_hash: bf5a35d9c182615746de8e548978dfcb8c4f4f396d653eb226ec749eb4974eef, deps_hash: f5afa2f13a5fae7ce9579309271426d63ca953f7caf6f86202a89f7f91e5a00f, spec_hash: 6023b6cdacf7a4189fbc14e29480486a3c7052d4d83bf5a8c4171ece41ef22b2 }
---
# src/lang.rs
## Summary
This file picks which `StackPack` should run against a given repo (`detect`), and separately holds the TypeScript + SQL pack's own internals — which file extensions count as source, which tree-sitter grammar (the parser CodeOwl uses to turn source text into a syntax tree) to load for a given path, how a file becomes a list of declarations, and heuristics for recognizing test code and shared UI-primitive components. `SourceKind` (a normal source file vs. a dedicated schema file) and `FileRole` (product code vs. a UI primitive vs. test code vs. machine-generated code) are the two stack-neutral classifications defined here, used to decide how a file's spec gets prioritized and whether it counts as shared infrastructure; everything else in this file is specific to the TypeScript stack. `detect` itself works by counting each candidate pack's own primary-source-file extension across the repo (skipping test and tooling trees, so a stray fixture file doesn't misidentify the repo's real language) and requires exactly one pack to claim the repo, erring out by name if none or more than one does.

## `SourceKind`
`pub enum SourceKind`
### Summary
Tags what kind of thing a source file is, once CodeOwl decides to look at it: ordinary code, or a dedicated database-schema file (like a `.sql` file full of `CREATE TABLE` statements). Which file extensions map to which kind is decided by each language "stack" (the pack of rules for one language/framework — see `GLOSSARY.md`), not by this enum itself.
### Behavior
Two variants: `Code` is a normal source file, parsed with the stack's grammar and run through the usual import-resolution and "flow edge" passes (the looser cross-file links like a rendered component — see `GLOSSARY.md`). `Schema` is a file parsed only for table declarations; the indexer skips the code-oriented passes for it entirely. Note that an ORM model class defined *inside* a regular code file (e.g. a Python class with `table=True`) is still `Code`, not `Schema` — it gets flagged as schema-like symbol-by-symbol later, via a separate mechanism (`is_schema_symbol`), not by this file-level classification. The associated `of` function (shown alongside this enum) is this module's own default mapping — as of the M17 milestone it recognizes only TypeScript/TSX as `Code` and no longer names `.sql` at all, since that mapping moved to be owned by the TypeScript/Next.js stack instead of living here.</content>
### Depends on
- (none)

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
The TypeScript extractor's public entry point: turns one TypeScript file's source text into its list of top-level declarations.
### Behavior
A thin wrapper around `extract::extract_file`, kept as its own free function so both `TypeScriptNextStack` (production extraction) and `graph.rs`'s test helper can share one call site rather than each calling into `extract.rs` directly. It handles TypeScript only — `.sql` files are dispatched to the schema extractor by the pack itself, not from here.
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
Figures out which single programming-language "stack" (the pack of language-specific rules CodeOwl uses to parse and index a repo — TypeScript/Next.js, Rust, Java, or Python) a repo is written in, or fails with a clear error rather than silently building an empty or wrong index. Runs once when CodeOwl starts up.
### Behavior
For each supported stack, counts how many files in the repo are that stack's "primary" source files (e.g. `.rs` for Rust) — a database schema file like `.sql` never counts toward this, since a Rust repo that happens to have migrations is still a Rust repo. While counting, it deliberately skips test directories (`tests/`, `test/`, `benches/`, Maven/Gradle's `src/test/...`) and tooling directories (`utility/`, `scripts/`, `tools/`, `hack/`) — those files are still parsed and indexed later once a stack is chosen, but they don't get a vote on *which* stack the repo is, so CodeOwl's own Python build scripts don't make CodeOwl's own (Rust) repo look ambiguous.

Only stacks with at least one matching file are kept as candidates. If none match, it fails with an error listing the stacks CodeOwl supports. If exactly one matches, that stack is returned. If more than one stack has matching files, it fails with an error naming every stack that matched and how many files each had — CodeOwl deliberately serves only one stack per repo (a genuinely polyglot repo is out of scope for now), so the fix is to point CodeOwl at a subdirectory that's a single stack.</content>
### Depends on
- `src/stack.rs::StackPack` — crate::stack
- externals: anyhow, std
