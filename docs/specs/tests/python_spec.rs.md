---
kind: file
source_paths: [tests/python_spec.rs]
file: { source_hash: cefb22a3522e1f44e37876f9d5b3bd5d88b154d8514b287a36b404001f3422bb, deps_hash: 79c4a84bd83881d6d3e203ca569b918810f2a977acfa4f1d805692aee2c48a84, spec_hash: 9e7e355bef8bf11ec6c9ec35a600f1656146f4ad4b04e8a8ff1d159c77669cf4 }
symbols:
  tests/python_spec.rs::py_graph: { source_hash: 91eb3160a68365c26780e123ad2e7ca5b3a771503ba4bba83e24fa2757a6c7ec, deps_hash: 5586d60160ea05dea2f3ff8ad9971353ca672d465b93cc63fce7decdb63dffaf, spec_hash: 84ed0065400b13a2f4032e73265aad35c4c70cc5b97a41b6dc34004c5ab558db }
  tests/python_spec.rs::a_class_renders_as_one_section_with_its_methods_folded_in: { source_hash: ec73b60918a626f6db40d634de5da281e44af63a7c2ca5be090b3ddfa6f70067, deps_hash: 479097bfebd5c3e7d47ff551cd258b8de6b674e73b8c589a4c15c5fb5036c394, spec_hash: 0fab453a23c55e0cd37fba67357af529c5066d8473cd67c31b0c67ea2e1499dd }
  tests/python_spec.rs::a_from_import_resolves_end_to_end_through_repo_index: { source_hash: d5f49648168bdcc7fd4fc7a7f22b5b8ca40c6c6120f02dbabd95b6f64714b440, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 3ddd65ef88005df8299b6f6bbf1bfe021433e5eaf81e7d4c1e1bfaacbb5b2de2 }
  tests/python_spec.rs::a_sqlmodel_table_class_becomes_a_schema_node_with_its_queriers: { source_hash: f0471133a458999139dbfef627c45812de2568f4e6d113269024499b67266e6e, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 77c539183e61632afbecb5dda6b34a1c98436f0b499f4b0fc9d67c582070c139 }
  tests/python_spec.rs::a_fastapi_route_becomes_a_feature_with_its_table_in_data: { source_hash: 9f7997d4c5642834671e451f133b425d8f803500cbbe224050949f426e6d7440, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 341915628bc125e5de5b1e8f03d5b96caea5f64dd21c04e4fb8269ddd69ec4f4 }
  tests/python_spec.rs::a_route_that_imports_modules_assembles_without_crashing: { source_hash: 4657a8c94bd27799fd177d7d71bcb7953e4c56a57dfc253d7fae75c539086cc7, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: ed306f6757eda4bd91513dfa0f35a9431f9448c46eecf830d6962158d4339304 }
  tests/python_spec.rs::colliding_route_slugs_are_disambiguated_not_dropped: { source_hash: 39cbc0cc9f1d2b9414e93f9ec1079aaca5456076ee00eafc2db8d43c0d73774d, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 6141b459a2d98a843793cdff7f0d2b5276973b5cd35bdc30767e43c685be9ada }
  tests/python_spec.rs::a_named_import_of_a_crud_function_promotes_its_file_to_core_like_a_module_import_does: { source_hash: ee83b5396f6c1d6aac758840c733954f6817314ca09569e4d9c4a469998619db, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 8bcd9daa0973e16ca1fc2ef1a360baeba1b863e9b4ef628be05204560da3c423 }
  tests/python_spec.rs::a_named_import_of_a_table_symbol_still_lands_in_data_not_core: { source_hash: baffff698dc7d30de33cbd7bcc0ddccfdfd2190402f9b1f1a9109cb28076e041, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: c99ce4919b08f93db29a0f6b10c060fe1ae63b348e4e76cb44fd029087f26ec7 }
  tests/python_spec.rs::a_module_import_of_a_schema_only_file_still_lands_in_data_not_core: { source_hash: cfaee6ebcd1a7098044508de1723dbc08c48a75cc05d6e5038b778654c3a0709, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 45ee2e34cb540c6576448b0e289b100500f76380187162267004ddcae8812651 }
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
  tests/python_spec.rs::py_graph -> src/graph.rs::FileExtraction: 0cb1c67d8a8b
  tests/python_spec.rs::py_graph -> src/graph.rs::Graph: 13dbdfa88b0e
  tests/python_spec.rs::py_graph -> src/hash.rs::hash_text: 70c099c1622c
  tests/python_spec.rs::a_class_renders_as_one_section_with_its_methods_folded_in -> src/spec.rs::SpecTask: 1f6b409c41df
  tests/python_spec.rs::a_class_renders_as_one_section_with_its_methods_folded_in -> src/spec.rs::next_task: 9d894695159c
  tests/python_spec.rs::a_class_renders_as_one_section_with_its_methods_folded_in -> src/spec.rs::read_file_spec: 6682201bb38f
  tests/python_spec.rs::a_class_renders_as_one_section_with_its_methods_folded_in -> src/spec.rs::render: bed79b32a9ec
  tests/python_spec.rs::a_class_renders_as_one_section_with_its_methods_folded_in -> src/spec.rs::submit: 6b3606c2f3b8
  tests/python_spec.rs::a_from_import_resolves_end_to_end_through_repo_index -> src/index.rs::RepoIndex: 481419547059
  tests/python_spec.rs::a_sqlmodel_table_class_becomes_a_schema_node_with_its_queriers -> src/index.rs::RepoIndex: 481419547059
  tests/python_spec.rs::a_fastapi_route_becomes_a_feature_with_its_table_in_data -> src/index.rs::RepoIndex: 481419547059
  tests/python_spec.rs::a_route_that_imports_modules_assembles_without_crashing -> src/index.rs::RepoIndex: 481419547059
  tests/python_spec.rs::colliding_route_slugs_are_disambiguated_not_dropped -> src/index.rs::RepoIndex: 481419547059
  tests/python_spec.rs::a_named_import_of_a_crud_function_promotes_its_file_to_core_like_a_module_import_does -> src/index.rs::RepoIndex: 481419547059
  tests/python_spec.rs::a_named_import_of_a_table_symbol_still_lands_in_data_not_core -> src/index.rs::RepoIndex: 481419547059
  tests/python_spec.rs::a_module_import_of_a_schema_only_file_still_lands_in_data_not_core -> src/index.rs::RepoIndex: 481419547059
---
# tests/python_spec.rs
## Summary
End-to-end tests for Python and FastAPI support, following the same path the real spec-writing command drives. They check that a Python class goes through extraction, the graph, the writing loop and rendering as one section with its methods folded in and never offered as separate tasks, and that a private method is not exported. They check that a `from app.models import Item` line resolves to the right class through the normal index, that a model class declared with `table=True` is recognised as a database table while its sibling request and response classes are not, and that the files using the table are found through ordinary imports with no extra link type. They check that a FastAPI route becomes a feature with its full address, including the router's prefix, and that its table lands in the data list and its shared dependency file in the core. Several tests are regressions for bugs found on real projects or in review: assembling a route that imports whole modules no longer fails; two routes with the same verb and path get distinct ids and neither is dropped; a data-access function imported by name pulls its file into the core the same as a module import does; and a table, or a schema-only file imported as a module, stays in the data list and never gets promoted into the core. Sample files are written to temporary folders, each test removing its own.

## `py_graph`
`fn py_graph(rel_path: &str, src: &str) -> Graph`
### Summary
A test helper that turns one piece of Python source text into a small graph, so the tests can look at how a single Python file is described without building a whole repo.
### Behavior
Takes a file path and the file's text. It extracts the declarations with the Python extractor, wraps them in a per-file extraction record together with a fingerprint of the text, and builds a graph from that one record. There is no import resolution and no other file, so the graph holds just that file's node and its symbols. It cannot fail itself, since the extractor returns an empty list for text it cannot parse.
### Depends on
- `src/graph.rs::FileExtraction` — codeowl::graph
- `src/graph.rs::Graph` — codeowl::graph
- `src/hash.rs::hash_text` — codeowl::hash

## `a_class_renders_as_one_section_with_its_methods_folded_in`
`fn a_class_renders_as_one_section_with_its_methods_folded_in()`
### Summary
A test that checks a Python class is documented as a single section with its methods folded in, that the writing loop asks only for the class and the module-level function, and that a method whose name starts with an underscore is not treated as exported.
### Behavior
Writes a sample routes file into a temporary repo, containing a class `ItemService` with a public method `read_all` and a private `_row_count`, a module-level function `get_current_user`, and a router assignment. It builds the graph and first asserts that `_row_count` is not exported, though it stays in the graph.

It then runs the generate loop up to 20 times, answering each task with canned text long enough to pass the quality check. For the class task it also asserts the class's docstring, "Reads and writes items for the current user.", is carried into the task. It asserts the symbols offered were exactly the class and the function, so neither the methods nor the router assignment, which is a value folded into the file, were offered.

It then renders the saved spec and asserts: a section exists for `ItemService`, no top-level section exists for `read_all` under either name, and there are exactly two top-level sections, the class and the function. It removes the temporary folder at the end.
### Depends on
- `src/spec.rs::SpecTask` — codeowl::spec
- `src/spec.rs::next_task` — codeowl::spec
- `src/spec.rs::read_file_spec` — codeowl::spec
- `src/spec.rs::render` — codeowl::spec
- `src/spec.rs::submit` — codeowl::spec

## `a_from_import_resolves_end_to_end_through_repo_index`
`fn a_from_import_resolves_end_to_end_through_repo_index()`
### Summary
An end-to-end test that checks a Python `from app.models import Item` line is linked to the `Item` class it names, going all the way from files on disk through the normal index.
### Behavior
Creates a temporary project with a model file, `app/models.py`, holding a class `Item` that is a database table, a routes file, `app/api/routes/items.py`, that imports `Item` from `app.models` and uses it in a function, and a `pyproject.toml`, which is harmless if language detection only counts `.py` files. It opens the repo through the normal index, which detects the Python pack, walks the files and builds the graph with its links.

It then finds the edge from `items.py` for the name `Item`, failing with "items.py imports Item from app.models" if absent, and asserts it resolved to a target whose id is `app/models.py::Item`. It deletes the temporary folder at the end. The test shows the dotted module path being turned into the model file and the class found in it, not only the resolver in isolation.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_sqlmodel_table_class_becomes_a_schema_node_with_its_queriers`
`fn a_sqlmodel_table_class_becomes_a_schema_node_with_its_queriers()`
### Summary
An end-to-end test that checks a Python model class marked as a database table is recognised as a table node, that its sibling request and response classes are not, and that the files using the table are found with no extra mechanism.
### Behavior
Creates a temporary project with a models file containing three classes: `ItemBase`, a shared base; `Item`, which extends it and is declared with `table=True`; and `ItemCreate`, another subclass of the base. Two other files, a routes file and a data-access file, each import `Item` from the models and use it. It opens the repo through the normal index.

It asserts that `Item` has the table kind, and that `ItemBase` and `ItemCreate` do not, since they are request and response shapes and not tables. It then collects every file whose resolved import targets the `Item` node and asserts both the routes file and the data-access file are in that list. Because the table node is the target of ordinary resolved imports, asking who uses the table works with no separate link type, unlike the TypeScript pack, which needs a query-string link. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_fastapi_route_becomes_a_feature_with_its_table_in_data`
`fn a_fastapi_route_becomes_a_feature_with_its_table_in_data()`
### Summary
An end-to-end test that checks a FastAPI route is recognised as a feature, with the route's full address, and that the database table it uses and the shared dependency file it relies on end up in that feature.
### Behavior
Creates a temporary project with a models file containing a table class `Item` and a plain response class `ItemPublic`; a shared dependencies file `deps.py` with a session function and a `SessionDep` alias; and a routes file with a router whose prefix is `/items`, and a `GET /{id}` route whose parameter is typed `SessionDep` and which reads an `Item`. It opens the repo through the normal index and asks for the feature model, which must exist for Python.

It finds the entry point with the id `http-get-items-id` and asserts its kind is `http`, its title is `GET /items/{id}`, which shows the router's prefix was applied to the route's path, and its file is the routes file. It then assembles the feature's participants and asserts that the data list contains `app/models.py::Item`, the table the route queries, and that the core list contains `app/api/deps.py`, which joins because it touches a table through `SessionDep`. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_route_that_imports_modules_assembles_without_crashing`
`fn a_route_that_imports_modules_assembles_without_crashing()`
### Summary
A regression test for a bug found on a real project: a Python route that imports whole modules (`from app import crud`) used to make feature assembly fail with a "not a symbol" error. The test checks it now assembles, with the data-access module included and an unrelated utility module left out.
### Behavior
Creates a temporary project with package marker files, a table model, a `crud.py` module that uses the model, a `security.py` utility module with a plain function, and a routes file whose `POST /items/` route imports both modules whole and calls into each. It opens the repo through the normal index and finds the entry point `http-post-items`.

It assembles the feature's participants and then calls `current_participant_hashes`, expecting success. This is the bug: a whole-module import resolves to a file node and used to leak into the dependencies as a bare file id, which the hash function rejected as "not a symbol". It then asserts that `crud.py`, a data-access module reached by a module import, joined the core; that `security.py`, a module import that is neither data access nor touches a table, appears in neither the core nor the dependencies, since a bare file id must never be a dependency participant. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `colliding_route_slugs_are_disambiguated_not_dropped`
`fn colliding_route_slugs_are_disambiguated_not_dropped()`
### Summary
A regression test for a bug found in code review: two different FastAPI routes that happen to produce the same short name used to collapse into one, so the second silently vanished from coverage and generation. The test checks both now survive with distinct ids.
### Behavior
Creates a temporary project with two routes files, `a_router.py` and `b_router.py`. Each defines a router with no distinguishing prefix and a `GET /health` route, in functions named `health_a` and `health_b`. Both produce the same name from verb and path. The old code sorted entry points by that name and removed duplicates, so one disappeared without any error, even though a comment claimed names were unique. It opens the repo through the normal index and lists the entry points.

It keeps those whose id starts with `http-get-health` and asserts there are exactly two, not collapsed to one. It asserts the set of their files contains both routes files, and that the set of their ids has two members, meaning the two entries have distinct ids. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_named_import_of_a_crud_function_promotes_its_file_to_core_like_a_module_import_does`
`fn a_named_import_of_a_crud_function_promotes_its_file_to_core_like_a_module_import_does()`
### Summary
A regression test that checks writing a dependency as `from app.crud import create_item` gives the same result as `from app import crud`: the data-access file joins the feature's core in both cases, and the file is not also listed as a separate dependency.
### Behavior
Before the fix, the feature walk only offered a file to the "belongs inside the feature" decision when the link was to a whole file, such as a module import. A link to one named function went straight into the dependencies list, so the same dependency written two ways ended up in two different tiers. This asymmetry had already been fixed once for the data tier, and a language with no whole-module imports at all, such as Java, where every injection names a specific class, needed the fix to be generic. The fixture happens to be Python only because it is the simplest pack to reproduce the shape.

It creates a temporary project with a table model, a `crud.py` that uses it, and a routes file with a `POST /items` route whose only import of the data-access code is the named `from app.crud import create_item`. It opens the repo, finds the entry point `http-post-items`, and assembles the feature's participants. It asserts that `app/crud.py` is in the core, and that no dependency starts with `app/crud.py::`, because a symbol whose own file is already core must not also appear as a one-hop stub. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_named_import_of_a_table_symbol_still_lands_in_data_not_core`
`fn a_named_import_of_a_table_symbol_still_lands_in_data_not_core()`
### Summary
A guard test that checks importing a database table class by name still puts the table in the feature's data list, and does not pull the whole models file into the feature's core. It protects against a risk introduced by the previous fix.
### Behavior
That fix lets a named import make its file join the core. If it also applied to a table, the models file would be listed both as core, as a file, and as data, through the table symbol, which is double counting, and would also hide the table from the data list. So a table's own file must never be promoted just because it was imported.

It creates a temporary project with a table model and a routes file whose `GET /items` route imports `Item` by name and queries it. It opens the repo, finds the entry point `http-get-items`, and assembles the feature's participants. It asserts that `app/models.py::Item` is in the data list, and that `app/models.py` is not in the core. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `a_module_import_of_a_schema_only_file_still_lands_in_data_not_core`
`fn a_module_import_of_a_schema_only_file_still_lands_in_data_not_core()`
### Summary
A regression test for a bug found in code review: importing a models file as a whole module, `from app import models`, pulled that file into a feature's core, which then silently hid its table from the feature's data list when another file imported the table by name. The test checks the file stays out of the core and the table still appears as data.
### Behavior
The earlier guard only covered the path where a named import resolves to a symbol. The older path, where a module import resolves directly to a file node, had no check that the file declares tables, so a schema-only file imported by its module name still got in. Once it was in the core, a later rule that skips a symbol whose own file is already core dropped the table out of the data list for any other core file that imported the table by name. The same masking bug appears with a different import style.

It creates a temporary project with a table model, a data-access module that imports the table by name, and a routes file whose `POST /items` route imports the models module whole and imports `create_item` by name. It opens the repo, finds the entry point `http-post-items`, and assembles the feature's participants. It asserts that `app/models.py` is not in the core, module-import style included, and that `app/models.py::Item` is still in the data list, surfaced through the data-access module's named import of it. It deletes the temporary folder at the end.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index
