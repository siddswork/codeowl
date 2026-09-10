---
kind: file
source_paths: [src/stack.rs]
file: { source_hash: ed8efcb63a2ffadaa460b6cd23c55916189ee0f9d01871ea598caed80278ed9d, deps_hash: 6764a92ab043b46adb7dc3a066c415dab8f4f593b24a1e58deb5774d8b9dae7c, spec_hash: 22f62ed3679398541c765ef2f4c9410debfd4f4a96b2bc92e93041d03709390e }
symbols:
  src/stack.rs::StackPack: { source_hash: 6886add43e5b9559534f9841974bc954ca8e717d96dc24f20c0fd9a175528df9, deps_hash: 6764a92ab043b46adb7dc3a066c415dab8f4f593b24a1e58deb5774d8b9dae7c, spec_hash: e4d9376a0ad2a65818ef703ea2b31aa2d1e008bdd3fdbb52d08250ccadb39bf2 }
  src/stack.rs::TypeScriptNextStack: { source_hash: 7339c1425f2b965667ba321a2387463f356a220700b37b33797914741fd59478, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 6ac5cb7081844dcaab4a7e9bed55637b4533f871c4ccd09c02d0fa03e6d6c02e }
  src/stack.rs::impl StackPack for TypeScriptNextStack: { source_hash: 824cbf0677d5505e79bc0d05a29f98383edb75d29e92b93e03fbdb1ea862a308, deps_hash: 6764a92ab043b46adb7dc3a066c415dab8f4f593b24a1e58deb5774d8b9dae7c, spec_hash: affc5d69faaf884e7d5ca4781f8527f33ed621eb8175efa2d0206cfc75c03153 }
  src/stack.rs::typescript_next: { source_hash: cb24ee687e22da4b567b250471dd57db5916521afe114f5c2499fc4db9d3b5dd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 1ffb78ce26c00315e674297ea1c1f7cf375e0bc08613f11a613d3f8cdbb89ccd }
  src/stack.rs::for_name: { source_hash: 4f5c1c40392de712b24e61ae5ce30069630befe00d3863f39882ec2c3f1b29ec, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 78ddbe8251e88e6d38733108590d8332c8fb828db66d6c24f48d5f918e3cccd7 }
  src/stack.rs::RustStack: { source_hash: 91296fba1e7d0f33966d98e6a33c0a0056415721b8236fc7f4424eba9a958839, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 177d7d15f8e2e6a377597734c46e611d998e3ba991c487c46ca6a7786a0a2d31 }
  src/stack.rs::impl StackPack for RustStack: { source_hash: 509c270d48147996cd85dfa4d9a75672eec3383f804815839f32a2d5877c143b, deps_hash: 6764a92ab043b46adb7dc3a066c415dab8f4f593b24a1e58deb5774d8b9dae7c, spec_hash: 3f3bf1304f679853c6e2c4f1e7e0f7a21a94307c93dc1620e7ffcf7d9bf3ae21 }
  src/stack.rs::JavaStack: { source_hash: 02294a4678dbdfc183faa54c1bfb7a1bc4d1d418948fd783688754a19ee191d8, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 34a34d689af669763493e805bb32f8f27b6211f364e30f268b1a8e4d7dbf4e94 }
  src/stack.rs::impl StackPack for JavaStack: { source_hash: c7c36c46c2c1dd733ef210d2abaa5c2114149002b7a358b1ecc5b6d401c4fc5f, deps_hash: 6764a92ab043b46adb7dc3a066c415dab8f4f593b24a1e58deb5774d8b9dae7c, spec_hash: c7c1a1036e479849376f6aebece5fe4a38c777c2adbdae7c29bb88c435e8bc85 }
---
# src/stack.rs
## Summary
Defines `StackPack` — the single interface every language-and-framework-specific decision in CodeOwl passes through. A pack answers a fixed set of questions: which files to read, how to label each one (product / test / generated), how to parse a file into symbols, how to parse and resolve its imports, which string-carried "flow" edges to look for (a `fetch("/api/...")`, a `.from("table")`), and — optionally — a model of "what is a feature." The generic machinery (`spec.rs`, `mcp.rs`, `graph.rs`, `index.rs`) only ever calls these methods and never names a stack directly. This file also holds the three implementations — `TypeScriptNextStack` (TypeScript + SQL + Next.js + Supabase — the original), `RustStack`, and `JavaStack`, each a data-less marker type — plus `for_name` and `typescript_next`, which build a pack from its stored name. `lang::detect` picks the one pack for a repo.

## `StackPack`
`pub trait StackPack: Send + Sync + std::fmt::Debug`
### Summary
The trait that captures everything the generic pipeline needs a stack to decide — extraction, resolution, file classification, flow edges, and optionally a feature model. One `StackPack` per repo, chosen by `lang::detect`. Every stack assumption lives behind this; the pipeline around it names no framework convention.
### Behavior
Nine methods. `name` is a stable id persisted with the cache so a pack switch forces a rebuild. `source_kind` decides which files are walked and which extractor they go to (replacing `is_extractable` + `SourceKind::of`). `classify` gives a path's `FileRole` for spec prioritisation. `extract_symbols` / `extract_imports` parse one file. `resolve_imports` owns resolver construction end to end — `oxc_resolver` for TS, a module-tree walk for Rust — and returns every edge, unresolved ones included. `extract_flow_edges` / `resolve_flow_edge` are the string-carried-edge pair (a pack with no such conventions returns empty / never gets called). `feature_model` returns `Some` only for a stack with a runtime entry surface; it defaults to `None`, and returns `&'static` because a feature model is a stateless ZST recoverable from just the pack name via `for_name`. `Send + Sync + Debug` because the file watcher carries the active pack to a background thread and `RepoIndex` holds one.
### Depends on
- `src/graph.rs::FlowTarget` — crate::graph
- `src/graph.rs::Graph` — crate::graph
- `src/graph.rs::UnresolvedFlowEdge` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- `src/lang.rs::FileRole` — crate::lang
- `src/lang.rs::SourceKind` — crate::lang
- `src/resolve.rs::ResolvedImport` — crate::resolve
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- externals: std

## `TypeScriptNextStack`
`pub struct TypeScriptNextStack`
### Summary
The Phase-1 `StackPack`: TypeScript / TSX plus SQL schema files, Next.js App Router routing, Supabase `.from()`. A zero-size unit struct.
### Behavior
Every trait method delegates to a free function that already existed before M13 — `extract.rs`, `imports.rs`, `resolve.rs`, `lang.rs`, `schema.rs`, `features.rs`. M13 was a re-wiring, not a rewrite. `name()` is `"typescript-next"`; `feature_model()` returns `Some(TypeScriptNextFeatureModel)`.
### Depends on
- (none)

## `impl StackPack for TypeScriptNextStack`
`impl StackPack for TypeScriptNextStack`
### Summary
Wires the `StackPack` trait to the TypeScript pack's free functions. Almost every method is a one-line delegation; the two with real logic are `extract_flow_edges` and `resolve_flow_edge`.
### Behavior
`name` → `"typescript-next"`. `source_kind` → `SourceKind::of`. `classify` / `extract_symbols` / `extract_imports` → the matching `lang.rs` / `imports.rs` function. `resolve_imports` builds a fresh `oxc_resolver` and calls `resolve::resolve_imports`. `feature_model` → `Some(default_feature_model())`.

`extract_flow_edges` runs all three narrow extractors (`extract_route_literals`, `extract_table_refs`, `extract_rendered_components`) and tags each result with its `kind` string (`"route-literal"` / `"table-ref"` / `"rendered-component"`) into a uniform `UnresolvedFlowEdge`. `resolve_flow_edge` dispatches back on that `kind` to the matching resolver; the `rendered-component` case resolves to a file path first, then to that file's `SymbolId`. Anything that resolves to nothing becomes `FlowTarget::Unresolved` rather than being dropped.
### Depends on
- `src/graph.rs::FlowTarget` — crate::graph
- `src/graph.rs::Graph` — crate::graph
- `src/graph.rs::UnresolvedFlowEdge` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- `src/lang.rs::FileRole` — crate::lang
- `src/lang.rs::SourceKind` — crate::lang
- `src/resolve.rs::ResolvedImport` — crate::resolve
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- externals: std

## `typescript_next`
`pub fn typescript_next() -> Box<dyn StackPack>`
### Summary
Boxes `TypeScriptNextStack` as a `Box<dyn StackPack>` — the trait-object shape `RepoIndex` and the test fixtures want.
### Behavior
`Box::new(TypeScriptNextStack)`, nothing else. It's the fallback a `RepoIndex` uses before `load` re-runs `detect` for a real repo — the real per-repo choice lives in `lang::detect`, not here.
### Depends on
- (none)

## `for_name`
`pub fn for_name(name: &str) -> Box<dyn StackPack>`
### Summary
Given a pack name string — `"rust"`, `"java"`, or anything else (treated as `"typescript-next"`) — returns the matching `StackPack`. Read-only callers like `spec.rs` and `mcp.rs` use this to recover the pack from the name stored in the cached graph, without it being passed down to them.
### Behavior
A plain match on the name: `"rust"` → `RustStack`, `"java"` → `JavaStack`, anything else — an unknown name, or an empty string from a test-built graph or a cache older than the pack system — falls back to `TypeScriptNextStack`. Each pack holds no data, so constructing one costs nothing.
### Depends on
- (none)

## `RustStack`
`pub struct RustStack`
### Summary
The Rust `StackPack` (M14): `tree-sitter-rust` extraction over `.rs` files, module-tree `use` resolution, no feature layer. Exercised on CodeOwl's own repo. A zero-size unit struct.
### Behavior
`name()` is `"rust"`. `source_kind` returns `Code` for `.rs` and `None` otherwise. `classify` puts `tests/` / `benches/` files in the `Test` role. `extract_symbols` / `extract_imports` / `resolve_imports` delegate to `rust.rs`. `extract_flow_edges` returns an empty vec — a Rust service's cross-file reach is plain function calls, and call-graph analysis is deferred (same stance as M10). `feature_model()` takes the trait default `None`: CodeOwl has cross-cutting workflows but no mechanically enumerable entry surface, so its corpus is symbol / file / rollup / system specs with a `## Key flows` system section instead of feature specs.
### Depends on
- (none)

## `impl StackPack for RustStack`
`impl StackPack for RustStack`
### Summary
Wires the `StackPack` trait to `rust.rs`. Extraction, imports, and resolution delegate there; the flow-edge and feature-model methods are deliberate no-ops.
### Behavior
`name` → `"rust"`. `source_kind` → `Code` iff the extension is exactly `rs`. `classify` returns `Test` for a `tests/` or `benches/` prefix or a `/tests/` segment, `Domain` otherwise — Rust has no `components/ui`-style primitive tier, and `target/` is gitignored so the walk never reaches it. `extract_symbols` / `extract_imports` / `resolve_imports` call the `rust::` functions directly (no separate resolver object — Rust resolution is a filesystem-convention walk, not `oxc_resolver`). `extract_flow_edges` returns empty and `resolve_flow_edge` always returns `Unresolved`; `feature_model` isn't overridden, so it takes the trait default `None`.
### Depends on
- `src/graph.rs::FlowTarget` — crate::graph
- `src/graph.rs::Graph` — crate::graph
- `src/graph.rs::UnresolvedFlowEdge` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- `src/lang.rs::FileRole` — crate::lang
- `src/lang.rs::SourceKind` — crate::lang
- `src/resolve.rs::ResolvedImport` — crate::resolve
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- externals: std

## `JavaStack`
`pub struct JavaStack`
### Summary
The Java language pack (M16) — CodeOwl's adapter for plain classic Java projects, first exercised on Apache commons-lang. An empty marker type; the actual behavior lives in its `impl StackPack` block.
### Behavior
Selecting this pack (via `lang::detect`, or `stack::for_name` from a cached graph) tells the rest of CodeOwl to read the repo as Java. Its `impl StackPack` supplies the specifics: `.java` files parsed with `tree-sitter-java`; imports resolved by the `src/main/java` package-directory layout rather than by reading `pom.xml`, so a Gradle project works the same way, plus a source scan to catch same-package references that carry no `import` at all; no feature model, because a utility library has no enumerable runtime entry points and its public API is already covered by the symbol and file specs; no flow edges, since Java call-graph analysis is deferred (the same stance as the Rust pack). Framework support — `@Path` endpoints and annotation-driven edges for Java *services* — arrives in M17 with Quarkus.
### Depends on
- (none)

## `impl StackPack for JavaStack`
`impl StackPack for JavaStack`
### Summary
Wires the Java pack into CodeOwl's generic pipeline. Each `StackPack` method here either states one Java-specific rule (which files to read, how to label them) or forwards straight to the real work in `src/java.rs`.
### Behavior
Method by method:
- `name` → `"java"` — the string written into the cache to remember which pack built the graph.
- `source_kind` — a file is Java source only if its extension is `.java`; everything else is ignored.
- `classify` — a path under `src/test/` or `/test/java/` is `Test`; one under `target/generated-sources/` or `build/generated/` is `Generated`; anything else is product code (`Domain`). Java has no `components/ui`-style "primitive" tier, so that role is never returned.
- `extract_symbols` / `extract_imports` / `resolve_imports` — forwarded to `crate::java`.
- `extract_flow_edges` returns nothing and `resolve_flow_edge` always returns `Unresolved`: Java has no string-carried "this reaches that" edges CodeOwl tracks yet (call-graph analysis is deferred), so there is nothing to extract or resolve.

`feature_model` is not overridden, so it takes the trait's default `None` — a Java library gets no feature layer.
### Depends on
- `src/graph.rs::FlowTarget` — crate::graph
- `src/graph.rs::Graph` — crate::graph
- `src/graph.rs::UnresolvedFlowEdge` — crate::graph
- `src/imports.rs::FileImports` — crate::imports
- `src/lang.rs::FileRole` — crate::lang
- `src/lang.rs::SourceKind` — crate::lang
- `src/resolve.rs::ResolvedImport` — crate::resolve
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- externals: std
