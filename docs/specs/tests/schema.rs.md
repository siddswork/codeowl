---
kind: file
source_paths: [tests/schema.rs]
file: { source_hash: 0cf2625a31d748f035d7263d1f10052f267049ed3271ca5ba5a757396ab5c9b5, deps_hash: 1df444dd72a2653710a1e710b05bfc39ccb25af714e552f0cda93375b9bcc0ea, spec_hash: 4eea2a084db22b4ed1aa6c2ceabd810e5d3c446f5aaf5b0a531d0b8fa3980d3b }
symbols:
  tests/schema.rs::tempdir: { source_hash: 70c64ecd3572b78d6213eef75fdb27b2c9f0b3e0cdd3e93f81f071d561b784f2, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 9bf9c3dbe23e638b6015029627be81bfbeb798aec8fc6b3110dbbdeb9824f867 }
  tests/schema.rs::build: { source_hash: 86ff9744510723f37521d8ef779fd2604f0a5bead18f9405a0d34124ae51f821, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: 4f14be46bf3ba310279863f38b63caacc93ae265fbcff4f3e17ccf157dba35a3 }
  tests/schema.rs::schema_nodes_are_created_for_every_create_table: { source_hash: 90e5cb31a4c0b2c46b8e9bb946b80ef3ae6dfce80218d8a3550e41a400c73120, deps_hash: f36b3c08581642a4028cf368f11a0643244626ccd02b3eb0c9cfbe5b6b2067b8, spec_hash: 9d14df319218e95998cb2426cb782a79d6e72eef6c9564b8cfcd14984cb5c5ce }
  tests/schema.rs::a_from_call_resolves_to_its_table_node: { source_hash: f6684d75017fcefb70df3ab1da17f1c92555da8f29a3b2a8fba5230a9f0a695d, deps_hash: f36b3c08581642a4028cf368f11a0643244626ccd02b3eb0c9cfbe5b6b2067b8, spec_hash: d840f0fca322764c0f660247e9e87151ebc936b10b18957fbdaf6be6b47654ca }
  tests/schema.rs::get_callers_on_a_table_lists_the_app_code_that_touches_it: { source_hash: e33bc7d86fff8c64f5e467e8f8807850aea2d4794d01852fc24633fd7e7ca07a, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c0975b12e59fbc338066a0d5897a725de3dc7b5a741a3c4cdbc17fdfc0077617 }
  tests/schema.rs::a_features_data_participants_are_the_tables_its_core_code_queries: { source_hash: 907fdb14054ce6de0cc22a64bf4577ab43d053bc94b1a39916dff98d7783296e, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 91089de16d8f0352e406c9cf8b6f7fde21db08eabe0fb53f19016a2a3b74f3b8 }
---
# tests/schema.rs
## Summary
Tests that check the database side of CodeOwl: that a `.sql` schema file yields a node for each table, and that a `.from("table")` query in application code is linked to the right one. The sample project mirrors a real pilot repo: a database-dump style schema with schema-qualified table names such as `public.payments`, a table that has a constraint, and trailing `ALTER TABLE` statements the reader must not trip over, plus an API route that queries two tables through a Supabase `.from()` chain and imports a small helper. Four tests build that project in a temporary folder. The first checks both tables become table nodes whose signatures list their columns. The second checks the route's `.from("payments")` resolves to the `payments` node. The third checks asking "who uses this table" for `registrations` returns exactly the route. The fourth checks that the route's feature lists exactly the two tables it queries as data participants, in order, and that the writing task given to the agent describes `payments` with its real columns, so a feature description can name them instead of guessing from query code. Helpers create a collision-proof temporary folder and build the graph from the sample files.

## `tempdir`
`fn tempdir(tag: &str) -> std::path::PathBuf`
### Summary
A test helper that creates a fresh, empty temporary folder for one test to use, with a name that cannot collide with another test's, even when tests run at the same time.
### Behavior
Builds a folder name from the prefix `codeowl-m10-`, the caller's tag, the process id, and a counter that increases on every call, under the system temporary directory. Tests run concurrently in one process, so the shared counter is what keeps two tests from sharing a folder even with the same tag. It creates the folder and any missing parents and returns its path. It panics if creation fails, which is fine in test code, and it does not delete the folder afterwards.
### Depends on
- externals: std

## `build`
`fn build(dir: &std::path::Path) -> codeowl::Graph`
### Summary
A test helper that writes a small project with a SQL schema, an API route that queries a table, and a database helper into a folder, then builds the full graph for it, so the schema tests have a realistic project to examine.
### Behavior
Creates three subfolders in the given folder: `supabase`, `app/api/pay` and `lib`. It writes the schema text to `supabase/schema.sql`, the route text to `app/api/pay/route.ts`, and a one-line helper exporting `getSupabase` to `lib/supabase.ts`. The schema and route contents come from constants defined elsewhere in the test file. It then builds the index for the folder and rebuilds the graph from it, which parses the tables, resolves imports and resolves the links from queries to tables as in normal use, and returns the graph. It panics on any failure, since a broken fixture should stop the test.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `schema_nodes_are_created_for_every_create_table`
`fn schema_nodes_are_created_for_every_create_table()`
### Summary
A test that checks every `CREATE TABLE` in the sample schema file becomes a table node in the graph, with its columns listed in its signature.
### Behavior
Builds the sample project in a fresh temporary folder. For each of the two expected tables, `payments` and `registrations`, it looks up the node `supabase/schema.sql::<table>`, failing with "no schema node for <table>" if it is absent. It then asserts the node's kind is `Schema`, its raw kind is `table`, and its signature contains the column name `status`, failing with the actual signature if not, which shows the column list was captured. The fixture is a database dump that includes a constraint the SQL grammar cannot handle, so the test also confirms the plain-text fallback keeps the table set complete.
### Depends on
- `src/symbol.rs::SymbolKind` — codeowl::symbol

## `a_from_call_resolves_to_its_table_node`
`fn a_from_call_resolves_to_its_table_node()`
### Summary
A test that checks a `.from("payments")` query in an API route is linked to the `payments` table node in the schema file.
### Behavior
Builds the sample project and looks up the `payments` table node. It collects the targets of every table-reference link that starts in `app/api/pay/route.ts`, keeping only the ones that resolved to a node and taking each node's id. It asserts that the list contains `supabase/schema.sql::payments`, failing with the actual list if not, and that the node's kind is `Schema`. This shows the full chain: a query string in application code is recognised, matched by name to a table parsed from the schema file, and recorded as a link, which is what lets a feature description name the table and its real columns.
### Depends on
- `src/symbol.rs::SymbolKind` — codeowl::symbol

## `get_callers_on_a_table_lists_the_app_code_that_touches_it`
`fn get_callers_on_a_table_lists_the_app_code_that_touches_it()`
### Summary
A test that checks asking "who uses this table" for the `registrations` table returns the API route that queries it, which is how the callers tool answers for a table.
### Behavior
Builds the sample project, then asks the graph for the table's callers by the id `supabase/schema.sql::registrations`, keeps only the file part of each result, and asserts the list is exactly `["app/api/pay/route.ts"]`, so that file and no other. This checks the table-side lookup of the links created from query strings: the route's query on `registrations` is recorded, and a table nothing else touches shows only its real user. The same lookup backs the callers tool when it is given a table node.
### Depends on
- (none)

## `a_features_data_participants_are_the_tables_its_core_code_queries`
`fn a_features_data_participants_are_the_tables_its_core_code_queries()`
### Summary
A test that checks a feature's list of database tables contains exactly the tables its own code queries, and that the task handed to the agent describes each with its real columns.
### Behavior
Builds the sample project and finds the feature entry point for the payments API route, which counts as its own feature because no page refers to it. It assembles the feature's participants and asserts the data list is exactly `supabase/schema.sql::payments` followed by `supabase/schema.sql::registrations`, in that order, since the route queries both. It then asks for the feature's writing task, expecting one because no spec exists yet, and finds the `payments` entry in the task's data list. It asserts that entry's text contains the column name `amount`, failing with the actual text if not, which shows the "Data touched" part of a feature description is grounded in the table's real columns and not guessed from query code.
### Depends on
- (none)
