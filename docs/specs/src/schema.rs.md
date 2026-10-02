---
kind: file
source_paths: [src/schema.rs]
file: { source_hash: 0d9fb8dd4ba8bb517dc5766830b7ab3d9109addb64405150a32db02e400bdf90, deps_hash: 5d10d0e1282025a9b52b20ce70ba10ebd083b412cf48d4afd271d271f9b3a9ab, spec_hash: fe774de27dba288d8c428227aaa0d100bc46c034949797a70f936b9682710cb2 }
symbols:
  src/schema.rs::Table: { source_hash: 99a6815c88acf037a161e04393937b9a1b4c1af1b2c958f4909de4ef75416a4e, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3e1d055df01adea599ef6a08242efaf0464aaa0c90c4f767f53cc56587c6c558 }
  src/schema.rs::extract_tables: { source_hash: 0b826ecaef84c1a0014c55feb5933c776a199718a312c7ca9c0cb0b13fd782b0, deps_hash: 5d10d0e1282025a9b52b20ce70ba10ebd083b412cf48d4afd271d271f9b3a9ab, spec_hash: d19555c4cbff372b454ff7eb8c3e30086fbbc5ab7271fa150798998ea78a7ef2 }
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
This file reads database schema files. It parses the `CREATE TABLE` statements in a `.sql` file into table symbols, so that a `.from("table")` call in application code has a real table to link to, and so a feature description's "Data touched" section can name actual tables and columns. It is deliberately shallow: only `CREATE TABLE` is read, never views, functions, triggers or access-policy rules, and only column names are kept, not types or foreign keys. It works in two passes. A SQL grammar parses the statements for structure, taking each table's name and columns. A plain line scan of `CREATE TABLE` header lines then guarantees the set of tables is complete even when the grammar fails, which happens with some unusual constraint syntax in database dumps, where it drops the whole statement and loses that table's columns. A table found only by the scan has just its name and header line. A schema-qualified name such as `public.payments` is reduced to `payments`, which is what lets a query on `payments` match it, and quote marks around names are removed. Each table is recorded with a signature of the name plus its column list, and a fingerprint of its own lines, so adding, dropping or renaming a column marks any feature that touches the table as stale.

## `Table`
`struct Table`
### Summary
A scratch record of one database table found in a schema file: its name, its column names, and where it sits in the file. It is used while parsing and is turned into the table symbols the rest of CodeOwl sees.
### Behavior
`name` is the table's name, which may be schema-qualified as written, and `columns` is the list of column names. Only names are kept, not types or foreign keys. `lines` is a 1-based, inclusive pair of start and end lines, as far as it is known: the full statement span when the grammar parsed it, but only the header line when the table was found by the plain line-scan fallback, since that scan sees nothing beyond the `CREATE TABLE` line. It is private to the module and plain data with no logic.
### Depends on
- (none)

## `extract_tables`
`pub fn extract_tables(source: &str, rel_path: &str) -> Vec<ExtractedSymbol>`
### Summary
Reads a `.sql` file and returns one table symbol for each `CREATE TABLE` in it, with the table's column names, so that a `.from("payments")` query in application code has something to link to and a feature description can name real columns.
### Behavior
First it parses the file's table statements with `parse_tables`, a real SQL grammar. Then `backstop_from_headers`, a plain line scan of `CREATE TABLE` headers, adds any table the grammar missed, which can happen when it chokes on an unusual statement such as a `pg_dump` check constraint and drops the whole statement, so the set of tables is always complete.

Each table becomes a symbol of kind `Schema` with raw kind `table`, the id `<path>::<table name>`, and a signature that is the table name followed by its column list, such as `payments(id, amount, user_id)`. The line range comes from the parse, or only the header line for a table found by the fallback. `source_hash` hashes the table's own lines, so adding, dropping or renaming a column marks anything that depends on it stale. The table is not exported and has no public-surface hash, since there is no separate public shape to track and the source hash is the staleness key. It has no docstring, markers, parent or children. Only column names are recorded, not types or constraints. It cannot fail, and a file with no tables gives an empty list.
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
