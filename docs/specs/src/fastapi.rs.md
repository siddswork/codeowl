---
kind: file
source_paths: [src/fastapi.rs]
file: { source_hash: d90d9b829608442c1e9c1d0228a8015e905c55ff2f5b651dddb005653d7ed289, deps_hash: b797102975ee03feaa59366adcc2d0299e976be435d6bdf46aaaee1ff29380ba, spec_hash: 711c8fae0a9eabe23d35096851b12917ef4d55df32870c80f6aae80da4a11759 }
symbols:
  src/fastapi.rs::FastApiFeatureModel: { source_hash: 7e2aaa5e9dd972ebbcfa09f181cf2bf1aa2c0265cf086cceaa2378e3783ff338, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 582f29f3bbd26f61f5219aa5d0f08bde5a9d044b4b644e05853c9f7366468dd6 }
  src/fastapi.rs::impl FeatureModel for FastApiFeatureModel: { source_hash: 6254a7fcbd8ece0de15626fdd5a32358b3539496e7a2c25400e3b40cd8a2e3e4, deps_hash: b797102975ee03feaa59366adcc2d0299e976be435d6bdf46aaaee1ff29380ba, spec_hash: a32c3c1b14c427a0c644b162bc2c2887589c3d7f47e5c801997051a483e0f659 }
  src/fastapi.rs::parse_route_decorator: { source_hash: c03313a7e32fca7abb97b8404fb7bbd8998f9a25dc1fd8849b39fd3eb9e49a91, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: dd1ae72cddd2edae365701bd3c87f914c7d86f1abf6ca0c08ef052e8c00f0971 }
  src/fastapi.rs::first_string_literal: { source_hash: 8316a508e0fce80ad33966c5f1e9c043a6b50579ffefc6c7d2565c7e6a025f60, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 6f1079426852ad2542c09da0aac452cc17ce0f7da175513c41b102c62cfc2c08 }
  src/fastapi.rs::router_prefix: { source_hash: 0131a7541de47d0534dc41d7b77a3638c2324dc73041498cc79bc1b96563c484, deps_hash: c7b3c780b3d5f3a7c9ea5d71f7ffa53a1413d36c92a0a62889c9e0d64c51cb94, spec_hash: b7a6ddc045602d3d5b51d31942b18b19855985b93fa5ec2cf5e804a773a4a532 }
  src/fastapi.rs::kwarg_string_literal: { source_hash: 2c51c23cb52a4f256dbc75171e6a9c50c977a0b28b474a773f0b2eed69804b02, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 8b3593407b4c2b3b2b78b2f3618ba5fef4ba2365f17eb222ac30bfa274568f27 }
  src/fastapi.rs::is_ident_byte: { source_hash: ad2053043283aaf6db6169ca16e4a244fbd9d7e421fd8aedc21f4efa1bee4fb2, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b1b30c332e2c31f32baea2b97d26dc70e9b703cebf743a6c35073a1e0b629bcc }
  src/fastapi.rs::join_route: { source_hash: 05e9c0981198a8d0a9658d5ce41118aabf6e74bba2784794513263d66e027270, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: cf57071828897c7aa4dbfa701a192f98166e760dd2eb7a1ed9860302d0d61ff8 }
  src/fastapi.rs::route_slug: { source_hash: 01919af3cc42ea93e2c7dc73bcc319cfb078b5d904f142fb7d151349adc51350, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 06f42a05b94a8dd8980c611a2a81604ed49134329920f2734789a9f154ed4711 }
  src/fastapi.rs::disambiguate_colliding_slugs: { source_hash: 6ca57c0e07fc6cb6f77d6250d901b84c7234d0a98293a540cf13bb5e95c15ab8, deps_hash: 7480c0c7143c5d04d29dad250239438a770b3ba4bdeff05c808d265e72d64835, spec_hash: f95fcd0b0cf6db6925456d97d3553d33e07d16aeb5ed87b0a2026961b3d3a530 }
  src/fastapi.rs::is_crud_module: { source_hash: cf448b80ac00375ba3d4c48a78dd429800ebe0fed130bce4e4866474d5bc8c10, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b3b3ffc166b257f6093d4d27e565d596c56e9bd583cd080509b90a5e55dcffd8 }
  src/fastapi.rs::file_touches_schema: { source_hash: 73390117722264f5f7194345c4c2ada05444ad7d7f3541e11eed5e9da9d10f11, deps_hash: c7b3c780b3d5f3a7c9ea5d71f7ffa53a1413d36c92a0a62889c9e0d64c51cb94, spec_hash: 4abcab6d151aa4bdcc71157974dc0cd23f3564a1f8631b7cfba82d16b12298c0 }
---
# src/fastapi.rs
## Summary
This file teaches CodeOwl to recognize a FastAPI web route as a "feature" — a capability worth its own spec document — the same role the Next.js feature model plays for pages and API routes, but for Python's FastAPI framework instead. A feature here is one HTTP route: a function decorated with `@router.get(...)`, `@router.post(...)`, and similar, found by scanning every function's decorators for a recognized FastAPI verb call and reading its path out of the decorator's first string argument. Because a route is usually attached to a router declared with a path prefix (`router = APIRouter(prefix="/items")`), this file also finds and applies that prefix so the reported route path is the real, full one. When two routes would otherwise end up with the same generated id — most commonly because the file doesn't yet account for an outer `app.include_router(..., prefix=...)` chain applied at the top level — a fallback step appends a disambiguating number so no route's spec silently disappears.

The other job this file does is deciding what other code belongs inside a given route's own feature versus staying just a one-hop dependency: a file counts as part of the route's "core" if it's a recognized data-access module by naming convention (`crud.py`, a `repositories/` directory, and similar) or if it declares or imports a database table — both signals that the file is doing real data work for the feature, not just incidental plumbing. Everything else — the generic walk that actually assembles a feature's participant set from these two judgments — lives in `features.rs`; this file supplies only the FastAPI-specific answers that walk can't derive on its own.</content>

## `FastApiFeatureModel`
`pub struct FastApiFeatureModel`
### Summary
The FastAPI implementation of CodeOwl's `FeatureModel` trait — the piece that lets the Python stack recognize a FastAPI route as a "feature" (a capability worth its own spec), the same role `TypeScriptNextFeatureModel` plays for Next.js pages and API routes.
### Behavior
A zero-sized marker struct with no fields — all its actual behavior lives in its `FeatureModel` trait implementation elsewhere in this file (enumerating routes as entry points, and deciding what belongs in a route's "core" code). It carries no state of its own because a feature model's decisions only ever depend on the graph and the specific entry point being asked about, both passed in as arguments rather than stored.</content>
### Depends on
- (none)

## `impl FeatureModel for FastApiFeatureModel`
`impl FeatureModel for FastApiFeatureModel`
### Summary
The two judgment calls the generic feature-walking logic (`assemble_participants` in `features.rs`) can't make on its own for a FastAPI codebase: what counts as an entry point (a route), and what other code belongs inside a route's own feature versus being just a one-hop dependency.
### Behavior
`enumerate_entry_points` scans every `Callable` symbol in the graph for one whose decorators (`markers`) match a recognized route decorator (`parse_route_decorator`, e.g. `@router.get(...)`), extracting the HTTP verb and the raw path. For each match, it looks up that route's file's `APIRouter(prefix=...)` value via `router_prefix` — memoized per file within this one enumeration call, since a file with several routes (the common shape — several `@router.get`/`@router.post` methods in one module) would otherwise re-scan the same file's symbols once per route for an identical answer. The prefix and the route's own path are joined (`join_route`) into the route's full path. A `"websocket"` verb gets `kind: "websocket"`; every other verb gets `kind: "http"`. Each entry point's `id` comes from `route_slug` (kind, verb, and full path), and its `title` is the verb (uppercased) plus the full path, e.g. `"GET /items/{id}"`. The whole list is sorted by `(id, file)` for deterministic output, then run through `disambiguate_colliding_slugs` to handle the rare case where two different routes would otherwise produce the same slug.

`admits_to_core` decides whether a file reached from a route belongs inside that route's own feature: it does if the file is a recognized CRUD module (`is_crud_module`) or if it touches a database schema symbol (`file_touches_schema`) — regardless of which specific entry point is asking, since both checks depend only on the candidate file itself, not on the particular route.</content>
### Depends on
- `src/features.rs::EntryPoint` — crate::features
- `src/features.rs::FeatureModel` — crate::features
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolKind` — crate::symbol
- externals: std

## `parse_route_decorator`
`fn parse_route_decorator(marker: &str) -> Option<(String, String)>`
### Summary
Checks whether a decorator's text is a FastAPI route decorator, and if so, pulls out its HTTP verb and path — e.g. `@router.get("/items/{id}", response_model=...)` becomes `("get", "/items/{id}")`.
### Behavior
Strips the leading `@` and looks for `.` followed by one of the known HTTP verbs (`ROUTE_VERBS`) followed by `(` anywhere in the decorator text. As soon as one is found, it reads the first string literal argument after that opening paren (`first_string_literal`) as the route's path, and returns the verb paired with that path. The receiver the decorator is called on — `router`, `app`, `api_router`, or any other name — is deliberately not checked, so any object with a method matching a known HTTP verb is treated as a route decorator; this keeps the matcher simple at the cost of theoretically matching a non-router object that happens to have a same-named method, considered an acceptable trade-off for FastAPI's very consistent convention. Returns `None` if no known verb is found, or if the matched call has no string-literal first argument to read a path from.</content>
### Depends on
- (none)

## `first_string_literal`
`fn first_string_literal(s: &str) -> Option<String>`
### Summary
Finds and extracts the first quoted string literal in a piece of text — used to pull a route's path out of the raw text right after a decorator's opening parenthesis.
### Behavior
Scans for the first `"` or `'` character, treats that as the opening quote, then finds the next occurrence of that same quote character to mark the end. Returns the text between the two quotes. This is a byte-level scan rather than a real parse, so it doesn't handle an escaped quote character inside the string (`\"` within a double-quoted literal) — considered acceptable given route path strings essentially never contain one.</content>
### Depends on
- (none)

## `router_prefix`
`fn router_prefix(graph: &Graph, file: &str) -> Option<String>`
### Summary
Finds a file's `APIRouter(prefix="...")` declaration and returns its prefix path — the segment FastAPI prepends to every route defined on that router, e.g. `router = APIRouter(prefix="/items", tags=[...])` gives `"/items"`.
### Behavior
Searches the graph for a `Value`-kind symbol in the given file whose signature contains the text `APIRouter(`, then extracts its `prefix` keyword argument as a string literal (via `kwarg_string_literal`). Returns `None` if the file has no such declaration, or if it does but the `prefix` argument isn't a plain string literal — for example `prefix=settings.API_PREFIX`, a value that can't be known just by reading the source text. In that case, the route ends up reported without any prefix rather than with a wrong one, which is the safer failure mode.</content>
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolKind` — crate::symbol

## `kwarg_string_literal`
`fn kwarg_string_literal(sig: &str, key: &str) -> Option<String>`
### Summary
Finds a specific keyword argument by name in a signature string and returns its value if it's a plain string literal — e.g. finding `prefix` in `APIRouter(prefix="/x", tags=[...])` returns `"/x"`; finding it where the value is a variable reference like `prefix=settings.X` returns `None`, since that's not a literal this can read.
### Behavior
Does a careful byte-by-byte scan rather than a naive substring search, because a naive `sig.find("key=")` could wrongly match the literal text `"key="` if it happened to appear inside an *earlier* argument's own string value (e.g. a `responses={404: {"description": "...prefix=legacy..."}}` argument before the real `prefix=` one) — this function's earlier version had exactly that bug. It tracks whether it's currently inside a quoted string (correctly skipping over escaped characters within one) and only tests for the key at a real token boundary — either the very start of the signature, or right after a character that isn't part of an identifier — so it can never falsely match text sitting inside someone else's string value. Once it finds `key=` at a genuine boundary, it checks whether what follows (after skipping whitespace) starts with a quote; if so, it delegates to `first_string_literal` to extract the value, and if not (the value is some other expression, not a literal), returns `None`. The scan also advances by whole UTF-8 characters rather than raw bytes when outside a string, since Python allows non-ASCII identifiers and blindly stepping one byte at a time could otherwise split a multi-byte character and cause a panic on the next slice operation.</content>
### Depends on
- (none)

## `is_ident_byte`
`fn is_ident_byte(c: u8) -> bool`
### Summary
Checks whether a byte could be part of a Python identifier (a variable, keyword argument, or similar name) — used to detect real token boundaries when scanning for a keyword argument.
### Behavior
Returns `true` for an ASCII letter, an ASCII digit, or an underscore.</content>
### Depends on
- (none)

## `join_route`
`fn join_route(prefix: &str, path: &str) -> String`
### Summary
Joins a router's prefix and a route's own path into one clean full path, handling the slash bookkeeping so the result never has doubled or missing slashes — e.g. prefix `"/items"` plus path `"/{id}"` becomes `"/items/{id}"`.
### Behavior
Trims a trailing slash off the prefix and a leading slash off the path, then combines them based on which are empty: both empty gives just `"/"`; an empty prefix gives the path with a leading slash added; an empty path gives just the prefix; and both non-empty are joined with a single `/` between them.</content>
### Depends on
- (none)

## `route_slug`
`fn route_slug(kind: &str, verb: &str, path: &str) -> String`
### Summary
Builds the filename-safe id used to name a route's spec file, from its kind, HTTP verb, and full path — e.g. `("http", "get", "/items/{id}")` becomes `"http-get-items-id"`.
### Behavior
Splits the path on every non-alphanumeric character (slashes, braces, hyphens, etc.), drops any resulting empty segments, lowercases what's left, and joins the pieces with hyphens. A path that reduces to nothing (the bare root path `/`) falls back to the literal segment `"root"` so the slug is never just `"http-get-"`. This produces a slug that's guaranteed unique across every route hosted by one router file, but two *different* routers can still coincidentally produce the same slug today — most likely because the outer `app.include_router(prefix=...)` chain isn't factored into the path yet — which is why `enumerate_entry_points` always runs `disambiguate_colliding_slugs` afterward to catch and fix that case.</content>
### Depends on
- (none)

## `disambiguate_colliding_slugs`
`fn disambiguate_colliding_slugs(entries: &mut [EntryPoint])`
### Summary
Fixes up a sorted list of entry points so that two different routes which happened to produce the same slug (two different router files exposing the same path, most likely because the outer `app.include_router(prefix=...)` isn't accounted for yet) get distinct, stable ids instead of silently colliding — a real bug this replaced, where an earlier version's plain dedup on id alone would drop every route after the first one whenever two files' routes collided, permanently hiding them from the generated spec corpus.
### Behavior
Requires `entries` to already be sorted by `(id, file)`, so that any routes sharing an id are adjacent. It walks the list once: for each run of consecutive entries sharing the same id, the *first* one in the run keeps its id unchanged — so a route with no collision at all never has its id disturbed, meaning existing generated specs for non-colliding routes don't churn on every run — while each subsequent entry in the run gets a `-2`, `-3`, and so on appended to the original id, assigned in a stable, deterministic order.</content>
### Depends on
- `src/features.rs::EntryPoint` — crate::features

## `is_crud_module`
`fn is_crud_module(file: &str) -> bool`
### Summary
Recognizes a file as a data-access module (a repository/CRUD layer) by naming convention, so it gets pulled into a route's feature `core` even though it isn't sitting next to the route file itself. This is the FastAPI expression of an intuition CodeOwl's M17 plan also carries over to Java: a module that does data work belongs in the feature's core, co-located or not.
### Behavior
Checks the file's bare filename against a fixed set of conventional names (`crud.py`, `repository.py`, `repositories.py`), or checks whether the file's path passes through a `/crud/` or `/repositories/` directory. Any match counts the file as a CRUD module.</content>
### Depends on
- (none)

## `file_touches_schema`
`fn file_touches_schema(graph: &Graph, file: &str) -> bool`
### Summary
Checks whether a file "does data work" — either declaring a database table itself, or importing one from elsewhere — the other half of `admits_to_core`'s check alongside `is_crud_module`.
### Behavior
Returns `true` if either: the file itself declares a symbol of kind `Schema` (a database table — already tagged that way earlier in extraction by `is_schema_symbol`, so this is a plain lookup, no re-analysis); or the file imports something that resolves to a `Schema`-kind symbol elsewhere. Both checks are simple graph lookups off already-computed symbol kinds and import edges — no call-graph analysis is involved, since CodeOwl doesn't trace actual query execution.</content>
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolKind` — crate::symbol
