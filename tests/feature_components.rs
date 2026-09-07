//! M11: a feature's `core` follows the client-component subtree the entry
//! point *renders* (`<Client/>` → `<Form/>`), not only the `fetch()` calls
//! it makes directly. Without this, a feature whose page is a thin server
//! wrapper around a client component produces a spec written from the
//! wrapper, with the actual flow (state, the fetch calls, validation)
//! invisible — the blind spot found generating the pilot's evaluation
//! feature specs.

use std::sync::atomic::{AtomicU32, Ordering};

use codeowl::features::assemble_participants;
use codeowl::index::RepoIndex;

fn tempdir(tag: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("codeowl-fc-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// A thin server wrapper — renders a co-located client component, no fetch
// of its own.
const PAGE: &str = r#"
// Default imports and default exports — the shape React actually uses.
import EvaluationClient from "./evaluation-client";
import Button from "../../../components/ui/button";
export default function Page() {
  return <><EvaluationClient /><Button /></>;
}
"#;

// Co-located with the page. Renders the real form, which lives elsewhere.
const CLIENT: &str = r#"
import EvaluationForm from "../../../components/judge/evaluation-form";
export default function EvaluationClient() {
  return <EvaluationForm />;
}
"#;

// NOT co-located with the page, but it does the data work.
const FORM: &str = r#"
export default function EvaluationForm() {
  async function submit() {
    await fetch("/api/judge/evaluations/submit");
  }
  return <button onClick={submit}>Submit</button>;
}
"#;

const ROUTE: &str = "export async function POST(): Promise<void> {}\n";

// A plain presentational component the page also renders — no data work,
// not co-located. Must stay out of `core`.
const BUTTON: &str = "export default function Button() { return null; }\n";

fn build(dir: &std::path::Path) -> codeowl::Graph {
    for (rel, src) in [
        ("app/judge/evaluate/page.tsx", PAGE),
        ("app/judge/evaluate/evaluation-client.tsx", CLIENT),
        ("components/judge/evaluation-form.tsx", FORM),
        ("components/ui/button.tsx", BUTTON),
        ("app/api/judge/evaluations/submit/route.ts", ROUTE),
    ] {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, src).unwrap();
    }
    RepoIndex::build(dir).unwrap().rebuild().unwrap()
}

#[test]
fn core_follows_the_rendered_component_subtree_into_its_fetches() {
    let dir = tempdir("subtree");
    let graph = build(&dir);

    let core = assemble_participants(&graph, "app/judge/evaluate/page.tsx").core;

    for expected in [
        "app/judge/evaluate/page.tsx",               // the entry point
        "app/judge/evaluate/evaluation-client.tsx",  // co-located component
        "components/judge/evaluation-form.tsx",      // not co-located, but has a fetch
        "app/api/judge/evaluations/submit/route.ts", // reached via that fetch
    ] {
        assert!(
            core.contains(&expected.to_string()),
            "core should contain {expected}, got {core:?}"
        );
    }
    assert!(
        !core.contains(&"components/ui/button.tsx".to_string()),
        "a presentational component with no data work must stay out of core"
    );
}
