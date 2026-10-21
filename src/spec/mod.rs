//! The spec document format and file writer: file specs, feature specs,
//! directory rollups and the system spec. Implements `ARCHITECTURE.md`'s
//! "Spec document format": the mirrored `docs/specs/` tree,
//! per-symbol/per-file/per-directory/per-repo hash frontmatter, and the
//! granularity rules deciding which documents exist at all.
//!
//! Frontmatter here is hand-parsed rather than run through a general YAML
//! library: it's one fixed, CodeOwl-owned shape (see `format::render` and
//! `format::parse`), not arbitrary human-authored YAML, so a parser scoped
//! exactly to that shape is simpler — and easier to reason about — than a
//! full grammar for something we control both ends of.
//!
//! The submodules, lowest layer first. A module imports only from the ones
//! above it in this list (`coverage` and `file_task` both read `format` and
//! `staleness`; nothing reads `coverage` except the report modules after it):
//!
//! - `layout`: which documents should exist, and where they live
//! - `source_text`: the source text a generation task shows, and its caps
//! - `format`: the per-file spec document, render, parse, read and write
//! - `staleness`: dependency hashing and change detection
//! - `smells`: deterministic quality checks on spec prose
//! - `task_input`: the paged file-task input and its stand-ins
//! - `file_task`, `feature`, `rollup`, `system`: one task-and-submit loop
//!   per document kind
//! - `coverage`, `orphans`, `priority`: the reports `get_spec_coverage` serves
//! - `migrate`: one-off rewrites of stored specs
//!
//! The token-budget recursion threshold (open question 2) is still out of
//! scope here.
//!
//! Callers outside this module name items as `spec::name`; the explicit
//! `pub use` lists below are that surface. They are lists, not globs: this
//! repo's own Rust resolver follows a named re-export but not a glob.

mod adopt;
mod adopt_write;
mod coverage;
mod feature;
mod file_task;
mod format;
mod layout;
mod migrate;
mod orphans;
mod priority;
mod rollup;
mod smells;
mod source_text;
mod staleness;
mod system;
mod task_input;
#[cfg(test)]
mod test_support;

pub use self::adopt::{AdoptionPlan, Deps, Finding, Successor, Summary, plan as plan_adoption};
pub use self::adopt_write::{
    ApplyReport, WritePlan, WriteSummary, apply as apply_adoption, plan_writes,
};
pub use self::coverage::{
    CoverageItem, CoverageSummary, by_kind, by_module, coverage, summarize, weighted_freshness,
};
pub use self::feature::{
    FeatureTask, current_feature_hashes, current_participant_hashes, next_feature_task,
    read_feature_spec, submit_feature,
};
pub use self::file_task::{SpecTask, next_task, submit};
pub use self::format::{FileSpec, HashPair, read_file_spec, render};
pub(crate) use self::layout::spec_bearing_children;
pub use self::layout::{
    GeneratedSourcesSummary, directory_is_spec_bearing, generated_sources_summary, spec_path,
};
pub use self::migrate::{backfill_dep_targets, migrate_file_fingerprints};
pub use self::orphans::{OrphanedSpec, find_orphaned_specs};
pub use self::priority::{prioritize, top_stale_by_impact};
pub use self::rollup::{
    current_file_hashes, current_module_hashes, enumerate_modules, next_rollup_task,
    next_task_for_directory, read_rollup_spec, submit_rollup,
};
pub use self::smells::{body_smells, file_spec_smells, prose_smells};
pub use self::source_text::{
    LARGE_CONTAINER_BYTES_DEFAULT, MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT,
    MAX_SOURCE_TEXT_BYTES_DEFAULT,
};
pub(crate) use self::source_text::{
    cap_generation_text, maybe_reduce_container_source, merged_symbol_spans, render_spans,
    scoped_symbol_deps, symbol_span_text,
};
pub use self::staleness::{diff_hash_lists, file_changes, symbol_changes};
pub(crate) use self::staleness::{
    hash_dependency_pairs, shorten_dep_targets, try_dependency_pairs, try_file_dependency_pairs,
};
pub use self::system::{next_system_task, read_system_spec, submit_system};
pub use self::task_input::{
    FILE_OUTLINE_CONTAINER_PERCENT_DEFAULT, FILE_RAW_MAX_PAGES_DEFAULT,
    FILE_SUMMARIES_MIN_SYMBOLS_DEFAULT, FileLadder, FileRung,
};
pub(crate) use self::task_input::{
    FileTaskInput, decode_page_cursor, encode_page_cursor, file_task_input,
};
#[cfg(test)]
pub(crate) use self::test_support::plant_smelly_symbol_spec;
