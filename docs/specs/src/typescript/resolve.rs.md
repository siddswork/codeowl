---
type: file
title: "src/typescript/resolve.rs"
description: "Specification for `src/typescript/resolve.rs`."
generated: { by: CodeOwl }
resource: "../../../../src/typescript/resolve.rs"
source_paths: [src/typescript/resolve.rs]
file: { source_hash: , deps_hash: , spec_hash:  }
symbols:
  src/typescript/resolve.rs::build_resolver: { source_hash: 5b0fc1ffe0ead87102dd5596a25736b6dd501923d3296ea2a087d4885c70a6c9, deps_hash: 85842da049815a5d167305c400dd84e167aa517159d127ba76a3c92c6eaf832b, spec_hash: 50b7596765a8e7cba399bd7c59859d42c482f973bcd8450b954d5449a70e3768 }
  src/typescript/resolve.rs::resolve_imports: { source_hash: 24f79b4c1973b53c5f4962c41f34cdc7a3e373d2871b26494ad711b100c1e35d, deps_hash: 64445603d69c60a181dc3ccacd0615f160073c69f369859dddd528b637e82775, spec_hash: b0ee178582dd55965ba43e873f047c612b90a13c797e21830657107e8bfdaec3 }
  src/typescript/resolve.rs::sorted_by_key: { source_hash: 617b8012a8c652159c02fb0db0c6b3717a03aa759e2c90c51bbdb9f5f24af7af, deps_hash: baf806a55f27b576723e1a9bc9913fd5657a443ce71840b90578afc60daa8162, spec_hash: 70d6c6f69cd057d7cf290db47966b8fbf95ad141c3daf2a7065fac1173e3b9ad }
  src/typescript/resolve.rs::resolve_default_imports: { source_hash: f8dbeae49c58a45c30dfa1776f71e48ae74629d82ae8c563cc9d63a1dc7e442b, deps_hash: 541dd32ad2ec7b47caf16f98d354eebb8478efac94762040ffc4e83ece3432b4, spec_hash: e6f3fb3df559ef479db11caac917cfdfbecf7ef7f26cfb4ce62dedf3a4c95bca }
  src/typescript/resolve.rs::specifier_to_rel_path: { source_hash: 3229e450f1ad78c4d8aa1a1fdfabeff484b3da0f90e81a3eebfee7e092aeec95, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: e428f2b0de6536c5a640b85ed8e722dc77dae8f1f301e0f27687e854d6357087 }
  src/typescript/resolve.rs::ResolveCtx: { source_hash: 4ea7ffb0afff8c7971523cac1044fa0ec71f09f4889755a3c3ae5a2af38eb998, deps_hash: 6b942dba596a1091b6956d7c88e88327109d9b8df39b06102f6291c24aafb2ad, spec_hash: c679a9c6d93030fc1ec17b2d960a046a86a143fd04774112d43ead42cef260f8 }
dep_targets:
  src/typescript/resolve.rs::build_resolver -> src/lang.rs::RESOLVER_EXTENSIONS: 1705cc23b763
  src/typescript/resolve.rs::resolve_imports -> src/graph.rs::Graph: a0fadcfbc2bc
  src/typescript/resolve.rs::resolve_imports -> src/imports.rs::FileImports: 7a834189d6b3
  src/typescript/resolve.rs::sorted_by_key -> src/imports.rs::FileImports: 7a834189d6b3
  src/typescript/resolve.rs::resolve_default_imports -> src/graph.rs::ResolvedDefaultImport: bebbd77bb6f0
  src/typescript/resolve.rs::resolve_default_imports -> src/imports.rs::FileImports: 7a834189d6b3
  src/typescript/resolve.rs::ResolveCtx -> src/graph.rs::Graph: a0fadcfbc2bc
  src/typescript/resolve.rs::ResolveCtx -> src/imports.rs::FileImports: 7a834189d6b3
  src/typescript/resolve.rs::ResolveCtx -> src/symbol.rs::SymbolId: 6733b5c62e75
---
# src/typescript/resolve.rs
## Summary


## `build_resolver`
`pub fn build_resolver() -> Resolver`
### Summary
Creates the tool that works out which file an import like `./foo` or `@/lib/foo` really points to, set up the way a TypeScript project expects.
### Behavior
Builds an `oxc_resolver` `Resolver` with two non-default settings: the extensions to try, taken from `lang::RESOLVER_EXTENSIONS` (`.ts`, `.tsx`, `.d.ts`, `.js`, `.jsx`, `.json`, in that order, so TypeScript files win), and automatic `tsconfig.json` discovery, which is what makes path aliases such as `@/lib/...` work without CodeOwl finding or parsing the config file itself. Everything else keeps the library defaults. It takes no arguments and cannot fail; the returned resolver is then used for each import by the resolution functions in this file.
### Depends on
- externals: oxc_resolver

## `resolve_imports`
`pub fn resolve_imports(
    repo_root: &Path,
    resolver: &Resolver,
    file_imports: &HashMap<String, FileImports>,
    graph: &Graph,
) -> Vec<ResolvedImport>`
### Summary
Resolves every named import across the whole repo to the declaration it points at, producing the list of links between files that the rest of CodeOwl builds on. Imports that lead nowhere are still included, with no target.
### Behavior
`file_imports` must have one entry per indexed file, keyed by the same repo-relative paths symbols use. First it canonicalises the repo root (the real path with symlinks resolved), since the resolver library returns symlink-resolved paths and stripping the root prefix only works if both are in the same form. Without this, a repo under a symlinked folder, such as `/tmp` or macOS `/var`, would resolve every import to nothing. If canonicalising fails it silently uses the root as given.

It then goes through the files in sorted order, not hash map order, because the result is saved to `.codeowl/graph` and a stable order lets the cache be compared run to run. For each file's entries in its `imports` list, in source order, it calls `resolve_named` with a cap on how many re-export hops it may follow, and records a `ResolvedImport` with the importing file, specifier, imported name and the target or `None`. One output entry is produced per import entry, with no deduplication. It only walks each file's `imports` list, not the `re_exports` list. It cannot fail; unresolvable specifiers, external packages and over-long re-export chains all show up as `None` targets, with no distinction between them.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/graph.rs::ResolvedImport` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- externals: oxc_resolver, std

## `sorted_by_key`
`fn sorted_by_key(file_imports: &HashMap<String, FileImports>) -> Vec<(&String, &FileImports)>`
### Summary
Puts a map of per-file import data into a fixed order, sorted by file path, so that resolving imports always visits files in the same sequence.
### Behavior
Collects the map's entries as pairs of path and imports, sorts them ascending by path (a plain string comparison), and returns them as a vector of borrowed pairs, so nothing is copied. A hash map has no stable iteration order, and both the named-import and default-import resolvers write results into the saved graph cache, so a fixed order keeps that file identical between runs. An empty map gives an empty vector. It cannot fail.
### Depends on
- `src/imports.rs::FileImports` — crate::imports
- externals: std

## `resolve_default_imports`
`pub fn resolve_default_imports(
    repo_root: &Path,
    resolver: &Resolver,
    file_imports: &HashMap<String, FileImports>,
) -> Vec<ResolvedDefaultImport>`
### Summary
Works out which file each default import points at, such as `import Header from './header'`. This is what lets a `<Header/>` shown on a page be traced back to the file that defines it and included in that page's feature description.
### Behavior
Canonicalises the repo root first, for the same symlink reason as `resolve_imports`. Then for each file in sorted path order and each of its default imports, it asks `specifier_to_rel_path` for the repo-relative file the specifier resolves to. An import that resolves outside the repo or not at all is dropped, so, unlike named imports, unresolved default imports leave no record. Each kept one becomes a `ResolvedDefaultImport` with the importing file, the local name, and the target file. The result is file-level only: it names no particular declaration, and it does not follow re-export chains, since a default import has no name to chase. It cannot fail.
### Depends on
- `src/graph.rs::ResolvedDefaultImport` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- externals: oxc_resolver, std

## `specifier_to_rel_path`
`fn specifier_to_rel_path(
    repo_root: &Path,
    resolver: &Resolver,
    from_file: &str,
    specifier: &str,
) -> Option<String>`
### Summary
The one-shot "specifier → repo-relative file path" helper both `resolve_imports` (via `ResolveCtx`) and `resolve_default_imports` build on. `None` when the specifier resolves outside the repo or not at all.
### Behavior
Joins `from_file` onto `repo_root` to get an absolute anchor, asks `oxc_resolver` to resolve `specifier` against it (applying tsconfig path aliases and the extension list from `build_resolver`), then strips `repo_root` back off. A resolution error, or a target that isn't under `repo_root` (`strip_prefix` fails — an external package, a symlink out of tree), yields `None`. The path is forward-slash normalised so it matches the `Symbol::file` id scheme on every platform.
### Depends on
- externals: oxc_resolver, std

## `ResolveCtx`
`struct ResolveCtx<'a>`
### Summary
Bundles the read-only things import resolution needs, and holds the logic that finds the declaration a named import points at, including when the file it names merely forwards the name from somewhere else.
### Behavior
The struct holds four references: the repo root, the resolver, every file's parsed imports, and the graph. It exists so those four are not passed separately through each recursive call. Its one method, `resolve_named(from_file, specifier, name, hops_left)`, works in steps. It turns the specifier into a repo-relative target file (`specifier_to_rel_path`) and returns `None` if that fails or points outside the repo. It then looks in the graph for a symbol with the id `<target file>::<name>` and returns it if found, so a direct declaration always wins. If not, the file may be a barrel that forwards the name, so it decrements `hops_left` (returning `None` if it is already zero, which is what stops a circular chain from recursing forever), finds a named re-export in the target file whose published name matches, and resolves again from there with the re-export's own specifier and original name. The hop budget is only spent on a re-export step, not on the first lookup. A name that is neither declared nor re-exported, or a target file with no recorded imports, gives `None`. It matches by name only: it does not follow default or namespace imports, wildcard re-exports, or any aliasing beyond a re-export's published name, and when two re-exports publish the same name only the first is used.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- externals: oxc_resolver, std
