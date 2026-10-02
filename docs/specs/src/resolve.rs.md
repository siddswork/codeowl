---
kind: file
source_paths: [src/resolve.rs]
file: { source_hash: 47d6f78b57fbc813b4072f46d61e074ae11b7f97db90a3dd88a937c9bd387fae, deps_hash: 00e0fc0d36678674130264c35daa0ad120e896ff68d5afb00b77df1d3155523b, spec_hash: 5de2d182abf6c4839e4206d4efa7fdd46196145d527f738b7a9c5cb84c6699ac }
symbols:
  src/resolve.rs::ResolvedImport: { source_hash: cd2ed656551ee418709be01ed70aac96e08eee44317d1d84c71be83d182d32aa, deps_hash: 2c612d8ff3471f6990442eae121f91802b45c305137b9b48b06f2d44bc45a54c, spec_hash: d5130d2e3fac678a086bed51b0194cbe8b625afad1650ae3c69303a6018a1f19 }
  src/resolve.rs::build_resolver: { source_hash: 5b0fc1ffe0ead87102dd5596a25736b6dd501923d3296ea2a087d4885c70a6c9, deps_hash: 85842da049815a5d167305c400dd84e167aa517159d127ba76a3c92c6eaf832b, spec_hash: 50b7596765a8e7cba399bd7c59859d42c482f973bcd8450b954d5449a70e3768 }
  src/resolve.rs::resolve_imports: { source_hash: 24f79b4c1973b53c5f4962c41f34cdc7a3e373d2871b26494ad711b100c1e35d, deps_hash: 6f0e2a70cb0d75b39c901b9146fb50bcab519e3f433b010e04f5275f32b1aa82, spec_hash: d387dde4df01898b1adae7175797b54e6db4ef7a98c21c7e6a2b6d6a1e3d4d59 }
  src/resolve.rs::sorted_by_key: { source_hash: 617b8012a8c652159c02fb0db0c6b3717a03aa759e2c90c51bbdb9f5f24af7af, deps_hash: baf806a55f27b576723e1a9bc9913fd5657a443ce71840b90578afc60daa8162, spec_hash: 70d6c6f69cd057d7cf290db47966b8fbf95ad141c3daf2a7065fac1173e3b9ad }
  src/resolve.rs::ResolvedDefaultImport: { source_hash: 24e3e4afb7e18eb288bbe7a21bd7f68f81277504059fc061adfeca1948201e34, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: fe280be05b001159253f040782f07d5105ed8940143b86fa530a990ca506c8ad }
  src/resolve.rs::resolve_default_imports: { source_hash: f8dbeae49c58a45c30dfa1776f71e48ae74629d82ae8c563cc9d63a1dc7e442b, deps_hash: baf806a55f27b576723e1a9bc9913fd5657a443ce71840b90578afc60daa8162, spec_hash: e6f3fb3df559ef479db11caac917cfdfbecf7ef7f26cfb4ce62dedf3a4c95bca }
  src/resolve.rs::specifier_to_rel_path: { source_hash: 3229e450f1ad78c4d8aa1a1fdfabeff484b3da0f90e81a3eebfee7e092aeec95, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: e428f2b0de6536c5a640b85ed8e722dc77dae8f1f301e0f27687e854d6357087 }
  src/resolve.rs::ResolveCtx: { source_hash: 4ea7ffb0afff8c7971523cac1044fa0ec71f09f4889755a3c3ae5a2af38eb998, deps_hash: a8b48d5312ca2852930b8adc41613f549dfd65e8ffe189dd0c38fd0185fe2127, spec_hash: 9dc9a83199f115430485574b9311d6fad562f0cb85f760acb9a990bdfb8a9440 }
---
# src/resolve.rs
## Summary
This file links what each file imports to the declarations that actually exist, producing the file-to-file references the rest of CodeOwl builds on, including the ones that decide which specs go stale. Resolving an import takes two steps. First, the `oxc_resolver` library turns a specifier like `./foo` or `@/lib/foo` into a real file path, the way Node and TypeScript do (relative paths, `tsconfig.json` path aliases, `node_modules`). Then CodeOwl looks for a declaration with the imported name in that file. If the file does not declare it, it may be a barrel (a file that only forwards other files' exports), so resolution follows named re-exports from file to file, up to a fixed number of hops so a circular chain cannot loop forever. Default imports are resolved the same way but only to a target file, which is what lets a rendered `<Component/>` be traced back to its source. Results are produced in sorted path order and the repo root is made absolute with symlinks resolved first, so the saved cache is stable between runs and repos under symlinked folders still work. An import that cannot be resolved is kept as a link with no target rather than dropped, so it can be counted.

## `ResolvedImport`
`pub struct ResolvedImport`
### Summary
One link between files: this file imports that name from that module, and, if it could be found, which declaration in the repo it points at. Links that lead nowhere are kept too, with no target.
### Behavior
Fields: `from_file` is the importing file's path, `specifier` is the module text as written (`./foo`, `@/lib/foo`, `react`), `imported_name` is the name asked for, and `target` is the arena id of the declaration it resolved to. `target` is `None` in three cases: the specifier points outside the indexed repo (an external package), it did not resolve at all (a broken import), or it resolved to a file that neither declares the name nor forwards it through a re-export chain. These edges are what drives staleness, since a dependent is judged by the target's `interface_hash`, so an unresolved one means a change cannot reach that importer. Plain data, no logic.
### Depends on
- `src/symbol.rs::SymbolId` — crate::graph

## `build_resolver`
`pub fn build_resolver() -> Resolver`
### Summary
Creates the tool that works out which file an import like `./foo` or `@/lib/foo` really points to, set up the way a TypeScript project expects.
### Behavior
Builds an `oxc_resolver` `Resolver` with two non-default settings: the extensions to try, taken from `lang::RESOLVER_EXTENSIONS` (`.ts`, `.tsx`, `.d.ts`, `.js`, `.jsx`, `.json`, in that order, so TypeScript files win), and automatic `tsconfig.json` discovery, which is what makes path aliases such as `@/lib/...` work without CodeOwl finding or parsing the config file itself. Everything else keeps the library defaults. It takes no arguments and cannot fail; the returned resolver is then used for each import by the resolution functions in this file.
### Depends on
- `src/lang.rs::RESOLVER_EXTENSIONS` — crate::lang
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
`file_imports` must have one entry per indexed file, keyed by the same repo-relative paths symbols use. First it canonicalises the repo root (the real path with symlinks resolved), since the resolver library returns symlink-resolved paths and stripping the root prefix only works if both are in the same form. Without this, a repo under a symlinked folder, such as `/tmp` or macOS `/var`, would resolve every import to nothing. If canonicalising fails it uses the root as given.

It then goes through the files in sorted order, not hash map order, because the result is saved to `.codeowl/graph` and a stable order lets the cache be compared run to run. For each file's named imports, in source order, it calls `resolve_named` with a cap on how many re-export hops it may follow, and records a `ResolvedImport` with the importing file, specifier, imported name and the target or `None`. Default imports and wildcards are not handled here. It cannot fail; problems show up as `None` targets.
### Depends on
- `src/graph.rs::Graph` — crate::graph
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

## `ResolvedDefaultImport`
`pub struct ResolvedDefaultImport`
### Summary
One `import <local_name> from '<specifier>'` (a default import), resolved to the file it points at inside the repo — the piece that lets a rendered `<Component/>` be traced back to the file that defines it.
### Behavior
`from_file` is the importing file; `local_name` is the name it imports the default export as; `target_file` is the resolved file. This is deliberately file-level only — it says which *file* the default import points at, not which specific symbol inside it, since that's all the rendered-component resolver needs (a default export doesn't have its own name to match the way a named import does). No `ResolvedDefaultImport` is produced at all when the specifier resolves to an external package rather than a file inside the repo.
### Depends on
- (none)

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
The struct holds four references: the repo root, the resolver, every file's parsed imports, and the graph. It exists so those four are not passed separately through each recursive call. Its one method, `resolve_named(from_file, specifier, name, hops_left)`, works in steps. It turns the specifier into a repo-relative target file (`specifier_to_rel_path`) and returns `None` if that is outside the repo or fails. It then looks in the graph for a symbol with the id `<target file>::<name>` and returns it if found. If not, the file may be a barrel that forwards the name, so it reduces `hops_left` (returning `None` if that would go below zero, which is what stops a circular chain from recursing forever), finds a re-export in the target file published under that name, and resolves again from there with the re-export's own specifier and original name. A name that is neither declared nor re-exported gives `None`. It does not follow default or namespace imports and does not try aliasing beyond a re-export's published name.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- externals: oxc_resolver, std
