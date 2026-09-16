//! The Quarkus feature model (M18, commit 1) — the second Java
//! `FeatureModel` implementation and the first non-web one; also the
//! first with more than one entry-point *kind* on the roadmap (HTTP now,
//! `@Incoming`/`@Scheduled`/`@GrpcService` in later commits — see
//! `ROADMAP.md`'s M18 section). This commit is HTTP resources only:
//! JAX-RS `@Path` (class-level prefix, optional per-method sub-path) +
//! a verb annotation (`@GET`/`@POST`/…).
//!
//! Returned unconditionally by `JavaStack::feature_model()`, same as
//! `PythonStack` → `FastApiFeatureModel` — a plain library with no
//! `@Path`-annotated classes just enumerates zero entry points (M17/M18's
//! `commons-lang` case, `ARCHITECTURE.md` open question 10).
//!
//! **Structurally different from FastAPI's routes**, which matters for
//! how this is parsed: FastAPI's path and verb are one decorator call
//! (`@router.get("/x")`), so `fastapi.rs` extracts both from a single
//! marker string. JAX-RS's `@Path` and `@GET`/`@POST`/… are two
//! *independent* annotations that can appear in either order and with
//! other annotations (`@Produces`, `@Transactional`, …) interleaved — so
//! this file matches path and verb from a method's `markers` list
//! separately, and reads the class-level `@Path` off the containing
//! `Container` symbol rather than scanning the file for a router
//! assignment.
//!
//! **`admits_to_core` is CDI-shaped, not co-location-shaped** — a Java
//! service's `core` grows through what's *managed* (`@ApplicationScoped`
//! / `@Singleton`) or is a Panache repository, not what's nearby on
//! disk. `@Inject` itself isn't parsed: the ordinary resolved reference
//! to the injected type is what `assemble_participants`'s walk already
//! sees (a field of that type, a constructor param, a bare static call),
//! so this only needs to judge the *referenced class*, the same
//! "type-reference classification, not dataflow" ROADMAP.md describes.
//! An `@Entity`/Panache-entity class deliberately does **not** admit
//! here — `stack.rs::JavaStack::is_schema_symbol` retags it `Schema`
//! instead, and a schema-bearing file is excluded from `core`-promotion
//! entirely by the generic core's `schema_files` guard
//! (`ARCHITECTURE.md`'s "Feature specs" section) before this hook is
//! even called.
//!
//! **`@RegisterRestClient` interfaces are excluded from
//! `enumerate_entry_points`, not modeled as cross-service flow edges.**
//! `HeroRestClient` in `quarkus-super-heroes` carries the identical
//! `@Path`/`@GET` shape as a real server resource (confirmed:
//! `extract_file` can't tell them apart from a method's own markers
//! alone) but describes an *outbound* call to another service, not
//! something this one serves — `is_register_rest_client` is the
//! discriminator. Deliberately **not** attempting to trace the call
//! itself into a flow edge: the real chain is
//! `FightResource → FightService → HeroClient → @RestClient HeroRestClient`,
//! three hops from any entry point to the interface that actually
//! carries the path, with no distinguishing syntax at the call site the
//! way `fetch("literal")` / `.from("literal")` / `Depends(x)` have —
//! every other pack's flow edges are single-hop and syntax-triggered.
//! Building multi-hop tracing here would be real call-graph analysis,
//! which this project has repeatedly declined to build (M16/M18 docs).
//! The ordinary resolved-import chain plus the client interface's own
//! Javadoc (which already states the target path in prose in the real
//! repo) already carry this information to a generating agent with no
//! new mechanism; whether that's actually sufficient is a question for
//! this milestone's dogfood validation, not something to guess at now.
//!
//! **Not yet (later M18 commits):** Kafka/`@Scheduled`/gRPC entry
//! points — same incremental pattern M17 used.

use std::collections::HashMap;

use crate::features::{EntryPoint, FeatureModel};
use crate::graph::Graph;
use crate::symbol::{SymbolId, SymbolKind};

const HTTP_VERBS: &[&str] = &["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];

#[derive(Debug, Default, Clone, Copy)]
pub struct QuarkusFeatureModel;

impl FeatureModel for QuarkusFeatureModel {
    fn enumerate_entry_points(&self, graph: &Graph) -> Vec<EntryPoint> {
        // The containing class's own `@Path` is the same answer for
        // every route method it declares, so it's looked up once per
        // class and cached for this one enumeration call — the same
        // memoization `fastapi.rs::router_prefix` uses for
        // `APIRouter(prefix=...)` (a code-review finding there).
        let mut class_path_cache: HashMap<SymbolId, Option<String>> = HashMap::new();
        // A `@RegisterRestClient` interface's methods carry the identical
        // `@Path`/`@GET` shape as a real server resource (confirmed:
        // `extract_file` can't tell them apart from a method's own
        // markers alone) but describe an *outbound* call this service
        // makes to another one's endpoint, not something it serves --
        // `HeroRestClient` in `quarkus-super-heroes` is the real case.
        // Same per-class memoization as the path cache above.
        let mut is_rest_client_cache: HashMap<SymbolId, bool> = HashMap::new();
        let mut out: Vec<EntryPoint> = graph
            .symbols()
            .filter(|s| s.kind == SymbolKind::Callable)
            .filter_map(|s| {
                let verb = s.markers.iter().find_map(|m| parse_verb(m))?;
                let method_path = s.markers.iter().find_map(|m| parse_path_annotation(m));
                let parent_id = s.parent?;
                let is_rest_client = *is_rest_client_cache
                    .entry(parent_id)
                    .or_insert_with(|| is_register_rest_client(graph, parent_id));
                if is_rest_client {
                    return None;
                }
                let class_path = class_path_cache
                    .entry(parent_id)
                    .or_insert_with(|| class_path_for(graph, parent_id))
                    .clone();
                let full = join_jaxrs_path(class_path.as_deref(), method_path.as_deref());
                Some(EntryPoint {
                    kind: "http".to_string(),
                    id: route_slug(&verb, &full),
                    title: format!("{} {full}", verb.to_uppercase()),
                    file: s.file.clone(),
                })
            })
            .collect();
        out.sort_by(|a, b| (a.id.as_str(), a.file.as_str()).cmp(&(b.id.as_str(), b.file.as_str())));
        disambiguate_colliding_slugs(&mut out);
        out
    }

    fn admits_to_core(&self, graph: &Graph, _entry: &EntryPoint, candidate_file: &str) -> bool {
        graph.symbols().any(|s| {
            s.file == candidate_file && s.kind == SymbolKind::Container && is_cdi_managed(s)
        })
    }
}

/// `sym` is a CDI-managed bean (`@ApplicationScoped` / `@Singleton`) or a
/// Panache repository (`implements PanacheRepository<...>`) -- see the
/// module doc comment for why an entity is excluded here. An injected
/// framework primitive (`Config`, `ObjectMapper`, …) never reaches this
/// check at all: those are external classes with no resolved reference
/// edge into this repo's graph in the first place.
fn is_cdi_managed(sym: &crate::symbol::Symbol) -> bool {
    sym.markers.iter().any(|m| is_admitting_annotation(m))
        || sym.signature.contains("PanacheRepository")
}

fn is_admitting_annotation(marker: &str) -> bool {
    let name = marker.trim_start().trim_start_matches('@');
    let name = name.split(['(', ' ']).next().unwrap_or(name);
    matches!(name, "ApplicationScoped" | "Singleton")
}

/// `@GET` / `@POST` / … -> its lowercase verb name. Exact match only (not
/// a prefix check) so a hypothetical `@GetSomething`-style custom
/// annotation is never mistaken for a real JAX-RS verb -- see
/// `ARCHITECTURE.md` open question 11 on annotation vocabularies.
fn parse_verb(marker: &str) -> Option<String> {
    let name = marker.trim_start().trim_start_matches('@');
    HTTP_VERBS
        .iter()
        .find(|&&v| name == v)
        .map(|v| v.to_ascii_lowercase())
}

/// `@Path("entity/fruits")` -> `"entity/fruits"`. `None` for any other
/// annotation, or a `@Path` with no string literal (shouldn't happen —
/// JAX-RS requires an argument — but a malformed annotation just yields
/// no method-level path rather than panicking).
fn parse_path_annotation(marker: &str) -> Option<String> {
    let m = marker.trim_start();
    let rest = m.strip_prefix("@Path")?.trim_start();
    if !rest.starts_with('(') {
        return None;
    }
    first_string_literal(rest)
}

/// The first `"…"` literal in `s`. Same trick as `fastapi.rs`'s helper of
/// the same name -- kept as its own copy rather than shared, matching
/// this project's "each pack owns its own parsing" stance (design
/// decision 5).
fn first_string_literal(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let start = b.iter().position(|&c| c == b'"')?;
    let rel_end = b[start + 1..].iter().position(|&c| c == b'"')?;
    Some(s[start + 1..start + 1 + rel_end].to_string())
}

/// `class_id`'s own `@Path`, if it has one. A resolved `Symbol::parent` is
/// already a `SymbolId` (unlike `ExtractedSymbol::parent`, which is a
/// pre-arena string — see `symbol.rs`), so this is a direct arena lookup,
/// no `graph.find` round-trip through a string id needed.
fn class_path_for(graph: &Graph, class_id: SymbolId) -> Option<String> {
    let sym = graph.get_symbol(class_id)?;
    sym.markers.iter().find_map(|m| parse_path_annotation(m))
}

/// Does `class_id`'s own declaration carry `@RegisterRestClient`? The
/// discriminator between a real server resource and an outbound client
/// contract that happens to share the identical `@Path`/verb shape.
fn is_register_rest_client(graph: &Graph, class_id: SymbolId) -> bool {
    let Some(sym) = graph.get_symbol(class_id) else {
        return false;
    };
    sym.markers.iter().any(|m| {
        let name = m.trim_start().trim_start_matches('@');
        let name = name.split(['(', ' ']).next().unwrap_or(name);
        name == "RegisterRestClient"
    })
}

/// Join a class-level `@Path` and an optional method-level `@Path` the
/// way JAX-RS does: plain concatenation, tolerant of either side missing
/// its slashes entirely (`@Path("entity/fruits")` is as valid as
/// `@Path("/hello")`) — unlike FastAPI's `join_route`, both inputs are
/// normalized the same way since neither is more "the base" than the
/// other syntactically.
fn join_jaxrs_path(class_path: Option<&str>, method_path: Option<&str>) -> String {
    let class_seg = class_path.unwrap_or("").trim_matches('/');
    let method_seg = method_path.unwrap_or("").trim_matches('/');
    match (class_seg.is_empty(), method_seg.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{method_seg}"),
        (false, true) => format!("/{class_seg}"),
        (false, false) => format!("/{class_seg}/{method_seg}"),
    }
}

/// `("get", "/entity/fruits/{id}")` -> `"http-get-entity-fruits-id"`.
/// Kind-prefixed from the start (`ROADMAP.md` M18 flags `http-fights` vs
/// `kafka-fights` as a real future collision) even though this commit
/// only ever produces `"http"` — cheaper to bake in now than retrofit
/// once a second kind exists.
fn route_slug(verb: &str, path: &str) -> String {
    let path_part = path
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|seg| !seg.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-");
    if path_part.is_empty() {
        format!("http-{verb}-root")
    } else {
        format!("http-{verb}-{path_part}")
    }
}

/// `entries` sorted by `(id, file)`: give every id beyond the first in a
/// run a stable `-2`, `-3`, … suffix instead of silently collapsing a
/// collision — same fix, same reasoning as `fastapi.rs`'s function of the
/// same name (a `dedup_by` on `id` alone would drop every route after the
/// first whenever two different files' routes collided).
fn disambiguate_colliding_slugs(entries: &mut [EntryPoint]) {
    let mut i = 0;
    while i < entries.len() {
        let mut n = 2;
        let mut j = i + 1;
        while j < entries.len() && entries[j].id == entries[i].id {
            entries[j].id = format!("{}-{n}", entries[i].id);
            n += 1;
            j += 1;
        }
        i = j;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_verbs_exactly_not_as_a_prefix() {
        assert_eq!(parse_verb("@GET"), Some("get".to_string()));
        assert_eq!(parse_verb("@POST"), Some("post".to_string()));
        assert_eq!(parse_verb("@DELETE"), Some("delete".to_string()));
        assert_eq!(parse_verb("@Path(\"/x\")"), None);
        assert_eq!(
            parse_verb("@GetMapping"),
            None,
            "not JAX-RS -- exact match only"
        );
        assert_eq!(parse_verb("@Override"), None);
    }

    #[test]
    fn parses_path_literals() {
        assert_eq!(
            parse_path_annotation("@Path(\"/hello\")"),
            Some("/hello".to_string())
        );
        assert_eq!(
            parse_path_annotation("@Path(\"entity/fruits\")"),
            Some("entity/fruits".to_string())
        );
        assert_eq!(parse_path_annotation("@GET"), None);
        assert_eq!(
            parse_path_annotation("@Produces(MediaType.TEXT_PLAIN)"),
            None
        );
    }

    #[test]
    fn joins_class_and_method_paths_regardless_of_slashes() {
        assert_eq!(
            join_jaxrs_path(Some("/hello"), Some("/greeting/{name}")),
            "/hello/greeting/{name}"
        );
        assert_eq!(
            join_jaxrs_path(Some("entity/fruits"), Some("{id}")),
            "/entity/fruits/{id}"
        );
        assert_eq!(join_jaxrs_path(Some("/hello"), None), "/hello");
        assert_eq!(join_jaxrs_path(Some("/"), None), "/");
        assert_eq!(join_jaxrs_path(Some("/env.js"), None), "/env.js");
        assert_eq!(join_jaxrs_path(None, None), "/");
    }

    #[test]
    fn slugs_are_kind_prefixed_and_lowercased() {
        assert_eq!(
            route_slug("get", "/entity/fruits/{id}"),
            "http-get-entity-fruits-id"
        );
        assert_eq!(route_slug("post", "/"), "http-post-root");
    }

    #[test]
    fn admitting_annotations_are_matched_exactly() {
        assert!(is_admitting_annotation("@ApplicationScoped"));
        assert!(is_admitting_annotation("@Singleton"));
        // `@Entity` is Schema territory (`stack.rs::is_entity_annotation`),
        // not a core-admitting annotation -- see the module doc comment.
        assert!(!is_admitting_annotation("@Entity"));
        assert!(!is_admitting_annotation("@RequestScoped"));
        assert!(!is_admitting_annotation("@Path(\"/x\")"));
        assert!(!is_admitting_annotation("@Override"));
    }
}
