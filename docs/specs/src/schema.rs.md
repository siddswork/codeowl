---
kind: file
source_paths: [src/schema.rs]
file: { source_hash: c34c1f94ae51e2c4d1bcd4b6797be073a26ba196a240d661e9de89b1189a88b0, deps_hash: d4034a9cbee7900a1fbe5e4401fc040618db110c2395b587568f12985ac649df, spec_hash: 93e68edf1f9574e108df2e4917292b84706b505f5f2b0693b37de0cdfc2beb9e }
symbols:
  src/schema.rs::Table: { source_hash: 0a79b1c50c0f737b984d05f06a58ecc8934822a7ec3f0cec9d1dd28d9abbb7f5, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: e04d6b2db72b776ba56b1331712332aa716bc867a0565031023716294c223d67 }
  src/schema.rs::extract_tables: { source_hash: d2c5790e88026e33641f1fd141cd3057c44ce543cf3656be36830d5c2834d9df, deps_hash: d4034a9cbee7900a1fbe5e4401fc040618db110c2395b587568f12985ac649df, spec_hash: 26710492f6d5b75a84c1fc74f62fd292c3ef820c70878eca607ed504eb4c33d3 }
  src/schema.rs::parse_tables: { source_hash: 7dd142db3d8994d80572549d9106e392efbb4ce379c72b4c58ee96a5815be54f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: d7172cad79fbaacccb7ca6b5aab4af1874c90d3782ab4d0c8f739f3189e46554 }
  src/schema.rs::table_from_node: { source_hash: 6918e83166852058c5919cc0adfb9053cd8bb244a9e685c5215530498f21ddde, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: bc69c5c794c6e190177ccfc10dadb8d1cc8277da1ef6ee85dd3d4e48e079d24b }
  src/schema.rs::unqualified_name: { source_hash: 74a28cd49d2c5f13012d57088d32c119add3acae2c125bafcf79545ddf1676e1, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 4e5b86a0dc3c4ff9d6fa7b5cd636d0e58a87be85b085da134a5e5a4adae6c4b4 }
  src/schema.rs::column_names: { source_hash: d1449b46c3a532d53937bb04af7d6a640aab488cdfa78c673bcde5624200306a, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 380807090ff4736badfd62de7a2699d8db2430901101226c1c4e87881dfe0fe8 }
  src/schema.rs::backstop_from_headers: { source_hash: 604c500af53d19d98cae1ad6e6e739db8b7e03cab66c7e606cc8fd5d85230dff, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: dd96cbe339a8385d8c0e3bd76e0b8d0d00d9d8f687f1a7f214082fba30373a00 }
  src/schema.rs::table_name_from_header: { source_hash: 9056e8383aaacef28d6c072b4025f4d87e16cc5023ba7de27d9f78139b3045f9, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 6945f9e66224f6e5ca68a2b0b6cbc0af8d7ae6952391a6a3a9cab615dad65e03 }
  src/schema.rs::text: { source_hash: f2f836c691a8002927cc493e38650603f8fc64232d33afeb1b8f41f9279d1f2f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: bd6a8d27f1d98b59abfe41397c3b67e213c0cd4fa813b50e1ced00660535a75f }
---
# src/schema.rs
## Summary
SQL schema extraction (M10) — the SQL pack's counterpart to `extract.rs`. Parses `CREATE TABLE` statements out of a `.sql` schema or migration file into `SymbolKind::Schema` table nodes, so a `.from("table")` call in application code has something to resolve to and a feature spec's "Data touched" section can name real tables. Deliberately shallow: only `CREATE TABLE` (no views, functions, triggers, RLS), column *names* only (no types or foreign keys). Two passes — `parse_tables` (tree-sitter-sequel) for structure, and `backstop_from_headers` (a plain line scan of `CREATE TABLE` headers) to guarantee the table *set* is complete even when the grammar chokes on pg_dump CHECK-constraint exotica and drops a whole statement node. `unqualified_name` is what makes `.from("payments")` match `CREATE TABLE public.payments`. A Phase-2 seam — its `.sql`-file assumption is generalised at M18 for annotation-based ORMs.

## `Table`
`struct Table`
### Summary
The private intermediate a `CREATE TABLE` parses to before becoming an `ExtractedSymbol` — its name, column names, and line span.
### Behavior
Plain data, not exported. `columns` is the column *names* only, no types (M10 scope). `lines` is 1-indexed inclusive but only as accurate as the parse path allowed — the tree-sitter path gives the full statement span, the line-scan fallback only knows the header line.
### Depends on
- (none)

## `extract_tables`
`pub fn extract_tables(source: &str, rel_path: &str) -> Vec<ExtractedSymbol>`
### Summary
The schema extractor — turns a `.sql` file's `CREATE TABLE`s into arena-ready symbols: one `SymbolKind::Schema` (raw `"table"`) per table, id `<file>::<table>`, signature `name(col, col, …)`. The `SourceKind::Schema` branch of `lang::extract_symbols`.
### Behavior
Runs `parse_tables` (tree-sitter) then `backstop_from_headers` — a line-scan pass that adds any `CREATE TABLE` the grammar missed. Each `Table` becomes an `ExtractedSymbol`: `signature` is the rendered column list, `source_hash` is a hash of the statement's line span, and `is_exported` / `interface_hash` / `docstring` / `markers` / `parent` / `children` are all empty — a table is resolvable and queryable (via `get_callers`) but never spec-bearing.
### Depends on
- `src/hash.rs::hash_text` — crate::hash
- `src/symbol.rs::ExtractedSymbol` — crate::symbol
- `src/symbol.rs::SymbolKind` — crate::symbol

## `parse_tables`
`fn parse_tables(source: &str) -> Vec<Table>`
### Summary
The tree-sitter pass — parses SQL with `tree-sitter-sequel` and collects a `Table` for every `create_table` node.
### Behavior
Iterative DFS over the tree (an explicit `stack`, not recursion). At a `create_table` node it calls `table_from_node` and does *not* descend further — a nested `CREATE TABLE` isn't a thing. Parse failure returns an empty vec, and `extract_tables`'s `backstop_from_headers` then picks up anything the grammar choked on.
### Depends on
- externals: tree_sitter

## `table_from_node`
`fn table_from_node(node: Node, source: &str) -> Option<Table>`
### Summary
Builds a `Table` from one `create_table` tree-sitter node — its name and column list.
### Behavior
Scans the node's children: an `object_reference` gives the name (via `unqualified_name`, which drops any `schema.` prefix), a `column_definitions` gives the columns (via `column_names`). Returns `None` only if there's no name — a table with no parseable columns still counts, it just has an empty list. `lines` is the full statement span, 1-indexed inclusive.
### Depends on
- externals: tree_sitter

## `unqualified_name`
`fn unqualified_name(node: Node, source: &str) -> String`
### Summary
The bare table name from a possibly-qualified `object_reference` — `public.payments` and `payments` both yield `payments`.
### Behavior
Takes the *last* `identifier` child (schema-qualified names put the schema first, the table last), stripping surrounding double quotes. This is why `.from("payments")` resolves to a `CREATE TABLE public.payments` — resolution compares unqualified names on both sides.
### Depends on
- externals: tree_sitter

## `column_names`
`fn column_names(defs: Node, source: &str) -> Vec<String>`
### Summary
The column names from a `column_definitions` node — the first identifier of each `column_definition`.
### Behavior
Iterates `column_definition` children, taking `named_child(0)` when it's an `identifier` (the column name comes before its type), quotes stripped. Skips non-column entries in the definitions list — a table-level `CONSTRAINT`, a `PRIMARY KEY (...)` clause — since those aren't `column_definition` nodes. Types, defaults, and constraints on the column are all discarded (M10 scope: names only).
### Depends on
- externals: tree_sitter

## `backstop_from_headers`
`fn backstop_from_headers(source: &str, tables: &mut Vec<Table>)`
### Summary
A line-scan fallback: adds any table whose `CREATE TABLE` header the tree-sitter grammar failed to turn into a node.
### Behavior
Scans every line for a `CREATE TABLE [IF NOT EXISTS] [schema.]name (` header (via `table_name_from_header`) and, if that name isn't already in `tables`, pushes a bare `Table` — name only, empty columns, `lines` just the header line. Exists because `tree-sitter-sequel` doesn't parse every Postgres dialect construct cleanly; this guarantees a `.from("that_table")` still resolves even when the full statement didn't parse. It only relies on pg_dump's convention of a single-line header.
### Depends on
- (none)

## `table_name_from_header`
`fn table_name_from_header(line: &str) -> Option<String>`
### Summary
Parses a table name out of a single `CREATE TABLE ...` header line — the string-matching companion to `backstop_from_headers`.
### Behavior
Requires a `CREATE TABLE ` / `create table ` prefix (case-sensitive on those two forms only), strips an optional `IF NOT EXISTS`, takes the first whitespace-delimited token, drops a trailing `(`, and takes the part after the last `.` (unqualifying `schema.name`), quotes stripped. Returns the name only if it's non-empty and all `[A-Za-z0-9_]` — so a header with an unusual identifier is skipped rather than misparsed.
### Depends on
- (none)

## `text`
`fn text<'a>(node: Node, source: &'a str) -> &'a str`
### Summary
A local helper: the source substring a tree-sitter node spans, `""` on error.
### Behavior
`node.utf8_text` against the source bytes. The same one-line wrapper `extract.rs`, `features.rs`, and `rust.rs` each define privately — kept per-module rather than sharing a tree-sitter utility surface.
### Depends on
- externals: tree_sitter
