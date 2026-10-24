---
type: file
title: "src/spec/rollup.rs"
description: "Specification for `src/spec/rollup.rs`."
generated: { by: CodeOwl }
resource: "../../../../src/spec/rollup.rs"
source_paths: [src/spec/rollup.rs]
file: { source_hash: , deps_hash: , spec_hash:  }
symbols:
  src/spec/rollup.rs::RollupSpec: { source_hash: 8440dac838a3fe545555e3ae5c92420d1666ab3a69085f75fc53b7a6fa533072, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 72b2eb6a85aafc7d203920f20444537b9772c93c0e17bca1f8e9ca61e224dc61 }
  src/spec/rollup.rs::rollup_spec_path: { source_hash: ed395ca7c54a4aa9dbdf5d508dbd761a5c3ee9fa7e3449ad9315ddb3713f0c9c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 054eff69524633ab224c57734062ccc7511f73be3106fab04745c9d9fdb20cf6 }
  src/spec/rollup.rs::render_rollup: { source_hash: c40644f4670a0f14dfd41521fe1d2850ef78661bc4046f0b3b2f6f77c2284773, deps_hash: af94a68f1258f3da593fc61dfb57e31186b55d64b31f53c65cf79a38720c4d97, spec_hash: 2603997dbdd114374d13199a9cca83a92e810ed4d69d4845a1381fdafd0551d0 }
  src/spec/rollup.rs::parse_rollup: { source_hash: d429b4c93ba432bab8aca1356ed45c8406dd15a132ad183fbdebb088fd2060b9, deps_hash: c378d364baed498bdbd2549c482c4a36fa77a0c5383c0ae74334ff13ee349263, spec_hash: a1c6bcc0de86674c5e3bf95c09457294e57a04ba7d8df2531766640e95002794 }
  src/spec/rollup.rs::read_rollup_spec: { source_hash: ed3235a0bbb16ea492ee6ae44cc2cc22d514d06722f6e8861176578610d42397, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 16a120d990086fce7e913ee7ccc4064a22b16c7eaa6f57c0dc7dbe699e18d5d7 }
  src/spec/rollup.rs::current_file_hashes: { source_hash: b99c8164889dbe6e70bb012d1f22182dbc2336d69bef42faafaab9eff2f52df4, deps_hash: 5610b760aca04f5943603f23ca475d4bee7861294c7d4e2a6d1d0246f5ebd555, spec_hash: e42579d5427df49e3d38ad70c82ee4829c06830303bed50213725a5eb58e5e89 }
  src/spec/rollup.rs::RollupTask: { source_hash: e34cdaad98744602ef2a9c32b6ae75aa6de848f51963a982c2cbe94d1f2b2b3d, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 8907de5edbaa9405cc34050969363f92e79dbfea32779228665f86a5cbbddc3b }
  src/spec/rollup.rs::next_task_for_directory: { source_hash: e203639c0dc7d9dcd9de5b05af70e194c72b21027560559293beaf320686c597, deps_hash: 5610b760aca04f5943603f23ca475d4bee7861294c7d4e2a6d1d0246f5ebd555, spec_hash: 1b8b4cbcfbb9dfe0514319a20fe84b50c5b1cad4f234bbafe6a21a39992473a8 }
  src/spec/rollup.rs::next_rollup_task: { source_hash: ee1a4d7679a501c028233877439ee2062bb63bbb1768f0d59ea428a37f53cddd, deps_hash: 5610b760aca04f5943603f23ca475d4bee7861294c7d4e2a6d1d0246f5ebd555, spec_hash: ced088fbd62f73352a0ea690e4687a18a43390f73ca5b41acd71a099870a0102 }
  src/spec/rollup.rs::submit_rollup: { source_hash: e33ae5a7842b23eda2ea1cb4fb64b68405809a2df495058f1a6297f195f25576, deps_hash: 95d771bb7e074397c1b0b4d8f3414dce03c969efe263c5362cbbf4ce49cf30c2, spec_hash: 8f99b5b84f86e5ed1d4e75292f660583d4e3a830d830bb5632f383316b160432 }
  src/spec/rollup.rs::enumerate_modules: { source_hash: 6e4059b44e8189252f474e6101869b0134c9ef963212fe08e2869c442d755838, deps_hash: 5610b760aca04f5943603f23ca475d4bee7861294c7d4e2a6d1d0246f5ebd555, spec_hash: 36e4cfdfc7e2001a9b530ca6cf0c5454383022cd5baea0286777af6441d58ebb }
  src/spec/rollup.rs::current_module_hashes: { source_hash: a99dd24910292383cb0b40bb0645a199d97eb6a03ab2397027205e3fd51fd533, deps_hash: 5610b760aca04f5943603f23ca475d4bee7861294c7d4e2a6d1d0246f5ebd555, spec_hash: 493ae5e92c5ef3791ea9b25da7df01cc8399ee2322781eae95afeeff17a25647 }
dep_targets:
  src/spec/rollup.rs::render_rollup -> src/graph.rs::Graph: 8d4d56d40124
  src/spec/rollup.rs::render_rollup -> src/spec/format.rs::PROVENANCE: 42ae46372083
  src/spec/rollup.rs::render_rollup -> src/spec/format.rs::read_file_spec: 6682201bb38f
  src/spec/rollup.rs::render_rollup -> src/spec/layout.rs::file_is_spec_bearing: ecf0750070e9
  src/spec/rollup.rs::render_rollup -> src/spec/layout.rs::files_in: 10c4c6ce57fd
  src/spec/rollup.rs::parse_rollup -> src/spec/format.rs::extract_section: 2f6754bf9a83
  src/spec/rollup.rs::current_file_hashes -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/rollup.rs::next_task_for_directory -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/rollup.rs::next_rollup_task -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/rollup.rs::submit_rollup -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/rollup.rs::submit_rollup -> src/hash.rs::hash_text: 70c099c1622c
  src/spec/rollup.rs::enumerate_modules -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/rollup.rs::current_module_hashes -> src/graph.rs::Graph: 13dbdfa88b0e
---
# src/spec/rollup.rs
> Generated using [CodeOwl](https://github.com/siddswork/codeowl)
## Summary


## `RollupSpec`
`pub struct RollupSpec`
### Summary
The stored form of a folder summary, one page describing what a whole directory is for, built from the summaries of the files inside it.
### Behavior
Fields: `dir_path` (the folder), `files` (a list of each documented file's id with that file's own current `spec_hash`), `spec_hash` (a fingerprint of the folder's prose) and `body` (only the agent-written summary paragraph). The `files` list is keyed on the file *spec* hashes, not source hashes, so the rollup only goes stale when a file's spec is actually rewritten, and not merely because the source changed and has not been regenerated yet. The per-file listing under the summary is not stored; it is rebuilt by `render_rollup` each time, so it always shows the current files. Like `FeatureSpec` it has one document and one hash, with no per-entry sections, since a directory has no code of its own. Plain data, no logic.
### Depends on
- (none)

## `rollup_spec_path`
`pub fn rollup_spec_path(root: &Path, dir_path: &str) -> PathBuf`
### Summary
Computes where a directory's rollup spec lives on disk.
### Behavior
Joins the repo root with `docs/specs/<dir_path>/_index.md` — mirroring the directory's own path under `docs/specs/`, with the fixed filename `_index.md` marking it as that directory's own rollup rather than a file spec.
### Depends on
- externals: std

## `render_rollup`
`pub fn render_rollup(graph: &Graph, root: &Path, spec: &RollupSpec) -> String`
### Summary
Turns a folder summary into the markdown page saved as that folder's `_index.md`. The agent's paragraph goes on top and CodeOwl adds a one-line-per-file contents list underneath, rebuilt from the current files each time.
### Behavior
Output has a frontmatter block (`kind: rollup`, `dir`, each file's id with its recorded hash, and the rollup's own `spec_hash`), then a `# <dir>` title, the one-line "Generated by CodeOwl" attribution, `## Summary` with the trimmed body, and `## Contents`. The contents list covers every file directly in the directory, including ones that are not documented. A documented file is shown as its path plus the first line of its current summary read from its spec on disk (blank if the spec is missing or unreadable, since errors are swallowed). An undocumented file is shown as "(no document; not spec-bearing)". The list is computed on render and never stored, so a newly added file appears at once. It never fails.
### Depends on
- [`src/spec/format.rs::PROVENANCE`](format.rs.md) — super::format
- [`src/spec/format.rs::read_file_spec`](format.rs.md) — super::format
- [`src/spec/layout.rs::file_is_spec_bearing`](layout.rs.md) — super::layout
- [`src/spec/layout.rs::files_in`](layout.rs.md) — super::layout
- [`src/graph.rs::Graph`](../graph.rs.md) — crate::graph
- externals: std

## `parse_rollup`
`pub fn parse_rollup(content: &str) -> Result<RollupSpec>`
### Summary
Parses a rollup document's rendered text back into a `RollupSpec` — the inverse of `render_rollup`.
### Behavior
Requires the document to open with `---`. Walks the frontmatter: a `files:` line switches to a nested mode where each indented line is split with `rsplit_once(':')` into a file path and its stored hash (splitting from the right, since a file path could theoretically be unusual but its hash value never contains a colon). Other frontmatter lines are split on the first `:` to capture `dir` and `spec_hash`, with unrecognized keys ignored. After the closing `---`, unlike `parse_feature` (which keeps the whole remaining text as one body), this specifically extracts just the `## Summary` section out of the body text via `extract_section` — the `## Contents` listing that follows it in the rendered document is deterministically regenerated on every render, so it's deliberately never parsed back in or stored in the `RollupSpec`.
### Depends on
- [`src/spec/format.rs::extract_section`](format.rs.md) — super::format
- externals: anyhow

## `read_rollup_spec`
`pub fn read_rollup_spec(root: &Path, dir_path: &str) -> Result<Option<RollupSpec>>`
### Summary
Reads and parses a directory's rollup spec off disk, if it exists — the rollup counterpart to `read_existing`/`read_feature_spec`.
### Behavior
Computes the expected path via `rollup_spec_path`; returns `Ok(None)` if nothing exists there, otherwise reads and parses via `parse_rollup`.
### Depends on
- externals: anyhow, std

## `current_file_hashes`
`pub fn current_file_hashes(
    graph: &Graph,
    root: &Path,
    dir_path: &str,
) -> Result<Vec<(String, String)>>`
### Summary
Lists a folder's documented files together with the fingerprint of each file's current spec. Comparing this with what a folder summary stored shows whether the folder summary needs rewriting.
### Behavior
For each spec-bearing file directly in the folder (path-sorted) it reads the file's stored spec. If that spec exists and `file_changes` reports nothing changed, meaning the spec is current, the pair carries the file's `spec_hash`. If the spec is missing or out of date, the hash is an empty string. Using empty values for stale files means a folder summary cannot count as current while any file under it is out of date or undocumented. A fresh generation records the same list. A failure reading a spec file is returned as an error.
### Depends on
- [`src/spec/format.rs::read_file_spec`](format.rs.md) — super::format
- [`src/spec/layout.rs::spec_bearing_files_in`](layout.rs.md) — super::layout
- [`src/spec/staleness.rs::file_changes`](staleness.rs.md) — super::staleness
- [`src/graph.rs::Graph`](../graph.rs.md) — crate::graph
- externals: anyhow, std

## `RollupTask`
`pub struct RollupTask`
### Summary
What an agent is given to write a folder summary: the folder's path and the already-written summary of each file in it. The agent writes the folder's purpose from those summaries alone, without reading any code.
### Behavior
Fields are `dir_path` and `files`, a list of (file path, that file's current summary text). Nothing from the files' raw source is included, which keeps the task small and means folder summaries cost little beyond what is already generated. A task is only created once every documented file in the folder has a current spec, which `next_task_for_directory` enforces, so the summaries given here are never outdated. Plain data, no logic.
### Depends on
- (none)

## `next_task_for_directory`
`pub fn next_task_for_directory(
    graph: &Graph,
    root: &Path,
    dir_path: &str,
) -> Result<Option<SpecTask>>`
### Summary
Finds the next piece of spec writing needed anywhere in a folder's documented files, so a whole folder can be generated by asking this repeatedly. Files come before the folder's own summary.
### Behavior
Goes through the folder's spec-bearing files in path order and calls `next_task` on each, returning the first task found (a symbol or a file summary). Returns `Ok(None)` when every file is current, which is the signal that the folder summary itself can be considered next (`next_rollup_task` decides that). Only direct children of the folder are walked, since subfolders have their own rollups. An error from `next_task` for any file stops the walk and is returned. Because `next_task` can quietly record hand-edited prose, this too can write small housekeeping updates.
### Depends on
- [`src/spec/file_task.rs::SpecTask`](file_task.rs.md) — super::file_task
- [`src/spec/file_task.rs::next_task`](file_task.rs.md) — super::file_task
- [`src/spec/layout.rs::spec_bearing_files_in`](layout.rs.md) — super::layout
- [`src/graph.rs::Graph`](../graph.rs.md) — crate::graph
- externals: anyhow, std

## `next_rollup_task`
`pub fn next_rollup_task(graph: &Graph, root: &Path, dir_path: &str) -> Result<Option<RollupTask>>`
### Summary
Decides whether a folder needs a new summary page and, if so, collects the file summaries the agent will write it from. It returns nothing while any file in the folder is still unfinished or when the folder summary is already current.
### Behavior
An empty `dir_path` (the repo root) is an error, because its summary would collide with the reserved top-level `docs/specs/_index.md`. It returns `Ok(None)` when the folder has fewer than two documented files (`directory_is_spec_bearing`). It computes `current_file_hashes`, and if any file's hash is empty, meaning that file's spec is missing or stale, it returns `None`, so a folder summary is never written on top of out-of-date file summaries. The folder summary counts as current when the stored file hash list equals the current one and its body passes `body_smells`; a new or removed file therefore triggers a rewrite. Otherwise it gathers each documented file's stored summary, keeping only a spec whose `source_hash` matches the file's present one, and errors with a message to exhaust `next_task_for_directory` first if one is missing. The result is a `RollupTask` with the directory and those summaries.
### Depends on
- [`src/spec/format.rs::read_file_spec`](format.rs.md) — super::format
- [`src/spec/layout.rs::directory_is_spec_bearing`](layout.rs.md) — super::layout
- [`src/spec/layout.rs::spec_bearing_files_in`](layout.rs.md) — super::layout
- [`src/spec/smells.rs::body_smells`](smells.rs.md) — super::smells
- [`src/spec/staleness.rs::diff_hash_lists`](staleness.rs.md) — super::staleness
- [`src/spec/staleness.rs::file_changes`](staleness.rs.md) — super::staleness
- [`src/graph.rs::Graph`](../graph.rs.md) — crate::graph
- externals: anyhow, std

## `submit_rollup`
`pub fn submit_rollup(
    graph: &Graph,
    root: &Path,
    dir_path: &str,
    content: &str,
) -> Result<RollupSpec>`
### Summary
Saves the agent's folder summary, together with fingerprints of the file summaries it was based on, as that folder's `_index.md`. It refuses text that is empty, weak, or written while the files below it are not ready.
### Behavior
Checks in order, each failure an error with nothing written: the directory must be spec-bearing (at least two documented files); the trimmed content must be non-empty; and it must pass `body_smells`. It then computes `current_file_hashes`, and if any file has an empty hash (no current spec) it fails naming that file and telling the caller to generate its file spec first. On success it builds a `RollupSpec` whose body is hashed with `hash_text`, creates the directory if needed, writes the output of `render_rollup`, and returns the `RollupSpec`. The recorded file hashes are what later make the folder summary go stale when any file's spec is rewritten or the file set changes. File errors carry "creating ..." or "writing ..." context.
### Depends on
- [`src/spec/format.rs::write_replacing`](format.rs.md) — super::format
- [`src/spec/layout.rs::directory_is_spec_bearing`](layout.rs.md) — super::layout
- [`src/spec/smells.rs::body_smells`](smells.rs.md) — super::smells
- [`src/spec/smells.rs::reject_if_smelly`](smells.rs.md) — super::smells
- [`src/graph.rs::Graph`](../graph.rs.md) — crate::graph
- [`src/hash.rs::hash_text`](../hash.rs.md) — crate::hash
- externals: anyhow, std

## `enumerate_modules`
`pub fn enumerate_modules(graph: &Graph) -> Vec<String>`
### Summary
Lists every folder in the repo that qualifies for its own summary page. This flat list is what the top-level system summary draws on.
### Behavior
Takes the parent directory of every indexed file (backslashes normalised to `/`), sorts and de-duplicates them, and keeps a directory only if it is non-empty, is not test code, and passes `directory_is_spec_bearing` (at least two documented files directly inside). The repo root, whose path is the empty string, is always excluded because it is reserved for the system spec itself. The test check appends a `/` to the directory, so a folder like `tests` is recognised as a test root. The list is deliberately flat at every nesting depth: a folder's summary does not roll up its subfolders' summaries, so no hierarchy is invented. Result is alphabetical, and it cannot fail.
### Depends on
- [`src/spec/layout.rs::directory_is_spec_bearing`](layout.rs.md) — super::layout
- [`src/spec/layout.rs::is_test_path`](layout.rs.md) — super::layout
- [`src/graph.rs::Graph`](../graph.rs.md) — crate::graph
- externals: std

## `current_module_hashes`
`pub fn current_module_hashes(graph: &Graph, root: &Path) -> Result<Vec<(String, String)>>`
### Summary
Lists every module folder with the fingerprint of its current folder summary, or an empty value if that summary is missing or out of date. The top-level system summary compares this with what it stored to know whether to be rewritten.
### Behavior
For each directory from `enumerate_modules` it computes the current file hashes and reads the stored rollup. The rollup's `spec_hash` is used only when its recorded file list matches the current one (`diff_hash_lists` is empty), meaning the rollup is current; otherwise the value is an empty string. It follows the same "empty means not ready" convention one level down in `current_file_hashes`. Any failure reading a spec file is returned as an error.
### Depends on
- [`src/spec/staleness.rs::diff_hash_lists`](staleness.rs.md) — super::staleness
- [`src/graph.rs::Graph`](../graph.rs.md) — crate::graph
- externals: anyhow, std
