//! Integration (M18, commit 1): the Quarkus feature model's entry-point
//! layer — `enumerate_entry_points` recognizing JAX-RS `@Path` + verb
//! (`@GET`/`@POST`/…) annotations end to end through `RepoIndex` (real
//! `detect()` → `JavaStack` → `QuarkusFeatureModel`), not just the unit
//! tests on the parsing helpers themselves (see `src/quarkus.rs`).
//!
//! Fixtures are drawn from real `quarkus-quickstarts` shapes (M18's chosen
//! test repo — see `ARCHITECTURE.md` open question 11 for why
//! `quarkus-super-heroes` was set aside for this specific piece): a plain
//! `@Path("/hello")` resource with a method-level sub-path
//! (`getting-started/GreetingResource`), and a CRUD resource whose class
//! path has no leading slash and whose methods mix bare verbs with
//! `@Path("{id}")` (`hibernate-orm-panache-quickstart/FruitEntityResource`).

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
