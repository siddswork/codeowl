---
kind: file
source_paths: [src/stack.rs]
file: { source_hash: 63f4cd991d31752bc1a5821084383bb04cbd5ebaeb867b4902b703b4421d3d4f, deps_hash: 6764a92ab043b46adb7dc3a066c415dab8f4f593b24a1e58deb5774d8b9dae7c, spec_hash: 27a1039d903148d6c88e84fb727193af6f2fef20364ed220f6fd24f8e6c9118a }
symbols:
  src/stack.rs::StackPack: { source_hash: 95be6e6095a5a9f1978b69750da7237db9948eba5234540f5ac1f55759e14d2a, deps_hash: 6764a92ab043b46adb7dc3a066c415dab8f4f593b24a1e58deb5774d8b9dae7c, spec_hash: 71eceb48962b96f8d3f8c0aaee3b310ce404ca89c6c8cb9e780b69536ff532ed }
  src/stack.rs::TypeScriptNextStack: { source_hash: 7339c1425f2b965667ba321a2387463f356a220700b37b33797914741fd59478, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 6ac5cb7081844dcaab4a7e9bed55637b4533f871c4ccd09c02d0fa03e6d6c02e }
  src/stack.rs::impl StackPack for TypeScriptNextStack: { source_hash: 730d2861e00ac27877748503d7dd63b7e9d5ad20c142a9b6ef6b67b59745f7f6, deps_hash: 6764a92ab043b46adb7dc3a066c415dab8f4f593b24a1e58deb5774d8b9dae7c, spec_hash: 8e7b86b6a33397254f54e06887287321c1c7b708485b5fc4d39d386e4e62dd18 }
  src/stack.rs::typescript_next: { source_hash: cb24ee687e22da4b567b250471dd57db5916521afe114f5c2499fc4db9d3b5dd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 1ffb78ce26c00315e674297ea1c1f7cf375e0bc08613f11a613d3f8cdbb89ccd }
  src/stack.rs::for_name: { source_hash: b353bad09b360e2c0dbf1f005ff8208dc43a4747ba5c733d8514380a0fd1b69c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b0f25d00e589f851fe09d1ce1f4ae103a03ac07e1c0ed46e5073093851bff1d8 }
  src/stack.rs::RustStack: { source_hash: 91296fba1e7d0f33966d98e6a33c0a0056415721b8236fc7f4424eba9a958839, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 177d7d15f8e2e6a377597734c46e611d998e3ba991c487c46ca6a7786a0a2d31 }
  src/stack.rs::impl StackPack for RustStack: { source_hash: 509c270d48147996cd85dfa4d9a75672eec3383f804815839f32a2d5877c143b, deps_hash: 6764a92ab043b46adb7dc3a066c415dab8f4f593b24a1e58deb5774d8b9dae7c, spec_hash: 3f3bf1304f679853c6e2c4f1e7e0f7a21a94307c93dc1620e7ffcf7d9bf3ae21 }
  src/stack.rs::JavaStack: { source_hash: 02294a4678dbdfc183faa54c1bfb7a1bc4d1d418948fd783688754a19ee191d8, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 34a34d689af669763493e805bb32f8f27b6211f364e30f268b1a8e4d7dbf4e94 }
  src/stack.rs::impl StackPack for JavaStack: { source_hash: c7c36c46c2c1dd733ef210d2abaa5c2114149002b7a358b1ecc5b6d401c4fc5f, deps_hash: 6764a92ab043b46adb7dc3a066c415dab8f4f593b24a1e58deb5774d8b9dae7c, spec_hash: c7c1a1036e479849376f6aebece5fe4a38c777c2adbdae7c29bb88c435e8bc85 }
  src/stack.rs::PythonStack: { source_hash: cf85c5bf7972b4faca37864abda6e8b8c9abbefd8144706164e209f7ca6458d0, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 958a3a06fdd405625560dc4478740955a802c834d273ffae151356de010d40ac }
  src/stack.rs::python_class_is_table: { source_hash: 64f56ffff47a70faf7e423e2b8e06a5f57879073180d4f1a1a1b10f5bac9d195, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3aa48a2bc55e703955eaf193b5d1e01433b7a96f934082e6198267b53cf35bc9 }
  src/stack.rs::impl StackPack for PythonStack: { source_hash: d0c02f9a5bb7dd91af5817695de0d558576c78473ac75a95414d649730c23155, deps_hash: 6764a92ab043b46adb7dc3a066c415dab8f4f593b24a1e58deb5774d8b9dae7c, spec_hash: 124b90cbf9b3b284d98f4b0ce4518cd9f8eacef672b3b9cd606f1e3851cbc617 }
---
# src/stack.rs
## Summary
This file defines `StackPack`, the one interface every language/framework-specific decision in CodeOwl passes through — parsing a file, resolving its imports, recognizing its "flow edges" (looser cross-file links a plain import graph can't see, like a URL string or a rendered UI component), and knowing what counts as a "feature" (an end-to-end capability like a web route) if the stack has one at all. Exactly one implementation is chosen per repo. This file holds four: `TypeScriptNextStack` (TypeScript/TSX + SQL schema files + Next.js routing + Supabase-style database queries), `RustStack` (plain Rust, no feature layer since a library/CLI has no obviously enumerable entry points), `JavaStack` (plain Java, likewise no feature layer), and `PythonStack` (Python + FastAPI, which does have routes as features, and recognizes SQLModel/SQLAlchemy ORM model classes as database tables). Each implementation mostly just calls into that language's own dedicated extraction/resolution code living in its own file (`lang.rs`, `rust.rs`, `java.rs`, `python.rs`, `fastapi.rs`) — this file's job is wiring, not the actual parsing logic. It also has a `for_name` lookup so code that only has an already-built graph (with no live reference to which pack built it) can recover the right pack by its saved name.</content>

## `StackPack`
`pub trait StackPack: Send + Sync + std::fmt::Debug`
### Summary
The interface every supported language/framework ("stack" — TypeScript/Next.js, Rust, Java, Python) implements to plug into CodeOwl's otherwise language-agnostic core. Exactly one implementation is chosen per repo (by `lang::detect`); everything the generic indexing, resolution, and feature-detection code needs a specific language to decide about — how to parse it, what counts as an import, what a "feature" even is — goes through this trait instead of being hard-coded per language.
### Behavior
`name` returns a stable identifier persisted alongside the on-disk cache, so switching which stack indexed a repo forces a full rebuild rather than reusing a stale, wrongly-shaped cache. `source_kind` says whether a given file is something this pack reads at all, and if so whether it's ordinary code or a dedicated schema file. `classify` labels a path's role (ordinary product code, shared UI primitive, test code, generated code) for spec-prioritization purposes. `extract_symbols` parses one file's text into its declarations. `extract_imports` parses one file's import/re-export statements; `resolve_imports` then resolves every tracked import across the whole repo to the symbol it actually points at (or `None` for an external package or an import that couldn't be resolved) — each pack owns its own resolver end to end (a proper module resolver for TypeScript, a directory-tree walk for Rust), rather than sharing one generic resolution algorithm.

`extract_flow_edges` and `resolve_flow_edge` handle "flow edges" — looser cross-file links the plain import graph can't see, like a web request path literal, a database query naming a table by string, or a rendered UI component tag (see `GLOSSARY.md`). A pack with no such conventions just returns an empty list from the extractor. `feature_model` returns the pack's notion of "what counts as a feature" (e.g. a web route), or `None` if the stack has no runtime entry surface at all — a CLI tool or plain library gets file/rollup/system-level documentation but no per-feature documents. `is_schema_symbol` (defaulting to `false`) lets a pack flag an individual extracted symbol — not just a whole file — as a database table, so an in-language ORM model class (e.g. a Python class with `table=True`) gets recognized as schema even though it lives in an ordinary code file rather than a dedicated `.sql` file; the generic indexing pipeline retags anything this returns `true` for.</content>
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
Wires the TypeScript + Next.js language stack into CodeOwl's generic `StackPack` interface, mostly by delegating each method to the TS-specific logic that already lives in `lang.rs`, `schema.rs`, `imports.rs`, `resolve.rs`, and `features.rs`.
### Behavior
`name` returns `"typescript-next"`. `source_kind` maps `.sql` files to `Schema` and defers everything else to `SourceKind::of` (TS/TSX → `Code`, anything else → not recognized). `classify` and `extract_imports` are thin pass-throughs to `lang::classify` and `imports::extract_imports`. `extract_symbols` branches on extension: `.sql` files go to `schema::extract_tables` (parses `CREATE TABLE` statements), everything else goes to `lang::extract_symbols`. `resolve_imports` builds a fresh module resolver each call (`resolve::build_resolver`) and runs `resolve::resolve_imports` with it.

`extract_flow_edges` is where the three TypeScript-specific "flow edge" conventions (see `GLOSSARY.md`) get collected into one generic list: `fetch("/api/...")` calls become `"route-literal"` edges, `.from("table")` calls become `"table-ref"` edges, and rendered `<Component/>` JSX tags become `"rendered-component"` edges — each tagged with its kind and the raw string matched, unresolved at this point. `resolve_flow_edge` then dispatches each edge to the matching resolver by its `kind` string (route path → the file serving it, table name → the schema symbol, component tag → the file it was imported from), falling back to `FlowTarget::Unresolved` for an edge whose raw string points nowhere or an unrecognized kind. `feature_model` returns the TypeScript/Next.js feature model (routes and pages as entry points), so this stack does get per-feature specs, unlike a stack with no runtime entry surface.</content>
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
Looks up the right `StackPack` implementation by its saved name string, so code that only has a `Graph` (already built, with no live reference to the pack that built it) can still get back the pack's behavior — used by read-only callers like the spec generator and the MCP server.
### Behavior
A direct match on the string: `"rust"` → `RustStack`, `"java"` → `JavaStack`, `"python"` → `PythonStack`, anything else (including an unrecognized name, an empty string, or a graph built before this pack-name stamping existed) falls back to `TypeScriptNextStack`. This fallback matters for backward compatibility with old cache files and for test-built graphs, which never call `Graph::set_pack_name` and so default to an empty name.</content>
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

## `PythonStack`
`pub struct PythonStack`
### Summary
The Python language stack (added in the M17 milestone): parses `.py` files with the `tree-sitter-python` grammar, resolves Python's dotted-module imports (e.g. `from a.b import c`), and recognizes FastAPI routes as features and SQLModel/SQLAlchemy ORM model classes as database tables. Proven against a real FastAPI + SQLModel backend (`full-stack-fastapi-template`).
### Behavior
A zero-sized marker struct (`pub struct PythonStack;`) — all its behavior lives in its `StackPack` trait implementation rather than any stored state. `source_kind` recognizes only `.py` files. `classify` sorts files into test code (a `tests/` directory, `conftest.py`, `test_*.py`, `*_test.py`), Alembic/migration files under `.../versions/` (treated as generated, since they describe schema *changes* rather than current state and don't participate in imports), or ordinary domain code. `extract_symbols`, `extract_imports`, `resolve_imports`, and `extract_flow_edges`/`resolve_flow_edge` all delegate to the Python-specific logic in `python.rs`. `feature_model` returns the FastAPI feature model (`fastapi.rs`'s `FastApiFeatureModel`), so — unlike the docstring on this struct in the source, which is now out of date — routes decorated as FastAPI endpoints do get their own per-feature specs, not just file-level ones. `is_schema_symbol` (defined just below this struct) flags a leaf class as a database table when it's a SQLModel class with `table=True` or subclasses a SQLAlchemy declarative base, letting an in-code ORM model be recognized as schema without needing a separate `.sql` file.</content>
### Depends on
- (none)

## `python_class_is_table`
`fn python_class_is_table(sig: &str) -> bool`
### Summary
Decides whether a Python class declaration is a database table (an ORM — object-relational mapper — model, i.e. Python code that represents a table), by inspecting the class's signature line: its name and parenthesized base classes/keyword arguments (e.g. `class Item(SQLModel, table=True):`).
### Behavior
Finds the parenthesized part of the signature and splits it on commas into individual base-class names or keyword arguments. Returns `true` if any of them, once whitespace is stripped, is exactly `table=True` (SQLModel's convention for marking a model as a real table) or is the bare base class `Base` or `DeclarativeBase` (SQLAlchemy's declarative-base conventions). Deliberately returns `false` for a bare `SQLModel` or `BaseModel` base with no `table=True` — those represent request/response validation schemas, not database tables. The comma split is naive: a base class written with a nested comma, such as a generic type like `Generic[T, U]`, could misparse, but that shape is considered unlikely for a real ORM model class.</content>
### Depends on
- (none)

## `impl StackPack for PythonStack`
`impl StackPack for PythonStack`
### Summary
Wires the Python stack into CodeOwl's generic `StackPack` interface, mostly by delegating to Python-specific logic in `python.rs` and `fastapi.rs`.
### Behavior
`name` returns `"python"`. `source_kind` recognizes only `.py` files as code. `classify` labels a file's role by filename/path convention: files under a `tests/` directory, `conftest.py`, or files matching `test_*.py`/`*_test.py` are `Test`; files under an Alembic or Django-style `.../migrations/versions/` directory are `Generated` (migrations describe schema changes rather than current state, and typically have no meaningful imports, so they're treated like generated code); everything else is ordinary `Domain` code. `extract_symbols`, `extract_imports`, `resolve_imports`, `extract_flow_edges`, and `resolve_flow_edge` all delegate straight to their equivalents in `python.rs`. `feature_model` returns FastAPI's feature model (`fastapi.rs`'s `FastApiFeatureModel`), so FastAPI routes get their own feature specs.

`is_schema_symbol` decides whether one extracted symbol is a database table: it must be a class (`sym.raw == "class"`), a leaf one with no nested children (a class with its own methods is treated as ordinary code, not purely a table — refining that is left for later if a real repo needs it), and its signature must pass `python_class_is_table`'s check for a SQLModel `table=True` or SQLAlchemy declarative-base pattern.</content>
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
