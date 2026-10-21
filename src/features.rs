//! The generic feature layer. A feature is a framework entry point plus what
//! it reaches, per `ARCHITECTURE.md`'s "Feature specs". This file is the
//! stack-neutral half: [`EntryPoint`], the [`FeatureModel`] trait a pack
//! implements to say what its framework's entry points are and which reached
//! files belong inside a feature, [`feature_model_for`] (the model of the
//! stack that built a graph), and [`assemble_participants`], the generic
//! graph walk (flow edges plus one-hop imports) that turns an entry point
//! into a participant set.
//!
//! Every framework-specific judgment sits behind [`trait FeatureModel`](FeatureModel),
//! in the pack that owns the framework: `typescript::TypeScriptNextFeatureModel`
//! (Next.js App Router, with the `fetch`, `.from("table")` and JSX extractors
//! it relies on), `quarkus::QuarkusFeatureModel` and the FastAPI model. A
//! stack with no runtime entry surface returns `None` from
//! [`crate::stack::StackPack::feature_model`] and has no feature layer at
//! all. This layer is deliberately narrow, not general call-graph analysis,
//! which stays deferred (see `ARCHITECTURE.md`'s open question 3).

use std::collections::{HashMap, HashSet, VecDeque};

use crate::graph::ResolvedImport;
use crate::graph::{FlowEdge, FlowTarget, Graph, SymbolId};

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

/// The feature model for the stack that built `graph`, or `None` if that
/// stack has none (`RustStack`; `TypeScriptNextStack`, `JavaStack` and
/// `PythonStack` always return one, which for a plain Java library such as
/// commons-lang simply enumerates zero entry points). Recovered from
/// `graph.pack_name()` via [`crate::stack::for_graph`] so a pure-read
/// caller doesn't need the pack threaded down. This is the M14 completion
/// of what M13 stubbed as `default_feature_model()`.
pub fn feature_model_for(graph: &Graph) -> Option<&'static dyn FeatureModel> {
    crate::stack::for_graph(graph).feature_model()
}

/// The entry points for `graph`'s stack — empty for a stack with no
/// feature model.
pub fn enumerate_entry_points(graph: &Graph) -> Vec<EntryPoint> {
    feature_model_for(graph).map_or_else(Vec::new, |fm| fm.enumerate_entry_points(graph))
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
    // Group every flow edge / resolved import by its `from_file` once, up
    // front, instead of re-filtering the whole list by `from_file == file`
    // in each of the three passes below (code-review finding: the BFS
    // below, the dependencies loop, and the data loop each independently
    // rescanned the same edge lists for the same file).
    let mut flow_by_file: HashMap<&str, Vec<&FlowEdge>> = HashMap::new();
    for e in graph.flow_edges() {
        flow_by_file
            .entry(e.from_file.as_str())
            .or_default()
            .push(e);
    }
    let mut imports_by_file: HashMap<&str, Vec<&ResolvedImport>> = HashMap::new();
    for i in graph.imports() {
        imports_by_file
            .entry(i.from_file.as_str())
            .or_default()
            .push(i);
    }
    let flow_edges_from = |file: &str| flow_by_file.get(file).into_iter().flatten().copied();
    let imports_from = |file: &str| imports_by_file.get(file).into_iter().flatten().copied();

    let mut core = Vec::new();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([entry.file.clone()]);
    // `entry` is fixed for the whole walk, so `fm.admits_to_core`'s answer
    // for a given candidate file never changes within one call -- memoized
    // so a file reached as a candidate from several different core files
    // before it's dequeued (a real shape: several routes each importing
    // the same crud module) is only ever judged once (code-review
    // finding: FastAPI's admits_to_core does a full graph scan per call).
    let mut admits_to_core_cache: HashMap<String, bool> = HashMap::new();
    // Every file that declares at least one `Schema` symbol -- computed
    // once, not per candidate. A schema-bearing file's role in a feature
    // is fully represented by the `data` tier (one entry per table); it
    // must never *also* become a `core` file just because one of its
    // other, non-table siblings (a request/response schema class sharing
    // `app/models.py` with a `table=True` one, say) happens to get
    // imported somewhere admits_to_core would otherwise admit. Found via
    // `a_fastapi_route_becomes_a_feature_with_its_table_in_data` going red
    // when the symbol-target promotion below was added: `ItemPublic`
    // (not a table) shares `app/models.py` with `Item` (a table), and
    // FastAPI's own `file_touches_schema` trivially says yes for that
    // file since `Item` lives there -- which would otherwise promote the
    // whole models file to `core` and, via the dedup guard below, mask
    // `Item` out of `data` entirely.
    let schema_files: HashSet<&str> = graph
        .symbols()
        .filter(|s| s.kind == crate::symbol::SymbolKind::Schema)
        .map(|s| s.file.as_str())
        .collect();

    while let Some(file) = queue.pop_front() {
        if !seen.insert(file.clone()) {
            continue;
        }
        core.push(file.clone());

        // Three ways a file can pull another *file* into `core`, all
        // gated by `fm.admits_to_core`:
        //   1. a flow edge into a file node (the pilot's route-literal /
        //      rendered-component edges; M17's `Depends()` / param-type
        //      edges),
        //   2. an import that resolves to a file node — a module import
        //      like Python's `from app import crud`, and
        //   3. an import or flow edge that resolves to a *symbol* whose
        //      own containing file might still qualify — a named import
        //      (`from app.crud import create_item`) or, for a language
        //      with no whole-module import concept at all (Java: every
        //      `@Inject` names a specific class, never a file), *every*
        //      import. See `ARCHITECTURE.md`'s "Feature specs" section
        //      for the full writeup. Fixed 2026-09-15: this used to skip
        //      straight past `admits_to_core`
        //      for a symbol target, so writing the identical dependency
        //      two different ways ("import the module" vs "import one
        //      name from it") put it in two different participant tiers —
        //      the same family of bug the M17 code review already fixed
        //      once for the `data` tier (`f0ba9e4`). Any schema-bearing
        //      file (`schema_files` above) is excluded here regardless of
        //      which of its symbols is the actual target — it belongs in
        //      `data` via its table symbol(s) (handled below), never
        //      duplicated into `core`.
        let flow_targets = flow_edges_from(&file).filter_map(|e| match e.target {
            FlowTarget::Node(t) => Some(t),
            FlowTarget::Unresolved => None,
        });
        let import_targets = imports_from(&file).filter_map(|i| i.target);
        for target in flow_targets.chain(import_targets) {
            // Normalize to a plain file path first, whichever kind of
            // target this is, then apply the schema_files exclusion once
            // uniformly -- code-review finding: this used to be
            // duplicated verbatim across both match arms, one edit away
            // from a future third target-resolution shape forgetting it.
            let target_file = match graph.get_file(target) {
                Some(_) => graph.string_id(target).to_string(),
                None => {
                    let Some(sym) = graph.get_symbol(target) else {
                        continue;
                    };
                    sym.file.clone()
                }
            };
            if schema_files.contains(target_file.as_str()) {
                continue;
            }
            if seen.contains(&target_file) {
                continue;
            }
            let admits = *admits_to_core_cache
                .entry(target_file.clone())
                .or_insert_with(|| fm.admits_to_core(graph, entry, &target_file));
            if admits {
                queue.push_back(target_file);
            }
        }
    }

    let mut data = Vec::new();
    let mut seen_data = HashSet::new();

    let mut dependencies = Vec::new();
    let mut seen_deps = HashSet::new();
    // A symbol target reached from a `core` file — by import or by flow
    // edge — lands in `data` if it's a table (an in-language ORM model,
    // M17's `is_schema_symbol`, or the TS+SQL pack's `.from("table")`
    // flow edge target) and `dependencies` otherwise, UNLESS its own
    // containing file already joined `core` above (the BFS's new symbol-
    // target promotion path, or the file being the entry point itself) —
    // that content is already fully source-tracked as `core`, so it must
    // never also appear as a one-hop `dependencies`/`data` stub for the
    // same file. Shared by both passes below so an import and a flow edge
    // to the same kind of symbol are classified identically.
    let mut classify_symbol_target = |id: String, target: SymbolId| {
        let Some(sym) = graph.get_symbol(target) else {
            return;
        };
        // `seen` has identical membership to `core` at this point (the
        // BFS above is the only thing that ever inserts into either, and
        // it's finished) but is a `HashSet` -- O(1) here instead of an
        // O(core.len()) linear scan per dependency/data symbol
        // (code-review finding).
        if seen.contains(&sym.file) {
            return;
        }
        if sym.kind == crate::symbol::SymbolKind::Schema {
            if seen_data.insert(id.clone()) {
                data.push(id);
            }
        } else if seen_deps.insert(id.clone()) {
            dependencies.push(id);
        }
    };
    for file in &core {
        for imp in imports_from(file) {
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
            // (Code-review finding: this loop used to also check
            // `core.contains(&id)` here, but `id` is a full symbol id
            // ("file::Symbol") which can never match any entry of `core`
            // -- a Vec of plain file paths -- so it was always false,
            // dead code shadowing the real guard already inside
            // `classify_symbol_target`.)
            let id = graph.string_id(target).to_string();
            classify_symbol_target(id, target);
        }
    }
    // The rest of the `data`/`dependencies` tiers: every flow edge from a
    // `core` file that resolves to a symbol rather than a file.
    for file in &core {
        for edge in flow_edges_from(file) {
            let FlowTarget::Node(target) = edge.target else {
                continue;
            };
            if graph.get_file(target).is_some() {
                continue;
            }
            classify_symbol_target(graph.string_id(target).to_string(), target);
        }
    }
    data.sort();

    Participants {
        core,
        dependencies,
        data,
    }
}

/// Write `files` to a fresh temp dir and build the whole graph off it
/// (`RepoIndex::rebuild` — imports resolved, flow edges set), the way
/// production does. Returns the graph and the dir (caller cleans up).
#[cfg(test)]
pub(crate) fn fixture(tag: &str, files: &[(&str, &str)]) -> (Graph, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("codeowl-features-{tag}-{}", std::process::id()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typescript::default_feature_model;

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
            .find(|e| e.id == "submit")
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
    fn a_flow_edge_resolving_to_a_non_schema_symbol_is_a_dependency_not_data() {
        // Code-review finding: the `data` tier was classified
        // inconsistently between the two paths that populate it -- the
        // import-resolved path (above) requires SymbolKind::Schema before
        // routing a target into `data`, but the flow-edge-resolved path
        // below it treated *any* symbol a flow edge resolves to as
        // `data`, unconditionally, with no kind check. Currently harmless
        // (every stack's flow edges that resolve straight to a symbol
        // happen to be table refs), but latent: a future flow-edge kind
        // resolving to an ordinary (non-table) symbol would be silently
        // mis-tagged into `data` instead of `dependencies`. Reproduced
        // here with a hand-crafted flow edge, since no real pack produces
        // this shape today.
        let (mut graph, dir) = fixture(
            "assemble-non-schema-flow-edge",
            &[
                (
                    "app/submit/page.tsx",
                    "export default function Page() { return null; }\n",
                ),
                ("lib/util.ts", "export function util(): void {}\n"),
            ],
        );
        let target = graph.find("lib/util.ts::util").unwrap();
        graph.set_flow_edges(vec![crate::graph::FlowEdge {
            from_file: "app/submit/page.tsx".to_string(),
            kind: "test-non-schema".to_string(),
            raw: "util".to_string(),
            target: FlowTarget::Node(target),
        }]);

        let fm = default_feature_model();
        let entry = fm
            .enumerate_entry_points(&graph)
            .into_iter()
            .find(|e| e.id == "submit")
            .unwrap();
        let participants = assemble_participants(&graph, fm, &entry);
        assert_eq!(
            participants.dependencies,
            vec!["lib/util.ts::util".to_string()],
            "a flow edge to a non-table symbol belongs in dependencies: {participants:?}"
        );
        assert!(
            !participants.data.contains(&"lib/util.ts::util".to_string()),
            "a non-table symbol must never land in data: {participants:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
