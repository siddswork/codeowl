//! The MCP surface — read tools from M3, plus M4's write side driving
//! generation. `get_spec`/`get_symbol`/`get_callers`/`get_callees`/
//! `search_code` are pure reads, always: none of them ever writes
//! anything or calls an LLM — see `CLAUDE.md`'s hard invariants.
//! `get_next_spec_task`/`submit_spec` are the two calls the *client's* own
//! LLM drives (never CodeOwl itself — it holds no credentials) via
//! `/codeowl generate`; see `ARCHITECTURE.md`'s "Who actually writes the
//! spec text".

use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::graph::{Graph, SymbolView};
use crate::search::SearchMatch;
use crate::symbol::SymbolKind;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IdRequest {
    /// A symbol's stable id, e.g. "lib/utils.ts::cn".
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchRequest {
    /// A regex pattern to search for across the repo's source files.
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CoverageRequest {
    /// Narrow the file/rollup portion of the report to this directory
    /// prefix (e.g. "lib"). Features and the system spec are always
    /// repo-wide, unaffected by scope. Omit for the whole repo.
    pub scope: Option<String>,
    /// Where to resume `pending` from (M20) — pass back the previous
    /// call's `next_cursor` verbatim. Omit, or `0`, for the first page.
    /// Every other field in the response (`current`/`stale`/`missing`/
    /// `by_kind`/`by_module`/`top_stale_by_impact`/`orphaned`/…) is
    /// always computed over the *whole* repo regardless of `cursor` —
    /// only `pending` itself is paginated, since it's the one field that
    /// grows unboundedly with repo size (confirmed real: 626 files on
    /// `commons-lang` serialized `pending` alone to 95 KB, over the MCP
    /// result limit).
    pub cursor: Option<usize>,
}

/// How many `pending` entries [`CodeOwlServer::get_spec_coverage`] returns
/// per page. A conservative default (not yet a CLI flag, unlike
/// `--max-spec-task-bytes`'s recalibration — this is sized from a
/// measured real repo, not a specific reported client failure; revisit
/// if one is ever reported the way the God-class payload size was).
const COVERAGE_PENDING_PAGE_SIZE: usize = 50;

/// One spec on disk whose target no longer exists in the graph at all —
/// see `crate::spec::OrphanedSpec`. Distinct from a `pending` entry: a
/// `pending` item still has something in the graph to regenerate against,
/// an orphan does not, and it's the caller's job to delete these, not
/// `/codeowl generate`'s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct OrphanedSpecResponse {
    pub id: String,
    /// One of `"file"` | `"rollup"` | `"feature"` | `"symbol"`. A `"symbol"`
    /// entry is a section inside an otherwise-still-valid file spec whose
    /// symbol was deleted from the source -- the file itself is fine, only
    /// that one section is dead weight (it prunes itself automatically the
    /// next time that file's spec is regenerated for any other reason).
    pub kind: String,
    /// For file/rollup/feature: the orphaned document's own path. For
    /// symbol: the *containing file's* spec path -- there's no separate
    /// document to delete, just a section to be aware is stale.
    pub path: String,
}

impl From<crate::spec::OrphanedSpec> for OrphanedSpecResponse {
    fn from(o: crate::spec::OrphanedSpec) -> Self {
        OrphanedSpecResponse {
            id: o.id,
            kind: o.kind,
            path: o.path,
        }
    }
}

impl From<crate::spec::CoverageItem> for CoverageItemResponse {
    fn from(i: crate::spec::CoverageItem) -> Self {
        CoverageItemResponse {
            id: i.id,
            kind: i.kind,
            status: i.status,
            fan_in: i.fan_in,
            smells: i.smells,
            generations: i.generations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct CoverageItemResponse {
    /// Pass straight to `get_next_spec_task`/`get_spec` — a repo-relative
    /// file path, or `"rollup:<dir>"`/`"feature:<slug>"`/`"system"`.
    pub id: String,
    /// One of `"file"` | `"rollup"` | `"feature"` | `"system"`.
    pub kind: String,
    /// One of `"missing"` | `"stale"` | `"current"`. `"current"` only
    /// appears here when `smells` is non-empty -- a hash-current document
    /// that a deterministic quality check still flagged (see `smells`).
    pub status: String,
    /// Import fan-in — how many other files import something from this
    /// one. Always 0 for non-file kinds.
    pub fan_in: usize,
    /// Deterministic quality smells found in this document's current
    /// content (e.g. `"cop_out_phrase"`, `"suspiciously_short"`,
    /// `"identical_dependencies_across_symbols"`) — independent of
    /// `status`, since hash-based staleness only verifies inputs haven't
    /// moved, never that the prose was ever meaningful. Empty for a
    /// genuinely clean document.
    pub smells: Vec<String>,
    /// How many `get_next_spec_task` → `submit_spec` cycles this one
    /// document still costs — one unit of `--budget=N`. For a file: its
    /// uncovered/stale/smelly top-level symbols plus 1 for the file's own
    /// `## Summary` if that needs writing (so a single `pending` row can
    /// be worth 20+). For a rollup/feature/`system`: 1.
    pub generations: usize,
}

/// The same `current`/`stale`/`missing`/`smelly`/`generations_remaining`
/// shape `CoverageResponse`'s top level carries, scoped to one `kind`
/// (`"file"`/`"rollup"`/`"feature"`/`"system"`) — see `by_kind`.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct KindBreakdown {
    pub kind: String,
    pub current: usize,
    pub stale: usize,
    pub missing: usize,
    pub smelly: usize,
    pub generations_remaining: usize,
    /// `current + stale + missing` — e.g. `by_kind`'s `"feature"` row's
    /// `total` directly answers "how many feature specs will exist."
    pub total: usize,
    /// `current / (current + stale)` within this kind alone — see the
    /// top-level `freshness` field's doc comment for what this axis means.
    pub freshness: f64,
    /// `(current + stale) / total` within this kind alone.
    pub coverage: f64,
}

/// The same breakdown as [`KindBreakdown`], scoped to one directory instead
/// of one kind — see `by_module`. A file's directory and its own rollup's
/// directory share a bucket (`rollup:lib/email` and `lib/email/foo.ts` both
/// land under `"lib/email"`), so a bucket answers "what's left in this
/// module" as one number, not two. Features and the system spec have no
/// directory and never appear here.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ModuleBreakdown {
    pub path: String,
    pub current: usize,
    pub stale: usize,
    pub missing: usize,
    pub smelly: usize,
    pub generations_remaining: usize,
    pub total: usize,
    /// `current / (current + stale)` within this directory alone.
    pub freshness: f64,
    /// `(current + stale) / total` within this directory alone.
    pub coverage: f64,
}

impl From<(String, crate::spec::CoverageSummary)> for KindBreakdown {
    fn from((kind, s): (String, crate::spec::CoverageSummary)) -> Self {
        KindBreakdown {
            kind,
            current: s.current,
            stale: s.stale,
            missing: s.missing,
            smelly: s.smelly,
            generations_remaining: s.generations_remaining,
            total: s.total(),
            freshness: s.freshness(),
            coverage: s.coverage_ratio(),
        }
    }
}

impl From<(String, crate::spec::CoverageSummary)> for ModuleBreakdown {
    fn from((path, s): (String, crate::spec::CoverageSummary)) -> Self {
        ModuleBreakdown {
            path,
            current: s.current,
            stale: s.stale,
            missing: s.missing,
            smelly: s.smelly,
            generations_remaining: s.generations_remaining,
            total: s.total(),
            freshness: s.freshness(),
            coverage: s.coverage_ratio(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct CoverageResponse {
    pub current: usize,
    pub stale: usize,
    pub missing: usize,
    /// Count of documents (of any status) carrying at least one quality
    /// smell — not exclusive with `current`.
    pub smelly: usize,
    /// Total `get_next_spec_task` → `submit_spec` cycles a full
    /// `/codeowl generate --all` run over this scope would spend — the
    /// `--budget=N` a complete generation needs. Sums every pending
    /// document's `generations`, so it counts uncovered symbols, not just
    /// files (`missing: 15` files can be 240+ generations).
    pub generations_remaining: usize,
    /// Of the specs that *exist* (current or stale), what fraction still
    /// match the code — `current / (current + stale)`, ignoring `missing`
    /// entirely. `1.0` when nothing has ever been generated (vacuously
    /// fresh — see `coverage` for "how much of the repo is documented at
    /// all"). Deliberately a separate number from `coverage`: "no spec
    /// yet" and "spec exists but is wrong" are different problems that
    /// need different fixes, and collapsing them hides which one a repo
    /// actually has.
    pub freshness: f64,
    /// What fraction of eligible nodes have *any* spec at all, current or
    /// stale — `(current + stale) / total`. The other axis from
    /// `freshness`: 100% coverage + 60% freshness means "everything's been
    /// written once, a lot needs re-running"; 60% coverage + 100%
    /// freshness means "nothing documented is wrong, there's just more
    /// left to write."
    pub coverage: f64,
    /// `freshness`, weighted by import fan-in instead of by item count — a
    /// stale file forty others import drags this down far more than a
    /// stale leaf utility does. Scoped to file-kind documents that
    /// actually have a spec (missing files don't count against it, same
    /// exclusion as `freshness`); falls back to the unweighted freshness
    /// over that same subset if nothing in scope has any fan-in at all.
    pub weighted_freshness: f64,
    /// The same counts as above, broken down by document kind — in
    /// `"file"`, `"rollup"`, `"feature"`, `"system"` order, omitting any
    /// kind with nothing in this scope. `by_kind`'s `"feature"` row's
    /// `total` is the full count of feature specs this repo will ever have
    /// (enumeration is deterministic — see `ARCHITECTURE.md`'s "Feature
    /// specs").
    pub by_kind: Vec<KindBreakdown>,
    /// The same counts broken down by directory instead of by kind, sorted
    /// by path. A directory's own rollup and the files inside it share one
    /// row — see `ModuleBreakdown`.
    pub by_module: Vec<ModuleBreakdown>,
    /// The 5 file-kind documents most worth regenerating, ranked by import
    /// fan-in (blast radius) rather than by `pending`'s generate-order
    /// tiering — "which stale documents would hurt the most if left wrong"
    /// rather than "which order to spend a budget on." Same "needs
    /// attention" criterion as `pending` (non-current, or current but
    /// smelly); rollups/features/system never appear here, only files.
    pub top_stale_by_impact: Vec<CoverageItemResponse>,
    /// Spec documents on disk whose target no longer exists in the graph at
    /// all — a deleted file, a directory that dropped below the rollup
    /// threshold, a removed feature entry point. Not counted in `coverage`/
    /// `freshness`/`total` at all (there's no eligible node left to score),
    /// and never appears in `pending` either — there's nothing for
    /// `/codeowl generate` to regenerate against. This is dead weight to
    /// delete, a genuinely different problem from "stale."
    pub orphaned: Vec<OrphanedSpecResponse>,
    /// Every document still needing attention — non-current, or
    /// current-but-smelly — in priority order. See `ARCHITECTURE.md`'s
    /// "Generation priority" and "Quality smells". **Paginated (M20)** at
    /// [`COVERAGE_PENDING_PAGE_SIZE`] entries per call — see `next_cursor`.
    pub pending: Vec<CoverageItemResponse>,
    /// `Some(cursor)` to pass back as `CoverageRequest::cursor` for the
    /// next page of `pending`, if there is one; `None` once `pending` has
    /// reached the end. Every other field in this response already covers
    /// the whole repo (or the whole `scope`) regardless of pagination —
    /// only `pending` itself is paged.
    pub next_cursor: Option<usize>,
    /// How many files sit under a known build-generated-source directory
    /// (M19 — e.g. Maven's `target/generated-sources`), and which
    /// directories were checked. `None` for a pack with no such
    /// convention (every pack but Java today) — the concept doesn't
    /// apply there, so there's nothing honest to report. `found: 0` on a
    /// pack that *does* declare them is the actionable signal: this repo
    /// could have build-generated entry points and none were found — has
    /// `mvn compile` (or equivalent) been run locally? See
    /// `setup/codeowl-generate.md` for why `mvn generate-sources` alone
    /// isn't enough, and why the Gradle case is still unverified.
    pub generated_sources: Option<GeneratedSourcesResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct GeneratedSourcesResponse {
    pub checked_dirs: Vec<String>,
    pub found: usize,
}

impl From<crate::spec::GeneratedSourcesSummary> for GeneratedSourcesResponse {
    fn from(s: crate::spec::GeneratedSourcesSummary) -> Self {
        GeneratedSourcesResponse {
            checked_dirs: s.checked_dirs,
            found: s.found,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct CallerInfo {
    pub from_file: String,
    pub imported_name: String,
}

/// `get_callers` result. Wrapped in a struct rather than returned as a
/// bare array: MCP requires a tool's `structuredContent` to be a JSON
/// object, and a spec-compliant client rejects a top-level array.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct CallersResponse {
    pub callers: Vec<CallerInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct CalleeInfo {
    pub specifier: String,
    pub imported_name: String,
    /// The imported symbol's stable id, when it resolved to one CodeOwl
    /// tracks. `None` covers both external packages and internal names
    /// M1 doesn't extract as symbols yet (a `type`/`interface`, a
    /// destructured const) — see M2's real-repo validation notes.
    pub resolved_id: Option<String>,
}

/// `get_callees` result — wrapped for the same reason as [`CallersResponse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct CalleesResponse {
    pub callees: Vec<CalleeInfo>,
}

/// `search_code` result — wrapped for the same reason as [`CallersResponse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SearchResponse {
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SpecResponse {
    pub id: String,
    /// One of `"missing"` | `"current"` | `"stale"` — see `ARCHITECTURE.md`'s
    /// "Ordering". `"stale"` means a spec exists but at least one of its
    /// inputs has moved since it was generated; the last-known-good
    /// `content` is still returned rather than withheld.
    pub status: String,
    pub signature: String,
    pub docstring: Option<String>,
    /// The LLM-written prose `submit_spec` persisted. `Some` for both
    /// `"current"` and `"stale"` (the last-known-good spec is always
    /// served — see "Ordering"'s read/write split); `None` only for
    /// `"missing"`.
    pub content: Option<String>,
    /// Which inputs moved since generation, deterministically named (e.g.
    /// `"source"`, `"changed:<dependency id>"`, `"added:<participant id>"`)
    /// — empty unless `status` is `"stale"`. Never requires an LLM to
    /// compute; see `ARCHITECTURE.md`'s "Ordering" ("plus which inputs
    /// moved (deterministic, off the graph)").
    pub changed: Vec<String>,
    /// Deterministic quality smells found in `content` itself (e.g.
    /// `"cop_out_phrase"`, `"suspiciously_short"`) — can be non-empty even
    /// when `status` is `"current"`, since hash-based staleness only
    /// verifies inputs haven't moved, never that the prose was ever
    /// meaningful. Always empty when `status` is `"missing"`. See
    /// `ARCHITECTURE.md`'s "Quality smells".
    pub smells: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GenerateTaskRequest {
    /// The `/codeowl generate <id>` target — a file id (a repo-relative
    /// path, e.g. "lib/utils.ts"), which may also be a feature entry point
    /// (a page or an orphan API route — see `features.rs`), a directory
    /// path (e.g. "lib") with >=2 spec-bearing files, or "system"/"." for
    /// the whole repo (every module, every feature, then the system spec).
    /// Stateless: safe to call repeatedly with the same target until it
    /// reports nothing left.
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct CoreSource {
    pub file: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct DependencyContext {
    pub id: String,
    /// The dependency's already-generated summary if it has a current
    /// spec, otherwise a deterministic stub (signature + docstring) — this
    /// task never triggers the dependency's own generation.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct TableContext {
    /// The table's schema node id, e.g. `"supabase/schema.sql::payments"`.
    pub id: String,
    /// `table(col, col, ...)` — the columns `schema.rs` parsed.
    pub columns: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct RollupFile {
    pub file: String,
    /// That file's own current `## Summary` prose — never its raw source
    /// (see "Bottom-up composition"). Only ever populated once the file
    /// itself is current; see `next_task_for_directory`.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpecTaskResponse {
    Symbol {
        id: String,
        signature: String,
        docstring: Option<String>,
        source: String,
        dependencies: Vec<String>,
        /// `Some` only for a reconciliation regeneration (M8's "Human
        /// corrections" case 4) — the human-edited `### Summary`/
        /// `### Behavior` prose still on record, to preserve whatever
        /// correction is still accurate rather than silently overwrite it.
        prior_summary: Option<String>,
        prior_behavior: Option<String>,
    },
    File {
        id: String,
        source: String,
        /// Same case-4 meaning as `Symbol::prior_summary`, for the file's
        /// own `## Summary`.
        prior: Option<String>,
    },
    /// Generated in one shot, unlike the bottom-up symbol-then-file chase
    /// above: a feature spec is a single document with a single
    /// `spec_hash`, so there's exactly one task, not several.
    Feature {
        /// `"feature:<slug>"` — pass this straight back as `submit_spec`'s
        /// `id`; it's never a real file/symbol id (see `submit_spec`'s own
        /// dispatch).
        id: String,
        entry_point: String,
        core_sources: Vec<CoreSource>,
        dependencies: Vec<DependencyContext>,
        /// SQL tables the core code queries via a resolved `.from("table")`
        /// (M10) — `id` is the table's schema node, `columns` its
        /// `table(col, col, ...)` shape. Ground the "Data touched" section
        /// in these instead of inferring column names from `.select()`.
        data: Vec<TableContext>,
    },
    /// Generated in one shot, like a feature spec: composed purely from
    /// its files' own already-generated summaries, never their raw source.
    /// Only ever produced once every file in `files` is itself current —
    /// `get_next_spec_task` walks a directory target's own files' bottom-up
    /// ladders first, the same "symbols before their file" order one level
    /// up: files before their directory's rollup.
    Rollup {
        /// `"rollup:<dir_path>"` — pass this straight back as
        /// `submit_spec`'s `id`; it's never a real file/symbol id.
        id: String,
        dir_path: String,
        files: Vec<RollupFile>,
    },
    /// Generated in one shot, like a feature or rollup spec: composed
    /// purely from every module's and every feature's own already-
    /// generated summary. Only ever produced once every module and every
    /// feature in the whole repo is itself current -- there is exactly
    /// one of these per repo.
    System {
        /// Always the literal `"system"` -- pass straight back as
        /// `submit_spec`'s `id`.
        id: String,
        modules: Vec<ModuleSummary>,
        features: Vec<FeatureSummary>,
    },
    /// Nothing left to generate for this target -- every document it
    /// covers already has a current spec. Serialized as `{"kind":"done"}`
    /// (a real object, unlike a bare `null`, which some MCP clients reject
    /// as an invalid `structuredContent`). The loop stops here.
    Done {},
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ModuleSummary {
    pub dir: String,
    /// That module's own current rollup `## Summary` prose.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct FeatureSummary {
    pub slug: String,
    /// That feature's own title plus its current `## Summary` prose.
    pub summary: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubmitSpecRequest {
    /// The id `get_next_spec_task` returned — a symbol id, a file id, the
    /// fixed id `"system"`, or (for a feature or rollup task) the
    /// `"feature:<slug>"`/`"rollup:<dir_path>"` id it reported.
    pub id: String,
    /// For a symbol task: markdown containing `### Summary` and
    /// `### Behavior` headings. For a file or rollup task: plain prose,
    /// becomes the `## Summary`. For a feature or system task: the whole
    /// document body, starting with a `# Title` line — see
    /// `ARCHITECTURE.md`'s "Feature specs"/"System spec shape" templates.
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SubmitSpecResponse {
    pub id: String,
    /// `None` for a feature, rollup, or system submission — none of the
    /// three has a single source hash: a feature has a participant map, a
    /// rollup has a per-file hash map, the system spec has both a module
    /// map and a feature map (see `get_spec`/the persisted frontmatter for
    /// any of them).
    pub source_hash: Option<String>,
    pub spec_hash: String,
}

#[derive(Clone)]
pub struct CodeOwlServer {
    /// Swapped wholesale by the file watcher as source files change
    /// mid-session (M9). Every request handler loads one consistent
    /// snapshot (`self.graph.load_full()`) up front rather than reading
    /// this field repeatedly — a `SymbolId` is only valid for the graph
    /// that produced it, so a handler must not straddle a swap.
    graph: Arc<ArcSwap<Graph>>,
    root: Arc<PathBuf>,
    tool_router: ToolRouter<Self>,
    /// The two `get_next_spec_task` size knobs — see
    /// `spec::LARGE_CONTAINER_BYTES_DEFAULT` /
    /// `spec::MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT` for why these need
    /// to be overridable per MCP client at all (a real user's class hit
    /// VS Code Copilot Chat's overflow threshold at a size well under the
    /// original hardcoded default). Explicit fields, not an env var read
    /// inside `spec.rs`, so the active values are visible in one place
    /// (the `codeowl serve` invocation, via `with_generation_limits`)
    /// rather than ambient process state — and so every existing test
    /// that builds a `CodeOwlServer` via `::new` keeps compiling
    /// unchanged, since these just default here.
    large_class_bytes: usize,
    max_spec_task_bytes: usize,
}

impl CodeOwlServer {
    pub fn new(root: PathBuf, graph: Graph) -> Self {
        Self {
            graph: Arc::new(ArcSwap::from_pointee(graph)),
            root: Arc::new(root),
            tool_router: Self::tool_router(),
            large_class_bytes: crate::spec::LARGE_CONTAINER_BYTES_DEFAULT,
            max_spec_task_bytes: crate::spec::MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT,
        }
    }

    /// Override the two `get_next_spec_task` size knobs from their
    /// compiled-in defaults — `codeowl serve`'s `--large-class-bytes`
    /// / `--max-spec-task-bytes` flags call this. `None` leaves the
    /// corresponding default in place, so a caller only needs to specify
    /// the one it actually wants to change.
    pub fn with_generation_limits(
        mut self,
        large_class_bytes: Option<usize>,
        max_spec_task_bytes: Option<usize>,
    ) -> Self {
        if let Some(v) = large_class_bytes {
            self.large_class_bytes = v;
        }
        if let Some(v) = max_spec_task_bytes {
            self.max_spec_task_bytes = v;
        }
        self
    }

    /// The shared graph cell for `watch::spawn` to publish reindexed
    /// graphs into. Cloning the `Arc` shares the same cell, so a swap on
    /// the watcher thread is immediately visible to every request handler.
    pub fn graph_store(&self) -> Arc<ArcSwap<Graph>> {
        Arc::clone(&self.graph)
    }

    /// The repo root this server was built against — for tests that need to
    /// plant a fixture under `docs/specs/` directly.
    #[cfg(test)]
    pub(crate) fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn not_found(id: &str) -> String {
        format!("no symbol with id {id:?}")
    }

    /// `get_next_spec_task`'s fallback once `target`'s own file/symbol
    /// tasks are exhausted: if `target` is a recognized feature entry
    /// point and its feature spec isn't current, return that as the next
    /// task; otherwise there's genuinely nothing left.
    ///
    /// **Several entry points can share one file** — several FastAPI
    /// routes decorated in one module (M17); the pilot's Next.js pages and
    /// routes never had this, one file per entry point. So this tries
    /// every entry on `target` in order and returns the first whose
    /// feature spec isn't current yet, rather than a single `.find()` that
    /// would collapse to the same (possibly already-current) entry
    /// regardless of which one is actually still missing — the M17
    /// dogfood bug where, once the first route's feature spec existed,
    /// every other route on that file reported `{"kind":"done"}`.
    ///
    /// This "first not-yet-current entry" walk is only correct for a bare
    /// file-path `target` (the intentional round-robin `codeowl-generate.md`
    /// documents: repeat against the same file to drain every feature it
    /// hosts). A `feature:<slug>` target must not land here — see
    /// `next_task_for_feature`, which resolves to that exact entry instead.
    fn next_feature_task_response(
        &self,
        graph: &Graph,
        target: &str,
    ) -> Result<Json<Option<SpecTaskResponse>>, String> {
        let entry_points = crate::features::enumerate_entry_points(graph);
        self.first_pending_feature_task(graph, entry_points.iter().filter(|e| e.file == target))
    }

    /// `feature:<slug>` targets resolve here, never through
    /// `next_task_for_target`/`next_feature_task_response`'s generic
    /// file-target walk. That walk returns the first not-yet-current entry
    /// point *on the file*, which is right for a bare file-path target but
    /// wrong for a `feature:<slug>` one: `codeowl-generate.md` documents
    /// `feature:<slug>` as "equivalent to naming the feature's entry
    /// point," so it must resolve to `entry`'s own task, or report done
    /// because `entry` itself is current — never a sibling's task, even
    /// one that sorts earlier and is still pending (the M17 dogfood bug:
    /// `feature:http-get-items` silently returned
    /// `feature:http-delete-items-id`'s task instead, because
    /// `enumerate_entry_points` sorts globally by slug id and the delete
    /// route sorts first).
    ///
    /// The file's own symbol/file ladder still runs first, same bottom-up
    /// order as every other target — a feature is never offered before its
    /// hosting file is itself current.
    fn next_task_for_feature(
        &self,
        graph: &Graph,
        entry: &crate::features::EntryPoint,
    ) -> Result<Json<Option<SpecTaskResponse>>, String> {
        let Some(file_id) = graph.find(&entry.file) else {
            return Ok(Json(None));
        };
        let task = crate::spec::next_task(graph, &self.root, file_id).map_err(|e| e.to_string())?;
        if let Some(task) = task {
            return Ok(Json(Some(self.spec_task_to_response(graph, task)?)));
        }
        self.first_pending_feature_task(graph, std::iter::once(entry))
    }

    /// The one loop `next_feature_task_response` (candidates: every entry
    /// sharing a file) and `next_task_for_feature` (candidates: exactly one
    /// entry) both need: try each candidate's feature task in order, return
    /// the first that isn't already current. Pulled out so a third
    /// target-resolution mode (a different candidate set, same "first
    /// not-yet-current" semantics) extends this instead of cloning the loop
    /// a third time — code-review finding, these two started as
    /// near-duplicate copies of it.
    fn first_pending_feature_task<'a>(
        &self,
        graph: &Graph,
        candidates: impl Iterator<Item = &'a crate::features::EntryPoint>,
    ) -> Result<Json<Option<SpecTaskResponse>>, String> {
        for entry in candidates {
            let Some(task) = crate::spec::next_feature_task(graph, &self.root, entry)
                .map_err(|e| e.to_string())?
            else {
                continue; // this entry's feature is already current
            };
            return Ok(Json(Some(Self::feature_task_response(task))));
        }
        Ok(Json(None))
    }

    /// Shared by `first_pending_feature_task`'s single return point: turns
    /// an assembled `FeatureTask` into the wire response.
    fn feature_task_response(task: crate::spec::FeatureTask) -> SpecTaskResponse {
        SpecTaskResponse::Feature {
            id: format!("feature:{}", task.slug),
            entry_point: task.entry_point,
            core_sources: task
                .core_sources
                .into_iter()
                .map(|(file, source)| CoreSource { file, source })
                .collect(),
            dependencies: task
                .dependencies
                .into_iter()
                .map(|(id, summary)| DependencyContext { id, summary })
                .collect(),
            data: task
                .data
                .into_iter()
                .map(|(id, columns)| TableContext { id, columns })
                .collect(),
        }
    }

    /// `get_next_spec_task`'s path when `target` doesn't resolve as a
    /// graph id at all (directories aren't graph nodes — see `ROADMAP.md`'s
    /// M6 scope): if it's a spec-bearing directory, walk its own files'
    /// bottom-up ladders first, then the rollup task itself once every file
    /// is current; otherwise there's genuinely nothing here.
    fn next_directory_task_response(
        &self,
        graph: &Graph,
        target: &str,
    ) -> Result<Json<Option<SpecTaskResponse>>, String> {
        if !crate::spec::directory_is_spec_bearing(graph, target) {
            return Ok(Json(None));
        }

        if let Some(task) = crate::spec::next_task_for_directory(graph, &self.root, target)
            .map_err(|e| e.to_string())?
        {
            return Ok(Json(Some(self.spec_task_to_response(graph, task)?)));
        }

        let Some(task) =
            crate::spec::next_rollup_task(graph, &self.root, target).map_err(|e| e.to_string())?
        else {
            return Ok(Json(None));
        };
        Ok(Json(Some(SpecTaskResponse::Rollup {
            id: format!("rollup:{}", task.dir_path),
            dir_path: task.dir_path,
            files: task
                .files
                .into_iter()
                .map(|(file, summary)| RollupFile { file, summary })
                .collect(),
        })))
    }

    /// Shared by `get_next_spec_task`'s direct-target path and its
    /// directory-target fallback: turns a bottom-up `SpecTask` (a symbol or
    /// a file, never a feature/rollup — those are assembled by their own
    /// callers) into the wire response, reading whatever source it needs
    /// off disk.
    fn spec_task_to_response(
        &self,
        graph: &Graph,
        task: crate::spec::SpecTask,
    ) -> Result<SpecTaskResponse, String> {
        Ok(match task {
            crate::spec::SpecTask::Symbol {
                id,
                signature,
                docstring,
                prior,
            } => {
                let sym_id = graph.find(&id).ok_or_else(|| Self::not_found(&id))?;
                let sym = graph
                    .get_symbol(sym_id)
                    .ok_or_else(|| Self::not_found(&id))?;
                let file_id = graph
                    .parent_id(sym_id)
                    .ok_or_else(|| Self::not_found(&id))?;
                let file = graph
                    .get_file(file_id)
                    .ok_or_else(|| Self::not_found(&id))?;
                // The symbol's own span plus any folded-in `impl` method
                // spans (M15) — see `spec::symbol_span_text`.
                let source = crate::spec::symbol_span_text(&self.root, graph, &file.id, sym)
                    .map_err(|e| e.to_string())?;
                // Scoped to what this symbol's own text names, and with
                // externals folded to one line — the same treatment the
                // rendered `### Depends on` section gets, so a Rust symbol
                // isn't handed a wall of `Vec`/`Result`/`BTreeMap`.
                let scoped = crate::spec::scoped_symbol_deps(graph, &file.id, source.as_str());
                let mut dependencies: Vec<String> = scoped
                    .resolved
                    .iter()
                    .map(|(target, specifier)| format!("{target} ({specifier})"))
                    .collect();
                if !scoped.externals.is_empty() {
                    dependencies.push(format!("externals: {}", scoped.externals.join(", ")));
                }
                // Dependency scanning above already ran against the true
                // full text -- only the payload actually handed to the
                // agent is reduced for a God-class Container (M18,
                // M16's headline finding), then hard-capped regardless
                // (a real MCP client's overflow threshold isn't known
                // precisely -- see `self.large_class_bytes` /
                // `self.max_spec_task_bytes`, both overridable via
                // `with_generation_limits`).
                let source = crate::spec::maybe_reduce_container_source(
                    sym,
                    graph,
                    source,
                    self.large_class_bytes,
                );
                let source = crate::spec::cap_generation_text(source, self.max_spec_task_bytes);
                SpecTaskResponse::Symbol {
                    id,
                    signature,
                    docstring,
                    source,
                    dependencies,
                    prior_summary: prior.as_ref().map(|p| p.summary.clone()),
                    prior_behavior: prior.map(|p| p.behavior),
                }
            }
            crate::spec::SpecTask::File { id, prior } => {
                let source =
                    std::fs::read_to_string(self.root.join(&id)).map_err(|e| e.to_string())?;
                // A `File` task had no size handling at all before this --
                // a large flat file with no single big class was
                // completely unprotected even after the Container fix.
                let source = crate::spec::cap_generation_text(source, self.max_spec_task_bytes);
                SpecTaskResponse::File { id, source, prior }
            }
        })
    }
}

#[tool_router]
impl CodeOwlServer {
    #[tool(
        description = "Look up one symbol's full record (signature, docstring, line range, hashes) by its stable id, e.g. \"lib/utils.ts::cn\"."
    )]
    async fn get_symbol(
        &self,
        Parameters(req): Parameters<IdRequest>,
    ) -> Result<Json<SymbolView>, String> {
        let graph = self.graph.load_full();
        let id = graph
            .find(&req.id)
            .ok_or_else(|| Self::not_found(&req.id))?;
        SymbolView::from_graph(&graph, id)
            .map(Json)
            .ok_or_else(|| Self::not_found(&req.id))
    }

    #[tool(
        description = "List every file that references this symbol: for a code symbol, the files that import it by name via a resolved reference edge (M2); for a SQL table node, the files with a `.from(\"table\")` call that resolves to it (M10)."
    )]
    async fn get_callers(
        &self,
        Parameters(req): Parameters<IdRequest>,
    ) -> Result<Json<CallersResponse>, String> {
        let graph = self.graph.load_full();
        let id = graph
            .find(&req.id)
            .ok_or_else(|| Self::not_found(&req.id))?;

        if graph
            .get_symbol(id)
            .is_some_and(|s| s.kind == SymbolKind::Schema)
        {
            let callers = graph
                .table_callers(&req.id)
                .into_iter()
                .map(|(from_file, table)| CallerInfo {
                    from_file,
                    imported_name: table,
                })
                .collect();
            return Ok(Json(CallersResponse { callers }));
        }

        let callers = graph
            .imports()
            .iter()
            .filter(|imp| imp.target == Some(id))
            .map(|imp| CallerInfo {
                from_file: imp.from_file.clone(),
                imported_name: imp.imported_name.clone(),
            })
            .collect();
        Ok(Json(CallersResponse { callers }))
    }

    #[tool(
        description = "List what the FILE containing this symbol imports. File-level granularity, not per-symbol: M2 resolves file-to-file reference edges, not call edges (see ROADMAP.md's M2 scope)."
    )]
    async fn get_callees(
        &self,
        Parameters(req): Parameters<IdRequest>,
    ) -> Result<Json<CalleesResponse>, String> {
        let graph = self.graph.load_full();
        let id = graph
            .find(&req.id)
            .ok_or_else(|| Self::not_found(&req.id))?;
        let file = graph
            .get_symbol(id)
            .ok_or_else(|| Self::not_found(&req.id))?
            .file
            .clone();
        let callees = graph
            .imports()
            .iter()
            .filter(|imp| imp.from_file == file)
            .map(|imp| CalleeInfo {
                specifier: imp.specifier.clone(),
                imported_name: imp.imported_name.clone(),
                resolved_id: imp.target.map(|t| graph.string_id(t).to_string()),
            })
            .collect();
        Ok(Json(CalleesResponse { callees }))
    }

    #[tool(
        description = "Get the spec for a symbol id, file id, 'feature:<slug>' id, 'rollup:<dir_path>' id, or the fixed id 'system'. Always a pure read -- never triggers generation (that's /codeowl generate, via get_next_spec_task/submit_spec). Returns status \"missing\" (no content) if nothing's been generated yet, \"current\" if the persisted spec's inputs all still match, or \"stale\" -- the last-known-good content, plus `changed` naming what moved -- if generation happened but the source (or something it depends on) has since changed."
    )]
    async fn get_spec(
        &self,
        Parameters(req): Parameters<IdRequest>,
    ) -> Result<Json<SpecResponse>, String> {
        fn missing(id: String, signature: String, docstring: Option<String>) -> SpecResponse {
            SpecResponse {
                id,
                status: "missing".to_string(),
                signature,
                docstring,
                content: None,
                changed: Vec::new(),
                smells: Vec::new(),
            }
        }

        let graph = self.graph.load_full();

        if req.id == "system" {
            let Some(spec) =
                crate::spec::read_system_spec(&self.root).map_err(|e| e.to_string())?
            else {
                return Ok(Json(missing(req.id, String::new(), None)));
            };
            let smells = crate::spec::body_smells(&spec.body);
            let current_modules = crate::spec::current_module_hashes(&graph, &self.root)
                .map_err(|e| e.to_string())?;
            let current_features = crate::spec::current_feature_hashes(&graph, &self.root)
                .map_err(|e| e.to_string())?;
            let mut current_all = current_modules;
            current_all.extend(current_features);
            let mut stored_all = spec.modules;
            stored_all.extend(spec.features);
            let changed = crate::spec::diff_hash_lists(&current_all, &stored_all);
            return Ok(Json(SpecResponse {
                id: req.id,
                status: if changed.is_empty() {
                    "current"
                } else {
                    "stale"
                }
                .to_string(),
                signature: String::new(),
                docstring: None,
                content: Some(spec.body),
                changed,
                smells,
            }));
        }
        if let Some(dir_path) = req.id.strip_prefix("rollup:") {
            let Some(spec) =
                crate::spec::read_rollup_spec(&self.root, dir_path).map_err(|e| e.to_string())?
            else {
                return Ok(Json(missing(req.id, String::new(), None)));
            };
            let smells = crate::spec::body_smells(&spec.body);
            let current = crate::spec::current_file_hashes(&graph, &self.root, dir_path)
                .map_err(|e| e.to_string())?;
            let changed = crate::spec::diff_hash_lists(&current, &spec.files);
            return Ok(Json(SpecResponse {
                id: req.id,
                status: if changed.is_empty() {
                    "current"
                } else {
                    "stale"
                }
                .to_string(),
                signature: String::new(),
                docstring: None,
                content: Some(spec.body),
                changed,
                smells,
            }));
        }
        if let Some(slug) = req.id.strip_prefix("feature:") {
            let Some(spec) =
                crate::spec::read_feature_spec(&self.root, slug).map_err(|e| e.to_string())?
            else {
                return Ok(Json(missing(req.id, String::new(), None)));
            };
            let smells = crate::spec::body_smells(&spec.body);
            let fm = crate::features::feature_model_for(&graph)
                .ok_or_else(|| Self::not_found(&req.id))?;
            let entry_points = fm.enumerate_entry_points(&graph);
            let entry = entry_points
                .iter()
                .find(|e| e.id == slug)
                .ok_or_else(|| Self::not_found(&req.id))?;
            let participants = crate::features::assemble_participants(&graph, fm, entry);
            let current = crate::spec::current_participant_hashes(&graph, &participants)
                .map_err(|e| e.to_string())?;
            let changed = crate::spec::diff_hash_lists(&current, &spec.participants);
            return Ok(Json(SpecResponse {
                id: req.id,
                status: if changed.is_empty() {
                    "current"
                } else {
                    "stale"
                }
                .to_string(),
                signature: String::new(),
                docstring: None,
                content: Some(spec.body),
                changed,
                smells,
            }));
        }

        let id = graph
            .find(&req.id)
            .ok_or_else(|| Self::not_found(&req.id))?;

        match graph.get(id) {
            crate::graph::Node::Symbol(symbol) => {
                let file_id = symbol.parent.ok_or_else(|| Self::not_found(&req.id))?;
                let file = graph
                    .get_file(file_id)
                    .ok_or_else(|| Self::not_found(&req.id))?;
                let existing =
                    crate::spec::read_file_spec(&self.root, &file.id).map_err(|e| e.to_string())?;
                let stored = existing.as_ref().and_then(|spec| {
                    spec.symbols
                        .iter()
                        .find(|(sid, _)| sid == &symbol.id)
                        .map(|(_, h)| h.clone())
                });
                let prose = existing.as_ref().and_then(|spec| {
                    spec.sections
                        .iter()
                        .find(|(sid, _)| sid == &symbol.id)
                        .map(|(_, p)| p.clone())
                });
                let (Some(stored), Some(prose)) = (stored, prose) else {
                    return Ok(Json(missing(
                        symbol.id.clone(),
                        symbol.signature.clone(),
                        symbol.docstring.clone(),
                    )));
                };
                let changed =
                    crate::spec::symbol_changes(&graph, &self.root, file_id, symbol, &stored);
                let mut smells = crate::spec::prose_smells(&prose.summary);
                smells.extend(crate::spec::prose_smells(&prose.behavior));
                smells.sort();
                smells.dedup();
                Ok(Json(SpecResponse {
                    id: symbol.id.clone(),
                    status: if changed.is_empty() {
                        "current"
                    } else {
                        "stale"
                    }
                    .to_string(),
                    signature: symbol.signature.clone(),
                    docstring: symbol.docstring.clone(),
                    content: Some(format!(
                        "### Summary\n{}\n\n### Behavior\n{}",
                        prose.summary, prose.behavior
                    )),
                    changed,
                    smells,
                }))
            }
            crate::graph::Node::File(file) => {
                let Some(spec) =
                    crate::spec::read_file_spec(&self.root, &file.id).map_err(|e| e.to_string())?
                else {
                    return Ok(Json(missing(file.id.clone(), String::new(), None)));
                };
                let changed = crate::spec::file_changes(&graph, id, &spec.file);
                let smells = crate::spec::file_spec_smells(&graph, &self.root, id, &spec);
                Ok(Json(SpecResponse {
                    id: file.id.clone(),
                    status: if changed.is_empty() {
                        "current"
                    } else {
                        "stale"
                    }
                    .to_string(),
                    signature: String::new(),
                    docstring: None,
                    content: Some(spec.file_summary.clone()),
                    changed,
                    smells,
                }))
            }
        }
    }

    #[tool(
        description = "The next unit /codeowl generate <target> still needs a spec for, bottom-up: a file's uncovered top-level symbols, then the file itself, then (if the target is also a feature entry point -- a page or an orphan API route) the feature spec. target may also be a `feature:<slug>` or `rollup:<dir>` id (as get_spec_coverage reports them), a directory path (e.g. \"lib\") with >=2 spec-bearing files -- walks each of its files' own symbol-then-file ladder first, then the directory's rollup spec once every file is current -- or \"system\"/\".\" to walk EVERY module and EVERY feature in the whole repo, then the one system spec once all of them are current. Returns `{\"kind\":\"done\"}` when the target isn't spec-bearing at all (e.g. a barrel file with no feature or rollup either) or everything on it is already current -- that's the generate loop's termination signal. Stateless: safe to call repeatedly with the same target."
    )]
    async fn get_next_spec_task(
        &self,
        Parameters(req): Parameters<GenerateTaskRequest>,
    ) -> Result<Json<SpecTaskResponse>, String> {
        let Json(task) = self.resolve_next_task(&req.target).await?;
        Ok(Json(task.unwrap_or(SpecTaskResponse::Done {})))
    }

    /// The dispatch behind `get_next_spec_task` — kept returning `Option`
    /// internally (the `next_*` helpers all speak it) and mapped to an
    /// explicit `Done` at the tool boundary above.
    async fn resolve_next_task(
        &self,
        target: &str,
    ) -> Result<Json<Option<SpecTaskResponse>>, String> {
        let graph = self.graph.load_full();
        if target == "system" || target == "." {
            return self.next_system_task_response(&graph);
        }
        // Accept the same id vocabulary get_spec_coverage emits and
        // get_spec/submit_spec take -- a budgeted --all walk hands these
        // straight back here.
        if let Some(slug) = target.strip_prefix("feature:") {
            let Some(entry) = crate::features::enumerate_entry_points(&graph)
                .into_iter()
                .find(|e| e.id == slug)
            else {
                return Ok(Json(None));
            };
            return self.next_task_for_feature(&graph, &entry);
        }
        if let Some(dir) = target.strip_prefix("rollup:") {
            return self.next_directory_task_response(&graph, dir);
        }
        self.next_task_for_target(&graph, target)
    }

    /// The bottom-up chase for a single file/symbol/directory target --
    /// shared by `get_next_spec_task`'s direct call and
    /// `next_system_task_response`'s per-module, per-feature walk.
    fn next_task_for_target(
        &self,
        graph: &Graph,
        target: &str,
    ) -> Result<Json<Option<SpecTaskResponse>>, String> {
        let Some(target_id) = graph.find(target) else {
            return self.next_directory_task_response(graph, target);
        };
        let task =
            crate::spec::next_task(graph, &self.root, target_id).map_err(|e| e.to_string())?;

        let Some(task) = task else {
            return self.next_feature_task_response(graph, target);
        };
        Ok(Json(Some(self.spec_task_to_response(graph, task)?)))
    }

    /// `get_next_spec_task`'s path for the whole-repo target (`"system"` or
    /// `"."`): walk every module directory's own chase (reusing
    /// `next_directory_task_response`, files then that directory's
    /// rollup), then every feature entry point's own chase (reusing
    /// `next_task_for_target`, its file then its feature) -- only once
    /// every one of those is current does the system task itself appear.
    fn next_system_task_response(
        &self,
        graph: &Graph,
    ) -> Result<Json<Option<SpecTaskResponse>>, String> {
        for dir in crate::spec::enumerate_modules(graph) {
            if let Json(Some(response)) = self.next_directory_task_response(graph, &dir)? {
                return Ok(Json(Some(response)));
            }
        }
        for entry in crate::features::enumerate_entry_points(graph) {
            if let Json(Some(response)) = self.next_task_for_target(graph, &entry.file)? {
                return Ok(Json(Some(response)));
            }
        }

        let Some(task) =
            crate::spec::next_system_task(graph, &self.root).map_err(|e| e.to_string())?
        else {
            return Ok(Json(None));
        };
        Ok(Json(Some(SpecTaskResponse::System {
            id: "system".to_string(),
            modules: task
                .modules
                .into_iter()
                .map(|(dir, summary)| ModuleSummary { dir, summary })
                .collect(),
            features: task
                .features
                .into_iter()
                .map(|(slug, summary)| FeatureSummary { slug, summary })
                .collect(),
        })))
    }

    #[tool(
        description = "Persist LLM-written spec prose for a symbol id, file id, 'feature:<slug>' id, 'rollup:<dir_path>' id, or the fixed id 'system' (all from get_next_spec_task). A symbol's content must contain '### Summary' and '### Behavior' headings; a file's or rollup's content is plain prose for its '## Summary'; a feature's or the system spec's content is the whole document starting with a '# Title' line. Never call this except as part of the get_next_spec_task -> write -> submit_spec loop /codeowl generate drives."
    )]
    async fn submit_spec(
        &self,
        Parameters(req): Parameters<SubmitSpecRequest>,
    ) -> Result<Json<SubmitSpecResponse>, String> {
        let graph = self.graph.load_full();
        if req.id == "system" {
            let spec = crate::spec::submit_system(&graph, &self.root, &req.content)
                .map_err(|e| e.to_string())?;
            return Ok(Json(SubmitSpecResponse {
                id: req.id,
                source_hash: None,
                spec_hash: spec.spec_hash,
            }));
        }
        if let Some(dir_path) = req.id.strip_prefix("rollup:") {
            let spec = crate::spec::submit_rollup(&graph, &self.root, dir_path, &req.content)
                .map_err(|e| e.to_string())?;
            return Ok(Json(SubmitSpecResponse {
                id: req.id,
                source_hash: None,
                spec_hash: spec.spec_hash,
            }));
        }
        if let Some(slug) = req.id.strip_prefix("feature:") {
            let entry_points = crate::features::enumerate_entry_points(&graph);
            let entry = entry_points
                .iter()
                .find(|e| e.id == slug)
                .ok_or_else(|| format!("no feature entry point with slug {slug:?}"))?;
            let spec = crate::spec::submit_feature(&graph, &self.root, &entry.id, &req.content)
                .map_err(|e| e.to_string())?;
            return Ok(Json(SubmitSpecResponse {
                id: req.id,
                source_hash: None,
                spec_hash: spec.spec_hash,
            }));
        }

        let hash = crate::spec::submit(&graph, &self.root, &req.id, &req.content)
            .map_err(|e| e.to_string())?;
        Ok(Json(SubmitSpecResponse {
            id: req.id,
            source_hash: Some(hash.source_hash),
            spec_hash: hash.spec_hash,
        }))
    }

    #[tool(
        name = "search_code",
        description = "Regex-search the repo's source files. Phase 1 implementation: embedded ripgrep, no index (see ARCHITECTURE.md's Storage section)."
    )]
    async fn search(
        &self,
        Parameters(req): Parameters<SearchRequest>,
    ) -> Result<Json<SearchResponse>, String> {
        crate::search::search_code(&self.root, &req.query)
            .map(|matches| Json(SearchResponse { matches }))
            .map_err(|e| e.to_string())
    }

    #[tool(
        description = "Coverage of the repo's spec inventory -- every file/rollup/feature/the system spec that the granularity rules say should exist -- broken down current/stale/missing/smelly, both overall and via `by_kind` (per document kind -- `by_kind`'s \"feature\" row's `total` is the full count of feature specs this repo will ever have) and `by_module` (per directory -- a directory's own rollup and the files inside it share one row). `coverage` and `freshness` are two DIFFERENT axes, not one score: `coverage` is what fraction of eligible nodes have any spec at all (current or stale); `freshness` is, of the specs that exist, what fraction still match the code (ignores `missing` entirely). A repo can be 100% covered and 60% fresh (needs regeneration) or 60% covered and 100% fresh (just isn't fully documented yet) -- read them separately. `weighted_freshness` is `freshness` weighted by import fan-in instead of item count, so a stale file forty others import counts far more than a stale leaf utility, and `top_stale_by_impact` is the 5 file documents that same weighting says matter most right now (ranked by fan-in, not generate order -- for \"what should I fix first\", not \"what would a budgeted run spend on first\"). `orphaned` lists spec documents (or, for `kind: \"symbol\"`, sections within an otherwise-fine file spec) whose target no longer exists in the graph at all (a deleted file, a directory that dropped below the rollup threshold, a removed feature entry point, a deleted function whose file is still current) -- these are NOT counted in coverage/freshness/total and never appear in `pending`, since there's nothing left for `/codeowl generate` to regenerate; they're dead weight to delete (a `\"symbol\"` entry prunes itself automatically next time that file is regenerated for any other reason). `generations_remaining` is the total get_next_spec_task/submit_spec cycles a full `/codeowl generate --all` run would spend (the real `--budget=N` for a complete pass -- it counts uncovered symbols, so a single missing file is often 20+); each `pending` entry, and each `by_kind`/`by_module` row, carries its own `generations_remaining` share (and its own `coverage`/`freshness`). `pending` lists every document still needing attention (non-current, OR current but flagged by a deterministic quality check -- see `smells`), its `id` ready to pass straight to get_next_spec_task/get_spec, in the exact order a budgeted `/codeowl generate --all --budget=N` run should spend on: high-fan-in files first, then feature specs, then the long tail of files, then rollups, then the system spec last. `pending` is paginated (M20) -- up to 50 entries per call; if `next_cursor` comes back non-null, pass it as this call's `cursor` to fetch the next page, repeating until `next_cursor` is null. Every OTHER field (the counts, `by_kind`, `by_module`, `top_stale_by_impact`, `orphaned`) always covers the whole scope regardless of pagination -- only `pending` itself is paged, since it's the one field that grows unboundedly with repo size (confirmed real: 626 files on a real Java library serialized `pending` alone to 95 KB, over the MCP result limit). Optionally narrow the file/rollup portion to a directory prefix via `scope` -- features and the system spec are always repo-wide. `generated_sources` (M19) reports how many files sit under a known build-generated-source directory (e.g. Maven's `target/generated-sources`) and which directories were checked -- `null` for a pack with no such convention (every pack but Java today), or `{checked_dirs, found}` for one that has it. `found: 0` is the actionable signal on a repo that could have build-generated entry points: has `mvn compile` (or equivalent) been run locally?"
    )]
    async fn get_spec_coverage(
        &self,
        Parameters(req): Parameters<CoverageRequest>,
    ) -> Result<Json<CoverageResponse>, String> {
        let graph = self.graph.load_full();
        let items = crate::spec::coverage(&graph, &self.root, req.scope.as_deref())
            .map_err(|e| e.to_string())?;
        let summary = crate::spec::summarize(&items);
        let weighted_freshness = crate::spec::weighted_freshness(&items);
        let by_kind = crate::spec::by_kind(&items)
            .into_iter()
            .map(KindBreakdown::from)
            .collect();
        let by_module = crate::spec::by_module(&items)
            .into_iter()
            .map(ModuleBreakdown::from)
            .collect();
        let top_stale_by_impact = crate::spec::top_stale_by_impact(&items, 5)
            .into_iter()
            .map(CoverageItemResponse::from)
            .collect();
        let orphaned = crate::spec::find_orphaned_specs(&graph, &self.root, req.scope.as_deref())
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(OrphanedSpecResponse::from)
            .collect();
        let pending_all = crate::spec::prioritize(items, &graph);
        let total_pending = pending_all.len();
        let cursor = req.cursor.unwrap_or(0).min(total_pending);
        let next_cursor = (cursor + COVERAGE_PENDING_PAGE_SIZE < total_pending)
            .then_some(cursor + COVERAGE_PENDING_PAGE_SIZE);
        let pending = pending_all
            .into_iter()
            .skip(cursor)
            .take(COVERAGE_PENDING_PAGE_SIZE)
            .map(CoverageItemResponse::from)
            .collect();
        let generated_sources =
            crate::spec::generated_sources_summary(&graph).map(GeneratedSourcesResponse::from);
        Ok(Json(CoverageResponse {
            current: summary.current,
            stale: summary.stale,
            missing: summary.missing,
            smelly: summary.smelly,
            generations_remaining: summary.generations_remaining,
            freshness: summary.freshness(),
            coverage: summary.coverage_ratio(),
            weighted_freshness,
            by_kind,
            by_module,
            top_stale_by_impact,
            orphaned,
            pending,
            next_cursor,
            generated_sources,
        }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for CodeOwlServer {
    fn get_info(&self) -> ServerInfo {
        // ServerInfo is #[non_exhaustive] -- built from Default, then its
        // public fields set individually, rather than a struct literal.
        let mut info = ServerInfo::default();
        info.protocol_version = ProtocolVersion::LATEST;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(
            "CodeOwl is a read-only structural index of this repository -- reach for it before \
             grep/file-reading on \"how is this wired\" questions. get_symbol gives a definition \
             and signature; get_callers / get_callees give the reference graph (\"what breaks if \
             I change this\"); search_code is a plain regex sweep (no semantic search in Phase \
             1). If the repo has a SQL schema, its CREATE TABLEs are indexed as `table` nodes \
             (id \"<schema-file>::<table>\"): get_symbol lists a table's columns, and get_callers \
             on it lists the files with a `.from(\"<table>\")` query against it. \
             The index tracks the working tree live -- files you edit during this session \
             are re-parsed within about a second, no restart needed. \
             get_spec returns an LLM-authored prose spec for a symbol id, a file path, \
             \"feature:<slug>\" (a cross-cutting flow the import graph alone can't see, e.g. \
             UI -> API route -> DB), \"rollup:<dir>\", or \"system\": status \"missing\" means \
             none exists yet, \"stale\" means the code moved since it was written (the last-good \
             text is still returned, with `changed` naming what moved), and a non-empty `smells` \
             list means a deterministic quality check distrusts the prose even though its hashes \
             still match. get_spec_coverage lists everything missing/stale/smelly in priority \
             order. To write or refresh specs, use the /codeowl-generate slash command (target a \
             file, a directory, \"system\", or \"--all [--budget=N]\") -- it drives the \
             get_next_spec_task -> submit_spec loop, which a normal consuming session otherwise \
             never touches."
                .to_string(),
        );
        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a small in-memory server the same way `main.rs`'s `serve`
    /// subcommand will: extract, resolve, wrap in a `Graph`. Files are
    /// still written to a real temp dir because `oxc_resolver` needs real
    /// paths on disk to resolve against (see `resolve.rs`'s own tests).
    ///
    /// The directory name includes a per-call counter, not just the pid:
    /// `#[tokio::test]`s in this module run concurrently in the same
    /// process, so a pid-only path let two tests race to write different
    /// content to the same "a.ts" and silently corrupt each other's fixture.
    fn test_server(files: &[(&str, &str)]) -> CodeOwlServer {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("codeowl-mcp-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        rebuild_server(dir, files)
    }

    /// Write `files` into `dir` (creating it if new, overwriting in place if
    /// not) and build a fresh server/graph against it. Reusing the same
    /// `dir` across two calls is what an M7 staleness test needs: the
    /// second call's graph reflects the "edit," while `docs/specs/` from
    /// the first call is still sitting on disk underneath it, exactly like
    /// a real edit-then-re-run-generate session.
    fn rebuild_server(dir: std::path::PathBuf, files: &[(&str, &str)]) -> CodeOwlServer {
        for (rel, content) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
        }
        let graph = crate::index::RepoIndex::build(&dir)
            .unwrap()
            .rebuild()
            .unwrap();
        CodeOwlServer::new(dir, graph)
    }

    #[tokio::test]
    async fn get_symbol_reflects_a_hot_swapped_graph() {
        // The M9 in-session model: the file watcher reindexes an edited
        // file and swaps a fresh graph into the server's cell, and the very
        // next request sees it -- no restart.
        let server = test_server(&[("a.ts", "export function f(): number { return 1; }\n")]);
        let root = server.root.to_path_buf();
        let before = server
            .get_symbol(Parameters(IdRequest {
                id: "a.ts::f".into(),
            }))
            .await
            .unwrap()
            .0
            .source_hash;

        let mut index = crate::index::RepoIndex::build(&root).unwrap();
        std::fs::write(
            root.join("a.ts"),
            "export function f(): number { return 2; }\n",
        )
        .unwrap();
        let (rebuilt, _) = index
            .apply_changes(&[root.join("a.ts")])
            .unwrap()
            .expect("a real edit rebuilds");
        server.graph_store().store(std::sync::Arc::new(rebuilt));

        let after = server
            .get_symbol(Parameters(IdRequest {
                id: "a.ts::f".into(),
            }))
            .await
            .unwrap()
            .0
            .source_hash;
        assert_ne!(
            before, after,
            "get_symbol must reflect the swapped-in graph"
        );
    }

    #[tokio::test]
    async fn get_symbol_returns_the_full_record() {
        let server = test_server(&[("a.ts", "export function double(x: number) {}\n")]);
        let result = server
            .get_symbol(Parameters(IdRequest {
                id: "a.ts::double".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(result.0.id, "a.ts::double");
        assert!(result.0.is_exported);
    }

    #[tokio::test]
    async fn get_symbol_on_unknown_id_is_an_error() {
        let server = test_server(&[("a.ts", "export function double(x: number) {}\n")]);
        let result = server
            .get_symbol(Parameters(IdRequest {
                id: "a.ts::nope".to_string(),
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_callers_lists_the_importing_file() {
        let server = test_server(&[
            ("a.ts", "import { helper } from './b';\n"),
            ("b.ts", "export function helper(): void {}\n"),
        ]);
        let result = server
            .get_callers(Parameters(IdRequest {
                id: "b.ts::helper".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(
            result.0.callers,
            vec![CallerInfo {
                from_file: "a.ts".to_string(),
                imported_name: "helper".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn get_callees_lists_the_files_imports_with_resolution() {
        // Query via "a.ts::marker" (not "helper") to confirm callees is
        // keyed off the containing FILE, not the queried symbol itself.
        let server = test_server(&[
            (
                "a.ts",
                "export const marker = 1;\nimport { helper } from './b';\nimport { z } from 'zod';\n",
            ),
            ("b.ts", "export function helper(): void {}\n"),
        ]);
        let result = server
            .get_callees(Parameters(IdRequest {
                id: "a.ts::marker".to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(result.0.callees.len(), 2);
        let helper = result
            .0
            .callees
            .iter()
            .find(|c| c.imported_name == "helper")
            .unwrap();
        assert!(
            helper.resolved_id.is_some(),
            "internal import should resolve"
        );
        let external = result
            .0
            .callees
            .iter()
            .find(|c| c.imported_name == "z")
            .unwrap();
        assert_eq!(
            external.resolved_id, None,
            "external package should not resolve"
        );
    }

    #[tokio::test]
    async fn get_spec_reports_missing_when_nothing_generated_yet() {
        let server = test_server(&[(
            "a.ts",
            "/** Doubles a number. */\nexport function double(x: number) {}\n",
        )]);
        let result = server
            .get_spec(Parameters(IdRequest {
                id: "a.ts::double".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(result.0.status, "missing");
        assert_eq!(result.0.docstring.as_deref(), Some("Doubles a number."));
        assert_eq!(result.0.content, None);
    }

    // --- M8: completeness & correction mechanics ------------------------

    #[tokio::test]
    async fn a_cop_out_symbol_spec_is_flagged_smelly_via_get_spec_and_coverage_even_though_current()
    {
        let server = test_server(&[("a.ts", "export function one(): void {}\n")]);
        // A hash-current symbol spec with a cop-out Behavior, planted
        // directly: `submit_spec` rejects such prose now, but a pre-fix
        // cache can still hold it and the MCP surface must still flag it.
        {
            let graph = server.graph_store().load_full();
            crate::spec::plant_smelly_symbol_spec(
                &graph,
                server.root(),
                "a.ts::one",
                "One does its one small job.",
                "See the source for details.",
            );
        }
        // Give the file itself a clean summary so the whole file's own
        // status is genuinely "current" -- isolating this test to the
        // "current but smelly because of one bad symbol" case, not also
        // exercising "the file's own summary was never submitted."
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "a.ts".to_string(),
                content: "A small file with one exported helper function.".to_string(),
            }))
            .await
            .unwrap();

        let spec = server
            .get_spec(Parameters(IdRequest {
                id: "a.ts::one".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(
            spec.0.status, "current",
            "hashes match -- nothing has moved"
        );
        assert_eq!(spec.0.smells, vec!["cop_out_phrase"]);

        let coverage = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: None,
                cursor: None,
            }))
            .await
            .unwrap();
        let item = coverage
            .0
            .pending
            .iter()
            .find(|i| i.id == "a.ts")
            .expect("the smelly file should still show up in pending despite being current");
        assert_eq!(item.status, "current");
        assert!(!item.smells.is_empty());
        assert!(coverage.0.smelly >= 1);
    }

    #[tokio::test]
    async fn get_spec_coverage_orders_pending_items_and_a_budgeted_walk_stops_early() {
        let server = test_server(&[
            ("util.ts", "export function helper(): void {}\n"),
            (
                "a.ts",
                "import { helper } from './util';\nexport function useA() {\n  helper();\n}\n",
            ),
            (
                "b.ts",
                "import { helper } from './util';\nexport function useB() {\n  helper();\n}\n",
            ),
        ]);

        // scope: Some("") matches every file/rollup path (every string
        // starts with "") but -- per `coverage`'s own doc comment --
        // excludes the always-present, repo-wide feature/system entries
        // whenever a scope is given at all. That's used deliberately here
        // to keep this test focused on file fan-in prioritization, not
        // system-spec content (covered by its own tests).
        let coverage = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: Some(String::new()),
                cursor: None,
            }))
            .await
            .unwrap();
        assert_eq!(coverage.0.missing, 3);
        assert_eq!(coverage.0.current, 0);
        assert_eq!(coverage.0.stale, 0);
        // Three missing files, one exported symbol each -> 3 symbol + 3
        // file generations. `generations_remaining` counts cycles, not
        // documents, and equals the sum of the per-item shares.
        assert_eq!(coverage.0.generations_remaining, 6);
        assert_eq!(
            coverage.0.generations_remaining,
            coverage
                .0
                .pending
                .iter()
                .map(|i| i.generations)
                .sum::<usize>()
        );
        assert!(coverage.0.pending.iter().all(|i| i.generations == 2));
        let ids: Vec<&str> = coverage.0.pending.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["util.ts", "a.ts", "b.ts"],
            "util.ts has fan-in 2 (imported by both a.ts and b.ts), so it's spent first"
        );
        assert_eq!(coverage.0.pending[0].fan_in, 2);

        // Simulate a budget=2 run: walk get_next_spec_task/submit_spec for
        // the first 2 pending targets only (draining each target's own
        // symbol-then-file ladder), then stop.
        let mut generations = 0;
        for item in coverage.0.pending.iter().take(2) {
            loop {
                let task = server
                    .get_next_spec_task(Parameters(GenerateTaskRequest {
                        target: item.id.clone(),
                    }))
                    .await
                    .unwrap()
                    .0;
                if matches!(task, SpecTaskResponse::Done {}) {
                    break;
                }
                generations += 1;
                let id = match &task {
                    SpecTaskResponse::Symbol { id, .. } => id.clone(),
                    SpecTaskResponse::File { id, .. } => id.clone(),
                    other => panic!("expected a Symbol or File task, got {other:?}"),
                };
                let content = if matches!(task, SpecTaskResponse::Symbol { .. }) {
                    "### Summary\nDoes one small, specific job.\n### Behavior\nRuns without any side effects.\n".to_string()
                } else {
                    "A small file of helper functions.".to_string()
                };
                server
                    .submit_spec(Parameters(SubmitSpecRequest { id, content }))
                    .await
                    .unwrap();
            }
        }
        assert_eq!(
            generations, 4,
            "util.ts and a.ts each need a symbol + file generation"
        );

        let after = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: Some(String::new()),
                cursor: None,
            }))
            .await
            .unwrap();
        assert_eq!(after.0.current, 2);
        assert_eq!(after.0.missing, 1);
        assert_eq!(
            after
                .0
                .pending
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["b.ts"],
            "budget was spent on the first 2 priority items only -- b.ts is untouched"
        );
    }

    #[tokio::test]
    async fn get_spec_coverage_breaks_down_by_kind_and_by_module() {
        let server = test_server(&[
            ("index.ts", "export function root(): void {}\n"),
            ("lib/email/send.ts", "export function send(): void {}\n"),
            ("lib/email/queue.ts", "export function queue(): void {}\n"),
        ]);

        let coverage = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: None,
                cursor: None,
            }))
            .await
            .unwrap()
            .0;

        // Whole-repo call -> feature/system rows exist too, alongside file
        // and (once lib/email has >=2 spec-bearing files) rollup.
        let kinds: Vec<&str> = coverage.by_kind.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(
            kinds,
            vec!["file", "rollup", "system"],
            "no feature entry points in this fixture, so \"feature\" is omitted, not zeroed"
        );
        let file_kind = coverage.by_kind.iter().find(|b| b.kind == "file").unwrap();
        assert_eq!(file_kind.missing, 3);
        assert_eq!(file_kind.total, 3);

        let paths: Vec<&str> = coverage.by_module.iter().map(|b| b.path.as_str()).collect();
        assert_eq!(paths, vec![".", "lib/email"]);
        let email_module = coverage
            .by_module
            .iter()
            .find(|b| b.path == "lib/email")
            .unwrap();
        assert_eq!(
            email_module.total, 3,
            "the two files plus their own rollup, bucketed together"
        );
        assert_eq!(email_module.missing, 3);

        // Every by_kind/by_module bucket's generations_remaining must sum
        // back to the same total the flat top-level field reports -- two
        // different groupings over the same underlying items.
        let by_kind_total: usize = coverage
            .by_kind
            .iter()
            .map(|b| b.generations_remaining)
            .sum();
        assert_eq!(by_kind_total, coverage.generations_remaining);
    }

    #[tokio::test]
    async fn get_spec_coverage_reports_generated_sources_for_java_only() {
        let server = test_server(&[("src/main/java/org/acme/App.java", "public class App {}\n")]);
        let coverage = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: None,
                cursor: None,
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(
            coverage.generated_sources,
            Some(GeneratedSourcesResponse {
                checked_dirs: vec![
                    "target/generated-sources".to_string(),
                    "build/generated".to_string()
                ],
                found: 0,
            }),
            "no generated files present, but Java always declares the convention"
        );

        let server = test_server(&[
            ("src/main/java/org/acme/App.java", "public class App {}\n"),
            (
                "target/generated-sources/foo/HeroesResource.java",
                "public interface HeroesResource { void getAllHeroes(); }\n",
            ),
        ]);
        let coverage = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: None,
                cursor: None,
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(coverage.generated_sources.unwrap().found, 1);
    }

    #[tokio::test]
    async fn get_spec_coverage_omits_generated_sources_for_a_non_java_pack() {
        let server = test_server(&[("a.ts", "export function f(): void {}\n")]);
        let coverage = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: None,
                cursor: None,
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(coverage.generated_sources, None);
    }

    #[tokio::test]
    async fn get_spec_coverage_reports_coverage_and_freshness_as_two_separate_axes() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-mcp-freshness-{}-1", std::process::id()));
        let v1 = "export function add(a: number, b: number): number {\n  return a + b;\n}\n";
        let server = rebuild_server(
            dir.clone(),
            &[("a.ts", v1), ("b.ts", "export function noop(): void {}\n")],
        );

        // Nothing generated yet: coverage is 0 (nothing documented), but
        // freshness and weighted_freshness are vacuously 1.0 -- there's
        // nothing stale to report when nothing exists yet.
        let before = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: Some(String::new()),
                cursor: None,
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(before.coverage, 0.0);
        assert_eq!(before.freshness, 1.0);
        assert_eq!(before.weighted_freshness, 1.0);

        // Fully generate a.ts only.
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "a.ts::add".to_string(),
                content: "### Summary\nAdds together two given numbers.\n### Behavior\nReturns the sum of its two arguments.\n"
                    .to_string(),
            }))
            .await
            .unwrap();
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "a.ts".to_string(),
                content: "A small numeric addition helper module.".to_string(),
            }))
            .await
            .unwrap();

        let after_one_current = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: Some(String::new()),
                cursor: None,
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(after_one_current.coverage, 0.5, "1 of 2 files documented");
        assert_eq!(
            after_one_current.freshness, 1.0,
            "the only spec that exists is current"
        );
        assert_eq!(after_one_current.weighted_freshness, 1.0);

        // Now edit a.ts's source without regenerating -- its spec is still
        // "documented" (coverage unaffected) but no longer matches the code
        // (freshness must drop; coverage must not).
        let v2 = "export function add(a: number, b: number, c: number): number {\n  return a + b + c;\n}\n";
        let server = rebuild_server(dir.clone(), &[("a.ts", v2)]);

        let after_stale = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: Some(String::new()),
                cursor: None,
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(
            after_stale.coverage, 0.5,
            "still 1 of 2 files documented -- a stale spec is still a spec"
        );
        assert_eq!(
            after_stale.freshness, 0.0,
            "the one spec that exists no longer matches its code"
        );
        assert_eq!(after_stale.weighted_freshness, 0.0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn get_spec_coverage_ranks_top_stale_by_impact_by_fan_in() {
        let server = test_server(&[
            ("util.ts", "export function helper(): void {}\n"),
            (
                "a.ts",
                "import { helper } from './util';\nexport function useA() {\n  helper();\n}\n",
            ),
            (
                "b.ts",
                "import { helper } from './util';\nexport function useB() {\n  helper();\n}\n",
            ),
        ]);

        let coverage = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: Some(String::new()),
                cursor: None,
            }))
            .await
            .unwrap()
            .0;
        let ids: Vec<&str> = coverage
            .top_stale_by_impact
            .iter()
            .map(|i| i.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["util.ts", "a.ts", "b.ts"],
            "util.ts has the highest fan-in (imported by both a.ts and b.ts)"
        );
        assert_eq!(coverage.top_stale_by_impact[0].fan_in, 2);
    }

    #[tokio::test]
    async fn get_spec_coverage_paginates_pending_and_a_cursor_reaches_the_rest() {
        // M20: `pending` on a large repo (626 files on real commons-lang)
        // serializes to 95 KB, over the MCP result limit -- confirmed real,
        // not hypothetical. Build enough files to span two pages of
        // COVERAGE_PENDING_PAGE_SIZE and walk the cursor to the end.
        let file_count = COVERAGE_PENDING_PAGE_SIZE + 7;
        let owned: Vec<(String, String)> = (0..file_count)
            .map(|i| {
                (
                    format!("f{i:03}.ts"),
                    format!("export function f{i:03}(): void {{}}\n"),
                )
            })
            .collect();
        let files: Vec<(&str, &str)> = owned
            .iter()
            .map(|(p, s)| (p.as_str(), s.as_str()))
            .collect();
        let server = test_server(&files);

        let first = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: None,
                cursor: None,
            }))
            .await
            .unwrap()
            .0;
        // +1 throughout: the always-present "system" entry (missing until
        // anything is generated) is itself one more pending item beyond
        // the file_count files -- confirmed via the by_kind breakdown
        // elsewhere in this test module, not assumed.
        let total_pending = file_count + 1;
        assert_eq!(first.pending.len(), COVERAGE_PENDING_PAGE_SIZE);
        assert_eq!(first.next_cursor, Some(COVERAGE_PENDING_PAGE_SIZE));
        // Every other field already covers the whole repo, unaffected by
        // pagination -- the one thing this feature must not do is make
        // the counts look like only the first page exists.
        assert_eq!(first.missing, total_pending);

        let second = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: None,
                cursor: first.next_cursor,
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(
            second.pending.len(),
            total_pending - COVERAGE_PENDING_PAGE_SIZE
        );
        assert_eq!(second.next_cursor, None, "the last page reports no more");
        assert_eq!(
            second.missing, total_pending,
            "unpaginated fields stay whole-repo on every page, not just the first"
        );

        // No id repeated across pages, and together they cover everything.
        let mut all_ids: Vec<&str> = first
            .pending
            .iter()
            .chain(&second.pending)
            .map(|i| i.id.as_str())
            .collect();
        all_ids.sort_unstable();
        all_ids.dedup();
        assert_eq!(
            all_ids.len(),
            total_pending,
            "every pending document appears exactly once across pages"
        );
    }

    #[tokio::test]
    async fn get_spec_coverage_reports_a_deleted_files_lingering_spec_as_orphaned() {
        let dir = std::env::temp_dir().join(format!("codeowl-mcp-orphan-{}-1", std::process::id()));
        let server = rebuild_server(
            dir.clone(),
            &[
                ("a.ts", "export function one(): void {}\n"),
                ("b.ts", "export function two(): void {}\n"),
            ],
        );
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "a.ts::one".to_string(),
                content: "### Summary\nDoes a small, specific job.\n### Behavior\nRuns without side effects.\n".to_string(),
            }))
            .await
            .unwrap();
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "a.ts".to_string(),
                content: "A small file with one exported helper.".to_string(),
            }))
            .await
            .unwrap();

        // Delete a.ts entirely and rebuild -- its spec is now dead weight,
        // not merely stale (there's nothing left in the graph to
        // regenerate it against).
        std::fs::remove_file(dir.join("a.ts")).unwrap();
        let server = rebuild_server(dir.clone(), &[]);

        let coverage = server
            .get_spec_coverage(Parameters(CoverageRequest {
                scope: None,
                cursor: None,
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(
            coverage.orphaned,
            vec![OrphanedSpecResponse {
                id: "a.ts".to_string(),
                kind: "file".to_string(),
                path: "docs/specs/a.ts.md".to_string(),
            }]
        );
        assert!(
            coverage.pending.iter().all(|i| i.id != "a.ts"),
            "an orphan must never appear in pending -- there's nothing to generate"
        );
        // b.ts and the (never-submitted) system spec are both "missing" --
        // not orphaned, since both are still real, eligible graph entries.
        // Neither the orphan count nor `missing` overlap with the other.
        assert_eq!(coverage.missing, 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn human_edited_spec_surfaces_as_a_reconciliation_task_via_the_mcp_surface() {
        let dir = std::env::temp_dir().join(format!("codeowl-mcp-human-{}-1", std::process::id()));
        let source_v1 = "export function one(): void {}\n";
        let server_v1 = rebuild_server(dir.clone(), &[("a.ts", source_v1)]);
        server_v1
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "a.ts::one".to_string(),
                content: "### Summary\nThis is the original summary.\n### Behavior\nThis is the original behavior.\n"
                    .to_string(),
            }))
            .await
            .unwrap();

        // A human hand-edits the spec file directly. Deliberately no
        // get_next_spec_task call in between here and the source edit
        // below -- that's the one thing that would silently reconcile
        // case 3 and "absorb" this edit as the new baseline before the
        // source change ever happens, which would turn this into a plain
        // case-2 regeneration instead of the case-4 this test is for.
        let spec_path = dir.join("docs/specs/a.ts.md");
        let content = std::fs::read_to_string(&spec_path).unwrap();
        std::fs::write(
            &spec_path,
            content.replace(
                "This is the original summary.",
                "A careful human-corrected summary here.",
            ),
        )
        .unwrap();

        // The source ALSO changes -- a fresh server against the same dir
        // should surface a reconciliation task carrying the human's edit
        // forward.
        let source_v2 = "export function one(): void {\n  console.log('changed');\n}\n";
        let server_v2 = rebuild_server(dir.clone(), &[("a.ts", source_v2)]);
        let task = server_v2
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "a.ts".to_string(),
            }))
            .await
            .unwrap()
            .0;
        let SpecTaskResponse::Symbol {
            prior_summary,
            prior_behavior,
            ..
        } = task
        else {
            panic!("expected a Symbol task");
        };
        assert_eq!(
            prior_summary.as_deref(),
            Some("A careful human-corrected summary here.")
        );
        assert_eq!(
            prior_behavior.as_deref(),
            Some("This is the original behavior.")
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    // --- M7: staleness & invalidation end-to-end -----------------------

    #[tokio::test]
    async fn dependency_signature_change_stales_importer_but_implementation_change_does_not() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-mcp-staletest-{}-1", std::process::id()));
        let math_v1 = "export function add(a: number, b: number): number {\n  return a + b;\n}\n";
        let user = "import { add } from './math';\n\nexport function sumThree(a: number, b: number, c: number): number {\n  return add(a, b) + c;\n}\n";
        let other = "export function noop(): void {}\n";

        let server = rebuild_server(
            dir.clone(),
            &[("math.ts", math_v1), ("user.ts", user), ("other.ts", other)],
        );
        for (sym_id, sym_content, file_id, file_content) in [
            (
                "user.ts::sumThree",
                "### Summary\nAdds together three given numbers.\n### Behavior\nCalls add then adds the third value.\n",
                "user.ts",
                "Sums three numbers together via the add helper.",
            ),
            (
                "other.ts::noop",
                "### Summary\nDoes nothing at all.\n### Behavior\nIntentionally performs no operation.\n",
                "other.ts",
                "An intentional, deliberate no-op function.",
            ),
        ] {
            server
                .submit_spec(Parameters(SubmitSpecRequest {
                    id: sym_id.to_string(),
                    content: sym_content.to_string(),
                }))
                .await
                .unwrap();
            server
                .submit_spec(Parameters(SubmitSpecRequest {
                    id: file_id.to_string(),
                    content: file_content.to_string(),
                }))
                .await
                .unwrap();
        }

        // math.ts's implementation changes; its exported signature does not.
        let math_v2_body = "export function add(a: number, b: number): number {\n  // logs\n  console.log('adding');\n  return a + b;\n}\n";
        let server_body_edit = rebuild_server(
            dir.clone(),
            &[
                ("math.ts", math_v2_body),
                ("user.ts", user),
                ("other.ts", other),
            ],
        );
        let done = server_body_edit
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "user.ts".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(
            done.0,
            SpecTaskResponse::Done {},
            "an implementation-only dependency edit must not stale the importer"
        );
        let sumthree = server_body_edit
            .get_spec(Parameters(IdRequest {
                id: "user.ts::sumThree".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(sumthree.0.status, "current");
        let other_spec = server_body_edit
            .get_spec(Parameters(IdRequest {
                id: "other.ts::noop".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(other_spec.0.status, "current");

        // math.ts's exported signature changes.
        let math_v2_sig = "export function add(a: number, b: number, c: number): number {\n  return a + b + c;\n}\n";
        let server_sig_edit = rebuild_server(
            dir.clone(),
            &[
                ("math.ts", math_v2_sig),
                ("user.ts", user),
                ("other.ts", other),
            ],
        );
        let task = server_sig_edit
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "user.ts".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(
            matches!(task, SpecTaskResponse::Symbol { ref id, .. } if id == "user.ts::sumThree")
        );

        let sumthree = server_sig_edit
            .get_spec(Parameters(IdRequest {
                id: "user.ts::sumThree".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(sumthree.0.status, "stale");
        assert_eq!(sumthree.0.changed, vec!["changed:dependencies".to_string()]);
        assert!(
            sumthree.0.content.is_some(),
            "a stale symbol must still return its last-known-good content"
        );
        let user_file = server_sig_edit
            .get_spec(Parameters(IdRequest {
                id: "user.ts".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(user_file.0.status, "stale");
        let other_spec = server_sig_edit
            .get_spec(Parameters(IdRequest {
                id: "other.ts::noop".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(
            other_spec.0.status, "current",
            "an unrelated file must not be affected"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    // --- M18: the God-class generation-task payload fix ----------------

    #[tokio::test]
    async fn a_god_class_generation_task_has_reduced_source_but_keeps_its_real_dependencies() {
        let dir = std::env::temp_dir().join(format!("codeowl-mcp-godclass-{}", std::process::id()));
        let helper_src = "package org.acme;\n\npublic class Helper {\n    public static int assist(int n) { return n; }\n}\n";
        let padding = "x".repeat(crate::spec::LARGE_CONTAINER_BYTES_DEFAULT + 1000);
        let big_src = format!(
            "package org.acme;\n\n\
             public class Big {{\n\
             \x20   /** Computes a padded total. */\n\
             \x20   public int compute(int n) {{\n\
             \x20       // {padding}\n\
             \x20       return Helper.assist(n) + 1;\n\
             \x20   }}\n\
             }}\n"
        );

        let server = rebuild_server(
            dir.clone(),
            &[
                ("src/main/java/org/acme/Helper.java", helper_src),
                ("src/main/java/org/acme/Big.java", &big_src),
            ],
        );

        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "src/main/java/org/acme/Big.java".to_string(),
            }))
            .await
            .unwrap();

        let SpecTaskResponse::Symbol {
            source,
            dependencies,
            ..
        } = task.0
        else {
            panic!("expected a Symbol task");
        };
        assert!(
            source.contains("Computes a padded total"),
            "docstring must survive:\n{source}"
        );
        assert!(
            source.contains("public int compute(int n)"),
            "signature must survive:\n{source}"
        );
        assert!(
            !source.contains("Helper.assist(n) + 1"),
            "the body must be dropped from the task source:\n{source}"
        );
        assert!(
            dependencies.iter().any(|d| d.contains("Helper")),
            "the real dependency, referenced only inside the now-dropped body, \
             must still be listed -- ### Depends on scans the full text, not \
             the reduced one: {dependencies:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn a_large_flat_file_task_gets_capped_even_with_no_big_container() {
        // The real-world case that surfaced this: `File` tasks had zero
        // size handling at all, even after the Container reduction fix --
        // a large file with no single dominant class (a long flat script,
        // or in this fixture a file padded outside any symbol's own span)
        // was completely unprotected.
        let dir = std::env::temp_dir().join(format!("codeowl-mcp-bigfile-{}", std::process::id()));
        let padding = "x".repeat(crate::spec::MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT + 2000);
        let src = format!("// {padding}\nexport function tiny(): number {{ return 1; }}\n");
        let server = rebuild_server(dir.clone(), &[("big.ts", &src)]);

        // The lone symbol is offered first; submit it so the file task
        // (which reads the whole raw file, comment padding included) is
        // what comes back next.
        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "big.ts".to_string(),
            }))
            .await
            .unwrap();
        let SpecTaskResponse::Symbol { id, .. } = task.0 else {
            panic!("expected the symbol task first");
        };
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id,
                content: "### Summary\nReturns the constant value one to its caller.\n### Behavior\nAlways evaluates to the integer one, with no branching.\n".to_string(),
            }))
            .await
            .unwrap();

        let file_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "big.ts".to_string(),
            }))
            .await
            .unwrap();
        let SpecTaskResponse::File { source, .. } = file_task.0 else {
            panic!("expected the file task next");
        };
        assert!(
            source.len() <= crate::spec::MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT,
            "a large flat file's task source must be capped, marker included: {} bytes",
            source.len()
        );
        assert!(
            source.contains("truncated"),
            "must visibly mark truncation, not silently drop content"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn with_generation_limits_overrides_the_compiled_in_defaults() {
        // The actual configurability this whole fix is for: a caller
        // (`codeowl serve --max-spec-task-bytes N`) can tighten the cap
        // below the compiled-in default for a client whose real overflow
        // threshold is smaller -- proven here with a tiny override that
        // truncates even a normally-small file.
        let dir =
            std::env::temp_dir().join(format!("codeowl-mcp-customlimit-{}", std::process::id()));
        let src = "export function tiny(): number { return 1; }\n";
        let server = rebuild_server(dir.clone(), &[("small.ts", src)])
            .with_generation_limits(None, Some(20));

        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "small.ts".to_string(),
            }))
            .await
            .unwrap();
        let SpecTaskResponse::Symbol { source, .. } = task.0 else {
            panic!("expected the symbol task");
        };
        assert!(
            source.len() <= 20,
            "a caller-supplied max_spec_task_bytes must actually take effect, \
             marker included, even on a file the compiled-in default would never touch: {} bytes",
            source.len()
        );
        assert!(source.contains("truncated"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn leaf_body_edit_stales_exactly_its_file_and_containing_rollup() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-mcp-staletest-{}-2", std::process::id()));
        let math_v1 = "export function add(a: number, b: number): number {\n  return a + b;\n}\n";
        let sibling = "export function noop(): void {}\n";

        let server = rebuild_server(
            dir.clone(),
            &[("lib/math.ts", math_v1), ("lib/sibling.ts", sibling)],
        );
        for (sym_id, sym_content, file_id, file_content) in [
            (
                "lib/math.ts::add",
                "### Summary\nAdds together two given numbers.\n### Behavior\nReturns the sum of its two arguments.\n",
                "lib/math.ts",
                "A small numeric addition helper module.",
            ),
            (
                "lib/sibling.ts::noop",
                "### Summary\nDoes nothing at all.\n### Behavior\nIntentionally performs no operation.\n",
                "lib/sibling.ts",
                "An intentional, deliberate no-op function.",
            ),
        ] {
            server
                .submit_spec(Parameters(SubmitSpecRequest {
                    id: sym_id.to_string(),
                    content: sym_content.to_string(),
                }))
                .await
                .unwrap();
            server
                .submit_spec(Parameters(SubmitSpecRequest {
                    id: file_id.to_string(),
                    content: file_content.to_string(),
                }))
                .await
                .unwrap();
        }
        let rollup_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "lib".to_string(),
            }))
            .await
            .unwrap()
            .0;
        let SpecTaskResponse::Rollup { id: rollup_id, .. } = rollup_task else {
            panic!("expected a Rollup task, got {rollup_task:?}");
        };
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: rollup_id.clone(),
                content: "Small numeric and no-op helper functions.".to_string(),
            }))
            .await
            .unwrap();

        for id in [
            "lib/math.ts::add",
            "lib/math.ts",
            "lib/sibling.ts::noop",
            "lib/sibling.ts",
            rollup_id.as_str(),
        ] {
            let spec = server
                .get_spec(Parameters(IdRequest { id: id.to_string() }))
                .await
                .unwrap();
            assert_eq!(
                spec.0.status, "current",
                "{id} should be current before any edit"
            );
        }

        // Edit ONLY math.ts's body -- same exported signature, sibling.ts
        // untouched.
        let math_v2 = "export function add(a: number, b: number): number {\n  // logs\n  console.log('adding');\n  return a + b;\n}\n";
        let server2 = rebuild_server(
            dir.clone(),
            &[("lib/math.ts", math_v2), ("lib/sibling.ts", sibling)],
        );

        let math_symbol = server2
            .get_spec(Parameters(IdRequest {
                id: "lib/math.ts::add".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(math_symbol.0.status, "stale");
        let math_file = server2
            .get_spec(Parameters(IdRequest {
                id: "lib/math.ts".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(math_file.0.status, "stale");
        let rollup = server2
            .get_spec(Parameters(IdRequest {
                id: rollup_id.clone(),
            }))
            .await
            .unwrap();
        assert_eq!(rollup.0.status, "stale");
        assert_eq!(rollup.0.changed, vec!["changed:lib/math.ts".to_string()]);

        let sibling_symbol = server2
            .get_spec(Parameters(IdRequest {
                id: "lib/sibling.ts::noop".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(sibling_symbol.0.status, "current");
        let sibling_file = server2
            .get_spec(Parameters(IdRequest {
                id: "lib/sibling.ts".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(sibling_file.0.status, "current");

        let next = server2
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "lib".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(
            matches!(next, SpecTaskResponse::Symbol { ref id, .. } if id == "lib/math.ts::add")
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn feature_spec_goes_stale_when_a_core_participant_changes_but_not_an_unrelated_file() {
        let dir =
            std::env::temp_dir().join(format!("codeowl-mcp-staletest-{}-3", std::process::id()));
        let page_v1 =
            "export default function Page() {\n  fetch(\"/api/widget\");\n  return null;\n}\n";
        let route = "export async function GET(): Promise<void> {}\n";
        let unrelated = "export function helper(): void {}\n";

        let server = rebuild_server(
            dir.clone(),
            &[
                ("app/widget/page.tsx", page_v1),
                ("app/api/widget/route.ts", route),
                ("lib/unrelated.ts", unrelated),
            ],
        );

        let symbol_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/widget/page.tsx".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(matches!(symbol_task, SpecTaskResponse::Symbol { .. }));
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "app/widget/page.tsx::Page".to_string(),
                content: "### Summary\nRenders the widget page.\n### Behavior\nFetches from the widget API on mount.\n".to_string(),
            }))
            .await
            .unwrap();
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "app/widget/page.tsx".to_string(),
                content: "The page that renders the widget for visitors.".to_string(),
            }))
            .await
            .unwrap();
        let feature_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/widget/page.tsx".to_string(),
            }))
            .await
            .unwrap()
            .0;
        let SpecTaskResponse::Feature { id: feature_id, .. } = feature_task else {
            panic!("expected a Feature task, got {feature_task:?}");
        };
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: feature_id.clone(),
                content: "# Widget\n## Summary\nShows the widget to visitors on its own page.\n## How it works\n1. Loads and fetches.\n## Data touched\nNone.\n## Rules & failure modes\nNone.\n".to_string(),
            }))
            .await
            .unwrap();

        let spec = server
            .get_spec(Parameters(IdRequest {
                id: feature_id.clone(),
            }))
            .await
            .unwrap();
        assert_eq!(spec.0.status, "current");

        // Edit the page itself -- a core participant.
        let page_v2 = "export default function Page() {\n  fetch(\"/api/widget\");\n  console.log('loaded');\n  return null;\n}\n";
        let server2 = rebuild_server(
            dir.clone(),
            &[
                ("app/widget/page.tsx", page_v2),
                ("app/api/widget/route.ts", route),
                ("lib/unrelated.ts", unrelated),
            ],
        );
        let spec2 = server2
            .get_spec(Parameters(IdRequest {
                id: feature_id.clone(),
            }))
            .await
            .unwrap();
        assert_eq!(spec2.0.status, "stale");
        assert!(
            spec2
                .0
                .changed
                .iter()
                .any(|c| c.contains("app/widget/page.tsx")),
            "changed should name the page participant, got {:?}",
            spec2.0.changed
        );
        assert!(
            spec2.0.content.is_some(),
            "a stale feature must still return its last-known-good narrative"
        );

        // Edit a file the feature never touches.
        let unrelated_v2 = "export function helper(): void {\n  console.log('changed');\n}\n";
        let server3 = rebuild_server(
            dir.clone(),
            &[
                ("app/widget/page.tsx", page_v1),
                ("app/api/widget/route.ts", route),
                ("lib/unrelated.ts", unrelated_v2),
            ],
        );
        let spec3 = server3
            .get_spec(Parameters(IdRequest { id: feature_id }))
            .await
            .unwrap();
        assert_eq!(
            spec3.0.status, "current",
            "editing a file the feature never touches must not stale it"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn get_next_spec_task_accepts_feature_and_rollup_ids() {
        // These are the ids get_spec_coverage puts in `pending`; a budgeted
        // --all walk hands them straight back to get_next_spec_task.
        let server = rebuild_server(
            std::env::temp_dir().join(format!("codeowl-idvocab-{}", std::process::id())),
            &[
                (
                    "app/submit/page.tsx",
                    "export default function Page() { return null; }\n",
                ),
                ("lib/one.ts", "export function one(): void {}\n"),
                ("lib/two.ts", "export function two(): void {}\n"),
            ],
        );

        let by_slug = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "feature:submit".to_string(),
            }))
            .await
            .unwrap()
            .0;
        let by_path = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/submit/page.tsx".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(
            !matches!(by_slug, SpecTaskResponse::Done {}),
            "feature:<slug> must resolve to a task"
        );
        assert_eq!(by_slug, by_path, "feature:<slug> == its entry-point path");

        let rollup_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "rollup:lib".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(
            !matches!(rollup_task, SpecTaskResponse::Done {}),
            "rollup:<dir> must resolve to a task for a spec-bearing directory"
        );

        // An unknown feature slug is a clean `done`, not an error.
        let missing = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "feature:does-not-exist".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(matches!(missing, SpecTaskResponse::Done {}));
    }

    #[tokio::test]
    async fn generate_loop_walks_symbol_then_file_then_reports_done() {
        let server = test_server(&[(
            "a.ts",
            "/** Doubles a number. */\nexport function double(x: number) {}\n",
        )]);

        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "a.ts".to_string(),
            }))
            .await
            .unwrap();
        let SpecTaskResponse::Symbol { id, source, .. } = task.0 else {
            panic!("expected a symbol task, got {:?}", task.0);
        };
        assert_eq!(id, "a.ts::double");
        assert!(source.contains("function double"));

        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: id.clone(),
                content: "### Summary\nDoubles the given number.\n### Behavior\nMultiplies its input by two and returns it."
                    .to_string(),
            }))
            .await
            .unwrap();

        // Re-running get_spec now finds a real, current spec instead of
        // the missing stub -- this is the M4 validation's core claim.
        let spec = server
            .get_spec(Parameters(IdRequest { id: id.clone() }))
            .await
            .unwrap();
        assert_eq!(spec.0.status, "current");
        assert!(
            spec.0
                .content
                .unwrap()
                .contains("Multiplies its input by two and returns it.")
        );

        // Re-running generate with source unchanged must skip the
        // now-current symbol and move on to the file-level task.
        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "a.ts".to_string(),
            }))
            .await
            .unwrap();
        let SpecTaskResponse::File { id: file_id, .. } = task.0 else {
            panic!("expected a file task, got {:?}", task.0);
        };
        assert_eq!(file_id, "a.ts");

        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: file_id,
                content: "A file with one doubling helper.".to_string(),
            }))
            .await
            .unwrap();

        let done = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "a.ts".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(done.0, SpecTaskResponse::Done {});
    }

    #[tokio::test]
    async fn generate_loop_skips_a_barrel_file() {
        let server = test_server(&[("a.ts", "export { Foo } from './foo';\n")]);
        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "a.ts".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(task.0, SpecTaskResponse::Done {});
    }

    #[test]
    fn done_serializes_as_an_object_not_a_bare_null() {
        // The bug this fixes: a bare `null` result fails some MCP clients'
        // `structuredContent` schema check ("expected record").
        let json = serde_json::to_value(SpecTaskResponse::Done {}).unwrap();
        assert!(json.is_object(), "got {json}");
        assert_eq!(json["kind"], "done");
    }

    #[tokio::test]
    async fn generate_loop_produces_a_feature_spec_once_the_target_files_are_covered() {
        let server = test_server(&[
            (
                "app/submit/page.tsx",
                "import { getSupabase } from '../../lib/supabase';\nexport default function Page() {\n  fetch(\"/api/submit-artwork\");\n  getSupabase();\n  return null;\n}\n",
            ),
            (
                "app/api/submit-artwork/route.ts",
                "import { getSupabase } from '../../../lib/supabase';\nexport async function POST(): Promise<void> {\n  getSupabase();\n}\n",
            ),
            (
                "lib/supabase.ts",
                "export function getSupabase(): void {}\n",
            ),
        ]);

        // `export default function Page()` is a *named* default export,
        // so M1 extracts it as a real, exported symbol -- page.tsx is
        // file-spec-bearing under M4's rule same as any other file. Drain
        // that ladder (its one symbol, then the file itself) before the
        // feature task appears, exactly like a real `/codeowl generate`
        // run against this target would.
        let symbol_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/submit/page.tsx".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(matches!(symbol_task, SpecTaskResponse::Symbol { .. }));
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "app/submit/page.tsx::Page".to_string(),
                content: "### Summary\nRenders the submission form.\n### Behavior\nCalls the submit-artwork API.\n".to_string(),
            }))
            .await
            .unwrap();
        let file_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/submit/page.tsx".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(matches!(file_task, SpecTaskResponse::File { .. }));
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "app/submit/page.tsx".to_string(),
                content: "The artwork submission page.".to_string(),
            }))
            .await
            .unwrap();

        // Now that page.tsx's own symbol+file specs are current, the next
        // task on this same target is the feature.
        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/submit/page.tsx".to_string(),
            }))
            .await
            .unwrap()
            .0;
        let SpecTaskResponse::Feature {
            id,
            entry_point,
            core_sources,
            dependencies,
            data,
        } = task
        else {
            panic!("expected a Feature task");
        };
        assert_eq!(id, "feature:submit");
        assert_eq!(entry_point, "app/submit/page.tsx");
        assert_eq!(
            core_sources
                .iter()
                .map(|c| c.file.as_str())
                .collect::<Vec<_>>(),
            vec!["app/submit/page.tsx", "app/api/submit-artwork/route.ts"]
        );
        assert_eq!(dependencies.len(), 1);
        assert_eq!(dependencies[0].id, "lib/supabase.ts::getSupabase");
        assert!(data.is_empty(), "no .sql fixture, so no data participants");

        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: id.clone(),
                content: "# Artwork submission\n## Summary\nLets an artist submit artwork.\n"
                    .to_string(),
            }))
            .await
            .unwrap();

        let spec = server
            .get_spec(Parameters(IdRequest { id: id.clone() }))
            .await
            .unwrap();
        assert_eq!(spec.0.status, "current");
        assert!(
            spec.0
                .content
                .unwrap()
                .contains("Lets an artist submit artwork.")
        );

        let done = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/submit/page.tsx".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(done.0, SpecTaskResponse::Done {});
    }

    #[tokio::test]
    async fn get_next_spec_task_does_not_collapse_several_entries_sharing_one_file() {
        // M17 dogfood bug: several FastAPI routes decorated in one module
        // share `EntryPoint.file` -- something the pilot's one-page/route-
        // per-file Next.js world never exercised. `next_feature_task_response`
        // used to `.find()` the *first* entry on that file regardless of
        // its status, so once one route's feature spec existed, every
        // other route on the same file reported `{"kind":"done"}` even
        // though `get_spec_coverage` still listed them as missing.
        let server = test_server(&[(
            "app/api/routes/items.py",
            "router = APIRouter(prefix=\"/items\")\n\n\n@router.get(\"/\")\ndef read_items():\n    return []\n\n\n@router.get(\"/{id}\")\ndef read_item(id: int):\n    return None\n",
        )]);

        // Drain the file's own ladder once -- both entry points share it.
        for sym_id in [
            "app/api/routes/items.py::read_items",
            "app/api/routes/items.py::read_item",
        ] {
            let task = server
                .get_next_spec_task(Parameters(GenerateTaskRequest {
                    target: "app/api/routes/items.py".to_string(),
                }))
                .await
                .unwrap()
                .0;
            assert!(matches!(task, SpecTaskResponse::Symbol { .. }));
            server
                .submit_spec(Parameters(SubmitSpecRequest {
                    id: sym_id.to_string(),
                    content: "### Summary\nReads item rows for the current caller.\n### Behavior\nRuns a SQLModel select against the item table and returns the rows.\n"
                        .to_string(),
                }))
                .await
                .unwrap();
        }
        let file_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/api/routes/items.py".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(matches!(file_task, SpecTaskResponse::File { .. }));
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "app/api/routes/items.py".to_string(),
                content: "Item routes -- list and read endpoints backed by the item table."
                    .to_string(),
            }))
            .await
            .unwrap();

        // The first feature on the file.
        let first = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "feature:http-get-items".to_string(),
            }))
            .await
            .unwrap()
            .0;
        let SpecTaskResponse::Feature { id: first_id, .. } = first else {
            panic!("expected a Feature task, got {first:?}");
        };
        assert_eq!(first_id, "feature:http-get-items");
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: first_id,
                content: "# Items\n## Summary\nLists every item belonging to the current user, newest first.\n".to_string(),
            }))
            .await
            .unwrap();

        // The bug: this used to return `Done` — the first (now-current)
        // entry sharing the file was all `.find()` ever saw.
        let second = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "feature:http-get-items-id".to_string(),
            }))
            .await
            .unwrap()
            .0;
        let SpecTaskResponse::Feature { id: second_id, .. } = second else {
            panic!("a sibling route on the same file must still be offered a task, got {second:?}");
        };
        assert_eq!(second_id, "feature:http-get-items-id");
    }

    #[tokio::test]
    async fn get_next_spec_task_for_a_specific_feature_slug_does_not_substitute_a_sibling() {
        // A third M17 dogfood finding, read-side, distinct from the two
        // above: `resolve_next_task`'s `feature:<slug>` branch resolves the
        // exact `EntryPoint` matching `slug`, then discards everything but
        // its `.file` and hands that off to the generic file-target chase
        // -- which, once the file itself is current, falls into
        // `next_feature_task_response`'s "first not-yet-current entry on
        // this file" walk. `enumerate_entry_points` sorts globally by slug
        // id, so a target like `feature:http-get-items` on a file that also
        // hosts `http-delete-items-id` (alphabetically prior) silently
        // returns the *delete* route's task instead -- even though nothing
        // about the delete route was asked for. `feature:<slug>` is
        // documented (`codeowl-generate.md`) as "equivalent to naming the
        // feature's entry point"; it must return that entry's own task, or
        // report done because that entry itself is current -- never a
        // sibling's task.
        let server = test_server(&[(
            "app/api/routes/items.py",
            "router = APIRouter(prefix=\"/items\")\n\n\n@router.get(\"/\")\ndef read_items():\n    return []\n\n\n@router.delete(\"/{id}\")\ndef delete_item(id: int):\n    return None\n",
        )]);

        // Drain the file's own ladder -- both entry points share it.
        for _ in 0..2 {
            let task = server
                .get_next_spec_task(Parameters(GenerateTaskRequest {
                    target: "app/api/routes/items.py".to_string(),
                }))
                .await
                .unwrap()
                .0;
            let SpecTaskResponse::Symbol { id, .. } = task else {
                panic!("expected a Symbol task, got {task:?}");
            };
            server
                .submit_spec(Parameters(SubmitSpecRequest {
                    id,
                    content: "### Summary\nHandles one HTTP endpoint.\n### Behavior\nDelegates to the ORM for the actual query or mutation.\n".to_string(),
                }))
                .await
                .unwrap();
        }
        let file_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/api/routes/items.py".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(matches!(file_task, SpecTaskResponse::File { .. }));
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "app/api/routes/items.py".to_string(),
                content: "Item routes -- list and delete endpoints backed by the item table."
                    .to_string(),
            }))
            .await
            .unwrap();

        // Ask for the GET route specifically, without ever touching the
        // DELETE route (which sorts first alphabetically as
        // `http-delete-items-id`). The bug: this used to hand back the
        // delete route's task instead.
        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "feature:http-get-items".to_string(),
            }))
            .await
            .unwrap()
            .0;
        let SpecTaskResponse::Feature { id, .. } = task else {
            panic!("expected the requested feature's own task, got {task:?}");
        };
        assert_eq!(
            id, "feature:http-get-items",
            "a feature:<slug> target must never substitute a file sibling's task"
        );
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id,
                content: "# Items\n## Summary\nLists every item belonging to the current user.\n"
                    .to_string(),
            }))
            .await
            .unwrap();

        // The untouched sibling is still independently obtainable and
        // wasn't silently consumed by the request above.
        let sibling_spec = server
            .get_spec(Parameters(IdRequest {
                id: "feature:http-delete-items-id".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(sibling_spec.status, "missing");
        let sibling_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "feature:http-delete-items-id".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(
            matches!(&sibling_task, SpecTaskResponse::Feature { id, .. } if id == "feature:http-delete-items-id"),
            "the sibling must still get its own task when targeted directly, got {sibling_task:?}"
        );
    }

    #[tokio::test]
    async fn submit_spec_does_not_misattribute_a_feature_to_its_file_sibling() {
        // The write-side twin of the read-side bug above, and the actual
        // dogfood report: `submit_feature` used to re-resolve "which entry
        // point" from the *file* (`.find(|e| e.file == entry_file)`), so
        // submitting `feature:http-get-items` silently wrote its content
        // into `http-get-items-id`'s spec file instead (whichever sibling
        // `enumerate_entry_points`'s sort happened to return first) --
        // overwriting that sibling's real, current spec with no error.
        let server = test_server(&[
            (
                "app/api/routes/items.py",
                "router = APIRouter(prefix=\"/items\")\n\n\n@router.get(\"/\")\ndef read_items():\n    return []\n\n\n@router.get(\"/{id}\")\ndef read_item(id: int):\n    return None\n",
            ),
            (
                "app/api/routes/users.py",
                "router = APIRouter(prefix=\"/users\")\n\n\n@router.delete(\"/{user_id}\")\ndef delete_user(user_id: str):\n    return None\n\n\n@router.delete(\"/me\")\ndef delete_me():\n    return None\n",
            ),
        ]);

        // Drain both files' own ladders (symbols, then file spec).
        for file in ["app/api/routes/items.py", "app/api/routes/users.py"] {
            for _ in 0..2 {
                let task = server
                    .get_next_spec_task(Parameters(GenerateTaskRequest {
                        target: file.to_string(),
                    }))
                    .await
                    .unwrap()
                    .0;
                let SpecTaskResponse::Symbol { id, .. } = task else {
                    panic!("expected a Symbol task for {file}, got {task:?}");
                };
                server
                    .submit_spec(Parameters(SubmitSpecRequest {
                        id,
                        content: "### Summary\nHandles one HTTP endpoint.\n### Behavior\nDelegates to the ORM for the actual query or mutation.\n".to_string(),
                    }))
                    .await
                    .unwrap();
            }
            let file_task = server
                .get_next_spec_task(Parameters(GenerateTaskRequest {
                    target: file.to_string(),
                }))
                .await
                .unwrap()
                .0;
            assert!(matches!(file_task, SpecTaskResponse::File { .. }));
            server
                .submit_spec(Parameters(SubmitSpecRequest {
                    id: file.to_string(),
                    content: "Route handlers for this resource.".to_string(),
                }))
                .await
                .unwrap();
        }

        // Submit exactly the two ids from the real dogfood report.
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "feature:http-get-items".to_string(),
                content:
                    "# List Items\n## Summary\nLists every item belonging to the current user.\n"
                        .to_string(),
            }))
            .await
            .unwrap();
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "feature:http-delete-users-user-id".to_string(),
                content: "# Delete User (Admin)\n## Summary\nAn administrator deletes another user's account.\n".to_string(),
            }))
            .await
            .unwrap();

        // Each submitted id must carry *its own* content...
        let items_spec = server
            .get_spec(Parameters(IdRequest {
                id: "feature:http-get-items".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(items_spec.status, "current");
        assert!(items_spec.content.unwrap().contains("List Items"));

        let delete_user_spec = server
            .get_spec(Parameters(IdRequest {
                id: "feature:http-delete-users-user-id".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert_eq!(delete_user_spec.status, "current");
        assert!(
            delete_user_spec
                .content
                .unwrap()
                .contains("Delete User (Admin)")
        );

        // ...and neither file sibling was clobbered -- they're still
        // missing, exactly as `get_spec_coverage` would report.
        for untouched in ["feature:http-get-items-id", "feature:http-delete-users-me"] {
            let spec = server
                .get_spec(Parameters(IdRequest {
                    id: untouched.to_string(),
                }))
                .await
                .unwrap()
                .0;
            assert_eq!(
                spec.status, "missing",
                "{untouched} must be untouched by its sibling's submit, got {spec:?}"
            );
        }
    }

    #[tokio::test]
    async fn generate_loop_produces_a_rollup_once_its_files_are_covered() {
        let server = test_server(&[
            ("lib/one.ts", "export function one(): void {}\n"),
            ("lib/two.ts", "export function two(): void {}\n"),
        ]);

        // Neither file has a spec yet -- targeting the directory should
        // return one of their own symbol tasks first, not the rollup.
        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "lib".to_string(),
            }))
            .await
            .unwrap()
            .0;
        assert!(matches!(task, SpecTaskResponse::Symbol { .. }));

        for (sym_id, sym_content, file_id, file_content) in [
            (
                "lib/one.ts::one",
                "### Summary\nDoes the first specific thing.\n### Behavior\nRuns synchronously with no side effects.\n",
                "lib/one.ts",
                "Performs one specific, deliberate task.",
            ),
            (
                "lib/two.ts::two",
                "### Summary\nDoes a second, different thing.\n### Behavior\nAlso runs synchronously with no side effects.\n",
                "lib/two.ts",
                "Performs a couple of related tasks.",
            ),
        ] {
            server
                .submit_spec(Parameters(SubmitSpecRequest {
                    id: sym_id.to_string(),
                    content: sym_content.to_string(),
                }))
                .await
                .unwrap();
            server
                .submit_spec(Parameters(SubmitSpecRequest {
                    id: file_id.to_string(),
                    content: file_content.to_string(),
                }))
                .await
                .unwrap();
        }

        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "lib".to_string(),
            }))
            .await
            .unwrap()
            .0;
        let SpecTaskResponse::Rollup {
            id,
            dir_path,
            files,
        } = task
        else {
            panic!("expected a Rollup task, got {task:?}");
        };
        assert_eq!(id, "rollup:lib");
        assert_eq!(dir_path, "lib");
        assert_eq!(
            files,
            vec![
                RollupFile {
                    file: "lib/one.ts".to_string(),
                    summary: "Performs one specific, deliberate task.".to_string(),
                },
                RollupFile {
                    file: "lib/two.ts".to_string(),
                    summary: "Performs a couple of related tasks.".to_string(),
                },
            ]
        );

        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: id.clone(),
                content: "Small shared helper functions for this module.".to_string(),
            }))
            .await
            .unwrap();

        let spec = server
            .get_spec(Parameters(IdRequest { id: id.clone() }))
            .await
            .unwrap();
        assert_eq!(spec.0.status, "current");
        assert_eq!(
            spec.0.content.unwrap(),
            "Small shared helper functions for this module."
        );

        let done = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "lib".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(done.0, SpecTaskResponse::Done {});
    }

    #[tokio::test]
    async fn generate_dot_walks_every_module_and_feature_then_produces_the_system_spec() {
        let server = test_server(&[
            (
                "app/submit/page.tsx",
                "import { getSupabase } from '../../lib/supabase';\nexport default function Page() {\n  fetch(\"/api/submit-artwork\");\n  getSupabase();\n  return null;\n}\n",
            ),
            (
                "app/api/submit-artwork/route.ts",
                "import { getSupabase } from '../../../lib/supabase';\nexport async function POST(): Promise<void> {\n  getSupabase();\n}\n",
            ),
            (
                "lib/supabase.ts",
                "export function getSupabase(): void {}\n",
            ),
            ("lib/one.ts", "export function one(): void {}\n"),
            ("lib/two.ts", "export function two(): void {}\n"),
        ]);

        let mut seen_kinds = Vec::new();
        let system_task = loop {
            let task = server
                .get_next_spec_task(Parameters(GenerateTaskRequest {
                    target: ".".to_string(),
                }))
                .await
                .unwrap()
                .0;
            match &task {
                SpecTaskResponse::Symbol { id, .. } => {
                    seen_kinds.push("symbol");
                    server
                        .submit_spec(Parameters(SubmitSpecRequest {
                            id: id.clone(),
                            content: "### Summary\nDoes one small, specific job.\n### Behavior\nRuns without any side effects.\n".to_string(),
                        }))
                        .await
                        .unwrap();
                }
                SpecTaskResponse::File { id, .. } => {
                    seen_kinds.push("file");
                    server
                        .submit_spec(Parameters(SubmitSpecRequest {
                            id: id.clone(),
                            content: "A small file of helper functions.".to_string(),
                        }))
                        .await
                        .unwrap();
                }
                SpecTaskResponse::Rollup { id, .. } => {
                    seen_kinds.push("rollup");
                    server
                        .submit_spec(Parameters(SubmitSpecRequest {
                            id: id.clone(),
                            content: "Small shared helper functions for this module.".to_string(),
                        }))
                        .await
                        .unwrap();
                }
                SpecTaskResponse::Feature { id, .. } => {
                    seen_kinds.push("feature");
                    server
                        .submit_spec(Parameters(SubmitSpecRequest {
                            id: id.clone(),
                            content:
                                "# Artwork submission\n## Summary\nLets an artist submit artwork for judging.\n"
                                    .to_string(),
                        }))
                        .await
                        .unwrap();
                }
                SpecTaskResponse::System { .. } => {
                    seen_kinds.push("system");
                    break task;
                }
                SpecTaskResponse::Done {} => panic!("done before the system task"),
            }
        };

        let SpecTaskResponse::System {
            id: system_id,
            modules,
            features,
        } = system_task
        else {
            panic!("expected a System task");
        };
        assert_eq!(system_id, "system");
        assert_eq!(
            modules,
            vec![ModuleSummary {
                dir: "lib".to_string(),
                summary: "Small shared helper functions for this module.".to_string(),
            }]
        );
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].slug, "submit");
        assert!(features[0].summary.contains("Artwork submission"));

        // Every module and feature was drained bottom-up before the
        // system task appeared -- symbols/files for lib's 3 files, its
        // rollup, then the feature, then finally system.
        assert_eq!(seen_kinds.last(), Some(&"system"));
        assert!(seen_kinds.contains(&"rollup"));
        assert!(seen_kinds.contains(&"feature"));
        assert_eq!(seen_kinds.iter().filter(|k| **k == "system").count(), 1);

        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: system_id.clone(),
                content: "# Acme\n## Summary\nA platform for running art competitions.\n"
                    .to_string(),
            }))
            .await
            .unwrap();

        let spec = server
            .get_spec(Parameters(IdRequest {
                id: system_id.clone(),
            }))
            .await
            .unwrap();
        assert_eq!(spec.0.status, "current");
        assert!(
            spec.0
                .content
                .unwrap()
                .contains("A platform for running art competitions.")
        );

        let done = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: ".".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(done.0, SpecTaskResponse::Done {});
    }

    #[tokio::test]
    async fn search_finds_a_known_string() {
        let server = test_server(&[("a.ts", "export function findMe() {}\n")]);
        let result = server
            .search(Parameters(SearchRequest {
                query: "findMe".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(result.0.matches.len(), 1);
        assert_eq!(result.0.matches[0].file, "a.ts");
    }
}
