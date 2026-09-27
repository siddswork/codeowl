---
kind: file
source_paths: [src/quarkus.rs]
file: { source_hash: , deps_hash: , spec_hash:  }
symbols:
  src/quarkus.rs::QuarkusFeatureModel: { source_hash: 02ea78577f5db8f51611046fdcd41c7bdeab14ef10fd10a159ec5c970e8e1b62, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 584899ada984a22be8f9a53b48f07d711e8f079d2fba00a50a585b302c1560ba }
  src/quarkus.rs::impl FeatureModel for QuarkusFeatureModel: { source_hash: 543270a07b5fc791bd1ec71a033b088922e63c3d2c5b6905e68f535fb3232b98, deps_hash: 403b28106519ed1e2fc405c1f482020b3b878e261b21a0760f663c2a5074170b, spec_hash: 174370b5a5f59e17de6ac241a8087dba7e6e851e67c0c7888ce4adc8099e486d }
  src/quarkus.rs::is_cdi_managed: { source_hash: 1a4fa3a5622317b5c477975617a04e8974f61c38b5f2f348dfbff0bda8fcae3d, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 9b1755d180cede89f9f35cc8aeeb63c805264d7c56768f551f99482c114e05ef }
  src/quarkus.rs::is_admitting_annotation: { source_hash: bf97a2404b7a67972ce68244dd30d0bbf7ffe0337d4f8a368a6b66e0614c397c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b6238ef6a70a40d53bb3953468c34b4ceed20e85b170f8c40c4fb493ca8ebb2e }
  src/quarkus.rs::parse_verb: { source_hash: 6db1a58f1f5ef49e0b9bf0964918cc5c675c0e1bec5bddffea5ce6e5c534e3dd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: f1b588be89733da9bee9b6895aae3360ed6618b183fd707ed915e4c4460ca356 }
  src/quarkus.rs::parse_path_annotation: { source_hash: 58fa3456ebc174a9fddb2c9e2c113197831d8ca2a7e76a992c8f5a4fb1243c19, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3d7ad188c7d87ecd5aab5cf6b3a6414ac1b166c8ee27645361ed974c8edc3dbd }
---
# src/quarkus.rs
## Summary


## `QuarkusFeatureModel`
`pub struct QuarkusFeatureModel`
### Summary
A marker type that lets the rest of the program treat Quarkus (a Java framework for building HTTP services) as one of the stacks it knows how to find feature entry points in — routes, scheduled jobs, and the like.
### Behavior
`QuarkusFeatureModel` carries no fields and no data of its own; it exists only to have the `FeatureModel` trait (the shared interface every supported stack implements to describe "what counts as a feature entry point here") implemented on it elsewhere in this file. Its methods do the real work of recognizing Quarkus-specific entry points (JAX-RS resource methods, `@Scheduled` jobs, and similar); this declaration itself is just the zero-sized handle those implementations hang off of.
### Depends on
- (none)

## `impl FeatureModel for QuarkusFeatureModel`
`impl FeatureModel for QuarkusFeatureModel`
### Summary
Where Quarkus's specific rules for "what counts as a feature" live: which symbols in the graph (the extracted map of a repo's declarations and how they reference each other) are entry points, and which other files belong alongside one when building that feature's spec.
### Behavior
Implements the two methods every stack's `FeatureModel` must provide:

- `enumerate_entry_points(&self, graph: &Graph) -> Vec<EntryPoint>` scans the graph and returns every symbol this stack considers a feature entry point (e.g. a JAX-RS resource method, a `@Scheduled` job) as an `EntryPoint` value.
- `admits_to_core(&self, graph: &Graph, _entry: &EntryPoint, candidate_file: &str) -> bool` decides, for a given entry point, whether a specific `candidate_file` should be pulled into that feature's `core_sources` (the set of files a feature spec is written from) — the `_entry` parameter name signals this decision doesn't actually depend on which entry point is asking, only on the candidate file itself.

Both methods take `&self` but no state on `QuarkusFeatureModel` itself — all the real logic is keyed off the shapes recorded in the graph (via `SymbolId`/`SymbolKind`) and the file's role (via `FileRole`, from `lang.rs`), not any per-instance data.
### Depends on
- `src/features.rs::EntryPoint` — crate::features
- `src/features.rs::FeatureModel` — crate::features
- `src/graph.rs::Graph` — crate::graph
- `src/lang.rs::FileRole` — crate::lang
- `src/symbol.rs::SymbolId` — crate::symbol
- `src/symbol.rs::SymbolKind` — crate::symbol
- externals: std

## `is_cdi_managed`
`fn is_cdi_managed(sym: &crate::symbol::Symbol) -> bool`
### Summary
Recognizes the kinds of classes Quarkus's dependency-injection system (CDI, "Contexts and Dependency Injection" — Java's standard for wiring components together without manual constructor calls) actually manages, so those classes can be pulled into a feature alongside the code that uses them.
### Behavior
Returns `true` if either of two things is true about the symbol:
- one of its markers (its annotations) is recognized by `is_admitting_annotation` as a scope annotation such as `@ApplicationScoped` or `@Singleton` — the annotations Quarkus uses to mark a class as a managed bean; or
- its signature text contains `"PanacheRepository"`, meaning it implements Quarkus's `PanacheRepository<...>` interface (a repository class for querying a database entity).

A plain data entity (a `@Entity`-annotated class with no service logic) deliberately does *not* count here — the module-level comment this function lives under explains why entities are excluded from this particular check. Note also that an injected framework class the repo doesn't define itself (like `Config` or `ObjectMapper`) never reaches this function at all: those never resolve to a `Symbol` in this repo's graph in the first place, since there's no declaration here to point at.
### Depends on
- (none)

## `is_admitting_annotation`
`fn is_admitting_annotation(marker: &str) -> bool`
### Summary
Checks whether one Java annotation is a Quarkus CDI (dependency-injection) scope that marks a class as a managed bean.
### Behavior
Strips the annotation down to its bare name (via `crate::java::bare_annotation_name`, which drops any `@`, package qualification, and arguments) and returns `true` only if that name is exactly `"ApplicationScoped"` or `"Singleton"` — the two scope annotations `is_cdi_managed` treats as evidence a class is injected and managed by the framework. Any other annotation (including other CDI scopes not covered here) returns `false`.
### Depends on
- (none)

## `parse_verb`
`fn parse_verb(marker: &str) -> Option<String>`
### Summary
Turns a JAX-RS HTTP annotation like `@GET` or `@POST` into the plain lowercase verb name (`"get"`, `"post"`) used to describe a route, or `None` if the annotation isn't one of the recognized verbs.
### Behavior
Strips the annotation to its bare name (bare or fully package-qualified, via `crate::java::bare_annotation_name`) and checks it for an *exact* match against the list of known HTTP verb annotations (`HTTP_VERBS`) — not a prefix or substring check, so a hypothetical custom annotation like `@GetSomething` is never mistaken for the real `@GET`. On a match, returns the verb lowercased (`Some("get")`); otherwise returns `None`.
### Depends on
- (none)

## `parse_path_annotation`
`fn parse_path_annotation(marker: &str) -> Option<String>`
### Summary
Pulls the URL path out of a JAX-RS `@Path` annotation — e.g. `@Path("entity/fruits")` becomes `"entity/fruits"` — so a route's full path can be assembled.
### Behavior
Checks the annotation's bare name (bare or fully package-qualified, e.g. `@jakarta.ws.rs.Path(...)`, via `crate::java::bare_annotation_name`); if it isn't exactly `"Path"`, returns `None` immediately. Otherwise, extracts the first string literal found inside the annotation's text (`first_string_literal`) and returns it. If a `@Path` annotation somehow has no string literal — which shouldn't happen, since JAX-RS requires one — this simply returns `None` for that method-level path rather than panicking.
### Depends on
- (none)
