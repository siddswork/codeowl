---
type: file
title: "src/imports.rs"
description: "Specification for `src/imports.rs`."
generated: { by: CodeOwl }
resource: "../../../src/imports.rs"
source_paths: [src/imports.rs]
file: { source_hash: 031fcec6569ec878281bec18a5fbc2d836581047d476664867aa3edf1814c138, fingerprint: d35615cccfb69523fdb2cec75d63dfac20496d414077aa7cda2eb856f3b4ea66, deps_hash: a1964b774915f48932ea74e02999d3d269ef7b4814dc96c0fd8d0cd974b40435, spec_hash: 3748d51ee7f67b6b67ab68eb386301ce8aff42ddb57f5853cf266c68a5247fa0 }
symbols:
  src/imports.rs::ImportRef: { source_hash: 5e4f34d49e172156333617bb4b0dd5cb42970f8cfe848c05e4b3d53d2dddd6cb, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 6daf4a1a4e0a21d5ebe84bb7692f5af6d199c3234a479810f90bbe015e14743e }
  src/imports.rs::ReExport: { source_hash: b9f7522994b6970557cd384bbf54eeb14c82d13c9a9ee27f21e7cabb583ce9e5, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 0233028f3f4c9f6b467929f5412f0b5504326a0affe5c90234b4d763735607c2 }
  src/imports.rs::DefaultImport: { source_hash: 3a7b058036a76f4074a2574a156c6c5e9e6691272901a907b8d56c27e9b03d88, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c2353e4863d9e9bf706ee143fea33ff7e76e8f1d843a4912abd44b596a2a7d8f }
  src/imports.rs::FileImports: { source_hash: 05695cac9d8033e676634e16dbda2b084b5f895fa0ad6361e42e5c7bb2f62aa1, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 97de1703021eb7a603d03c80f40245bd1bdb74499b779573eb70361dc5ee59c4 }
dep_targets:
  file -> src/lang.rs::ts_parser: ca09f16dafc1
---
# src/imports.rs
## Summary
The TypeScript import reader: it parses one file and lists where its code comes from, which is the raw input for linking files to one another. It runs as its own tree-sitter pass, separate from the declaration extractor, so each stays single-purpose, and parsing a small file twice costs next to nothing. The result for a file is a `FileImports` with four lists: named imports (`import { a } from './x'`), re-exports (names a file forwards, such as `export { a } from './x'`, or a local `export { a }` of something it imported), default imports (`import Foo from './x'`), and qualified references, which the TypeScript pass never fills but the Rust and Java packs do. Named forms resolve to a specific symbol later, default imports only to a file, and the file-level reason for tracking those is to trace a rendered `<Component/>` back to its source. Namespace imports (`import * as ns`) and wildcard re-exports (`export * from`) are deliberately not tracked, since neither names one declaration. Small helpers read names out of export clauses and import lists, and are shared with the declaration extractor so both read an `export { ... }` line the same way. Everything here is specific to TypeScript; other languages have their own import readers.

## `ImportRef`
`pub struct ImportRef`
### Summary
One named `import` statement as extracted from a file's source, before it's been resolved to an actual target — the raw shape of `import { <imported_name> } from '<specifier>'`.
### Behavior
`specifier` is the module string as written (`'./graph'`, `'@/lib/utils'`). `imported_name` is the name being imported. A local alias (`import { Foo as Bar }`) is deliberately not tracked here: renaming on import is purely local to the importing file and has no bearing on what the import actually resolves to, so keeping it would add a field nothing downstream needs.
### Depends on
- (none)

## `ReExport`
`pub struct ReExport`
### Summary
One named re-export — how a barrel file (a file whose only job is to gather and forward names from other files, without declaring anything itself) passes along a name it doesn't own: `export { <source_name> as <exported_as> } from '<specifier>'`.
### Behavior
`specifier` is the module the name is being forwarded from. `source_name` is that name as declared in the source module; `exported_as` is the name this file re-exports it under (identical to `source_name` when there's no `as` rename). Resolving an import against a barrel means following its `ReExport`s through to wherever `source_name` is actually declared, rather than stopping at the barrel file itself.
### Depends on
- (none)

## `DefaultImport`
`pub struct DefaultImport`
### Summary
One default import: `import <local_name> from '<specifier>'` (or the default half of `import <local_name>, { ... } from '<specifier>'`). Tracked specifically so a rendered `<Component/>` tag can be resolved to the file it came from — React components are almost always default-exported, so the named-import list alone can't answer "what file defines this component."
### Behavior
`local_name` is the name this file calls the import by; `specifier` is the module string it came from. Resolution here is deliberately file-level only — it answers "which file does this default import point at," not "which specific symbol in that file," since a default export doesn't have a name of its own to match against the way a named import does. See `resolve::resolve_default_imports` for how this gets turned into an actual file target.
### Depends on
- (none)

## `FileImports`
`pub struct FileImports`
### Summary
Everything a single file says about where it gets code from: its named imports, the names it forwards from other files, its default imports, and any fully-spelled-out references that need no import line at all.
### Behavior
Four lists. `imports` holds named imports. `re_exports` holds names the file forwards from elsewhere, which is how a barrel file (one that only re-exports) passes a name along. `default_imports` holds `import Name from './x'` forms. `qualified_refs` holds references written with their full path and no `use` or `import` statement, such as Rust's `crate::a::b::Item` or a Java fully-qualified class name. These are kept apart from `imports` on purpose: Java's same-package scan treats an explicit import of a simple name as hiding a same-package guess for that name, and a fully-qualified reference to a different type of the same name, as with two distinct `Fight` classes, must not trigger that hiding. TypeScript and Python never fill `qualified_refs`, because their module systems have no way to reach another module without naming it. `default_imports` and `qualified_refs` default to empty when absent from older cached data. Plain data, no logic.
### Depends on
- (none)
