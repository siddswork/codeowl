//! SQL schema extraction (M10) — the SQL pack's counterpart to
//! `extract.rs`. Parses `CREATE TABLE` statements out of a `.sql`
//! schema/migration file into table nodes, so a `.from("table")` call in
//! application code has something to resolve to and a feature spec's "Data
//! touched" section can name real tables.
//!
//! **SQL pack — Phase 2 seam here** (companion to the TypeScript + Next.js
//! pack; see `ROADMAP.md`'s "Stack modularization"). Deliberately shallow,
//! like `extract.rs`: only `CREATE TABLE` — never views, functions,
//! triggers, or RLS policies. Foreign keys and column types are out of
//! scope for M10.
//!
//! Columns are best-effort: `tree-sitter-sequel` chokes on some pg_dump
//! CHECK-constraint exotica (`((col)::text = ANY (...))`) and drops the
//! whole `create_table` node, so a plain line scan over `CREATE TABLE`
//! headers backstops the table *set* even when the grammar loses a table's
//! structure.

use tree_sitter::{Node, Parser};

use crate::hash::hash_text;
use crate::symbol::{ExtractedSymbol, SymbolKind};

struct Table {
    name: String,
    columns: Vec<String>,
    /// 1-indexed inclusive line span, as far as it's known. The line-scan
    /// fallback only knows the header line.
    lines: [usize; 2],
}

/// Turn a `.sql` file's `CREATE TABLE`s into arena-ready symbols: one
/// `SymbolKind::Table` per table, id `<rel_path>::<table>`, signature the
/// table name plus its column list.
pub fn extract_tables(source: &str, rel_path: &str) -> Vec<ExtractedSymbol> {
    let mut tables = parse_tables(source);
    backstop_from_headers(source, &mut tables);

    tables
        .into_iter()
        .map(|t| {
            let signature = format!("{}({})", t.name, t.columns.join(", "));
            let span = source
                .lines()
                .skip(t.lines[0].saturating_sub(1))
                .take(t.lines[1] + 1 - t.lines[0])
                .collect::<Vec<_>>()
                .join("\n");
            ExtractedSymbol {
                id: format!("{rel_path}::{}", t.name),
                kind: SymbolKind::Table,
                file: rel_path.to_string(),
                lines: t.lines,
                source_hash: hash_text(&span),
                signature,
                docstring: None,
                is_exported: false,
                interface_hash: None,
                markers: Vec::new(),
                parent: None,
                children: Vec::new(),
            }
        })
        .collect()
}

fn parse_tables(source: &str) -> Vec<Table> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_sequel::LANGUAGE.into())
        .expect("bundled tree-sitter-sequel grammar should always load");
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let root = tree.root_node();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "create_table" {
            if let Some(t) = table_from_node(node, source) {
                out.push(t);
            }
            continue;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    out
}

fn table_from_node(node: Node, source: &str) -> Option<Table> {
    let mut name = None;
    let mut columns = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "object_reference" => name = Some(unqualified_name(child, source)),
            "column_definitions" => columns = column_names(child, source),
            _ => {}
        }
    }
    Some(Table {
        name: name?,
        columns,
        lines: [node.start_position().row + 1, node.end_position().row + 1],
    })
}

/// The last `identifier` in an `object_reference` — `public.payments` and a
/// bare `payments` both yield `payments`.
fn unqualified_name(node: Node, source: &str) -> String {
    let mut last = String::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            last = text(child, source).trim_matches('"').to_string();
        }
    }
    last
}

fn column_names(defs: Node, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = defs.walk();
    for child in defs.children(&mut cursor) {
        if child.kind() == "column_definition"
            && let Some(first) = child.named_child(0)
            && first.kind() == "identifier"
        {
            out.push(text(first, source).trim_matches('"').to_string());
        }
    }
    out
}

/// Add any table whose `CREATE TABLE` header the grammar didn't turn into a
/// node (see the module doc). pg_dump always writes the header on one line:
/// `CREATE TABLE [IF NOT EXISTS] [schema.]name (`.
fn backstop_from_headers(source: &str, tables: &mut Vec<Table>) {
    for (i, line) in source.lines().enumerate() {
        let Some(name) = table_name_from_header(line) else {
            continue;
        };
        if !tables.iter().any(|t| t.name == name) {
            tables.push(Table {
                name,
                columns: Vec::new(),
                lines: [i + 1, i + 1],
            });
        }
    }
}

fn table_name_from_header(line: &str) -> Option<String> {
    let rest = line
        .trim()
        .strip_prefix("CREATE TABLE ")
        .or_else(|| line.trim().strip_prefix("create table "))?;
    let rest = rest
        .trim_start_matches("IF NOT EXISTS ")
        .trim_start_matches("if not exists ");
    let token = rest.split_whitespace().next()?.trim_end_matches('(');
    let name = token.rsplit('.').next()?.trim_matches('"');
    (!name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .then(|| name.to_string())
}

fn text<'a>(node: Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = r#"
CREATE TABLE public.payments (
    id integer NOT NULL,
    registration_id integer NOT NULL,
    status text DEFAULT 'created'::text NOT NULL,
    CONSTRAINT payments_status_check CHECK (((status)::text = ANY (ARRAY['created'::text])))
);

ALTER TABLE ONLY public.payments
    ADD CONSTRAINT payments_pkey PRIMARY KEY (id);

CREATE TABLE public.registrations (
    id integer NOT NULL,
    competition_id integer NOT NULL
);
"#;

    #[test]
    fn extracts_a_table_per_create_statement() {
        let syms = extract_tables(DUMP, "supabase/schema.sql");
        let names: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert!(names.contains(&"supabase/schema.sql::payments"));
        assert!(names.contains(&"supabase/schema.sql::registrations"));
        assert!(syms.iter().all(|s| s.kind == SymbolKind::Table));
    }

    #[test]
    fn signature_lists_columns_parsed_before_a_broken_constraint() {
        let syms = extract_tables(DUMP, "s.sql");
        let payments = syms.iter().find(|s| s.id == "s.sql::payments").unwrap();
        assert!(payments.signature.contains("id"));
        assert!(payments.signature.contains("registration_id"));
        assert!(payments.signature.contains("status"));
    }

    #[test]
    fn header_scan_backstops_a_table_the_grammar_drops() {
        // A CHECK clause the grammar can't handle at all, losing the node.
        let sql = "CREATE TABLE public.queue (\n  id integer,\n  status varchar(20) DEFAULT 'x'::character varying,\n  CONSTRAINT c CHECK (((status)::text = ANY ((ARRAY['a'::character varying])::text[])))\n);\n";
        let syms = extract_tables(sql, "s.sql");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].id, "s.sql::queue");
    }

    #[test]
    fn ignores_views_and_other_ddl() {
        let sql = "CREATE VIEW v AS SELECT 1;\nCREATE INDEX i ON t (c);\n";
        assert!(extract_tables(sql, "s.sql").is_empty());
    }
}
