---
kind: file
source_paths: [src/resolve.rs]
file: { source_hash: 4cbbbe1ebc75747a16fe3674c05c0b422ca2f8a9998c04c0dcf66a73e6daa8dc, deps_hash: 2dc9684a80741e34b3ffd4549cd0cf8c54ddf39b72942a96621673bde276cf67, spec_hash: 577a5c80f8ce87f155eb83631f58fe794d20c0bf35200f28d43d7698c725a791 }
symbols:
  src/resolve.rs::ResolvedImport: { source_hash: 74349961b4a8ba1c665ea7eec95d24e6b0c9921cd22bed0944aeb367af610818, deps_hash: ac583dfc44b8c03e1d77dc3191f47c59b6bb6a7b182fa0031e169bb47d564290, spec_hash: 9bf21c9ea38f862917fad661bd80739849ddb3f4c1fd82717c258bbc5d83280c }
  src/resolve.rs::build_resolver: { source_hash: 5b0fc1ffe0ead87102dd5596a25736b6dd501923d3296ea2a087d4885c70a6c9, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 544f7dee56518989e20066d205cecdef848471e1a35ce377cfd34bb74e6f46f4 }
  src/resolve.rs::resolve_imports: { source_hash: 24f79b4c1973b53c5f4962c41f34cdc7a3e373d2871b26494ad711b100c1e35d, deps_hash: 337a50a06bf84de91dbf844388e6be67177c4ea99c6c77935d9bf40495e9ab79, spec_hash: ecc63b9784eb555799d80545f2a73a89cebcabaf87181953849a29fcdee092d0 }
  src/resolve.rs::sorted_by_key: { source_hash: 617b8012a8c652159c02fb0db0c6b3717a03aa759e2c90c51bbdb9f5f24af7af, deps_hash: 8fcd29bb9cdbe400a1c914b64ea9a910b9aae08ec252f1e91a7f090f6fb6e82c, spec_hash: 549a1d95a6fb04e6039a9c8bbfe0492aebad408576935e6051ce06caa9522521 }
  src/resolve.rs::ResolvedDefaultImport: { source_hash: 4d9ef6b2727d4ae3c687d8789585af98c04eb83a663d2f94c2c243bdc9ae7aec, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 28df4b5b73749521d55c230b394c4ab8fcd10cf3e4b66c1e9acd2b41bed2692b }
  src/resolve.rs::resolve_default_imports: { source_hash: f8dbeae49c58a45c30dfa1776f71e48ae74629d82ae8c563cc9d63a1dc7e442b, deps_hash: 8fcd29bb9cdbe400a1c914b64ea9a910b9aae08ec252f1e91a7f090f6fb6e82c, spec_hash: 011b1f6e7759f4b3fb16b4967e82321acf177bf4f82c74aef597405358620ac8 }
  src/resolve.rs::specifier_to_rel_path: { source_hash: 3229e450f1ad78c4d8aa1a1fdfabeff484b3da0f90e81a3eebfee7e092aeec95, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: e428f2b0de6536c5a640b85ed8e722dc77dae8f1f301e0f27687e854d6357087 }
  src/resolve.rs::ResolveCtx: { source_hash: bdeed0f0ec8da67a67740cab5f7d27e4caaf83d430db165877a93fe38c90c744, deps_hash: 2dc9684a80741e34b3ffd4549cd0cf8c54ddf39b72942a96621673bde276cf67, spec_hash: e24b82f35bec742948a6454a438060de79f037215b6eb032151a304d71dfc132 }
---
# src/resolve.rs
## Summary
Turns the raw import lists from `imports.rs` into resolved dependency edges — the "file A's code depends on symbol B in file C" links that `get_callers` and `get_callees` answer from. For each import it uses `oxc_resolver` to work out which file a specifier like `./foo` or `@/lib/foo` actually means (following relative paths, `tsconfig` path aliases, and `node_modules` the way Node and TypeScript do), then finds the named declaration there, chasing through re-export "barrel" files (which just forward a name from elsewhere) up to a fixed depth. Unresolved imports — external packages, broken paths — are kept in the output rather than dropped, since a broken import is real signal. `build_resolver` configures the resolver for a TypeScript project; `resolve_default_imports` is the file-level-only variant the feature layer uses to follow `<Component/>` tags. All of this is TypeScript/Node-specific — Rust and Java have their own resolution in `rust.rs` / `java.rs`.

## `ResolvedImport`
`pub struct ResolvedImport`
### Summary
One named import after resolution — the file it's written in, the specifier and imported name as written, and the `SymbolId` it points at (or `None`). The reference edge every `get_callers` / `get_callees` / dependency-list answer reads off.
### Behavior
Plain data, persisted on `Graph`. `target` is `None` in three cases, all treated the same by consumers: the specifier resolves to an external package outside the walked repo, it doesn't resolve at all (a broken import — kept deliberately, it's real signal), or it resolves to a file but no declaration or re-export chain there actually provides `imported_name`. A `Some(target)` always names a symbol node, never a file node — following a re-export chain to the underlying declaration is `resolve_imports`'s job.
### Depends on
- `src/symbol.rs::SymbolId` — crate::graph

## `build_resolver`
`pub fn build_resolver() -> Resolver`
### Summary
Constructs the `oxc_resolver::Resolver` CodeOwl uses to turn an import specifier into a real file path — configured for a TypeScript project.
### Behavior
Sets two non-default options: the extension list from `lang::RESOLVER_EXTENSIONS` (`.ts`, `.tsx`, `.d.ts`, `.js`, `.jsx`, `.json`, in Node/TS resolution order), and `tsconfig: Auto` discovery. The `Auto` discovery is what makes `@/lib/...` path aliases resolve — `oxc_resolver` locates and parses the nearest `tsconfig.json` itself, so CodeOwl never has to. Everything else is `ResolveOptions::default()`. The returned `Resolver` is cheap to clone and safe to reuse across files.
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
Takes every `import` CodeOwl found across the repo and works out which actual declaration each one points at — turning `import { Graph } from './graph'` into a link to `src/graph.rs::Graph`. These resolved links are the file-to-file dependency edges the rest of the graph is built on (what `get_callers` / `get_callees` read). Runs once per rebuild, after the symbol graph already exists.
### Behavior
First it canonicalizes `repo_root` — resolving any symlinks in the path. The underlying resolver (`oxc_resolver`) returns symlink-free absolute paths, and a later step strips `repo_root` off them to recover a repo-relative path; if `repo_root` itself still contained a symlink (on macOS, temp dirs live under `/var`, which is a symlink to `/private/var`), that strip would fail and *every* import would come back unresolved. Falls back to the path as given if it can't be canonicalized.

Then it walks the files in sorted path order — not hash-map order — because the result is written to the on-disk cache, and a run-to-run-stable order is what lets that cache be diffed. Within a file, imports stay in source order.

For each import it calls `resolve_named`, which asks `oxc_resolver` for the file the specifier lands on (handling relative paths, `tsconfig` path aliases, and `node_modules` the way Node/TypeScript does), then looks for the named declaration in that file — following up to `MAX_REEXPORT_HOPS` re-export hops if the file just forwards the name from somewhere else. The outcome (the target, or `None`) is recorded exactly as found: an unresolved import — an external package, a broken path, or a name the target file doesn't actually export — is kept in the list, not dropped, because a broken import is real signal worth surfacing.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- externals: oxc_resolver, std

## `sorted_by_key`
`fn sorted_by_key(file_imports: &HashMap<String, FileImports>) -> Vec<(&String, &FileImports)>`
### Summary
Returns a `HashMap<String, FileImports>`'s entries as a `Vec` in ascending path order — the deterministic iteration order `resolve_imports` needs so its persisted output is stable run to run.
### Behavior
Collects the map's entries into a `Vec` of borrows and sorts by key (the repo-relative path string). Borrows, not clones — cheap. Named "by_key" because the key is the only sort dimension; a file's own `imports` list is already in source order and isn't touched.
### Depends on
- `src/imports.rs::FileImports` — crate::imports
- externals: std

## `ResolvedDefaultImport`
`pub struct ResolvedDefaultImport`
### Summary
One `import Name from './x'` default import resolved to the repo file it points at — the file, not a symbol. Feeds M11's rendered-component matcher, which needs to know which file a `<Name/>` tag comes from and nothing more.
### Behavior
Plain data: the importing file, the local binding name, the resolved target file path. Only produced when the specifier resolves *inside* the repo — a default import of an external package (`import React from 'react'`) produces nothing. Stored as its own `Graph` field (`resolved_default_imports`) rather than folded into the named-import edges because it's file-level plumbing for one flow-edge kind, not a reference edge itself; M18 folds it into the pack's resolution output.
### Depends on
- (none)

## `resolve_default_imports`
`pub fn resolve_default_imports(
    repo_root: &Path,
    resolver: &Resolver,
    file_imports: &HashMap<String, FileImports>,
) -> Vec<ResolvedDefaultImport>`
### Summary
Resolves each file's *default* imports (`import Form from './form'`) to the repo file they point at — just the file, not a specific symbol. The feature-spec builder uses this to follow a `<Component/>` tag in JSX into the component's source, since React components are almost always default-exported and the named-import list alone can't locate them.
### Behavior
Same shape as `resolve_imports`, but file-level and simpler. Canonicalizes `repo_root` first (see `resolve_imports` for why a symlinked root would otherwise break every lookup), walks files in sorted path order for a stable cache, and for each `default_imports` entry asks `oxc_resolver` which file the specifier resolves to. A specifier that lands outside the repo, or doesn't resolve at all, is silently skipped — `assemble_participants`, the only caller, has nothing to do with an external default import. No symbol lookup and no re-export chasing: it stops once it has the file.
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
The read-only bundle every named-import resolution step needs — repo root, resolver, the whole `file_imports` map, and the built graph — plus `resolve_named`, the recursive resolver that follows barrel re-export chains. Bundled into a struct rather than threaded as four parameters through each recursive call.
### Behavior
`resolve_named(from_file, specifier, name, hops_left)`: resolve the specifier to a target file, then look for `<target_file>::<name>` as a direct symbol — a hit returns immediately. On a miss it assumes a barrel: it decrements `hops_left` (returning `None` if the budget is spent — this is what stops a circular re-export chain from recursing forever), looks in the target file's `re_exports` for one whose `exported_as` matches `name`, and recurses with that re-export's own specifier and `source_name`. So `import { X } from './barrel'` where the barrel does `export { X as X } from './real'` resolves through to `real::X`. Any step that can't resolve — bad specifier, no such re-export, no such symbol — collapses the whole chain to `None`.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- externals: oxc_resolver, std
