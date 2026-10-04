---
kind: file
source_paths: [tests/java_spec.rs]
file: { source_hash: 56c01160389a76624891d254a2c2991562674d5fec4fe2238882e24c7e1fc75c, deps_hash: 79c4a84bd83881d6d3e203ca569b918810f2a977acfa4f1d805692aee2c48a84, spec_hash: ef64c8cd9235d39abcf70e1845e835fcf6ca93888661cb3f48a37b62a70a57da }
symbols:
  tests/java_spec.rs::java_graph: { source_hash: c94e97052cc99407bc92cf2f09838f6f10bc5d8a74fe56b960d1a7d655f2005d, deps_hash: 5586d60160ea05dea2f3ff8ad9971353ca672d465b93cc63fce7decdb63dffaf, spec_hash: a6f196e5e9ee4c78b69b1b65682f81f70d90536c0b207f81b712b4cad39310a9 }
  tests/java_spec.rs::a_class_renders_as_one_section_with_methods_and_a_nested_enum_folded_in: { source_hash: 175257dd58b496f06b5f897de529b9c06c0e887093f3136e210c31d1f66345fd, deps_hash: 479097bfebd5c3e7d47ff551cd258b8de6b674e73b8c589a4c15c5fb5036c394, spec_hash: a8e706d51515c65e3a726bb876cba78dd087a938e9fc1b2a7c9cf3efa7e68c6d }
  tests/java_spec.rs::a_same_package_reference_resolves_end_to_end_through_repo_index: { source_hash: 4914691296174975079cade93306046a6c24157babd9f9b1c3a04a7914160b90, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: a97b02a79f249aa0009b60d3bf09d8959fb4b915bd086f6c6302315493502bb7 }
dep_targets:
  file -> src/graph.rs::FileExtraction: 0cb1c67d8a8b
  file -> src/graph.rs::Graph: 13dbdfa88b0e
  file -> src/hash.rs::hash_text: 70c099c1622c
  file -> src/index.rs::RepoIndex: 481419547059
  file -> src/spec.rs::SpecTask: 1f6b409c41df
  file -> src/spec.rs::next_task: 9d894695159c
  file -> src/spec.rs::read_file_spec: 6682201bb38f
  file -> src/spec.rs::render: bed79b32a9ec
  file -> src/spec.rs::submit: 6b3606c2f3b8
  tests/java_spec.rs::java_graph -> src/graph.rs::FileExtraction: 0cb1c67d8a8b
  tests/java_spec.rs::java_graph -> src/graph.rs::Graph: 13dbdfa88b0e
  tests/java_spec.rs::java_graph -> src/hash.rs::hash_text: 70c099c1622c
  tests/java_spec.rs::a_class_renders_as_one_section_with_methods_and_a_nested_enum_folded_in -> src/spec.rs::SpecTask: 1f6b409c41df
  tests/java_spec.rs::a_class_renders_as_one_section_with_methods_and_a_nested_enum_folded_in -> src/spec.rs::next_task: 9d894695159c
  tests/java_spec.rs::a_class_renders_as_one_section_with_methods_and_a_nested_enum_folded_in -> src/spec.rs::read_file_spec: 6682201bb38f
  tests/java_spec.rs::a_class_renders_as_one_section_with_methods_and_a_nested_enum_folded_in -> src/spec.rs::render: bed79b32a9ec
  tests/java_spec.rs::a_class_renders_as_one_section_with_methods_and_a_nested_enum_folded_in -> src/spec.rs::submit: 6b3606c2f3b8
  tests/java_spec.rs::a_same_package_reference_resolves_end_to_end_through_repo_index -> src/index.rs::RepoIndex: 481419547059
---
# tests/java_spec.rs
## Summary
End-to-end tests for Java support, following the same path the real spec-writing command drives, not just the functions in isolation. The first test sends a sample Java class with three methods, one of them private, and a nested enum through extraction, the graph, the loop that hands out writing tasks, and the rendering of the saved document. It checks that the class is described as a single section, that its Javadoc reaches the task, that only the top-level class is offered as a task and neither its methods nor the nested enum, and that the rendered output has exactly one top-level section and none for the enum. It also checks that a private method is not exported yet stays in the graph. The second test writes two classes in the same package, one using the other with no import, which Java allows, and opens the project through the normal index, checking that a link to the used class is found and resolved to the right declaration, and that there is no invented reverse link from the used class back. A small helper builds a one-file graph from Java text.

## `java_graph`
`fn java_graph(rel_path: &str, src: &str) -> Graph`
### Summary
A test helper that turns one piece of Java source text into a small graph, so the tests can look at how a single Java file is described without building a whole repo.
### Behavior
Takes a file path and the file's text. It extracts the declarations with the Java extractor, wraps them in a per-file extraction record together with a fingerprint of the text, and builds a graph from that one record. There is no import resolution and no other file, so the graph holds just that file's node and its symbols. It cannot fail itself, since the extractor returns an empty list for text it cannot parse.
### Depends on
- `src/graph.rs::FileExtraction` — codeowl::graph
- `src/graph.rs::Graph` — codeowl::graph
- `src/hash.rs::hash_text` — codeowl::hash

## `a_class_renders_as_one_section_with_methods_and_a_nested_enum_folded_in`
`fn a_class_renders_as_one_section_with_methods_and_a_nested_enum_folded_in()`
### Summary
A test that checks a Java class is documented as a single section, with its methods and its nested enum folded into that one section and not given sections of their own, and that the writing loop asks for exactly one piece of writing for it.
### Behavior
Writes a sample `StringUtils` source file into a temporary repo, builds a one-file graph from it, and first asserts that a `private` helper method named `len` is not exported, though it stays in the graph.

It then runs the generate loop up to 20 times: each time it asks for the next task and, for each, submits canned text long enough to pass the quality check, so nothing is offered again and the cap only guards against a hang. For a symbol task on the class it also asserts that the class's Javadoc, "Operations on {@link String} that are null safe.", is carried into the task. After the loop it asserts the only symbol offered was the top-level class itself, not its methods or the nested enum, and that the file-level summary was also requested and answered.

Finally it reads the saved spec back and renders it, asserting that the output has a section for `StringUtils`, no section for the nested `Pad` enum under either name, and exactly one top-level section in total. It removes the temporary folder at the end. Any failed assertion fails the test.
### Depends on
- `src/spec.rs::SpecTask` — codeowl::spec
- `src/spec.rs::next_task` — codeowl::spec
- `src/spec.rs::read_file_spec` — codeowl::spec
- `src/spec.rs::render` — codeowl::spec
- `src/spec.rs::submit` — codeowl::spec

## `a_same_package_reference_resolves_end_to_end_through_repo_index`
`fn a_same_package_reference_resolves_end_to_end_through_repo_index()`
### Summary
An end-to-end test that checks a Java class using another class from the same package, with no import line, still gets a link to it, and that the link goes only one way.
### Behavior
Creates a temporary project with two files in the package `com.foo`: class `A`, whose method calls `B.value()`, and class `B`, which has a static method and never mentions `A`. Neither file has an import, which Java allows for same-package classes. It opens the repo through the normal index, which detects the Java language pack, walks the files, and builds the graph with its links.

It asserts that an edge exists from `A.java` for the name `B`, then that the edge resolved to a target and that the target's id is `src/main/java/com/foo/B.java::B`. It then asserts there is no edge from `B.java` for the name `A`, so the scan is directional and does not invent a reverse link. It deletes the temporary folder at the end. The test shows the full path from files on disk to a resolved link, not only the resolver function in isolation.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index
