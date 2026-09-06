//! The feature layer — M5. Entry-point enumeration, the route-literal
//! resolver, and participant-set assembly, per `ARCHITECTURE.md`'s
//! "Feature specs". This is the one place CodeOwl looks past declarations
//! and import statements into arbitrary call expressions — deliberately
//! narrow (only `fetch(...)` calls, only `/api/...` literals, only a
//! Next.js path-convention join), not general call-graph analysis, which
//! stays deferred past Phase 1 (see `ARCHITECTURE.md`'s open question 3).

use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser};

use crate::graph::{Graph, SymbolId};

/// One `fetch("/api/...")` call site found anywhere in a file — not just
/// top-level, since these calls are almost always inside event handlers or
/// effects. `static_path` has already had its query string stripped and is
/// guaranteed to start with `/api/`. Persisted on `Graph` (like
/// `ResolvedImport`) so `get_next_spec_task` doesn't need to re-walk every
/// file's syntax tree on every call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteLiteral {
    pub from_file: String,
    pub static_path: String,
}

/// Walk every node in `source` (unlike `extract_file`'s shallow top-level
/// walk — a `fetch` call can be arbitrarily nested) looking for
/// `fetch(<literal>)` call expressions.
pub fn extract_route_literals(source: &str, rel_path: &str) -> Vec<RouteLiteral> {
    let language = if rel_path.ends_with(".tsx") {
        tree_sitter_typescript::LANGUAGE_TSX
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT
    };

    let mut parser = Parser::new();
    parser
        .set_language(&language.into())
        .expect("bundled tree-sitter-typescript grammar should always load");

    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    walk_for_fetch(tree.root_node(), source, rel_path, &mut out);
    out
}

fn walk_for_fetch(node: Node, source: &str, rel_path: &str, out: &mut Vec<RouteLiteral>) {
    if node.kind() == "call_expression"
        && let Some(func) = node.child_by_field_name("function")
        && func.kind() == "identifier"
        && text(func, source) == "fetch"
        && let Some(args) = node.child_by_field_name("arguments")
        && let Some(first_arg) = args.named_child(0)
        && let Some(static_path) = static_path_from_literal(first_arg, source)
    {
        out.push(RouteLiteral {
            from_file: rel_path.to_string(),
            static_path,
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_for_fetch(child, source, rel_path, out);
    }
}

/// Pull a `/api/...` static path out of a plain string or a template
/// string's static prefix. `None` if the node isn't a string-shaped
/// literal, or if the path portion itself (before any `?` query string)
/// depends on an interpolated value — matching an incomplete path would
/// mean guessing, which this resolver deliberately never does (see the
/// module doc comment).
fn static_path_from_literal(node: Node, source: &str) -> Option<String> {
    match node.kind() {
        "string" => {
            let raw = text(node.named_child(0)?, source);
            path_from_raw(raw, true)
        }
        "template_string" => {
            // `.children()` includes anonymous tokens too (the backtick
            // punctuation) -- named children only, or "first" would often
            // be the opening backtick rather than the actual fragment.
            let mut cursor = node.walk();
            let mut children = node.children(&mut cursor).filter(Node::is_named);
            let first = children.next()?;
            if first.kind() != "string_fragment" {
                return None;
            }
            let raw = text(first, source);
            let has_substitution = children.next().is_some();
            // Safe to use even with a substitution later, as long as that
            // substitution falls inside the query string (after '?'), not
            // the path itself.
            let path_complete = !has_substitution || raw.contains('?');
            path_from_raw(raw, path_complete)
        }
        _ => None,
    }
}

fn path_from_raw(raw: &str, path_complete: bool) -> Option<String> {
    if !path_complete {
        return None;
    }
    let path_only = raw.split('?').next().unwrap_or(raw);
    path_only
        .starts_with("/api/")
        .then(|| path_only.to_string())
}

fn text<'a>(node: Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or_default()
}

/// Resolve a `/api/...` static path to the `app/api/.../route.ts` file it
/// names, by Next.js's directory convention — a path join, not fuzzy
/// matching. A candidate's dynamic segments (`[id]`, `[...slug]`) match any
/// corresponding literal segment; every other segment must match exactly,
/// and segment *counts* must match exactly too (no prefix matching — see
/// `static_path_from_literal`'s "path_complete" guard, which is what keeps
/// an incomplete path from ever reaching here).
pub fn resolve_route_literal(graph: &Graph, static_path: &str) -> Option<SymbolId> {
    let want: Vec<&str> = static_path
        .trim_start_matches("/api/")
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    let target = graph.files().find(|f| {
        api_route_segments(&f.id).is_some_and(|candidate| {
            candidate.len() == want.len()
                && candidate
                    .iter()
                    .zip(&want)
                    .all(|(c, w)| is_dynamic_segment(c) || c == w)
        })
    })?;
    graph.find(&target.id)
}

fn api_route_segments(file_id: &str) -> Option<Vec<&str>> {
    let inner = file_id
        .strip_prefix("app/api/")?
        .strip_suffix("/route.ts")?;
    Some(inner.split('/').collect())
}

fn is_dynamic_segment(seg: &str) -> bool {
    seg.starts_with('[') && seg.ends_with(']')
}

/// One framework-enumerated entry point — a page, or an API route no page
/// reaches via a resolved `fetch()` literal (a webhook, a cron target).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryPoint {
    pub file: String,
    pub slug: String,
}

fn is_page(file_id: &str) -> bool {
    file_id == "app/page.tsx" || (file_id.starts_with("app/") && file_id.ends_with("/page.tsx"))
}

fn is_api_route(file_id: &str) -> bool {
    file_id.starts_with("app/api/") && file_id.ends_with("/route.ts")
}

/// Every `app/**/page.tsx`, plus every `app/api/**/route.ts` that no
/// resolved route literal targets. `route_literals` should be every one
/// `extract_route_literals` found across the whole repo walk.
pub fn enumerate_entry_points(graph: &Graph, route_literals: &[RouteLiteral]) -> Vec<EntryPoint> {
    let targeted: HashSet<String> = route_literals
        .iter()
        .filter_map(|lit| resolve_route_literal(graph, &lit.static_path))
        .map(|id| graph.string_id(id).to_string())
        .collect();

    let mut entries: Vec<EntryPoint> = graph
        .files()
        .filter(|f| is_page(&f.id))
        .map(|f| EntryPoint {
            file: f.id.clone(),
            slug: feature_slug(&f.id),
        })
        .collect();

    entries.extend(
        graph
            .files()
            .filter(|f| is_api_route(&f.id) && !targeted.contains(&f.id))
            .map(|f| EntryPoint {
                file: f.id.clone(),
                slug: feature_slug(&f.id),
            }),
    );

    entries
}

/// A human-followable slug from a route path — `app/submit/page.tsx` ->
/// `submit`, `app/page.tsx` -> `home`, `app/api/stripe-webhook/route.ts`
/// -> `api-stripe-webhook`. Human-friendly titles live inside the document
/// itself; this is only ever a filename.
pub fn feature_slug(entry_file: &str) -> String {
    let stripped = entry_file
        .strip_prefix("app/")
        .unwrap_or(entry_file)
        .trim_end_matches("page.tsx")
        .trim_end_matches("route.ts")
        .trim_end_matches('/');
    if stripped.is_empty() {
        "home".to_string()
    } else {
        stripped.replace('/', "-")
    }
}

/// A feature's participant set, split by tier per `ARCHITECTURE.md`'s
/// example: `core` is the feature's own code (the entry point plus
/// whatever it reaches via route-literal edges — tracked by `source_hash`,
/// since a body change is a real change to the feature), `dependencies`
/// are the symbols that code imports directly (tracked by `interface_hash`
/// — only a public-surface change matters, same as any other reference
/// edge). Dependencies are one hop only, never expanded further: a feature
/// spec consumes what it depends on, it doesn't recursively pull in the
/// rest of the graph (see "Recursive spec generation"'s containment-only
/// invariant, which this mirrors for reference edges).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Participants {
    pub core: Vec<String>,
    pub dependencies: Vec<String>,
}

pub fn assemble_participants(
    graph: &Graph,
    route_literals: &[RouteLiteral],
    entry_file: &str,
) -> Participants {
    let mut core = Vec::new();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([entry_file.to_string()]);

    while let Some(file) = queue.pop_front() {
        if !seen.insert(file.clone()) {
            continue;
        }
        core.push(file.clone());
        for lit in route_literals.iter().filter(|l| l.from_file == file) {
            if let Some(target_id) = resolve_route_literal(graph, &lit.static_path) {
                let target_file = graph.string_id(target_id).to_string();
                if !seen.contains(&target_file) {
                    queue.push_back(target_file);
                }
            }
        }
    }

    let mut dependencies = Vec::new();
    let mut seen_deps = HashSet::new();
    for file in &core {
        for imp in graph.imports().iter().filter(|i| &i.from_file == file) {
            let Some(target) = imp.target else { continue };
            let id = graph.string_id(target).to_string();
            if !core.contains(&id) && seen_deps.insert(id.clone()) {
                dependencies.push(id);
            }
        }
    }

    Participants { core, dependencies }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_graph_from_sources;

    #[test]
    fn plain_string_fetch_is_extracted() {
        let src = "async function f() { await fetch(\"/api/submit-artwork\"); }\n";
        let lits = extract_route_literals(src, "a.tsx");
        assert_eq!(
            lits,
            vec![RouteLiteral {
                from_file: "a.tsx".to_string(),
                static_path: "/api/submit-artwork".to_string(),
            }]
        );
    }

    #[test]
    fn template_string_with_query_interpolation_is_extracted() {
        let src = "async function f() { await fetch(`/api/get-registration?code=${x}`); }\n";
        let lits = extract_route_literals(src, "a.tsx");
        assert_eq!(lits[0].static_path, "/api/get-registration");
    }

    #[test]
    fn template_string_with_path_interpolation_is_skipped() {
        let src = "async function f() { await fetch(`/api/judge/competitions/${id}`); }\n";
        let lits = extract_route_literals(src, "a.tsx");
        assert!(lits.is_empty());
    }

    #[test]
    fn non_fetch_calls_are_ignored() {
        let src = "async function f() { await axios.get(\"/api/whatever\"); }\n";
        let lits = extract_route_literals(src, "a.tsx");
        assert!(lits.is_empty());
    }

    #[test]
    fn nested_fetch_inside_a_callback_is_still_found() {
        let src = "function f() { useEffect(() => { fetch(\"/api/x\"); }, []); }\n";
        let lits = extract_route_literals(src, "a.tsx");
        assert_eq!(lits[0].static_path, "/api/x");
    }

    #[test]
    fn resolve_matches_a_static_route() {
        let graph = build_graph_from_sources(&[(
            "app/api/submit-artwork/route.ts",
            "export async function POST(): Promise<void> {}\n",
        )]);
        let id = resolve_route_literal(&graph, "/api/submit-artwork");
        assert_eq!(
            id.map(|i| graph.string_id(i).to_string()),
            Some("app/api/submit-artwork/route.ts".to_string())
        );
    }

    #[test]
    fn resolve_matches_through_a_dynamic_segment() {
        let graph = build_graph_from_sources(&[(
            "app/api/competitions/[id]/route.ts",
            "export async function GET(): Promise<void> {}\n",
        )]);
        let id = resolve_route_literal(&graph, "/api/competitions/123");
        assert!(id.is_some());
    }

    #[test]
    fn resolve_returns_none_for_an_unknown_route() {
        let graph = build_graph_from_sources(&[("a.ts", "export const x = 1;\n")]);
        assert_eq!(resolve_route_literal(&graph, "/api/nope"), None);
    }

    #[test]
    fn feature_slug_examples() {
        assert_eq!(feature_slug("app/submit/page.tsx"), "submit");
        assert_eq!(feature_slug("app/page.tsx"), "home");
        assert_eq!(
            feature_slug("app/api/stripe-webhook/route.ts"),
            "api-stripe-webhook"
        );
    }

    #[test]
    fn enumerate_lists_pages_and_only_orphan_routes() {
        let graph = build_graph_from_sources(&[
            (
                "app/submit/page.tsx",
                "export default function Page() { fetch(\"/api/submit-artwork\"); return null; }\n",
            ),
            (
                "app/api/submit-artwork/route.ts",
                "export async function POST(): Promise<void> {}\n",
            ),
            (
                "app/api/stripe-webhook/route.ts",
                "export async function POST(): Promise<void> {}\n",
            ),
        ]);
        let lits = extract_route_literals(
            "export default function Page() { fetch(\"/api/submit-artwork\"); return null; }\n",
            "app/submit/page.tsx",
        );
        let entries = enumerate_entry_points(&graph, &lits);
        let files: Vec<&str> = entries.iter().map(|e| e.file.as_str()).collect();
        assert!(files.contains(&"app/submit/page.tsx"));
        assert!(files.contains(&"app/api/stripe-webhook/route.ts"));
        assert!(!files.contains(&"app/api/submit-artwork/route.ts"));
    }

    #[test]
    fn assemble_participants_follows_route_literal_then_collects_direct_imports() {
        let files: &[(&str, &str)] = &[
            (
                "app/submit/page.tsx",
                "import { getSupabase } from '../../lib/supabase';\nexport default function Page() { fetch(\"/api/submit-artwork\"); getSupabase(); return null; }\n",
            ),
            (
                "app/api/submit-artwork/route.ts",
                "import { getSupabase } from '../../../lib/supabase';\nexport async function POST(): Promise<void> { getSupabase(); }\n",
            ),
            (
                "lib/supabase.ts",
                "export function getSupabase(): void {}\n",
            ),
        ];

        let dir =
            std::env::temp_dir().join(format!("codeowl-features-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut extractions = Vec::new();
        let mut file_imports = std::collections::HashMap::new();
        let mut route_literals = Vec::new();
        for (rel, content) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            extractions.push(crate::graph::extract_and_hash(rel, content));
            file_imports.insert(
                rel.to_string(),
                crate::imports::extract_imports(content, rel),
            );
            route_literals.extend(extract_route_literals(content, rel));
        }
        let mut graph = Graph::build(extractions);
        let resolver = crate::resolve::build_resolver();
        let resolved = crate::resolve::resolve_imports(&dir, &resolver, &file_imports, &graph);
        graph.set_resolved_imports(resolved);

        let participants = assemble_participants(&graph, &route_literals, "app/submit/page.tsx");
        assert_eq!(
            participants.core,
            vec![
                "app/submit/page.tsx".to_string(),
                "app/api/submit-artwork/route.ts".to_string()
            ]
        );
        assert_eq!(
            participants.dependencies,
            vec!["lib/supabase.ts::getSupabase".to_string()]
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
