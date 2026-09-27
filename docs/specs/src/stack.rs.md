---
kind: file
source_paths: [src/stack.rs]
file: { source_hash: 206306bc8ad7415fded7f553e05e21b920cb193dd6217469b598a478f9eb37f2, deps_hash: 293cfcb9ec774db934b090fa20d51039cd0ab7abdc517a9efd20d2a087c9aca0, spec_hash: a6bbdc67fbe7c286ab1c4832a777260bcd7e9f52af41293c5c31241197439336 }
symbols:
  src/stack.rs::StackPack: { source_hash: e8bf278f73aee557c8c9e3abf34cbf8d4a94eaf40076760d5bd9efb6d8d59d63, deps_hash: 293cfcb9ec774db934b090fa20d51039cd0ab7abdc517a9efd20d2a087c9aca0, spec_hash: c314aa0ef27ffc0e34dada40fc19f3c5db4fe2025191c03f7423f537bd2b246b }
  src/stack.rs::TypeScriptNextStack: { source_hash: 5419c848b51ed5ec0b2ce00f60daa57b588f75a610504e9a5492f5e7ee5d6f76, deps_hash: 293cfcb9ec774db934b090fa20d51039cd0ab7abdc517a9efd20d2a087c9aca0, spec_hash: 95ca528b04ae6a1ffc6722ddd5fc355c0c74c5a12943b91c43e7c5402f1a650c }
  src/stack.rs::typescript_next: { source_hash: cb24ee687e22da4b567b250471dd57db5916521afe114f5c2499fc4db9d3b5dd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 1ffb78ce26c00315e674297ea1c1f7cf375e0bc08613f11a613d3f8cdbb89ccd }
  src/stack.rs::for_name: { source_hash: b353bad09b360e2c0dbf1f005ff8208dc43a4747ba5c733d8514380a0fd1b69c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b0f25d00e589f851fe09d1ce1f4ae103a03ac07e1c0ed46e5073093851bff1d8 }
  src/stack.rs::RustStack: { source_hash: a1d35afae07afa7c288a4220da929a7622554b1b2ec24d7e41746a28d12dce62, deps_hash: 293cfcb9ec774db934b090fa20d51039cd0ab7abdc517a9efd20d2a087c9aca0, spec_hash: 309330b49320d3a6b382822af7610a2fd18d2e5cf080094f82c09c5ba822257e }
  src/stack.rs::JavaStack: { source_hash: 2f39dfcb753e73b346567a8f6e4beae03da47877a78127b69f98ce31a3468eb2, deps_hash: 293cfcb9ec774db934b090fa20d51039cd0ab7abdc517a9efd20d2a087c9aca0, spec_hash: abacc2a7f1491d8f764de72a1b7b92d8ce5da89ecaf2adab95332bd21a927ee8 }
  src/stack.rs::is_entity_annotation: { source_hash: ce1a6139fe9840ed1abb3d032dada299f406e73066d020c796095909d1b0ff34, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 13d2eeb4d01014ee7f262505500e82152a2c6f28b88e95e541c57b3b467ae2ec }
  src/stack.rs::extends_panache_entity: { source_hash: 0cfc02821c8322cdf4aa5897f984c2dc859a4ddeac1a29f47cd52c2fadada0aa, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a40e80c1a9361bb9090d6835090204ec9c668e5ec9a4abbd99492f3eb352d38d }
  src/stack.rs::strip_angle_bracket_groups: { source_hash: d6e7b6f3666e52d83651ceda7eba5a2237af89d577817cb34c7bf91d859ea6b8, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: d51aa273175c726fc07a5a8c1ef4d1cbaff5d1d9d3b338545fd814496cb3eb46 }
  src/stack.rs::PythonStack: { source_hash: 92c4620f31be21ec4d356ea806055317e6631b236277d7fd17044b661e757b1f, deps_hash: 293cfcb9ec774db934b090fa20d51039cd0ab7abdc517a9efd20d2a087c9aca0, spec_hash: 05daece0e6e3955702d9a27238b6714ec91a0f1ec58bd05c0ff5d3dd89353c71 }
  src/stack.rs::python_class_is_table: { source_hash: 64f56ffff47a70faf7e423e2b8e06a5f57879073180d4f1a1a1b10f5bac9d195, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3aa48a2bc55e703955eaf193b5d1e01433b7a96f934082e6198267b53cf35bc9 }
---
# src/stack.rs
## Summary
This file defines `trait StackPack` — the single interface every stack-specific decision passes through, and the seam that lets CodeOwl's generic core (extraction, resolution, spec assembly) stay entirely language-agnostic. Each supported language/framework combination gets its own implementing type — `TypeScriptNextStack`, `RustStack`, `JavaStack`, `PythonStack` — and exactly one is chosen per repo by `detect`. Most of a `StackPack` impl is thin delegation to the free functions already living in that language's own module (`extract.rs`/`lang.rs` for TypeScript, `rust.rs`, `java.rs`, `python.rs`); the file also holds the small stack-specific helper functions those impls need directly, like the Java table-recognition helpers for JPA entities and Panache active-record classes.

## `StackPack`
`pub trait StackPack: Send + Sync + std::fmt::Debug`
### Summary
The interface between CodeOwl's generic core and one specific language/framework combination ("stack") — everything the core needs a stack to decide, so the core itself never has to know whether it's looking at TypeScript, Rust, Python, or Java. Exactly one `StackPack` is chosen per repo, by `detect`.
### Behavior
`name` is a stable identifier for the pack, persisted alongside the cache so switching which pack built a repo's index forces a rebuild rather than silently mixing formats.

`source_kind` decides which extractor a file's contents should go to (or `None` if the pack reads nothing from that file at all) — this is what makes a file "walkable" in the first place. `classify` decides the role a repo-relative path plays for spec prioritization (ordinary product code vs. a UI primitive vs. test code vs. generated code).

`extract_symbols` parses one file into its declarations, and `extract_imports` parses its `import`/re-export statements — the pack picks the grammar and maps its own node kinds onto CodeOwl's generic `SymbolKind`. `resolve_imports` resolves every tracked import across the whole repo to a target symbol (or `None` for an external package or an import that doesn't resolve) — the pack owns building its own resolver end to end (`oxc_resolver` for TypeScript, a module-tree walk for Rust, since each language's import semantics are different enough that there's no one generic resolver that fits all of them).

`extract_flow_edges` finds the "this file reaches that thing" connections a structural import graph can't see (for the TypeScript+Next stack: a `fetch("/api/…")` call, a `.from("table")` query, a rendered `<Component/>`) as raw, unresolved strings — a stack with no such conventions just returns an empty list. `resolve_flow_edge` resolves one of those edges against the already-built graph, returning an unresolved marker for a raw string that points nowhere (an external URL, a database view, a dynamic path, a typo).

`feature_model` returns the stack's model of "what counts as a feature," or `None` if the stack has no runtime entry surface at all — a CLI or a plain utility library still gets symbol, file, rollup, and system specs, just no feature layer. It's `'static` because a feature model carries no per-repo state of its own, so it can be recovered from just a pack's name rather than needing to stay tied to a specific `StackPack` instance's lifetime.

`is_schema_symbol` asks, of a symbol the pack just extracted, whether it's actually a database table — the hook that lets an in-language ORM model (a Python class with `table=True`, a JPA `@Entity`) get retagged as a schema table without needing its own dedicated schema file the way a `.sql` file does. The default answer is "no, this stack has no persistence model."

`generated_source_dirs` names the repo-relative, build-output directories a pack knows can hold real source-language files generated from a declarative contract (an OpenAPI spec, a `.proto` file, a GraphQL schema) — Maven's `target/generated-sources`, Gradle's `build/generated`, for the Java stack. The default is no known convention. CodeOwl's file walk reads through whatever directories are named here regardless of `.gitignore`, since these directories are exactly what an ordinary `.gitignore` excludes — a repo that's never actually been built locally simply has nothing there yet, which is expected, not an error.
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
The `StackPack` for TypeScript/TSX with Next.js App Router routing and SQL schema files (Supabase's `.from()` convention) — the first stack CodeOwl supported. A marker type (no fields); every trait method delegates to the existing free functions in `extract.rs`, `imports.rs`, `resolve.rs`, `lang.rs`, `schema.rs`, and `features.rs`, so this type is wiring, not new logic.
### Behavior
`name` returns `"typescript-next"`. `source_kind` maps a `.sql` extension to `SourceKind::Schema`; everything else falls through to `lang.rs`'s ordinary TypeScript source-kind check. `classify` delegates straight to `lang::classify`. `extract_symbols` branches the same way `source_kind` does: a `.sql` file goes to the schema table extractor, anything else to the ordinary TypeScript symbol extractor. `extract_imports` delegates to `imports::extract_imports`.

`resolve_imports` builds a fresh TypeScript module resolver (`resolve::build_resolver`) and hands it to `resolve::resolve_imports` along with the repo root, the collected imports, and the graph.

`extract_flow_edges` runs all three of this stack's raw-edge extractors — route literals, table references, and rendered components — and tags each result with its own `kind` string (`"route-literal"`, `"table-ref"`, `"rendered-component"`) before collecting them into one list. `resolve_flow_edge` dispatches back out by that same `kind` string: a route literal resolves via the Next.js path-convention resolver, a table reference via the schema-table resolver, and a rendered component first resolves to its source file and then looks that file up in the graph. Any edge with an unrecognized `kind` resolves to nothing.

`feature_model` always returns this stack's one feature model — TypeScript+Next.js has exactly one framework convention to model, so there's no conditional logic here.

`.sql` handling is this pack's own concern rather than the generic core's: mapping the extension and routing to the dedicated table extractor both happen here, not in any shared code that would need to know about every stack's file conventions.
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
The `StackPack` for Rust — extraction over `.rs` files, exercised on CodeOwl's own repo. A marker type implementing `StackPack`; every trait method delegates to `rust.rs`'s own extraction and resolution functions.
### Behavior
`name` returns `"rust"`. `source_kind` recognizes only `.rs` files, as `SourceKind::Code` — everything else is `None`. `classify` marks a path under `tests/`, `benches/`, or containing a `/tests/` segment as `FileRole::Test`; everything else is `FileRole::Domain`, since Rust has no equivalent of a `components/ui`-style shared-primitive tier, and the build output directory is gitignored so a repo walk never even reaches it.

`extract_symbols` and `extract_imports` delegate straight to `rust::extract_file` and `rust::extract_imports`. `resolve_imports` delegates to `rust::resolve_imports`, passing the repo root, collected imports, and graph through unchanged — `use` imports resolve against Rust's own module tree by filesystem convention, not by any generic resolver.

`extract_flow_edges` always returns an empty list, and `resolve_flow_edge` always returns `FlowTarget::Unresolved` — this stack has no flow-edge conventions of its own to extract or resolve at all. `feature_model` isn't overridden here, so it takes the trait's default `None`: a Rust project like this one has cross-cutting workflows but no mechanically enumerable runtime entry surface the way a web framework's routes are.
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
The `StackPack` for Java — extraction over `.java` files, with a feature layer for JAX-RS-annotated Quarkus resources. A marker type implementing `StackPack`.
### Behavior
`name` returns `"java"`. `source_kind` recognizes only `.java` files. `classify` marks anything under `src/test/` or `/test/java/` as `FileRole::Test`, anything under `target/generated-sources/` or `build/generated/` as `FileRole::Generated`, and everything else as `FileRole::Domain` — Java has no equivalent of a shared UI-primitive tier. `extract_symbols`/`extract_imports`/`resolve_imports` delegate to `java.rs`'s own functions: `import`s resolve by the `src/main/java` package layout convention rather than by parsing `pom.xml`, so a Gradle-built repo resolves the same way a Maven-built one does; a same-package reference with no `import` statement at all is still picked up via a plain source scan.

`extract_flow_edges` always returns empty and `resolve_flow_edge` always returns unresolved — a Java call graph is out of scope the same way Rust's is.

`feature_model` always returns the Quarkus feature model unconditionally, regardless of whether the repo actually uses Quarkus — there's no separate "this is a plain library" branch. A repo with no JAX-RS-annotated resources (a utility library, say) just has that model recognize zero entry points, the same non-fire behavior any stack's feature model has on a repo that doesn't match its framework conventions.

`is_schema_symbol` recognizes a database table as a `class`-kind symbol that either carries a JPA entity annotation or extends the Panache active-record base class (which already implies the entity mapping whether or not it's separately annotated). It deliberately does **not** treat a class implementing a Panache repository interface as a table — a repository is the data-access layer sitting *around* a table, not the table itself, and it has no columns of its own to report; tagging it as a schema symbol would incorrectly exclude its file from ever joining a feature's core file set, where an injected repository actually belongs.

`generated_source_dirs` names Maven's `target/generated-sources` and Gradle's `build/generated` as the build-output directories this pack knows to check for source generated from a declarative contract (an OpenAPI spec, a `.proto` file).
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

## `is_entity_annotation`
`fn is_entity_annotation(marker: &str) -> bool`
### Summary
Checks whether one of a class's markers is a JPA `@Entity` annotation.
### Behavior
Strips the marker down to its bare annotation name (stripping any package qualification, so a fully-qualified `@jakarta.persistence.Entity` is recognized the same as a plain `@Entity`) and compares it exactly against `"Entity"` — an exact match, not a prefix check, so a similarly-named but different annotation doesn't accidentally match. Recognizes both the bare `@Entity` and the parameterized `@Entity(name = "…")` form, since both share the same annotation name.
### Depends on
- (none)

## `extends_panache_entity`
`fn extends_panache_entity(signature: &str) -> bool`
### Summary
Checks whether a class's signature `extends` one of Panache's two active-record base classes (`PanacheEntity` or `PanacheEntityBase`).
### Behavior
Strips out every balanced `<...>` span in the signature first — a generic type parameter's bound (`class Foo<T extends PanacheEntityBase>`) always sits inside one of these, before the real `extends` clause of the class itself even starts. Without stripping these first, a plain substring search over the whole signature would wrongly match a class whose only relation to `PanacheEntityBase` is an unrelated generic bound, not its own actual superclass.

After stripping, it scans the remaining words for the first `extends` keyword and checks whether the very next word starts with `"PanacheEntity"` — a prefix check deliberately covering both `PanacheEntity` and `PanacheEntityBase` in one comparison. If no `extends` keyword remains at all, it returns `false`.
### Depends on
- (none)

## `strip_angle_bracket_groups`
`pub(crate) fn strip_angle_bracket_groups(s: &str) -> String`
### Summary
Removes every balanced `<...>` span from a string — Java's generic type parameters and arguments — so text like `"Foo<T extends Bar>"` becomes `"Foo"`.
### Behavior
Walks the string character by character, tracking bracket depth: a `<` increases depth, a matching `>` decreases it, and any character seen while depth is above zero is dropped rather than copied to the output. This isn't a general parser — it assumes `<`/`>` only ever mean generics in whatever text it's given, which holds for a class's own signature (the text before its body) but wouldn't hold inside a method body, where `<`/`>` can be ordinary comparison operators instead.

Used both by `extends_panache_entity`'s `extends`-clause parsing and by the equivalent `implements`-clause parsing elsewhere, since both need to strip the same generic-bound noise before looking for the real superclass or interface name.
### Depends on
- (none)

## `PythonStack`
`pub struct PythonStack`
### Summary
The `StackPack` for Python — extraction over `.py` files, with a FastAPI feature layer and SQLModel/SQLAlchemy schema recognition. A marker type implementing `StackPack`.
### Behavior
`name` returns `"python"`. `source_kind` recognizes only `.py` files. `classify` marks a path as `FileRole::Test` if it sits under a `tests/` directory, or its filename is `conftest.py`, starts with `test_`, or ends with `_test.py`. A path under an Alembic or Django-style `.../migrations/versions/` directory is `FileRole::Generated` — a migration file describes a schema *change*, not current state, and typically has no imports of its own, so it's sunk the same way generated code is. Everything else is `FileRole::Domain`.

`extract_symbols`/`extract_imports`/`resolve_imports`/`extract_flow_edges`/`resolve_flow_edge` all delegate straight to `python.rs`'s own functions.

`feature_model` always returns the FastAPI feature model — routes as entry points, with `Depends()`/parameter-type flow edges resolved through it.

`is_schema_symbol` recognizes a database table as a `class`-kind symbol whose signature `python_class_is_table` identifies as a SQLModel `table=True` model or a SQLAlchemy `Base` subclass. This check used to also require the class to have zero methods of its own ("a table with methods is more class than table"), but that guard was dropped: once fields became real extracted members, a genuine table model with any real field would fail that check just as easily as one with a real method, since neither `is_schema_symbol` nor its caller can tell a field-child from a method-child apart from `sym` alone. Dropping it also brings Python in line with `java.rs`'s own `is_schema_symbol`, which never had this restriction — an `@Entity` class with real methods is tagged `Schema` there without issue.
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

## `python_class_is_table`
`fn python_class_is_table(sig: &str) -> bool`
### Summary
Decides whether a Python class declaration is a database table (an ORM — object-relational mapper — model, i.e. Python code that represents a table), by inspecting the class's signature line: its name and parenthesized base classes/keyword arguments (e.g. `class Item(SQLModel, table=True):`).
### Behavior
Finds the parenthesized part of the signature and splits it on commas into individual base-class names or keyword arguments. Returns `true` if any of them, once whitespace is stripped, is exactly `table=True` (SQLModel's convention for marking a model as a real table) or is the bare base class `Base` or `DeclarativeBase` (SQLAlchemy's declarative-base conventions). Deliberately returns `false` for a bare `SQLModel` or `BaseModel` base with no `table=True` — those represent request/response validation schemas, not database tables. The comma split is naive: a base class written with a nested comma, such as a generic type like `Generic[T, U]`, could misparse, but that shape is considered unlikely for a real ORM model class.</content>
### Depends on
- (none)
