---
kind: file
source_paths: [tests/rust_spec.rs]
file: { source_hash: d8305ebde904356df7128e0e04486585a5e7945fce1e6f4bafd6c8d72875a265, deps_hash: 473f697baac7460866fd1fcf5f422866786143fe7c1cc5c68a2c9a0b4277cd58, spec_hash: cfcf490f13f8efa7a28318f5215e61edb033060ca8d75e9be29391c6a84405bf }
symbols:
  tests/rust_spec.rs::rust_graph_multi: { source_hash: 12cc578dcb4d8a6bd79f8cb4e77aea6adec95a586b3afe7213f4c03e5b100e52, deps_hash: 098344bb3b8bc36475225ac2b089b06b1b395c45978ed80e47b01d7e640be2a0, spec_hash: a3f093d8863a9b444ae9145fe52a307e7445f5207f3d530c4f806d4c7c747652 }
  tests/rust_spec.rs::rust_graph: { source_hash: 9e687ed9f2c8e37466df50565432966647b3083dd2177282308e166877c5e796, deps_hash: 5586d60160ea05dea2f3ff8ad9971353ca672d465b93cc63fce7decdb63dffaf, spec_hash: d4197ae1b91e8ebe6cbd815323a03d3c287a744735a87f5c2ef47d21368ab271 }
  tests/rust_spec.rs::section: { source_hash: b3c965cd349a14b1bf14531b7719cdd20ec4bc25566f84ae771a7b3cc621ac04, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 0d4f7ef98f6031f89afb54a500674044bdfb8aa6a3e00e996cc7ab90b8201b45 }
  tests/rust_spec.rs::inherent_impl_is_folded_but_trait_impl_stays_its_own_section: { source_hash: e42ed3424ebf1fb000c27ecba3a0101b6a154d42f3d3f49316be69c083019198, deps_hash: 479097bfebd5c3e7d47ff551cd258b8de6b674e73b8c589a4c15c5fb5036c394, spec_hash: 8f8fef5b1b62201a98fdc12c2085695608399dc6268d22dba6957476af973555 }
  tests/rust_spec.rs::a_zero_field_single_trait_impl_marker_type_folds_into_one_document: { source_hash: 3abe0d1057c3d4cefebe42a5a84c40b15970517153e7e012d1d264e170451b85, deps_hash: 479097bfebd5c3e7d47ff551cd258b8de6b674e73b8c589a4c15c5fb5036c394, spec_hash: 08e59bd5ba22505fb0183e1e8dddbf9ad23028809fdf757fd6572f86edf8611e }
  tests/rust_spec.rs::personal_greeter_src: { source_hash: babff57c70c8efbcdb7fd3fe6d71d9f9d41e2fa83776a89a1c0005e98ed63a66, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 7947b979b246361ff19ed5d1efc659c7f8bd826b8bcbb1e9cb5f10c7507c582e }
  tests/rust_spec.rs::a_types_own_field_change_invalidates_its_trait_impls_document_without_folding: { source_hash: d25e488d131b672a6b2c29447fb0c43d3cb4138f2a4aa1ad85ece0f6bfc5d558, deps_hash: 479097bfebd5c3e7d47ff551cd258b8de6b674e73b8c589a4c15c5fb5036c394, spec_hash: a9d5a73997ac9fc705d2262b7366733a3ae54c47049b3e78c31243750cbc97bb }
  tests/rust_spec.rs::target_src: { source_hash: ad3cfa6d4ae52fc778482d54c4ab7eaf57525eecce78e99181f37fe986537846, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 267a41d0ea7ed43045c3e9d54ee3e1cd3a1c872694c3d8ebf2b82a21615cd4de }
  tests/rust_spec.rs::a_cross_file_fully_qualified_call_gets_a_real_dependency_edge_that_invalidates_on_change: { source_hash: cb7f4a1b7665037628f84bd9c3da0295ea6192a2804b46d69d4ef755013e46f8, deps_hash: 479097bfebd5c3e7d47ff551cd258b8de6b674e73b8c589a4c15c5fb5036c394, spec_hash: 2a734fdbc33bc2f556d248058b7eda492966cd933239244945c438b616c4af39 }
dep_targets:
  file -> src/graph.rs::FileExtraction: 0cb1c67d8a8b
  file -> src/graph.rs::Graph: 13dbdfa88b0e
  file -> src/hash.rs::hash_text: 70c099c1622c
  file -> src/imports.rs::FileImports: 7a834189d6b3
  file -> src/spec.rs::SpecTask: 1f6b409c41df
  file -> src/spec.rs::next_task: 9d894695159c
  file -> src/spec.rs::read_file_spec: 6682201bb38f
  file -> src/spec.rs::render: bed79b32a9ec
  file -> src/spec.rs::submit: 6b3606c2f3b8
  tests/rust_spec.rs::rust_graph_multi -> src/graph.rs::FileExtraction: 0cb1c67d8a8b
  tests/rust_spec.rs::rust_graph_multi -> src/graph.rs::Graph: 13dbdfa88b0e
  tests/rust_spec.rs::rust_graph_multi -> src/hash.rs::hash_text: 70c099c1622c
  tests/rust_spec.rs::rust_graph_multi -> src/imports.rs::FileImports: 7a834189d6b3
  tests/rust_spec.rs::rust_graph -> src/graph.rs::FileExtraction: 0cb1c67d8a8b
  tests/rust_spec.rs::rust_graph -> src/graph.rs::Graph: 13dbdfa88b0e
  tests/rust_spec.rs::rust_graph -> src/hash.rs::hash_text: 70c099c1622c
  tests/rust_spec.rs::inherent_impl_is_folded_but_trait_impl_stays_its_own_section -> src/spec.rs::SpecTask: 1f6b409c41df
  tests/rust_spec.rs::inherent_impl_is_folded_but_trait_impl_stays_its_own_section -> src/spec.rs::next_task: 9d894695159c
  tests/rust_spec.rs::inherent_impl_is_folded_but_trait_impl_stays_its_own_section -> src/spec.rs::read_file_spec: 6682201bb38f
  tests/rust_spec.rs::inherent_impl_is_folded_but_trait_impl_stays_its_own_section -> src/spec.rs::render: bed79b32a9ec
  tests/rust_spec.rs::inherent_impl_is_folded_but_trait_impl_stays_its_own_section -> src/spec.rs::submit: 6b3606c2f3b8
  tests/rust_spec.rs::a_zero_field_single_trait_impl_marker_type_folds_into_one_document -> src/spec.rs::SpecTask: 1f6b409c41df
  tests/rust_spec.rs::a_zero_field_single_trait_impl_marker_type_folds_into_one_document -> src/spec.rs::next_task: 9d894695159c
  tests/rust_spec.rs::a_zero_field_single_trait_impl_marker_type_folds_into_one_document -> src/spec.rs::read_file_spec: 6682201bb38f
  tests/rust_spec.rs::a_zero_field_single_trait_impl_marker_type_folds_into_one_document -> src/spec.rs::render: bed79b32a9ec
  tests/rust_spec.rs::a_zero_field_single_trait_impl_marker_type_folds_into_one_document -> src/spec.rs::submit: 6b3606c2f3b8
  tests/rust_spec.rs::a_types_own_field_change_invalidates_its_trait_impls_document_without_folding -> src/spec.rs::SpecTask: 1f6b409c41df
  tests/rust_spec.rs::a_types_own_field_change_invalidates_its_trait_impls_document_without_folding -> src/spec.rs::next_task: 9d894695159c
  tests/rust_spec.rs::a_types_own_field_change_invalidates_its_trait_impls_document_without_folding -> src/spec.rs::read_file_spec: 6682201bb38f
  tests/rust_spec.rs::a_types_own_field_change_invalidates_its_trait_impls_document_without_folding -> src/spec.rs::render: bed79b32a9ec
  tests/rust_spec.rs::a_types_own_field_change_invalidates_its_trait_impls_document_without_folding -> src/spec.rs::submit: 6b3606c2f3b8
  tests/rust_spec.rs::a_cross_file_fully_qualified_call_gets_a_real_dependency_edge_that_invalidates_on_change -> src/spec.rs::SpecTask: 1f6b409c41df
  tests/rust_spec.rs::a_cross_file_fully_qualified_call_gets_a_real_dependency_edge_that_invalidates_on_change -> src/spec.rs::next_task: 9d894695159c
  tests/rust_spec.rs::a_cross_file_fully_qualified_call_gets_a_real_dependency_edge_that_invalidates_on_change -> src/spec.rs::read_file_spec: 6682201bb38f
  tests/rust_spec.rs::a_cross_file_fully_qualified_call_gets_a_real_dependency_edge_that_invalidates_on_change -> src/spec.rs::render: bed79b32a9ec
  tests/rust_spec.rs::a_cross_file_fully_qualified_call_gets_a_real_dependency_edge_that_invalidates_on_change -> src/spec.rs::submit: 6b3606c2f3b8
---
# tests/rust_spec.rs
## Summary
End-to-end tests for Rust support, following the same path the real spec-writing command drives on CodeOwl's own code: extraction, the graph, the loop that hands out writing tasks, and the rendering of the saved document. They pin down how Rust types are documented. A plain `impl Foo` block is folded into the type's own section, while a trait implementation keeps its own section, which the first test checks with a counter type, a `Display` implementation and a free function. A second test checks the narrow special case where a type with no fields and one trait implementation is folded too, because a separate document for it would have nothing real to say. Two more tests check that changes reach the right dependents. When a type gains a public field, the description of a trait implementation in the same file goes stale even though its own text did not change, while an unrelated trait does not, and the type never appears to depend on itself. And when a function in one file reaches a type in another file through a fully-written `crate::` path with no `use` line, it is recorded as depending on that type, with the real module path, and goes stale when the type changes. Helpers build one-file and multi-file graphs with their links resolved the way the real index does, create sample source text with a configurable field, and cut one symbol's section out of a rendered document.

## `rust_graph_multi`
`fn rust_graph_multi(files: &[(&str, &str)]) -> Graph`
### Summary
A test helper that builds a graph from several Rust source files at once, with the links between the files resolved, so tests can check behavior that only exists between two distinct files, such as a reference written as a full `crate::` path.
### Behavior
Takes a list of path and source text pairs. For each it extracts the declarations with the Rust extractor and records them with a fingerprint of the text, then builds the graph from all of them. Separately it reads each file's imports with the Rust import reader and puts them in a map by path. It then resolves all those imports against the built graph, using a placeholder repo root since this resolver reads the root only for Cargo manifests and none exist in these in-memory fixtures, and stores the resolved links on the graph before returning it. It is the multi-file version of the single-file helper in the same test file. It cannot fail itself, though a bad fixture simply yields fewer links.
### Depends on
- `src/graph.rs::FileExtraction` — codeowl::graph
- `src/graph.rs::Graph` — codeowl::graph
- `src/hash.rs::hash_text` — codeowl::hash
- `src/imports.rs::FileImports` — codeowl::imports

## `rust_graph`
`fn rust_graph(rel_path: &str, src: &str) -> Graph`
### Summary
A test helper that builds a graph from one Rust source file, including the links resolved after the graph exists, so tests can check how a single file is described and how its parts depend on each other.
### Behavior
Extracts the file's declarations with the Rust extractor and builds a graph from that one file with a fingerprint of the text. It then reads the file's imports and resolves them against the built graph, using a placeholder repo root since none is needed, and stores the resolved links on the graph. This second step mirrors what the real index does and is essential: links are only resolved once the graph exists, never at extraction time, and the staleness checks read the graph's links, so skipping it would leave every dependency invisible. The resolver also adds the links between a trait implementation and its type when both sit in the same file. It returns the graph and cannot fail itself.
### Depends on
- `src/graph.rs::FileExtraction` — codeowl::graph
- `src/graph.rs::Graph` — codeowl::graph
- `src/hash.rs::hash_text` — codeowl::hash

## `section`
`fn section<'a>(rendered: &'a str, name: &str) -> &'a str`
### Summary
A test helper that cuts one symbol's block out of a rendered spec document, so a test can check what is in that symbol's section, including its Depends-on list, without being confused by neighbouring sections.
### Behavior
Takes the whole rendered text and a symbol name, and looks for the heading ``## `name` ``. If it is not found it panics, printing the full rendered text to help debugging. The block starts at that heading and ends just before the next `## ` heading that starts a line, or at the end of the text if there is none. The search for the next heading begins after the current heading's length, so it cannot match itself. The returned slice borrows from the input and includes everything the renderer wrote for that symbol: its signature, summary, behavior and dependency list.
### Depends on
- (none)

## `inherent_impl_is_folded_but_trait_impl_stays_its_own_section`
`fn inherent_impl_is_folded_but_trait_impl_stays_its_own_section()`
### Summary
A test that checks a Rust type's plain `impl` block is folded into the type's own section, while a trait implementation keeps its own section, so the file reads as the type, the trait implementation, and a free function.
### Behavior
Writes a sample `counter.rs` into a temporary repo, builds its graph, and runs the generate loop until it returns nothing, answering each symbol task with canned text long enough to pass the quality check and the file task with a one-line summary. It asserts that the symbols offered were exactly, in order, `Counter`, `impl fmt::Display for Counter` and `reset`, so the plain `impl Counter` block was never offered as its own task.

It then reads the saved spec back and renders it. It asserts the output has a section for `Counter`, no section for `impl Counter`, a section for the `Display` trait implementation, a section for `reset`, and exactly three top-level sections in total. It removes the temporary folder at the end. The test pins the Rust-specific rule that a type's methods are documented with the type, but "how the type satisfies a trait" is a separate concern with its own section.
### Depends on
- `src/spec.rs::SpecTask` — codeowl::spec
- `src/spec.rs::next_task` — codeowl::spec
- `src/spec.rs::read_file_spec` — codeowl::spec
- `src/spec.rs::render` — codeowl::spec
- `src/spec.rs::submit` — codeowl::spec

## `a_zero_field_single_trait_impl_marker_type_folds_into_one_document`
`fn a_zero_field_single_trait_impl_marker_type_folds_into_one_document()`
### Summary
A test that checks the narrow special case where a Rust type with no fields and a single trait implementation is documented as one section, with the trait implementation folded into the type, while the trait itself keeps its own section.
### Behavior
Writes a sample `greeter.rs` with a trait `Greeter` and a marker type `EnglishGreeter`, which has no fields and one `impl Greeter for EnglishGreeter`. It builds the graph and runs the generate loop until nothing is left, answering each symbol task with canned text long enough to pass the quality check. It asserts that the symbols offered were exactly `Greeter` and `EnglishGreeter`, in that order, so the trait implementation was not offered as its own task, because it was folded into the type the same way a plain `impl` block already is.

It then renders the saved spec and asserts: a section exists for `EnglishGreeter`, no section exists for `impl Greeter for EnglishGreeter`, and there are exactly two top-level sections, the untouched trait and the folded type. It removes the temporary folder at the end. This matters because with no fields and one implementation a separate document for the bare type would have nothing real to say, and nothing would mark it stale when the implementation's behavior changed.
### Depends on
- `src/spec.rs::SpecTask` — codeowl::spec
- `src/spec.rs::next_task` — codeowl::spec
- `src/spec.rs::read_file_spec` — codeowl::spec
- `src/spec.rs::render` — codeowl::spec
- `src/spec.rs::submit` — codeowl::spec

## `personal_greeter_src`
`fn personal_greeter_src(field_decl: &str) -> String`
### Summary
A test helper that produces the text of a small Rust file with a trait `Greeter`, a struct `PersonalGreeter` holding one field, and a trait implementation that reads that field. The caller chooses how the field is declared, so a test can change the field and see what goes stale.
### Behavior
Returns a string built with a template. The file has a `pub trait Greeter` with one method `greet`, a `pub struct PersonalGreeter` whose body is the text given as `field_decl`, and `impl Greeter for PersonalGreeter`, whose `greet` method formats `Hello, {}!` using `self.name`. The field text is inserted as written, so a test can pass `name: String` for the original, and another value, such as a different type or name, to simulate an edit to the type's fields. Because the trait implementation reads `self.name`, a change to that field should make the implementation's description stale, which is what the tests using this helper check. It only builds text and cannot fail.
### Depends on
- (none)

## `a_types_own_field_change_invalidates_its_trait_impls_document_without_folding`
`fn a_types_own_field_change_invalidates_its_trait_impls_document_without_folding()`
### Summary
A test that checks that when a Rust type gains a public field, the description of a trait implementation in the same file is marked out of date, even though the implementation's own text did not change, and that the two descriptions are still kept as separate sections.
### Behavior
First version: writes a sample file with a trait `Greeter`, a struct `PersonalGreeter` with one field, and an `impl Greeter for PersonalGreeter` that reads that field. It builds the graph and runs the generate loop to completion with canned text. It asserts the symbols offered were `Greeter`, `PersonalGreeter` and the trait implementation, so with a real field the type and its implementation are separate tasks, and that the rendered document has exactly three top-level sections.

It then checks two properties of the generated dependency lists. The type's own section must say "Depends on" nothing, least of all itself: the link from the implementation to its type has the type's own name as the imported name, and the type's declaration necessarily contains that name, so without an explicit self-exclusion the type's scan would match its own link, and this is a regression guard for that. The implementation's section must show a dependency on the type with the specifier `same file`, not a malformed line from an empty specifier.

Second version: it adds a second public field and asserts, as a sanity check, that the implementation's header line is unchanged. It rebuilds the graph and runs the loop again, recording what is offered. It asserts the type is regenerated because its own text changed, that the trait implementation is regenerated too because the type's shape moved underneath it, and that the unrelated trait `Greeter` is not regenerated, which shows the staleness is precise and not a blanket refresh. Finally it renders again and asserts there are still exactly three sections, showing the staleness link never merges them. It removes the temporary folder at the end.
### Depends on
- `src/spec.rs::SpecTask` — codeowl::spec
- `src/spec.rs::next_task` — codeowl::spec
- `src/spec.rs::read_file_spec` — codeowl::spec
- `src/spec.rs::render` — codeowl::spec
- `src/spec.rs::submit` — codeowl::spec

## `target_src`
`fn target_src(field_decl: &str) -> String`
### Summary
A test helper that produces the text of a tiny Rust file holding one struct named `Target`, with a body the caller chooses, so a test can change the struct's field and see what goes stale in another file.
### Behavior
Returns a string with `pub struct Target {` followed by the given field text on its own indented line and a closing brace. The field text is inserted as written, so a test can pass one declaration for the original and a different one to simulate an edit. It is the type-defining half of a two-file test, where another file reaches `Target` only through a fully-written `crate::` path, with no import. It only builds text and cannot fail.
### Depends on
- (none)

## `a_cross_file_fully_qualified_call_gets_a_real_dependency_edge_that_invalidates_on_change`
`fn a_cross_file_fully_qualified_call_gets_a_real_dependency_edge_that_invalidates_on_change()`
### Summary
A test that checks a Rust function reaching a type in another file through a fully-written `crate::` path, with no `use` line, is recorded as depending on that type, and that its description is marked out of date when the type changes shape even though the function's own text did not.
### Behavior
Writes two files into a temporary repo: `target.rs`, with a struct `Target` holding one public field, and `caller.rs`, whose function `read` refers to `Target` by its full path. It builds a two-file graph with the links resolved, then runs the generate loop on both files to completion with canned text.

First it renders the caller's spec and looks at the `read` section, asserting it lists a dependency on `src/target.rs::Target` with the real module path `crate::target` as its specifier, and not the `same file` placeholder used for same-file links.

Then it adds a second public field to `Target` and rebuilds the graph. The function `read` and its file are unchanged. It runs the loop again, recording what is offered, and asserts that `Target` is regenerated because its own text changed and, crucially, that `read` is regenerated too, because the shape of what it depends on moved underneath it. Without the link, this would be the silent miss where a change cannot reach a dependent that names the type only by its full path. It removes the temporary folder at the end.
### Depends on
- `src/spec.rs::SpecTask` — codeowl::spec
- `src/spec.rs::next_task` — codeowl::spec
- `src/spec.rs::read_file_spec` — codeowl::spec
- `src/spec.rs::render` — codeowl::spec
- `src/spec.rs::submit` — codeowl::spec
