//! M10 validation: a `.sql` schema file produces schema nodes for its
//! tables, and a `.from("table")` call in application code resolves to the
//! right one (`ROADMAP.md`'s M10 validation).
//!
//! The fixture mirrors the pilot repo's pg_dump-style `supabase/schema.sql`
//! — schema-qualified table names (`public.payments`), columns, and
//! trailing `ALTER TABLE ... ADD CONSTRAINT` statements the extractor must
//! not trip over — plus a Supabase `.from()` call chain in an API route.

use std::sync::atomic::{AtomicU32, Ordering};

use codeowl::index::RepoIndex;
use codeowl::symbol::SymbolKind;

fn tempdir(tag: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("codeowl-m10-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const SCHEMA_SQL: &str = r#"
--
-- PostgreSQL database dump
--

CREATE TABLE public.registrations (
    id integer NOT NULL,
    competition_id integer NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL
);

CREATE TABLE public.payments (
    id integer NOT NULL,
    registration_id integer NOT NULL,
    razorpay_order_id text NOT NULL,
    status text DEFAULT 'created'::text NOT NULL,
    amount integer NOT NULL,
    CONSTRAINT payments_amount_positive CHECK ((amount > 0))
);

ALTER TABLE ONLY public.payments
    ADD CONSTRAINT payments_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.payments
    ADD CONSTRAINT payments_registration_id_fkey FOREIGN KEY (registration_id) REFERENCES public.registrations(id);
"#;

const ROUTE_TS: &str = r#"
import { getSupabase } from "@/lib/supabase";

export async function POST(req: Request) {
  const supabase = getSupabase();
  const { data } = await supabase.from("payments").select("*").eq("status", "paid");
  await supabase.from("registrations").update({ status: "confirmed" }).eq("id", data.registration_id);
  return Response.json({ ok: true });
}
"#;

fn build(dir: &std::path::Path) -> codeowl::Graph {
    std::fs::create_dir_all(dir.join("supabase")).unwrap();
    std::fs::create_dir_all(dir.join("app/api/pay")).unwrap();
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    std::fs::write(dir.join("supabase/schema.sql"), SCHEMA_SQL).unwrap();
    std::fs::write(dir.join("app/api/pay/route.ts"), ROUTE_TS).unwrap();
    std::fs::write(
        dir.join("lib/supabase.ts"),
        "export function getSupabase() {}\n",
    )
    .unwrap();
    RepoIndex::build(dir).unwrap().rebuild().unwrap()
}

#[test]
fn schema_nodes_are_created_for_every_create_table() {
    let dir = tempdir("tables");
    let graph = build(&dir);

    for table in ["payments", "registrations"] {
        let id = graph
            .find(&format!("supabase/schema.sql::{table}"))
            .unwrap_or_else(|| panic!("no schema node for {table}"));
        let sym = graph.get_symbol(id).unwrap();
        assert_eq!(sym.kind, SymbolKind::Table);
        assert!(
            sym.signature.contains("status"),
            "{table} signature should list its columns, got {:?}",
            sym.signature
        );
    }
}

#[test]
fn a_from_call_resolves_to_its_table_node() {
    let dir = tempdir("from");
    let graph = build(&dir);

    let payments = graph.find("supabase/schema.sql::payments").unwrap();
    let touched: Vec<&str> = graph
        .table_refs()
        .iter()
        .filter(|r| r.from_file == "app/api/pay/route.ts")
        .filter_map(|r| codeowl::features::resolve_table_ref(&graph, &r.table))
        .map(|id| graph.string_id(id))
        .collect();

    assert!(
        touched.contains(&"supabase/schema.sql::payments"),
        "the route's .from(\"payments\") should resolve to the payments node, got {touched:?}"
    );
    assert_eq!(graph.get_symbol(payments).unwrap().kind, SymbolKind::Table);
}

#[test]
fn get_callers_on_a_table_lists_the_app_code_that_touches_it() {
    let dir = tempdir("callers");
    let graph = build(&dir);

    let callers: Vec<String> = graph
        .table_callers("supabase/schema.sql::registrations")
        .into_iter()
        .map(|c| c.from_file)
        .collect();

    assert_eq!(callers, vec!["app/api/pay/route.ts".to_string()]);
}

#[test]
fn a_features_data_participants_are_the_tables_its_core_code_queries() {
    let dir = tempdir("participants");
    let graph = build(&dir);

    // The pay route is an orphan API route -> its own feature entry point.
    let participants = codeowl::features::assemble_participants(&graph, "app/api/pay/route.ts");

    assert_eq!(
        participants.data,
        vec![
            "supabase/schema.sql::payments".to_string(),
            "supabase/schema.sql::registrations".to_string(),
        ],
        "both .from() targets should be data participants"
    );

    // And the feature task hands the agent each table's column list.
    let entry = codeowl::features::enumerate_entry_points(&graph)
        .into_iter()
        .find(|e| e.file == "app/api/pay/route.ts")
        .unwrap();
    let task = codeowl::spec::next_feature_task(&graph, &dir, &entry)
        .unwrap()
        .expect("payments feature needs a spec");
    let payments = task
        .data
        .iter()
        .find(|(id, _)| id == "supabase/schema.sql::payments")
        .unwrap();
    assert!(payments.1.contains("amount"), "got {:?}", payments.1);
}
