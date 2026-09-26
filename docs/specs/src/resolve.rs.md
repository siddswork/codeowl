---
kind: file
source_paths: [src/resolve.rs]
file: { source_hash: 4cbbbe1ebc75747a16fe3674c05c0b422ca2f8a9998c04c0dcf66a73e6daa8dc, deps_hash: 4fa387a1b6c60a579d6501dae1c21f4893d523fc340f7634c47e0203066e7680, spec_hash: 7cd4fb6b4c4824d1d0174a049993e2341f2ad124734635577cffd073de63c5e7 }
symbols:
  src/resolve.rs::ResolvedImport: { source_hash: cd2ed656551ee418709be01ed70aac96e08eee44317d1d84c71be83d182d32aa, deps_hash: ac583dfc44b8c03e1d77dc3191f47c59b6bb6a7b182fa0031e169bb47d564290, spec_hash: ec508b64f05d01e3324534034899277fba3d297b2565c47a9458d0429944eae7 }
  src/resolve.rs::build_resolver: { source_hash: 5b0fc1ffe0ead87102dd5596a25736b6dd501923d3296ea2a087d4885c70a6c9, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 544f7dee56518989e20066d205cecdef848471e1a35ce377cfd34bb74e6f46f4 }
  src/resolve.rs::resolve_imports: { source_hash: 24f79b4c1973b53c5f4962c41f34cdc7a3e373d2871b26494ad711b100c1e35d, deps_hash: e157b22158ede8851ce6e03839fa93365cde6fccb5297947ec39555d58222563, spec_hash: ba0968adcf5f7ef9fa7adfb572a877719c3a428504a5de59351f3b8daf29511d }
  src/resolve.rs::sorted_by_key: { source_hash: 617b8012a8c652159c02fb0db0c6b3717a03aa759e2c90c51bbdb9f5f24af7af, deps_hash: 63c3b1ffe0e29e0b2099b0ea5c298cc77ba1088b85aa141e80c2ea443933e0e7, spec_hash: af259c48185c75dd28565d418e3fab312196d6a6cb5b22749085c02f4067cdec }
  src/resolve.rs::ResolvedDefaultImport: { source_hash: 24e3e4afb7e18eb288bbe7a21bd7f68f81277504059fc061adfeca1948201e34, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: fe280be05b001159253f040782f07d5105ed8940143b86fa530a990ca506c8ad }
  src/resolve.rs::resolve_default_imports: { source_hash: f8dbeae49c58a45c30dfa1776f71e48ae74629d82ae8c563cc9d63a1dc7e442b, deps_hash: 63c3b1ffe0e29e0b2099b0ea5c298cc77ba1088b85aa141e80c2ea443933e0e7, spec_hash: e26d90f822496707b5d64c11d4d6b66e957fb27f3322ffae756f9242be577084 }
  src/resolve.rs::specifier_to_rel_path: { source_hash: 3229e450f1ad78c4d8aa1a1fdfabeff484b3da0f90e81a3eebfee7e092aeec95, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: e428f2b0de6536c5a640b85ed8e722dc77dae8f1f301e0f27687e854d6357087 }
  src/resolve.rs::ResolveCtx: { source_hash: 4ea7ffb0afff8c7971523cac1044fa0ec71f09f4889755a3c3ae5a2af38eb998, deps_hash: 4fa387a1b6c60a579d6501dae1c21f4893d523fc340f7634c47e0203066e7680, spec_hash: 3609f2720d378ee3f753158039948f6b5f6d8b4c58231f37303927e9c7748c73 }
---
# src/resolve.rs
## Summary
This file connects what each file imports (from `imports.rs`) to what symbols actually exist (from `graph.rs`), using `oxc_resolver` to work out what file a specifier like `'./foo'` or `'@/lib/foo'` really points at — producing the file-to-file reference edges the rest of CodeOwl builds on. Resolving a specifier happens in two steps: first `oxc_resolver` turns it into an absolute file path, the same way Node/TypeScript's own module resolution would (handling relative imports, `tsconfig.json` path aliases, and `node_modules` lookups); then CodeOwl looks for a symbol with the imported name declared in that file. If it isn't declared there directly, the target file might be a barrel forwarding it through a named re-export, so resolution chases through those re-exports, capped at a fixed number of hops so a circular barrel chain can't recurse forever. The same machinery, with the same symlink-canonicalization step, also resolves default imports (`import Name from './x'`) to their target file, which is what lets a rendered `<Component/>` be traced back to the file that defines it.

## `ResolvedImport`
`pub struct ResolvedImport`
### Summary
One `import` statement, after CodeOwl has worked out what it actually points at — the file-to-file connection that lets `get_callers`/`get_callees` answer "who uses this" across files.
### Behavior
`from_file`, `specifier`, and `imported_name` are carried straight over from the original `import` statement (the module string and the name being imported). `target` is the resolved `SymbolId` it points at — or `None` when resolution fails for any of three reasons: the specifier resolves outside the walked repo (an external package, not something CodeOwl indexes), the specifier doesn't resolve to any real file at all (a broken import), or it resolves to a real file but that file has no declaration, and no re-export chain, ending in `imported_name`. All three failure cases collapse to the same `None` — this struct doesn't distinguish which one happened.
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
Resolves every tracked `import` across the whole repo in one pass, turning each one from a raw specifier string into a `ResolvedImport` pointing at the actual symbol (or file) it names.
### Behavior
Takes one `FileImports` entry per walked file — its named imports and re-exports, as extracted by `imports::extract_imports` — and, for each import, calls `resolve_named` to follow it through any re-export chain (up to `MAX_REEXPORT_HOPS` hops) to its real target.

Before doing any of that, it canonicalizes `repo_root`. This matters because `oxc_resolver` (the module resolver CodeOwl uses, configured with symlinks enabled) hands back symlink-resolved paths, and turning a resolved absolute path back into a repo-relative one works by stripping `repo_root` as a prefix — which only succeeds if `repo_root` is in that same canonical form. Without this step, a repo living under a symlinked directory (macOS's `/var` being a symlink to `/private/var`, `/tmp`, or a symlinked clone) would have every single import resolve to `None`, since the prefix strip would silently fail on every path.

Files are walked in sorted order rather than whatever order the input map happens to hold them in, since the result is persisted to `.codeowl/graph` — a run-to-run-stable array is what makes that cache diffable between runs. Each file's own imports stay in their original source order.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- externals: oxc_resolver, std

## `sorted_by_key`
`fn sorted_by_key(file_imports: &HashMap<String, FileImports>) -> Vec<(&String, &FileImports)>`
### Summary
Returns a file-import map's entries sorted by path, so the resolvers that walk them get a deterministic order instead of whatever arbitrary order a hash map happens to hold.
### Behavior
Collects the map's entries into a `Vec` and sorts it by the path key (ascending). Used by both `resolve_imports` and the default-import resolver, since their output is persisted to `.codeowl/graph` and needs to come out the same way on every run for that cache to be diffable between runs.
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
Resolves every file's default imports to the file each one actually points at — the step that gives `assemble_participants` what it needs to follow a rendered `<Component/>` (a default-exported React component) into a feature's `core` set.
### Behavior
Walks every file's `default_imports`, in the same sorted, deterministic order `resolve_imports` uses, and turns each specifier into a repo-relative target file via `specifier_to_rel_path`. A default import whose specifier can't be resolved to a file inside the repo (an external package, or a broken import) is simply skipped — no `ResolvedDefaultImport` is produced for it. Canonicalizes `repo_root` first, for the same reason `resolve_imports` does: the module resolver hands back symlink-resolved paths, so `repo_root` needs to be in that same form for the prefix-stripping that turns an absolute path back into a repo-relative one to succeed at all.
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
A bundle of the read-only context every import-resolution step needs, so it can be passed around as one value instead of four separate parameters threaded through every recursive call.
### Behavior
Holds `repo_root`, the module `resolver`, every file's extracted `file_imports`, and the `graph` built so far — everything `resolve_named` needs to look a name up.

`resolve_named(from_file, specifier, name, hops_left)` is where the actual named-import resolution happens. It first turns `specifier` into a repo-relative target file (bailing out with `None` if that fails). If the target file directly declares a symbol matching `"<target_file>::<name>"`, that's the answer. Otherwise, the name might be forwarded through a re-export instead — a barrel file passing along a name it doesn't declare itself — so it looks for a `ReExport` in the target file matching `name`, and if found, recurses into *that* re-export's own specifier and source name to keep following the chain. `hops_left` caps how many re-export hops this recursion will follow, decrementing by one each time and returning `None` once it hits zero — the guard against a circular barrel chain (file A re-exports from B, B re-exports from A) recursing forever.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- externals: oxc_resolver, std
