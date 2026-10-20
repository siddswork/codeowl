---
kind: file
source_paths: [tests/quarkus_spec.rs]
file: { source_hash: 56f6df770c6394191954f6c91812205b71ede1141cb239955fe4f5f3b208a7c7, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 9ffe36c2b5388c8a2a1283692993043884f3f28ff465bd86b92f84b367fd6cc7 }
symbols:
  tests/quarkus_spec.rs::a_class_path_plus_method_path_join_into_one_http_entry_point: { source_hash: 43de154757dff282c1b3361cfff4a7af1af68a761addfac52ff5d55ead92c735, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 1739d771d7113fd0f403584225d48c498d800d60f39dd05e32ca0dddf57c7ad0 }
  tests/quarkus_spec.rs::fully_qualified_annotations_with_no_imports_are_still_recognized: { source_hash: 2ae01bbc62f81dcb40054e85d14937867d2c16670c31a7dbd4c25d44167f160d, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 5bd33d183612e5ae93fe779d483b3a3717b245ffee09e06b41cb732ec90458b6 }
  tests/quarkus_spec.rs::a_generated_interfaces_own_annotations_never_produce_an_entry_point_directly: { source_hash: d509d4edfae0ddbe7d9e8219002f4db5a603dedb6f8961c6d112d076625cb374, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 25815f0ab655fff809cac2533a38ab93cfb60e817bd567082e06f74462180c5b }
  tests/quarkus_spec.rs::a_hand_written_class_inherits_its_generated_interfaces_entry_point: { source_hash: 2937cec142140e973a5b916572adc5040981ea6eb81fbef1667d23c2bffd6658, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 1c731148c40dadcc85b91b9c5b08c2c895db6d4882ee681a2eb21d61bf6aae68 }
  tests/quarkus_spec.rs::implements_clause_names_the_interface_fully_qualified_inline_with_no_import: { source_hash: babe52cabb54aa598c3c6a2adca9908037fb0874fe293cd614c9a38701df4a97, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 623a869f8eb62745facaa7241a728d17489073e6b77a0f06ef091eb6f8069c26 }
  tests/quarkus_spec.rs::a_generated_interface_is_found_even_when_not_the_first_implements_name: { source_hash: b13e7e5462a57c4bfdfcb3dc919a602be2134bcd2f419b8d698bb2a3364b63fc, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 8d369876b683800319c7cdfa74e64a888ecb3c3c9733831d52d15464d48cd613 }
  tests/quarkus_spec.rs::method_path_is_inherited_independently_of_an_own_verb_marker: { source_hash: b98e7d665c9b4838de054c93c7178e7f625c5f95257201f1f3e42335dcc33c00, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 856c020584875815beec2ea028b5178084d955aa51fa020850658d7ee2913ae8 }
  tests/quarkus_spec.rs::a_class_path_with_no_leading_slash_and_mixed_verb_annotations_all_resolve: { source_hash: 6be4f329bb070c8a2be2193c97b16f89b520932d60d3c102d5fb9879c16e5365, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: dbf1d5317b9c33a8a5a983bf16ea95af31bc23cc269e18c2b53b0557ee63f4c7 }
  tests/quarkus_spec.rs::a_nested_provider_class_with_an_override_is_not_an_entry_point: { source_hash: 8a00cb197d86ba3c06bb3514b7389346ff1f673e2da4140e3bc800982de70a02, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: ec3c3af6343230cabbda40901fcac23b81a3a9d51066250b3ee657ef532702e3 }
  tests/quarkus_spec.rs::an_injected_application_scoped_panache_repository_joins_core: { source_hash: df4f28396ad185ba7effd24f555463b9bf6b5e2a308e6b5c6e977d74313efa5f, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 08ca1f427fb95fcdb99b6487aec1353dbbfe98615174daf0b119511423c8f6ab }
  tests/quarkus_spec.rs::a_cross_package_fully_qualified_call_with_no_import_still_joins_dependencies: { source_hash: 1001dbc790b8a1b4f6c06439d5122769fda1f846cb3b4ce4ad70fb276a7d46ad, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 9406481805e3dc0339b2cb3bd70b7dc7866b95a88ad4d026d0d292019f30be0a }
  tests/quarkus_spec.rs::a_plain_unannotated_helper_class_stays_a_one_hop_dependency: { source_hash: d09fb9ab784b6194bb342fa9ed10aa590f4e08f0a423c214358f5f41eacaf9a7, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 99f77ba6a6b8fac391440a1b7f9710cb05be93947921fe71eb7216bfd10262dd }
  tests/quarkus_spec.rs::a_panache_entity_is_schema_and_a_repository_using_it_stays_core: { source_hash: a5fb89ca47ee4b66dca8ca467cbcb083de22a2142f98c5b3d660ed6dbd350de3, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: b638b2860606876e55b1d16735b8efe2a7406670a4d7ee4b36ccee3d912f4ffc }
  tests/quarkus_spec.rs::a_register_rest_client_interface_is_not_an_entry_point: { source_hash: e273792d84b85e2001d1a0d180f911c096c2c81edb2e4c3b64e57d532a9a2c91, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: b5bc10aa8a15c9ae80456a70ee0ba1c41f00bdaeea0ed90b88a951f4d56e66bd }
  tests/quarkus_spec.rs::an_incoming_only_consumer_method_is_a_kafka_entry_point: { source_hash: d475985f6e62b24125140cb67fcaed94ec7f619ae7e3a443ca3fffb39125d15f, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: eed5d0388f44a8067d496529dcbd9b96d285e42df1187bff25f489f8b55b7a0a }
  tests/quarkus_spec.rs::an_outgoing_only_producer_method_is_a_kafka_entry_point: { source_hash: 6a2903e3cf788c25e2a69774a37d2ccf8d45fbc683226a72852146c2bf659991, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: da81e5bc005ac0946b556d9076795acdc24e2eb688a9c300df21e1c142f41470 }
  tests/quarkus_spec.rs::a_combined_incoming_and_outgoing_method_resolves_same_class_constants_into_one_entry_point: { source_hash: 2774b884c06cd59adf94c91c0a6a735b9e825cc9a5e36a888fef3feac71bf5e8, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 891ce9fe8a955342d3e2477af7ce1112a77a8b42da574e94f4e8e864b2e19e25 }
  tests/quarkus_spec.rs::a_channel_annotated_constructor_param_is_not_its_own_entry_point: { source_hash: daf29fe2e2254b8236b3c915cdee290425d546e75c391a33b05eb7c0722e1777, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: e8c6edfb5a5a84db58bea4b1be8d08111d79fdee2fbcf0c941ac24a440537b3c }
  tests/quarkus_spec.rs::http_and_kafka_slugs_never_collide_on_the_same_bare_word: { source_hash: 313c16e85cae168c44a3140003ad7413b0d916eb72c92b41ef81016b3f30a6f0, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 2737d7b52813bb3a2addf209b17b20410b5797a8955e7f1c79439874013ce072 }
  tests/quarkus_spec.rs::scheduled_jobs_are_entry_points_keyed_on_method_name_with_distinct_details: { source_hash: 81a41d04db8e750f4c0ec41212f674238712b6f7ae835ecb0155b7ee8a1daef0, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 72d5f50acebee65b09ac95d0caf20955d02760b3516d6d0b49abcdd85d7f8d77 }
  tests/quarkus_spec.rs::a_scheduled_fixed_rate_bare_numeric_attribute_parses_without_quotes: { source_hash: 421959470e3058ff09d8a754315951e11c1bdabdf12cfcee339a8aefb8157ec9, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 4a574bddcfdf1f80823826feb679fc0022935685e2f9aff29ff9cfe297b0b19c }
  tests/quarkus_spec.rs::a_grpc_services_public_method_is_a_grpc_entry_point: { source_hash: 63e7169fa42d3a9e11bd3b75846ec0832b4d5f4e988d65e85e22d0a8911ef057, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 13f46e9eb4918c6ab65d011e96fa0a69f0b2032a1deed99bc2cedd85b882e0c1 }
  tests/quarkus_spec.rs::a_grpc_services_private_helper_method_is_not_an_entry_point: { source_hash: 6f2a359e53c0612ee2a2a470c202a9413d9a32e15b824d27aabcfe3591ae9257, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: cfd92ad72bdfeb5ad5218a400d60ba01b5516c38ab150065e5f07866803986e3 }
  tests/quarkus_spec.rs::a_non_grpc_classs_public_methods_are_not_grpc_entry_points: { source_hash: 46eab9a42fc085b3bc3f3f3162a10d9d2aa7173e507943a00bed3f059b2397e8, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 553ed5e52b9332e58cd541454145e510ff48a2ae9b307b7993d62b5d3af6fafb }
dep_targets:
  file -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_class_path_plus_method_path_join_into_one_http_entry_point -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::fully_qualified_annotations_with_no_imports_are_still_recognized -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_generated_interfaces_own_annotations_never_produce_an_entry_point_directly -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_hand_written_class_inherits_its_generated_interfaces_entry_point -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::implements_clause_names_the_interface_fully_qualified_inline_with_no_import -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_generated_interface_is_found_even_when_not_the_first_implements_name -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::method_path_is_inherited_independently_of_an_own_verb_marker -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_class_path_with_no_leading_slash_and_mixed_verb_annotations_all_resolve -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_nested_provider_class_with_an_override_is_not_an_entry_point -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::an_injected_application_scoped_panache_repository_joins_core -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_cross_package_fully_qualified_call_with_no_import_still_joins_dependencies -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_plain_unannotated_helper_class_stays_a_one_hop_dependency -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_panache_entity_is_schema_and_a_repository_using_it_stays_core -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_register_rest_client_interface_is_not_an_entry_point -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::an_incoming_only_consumer_method_is_a_kafka_entry_point -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::an_outgoing_only_producer_method_is_a_kafka_entry_point -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_combined_incoming_and_outgoing_method_resolves_same_class_constants_into_one_entry_point -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_channel_annotated_constructor_param_is_not_its_own_entry_point -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::http_and_kafka_slugs_never_collide_on_the_same_bare_word -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::scheduled_jobs_are_entry_points_keyed_on_method_name_with_distinct_details -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_scheduled_fixed_rate_bare_numeric_attribute_parses_without_quotes -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_grpc_services_public_method_is_a_grpc_entry_point -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_grpc_services_private_helper_method_is_not_an_entry_point -> src/index.rs::RepoIndex: 481419547059
  tests/quarkus_spec.rs::a_non_grpc_classs_public_methods_are_not_grpc_entry_points -> src/index.rs::RepoIndex: 481419547059
---
# tests/quarkus_spec.rs
## Summary
End-to-end tests for Quarkus support, which recognises entry points in a Java web-service project. Each test writes a small sample project to a temporary folder, opens it through the normal index, which detects Java and picks the Quarkus feature model, and checks the entry points found. The samples are drawn from real quickstart and demo projects. For web routes, the tests check a class address and a method address are joined into one full route, a class address with no leading slash and a mix of verbs and annotation orders resolve to exactly the right routes, and a nested error-handling class is not mistaken for a route. For generated code, they check routes written with full package names are still recognised, a generated interface never becomes an entry point on its own, a hand-written class that implements a generated interface inherits its routes, and the lookup works when the interface is named in full inline or is not first in the `implements` list, and when a method repeats the verb but omits the path. For a client interface marked `@RegisterRestClient`, they check it is excluded, since it describes a call this service makes. For message handlers, they check consumers, producers and methods that do both, with channel names written as constants, are found, and that an injected sender is not. For scheduled jobs and remote-call services, they check jobs are identified by method name with their interval or cron shown, Spring's bare-number rate is parsed, only public methods of a `@GrpcService` class count, and a plain class does not. Others check the join rules for a feature's contents: a framework-managed repository joins the core, a plain helper stays a one-hop dependency, a database entity is data and never core, and a cross-package call written in full still counts as a dependency. One checks a web route and a message handler with the same bare name get different ids.

## `a_class_path_plus_method_path_join_into_one_http_entry_point`
`fn a_class_path_plus_method_path_join_into_one_http_entry_point()`
### Summary
A test that checks a Java web resource's class-level address and a method's own address are joined into one full route, and that a method with no address of its own uses the class address alone.
### Behavior
Writes a sample `GreetingResource` into a temporary project: the class has `@Path("/hello")`, one method has `@GET` and `@Path("/greeting/{name}")`, and another has just `@GET`. It opens the repo through the normal index, which detects Java, and gets the feature model, which must exist for Java.

It finds the entry point with the id `http-get-hello-greeting-name` and asserts its kind is `http`, its title is `GET /hello/greeting/{name}`, and its file is the resource file, so the class path and method path were joined. It then finds `http-get-hello` and asserts its title is `GET /hello`, which shows a bare `@GET` with no method-level `@Path` falls back to the class path alone. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `fully_qualified_annotations_with_no_imports_are_still_recognized`
`fn fully_qualified_annotations_with_no_imports_are_still_recognized()`
### Summary
A regression test that checks web route annotations written with their full package path, such as `@jakarta.ws.rs.Path`, are still recognised, which is how machine-generated Java interfaces write them.
### Behavior
Machine-generated interfaces, such as those produced from an API contract, have no imports and write every annotation fully qualified, since generated code has no reason to import names that no person will read. The annotation matching used to strip only the leading `@`, so `@jakarta.ws.rs.Path(...)` never matched the plain name `Path`, and no route was found. This was fixed before generated interfaces were wired in, so it would not be discovered afterwards.

It writes a sample `HeroesResource` interface with no imports, a fully-qualified `@jakarta.ws.rs.Path("/api/heroes")` on the interface, and a method with a fully-qualified `@jakarta.ws.rs.GET` and `@jakarta.ws.rs.Path("/random")`. It opens the repo through the normal index and gets the feature model. It asserts an entry point exists with the title `GET /api/heroes/random`, with kind `http` and the interface's file, so the pair was recognised and the class and method addresses were joined. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_generated_interfaces_own_annotations_never_produce_an_entry_point_directly`
`fn a_generated_interfaces_own_annotations_never_produce_an_entry_point_directly()`
### Summary
A test that checks a machine-generated interface never becomes a feature entry point on its own, even though its route annotations would match. A feature built from it would be empty and look like a real answer instead of an obvious gap.
### Behavior
Once generated source folders are read into the graph, and fully-qualified annotations are matchable, a generated interface would otherwise be reported as an entry point, attributed to a read-only file with no body and no calls into any service or repository. That is worse than finding nothing, because the resulting description would be structurally empty yet look like a working answer. The intended design is that a hand-written class implementing the interface picks up the interface's annotations through a one-hop lookup, so the interface itself must never produce its own entry point.

It writes a hand-written `App` class with no routes, and a generated `HeroesResource` interface under `target/generated-sources/...` with fully-qualified route annotations, with no hand-written implementor at all. It opens the repo through the normal index, which reads the generated folder, and lists the entry points. It asserts none of them has a file path containing `generated-sources`, failing with the list if any does. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_hand_written_class_inherits_its_generated_interfaces_entry_point`
`fn a_hand_written_class_inherits_its_generated_interfaces_entry_point()`
### Summary
A test that checks a hand-written Java class that implements a generated interface gets that interface's web route as its own entry point, and that the route is attributed to the hand-written file, which is the one a description should be about, not the generated one.
### Behavior
This is the real shape in projects whose API is generated from a contract: the hand-written resource implements a generated interface, and every route annotation lives on the interface while the class itself carries only `@Override`. The class imports the interface from a different package, which the existing import resolution already handles with no new mechanism.

It writes a hand-written `HeroResource` that implements `HeroesResource`, which it imports from `org.acme.generated`, and a generated `HeroesResource` interface under `target/generated-sources/...` carrying the fully-qualified class-level and method-level route annotations. It opens the repo through the normal index and lists the entry points.

It asserts an entry point exists with the title `GET /api/heroes/random` and kind `http`, and that its file is `src/main/java/org/acme/HeroResource.java`, the hand-written implementor, and never the generated interface. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `implements_clause_names_the_interface_fully_qualified_inline_with_no_import`
`fn implements_clause_names_the_interface_fully_qualified_inline_with_no_import()`
### Summary
A test that checks a hand-written Java class whose `implements` clause names its interface by the full package path, with no import, still finds that interface's route. This is needed when the class and the interface share the same short name.
### Behavior
This shape was found on a real project. A hand-written class `NarrationResource` implements a generated interface that is also called `NarrationResource`, so Java's own naming rules force the interface to be written fully qualified inline, because importing it would collide with the enclosing class's own name. There is therefore no import link to look up, unlike a plain `implements HeroesResource` that has a separate import.

It writes the hand-written class with `implements org.acme.api.resources.NarrationResource`, and the generated interface under `target/generated-sources/...` in that package, with a class-level `@jakarta.ws.rs.Path("/api/narration")` and a method annotated `@jakarta.ws.rs.POST`. It opens the repo through the normal index and lists the entry points.

It asserts an entry point exists with the title `POST /api/narration`, which shows the fully qualified name was resolved by its path, and that its file is the hand-written class under `src/main/java/org/acme/rest/`, never the generated interface. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_generated_interface_is_found_even_when_not_the_first_implements_name`
`fn a_generated_interface_is_found_even_when_not_the_first_implements_name()`
### Summary
A regression test for a bug found in code review: a class that implements several interfaces, with the generated one not listed first, used to miss the generated interface's routes entirely. The test checks it is now found.
### Behavior
The first version of the lookup only looked at the first name after `implements`, so `implements Serializable, HeroesResource` would silently miss `HeroesResource`. That is valid Java, even though no project this tool had been run on happened to order its interfaces that way.

It writes a hand-written `HeroResource` that imports the generated `HeroesResource` and declares `implements java.io.Serializable, HeroesResource`, so the generated interface is second. It also writes the generated interface under `target/generated-sources/...` with fully-qualified route annotations for `GET /api/heroes/random`. It opens the repo through the normal index and lists the entry points. It asserts that some entry point has the title `GET /api/heroes/random`, failing with the whole list otherwise, which shows every name in the clause is tried in order. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `method_path_is_inherited_independently_of_an_own_verb_marker`
`fn method_path_is_inherited_independently_of_an_own_verb_marker()`
### Summary
A regression test for a bug found in code review: when a hand-written method repeats `@GET` but leaves out `@Path`, the missing path must still be taken from the generated interface's matching method, independently of the verb.
### Behavior
The first version of the lookup chose one whole source of annotations, either the method's own or the interface's, based only on whether the method had a verb. A hand-written override that redeclared `@GET` but omitted `@Path`, a plausible copy-paste shape, therefore used its own annotations for both checks and silently lost the inherited `/random` path.

It writes a hand-written `HeroResource` whose method has `@Override` and a fully-qualified `@GET` but no `@Path`, and a generated `HeroesResource` with a class-level `@Path("/api/heroes")` and a matching method with `@GET` and `@Path("/random")`. It opens the repo through the normal index and lists the entry points. It finds the one in the hand-written file, which must exist because its own `@GET` still produces an entry point, and asserts its title is `GET /api/heroes/random`. That shows the path was inherited from the interface's method even though the method's own annotation had already supplied the verb. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_class_path_with_no_leading_slash_and_mixed_verb_annotations_all_resolve`
`fn a_class_path_with_no_leading_slash_and_mixed_verb_annotations_all_resolve()`
### Summary
A test that checks a realistic Java resource, with a class address that has no leading slash and a mix of verbs and annotation orders, produces exactly the five expected routes and no extras.
### Behavior
The sample mirrors a real resource. The class has `@Path("entity/fruits")` with no leading slash. Its methods cover a bare `@GET`, a `@GET` with `@Path("{id}")`, a `@POST` with no path, a `@PUT` where the `@Path` comes before the verb, and a `@DELETE` with `@Path("{id}")` after it. The path and the verb are two independent annotations, unlike FastAPI's single decorator call, so their order must not matter.

It opens the repo through the normal index and collects the entry point ids. It asserts the list contains `http-get-entity-fruits`, `http-get-entity-fruits-id`, `http-post-entity-fruits`, `http-put-entity-fruits-id` and `http-delete-entity-fruits-id`, with a message noting that annotation order must not matter for the PUT. It asserts there are exactly five entry points, so the class itself produces no false positive. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_nested_provider_class_with_an_override_is_not_an_entry_point`
`fn a_nested_provider_class_with_an_override_is_not_an_entry_point()`
### Summary
A test that checks an error-handling class nested inside a web resource is not mistaken for a route, even though its method carries `@Override` and the outer class has a `@Path`.
### Behavior
The real shape is a nested `@Provider` class, `ErrorMapper`, that implements an exception mapper. Its `toResponse` method has `@Override` but no web verb, so it must never be counted as a route just because it sits inside a class annotated with `@Path`.

It writes a sample resource with `@Path("entity/fruits")`, one real `@GET` method, and the nested provider class with its overridden method. It opens the repo through the normal index and lists the entry points. It asserts there is exactly one entry point, with a message saying only the real `@GET` route counts and not `toResponse`, and that its id is `http-get-entity-fruits`. The test shows routes are recognised by a web verb annotation on a method, not by being inside an annotated class. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `an_injected_application_scoped_panache_repository_joins_core`
`fn an_injected_application_scoped_panache_repository_joins_core()`
### Summary
A test that checks a framework-managed data-access class used by a web route joins that route's feature, so the feature description covers where the real data work happens.
### Behavior
The sample mirrors a real quickstart. A web resource, `FruitRepositoryResource`, has a field marked `@Inject` of type `FruitRepository`. That repository is `@ApplicationScoped` and implements `PanacheRepository<Fruit>`, the textbook case of a framework-managed bean or repository. A plain `Fruit` class holds the data. The injection marker itself is not what the feature code reads: it sees only the resolved reference, any ordinary use of the type, and the framework's runtime wiring is not something it needs to parse.

It writes the three files, opens the repo through the normal index, finds the entry point `http-get-repository-fruits`, and assembles the feature's participants. It asserts the core list contains `src/main/java/org/acme/FruitRepository.java`, which shows a bean marked `@ApplicationScoped` that is also a Panache repository was admitted into the feature's core. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_cross_package_fully_qualified_call_with_no_import_still_joins_dependencies`
`fn a_cross_package_fully_qualified_call_with_no_import_still_joins_dependencies()`
### Summary
A test that checks a helper class in another package, called by its full path with no import line, is still counted as one of a feature's dependencies. Before this was handled, such a dependency was invisible, so a feature description would stay marked current even after the helper's public interface changed.
### Behavior
The sample mirrors a real project. A web resource's `GET /hello` method calls `org.acme.util.Labeler.label("Hello")`, written out in full, and the helper lives in a different package. Java allows this with no import, but the feature code only saw explicit imports, which is the silent miss the open design question described.

It writes the helper class `Labeler` in `org.acme.util` and a `GreetingResource` with `@Path("/hello")` and a `@GET` method that calls it by its full name. It opens the repo through the normal index, finds the entry point `http-get-hello`, and assembles the feature's participants. It asserts the dependencies list contains `src/main/java/org/acme/util/Labeler.java::Labeler`, with a message that the call must surface the same as an explicit import would. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_plain_unannotated_helper_class_stays_a_one_hop_dependency`
`fn a_plain_unannotated_helper_class_stays_a_one_hop_dependency()`
### Summary
A negative test that checks a plain helper class, with no framework-managed scope and no data-access shape, is listed as a one-hop dependency of a feature and not promoted into the feature's core.
### Behavior
Only classes the framework manages, or data-access repositories, should join a feature's core. A plain utility class plays the same role that an injected framework primitive such as a configuration object would; those particular ones are external and would never resolve to a file in the repo, so a local utility class stands in for the case.

It writes a plain `SlugFormatter` with a static method and no annotations, and a `GreetingResource` with `@Path("/hello")` whose `@GET` method calls `SlugFormatter.slugify`. It opens the repo through the normal index, finds the entry point `http-get-hello`, and assembles the feature's participants. It asserts the core list does not contain `SlugFormatter.java`, with a message that a plain class must stay a one-hop dependency, and that the dependencies list does contain `src/main/java/org/acme/SlugFormatter.java::SlugFormatter`, so it is still visible and only tracked by its public shape. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_panache_entity_is_schema_and_a_repository_using_it_stays_core`
`fn a_panache_entity_is_schema_and_a_repository_using_it_stays_core()`
### Summary
A test that checks a Java database entity is treated as a table and listed under a feature's data, while the repository class that uses it still joins the feature's core, and the entity's own file is not also promoted into the core.
### Behavior
The sample combines two real persistence styles. `Fruit` is a plain `@Entity` class, `FruitRepository` is an `@ApplicationScoped` class implementing `PanacheRepository<Fruit>`, the data-access layer around it, and `FruitRepositoryResource` injects that repository in a `GET repository/fruits` route. The test's real purpose is to confirm that adding the table-recognition feature does not regress the earlier rule that such a repository joins the core.

It opens the repo through the normal index. It asserts `Fruit` has the table kind, and that `FruitRepository` does not, since a repository has no columns of its own and is not a table. It finds the entry point `http-get-repository-fruits` and assembles the feature's participants. It asserts the data list contains `Fruit.java::Fruit`, the core contains `FruitRepository.java`, and the core does not contain `Fruit.java`, so the entity appears as data only and never also as core. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_register_rest_client_interface_is_not_an_entry_point`
`fn a_register_rest_client_interface_is_not_an_entry_point()`
### Summary
A test that checks an interface describing an outbound call to another service, marked `@RegisterRestClient`, is not treated as a route this service serves, even though it has the same `@Path` and `@GET` annotations as a real resource.
### Behavior
The real shape is a client interface, `HeroRestClient`, with a class-level `@Path`, a `@RegisterRestClient(configKey = "hero-client")` marker, and a `@GET @Path("/random")` method. The extractor cannot tell it apart from a server resource by looking at the method's own annotations, so the deciding clue is the containing interface's `@RegisterRestClient` marker.

It writes that client interface and, in the same repo, a real server resource `GreetingResource` with `@Path("/hello")` and a `@GET` method, so the exclusion can be checked as narrow: only `@RegisterRestClient` interfaces are skipped, not every interface or every `@Path` type. It opens the repo through the normal index and lists the entry points.

It asserts that none of them is in `HeroRestClient.java`, that there is exactly one entry point, and that its id is `http-get-hello`, so only the real server resource remains. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `an_incoming_only_consumer_method_is_a_kafka_entry_point`
`fn an_incoming_only_consumer_method_is_a_kafka_entry_point()`
### Summary
A test that checks a method that only receives messages from a Kafka channel, marked `@Incoming`, is recognised as its own entry point of the message-handler kind.
### Behavior
The sample mirrors a real quickstart: a `PriceStorage` bean with a method `store` annotated `@Incoming("prices")`, with the channel name as a plain inline string and no matching `@Outgoing`. It opens the repo through the normal index and lists the entry points. It asserts there is exactly one, its kind is `kafka`, its id is `kafka-prices`, and its title contains `prices`. The id comes from the channel name, not from the method name or the schedule. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `an_outgoing_only_producer_method_is_a_kafka_entry_point`
`fn an_outgoing_only_producer_method_is_a_kafka_entry_point()`
### Summary
A test that checks a method that only produces messages for a Kafka channel, marked `@Outgoing`, is recognised as an entry point on its own, even though no application code calls it.
### Behavior
The sample mirrors a real quickstart: a `PriceGenerator` bean with a method `generate` annotated `@Outgoing("generated-price")`. It is a purely declarative producer, invoked by the messaging framework's own timer, with no injected emitter anywhere, so it is a genuine entry point just like a scheduled job. It opens the repo through the normal index and lists the entry points. It asserts there is exactly one, its kind is `kafka`, and its id is `kafka-generated-price`, taken from the channel name. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_combined_incoming_and_outgoing_method_resolves_same_class_constants_into_one_entry_point`
`fn a_combined_incoming_and_outgoing_method_resolves_same_class_constants_into_one_entry_point()`
### Summary
A test that checks a Kafka method that both consumes one channel and publishes to another, with the channel names written as constants in the same class, becomes one entry point with both constants resolved to their real names.
### Behavior
The sample mirrors a real project: a `SuperStats` bean with two `static final String` constants, `FIGHTS_CHANNEL_NAME = "fights"` and `TOP_WINNERS_CHANNEL_NAME = "winner-stats"`, and one method `processFight` carrying both `@Incoming(FIGHTS_CHANNEL_NAME)` and `@Outgoing(TOP_WINNERS_CHANNEL_NAME)`. Neither annotation uses an inline literal.

It opens the repo through the normal index and lists the entry points. It asserts there is exactly one, with a message that both annotations on one method are one entry point and not two; that its kind is `kafka`; and that its id is `kafka-fights`, keyed on the incoming, triggering channel with the constant resolved to the string, not left as the identifier. It asserts that the title contains both `fights` and `winner-stats`, so both resolved names are visible. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_channel_annotated_constructor_param_is_not_its_own_entry_point`
`fn a_channel_annotated_constructor_param_is_not_its_own_entry_point()`
### Summary
A test that checks a constructor parameter marked `@Channel`, which injects a message sender, is not treated as an entry point. It only lets the class send messages later, and the framework never calls it on its own.
### Behavior
The real shape is a `FightService` bean whose constructor takes `@Channel("fights") MutinyEmitter<String> emitter`. That is dependency injection of an emitter for later `.send()` calls from inside some other, already-modeled entry point. Annotations on a parameter are never captured in a symbol's markers, since only class-level and method-level modifiers are, so this should fall out of extraction with no special-casing. The test records that behavior as a checked case, not an assumption.

It writes the sample class, opens the repo through the normal index, and lists the entry points. It asserts the list is empty, failing with the list if anything appears. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `http_and_kafka_slugs_never_collide_on_the_same_bare_word`
`fn http_and_kafka_slugs_never_collide_on_the_same_bare_word()`
### Summary
A test that checks a web route and a Kafka handler that share the same bare word, such as `/fights` and a channel named `fights`, get different feature ids, so neither silently overwrites the other's document.
### Behavior
It guards a risk flagged in the project plan: without a prefix saying what kind of entry point it is, `GET /fights` and a message consumer on the channel `fights` would both be named just `fights` and collide on the same file under `docs/specs/_features/`. It writes a `FightResource` with `@Path("/fights")` and a `@GET` method, and a `FightConsumer` bean with a method annotated `@Incoming("fights")`. It opens the repo through the normal index, lists the entry points and sorts them by id. It asserts the ids are exactly `http-get-fights` and `kafka-fights`, in that order, with a message that the kind prefix must keep them apart. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `scheduled_jobs_are_entry_points_keyed_on_method_name_with_distinct_details`
`fn scheduled_jobs_are_entry_points_keyed_on_method_name_with_distinct_details()`
### Summary
A test that checks scheduled jobs become entry points identified by their method names, and that the schedule detail, such as an interval or a cron expression, appears in the title, so several jobs on one class stay distinct.
### Behavior
The sample mirrors a real quickstart: a `CounterBean` with three `@Scheduled` methods, one interval-based (`every = "10s"`), one with a hardcoded cron expression, and one whose cron is a configuration placeholder, `{cron.expr}`, which is still just a string to show and not something to resolve further. Scheduled jobs have no path or channel to give them an identity, so the method name has to make each id unique.

It opens the repo through the normal index, lists the entry points and sorts them by id. It asserts there are exactly three, all of kind `scheduled`, and that their ids are `scheduled-cronjob`, `scheduled-cronjobwithexpressioninconfig` and `scheduled-increment`, lower-cased from the method names. It then asserts that the title of the interval job contains `10s` and that the title of the cron job contains `0 15 10 * * ?`, so the schedule detail is visible. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_scheduled_fixed_rate_bare_numeric_attribute_parses_without_quotes`
`fn a_scheduled_fixed_rate_bare_numeric_attribute_parses_without_quotes()`
### Summary
A test that checks a scheduled job written with Spring's `fixedRate = 1000`, a bare number with no quotes, is still recognised as an entry point and shows the number in its title.
### Behavior
The sample mirrors a real quickstart that demonstrates Quarkus's compatibility with Spring's scheduling annotation. A `CounterBean` imports `org.springframework.scheduling.annotation.Scheduled` and has a method `jobAtFixedRate` annotated `@Scheduled(fixedRate = 1000)`. Unlike every Quarkus-native attribute, which is a quoted string, this one is a bare numeric literal, and the matching deliberately does not check which package the annotation was imported from.

It opens the repo through the normal index and lists the entry points. It asserts there is exactly one, of kind `scheduled`, with the id `scheduled-jobatfixedrate`, and that its title contains `1000`, so the bare number surfaces. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_grpc_services_public_method_is_a_grpc_entry_point`
`fn a_grpc_services_public_method_is_a_grpc_entry_point()`
### Summary
A test that checks a public method of a class marked `@GrpcService` is recognised as a remote-call entry point, even though the marker is on the class and the method itself carries only `@Override`.
### Behavior
The sample mirrors a real quickstart: a `HelloWorldService` class with a class-level `@GrpcService`, implementing `Greeter`, an interface generated from a `.proto` file, which this tool does not parse at all. Its method `sayHello` has `@Override`. Entry-point detection deliberately does not require that marker, since the project has declined to walk interface hierarchies for inherited annotations. It keys on the method being public on a class already marked `@GrpcService`.

It opens the repo through the normal index and lists the entry points. It asserts there is exactly one, of kind `grpc`, with the id `grpc-sayhello`, and that its title contains `sayHello`. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_grpc_services_private_helper_method_is_not_an_entry_point`
`fn a_grpc_services_private_helper_method_is_not_an_entry_point()`
### Summary
A test that checks a private helper method on a `@GrpcService` class is not treated as an entry point. Only its public methods are real remote calls.
### Behavior
A gRPC service class can have ordinary helper methods next to its public remote-call methods, and the gRPC runtime never calls a private one directly. It writes a `HelloWorldService` with a public `sayHello` method, which calls a private `format` helper. It opens the repo through the normal index and lists the entry points. It asserts there is exactly one, with a message that the private helper must not also become an entry point, and that its id is `grpc-sayhello`. The test shows the rule is "public method on a gRPC service class", not "any method". It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_non_grpc_classs_public_methods_are_not_grpc_entry_points`
`fn a_non_grpc_classs_public_methods_are_not_grpc_entry_points()`
### Summary
A test that checks a plain class with public methods, and no `@GrpcService` marker at all, produces no entry points, which shows the class-level marker is what gates the rule and not just a method's own visibility.
### Behavior
It writes a `PlainHelper` class with one public method, `greet`, and no framework annotations. It opens the repo through the normal index and lists the entry points. It asserts the list is empty, failing with the list if anything appears. Together with the test of a private helper, this pins both halves of the gRPC rule: a method counts only when it is public and its class is marked `@GrpcService`. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index
