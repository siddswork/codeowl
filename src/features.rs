//! The feature layer — M5, extended in M10 (`.from("table")`) and M11
//! (rendered-component `core` expansion). Entry-point enumeration, the
//! route-literal resolver, and participant-set assembly, per
//! `ARCHITECTURE.md`'s "Feature specs". This is where CodeOwl looks past
//! declarations and import statements into call expressions and JSX —
//! deliberately narrow (`fetch("/api/...")` literals, `.from("table")`
//! calls, and `<Component/>` tags matched to imports), not general
//! call-graph analysis, which stays deferred past Phase 1 (see
//! `ARCHITECTURE.md`'s open question 3).
//!
//! **TypeScript + Next.js pack — Phase 2 seam here.** This layer is the
//! most framework-coupled of all: the `fetch("/api/...")` convention and
//! the Next.js App Router path join are specific to this one stack. A
//! different framework needs a different entry-point + route resolver. See
//! `ROADMAP.md`'s "Stack modularization".
//!
//! M13 split it in two: [`assemble_participants`] is now a generic graph
//! walk (flow edges + one-hop imports), and every Next-specific judgment —
//! `is_page`, the route-path slug, the `is_colocated || does_data_work`
//! admission rule — lives behind [`trait FeatureModel`](FeatureModel),
//! implemented once here by [`TypeScriptNextFeatureModel`]. A stack with
//! no runtime entry surface returns `None` from
//! [`crate::stack::StackPack::feature_model`] and has no feature layer at
//! all. The `extract_*` / `resolve_*` free functions below stay `pub` as
//! shims for `TypeScriptNextStack` and the tests.

use std::collections::{BTreeSet, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use tree_sitter::Node;

use crate::graph::{FlowTarget, Graph, SymbolId};
use crate::lang::ts_parser;

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
    let mut parser = ts_parser(rel_path);

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

/// One `.from("<table>")` call site — the Supabase/PostgREST convention for
/// naming the table a query runs against (M10). Stored on `Graph` like
/// `RouteLiteral`, resolved against the schema nodes `schema.rs` extracts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRef {
    pub from_file: String,
    pub table: String,
}

/// Every `.from("<table>")` member call in `source`. Deliberately narrow,
/// like `extract_route_literals`: the argument must be a single plain
/// string literal shaped like a table name (`[a-z_][a-z0-9_]*`), which
/// filters out `Array.from(...)` / `Buffer.from(...)` and any dynamic
/// `.from(tableVar)`. A fuzzier ORM-aware matcher (`db.payments`,
/// `Payment.findMany`) stays deferred — see `ROADMAP.md`'s M10 scope.
pub fn extract_table_refs(source: &str, rel_path: &str) -> Vec<TableRef> {
    let mut parser = ts_parser(rel_path);
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk_for_from(tree.root_node(), source, rel_path, &mut out);
    out
}

fn walk_for_from(node: Node, source: &str, rel_path: &str, out: &mut Vec<TableRef>) {
    if node.kind() == "call_expression"
        && let Some(func) = node.child_by_field_name("function")
        && func.kind() == "member_expression"
        && func
            .child_by_field_name("property")
            .is_some_and(|p| text(p, source) == "from")
        && !is_builtin_from_receiver(func, source)
        && let Some(args) = node.child_by_field_name("arguments")
        && let Some(first) = args.named_child(0)
        && args.named_child(1).is_none()
        && first.kind() == "string"
        && let Some(table) = first.named_child(0).map(|frag| text(frag, source))
        && is_table_name(table)
    {
        out.push(TableRef {
            from_file: rel_path.to_string(),
            table: table.to_string(),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_for_from(child, source, rel_path, out);
    }
}

fn is_table_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `Array.from(...)` / `Buffer.from(...)` / `Object.from...` — a `.from` on
/// a global constructor, not a query builder. A Supabase client is always
/// a lowercase-initial variable (`supabase`, `db`) or a call expression, so
/// an uppercase-initial identifier receiver is the tell.
fn is_builtin_from_receiver(member: Node, source: &str) -> bool {
    member
        .child_by_field_name("object")
        .filter(|o| o.kind() == "identifier")
        .and_then(|o| text(o, source).chars().next())
        .is_some_and(|c| c.is_ascii_uppercase())
}

/// Resolve a table name to its schema node, by exact name match against the
/// `SymbolKind::Schema` symbols `schema.rs` produced. `None` for a name with
/// no `CREATE TABLE` (a database view, or a typo) — the "resolves to a
/// plausible candidate" bar from `ROADMAP.md`'s M10 validation, not a
/// guarantee.
pub fn resolve_table_ref(graph: &Graph, table: &str) -> Option<SymbolId> {
    graph
        .symbols()
        .find(|s| s.kind == crate::symbol::SymbolKind::Schema && table_name_of(&s.id) == table)
        .and_then(|s| graph.find(&s.id))
}

fn table_name_of(symbol_id: &str) -> &str {
    symbol_id.rsplit("::").next().unwrap_or(symbol_id)
}

/// One `<Component/>` an entry point (or another core file) renders — a
/// containment edge the import graph can't distinguish from importing a
/// utility. `assemble_participants` follows these into `core` so a feature
/// spec is written from the code that does the work, not the thin server
/// wrapper that renders it (M11).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RenderedComponent {
    pub from_file: String,
    /// The JSX tag as written — an uppercase-initial identifier
    /// (`<Foo/>`), matched against `from_file`'s imports at assemble time.
    pub name: String,
}

/// Every distinct `<Component/>` rendered anywhere in `source` — an
/// uppercase-initial JSX tag (lowercase tags are DOM elements). `<Foo.Bar/>`
/// namespaced tags are skipped for now.
pub fn extract_rendered_components(source: &str, rel_path: &str) -> Vec<RenderedComponent> {
    let mut parser = ts_parser(rel_path);
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let mut names: BTreeSet<String> = BTreeSet::new();
    walk_for_jsx(tree.root_node(), source, &mut names);
    names
        .into_iter()
        .map(|name| RenderedComponent {
            from_file: rel_path.to_string(),
            name,
        })
        .collect()
}

fn walk_for_jsx(node: Node, source: &str, out: &mut BTreeSet<String>) {
    if matches!(
        node.kind(),
        "jsx_opening_element" | "jsx_self_closing_element"
    ) {
        let mut cursor = node.walk();
        if let Some(tag) = node
            .children(&mut cursor)
            .find(|c| c.kind() == "identifier")
        {
            let name = text(tag, source);
            if name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                out.insert(name.to_string());
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_for_jsx(child, source, out);
    }
}

/// The repo-relative file a `<Name/>` rendered in `from_file` resolves to.
/// `None` when the component's import resolved outside the repo (an
/// external UI library) or not at all. Checks default imports first
/// (`import Name from './x'` — how React components are almost always
/// imported), then falls back to a named import. `pub` so
/// `TypeScriptNextStack::resolve_flow_edge` can call it.
pub fn resolve_rendered_component(graph: &Graph, from_file: &str, name: &str) -> Option<String> {
    if let Some(di) = graph
        .resolved_default_imports()
        .iter()
        .find(|di| di.from_file == from_file && di.local_name == name)
    {
        return Some(di.target_file.clone());
    }
    let target = graph
        .imports()
        .iter()
        .find(|imp| imp.from_file == from_file && imp.imported_name == name)?
        .target?;
    let file_id = graph.parent_id(target)?;
    Some(graph.string_id(file_id).to_string())
}

/// A rendered component joins a feature's `core` iff it does data work
/// (contains a `fetch("/api/...")` or `.from("table")` flow edge, resolved
/// or not) — i.e. it's part of the feature's behavior, not just its chrome.
fn file_does_data_work(graph: &Graph, file: &str) -> bool {
    graph
        .flow_edges()
        .iter()
        .any(|e| e.from_file == file && matches!(e.kind.as_str(), "route-literal" | "table-ref"))
}

/// ...or it's co-located with the entry point — in the entry's own
/// directory or a subdirectory of it (`app/submit/artwork-uploader.tsx`
/// for `app/submit/page.tsx`). Shared primitives under `components/ui/`
/// are neither, so they stay one-hop stub dependencies.
fn is_colocated(component_file: &str, entry_file: &str) -> bool {
    let entry_dir = entry_file.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    !entry_dir.is_empty() && component_file.starts_with(&format!("{entry_dir}/"))
}

/// One framework-enumerated entry point. `kind` names the framework
/// convention it came from — `"page"` / `"api-route"` for Next.js; M17
/// (Quarkus) adds `"http"`, `"kafka"`, … so `enumerate_entry_points` can
/// return a heterogeneous list. `id` is the unique slug that names the
/// spec file (`docs/specs/_features/<id>.md`) and must not collide across
/// kinds — Next.js pages and API routes come from disjoint path spaces so
/// `feature_slug` alone suffices here; M17 kind-prefixes. `title` is the
/// human-facing label rendered inside the document (nothing reads it yet
/// — it's here so the shape is right before a second `FeatureModel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryPoint {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub file: String,
}

/// The pack-owned model of "what is a feature" for a stack that has one.
/// A stack with no runtime entry surface (a CLI, a utility library)
/// returns `None` from [`crate::stack::StackPack::feature_model`] and
/// never constructs this — its corpus is symbol / file / rollup / system
/// specs alone (see `ROADMAP.md` M13 piece 3 and
/// `experiments/exp-02-feature-layer.md`).
///
/// Two methods, deliberately (M13 design decision 8 — keep this minimal;
/// it gets its second implementation only at M17). Entry-point
/// enumeration and `core` admission are the only judgments the otherwise
/// generic walk in [`assemble_participants`] can't make for itself: where
/// a flow edge points is [`crate::stack::StackPack::resolve_flow_edge`],
/// and whether the file it lands on belongs *inside* the feature (a body
/// change is a feature change) or stays a one-hop dependency is
/// [`FeatureModel::admits_to_core`].
pub trait FeatureModel: Send + Sync {
    /// Every entry point the framework's file/annotation conventions
    /// expose in `graph`.
    fn enumerate_entry_points(&self, graph: &Graph) -> Vec<EntryPoint>;

    /// Does `candidate_file` — reached from `entry` by a resolved flow
    /// edge into a file node — belong in the feature's `core` (tracked by
    /// `source_hash`), or is it a one-hop dependency (`interface_hash`)?
    fn admits_to_core(&self, graph: &Graph, entry: &EntryPoint, candidate_file: &str) -> bool;
}

/// The TypeScript + Next.js App Router feature model: a feature is an
/// `app/**/page.tsx` (or an orphan `app/api/**/route.ts`) plus the
/// component subtree and API routes it reaches. This is the whole of the
/// Next-specific "feature" concept — `is_page`, the route-path slug, and
/// the `is_colocated || does_data_work` admission rule all live here, not
/// in the generic walk.
#[derive(Debug, Default, Clone, Copy)]
pub struct TypeScriptNextFeatureModel;

impl FeatureModel for TypeScriptNextFeatureModel {
    fn enumerate_entry_points(&self, graph: &Graph) -> Vec<EntryPoint> {
        let targeted: HashSet<String> = graph
            .flow_edges()
            .iter()
            .filter(|e| e.kind == "route-literal")
            .filter_map(|e| match e.target {
                FlowTarget::Node(id) => Some(graph.string_id(id).to_string()),
                FlowTarget::Unresolved => None,
            })
            .collect();

        let mut entries: Vec<EntryPoint> = graph
            .files()
            .filter(|f| is_page(&f.id))
            .map(|f| EntryPoint {
                kind: "page".to_string(),
                id: feature_slug(&f.id),
                title: entry_route_path(&f.id),
                file: f.id.clone(),
            })
            .collect();

        entries.extend(
            graph
                .files()
                .filter(|f| is_api_route(&f.id) && !targeted.contains(&f.id))
                .map(|f| EntryPoint {
                    kind: "api-route".to_string(),
                    id: feature_slug(&f.id),
                    title: entry_route_path(&f.id),
                    file: f.id.clone(),
                }),
        );

        entries
    }

    fn admits_to_core(&self, graph: &Graph, entry: &EntryPoint, candidate_file: &str) -> bool {
        // The generic walk asks this for *every* resolved file edge. Old
        // behavior: a `route-literal` target joined unconditionally, a
        // `rendered-component` target only if the feature's own code.
        // Route-literal targets are always `app/api/**/route.ts`, so
        // `is_api_route` reproduces the unconditional case; component
        // targets are `.tsx`, so they fall to the co-location / data-work
        // test exactly as before.
        is_api_route(candidate_file)
            || is_colocated(candidate_file, &entry.file)
            || file_does_data_work(graph, candidate_file)
    }
}

/// The TypeScript + Next feature model — the value behind
/// `TypeScriptNextStack::feature_model()`.
pub fn default_feature_model() -> &'static dyn FeatureModel {
    &TypeScriptNextFeatureModel
}

/// The feature model for the stack that built `graph`, or `None` if that
/// stack has none (M14 `RustStack`, M16 commons-lang). Recovered from
/// `graph.pack_name()` via [`crate::stack::for_name`] so a pure-read
/// caller doesn't need the pack threaded down. This is the M14 completion
/// of what M13 stubbed as `default_feature_model()`.
pub fn feature_model_for(graph: &Graph) -> Option<&'static dyn FeatureModel> {
    crate::stack::for_name(graph.pack_name()).feature_model()
}

/// The entry points for `graph`'s stack — empty for a stack with no
/// feature model.
pub fn enumerate_entry_points(graph: &Graph) -> Vec<EntryPoint> {
    feature_model_for(graph).map_or_else(Vec::new, |fm| fm.enumerate_entry_points(graph))
}

fn is_page(file_id: &str) -> bool {
    file_id == "app/page.tsx" || (file_id.starts_with("app/") && file_id.ends_with("/page.tsx"))
}

fn is_api_route(file_id: &str) -> bool {
    file_id.starts_with("app/api/") && file_id.ends_with("/route.ts")
}

/// The URL path an entry file serves — `app/submit/page.tsx` -> `/submit`,
/// `app/page.tsx` -> `/`, `app/api/stripe-webhook/route.ts` ->
/// `/api/stripe-webhook`. Only used for [`EntryPoint::title`].
fn entry_route_path(entry_file: &str) -> String {
    let path = entry_file
        .strip_prefix("app/")
        .unwrap_or(entry_file)
        .trim_end_matches("page.tsx")
        .trim_end_matches("route.ts")
        .trim_end_matches('/');
    format!("/{path}")
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
/// edge), and `data` are the SQL tables the core code queries via a
/// resolved `.from("table")` (M10 — tracked by `source_hash`, so a schema
/// change to a table the feature touches makes the feature spec stale).
/// Dependencies and data are one hop only, never expanded further: a
/// feature spec consumes what it depends on, it doesn't recursively pull
/// in the rest of the graph (see "Recursive spec generation"'s
/// containment-only invariant, which this mirrors for reference edges).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Participants {
    pub core: Vec<String>,
    pub dependencies: Vec<String>,
    pub data: Vec<String>,
}

/// Walk the feature rooted at `entry`, using `fm` for the two judgments
/// the walk can't make itself. Generic over the stack: it follows
/// `flow_edges` and one-hop imports off the graph and never names a
/// framework convention — `fm.admits_to_core` decides what joins `core`,
/// and edges that resolve to a *symbol* rather than a file (a SQL table)
/// are the `data` tier.
pub fn assemble_participants(
    graph: &Graph,
    fm: &dyn FeatureModel,
    entry: &EntryPoint,
) -> Participants {
    let mut core = Vec::new();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([entry.file.clone()]);

    while let Some(file) = queue.pop_front() {
        if !seen.insert(file.clone()) {
            continue;
        }
        core.push(file.clone());

        // Two ways a file can pull another *file* into `core`, both gated
        // by `fm.admits_to_core`:
        //   1. a flow edge into a file node (the pilot's route-literal /
        //      rendered-component edges; M17's `Depends()` / param-type
        //      edges), and
        //   2. an import that resolves to a file node — a module import
        //      like Python's `from app import crud` (M17; a named symbol
        //      import resolves to a symbol and is handled in the
        //      `dependencies` tier below, never here).
        let flow_targets = graph
            .flow_edges()
            .iter()
            .filter(|e| e.from_file == file)
            .filter_map(|e| match e.target {
                FlowTarget::Node(t) => Some(t),
                FlowTarget::Unresolved => None,
            });
        let import_targets = graph
            .imports()
            .iter()
            .filter(|i| i.from_file == file)
            .filter_map(|i| i.target);
        for target in flow_targets.chain(import_targets) {
            if graph.get_file(target).is_none() {
                continue; // an edge into a symbol (a table) — not `core`
            }
            let target_file = graph.string_id(target).to_string();
            if seen.contains(&target_file) {
                continue;
            }
            if fm.admits_to_core(graph, entry, &target_file) {
                queue.push_back(target_file);
            }
        }
    }

    let mut data = Vec::new();
    let mut seen_data = HashSet::new();

    let mut dependencies = Vec::new();
    let mut seen_deps = HashSet::new();
    for file in &core {
        for imp in graph.imports().iter().filter(|i| &i.from_file == file) {
            let Some(target) = imp.target else { continue };
            // A module import (`from app import crud`) resolves to a file
            // node — it either joined `core` above or does no data work;
            // either way it isn't a one-hop *symbol* dependency (those are
            // keyed on `interface_hash`, which a file has none of). The
            // edge still shows in `get_callers`. (M17 — feature-participant
            // granularity for module imports is an M19 question.)
            if graph.get_file(target).is_some() {
                continue;
            }
            let id = graph.string_id(target).to_string();
            if core.contains(&id) {
                continue;
            }
            // An import that resolves to a table symbol is `data`, not a
            // plain dependency — an in-language ORM model (M17's
            // `is_schema_symbol`) is reached this way, where the TS+SQL
            // pack reaches its tables via a `.from("table")` flow edge.
            if graph
                .get_symbol(target)
                .is_some_and(|s| s.kind == crate::symbol::SymbolKind::Schema)
            {
                if seen_data.insert(id.clone()) {
                    data.push(id);
                }
            } else if seen_deps.insert(id.clone()) {
                dependencies.push(id);
            }
        }
    }

    // The rest of the `data` tier: every flow edge from a `core` file that
    // resolves to a *symbol* rather than a file. For the TS+SQL pack those
    // are the Supabase `.from("table")` refs landing on a `SymbolKind::Schema`;
    // the walk doesn't need to know that.
    for file in &core {
        for edge in graph.flow_edges().iter().filter(|e| &e.from_file == file) {
            let FlowTarget::Node(target) = edge.target else {
                continue;
            };
            if graph.get_file(target).is_some() {
                continue;
            }
            let id = graph.string_id(target).to_string();
            if seen_data.insert(id.clone()) {
                data.push(id);
            }
        }
    }
    data.sort();

    Participants {
        core,
        dependencies,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_graph_from_sources;

    /// Write `files` to a fresh temp dir and build the whole graph off it
    /// (`RepoIndex::rebuild` — imports resolved, flow edges set), the way
    /// production does. Returns the graph and the dir (caller cleans up).
    fn fixture(tag: &str, files: &[(&str, &str)]) -> (Graph, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("codeowl-features-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (rel, content) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
        }
        let graph = crate::index::RepoIndex::build(&dir)
            .unwrap()
            .rebuild()
            .unwrap();
        (graph, dir)
    }

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
        let (graph, dir) = fixture(
            "enumerate",
            &[
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
            ],
        );
        let entries = enumerate_entry_points(&graph);
        let files: Vec<&str> = entries.iter().map(|e| e.file.as_str()).collect();
        assert!(files.contains(&"app/submit/page.tsx"));
        assert!(files.contains(&"app/api/stripe-webhook/route.ts"));
        assert!(!files.contains(&"app/api/submit-artwork/route.ts"));

        // The new EntryPoint shape: kind names the convention, id is the
        // spec-file slug (unchanged from feature_slug), title is the path.
        let page = entries
            .iter()
            .find(|e| e.file == "app/submit/page.tsx")
            .unwrap();
        assert_eq!(page.kind, "page");
        assert_eq!(page.id, "submit");
        assert_eq!(page.title, "/submit");
        let route = entries
            .iter()
            .find(|e| e.file == "app/api/stripe-webhook/route.ts")
            .unwrap();
        assert_eq!(route.kind, "api-route");
        assert_eq!(route.id, "api-stripe-webhook");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn admits_to_core_keeps_the_m11_rule_for_the_ts_model() {
        let fm = TypeScriptNextFeatureModel;
        let (graph, dir) = fixture(
            "admits",
            &[
                (
                    "app/submit/page.tsx",
                    "export default function Page() { return null; }\n",
                ),
                (
                    "app/submit/uploader.tsx",
                    "export default function Uploader() { return null; }\n",
                ),
                (
                    "components/ui/button.tsx",
                    "export default function Button() { return null; }\n",
                ),
                (
                    "app/api/x/route.ts",
                    "export async function POST(): Promise<void> {}\n",
                ),
            ],
        );
        let entry = EntryPoint {
            kind: "page".to_string(),
            id: "submit".to_string(),
            title: "/submit".to_string(),
            file: "app/submit/page.tsx".to_string(),
        };
        // co-located -> in; shared primitive -> out; API route -> always in.
        assert!(fm.admits_to_core(&graph, &entry, "app/submit/uploader.tsx"));
        assert!(!fm.admits_to_core(&graph, &entry, "components/ui/button.tsx"));
        assert!(fm.admits_to_core(&graph, &entry, "app/api/x/route.ts"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn assemble_participants_follows_route_literal_then_collects_direct_imports() {
        let (graph, dir) = fixture(
            "assemble",
            &[
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
            ],
        );

        let fm = default_feature_model();
        let entry = fm
            .enumerate_entry_points(&graph)
            .into_iter()
            .find(|e| e.file == "app/submit/page.tsx")
            .unwrap();
        let participants = assemble_participants(&graph, fm, &entry);
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

    #[test]
    fn from_call_with_a_string_literal_is_extracted() {
        let src = "async function f(db) { await db.from(\"payments\").select(\"*\"); }\n";
        let refs = extract_table_refs(src, "route.ts");
        assert_eq!(
            refs,
            vec![TableRef {
                from_file: "route.ts".to_string(),
                table: "payments".to_string(),
            }]
        );
    }

    #[test]
    fn chained_and_nested_from_calls_are_all_found() {
        let src = "async function f(db) {\n  const a = await db.from(\"a\").select();\n  if (a) { await db.from(\"b_two\").insert({}); }\n}\n";
        let tables: Vec<String> = extract_table_refs(src, "r.ts")
            .into_iter()
            .map(|r| r.table)
            .collect();
        assert_eq!(tables, vec!["a".to_string(), "b_two".to_string()]);
    }

    #[test]
    fn array_from_and_dynamic_from_are_ignored() {
        let src = "function f(db, name) {\n  const xs = Array.from(\"abc\");\n  db.from(name);\n  db.from(`t_${name}`);\n}\n";
        assert!(extract_table_refs(src, "r.ts").is_empty());
    }

    #[test]
    fn resolve_table_ref_matches_a_schema_node_by_bare_name() {
        let graph = build_graph_from_sources(&[(
            "supabase/schema.sql",
            "CREATE TABLE public.payments (id integer NOT NULL);\n",
        )]);
        let id = resolve_table_ref(&graph, "payments");
        assert_eq!(
            id.map(|i| graph.string_id(i).to_string()),
            Some("supabase/schema.sql::payments".to_string())
        );
        assert_eq!(resolve_table_ref(&graph, "nonexistent"), None);
    }

    #[test]
    fn rendered_components_are_the_uppercase_jsx_tags_deduped() {
        let src = "export default function Page() {\n  return <div><Header/><Card><Card>x</Card></Card><footer/></div>;\n}\n";
        let names: Vec<String> = extract_rendered_components(src, "app/page.tsx")
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names, vec!["Card".to_string(), "Header".to_string()]);
    }

    #[test]
    fn is_colocated_is_the_entry_dir_or_a_subdir() {
        let entry = "app/submit/page.tsx";
        assert!(is_colocated("app/submit/uploader.tsx", entry));
        assert!(is_colocated("app/submit/_parts/field.tsx", entry));
        assert!(!is_colocated("components/ui/button.tsx", entry));
        assert!(!is_colocated("app/submitother/x.tsx", entry));
    }
}
