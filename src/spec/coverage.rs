//! The coverage report: every document the granularity rules call for with its status, and the summaries `get_spec_coverage` serves.

use std::path::Path;

use anyhow::{Context, Result};

use super::feature::{current_feature_hashes, current_participant_hashes, read_feature_spec};
use super::format::{read_existing, read_file_spec};
use super::layout::{file_is_spec_bearing, is_test_path, spec_bearing_children};
use super::rollup::{
    current_file_hashes, current_module_hashes, enumerate_modules, read_rollup_spec,
};
use super::smells::{body_smells, file_spec_smells, prose_smells};
use super::staleness::{diff_hash_lists, file_changes, symbol_changes};
use super::system::read_system_spec;
use crate::features::{
    EntryPoint, assemble_participants, enumerate_entry_points, feature_model_for,
};
use crate::graph::{Graph, SymbolId};
use crate::text::within_scope;

/// One document the granularity rules say should exist, and where it
/// currently stands (M8) — the unit `get_spec_coverage` reports against.
/// `id` is exactly the id `get_next_spec_task`/`get_spec` expect for that
/// document: a repo-relative path for a file, `"rollup:<dir>"`,
/// `"feature:<slug>"`, or the fixed `"system"`.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageItem {
    pub id: String,
    pub kind: String,
    pub status: String,
    /// Import fan-in — how many other files import something from this
    /// one. Always 0 for non-file kinds; this is what "high-fan-in files"
    /// in `ARCHITECTURE.md`'s "Generation priority" is computed from.
    pub fan_in: usize,
    /// Deterministic quality smells found in this document's own current
    /// content — never empty just because `status` is `"missing"`
    /// (nothing to smell-check) but can be non-empty even when `status`
    /// is `"current"`: hash-based staleness only verifies a spec's
    /// *inputs* haven't moved, never that its prose was ever meaningful.
    /// See `prose_smells`/`ARCHITECTURE.md`'s "Quality smells".
    pub smells: Vec<String>,
    /// How many `get_next_spec_task` → `submit_spec` cycles this document
    /// still costs — i.e. what one unit of `/codeowl generate --budget=N`
    /// buys. For a file: its uncovered/stale/smelly top-level symbols,
    /// plus 1 for the file's own `## Summary` if that needs (re)writing.
    /// For a rollup/feature/the system spec: 1 while non-current, else 0.
    /// `0` for a genuinely clean, current document. Summed across the
    /// report this is the total budget a full run needs (see
    /// `CoverageSummary::generations_remaining`).
    pub generations: usize,
}

/// How many *non-test* files import something from this file — the "shared
/// infrastructure" signal `prioritize` tiers on. Imports from test code
/// don't count: a `lib/` helper pulled in by ten e2e specs and two real
/// callers is depended on by two things, not twelve.
fn file_fan_in(graph: &Graph, file_id: SymbolId) -> usize {
    graph
        .imports()
        .iter()
        .filter(|imp| !is_test_path(graph, &imp.from_file))
        .filter(|imp| {
            imp.target
                .is_some_and(|t| graph.parent_id(t) == Some(file_id))
        })
        .count()
}

/// A file document's status: "missing" if nothing at all has ever been
/// generated for it (neither any symbol nor its own summary — the same
/// `HashPair`-is-empty/`symbols`-is-empty signal `next_task` itself uses
/// to distinguish a first-ever generation from a reconciliation), else
/// "current" iff every symbol's and the file's own hashes still match,
/// else "stale". Deliberately *not* implemented via `next_task` (which
/// also treats a quality smell as a reason to offer a task, so the
/// generate loop can act on one) — `status` here stays purely hash-based,
/// so it never contradicts `smells` being reported as a *separate* signal
/// on a document that's otherwise fully current. See "Quality smells" in
/// `ARCHITECTURE.md`.
/// How many generation cycles `next_task` would still yield for `file_id`
/// if drained — a read-only mirror of `next_task`'s per-symbol decision
/// (never generated / source moved / smelly → a task; source unchanged +
/// human-edited → a silent reconcile, no task). No side effects: unlike
/// `next_task` it doesn't reconcile, it just counts.
fn file_pending_generations(graph: &Graph, root: &Path, file_id: SymbolId) -> Result<usize> {
    let file = graph.get_file(file_id).context("not a file id")?;
    let existing = read_existing(root, &file.id)?;
    let mut n = 0;

    for sym_id in spec_bearing_children(graph, file_id) {
        let sym = graph.get_symbol(sym_id).expect("filtered to symbol ids");
        match existing.as_ref().and_then(|spec| spec.symbol_hash(&sym.id)) {
            None => n += 1, // never generated
            Some(hash) => {
                let source_changed = !symbol_changes(graph, root, file_id, sym, hash).is_empty();
                let prose = existing
                    .as_ref()
                    .and_then(|spec| spec.section(&sym.id).cloned())
                    .unwrap_or_default();
                let smelly = !prose_smells(&prose.summary).is_empty()
                    || !prose_smells(&prose.behavior).is_empty();
                // source unchanged + human-edited reconciles silently
                // (no task); everything else offered is a real generation.
                if source_changed || smelly {
                    n += 1;
                }
            }
        }
    }

    let file_needs_writing = match existing
        .as_ref()
        .filter(|spec| !spec.file.spec_hash.is_empty())
    {
        None => true, // the file's own `## Summary` was never generated
        Some(spec) => {
            !file_changes(graph, file_id, &spec.file).is_empty()
                || !prose_smells(&spec.file_summary).is_empty()
        }
    };
    if file_needs_writing {
        n += 1;
    }
    Ok(n)
}

fn file_status(graph: &Graph, root: &Path, file_id: SymbolId) -> Result<(String, Vec<String>)> {
    let file = graph.get_file(file_id).context("not a file id")?;
    let Some(spec) = read_file_spec(root, &file.id)? else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let has_any_content = !spec.file.spec_hash.is_empty() || !spec.symbols.is_empty();
    if !has_any_content {
        return Ok(("missing".to_string(), Vec::new()));
    }

    let mut hash_current = true;
    for sym_id in spec_bearing_children(graph, file_id) {
        let sym = graph.get_symbol(sym_id).expect("filtered to symbol ids");
        let up_to_date = spec
            .symbol_hash(&sym.id)
            .is_some_and(|hash| symbol_changes(graph, root, file_id, sym, hash).is_empty());
        if !up_to_date {
            hash_current = false;
            break;
        }
    }
    if hash_current {
        hash_current = file_changes(graph, file_id, &spec.file).is_empty();
    }
    let status = if hash_current { "current" } else { "stale" }.to_string();
    Ok((status, file_spec_smells(graph, root, file_id, &spec)))
}

fn rollup_status(graph: &Graph, root: &Path, dir: &str) -> Result<(String, Vec<String>)> {
    let Some(spec) = read_rollup_spec(root, dir)? else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let current = current_file_hashes(graph, root, dir)?;
    let status = if diff_hash_lists(&current, &spec.files).is_empty() {
        "current"
    } else {
        "stale"
    }
    .to_string();
    Ok((status, body_smells(&spec.body)))
}

fn feature_status(graph: &Graph, root: &Path, entry: &EntryPoint) -> Result<(String, Vec<String>)> {
    let Some(spec) = read_feature_spec(root, &entry.id)? else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let Some(fm) = feature_model_for(graph) else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let participants = assemble_participants(graph, fm, entry);
    let current = current_participant_hashes(graph, &participants)?;
    let status = if diff_hash_lists(&current, &spec.participants).is_empty() {
        "current"
    } else {
        "stale"
    }
    .to_string();
    Ok((status, body_smells(&spec.body)))
}

fn system_status(graph: &Graph, root: &Path) -> Result<(String, Vec<String>)> {
    let Some(spec) = read_system_spec(root)? else {
        return Ok(("missing".to_string(), Vec::new()));
    };
    let smells = body_smells(&spec.body);
    let mut current_all = current_module_hashes(graph, root)?;
    current_all.extend(current_feature_hashes(graph, root)?);
    let mut stored_all = spec.modules;
    stored_all.extend(spec.features);
    let status = if diff_hash_lists(&current_all, &stored_all).is_empty() {
        "current"
    } else {
        "stale"
    }
    .to_string();
    Ok((status, smells))
}

/// Every document the granularity rules say should exist, each with its
/// current status — the whole-repo inventory `get_spec_coverage` reports
/// (M8), optionally narrowed to files/rollups under `scope` (a directory
/// prefix — see `within_scope`). Features and the system spec are
/// unaffected by `scope` — both are repo-wide concepts, and `scope`
/// itself excludes the system spec entirely (a system spec scoped to one
/// directory isn't a coherent thing to ask for).
pub fn coverage(graph: &Graph, root: &Path, scope: Option<&str>) -> Result<Vec<CoverageItem>> {
    let mut items = Vec::new();

    let mut file_ids: Vec<SymbolId> = graph.files().filter_map(|f| graph.find(&f.id)).collect();
    file_ids.sort_by_key(|&id| graph.string_id(id).to_string());
    for id in file_ids {
        if !file_is_spec_bearing(graph, id) {
            continue;
        }
        let path = graph.string_id(id).to_string();
        if scope.is_some_and(|s| !within_scope(&path, s)) {
            continue;
        }
        let (status, smells) = file_status(graph, root, id)?;
        items.push(CoverageItem {
            status,
            fan_in: file_fan_in(graph, id),
            smells,
            generations: file_pending_generations(graph, root, id)?,
            id: path,
            kind: "file".to_string(),
        });
    }

    for dir in enumerate_modules(graph) {
        if scope.is_some_and(|s| !within_scope(&dir, s)) {
            continue;
        }
        let (status, smells) = rollup_status(graph, root, &dir)?;
        items.push(CoverageItem {
            generations: usize::from(status != "current"),
            status,
            fan_in: 0,
            smells,
            id: format!("rollup:{dir}"),
            kind: "rollup".to_string(),
        });
    }

    if scope.is_none() {
        for entry in enumerate_entry_points(graph) {
            let (status, smells) = feature_status(graph, root, &entry)?;
            items.push(CoverageItem {
                generations: usize::from(status != "current"),
                status,
                fan_in: 0,
                smells,
                id: format!("feature:{}", entry.id),
                kind: "feature".to_string(),
            });
        }
        let (status, smells) = system_status(graph, root)?;
        items.push(CoverageItem {
            generations: usize::from(status != "current"),
            status,
            fan_in: 0,
            smells,
            id: "system".to_string(),
            kind: "system".to_string(),
        });
    }

    Ok(items)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoverageSummary {
    pub current: usize,
    pub stale: usize,
    pub missing: usize,
    /// Count of items with at least one quality smell, of *any* status —
    /// deliberately not exclusive with `current`: a document can be
    /// hash-current and still carry a smell, which is exactly the case
    /// this field exists to surface (see `CoverageItem.smells`).
    pub smelly: usize,
    /// Total `get_next_spec_task` → `submit_spec` cycles a full
    /// `/codeowl generate --all` run against this scope would spend — the
    /// sum of every item's [`CoverageItem::generations`]. This is the
    /// `--budget=N` a complete generation needs (a file with 20 uncovered
    /// symbols is 21, not 1).
    pub generations_remaining: usize,
}

pub fn summarize(items: &[CoverageItem]) -> CoverageSummary {
    let mut summary = CoverageSummary::default();
    for item in items {
        match item.status.as_str() {
            "current" => summary.current += 1,
            "stale" => summary.stale += 1,
            "missing" => summary.missing += 1,
            _ => {}
        }
        if !item.smells.is_empty() {
            summary.smelly += 1;
        }
        summary.generations_remaining += item.generations;
    }
    summary
}

impl CoverageSummary {
    /// `current + stale + missing` — every item this summary covers.
    /// Deliberately not a stored field: it's trivially derived, and storing
    /// it would just be one more thing `summarize` could get out of sync.
    pub fn total(&self) -> usize {
        self.current + self.stale + self.missing
    }

    /// Of the specs that *exist* (current or stale), what fraction still
    /// match the code — deliberately excludes `missing`: "no spec yet" and
    /// "spec exists but is wrong" are different failure modes needing
    /// different fixes (write one vs. regenerate one), and collapsing them
    /// into a single score hides which one a repo actually has. `1.0` when
    /// nothing has ever been generated — vacuously fresh, since there's
    /// nothing stale to report yet (see `coverage_ratio` for the other
    /// half of that same repo's story).
    pub fn freshness(&self) -> f64 {
        let documented = self.current + self.stale;
        if documented == 0 {
            1.0
        } else {
            self.current as f64 / documented as f64
        }
    }

    /// What fraction of eligible nodes have *any* spec at all, current or
    /// stale — the other axis from `freshness`. A repo can be 100% covered
    /// and 60% fresh (everything's been written once, a lot of it needs
    /// re-running) or 60% covered and 100% fresh (nothing documented is
    /// wrong, there's just more left to write) — two different problems
    /// with two different fixes, which is why this is a separate number
    /// rather than folded into `freshness`.
    pub fn coverage_ratio(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            1.0
        } else {
            (self.current + self.stale) as f64 / total as f64
        }
    }
}

/// `freshness`, but weighted by blast radius (import fan-in) instead of by
/// item count — a stale leaf utility matters far less than a stale file
/// forty others import. Scoped to `kind == "file"` items with a spec that
/// actually exists (`status != "missing"`), matching `freshness`'s own
/// "missing is coverage's problem, not freshness's" exclusion — a
/// high-fan-in file that simply has no spec yet shouldn't drag this number
/// down, that's what `coverage_ratio` is for. Falls back to the plain,
/// unweighted freshness over that same subset when every item's `fan_in` is
/// 0 (nothing to weight by, and 0/0 would otherwise need its own case).
pub fn weighted_freshness(items: &[CoverageItem]) -> f64 {
    let documented: Vec<&CoverageItem> = items
        .iter()
        .filter(|i| i.kind == "file" && i.status != "missing")
        .collect();
    let total_fan_in: usize = documented.iter().map(|i| i.fan_in).sum();
    if total_fan_in == 0 {
        if documented.is_empty() {
            return 1.0;
        }
        let current = documented.iter().filter(|i| i.status == "current").count();
        return current as f64 / documented.len() as f64;
    }
    let weighted_current: usize = documented
        .iter()
        .filter(|i| i.status == "current")
        .map(|i| i.fan_in)
        .sum();
    weighted_current as f64 / total_fan_in as f64
}

/// The canonical kind order a caller should render `by_kind` in — matches
/// `prioritize`'s own file/rollup/feature/system tiering, so "coverage
/// broken down by kind" reads in the same order a generate run would work
/// through them.
const KIND_ORDER: [&str; 4] = ["file", "rollup", "feature", "system"];

/// `coverage()`'s items, grouped by `kind` and each group summarized with
/// the same `summarize` every other rollup uses — so "how many feature
/// specs total" is `by_kind` finding `"feature"` and reading its `total()`,
/// not a separate count computed a second way. Kinds with no items are
/// omitted rather than reported as an all-zero row (a scoped `coverage()`
/// call never produces `"feature"`/`"system"` items at all).
pub fn by_kind(items: &[CoverageItem]) -> Vec<(String, CoverageSummary)> {
    KIND_ORDER
        .iter()
        .filter_map(|kind| {
            let matching: Vec<CoverageItem> =
                items.iter().filter(|i| i.kind == *kind).cloned().collect();
            if matching.is_empty() {
                None
            } else {
                Some((kind.to_string(), summarize(&matching)))
            }
        })
        .collect()
}

/// The directory `item` should be bucketed under for `by_module`, or `None`
/// if it doesn't belong to one (a feature or the system spec is a repo-wide
/// concept, not a directory's). A file's module is its own containing
/// directory; a rollup's module is the directory it *summarizes* (not that
/// directory's parent) — so `rollup:lib/email` lands in the same bucket as
/// `lib/email/foo.ts`, which is what "what's left in lib/email" actually
/// means. The repo root is `"."`, never an empty string.
fn module_of(item: &CoverageItem) -> Option<String> {
    let dir = match item.kind.as_str() {
        "file" => Path::new(&item.id).parent()?.to_string_lossy().into_owned(),
        "rollup" => item.id.strip_prefix("rollup:")?.to_string(),
        _ => return None,
    };
    Some(if dir.is_empty() { ".".to_string() } else { dir })
}

/// `coverage()`'s items, grouped by directory (see `module_of`) and each
/// group summarized with `summarize` — the per-directory breakdown
/// `ARCHITECTURE.md` already (prematurely) claimed `get_spec_coverage` had.
/// Sorted by path so the result is deterministic and reads top-down like a
/// file tree.
pub fn by_module(items: &[CoverageItem]) -> Vec<(String, CoverageSummary)> {
    let mut buckets: Vec<(String, Vec<CoverageItem>)> = Vec::new();
    for item in items {
        let Some(module) = module_of(item) else {
            continue;
        };
        match buckets.iter_mut().find(|(m, _)| *m == module) {
            Some((_, v)) => v.push(item.clone()),
            None => buckets.push((module, vec![item.clone()])),
        }
    }
    buckets.sort_by(|a, b| a.0.cmp(&b.0));
    buckets
        .into_iter()
        .map(|(m, v)| (m, summarize(&v)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::fixtures::build_graph_from_sources;
    use crate::hash::hash_text;
    use crate::spec::feature::current_feature_hashes;
    use crate::spec::file_task::submit;

    use crate::spec::layout::file_is_spec_bearing;
    use crate::spec::system::next_system_task;
    use crate::spec::test_support::{SYSTEM_FIXTURE, build_feature_fixture};

    #[test]
    fn a_classs_field_members_do_not_inflate_generations_remaining_ts() {
        // Same invariant as the Rust version above, for typescript/mod.rs's own
        // field extraction -- a field's parent is the class, never the
        // file, so spec_bearing_children can't see it regardless of kind.
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-field-generations-ts",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = "export class FileNode {\n    id: string;\n    sourceHash: string;\n    children: string[];\n}\n";
        std::fs::write(dir.join("a.ts"), src).unwrap();

        let graph = build_graph_from_sources(&[("a.ts", src)]);
        let file_id = graph.find("a.ts").unwrap();

        assert!(file_is_spec_bearing(&graph, file_id));
        // 2: the class's own task (1) plus the file's never-generated
        // `## Summary` (1) -- never one extra per field.
        assert_eq!(file_pending_generations(&graph, &dir, file_id).unwrap(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_classs_field_members_do_not_inflate_generations_remaining_python() {
        // Same invariant as the Rust/TS versions above, for python/mod.rs's
        // own class-body-assignment extraction.
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-field-generations-py",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = "class FileNode:\n    id: str\n    source_hash: str\n    children: list\n";
        std::fs::write(dir.join("a.py"), src).unwrap();

        let graph = Graph::build(vec![crate::graph::FileExtraction {
            rel_path: "a.py".to_string(),
            source_hash: hash_text(src),
            symbols: crate::python::extract_file(src, "a.py"),
        }]);
        let file_id = graph.find("a.py").unwrap();

        assert!(file_is_spec_bearing(&graph, file_id));
        // 2: the class's own task (1) plus the file's never-generated
        // `## Summary` (1) -- never one extra per attribute.
        assert_eq!(file_pending_generations(&graph, &dir, file_id).unwrap(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_structs_field_members_do_not_inflate_generations_remaining() {
        // M21 invariant 1: adding fields as SymbolKind::Value members must
        // not multiply generations_remaining across every repo.
        // spec_bearing_children only ever looks at a FILE's direct
        // children, and a field's parent is the struct, never the file, so
        // this holds structurally -- confirmed here against a real fixture
        // rather than just argued from the filter's shape.
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-field-generations",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = "pub struct FileNode {\n    pub id: String,\n    pub source_hash: String,\n    pub children: Vec<u32>,\n}\n";
        std::fs::write(dir.join("a.rs"), src).unwrap();

        let graph = Graph::build(vec![crate::graph::FileExtraction {
            rel_path: "a.rs".to_string(),
            source_hash: hash_text(src),
            symbols: crate::rust::extract_file(src, "a.rs"),
        }]);
        let file_id = graph.find("a.rs").unwrap();

        assert!(file_is_spec_bearing(&graph, file_id));
        // 2, not 5: the struct's own `### Summary`/`### Behavior` task (1)
        // plus the file's own never-generated `## Summary` (1) -- never
        // one extra per field.
        assert_eq!(file_pending_generations(&graph, &dir, file_id).unwrap(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn coverage_counts_generations_not_just_documents() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-gens", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = "export function one(): void {}\n\
                   export function two(): void {}\n\
                   export function three(): void {}\n";
        std::fs::write(dir.join("a.ts"), src).unwrap();
        let graph = build_graph_from_sources(&[("a.ts", src)]);

        // Nothing generated: one `pending` document (a.ts), but four
        // generations — three symbols + the file's own summary.
        let a = |items: &[CoverageItem]| items.iter().find(|i| i.id == "a.ts").unwrap().generations;
        assert_eq!(a(&coverage(&graph, &dir, None).unwrap()), 4);

        // Each symbol spec drops the count by one.
        submit(
            &graph,
            &dir,
            "a.ts::one",
            "### Summary\nDoes the first job.\n### Behavior\nRuns with no side effects.\n",
        )
        .unwrap();
        assert_eq!(a(&coverage(&graph, &dir, None).unwrap()), 3);

        for id in ["a.ts::two", "a.ts::three"] {
            submit(
                &graph,
                &dir,
                id,
                "### Summary\nDoes a second deliberate thing.\n### Behavior\nRuns with no side effects.\n",
            )
            .unwrap();
        }
        // Symbols done, only the file summary left.
        assert_eq!(a(&coverage(&graph, &dir, None).unwrap()), 1);

        submit(
            &graph,
            &dir,
            "a.ts",
            "Three small deliberate no-op helpers.",
        )
        .unwrap();
        let items = coverage(&graph, &dir, None).unwrap();
        assert_eq!(a(&items), 0);
        // The system spec is still missing, so the repo-wide total is 1,
        // and it equals the sum of every item's own share.
        let summary = summarize(&items);
        assert_eq!(summary.generations_remaining, 1);
        assert_eq!(
            summary.generations_remaining,
            items.iter().map(|i| i.generations).sum::<usize>()
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_stack_with_no_feature_model_composes_a_system_spec_from_modules_alone() {
        // M14 / design decision 3: `RustStack` has no feature layer, so
        // `coverage` must report zero features and the system-spec
        // composition must not panic or emit an empty `## Features`.
        let (graph, dir) = build_feature_fixture(
            &[
                ("src/lib.rs", "pub mod parse;\npub mod store;\n"),
                (
                    "src/parse.rs",
                    "//! Parsing.\npub fn tokenize(_s: &str) -> Vec<String> { Vec::new() }\npub fn lex() {}\n",
                ),
                (
                    "src/store.rs",
                    "//! Storage.\nuse crate::parse::tokenize;\npub fn save(_s: &str) { tokenize(_s); }\npub fn load() {}\n",
                ),
            ],
            "rust-nofeat",
        );
        assert_eq!(graph.pack_name(), "rust");
        assert!(feature_model_for(&graph).is_none());
        assert!(current_feature_hashes(&graph, &dir).unwrap().is_empty());

        let items = coverage(&graph, &dir, None).unwrap();
        assert!(
            !items.iter().any(|i| i.kind == "feature"),
            "a no-feature stack must produce no feature coverage items"
        );
        assert!(items.iter().any(|i| i.id == "system"));

        // The system task is generable and its `features` list is empty.
        // (Modules/rollups come first; drain them, then ask for system.)
        let sys = next_system_task(&graph, &dir).unwrap();
        if let Some(task) = sys {
            assert!(task.features.is_empty(), "no features to compose over");
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn coverage_scope_narrows_to_files_and_rollups_under_that_prefix() {
        let (graph, dir) = build_feature_fixture(SYSTEM_FIXTURE, "coverage2");
        let items = coverage(&graph, &dir, Some("lib")).unwrap();
        let ids: std::collections::HashSet<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            std::collections::HashSet::from([
                "lib/supabase.ts",
                "lib/one.ts",
                "lib/two.ts",
                "rollup:lib",
            ]),
            "scope excludes app/* files, the feature, and the system spec"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn by_kind_groups_and_summarizes_per_kind_in_canonical_order() {
        fn item(id: &str, kind: &str, status: &str) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: status.into(),
                fan_in: 0,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        let items = vec![
            item("system", "system", "stale"),
            item("feature:checkout", "feature", "missing"),
            item("rollup:lib", "rollup", "current"),
            item("lib/db.ts", "file", "current"),
            item("lib/utils.ts", "file", "stale"),
        ];
        let buckets = by_kind(&items);
        let kinds: Vec<&str> = buckets.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            kinds,
            vec!["file", "rollup", "feature", "system"],
            "canonical order regardless of input order"
        );

        let file_summary = &buckets.iter().find(|(k, _)| k == "file").unwrap().1;
        assert_eq!(file_summary.current, 1);
        assert_eq!(file_summary.stale, 1);
        assert_eq!(file_summary.total(), 2);
    }

    #[test]
    fn by_kind_omits_kinds_with_no_items() {
        fn item(id: &str, kind: &str) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: "missing".into(),
                fan_in: 0,
                smells: Vec::new(),
                generations: 1,
            }
        }
        // A scoped coverage() call never produces feature/system items.
        let items = vec![item("lib/db.ts", "file")];
        let kinds: Vec<String> = by_kind(&items).into_iter().map(|(k, _)| k).collect();
        assert_eq!(kinds, vec!["file".to_string()]);
    }

    #[test]
    fn by_module_groups_files_and_their_rollup_by_directory() {
        fn item(id: &str, kind: &str, status: &str) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: status.into(),
                fan_in: 0,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        let items = vec![
            item("lib/email/foo.ts", "file", "current"),
            item("lib/email/bar.ts", "file", "stale"),
            item("rollup:lib/email", "rollup", "missing"),
            item("index.ts", "file", "current"),
            item("feature:checkout", "feature", "missing"),
            item("system", "system", "missing"),
        ];
        let buckets = by_module(&items);
        let paths: Vec<&str> = buckets.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            vec![".", "lib/email"],
            "sorted by path; features/system have no module and are excluded"
        );

        let email = &buckets.iter().find(|(p, _)| p == "lib/email").unwrap().1;
        assert_eq!(
            email.total(),
            3,
            "the two files plus their own directory's rollup, bucketed together"
        );
        assert_eq!(email.current, 1);
        assert_eq!(email.stale, 1);
        assert_eq!(email.missing, 1);
    }

    #[test]
    fn freshness_ignores_missing_entirely() {
        // 1 current, 1 stale, 5 missing -- freshness is about the specs
        // that exist, not about how much of the repo is documented yet.
        let summary = CoverageSummary {
            current: 1,
            stale: 1,
            missing: 5,
            ..CoverageSummary::default()
        };
        assert_eq!(summary.freshness(), 0.5);
    }

    #[test]
    fn freshness_is_vacuously_full_when_nothing_documented_yet() {
        let summary = CoverageSummary {
            missing: 10,
            ..CoverageSummary::default()
        };
        assert_eq!(
            summary.freshness(),
            1.0,
            "nothing stale to report -- undocumented is coverage's problem, not freshness's"
        );
    }

    #[test]
    fn coverage_ratio_counts_stale_as_covered() {
        // A stale spec still means "something was generated for this" --
        // coverage and freshness are deliberately different axes.
        let summary = CoverageSummary {
            current: 1,
            stale: 1,
            missing: 2,
            ..CoverageSummary::default()
        };
        assert_eq!(summary.coverage_ratio(), 0.5);
    }

    #[test]
    fn coverage_ratio_is_vacuously_full_when_nothing_is_eligible() {
        assert_eq!(CoverageSummary::default().coverage_ratio(), 1.0);
    }

    #[test]
    fn weighted_freshness_weights_by_fan_in_not_item_count() {
        fn item(id: &str, status: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: "file".into(),
                status: status.into(),
                fan_in,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        // One stale file with huge fan-in, nine current leaf files with
        // none -- by item count this reads 90% fresh; weighted by blast
        // radius, the busy stale file dominates.
        let mut items = vec![item("lib/db.ts", "stale", 40)];
        for i in 0..9 {
            items.push(item(&format!("leaf{i}.ts"), "current", 0));
        }
        assert_eq!(weighted_freshness(&items), 0.0);
    }

    #[test]
    fn weighted_freshness_falls_back_to_unweighted_when_nothing_has_fan_in() {
        fn item(id: &str, status: &str) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: "file".into(),
                status: status.into(),
                fan_in: 0,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        let items = vec![
            item("a.ts", "current"),
            item("b.ts", "current"),
            item("c.ts", "stale"),
        ];
        assert_eq!(weighted_freshness(&items), 2.0 / 3.0);
    }

    #[test]
    fn weighted_freshness_ignores_missing_and_non_file_items() {
        fn item(id: &str, kind: &str, status: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: status.into(),
                fan_in,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        let items = vec![
            item("a.ts", "file", "current", 5),
            item("b.ts", "file", "missing", 99), // no spec yet -- excluded
            item("rollup:lib", "rollup", "stale", 0), // not a file -- excluded
        ];
        assert_eq!(
            weighted_freshness(&items),
            1.0,
            "only a.ts is an eligible (file, non-missing) item, and it's current"
        );
    }
}
