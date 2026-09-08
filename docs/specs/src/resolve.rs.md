---
kind: file
source_paths: [src/resolve.rs]
file: { source_hash: 720db06e10107e6dedf579227114756f70c9d4f28452c8afd9ed3b51979ac5e8, deps_hash: 2dc9684a80741e34b3ffd4549cd0cf8c54ddf39b72942a96621673bde276cf67, spec_hash: 87a86df9fa76fb244a613b0c71ad9e959863c21cdcd9c0cc135ff7c5a0592a76 }
symbols:
  src/resolve.rs::ResolvedImport: { source_hash: 74349961b4a8ba1c665ea7eec95d24e6b0c9921cd22bed0944aeb367af610818, deps_hash: ac583dfc44b8c03e1d77dc3191f47c59b6bb6a7b182fa0031e169bb47d564290, spec_hash: 9bf21c9ea38f862917fad661bd80739849ddb3f4c1fd82717c258bbc5d83280c }
  src/resolve.rs::build_resolver: { source_hash: 5b0fc1ffe0ead87102dd5596a25736b6dd501923d3296ea2a087d4885c70a6c9, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 544f7dee56518989e20066d205cecdef848471e1a35ce377cfd34bb74e6f46f4 }
  src/resolve.rs::resolve_imports: { source_hash: c7de32163705f09a70d05ecf7eb6ae64fccfef609e49bf1d8cb6138cbb411c85, deps_hash: 337a50a06bf84de91dbf844388e6be67177c4ea99c6c77935d9bf40495e9ab79, spec_hash: 79987c6f6c1110dc0d5f28b48cb2e2903f440ed90febd58eaa6d2494e5117172 }
  src/resolve.rs::sorted_by_key: { source_hash: 617b8012a8c652159c02fb0db0c6b3717a03aa759e2c90c51bbdb9f5f24af7af, deps_hash: 8fcd29bb9cdbe400a1c914b64ea9a910b9aae08ec252f1e91a7f090f6fb6e82c, spec_hash: 549a1d95a6fb04e6039a9c8bbfe0492aebad408576935e6051ce06caa9522521 }
  src/resolve.rs::ResolvedDefaultImport: { source_hash: 4d9ef6b2727d4ae3c687d8789585af98c04eb83a663d2f94c2c243bdc9ae7aec, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 28df4b5b73749521d55c230b394c4ab8fcd10cf3e4b66c1e9acd2b41bed2692b }
  src/resolve.rs::resolve_default_imports: { source_hash: a69428cdc5d2b88dc3986fca09114f475cb390f86f3eb7fc04b62cd8cddf33c5, deps_hash: 8fcd29bb9cdbe400a1c914b64ea9a910b9aae08ec252f1e91a7f090f6fb6e82c, spec_hash: 40ec1835bf35299b6d9db2b76834be7ef5c83a1155c31d7d4dcd3e3c561a69e3 }
  src/resolve.rs::specifier_to_rel_path: { source_hash: 3229e450f1ad78c4d8aa1a1fdfabeff484b3da0f90e81a3eebfee7e092aeec95, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: e428f2b0de6536c5a640b85ed8e722dc77dae8f1f301e0f27687e854d6357087 }
  src/resolve.rs::ResolveCtx: { source_hash: bdeed0f0ec8da67a67740cab5f7d27e4caaf83d430db165877a93fe38c90c744, deps_hash: 2dc9684a80741e34b3ffd4549cd0cf8c54ddf39b72942a96621673bde276cf67, spec_hash: e24b82f35bec742948a6454a438060de79f037215b6eb032151a304d71dfc132 }
---
# src/resolve.rs
## Summary
Ties `imports.rs` (what each file imports) to `graph.rs` (what symbols exist) via `oxc_resolver` (what file a specifier points at), producing the file-to-file reference edges M2 is about. Resolution is two steps: `oxc_resolver` turns `'./foo'` or `'@/lib/foo'` into an absolute path — handling relative imports, `tsconfig.json` path aliases (auto-discovered, so CodeOwl never parses `tsconfig` itself), and `node_modules` the way Node/TypeScript does — then a symbol named `<name>` is looked up in that file, following named re-export (barrel) chains up to `MAX_REEXPORT_HOPS` deep. `resolve_imports` produces the `ResolvedImport` edges (unresolved ones kept, not dropped); `resolve_default_imports` produces file-level `ResolvedDefaultImport`s for M11's component matcher; `ResolveCtx` bundles the recursion state. Both iterate files in path-sorted order so the persisted cache is diffable. This is the TypeScript + Next.js pack's resolver — a second language needs its own (`rust.rs` has one).

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
Resolves every named import across the repo into a flat `Vec<ResolvedImport>` — the file-to-file reference edges the graph is built with. Called once per rebuild, after `Graph::build` (it needs the arena to look symbols up in).
### Behavior
Wraps the four inputs in a `ResolveCtx` and iterates `file_imports` in **path-sorted** order — not `HashMap` order — because the result is persisted to `.codeowl/graph` and a run-to-run-stable array is what makes that cache diffable. Within a file, imports stay in source order. For each import it calls `ctx.resolve_named` with a hop budget of `MAX_REEXPORT_HOPS` (to follow barrel re-export chains without looping) and records the outcome — `target: Some(id)` or `None` — verbatim; an unresolved import is kept, not dropped. `re_exports` themselves aren't emitted as edges here; they're only followed while resolving someone else's import.
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
Resolves every file's default imports to the repo-relative file each points at — the lookup `assemble_participants` uses to follow a `<Component/>` (a default-exported React component) into a feature's `core`.
### Behavior
Iterates `file_imports` in path-sorted order (same determinism reasoning as `resolve_imports`), and for each `default_imports` entry runs `specifier_to_rel_path` — a *file*-only resolution, no symbol lookup and no re-export following. A specifier that resolves outside the repo, or not at all, is silently skipped (`continue`) rather than recorded as unresolved, because the only consumer (component matching) has nothing to do with an external default import. Separate from `resolve_imports` because default imports need file granularity, not symbol granularity.
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
