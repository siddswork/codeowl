//! The Quarkus feature model (M18) — the second Java `FeatureModel`
//! implementation and the first non-web one; also the first with more
//! than one entry-point *kind* on the roadmap (see `ROADMAP.md`'s M18
//! section). Built incrementally, one kind per commit: HTTP resources
//! first (JAX-RS `@Path`, class-level prefix + optional per-method
//! sub-path, plus a verb annotation `@GET`/`@POST`/…), then Kafka
//! `@Incoming`/`@Outgoing` listeners (below).
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
//! **Kafka `@Incoming`/`@Outgoing` listeners (M18, commit 2).** Real-
//! repo census before building anything: `quarkus-super-heroes` (the
//! milestone's actual corpus) has 0 `@Scheduled` and 0 `@GrpcService`
//! but 4 real reactive-messaging usages; `quarkus-quickstarts` has all
//! three (12 `@Incoming`/`@Outgoing`, 3 `@Scheduled`, 2 `@GrpcService`),
//! confirming Kafka is the higher-priority kind to build first. Three
//! real shapes, all handled: a pure `@Incoming`-only consumer
//! (`kafka-panache-quickstart`'s `PriceStorage`), a pure `@Outgoing`-only
//! *declarative* producer with no injected emitter anywhere
//! (`PriceGenerator` — invoked by the reactive-messaging runtime's own
//! ticker, not application code, so it's a genuine entry point exactly
//! like `@Scheduled` will be), and a method carrying **both** annotations
//! at once (`quarkus-super-heroes`'s `SuperStats.processFight`, which
//! consumes one channel and republishes to another) — one entry point,
//! keyed on the incoming/triggering channel, not two.
//!
//! **Channel names aren't always inline literals.** `SuperStats` names
//! its channels via same-class `static final String` constants
//! (`@Incoming(FIGHTS_CHANNEL_NAME)`), not `@Incoming("fights")` —
//! `resolve_channel_name` resolves this the same "textual scan within
//! known scope" way `same_package_edges` resolves an implicit same-
//! package reference: no cross-file interpretation, just a same-class
//! constant lookup, falling back to the raw identifier (slugified) if
//! nothing matches so an entry point is never silently dropped.
//!
//! **A `@Channel`-annotated constructor/field parameter is never its own
//! entry point** (the real `FightService(@Channel("fights") MutinyEmitter
//! emitter, …)` shape — dependency injection of an emitter for a later
//! imperative `.send()` call from *inside* an already-modeled entry
//! point, not something the framework invokes on its own). This falls
//! out of extraction with no special-casing at all: `annotations()` only
//! captures a node's own modifiers, never a parameter's, so a
//! parameter-level `@Channel` is invisible to `markers` in the first
//! place — confirmed by a real-shaped regression test, not assumed.
//!
//! **`@Scheduled` jobs (M18, commit 3).** No path/channel-equivalent
//! identity exists for a scheduled method, so the entry-point id is keyed
//! on the method's own name (`scheduled-{name}`) rather than on the
//! schedule expression itself — two methods can share an identical
//! interval or cron string (real `quarkus-quickstarts` shape:
//! `scheduler-quickstart`'s `CounterBean` has three `@Scheduled` methods,
//! none of them path-shaped). The schedule detail (`cron=…` / `every=…` /
//! `fixedRate=…`) still enriches the *title*, tried in that priority
//! order via `scheduled_detail`. Bare annotation-name matching again
//! deliberately doesn't check imports: `quarkus-quickstarts`'s
//! `spring-scheduled-quickstart` demonstrates Quarkus's Spring-scheduling
//! compatibility layer with Spring's own `@Scheduled`
//! (`fixedRate`/`fixedRateString`, no quotes — a bare numeric literal,
//! unlike every Quarkus-native attribute) — still Quarkus underneath, and
//! treating both vocabularies uniformly is correct here, not a gap (see
//! `ARCHITECTURE.md` open question 11 on annotation vocabularies more
//! generally).
//!
//! **`@GrpcService` methods (M18, commit 4 — the last entry-point kind).**
//! Structurally the odd one out: the annotation lives only on the
//! *class* (`quarkus-quickstarts/grpc-plain-text-quickstart`'s
//! `HelloWorldService`), never on a method, because the actual RPC
//! method shapes come from a generated proto interface (`Greeter`, from
//! a `.proto` file — out of scope, no grammar registered for it here).
//! Every `public` method on an already-`@GrpcService` class is treated
//! as an entry point, deliberately without requiring the interface's own
//! `@Override` marker — this project has already declined to walk
//! interface hierarchies for inherited annotations
//! (`ARCHITECTURE.md` open question 11). A member has no visibility
//! field of its own (`ExtractedSymbol.is_exported` is file-level only),
//! so `is_public_signature` reads the `public` modifier straight out of
//! `signature`'s own text — skipping past any leading annotations first,
//! since `@Override` always precedes the modifiers there.

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
        // Same per-class memoization again: `@GrpcService` lives on the
        // class, not the method, so every public method on the same
        // class shares this one answer.
        let mut is_grpc_service_cache: HashMap<SymbolId, bool> = HashMap::new();
        // Per-class memoization for `resolve_channel_name`'s constant
        // lookups -- see its own doc comment.
        let mut channel_constants_cache: HashMap<SymbolId, Vec<&crate::symbol::Symbol>> =
            HashMap::new();
        // Deliberately first-match-wins across kinds (http, then kafka,
        // then scheduled, then grpc), not "detect every kind a method's
        // markers could match." Code-review finding: a method combining
        // e.g. `@Incoming` with `@Scheduled` would only ever report as
        // kafka. Considered and declined, for two reasons: (1) no
        // occurrence of any two entry-point-kind markers on one method
        // exists in either real test repo (`quarkus-quickstarts` or
        // `quarkus-super-heroes`) -- this project's "wait for real
        // evidence" policy applies; (2) `@Incoming` (message-triggered)
        // and `@Scheduled` (timer-triggered) are two different invocation
        // mechanisms for the same method, which is arguably semantically
        // incoherent, not just rare -- a method can't genuinely be
        // invoked both ways. Restructuring this into "detect all kinds
        // independently" would also introduce a real regression: the
        // grpc branch's condition (any `public` method on a
        // `@GrpcService` class) is far more permissive than the other
        // three, so removing this exclusivity would make it double-fire
        // on an http/kafka/scheduled method that happens to sit in a
        // `@GrpcService`-annotated class.
        let mut out: Vec<EntryPoint> = graph
            .symbols()
            .filter(|s| s.kind == SymbolKind::Callable)
            .filter_map(|s| {
                let parent_id = s.parent?;
                let is_rest_client = *is_rest_client_cache
                    .entry(parent_id)
                    .or_insert_with(|| is_register_rest_client(graph, parent_id));
                if is_rest_client {
                    return None;
                }
                if let Some(verb) = s.markers.iter().find_map(|m| parse_verb(m)) {
                    let method_path = s.markers.iter().find_map(|m| parse_path_annotation(m));
                    let class_path = class_path_cache
                        .entry(parent_id)
                        .or_insert_with(|| class_path_for(graph, parent_id))
                        .clone();
                    let full = join_jaxrs_path(class_path.as_deref(), method_path.as_deref());
                    return Some(EntryPoint {
                        kind: "http".to_string(),
                        id: route_slug(&verb, &full),
                        title: format!("{} {full}", verb.to_uppercase()),
                        file: s.file.clone(),
                    });
                }
                let incoming = s.markers.iter().find_map(|m| incoming_channel(m));
                let outgoing = s.markers.iter().find_map(|m| outgoing_channel(m));
                if incoming.is_some() || outgoing.is_some() {
                    let incoming_name = incoming.map(|a| {
                        resolve_channel_name(graph, parent_id, a, &mut channel_constants_cache)
                    });
                    let outgoing_name = outgoing.map(|a| {
                        resolve_channel_name(graph, parent_id, a, &mut channel_constants_cache)
                    });
                    let (primary, title) = match (&incoming_name, &outgoing_name) {
                        (Some(i), Some(o)) => (i.clone(), format!("Kafka: {i} -> {o}")),
                        (Some(i), None) => (i.clone(), format!("Kafka: {i}")),
                        (None, Some(o)) => (o.clone(), format!("Kafka: {o} (produced)")),
                        (None, None) => unreachable!("checked above"),
                    };
                    return Some(EntryPoint {
                        kind: "kafka".to_string(),
                        id: kafka_slug(&primary),
                        title,
                        file: s.file.clone(),
                    });
                }
                if let Some(sched) = s.markers.iter().find(|m| is_scheduled_annotation(m)) {
                    let method_name = s.id.rsplit("::").next().unwrap_or(s.id.as_str());
                    let title = match scheduled_detail(sched) {
                        Some(detail) => format!("Scheduled: {method_name} ({detail})"),
                        None => format!("Scheduled: {method_name}"),
                    };
                    return Some(EntryPoint {
                        kind: "scheduled".to_string(),
                        id: scheduled_slug(method_name),
                        title,
                        file: s.file.clone(),
                    });
                }
                if s.raw == "method" && is_public_signature(&s.signature) {
                    let is_grpc = *is_grpc_service_cache
                        .entry(parent_id)
                        .or_insert_with(|| is_grpc_service_class(graph, parent_id));
                    if is_grpc {
                        let method_name = s.id.rsplit("::").next().unwrap_or(s.id.as_str());
                        return Some(EntryPoint {
                            kind: "grpc".to_string(),
                            id: grpc_slug(method_name),
                            title: format!("gRPC: {method_name}"),
                            file: s.file.clone(),
                        });
                    }
                }
                None
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
    matches!(
        crate::java::bare_annotation_name(marker),
        "ApplicationScoped" | "Singleton"
    )
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
    sym.markers
        .iter()
        .any(|m| crate::java::bare_annotation_name(m) == "RegisterRestClient")
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

/// Split `s` on every non-alphanumeric-ASCII run and lowercase what's
/// left — the shared slug-word step behind both `route_slug` and
/// `kafka_slug` (kept as one helper so a future third kind's slug rule
/// can't drift from the first two).
fn ascii_words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|seg| !seg.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

/// The one kind-prefixed-slug rule behind all four entry-point kinds:
/// split `words_source` into alphanumeric words, lowercase, join with
/// `-`, prefixed by `kind` — or `kind-{empty_fallback}` if there were no
/// words at all (`ROADMAP.md` M18 flags `http-fights` vs `kafka-fights`
/// as a real future collision, hence the prefix). Code-review finding:
/// this used to be four near-identical 7-line functions, differing only
/// in their prefix string and empty-input fallback — exactly the drift
/// risk `ascii_words`'s own doc comment already worried about, just one
/// layer up.
fn kind_slug(kind: &str, words_source: &str, empty_fallback: &str) -> String {
    let words = ascii_words(words_source);
    if words.is_empty() {
        format!("{kind}-{empty_fallback}")
    } else {
        format!("{kind}-{}", words.join("-"))
    }
}

/// `("get", "/entity/fruits/{id}")` -> `"http-get-entity-fruits-id"`.
fn route_slug(verb: &str, path: &str) -> String {
    kind_slug(&format!("http-{verb}"), path, "root")
}

/// `"fights"` -> `"kafka-fights"`.
fn kafka_slug(channel: &str) -> String {
    kind_slug("kafka", channel, "channel")
}

/// `"increment"` -> `"scheduled-increment"`. Keyed on the method name --
/// a `@Scheduled` method has no path/channel-equivalent identity of its
/// own (`ROADMAP.md`'s "cron expr" is title material, not a stable,
/// collision-safe id: two methods can share an identical schedule).
fn scheduled_slug(method_name: &str) -> String {
    kind_slug("scheduled", method_name, "job")
}

fn is_scheduled_annotation(marker: &str) -> bool {
    crate::java::bare_annotation_name(marker) == "Scheduled"
}

/// The single value bound to `attr` inside `marker`'s parens (e.g.
/// `attr_value("@Scheduled(every = \"10s\")", "every")` -> `Some("10s")`),
/// whether the value is a quoted string or a bare token (Quarkus's own
/// `@Scheduled` always quotes; the real Spring-interop shape in
/// `quarkus-quickstarts/spring-scheduled-quickstart` does not —
/// `fixedRate = 1000` is a bare integer literal). `None` if `attr` isn't
/// one of the annotation's arguments.
fn attr_value(marker: &str, attr: &str) -> Option<String> {
    let m = marker.trim_start();
    let start = m.find('(')?;
    let end = m.rfind(')')?;
    if end <= start {
        return None;
    }
    for part in split_top_level_commas(&m[start + 1..end]) {
        let (name, value) = part.split_once('=')?;
        if name.trim() != attr {
            continue;
        }
        let value = value.trim();
        if let Some(lit) = first_string_literal(value) {
            return Some(lit);
        }
        return Some(value.to_string());
    }
    None
}

/// Split `s` on top-level commas only -- one inside a `"…"` string
/// literal (a cron expression can itself contain commas, e.g.
/// `"0 0 12 * * MON,WED,FRI"`) is never a split point. Deliberately not a
/// general expression parser: `@Scheduled`'s argument list is flat name
/// `=` value pairs, never nested annotations or parens.
fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                parts.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// The most human-readable scheduling detail off a `@Scheduled(...)`
/// marker, tried in priority order (a cron expression is the most
/// informative; a bare `fixedRate`/`fixedRateString` the least). `None`
/// if none of the known attribute names appear — the entry point is
/// still created off `is_scheduled_annotation` alone (see the call
/// site), just with a plainer title.
fn scheduled_detail(marker: &str) -> Option<String> {
    for attr in ["cron", "every", "fixedRate", "fixedRateString"] {
        if let Some(v) = attr_value(marker, attr) {
            return Some(format!("{attr}={v}"));
        }
    }
    None
}

/// Does `class_id` carry a class-level `@GrpcService`? Unlike JAX-RS and
/// Kafka, gRPC's own annotation lives only on the class (the generated
/// proto interface -- `Greeter` here, from a `.proto` file this project
/// doesn't parse at all -- is what actually shapes the RPC methods); a
/// `@GrpcService` class's public methods are what `enumerate_entry_points`
/// treats as entry points, without requiring the interface's own
/// `@Override` marker (this project has already declined to walk
/// interface hierarchies for inherited annotations).
fn is_grpc_service_class(graph: &Graph, class_id: SymbolId) -> bool {
    let Some(sym) = graph.get_symbol(class_id) else {
        return false;
    };
    sym.markers
        .iter()
        .any(|m| crate::java::bare_annotation_name(m) == "GrpcService")
}

/// A member has no `is_exported`/visibility field of its own (M13 design
/// decision 9's `ExtractedSymbol.is_exported` is deliberately
/// file-level-only — see `java.rs::push_named_leaf`), but `signature` is
/// the declaration's own source text up to its body, so the `public`
/// modifier is right there to check textually — same "read what's
/// already captured" spirit as `sym.signature.contains("PanacheRepository")`
/// in `is_cdi_managed`. Any leading annotations (the real
/// `@Override public Uni<HelloReply> sayHello(...)` shape — `@Override`
/// always comes first in `signature`, before the modifiers) are skipped
/// first, so `public` is found regardless of how many markers precede it.
fn is_public_signature(signature: &str) -> bool {
    let mut rest = signature.trim_start();
    while let Some(after_at) = rest.strip_prefix('@') {
        let after_name =
            after_at.trim_start_matches(|c: char| c.is_alphanumeric() || c == '_' || c == '.');
        rest = match after_name.strip_prefix('(') {
            Some(after_paren) => match matching_close_paren(after_paren) {
                Some(idx) => &after_paren[idx + 1..],
                None => after_name,
            },
            None => after_name,
        }
        .trim_start();
    }
    rest.starts_with("public")
}

/// The index in `s` of the `)` matching an already-consumed opening `(`
/// — quote-aware (a string-literal annotation argument can itself
/// contain a `)`, e.g. `@Counted(description = "calls (per request)")`,
/// the real code-review-caught bug: a naive `find(')')` stops inside the
/// literal) and paren-depth-aware (Java allows a nested annotation as an
/// attribute value, `@Foo(@Bar(1))`). `None` if unbalanced.
fn matching_close_paren(s: &str) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut in_quotes = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            '(' if !in_quotes => depth += 1,
            ')' if !in_quotes => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// `"sayHello"` -> `"grpc-sayhello"`. Same kind-prefix rule as the other
/// three kinds, keyed on the method name — gRPC has no path/channel-
/// equivalent identity either, same reasoning as `scheduled_slug`.
fn grpc_slug(method_name: &str) -> String {
    kind_slug("grpc", method_name, "method")
}

/// A `@Incoming`/`@Outgoing` annotation argument, before resolution: an
/// inline string literal is already the answer, but a bare identifier
/// (`@Incoming(FIGHTS_CHANNEL_NAME)`, the real `quarkus-super-heroes`
/// shape) names a same-class constant that `resolve_channel_name` still
/// has to look up.
#[derive(Debug, PartialEq)]
enum ChannelArg {
    Literal(String),
    ConstantName(String),
}

/// The single argument inside `marker`'s parens, classified as a literal
/// or a bare constant reference. `None` for a no-arg or malformed
/// annotation.
fn channel_arg(marker: &str) -> Option<ChannelArg> {
    let m = marker.trim_start();
    let start = m.find('(')?;
    let end = m.rfind(')')?;
    if end <= start {
        return None;
    }
    let inner = m[start + 1..end].trim();
    if inner.is_empty() {
        return None;
    }
    if let Some(lit) = first_string_literal(inner) {
        return Some(ChannelArg::Literal(lit));
    }
    Some(ChannelArg::ConstantName(inner.to_string()))
}

/// `marker` if it's a bare `@Incoming(...)`, else `None`.
fn incoming_channel(marker: &str) -> Option<ChannelArg> {
    if crate::java::bare_annotation_name(marker) != "Incoming" {
        return None;
    }
    channel_arg(marker)
}

/// `marker` if it's a bare `@Outgoing(...)`, else `None`.
fn outgoing_channel(marker: &str) -> Option<ChannelArg> {
    if crate::java::bare_annotation_name(marker) != "Outgoing" {
        return None;
    }
    channel_arg(marker)
}

/// Resolve a Kafka channel-name annotation argument to its actual string
/// value. A literal resolves to itself; a constant reference is looked
/// up among `class_id`'s own `Value`-kind members for one whose name
/// matches, extracting the first string literal out of *that* symbol's
/// `signature` (a field's signature is its whole declaration text,
/// initializer included — see `java.rs::signature_before_body`). Same
/// "textual scan within known scope" pattern as `same_package_edges`,
/// deliberately not a general interpreter: same class only, no cross-
/// file follow. Falls back to the raw identifier if no matching constant
/// is found, so an entry point is never silently dropped over an
/// unresolved reference — just less nicely named.
fn resolve_channel_name<'g>(
    graph: &'g Graph,
    class_id: SymbolId,
    arg: ChannelArg,
    class_constants_cache: &mut HashMap<SymbolId, Vec<&'g crate::symbol::Symbol>>,
) -> String {
    match arg {
        ChannelArg::Literal(s) => s,
        ChannelArg::ConstantName(name) => {
            // Same per-class memoization as `class_path_cache` /
            // `is_rest_client_cache` / `is_grpc_service_cache` above --
            // code-review finding: this used to re-scan every symbol in
            // the graph on every call instead of caching a class's own
            // constants once, inconsistent with this file's otherwise
            // uniform caching discipline.
            let constants = class_constants_cache.entry(class_id).or_insert_with(|| {
                graph
                    .symbols()
                    .filter(|s| s.parent == Some(class_id) && s.kind == SymbolKind::Value)
                    .collect()
            });
            constants
                .iter()
                .find(|s| s.id.ends_with(&format!("::{name}")))
                .and_then(|s| first_string_literal(&s.signature))
                .unwrap_or(name)
        }
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

    #[test]
    fn channel_arg_distinguishes_literals_from_bare_constant_references() {
        assert_eq!(
            channel_arg("@Incoming(\"prices\")"),
            Some(ChannelArg::Literal("prices".to_string()))
        );
        assert_eq!(
            channel_arg("@Incoming(FIGHTS_CHANNEL_NAME)"),
            Some(ChannelArg::ConstantName("FIGHTS_CHANNEL_NAME".to_string()))
        );
        assert_eq!(channel_arg("@Incoming()"), None);
        assert_eq!(channel_arg("@Incoming"), None, "no parens at all");
    }

    #[test]
    fn incoming_and_outgoing_channel_only_match_their_own_annotation() {
        assert_eq!(
            incoming_channel("@Incoming(\"prices\")"),
            Some(ChannelArg::Literal("prices".to_string()))
        );
        assert_eq!(incoming_channel("@Outgoing(\"prices\")"), None);
        assert_eq!(
            outgoing_channel("@Outgoing(\"generated-price\")"),
            Some(ChannelArg::Literal("generated-price".to_string()))
        );
        assert_eq!(outgoing_channel("@Incoming(\"prices\")"), None);
        assert_eq!(incoming_channel("@Blocking"), None);
    }

    #[test]
    fn kafka_slugs_are_kind_prefixed_and_lowercased() {
        assert_eq!(kafka_slug("fights"), "kafka-fights");
        assert_eq!(kafka_slug("winner-stats"), "kafka-winner-stats");
    }

    #[test]
    fn scheduled_annotations_are_matched_exactly() {
        assert!(is_scheduled_annotation("@Scheduled(every = \"10s\")"));
        assert!(is_scheduled_annotation("@Scheduled"));
        assert!(!is_scheduled_annotation("@Incoming(\"prices\")"));
    }

    #[test]
    fn attr_value_extracts_quoted_and_bare_values_by_name() {
        assert_eq!(
            attr_value(
                "@Scheduled(every = \"10s\", identity = \"task-job\")",
                "every"
            ),
            Some("10s".to_string())
        );
        assert_eq!(
            attr_value(
                "@Scheduled(every = \"10s\", identity = \"task-job\")",
                "identity"
            ),
            Some("task-job".to_string())
        );
        assert_eq!(
            attr_value("@Scheduled(fixedRate = 1000)", "fixedRate"),
            Some("1000".to_string())
        );
        assert_eq!(attr_value("@Scheduled(every = \"10s\")", "cron"), None);
    }

    #[test]
    fn split_top_level_commas_never_splits_inside_a_quoted_cron_expression() {
        assert_eq!(
            split_top_level_commas("cron = \"0 0 12 * * MON,WED,FRI\", identity = \"x\""),
            vec!["cron = \"0 0 12 * * MON,WED,FRI\"", " identity = \"x\"",]
        );
    }

    #[test]
    fn scheduled_detail_prefers_cron_over_every_over_fixed_rate() {
        assert_eq!(
            scheduled_detail("@Scheduled(cron = \"0 15 10 * * ?\")"),
            Some("cron=0 15 10 * * ?".to_string())
        );
        assert_eq!(
            scheduled_detail("@Scheduled(every = \"10s\")"),
            Some("every=10s".to_string())
        );
        assert_eq!(
            scheduled_detail("@Scheduled(fixedRate = 1000)"),
            Some("fixedRate=1000".to_string())
        );
        assert_eq!(scheduled_detail("@Scheduled(identity = \"x\")"), None);
    }

    #[test]
    fn scheduled_slugs_are_kind_prefixed_and_lowercased() {
        assert_eq!(scheduled_slug("increment"), "scheduled-increment");
        assert_eq!(scheduled_slug("cronJob"), "scheduled-cronjob");
    }

    #[test]
    fn is_public_signature_skips_leading_annotations() {
        assert!(is_public_signature(
            "@Override\n    public Uni<HelloReply> sayHello(HelloRequest request)"
        ));
        assert!(is_public_signature("public String greet(String name)"));
        assert!(!is_public_signature(
            "private Uni<HelloReply> format(String name)"
        ));
        assert!(!is_public_signature(
            "@Blocking\n    void store(int priceInUsd)"
        ));
        assert!(is_public_signature(
            "@Blocking @Transactional\n    public void store(int priceInUsd)"
        ));
    }

    #[test]
    fn is_public_signature_handles_a_closing_paren_inside_an_annotation_string_argument() {
        // Code-review finding: a naive "find the first `)`" skip stops
        // inside a string literal that itself contains a `)`, leaving
        // stray characters in place of `public` -- a realistic
        // MicroProfile Metrics annotation shape.
        assert!(is_public_signature(
            "@Counted(description = \"calls (per request)\")\n    public Uni<HelloReply> sayHello(HelloRequest request)"
        ));
        // Also covers a nested annotation attribute value.
        assert!(is_public_signature("@Foo(@Bar(1))\n    public void go()"));
    }

    #[test]
    fn grpc_slugs_are_kind_prefixed_and_lowercased() {
        assert_eq!(grpc_slug("sayHello"), "grpc-sayhello");
    }
}
