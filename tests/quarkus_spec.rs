//! Integration (M18): the Quarkus feature model's entry-point layer —
//! `enumerate_entry_points` recognizing JAX-RS `@Path` + verb
//! (`@GET`/`@POST`/…) and Kafka `@Incoming`/`@Outgoing` annotations end
//! to end through `RepoIndex` (real `detect()` → `JavaStack` →
//! `QuarkusFeatureModel`), not just the unit tests on the parsing
//! helpers themselves (see `src/quarkus.rs`).
//!
//! HTTP fixtures are drawn from real `quarkus-quickstarts` shapes (M18's
//! chosen test repo — see `ARCHITECTURE.md` open question 11 for why
//! `quarkus-super-heroes` was set aside for this specific piece): a plain
//! `@Path("/hello")` resource with a method-level sub-path
//! (`getting-started/GreetingResource`), and a CRUD resource whose class
//! path has no leading slash and whose methods mix bare verbs with
//! `@Path("{id}")` (`hibernate-orm-panache-quickstart/FruitEntityResource`).
//!
//! Kafka fixtures are drawn from real shapes in **both** test repos: a
//! pure `@Incoming`-only consumer and a pure `@Outgoing`-only declarative
//! producer (`quarkus-quickstarts/kafka-panache-quickstart`'s
//! `PriceStorage`/`PriceGenerator`), and a method carrying **both**
//! annotations at once plus same-class constant channel names
//! (`quarkus-super-heroes/event-statistics`'s `SuperStats.processFight`,
//! the actual milestone corpus's shape) — real evidence census in
//! `src/quarkus.rs`'s module doc comment.

use codeowl::index::RepoIndex;

#[test]
fn a_class_path_plus_method_path_join_into_one_http_entry_point() {
    let dir =
        std::env::temp_dir().join(format!("codeowl-quarkus-spec-{}-hello", std::process::id()));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/GreetingResource.java"),
        "package org.acme;\n\
         \n\
         import jakarta.ws.rs.GET;\n\
         import jakarta.ws.rs.Path;\n\
         \n\
         @Path(\"/hello\")\n\
         public class GreetingResource {\n\
         \n\
         \x20   @GET\n\
         \x20   @Path(\"/greeting/{name}\")\n\
         \x20   public String greeting(String name) {\n\
         \x20       return \"hi \" + name;\n\
         \x20   }\n\
         \n\
         \x20   @GET\n\
         \x20   public String hello() {\n\
         \x20       return \"hello\";\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    let greeting = eps
        .iter()
        .find(|e| e.id == "http-get-hello-greeting-name")
        .expect("class path + method path join into one route");
    assert_eq!(greeting.kind, "http");
    assert_eq!(greeting.title, "GET /hello/greeting/{name}");
    assert_eq!(
        greeting.file,
        "src/main/java/org/acme/GreetingResource.java"
    );

    let hello = eps
        .iter()
        .find(|e| e.id == "http-get-hello")
        .expect("a bare @GET with no method-level @Path falls back to the class path alone");
    assert_eq!(hello.title, "GET /hello");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn fully_qualified_annotations_with_no_imports_are_still_recognized() {
    // Real shape confirmed against an actual `mvn compile` of
    // quarkus-super-heroes (M19 dogfooding, 2026-09-19): OpenAPI-codegen'd
    // interfaces carry zero imports and write every annotation fully
    // qualified (`@jakarta.ws.rs.Path(...)`, `@jakarta.ws.rs.GET`) rather
    // than the bare `@Path`/`@GET` every hand-written class in this
    // project's other test repos uses -- generated code has no reason to
    // import anything a human will never read. `bare_annotation_name`
    // (java.rs) and `parse_verb`/`parse_path_annotation`'s own inline
    // `@`-stripping (quarkus.rs) both missed this: they stopped at
    // stripping the leading `@`, so `@jakarta.ws.rs.Path(...)` never
    // matched a bare `"Path"` comparison anywhere in this file. This is
    // exactly the shape M19 commit 3's generated-interface lookup will
    // hand these functions, so it must be fixed before that lands, not
    // discovered after.
    let dir =
        std::env::temp_dir().join(format!("codeowl-quarkus-spec-{}-fqcn", std::process::id()));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/HeroesResource.java"),
        "package org.acme;\n\
         \n\
         @jakarta.ws.rs.Path(\"/api/heroes\")\n\
         public interface HeroesResource {\n\
         \n\
         \x20   @jakarta.ws.rs.GET\n\
         \x20   @jakarta.ws.rs.Path(\"/random\")\n\
         \x20   public Object getRandomHero();\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    let ep = eps
        .iter()
        .find(|e| e.title == "GET /api/heroes/random")
        .expect("a fully-qualified @Path/@GET pair must still be recognized as an entry point");
    assert_eq!(ep.kind, "http");
    assert_eq!(ep.file, "src/main/java/org/acme/HeroesResource.java");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_class_path_with_no_leading_slash_and_mixed_verb_annotations_all_resolve() {
    // FruitEntityResource's real shape: `@Path("entity/fruits")` (no
    // leading slash), a bare `@GET`, a `@GET @Path("{id}")`, a `@POST`
    // with no path, and a `@PUT`/`@DELETE` each with `@Path("{id}")` --
    // annotation order also varies (verb before or after `@Path`), which
    // must not matter since they're two independent markers, not one
    // decorator call the way FastAPI's `.get(path)` is.
    let dir =
        std::env::temp_dir().join(format!("codeowl-quarkus-spec-{}-fruit", std::process::id()));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/FruitResource.java"),
        "package org.acme;\n\
         \n\
         import jakarta.ws.rs.DELETE;\n\
         import jakarta.ws.rs.GET;\n\
         import jakarta.ws.rs.POST;\n\
         import jakarta.ws.rs.PUT;\n\
         import jakarta.ws.rs.Path;\n\
         \n\
         @Path(\"entity/fruits\")\n\
         public class FruitResource {\n\
         \n\
         \x20   @GET\n\
         \x20   public String get() { return \"[]\"; }\n\
         \n\
         \x20   @GET\n\
         \x20   @Path(\"{id}\")\n\
         \x20   public String getSingle(Long id) { return \"{}\"; }\n\
         \n\
         \x20   @POST\n\
         \x20   public String create() { return \"{}\"; }\n\
         \n\
         \x20   @Path(\"{id}\")\n\
         \x20   @PUT\n\
         \x20   public String update(Long id) { return \"{}\"; }\n\
         \n\
         \x20   @DELETE\n\
         \x20   @Path(\"{id}\")\n\
         \x20   public String delete(Long id) { return \"\"; }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    let ids: Vec<&str> = eps.iter().map(|e| e.id.as_str()).collect();
    assert!(ids.contains(&"http-get-entity-fruits"), "{ids:?}");
    assert!(ids.contains(&"http-get-entity-fruits-id"), "{ids:?}");
    assert!(ids.contains(&"http-post-entity-fruits"), "{ids:?}");
    assert!(
        ids.contains(&"http-put-entity-fruits-id"),
        "annotation order (`@Path` before `@PUT`) must not matter: {ids:?}"
    );
    assert!(ids.contains(&"http-delete-entity-fruits-id"), "{ids:?}");
    assert_eq!(eps.len(), 5, "no false positives from the class itself");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_nested_provider_class_with_an_override_is_not_an_entry_point() {
    // Real shape: `FruitEntityResource.ErrorMapper` is a nested
    // `@Provider` class implementing `ExceptionMapper`, whose
    // `toResponse` is `@Override`-annotated but carries no JAX-RS verb --
    // it must never be mistaken for a route just because it sits inside a
    // `@Path`-annotated outer class.
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-provider",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/FruitResource.java"),
        "package org.acme;\n\
         \n\
         import jakarta.ws.rs.GET;\n\
         import jakarta.ws.rs.Path;\n\
         import jakarta.ws.rs.core.Response;\n\
         import jakarta.ws.rs.ext.ExceptionMapper;\n\
         import jakarta.ws.rs.ext.Provider;\n\
         \n\
         @Path(\"entity/fruits\")\n\
         public class FruitResource {\n\
         \n\
         \x20   @GET\n\
         \x20   public String get() { return \"[]\"; }\n\
         \n\
         \x20   @Provider\n\
         \x20   public static class ErrorMapper implements ExceptionMapper<Exception> {\n\
         \x20       @Override\n\
         \x20       public Response toResponse(Exception e) { return null; }\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    assert_eq!(
        eps.len(),
        1,
        "only the real @GET route, not toResponse: {eps:?}"
    );
    assert_eq!(eps[0].id, "http-get-entity-fruits");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_injected_application_scoped_panache_repository_joins_core() {
    // Real shape: `hibernate-orm-panache-quickstart/FruitRepositoryResource`
    // has `@Inject FruitRepository fruitRepository;`, and
    // `FruitRepository` is `@ApplicationScoped` + `implements
    // PanacheRepository<Fruit>` -- the textbook CDI-managed-bean-or-
    // Panache-repository case `ROADMAP.md`'s M18 section describes.
    // `@Inject` itself isn't load-bearing here -- the resolved reference
    // (any ordinary use of the type) is what the walk sees; CDI wiring
    // is a runtime concept `admits_to_core` doesn't need to parse.
    let dir =
        std::env::temp_dir().join(format!("codeowl-quarkus-spec-{}-repo", std::process::id()));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/Fruit.java"),
        "package org.acme;\n\
         \n\
         public class Fruit {\n\
         \x20   public Long id;\n\
         \x20   public String name;\n\
         }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/FruitRepository.java"),
        "package org.acme;\n\
         \n\
         import jakarta.enterprise.context.ApplicationScoped;\n\
         import io.quarkus.hibernate.orm.panache.PanacheRepository;\n\
         \n\
         @ApplicationScoped\n\
         public class FruitRepository implements PanacheRepository<Fruit> {\n\
         }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/FruitRepositoryResource.java"),
        "package org.acme;\n\
         \n\
         import jakarta.inject.Inject;\n\
         import jakarta.ws.rs.GET;\n\
         import jakarta.ws.rs.Path;\n\
         \n\
         @Path(\"repository/fruits\")\n\
         public class FruitRepositoryResource {\n\
         \n\
         \x20   @Inject\n\
         \x20   FruitRepository fruitRepository;\n\
         \n\
         \x20   @GET\n\
         \x20   public String get() { return fruitRepository.listAll().toString(); }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let ep = fm
        .enumerate_entry_points(&graph)
        .into_iter()
        .find(|e| e.id == "http-get-repository-fruits")
        .expect("the GET repository/fruits route");

    let p = codeowl::features::assemble_participants(&graph, fm, &ep);
    assert!(
        p.core
            .contains(&"src/main/java/org/acme/FruitRepository.java".to_string()),
        "an @ApplicationScoped Panache repository joins core: {p:?}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_plain_unannotated_helper_class_stays_a_one_hop_dependency() {
    // The negative case ROADMAP.md calls out: a referenced class with no
    // CDI scope and no Panache shape (a plain POJO/util class, the same
    // role an injected `Config`/`ObjectMapper` framework primitive plays
    // -- those specific two are external and would never even resolve to
    // a repo file, so this in-repo stand-in is what actually exercises
    // the "stays a stub" branch) must not be promoted to `core`.
    let dir =
        std::env::temp_dir().join(format!("codeowl-quarkus-spec-{}-plain", std::process::id()));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/SlugFormatter.java"),
        "package org.acme;\n\
         \n\
         public class SlugFormatter {\n\
         \x20   public static String slugify(String s) { return s.toLowerCase(); }\n\
         }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/GreetingResource.java"),
        "package org.acme;\n\
         \n\
         import jakarta.ws.rs.GET;\n\
         import jakarta.ws.rs.Path;\n\
         \n\
         @Path(\"/hello\")\n\
         public class GreetingResource {\n\
         \n\
         \x20   @GET\n\
         \x20   public String hello() { return SlugFormatter.slugify(\"Hello\"); }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let ep = fm
        .enumerate_entry_points(&graph)
        .into_iter()
        .find(|e| e.id == "http-get-hello")
        .expect("the GET /hello route");

    let p = codeowl::features::assemble_participants(&graph, fm, &ep);
    assert!(
        !p.core
            .contains(&"src/main/java/org/acme/SlugFormatter.java".to_string()),
        "a plain unannotated class must stay a one-hop dependency, not core: {p:?}"
    );
    assert!(
        p.dependencies
            .contains(&"src/main/java/org/acme/SlugFormatter.java::SlugFormatter".to_string()),
        "it should still show up as a dependency: {p:?}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_panache_entity_is_schema_and_a_repository_using_it_stays_core() {
    // The two real quarkus-quickstarts persistence idioms in one fixture:
    // `Fruit` is a plain JPA `@Entity` (repository style -- no Panache
    // base of its own), `FruitRepository implements
    // PanacheRepository<Fruit>` is the DAO around it, and
    // `FruitRepositoryResource` injects the repository. The entity must
    // land in `data` (not `core`, not `dependencies`); the repository
    // must still land in `core`, exactly as
    // `an_injected_application_scoped_panache_repository_joins_core`
    // already established -- this test's real job is confirming that
    // adding the schema seam doesn't regress that.
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-schema",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/Fruit.java"),
        "package org.acme;\n\
         \n\
         import jakarta.persistence.Entity;\n\
         import jakarta.persistence.Id;\n\
         \n\
         @Entity\n\
         public class Fruit {\n\
         \x20   @Id\n\
         \x20   public Long id;\n\
         \x20   public String name;\n\
         }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/FruitRepository.java"),
        "package org.acme;\n\
         \n\
         import jakarta.enterprise.context.ApplicationScoped;\n\
         import io.quarkus.hibernate.orm.panache.PanacheRepository;\n\
         \n\
         @ApplicationScoped\n\
         public class FruitRepository implements PanacheRepository<Fruit> {\n\
         }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/FruitRepositoryResource.java"),
        "package org.acme;\n\
         \n\
         import jakarta.inject.Inject;\n\
         import jakarta.ws.rs.GET;\n\
         import jakarta.ws.rs.Path;\n\
         \n\
         @Path(\"repository/fruits\")\n\
         public class FruitRepositoryResource {\n\
         \n\
         \x20   @Inject\n\
         \x20   FruitRepository fruitRepository;\n\
         \n\
         \x20   @GET\n\
         \x20   public String get() { return fruitRepository.listAll().toString(); }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();

    let fruit = graph
        .find("src/main/java/org/acme/Fruit.java::Fruit")
        .expect("Fruit node");
    assert_eq!(
        graph.get_symbol(fruit).unwrap().kind,
        codeowl::symbol::SymbolKind::Schema,
        "an @Entity class is retagged Schema"
    );
    let repo = graph
        .find("src/main/java/org/acme/FruitRepository.java::FruitRepository")
        .expect("FruitRepository node");
    assert_ne!(
        graph.get_symbol(repo).unwrap().kind,
        codeowl::symbol::SymbolKind::Schema,
        "a PanacheRepository is not itself a table"
    );

    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let ep = fm
        .enumerate_entry_points(&graph)
        .into_iter()
        .find(|e| e.id == "http-get-repository-fruits")
        .expect("the GET repository/fruits route");
    let p = codeowl::features::assemble_participants(&graph, fm, &ep);

    assert!(
        p.data
            .contains(&"src/main/java/org/acme/Fruit.java::Fruit".to_string()),
        "the entity is a data participant: {p:?}"
    );
    assert!(
        p.core
            .contains(&"src/main/java/org/acme/FruitRepository.java".to_string()),
        "the repository still joins core, schema seam notwithstanding: {p:?}"
    );
    assert!(
        !p.core
            .contains(&"src/main/java/org/acme/Fruit.java".to_string()),
        "the entity's own file must never be promoted to core: {p:?}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_register_rest_client_interface_is_not_an_entry_point() {
    // Real shape: `rest-fights/client/HeroRestClient` -- a
    // `@RegisterRestClient` interface with the exact same `@Path` +
    // `@GET` shape as a real server resource, but it describes an
    // *outbound* call this service makes to another one's endpoint, not
    // something this service serves. `extract_file` genuinely can't tell
    // the two apart from a method's own markers alone (confirmed: it
    // extracts as an ordinary Callable with `@GET`/`@Path` markers) --
    // the discriminator is the *containing interface's* own
    // `@RegisterRestClient` marker.
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-restclient",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme/client")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/client/HeroRestClient.java"),
        "package org.acme.client;\n\
         \n\
         import jakarta.ws.rs.GET;\n\
         import jakarta.ws.rs.Path;\n\
         import org.eclipse.microprofile.rest.client.inject.RegisterRestClient;\n\
         \n\
         @Path(\"/api/heroes\")\n\
         @RegisterRestClient(configKey = \"hero-client\")\n\
         interface HeroRestClient {\n\
         \x20   @GET\n\
         \x20   @Path(\"/random\")\n\
         \x20   String findRandomHero();\n\
         }\n",
    )
    .unwrap();
    // A real server resource in the same repo, to confirm the exclusion
    // is narrow -- only `@RegisterRestClient` interfaces are skipped, not
    // every interface or every `@Path`-annotated type.
    std::fs::write(
        dir.join("src/main/java/org/acme/GreetingResource.java"),
        "package org.acme;\n\
         \n\
         import jakarta.ws.rs.GET;\n\
         import jakarta.ws.rs.Path;\n\
         \n\
         @Path(\"/hello\")\n\
         public class GreetingResource {\n\
         \x20   @GET\n\
         \x20   public String hello() { return \"hello\"; }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    assert!(
        !eps.iter().any(|e| e.file.ends_with("HeroRestClient.java")),
        "a @RegisterRestClient interface's methods must never be entry points: {eps:?}"
    );
    assert_eq!(
        eps.len(),
        1,
        "only the real server resource remains: {eps:?}"
    );
    assert_eq!(eps[0].id, "http-get-hello");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_incoming_only_consumer_method_is_a_kafka_entry_point() {
    // Real shape: quarkus-quickstarts/kafka-panache-quickstart's
    // PriceStorage -- a plain `@Incoming("prices")` consumer, inline
    // string literal, no `@Outgoing` counterpart.
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-kafka-incoming-only",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/PriceStorage.java"),
        "package org.acme;\n\
         \n\
         import jakarta.enterprise.context.ApplicationScoped;\n\
         import org.eclipse.microprofile.reactive.messaging.Incoming;\n\
         \n\
         @ApplicationScoped\n\
         public class PriceStorage {\n\
         \x20   @Incoming(\"prices\")\n\
         \x20   public void store(int priceInUsd) {\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    assert_eq!(eps.len(), 1, "{eps:?}");
    assert_eq!(eps[0].kind, "kafka");
    assert_eq!(eps[0].id, "kafka-prices");
    assert!(eps[0].title.contains("prices"), "{:?}", eps[0].title);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_outgoing_only_producer_method_is_a_kafka_entry_point() {
    // Real shape: kafka-panache-quickstart's PriceGenerator -- a pure
    // declarative producer, invoked by the reactive-messaging runtime's
    // own ticker, no injected `@Channel` emitter anywhere in sight.
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-kafka-outgoing-only",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/PriceGenerator.java"),
        "package org.acme;\n\
         \n\
         import jakarta.enterprise.context.ApplicationScoped;\n\
         import org.eclipse.microprofile.reactive.messaging.Outgoing;\n\
         \n\
         @ApplicationScoped\n\
         public class PriceGenerator {\n\
         \x20   @Outgoing(\"generated-price\")\n\
         \x20   public int generate() {\n\
         \x20       return 42;\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    assert_eq!(eps.len(), 1, "{eps:?}");
    assert_eq!(eps[0].kind, "kafka");
    assert_eq!(eps[0].id, "kafka-generated-price");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_combined_incoming_and_outgoing_method_resolves_same_class_constants_into_one_entry_point() {
    // Real shape: quarkus-super-heroes/event-statistics's SuperStats --
    // the actual milestone corpus's Kafka listener. `processFight`
    // carries BOTH `@Incoming` and `@Outgoing` on the same method (it
    // consumes one channel and republishes to another), and neither
    // annotation uses an inline literal -- both name a same-class
    // `static final String` constant instead. Must be exactly one entry
    // point (keyed on the incoming/triggering channel), with both
    // constants resolved by name, not left as the raw identifiers.
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-kafka-combined",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/SuperStats.java"),
        "package org.acme;\n\
         \n\
         import jakarta.enterprise.context.ApplicationScoped;\n\
         import org.eclipse.microprofile.reactive.messaging.Incoming;\n\
         import org.eclipse.microprofile.reactive.messaging.Outgoing;\n\
         \n\
         @ApplicationScoped\n\
         public class SuperStats {\n\
         \x20   static final String FIGHTS_CHANNEL_NAME = \"fights\";\n\
         \x20   static final String TOP_WINNERS_CHANNEL_NAME = \"winner-stats\";\n\
         \n\
         \x20   @Incoming(FIGHTS_CHANNEL_NAME)\n\
         \x20   @Outgoing(TOP_WINNERS_CHANNEL_NAME)\n\
         \x20   public String processFight(String fight) {\n\
         \x20       return fight;\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    assert_eq!(
        eps.len(),
        1,
        "@Incoming + @Outgoing on the same method is one entry point, not two: {eps:?}"
    );
    assert_eq!(eps[0].kind, "kafka");
    assert_eq!(
        eps[0].id, "kafka-fights",
        "keyed on the incoming/triggering channel, constant resolved: {eps:?}"
    );
    assert!(
        eps[0].title.contains("fights") && eps[0].title.contains("winner-stats"),
        "both resolved channel names should be visible in the title: {:?}",
        eps[0].title
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_channel_annotated_constructor_param_is_not_its_own_entry_point() {
    // Real shape: quarkus-super-heroes/rest-fights's FightService -- a
    // `@Channel("fights") MutinyEmitter<...>` constructor parameter is
    // dependency injection of an emitter for later imperative `.send()`
    // calls, not something the framework invokes on its own. Annotations
    // on a *parameter* are never captured in a symbol's `markers` in the
    // first place (only class/method-level modifiers are), so this
    // should already just fall out of extraction with no special-casing
    // -- this test locks that behavior in as a real, checked case rather
    // than an assumption.
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-kafka-emitter-param",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/FightService.java"),
        "package org.acme;\n\
         \n\
         import jakarta.enterprise.context.ApplicationScoped;\n\
         import org.eclipse.microprofile.reactive.messaging.Channel;\n\
         import io.smallrye.reactive.messaging.MutinyEmitter;\n\
         \n\
         @ApplicationScoped\n\
         public class FightService {\n\
         \x20   private final MutinyEmitter<String> emitter;\n\
         \n\
         \x20   public FightService(@Channel(\"fights\") MutinyEmitter<String> emitter) {\n\
         \x20       this.emitter = emitter;\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    assert!(
        eps.is_empty(),
        "an injected emitter's constructor param must never become its own entry point: {eps:?}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn http_and_kafka_slugs_never_collide_on_the_same_bare_word() {
    // ROADMAP.md's explicit flagged risk: `GET /fights` and a Kafka
    // consumer on channel `fights` would both slug to `fights` with no
    // kind prefix (`docs/specs/_features/fights.md` collision). Confirms
    // the kind prefix actually keeps them apart end to end.
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-kind-prefix-collision",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/FightResource.java"),
        "package org.acme;\n\
         \n\
         import jakarta.ws.rs.GET;\n\
         import jakarta.ws.rs.Path;\n\
         \n\
         @Path(\"/fights\")\n\
         public class FightResource {\n\
         \x20   @GET\n\
         \x20   public String list() { return \"[]\"; }\n\
         }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/FightConsumer.java"),
        "package org.acme;\n\
         \n\
         import jakarta.enterprise.context.ApplicationScoped;\n\
         import org.eclipse.microprofile.reactive.messaging.Incoming;\n\
         \n\
         @ApplicationScoped\n\
         public class FightConsumer {\n\
         \x20   @Incoming(\"fights\")\n\
         \x20   public void consume(String fight) {\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let mut eps = fm.enumerate_entry_points(&graph);
    eps.sort_by(|a, b| a.id.cmp(&b.id));

    let ids: Vec<&str> = eps.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["http-get-fights", "kafka-fights"],
        "the kind prefix must keep these apart, no silent collision: {eps:?}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn scheduled_jobs_are_entry_points_keyed_on_method_name_with_distinct_details() {
    // Real shape: quarkus-quickstarts/scheduler-quickstart's CounterBean --
    // three `@Scheduled` methods on one class, one interval-based
    // (`every`), two cron-based (one hardcoded, one a config placeholder
    // -- both still just a string to surface, not something to resolve
    // further). No path/channel-equivalent identity exists for this kind,
    // so the method name itself is what has to make the id unique.
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-scheduled",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/CounterBean.java"),
        "package org.acme;\n\
         \n\
         import jakarta.enterprise.context.ApplicationScoped;\n\
         import io.quarkus.scheduler.Scheduled;\n\
         \n\
         @ApplicationScoped\n\
         public class CounterBean {\n\
         \x20   @Scheduled(every = \"10s\")\n\
         \x20   void increment() {\n\
         \x20   }\n\
         \n\
         \x20   @Scheduled(cron = \"0 15 10 * * ?\")\n\
         \x20   void cronJob() {\n\
         \x20   }\n\
         \n\
         \x20   @Scheduled(cron = \"{cron.expr}\")\n\
         \x20   void cronJobWithExpressionInConfig() {\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let mut eps = fm.enumerate_entry_points(&graph);
    eps.sort_by(|a, b| a.id.cmp(&b.id));

    assert_eq!(eps.len(), 3, "{eps:?}");
    assert!(eps.iter().all(|e| e.kind == "scheduled"), "{eps:?}");

    let ids: Vec<&str> = eps.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "scheduled-cronjob",
            "scheduled-cronjobwithexpressioninconfig",
            "scheduled-increment",
        ],
        "method name keeps three same-schedule-shape jobs apart: {eps:?}"
    );

    let increment = eps.iter().find(|e| e.id == "scheduled-increment").unwrap();
    assert!(
        increment.title.contains("10s"),
        "the interval should surface in the title: {:?}",
        increment.title
    );
    let cron_job = eps.iter().find(|e| e.id == "scheduled-cronjob").unwrap();
    assert!(
        cron_job.title.contains("0 15 10 * * ?"),
        "the cron expression should surface in the title: {:?}",
        cron_job.title
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_scheduled_fixed_rate_bare_numeric_attribute_parses_without_quotes() {
    // Real shape: quarkus-quickstarts/spring-scheduled-quickstart's
    // CounterBean, demonstrating Quarkus's Spring-scheduling
    // compatibility layer -- `fixedRate = 1000` is a bare numeric
    // literal, not a quoted string, unlike every other `@Scheduled`
    // attribute this pack has seen so far.
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-scheduled-fixedrate",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/CounterBean.java"),
        "package org.acme;\n\
         \n\
         import jakarta.enterprise.context.ApplicationScoped;\n\
         import org.springframework.scheduling.annotation.Scheduled;\n\
         \n\
         @ApplicationScoped\n\
         public class CounterBean {\n\
         \x20   @Scheduled(fixedRate = 1000)\n\
         \x20   void jobAtFixedRate() {\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    assert_eq!(eps.len(), 1, "{eps:?}");
    assert_eq!(eps[0].kind, "scheduled");
    assert_eq!(eps[0].id, "scheduled-jobatfixedrate");
    assert!(
        eps[0].title.contains("1000"),
        "the bare numeric fixedRate should still surface in the title: {:?}",
        eps[0].title
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_grpc_services_public_method_is_a_grpc_entry_point() {
    // Real shape: quarkus-quickstarts/grpc-plain-text-quickstart's
    // HelloWorldService -- a class-level `@GrpcService` marker (never a
    // method-level one) implementing a generated proto interface
    // (`Greeter`, from `helloworld.proto` -- `.proto` files aren't
    // parsed at all, out of scope). `sayHello` is `@Override`d, but
    // entry-point detection deliberately doesn't require that marker --
    // this project has already declined to walk interface hierarchies
    // for inherited annotations (`ARCHITECTURE.md` open question 11) --
    // it keys on the method being `public` on an already-`@GrpcService`
    // class instead.
    let dir =
        std::env::temp_dir().join(format!("codeowl-quarkus-spec-{}-grpc", std::process::id()));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/HelloWorldService.java"),
        "package org.acme;\n\
         \n\
         import io.quarkus.grpc.GrpcService;\n\
         import io.smallrye.mutiny.Uni;\n\
         \n\
         @GrpcService\n\
         public class HelloWorldService implements Greeter {\n\
         \n\
         \x20   @Override\n\
         \x20   public Uni<HelloReply> sayHello(HelloRequest request) {\n\
         \x20       return null;\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    assert_eq!(eps.len(), 1, "{eps:?}");
    assert_eq!(eps[0].kind, "grpc");
    assert_eq!(eps[0].id, "grpc-sayhello");
    assert!(eps[0].title.contains("sayHello"), "{:?}", eps[0].title);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_grpc_services_private_helper_method_is_not_an_entry_point() {
    // A `@GrpcService` class can still have ordinary non-RPC helper
    // methods alongside its public RPC ones -- only the public methods
    // are real entry points (a private helper is never invoked by the
    // gRPC runtime directly).
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-grpc-private-helper",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/HelloWorldService.java"),
        "package org.acme;\n\
         \n\
         import io.quarkus.grpc.GrpcService;\n\
         import io.smallrye.mutiny.Uni;\n\
         \n\
         @GrpcService\n\
         public class HelloWorldService implements Greeter {\n\
         \n\
         \x20   @Override\n\
         \x20   public Uni<HelloReply> sayHello(HelloRequest request) {\n\
         \x20       return format(request.getName());\n\
         \x20   }\n\
         \n\
         \x20   private Uni<HelloReply> format(String name) {\n\
         \x20       return null;\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    assert_eq!(
        eps.len(),
        1,
        "the private helper must not also become an entry point: {eps:?}"
    );
    assert_eq!(eps[0].id, "grpc-sayhello");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_non_grpc_classs_public_methods_are_not_grpc_entry_points() {
    // A plain public class with public methods and no `@GrpcService`
    // marker at all must enumerate zero entry points -- confirms the
    // class-level gate actually gates, not just the method's own
    // visibility.
    let dir = std::env::temp_dir().join(format!(
        "codeowl-quarkus-spec-{}-grpc-not-a-service",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("src/main/java/org/acme")).unwrap();
    std::fs::write(
        dir.join("src/main/java/org/acme/PlainHelper.java"),
        "package org.acme;\n\
         \n\
         public class PlainHelper {\n\
         \x20   public String greet(String name) {\n\
         \x20       return \"hi \" + name;\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();

    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();
    let fm = codeowl::features::feature_model_for(&graph).expect("JavaStack has a feature model");
    let eps = fm.enumerate_entry_points(&graph);

    assert!(eps.is_empty(), "{eps:?}");

    std::fs::remove_dir_all(&dir).ok();
}
