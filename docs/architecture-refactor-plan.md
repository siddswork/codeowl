# Architectural review: refactoring plan

Status (2026-10-21): **PR 1 merged (#103). PR 2 built on branch `split-spec-module`, in review. PR 3 skipped.** D1 yes, D2 keep the by-name lookup, D3 skip. Written on branch `refactor-plan`, cut from `master` at `1940316`.

PR 2 as built differs from the table below in three ways, all found by checking the code: the module list gained `source_text` (the span and size-cap helpers both `format` and `staleness` read, which removed a `format` and `task_input` cycle), `orphans` and `priority` were split out of `coverage`, and the tests did not stay together in one file: a private helper is invisible to a sibling module, so each test moved into the module it exercises, with fixtures shared across modules in `test_support`. The re-exports in `spec/mod.rs` are written `pub use self::module::{...}`, because this repo's own Rust resolver follows a named re-export but not a glob, and does not resolve a bare child-module path (`pub use module::{...}` left 70 imports unresolved in its self-index; a gap in the resolver, not fixed here).

Scope: the code only. Docs and stored specs are out of scope; the specs can be regenerated from scratch after the refactor.

## How this was measured

| Check | Method |
|---|---|
| Production vs test lines | Line of the first `mod tests` per file, then function-boundary listing (a first `#[cfg(test)]` can be a test helper, as in `spec.rs:2136`) |
| What crosses a region boundary inside `spec.rs` | A throwaway script (`xref.py`, scratchpad, not committed) that tags each top-level item with its region and lists uses from other regions |
| Module dependency cycles | A throwaway script (`moddeps.py`, scratchpad) that reads `crate::<module>` references outside test modules, then finds strongly connected groups |
| Who uses `spec::*` from outside | `grep` over `src`, `tests`, `examples` |

Sizes (production lines): `spec.rs` ~4,000, `mcp.rs` ~1,830, `rust.rs` ~1,500, then `java.rs`, `python.rs`, `quarkus.rs`, `features.rs`, `extract.rs` at 700 to 900 each. Test code adds ~4,400 to `spec.rs` and ~3,450 to `mcp.rs`.

## Findings

### Kept

| # | Finding | Evidence |
|---|---|---|
| F1 | **Two pack registries that must agree.** `lang::detect` (`lang.rs:195`) lists four packs; `stack::for_name` (`stack.rs:262`) lists them again by name string. A name that matches neither arm silently becomes the TypeScript pack, so a typo classifies a Rust repo with TypeScript rules and nothing errors. | `lang.rs:194-202`, `stack.rs:262-269` |
| F2 | **The core depends on its leaves: one 14-module dependency cycle containing `graph.rs`.** Modules in the cycle: `extract, fastapi, features, graph, imports, java, lang, python, quarkus, resolve, rust, rust_crates, spec, stack`. The edges that cause it are listed in the table below. | `moddeps.py` output |
| F3 | **`spec.rs` is several modules in one file**, with clean layers. Cross-region references are few (the table in PR 2). | `xref.py` output |
| F4 | **Inside `spec.rs`, the write path depends on reporting code.** `submit`/`next_task` call `prose_smells`, which lives in the coverage region. Same for `body_smells` (feature, rollup, system). | `xref.py` |
| F5 | **Spec writes are not atomic.** `spec::write` (`spec.rs:1532`) is a plain `fs::write`. Only the migration path uses temp file plus rename (`write_replacing`, `spec.rs:2122`). A crash mid-write leaves a torn spec in git. | `spec.rs:1532-1539` |
| F6 | **Two one-off migrations live in the library API.** `backfill_dep_targets` and `migrate_file_fingerprints` (~200 lines) sit inside `spec.rs`, each called from one CLI arm. | `main.rs:109`, `main.rs:131` |

Edges that make up F2:

| Edge | Where | What is used |
|---|---|---|
| `graph → stack` | `graph.rs:463` | `Graph::file_role` looks the pack up by name |
| `graph → lang` | `graph.rs:462` | the `FileRole` type |
| `graph → resolve` | `graph.rs:26, 351, 559` | `ResolvedImport`, `ResolvedDefaultImport` (language-neutral types that live in the TypeScript resolver module; `rust.rs` imports them from there too) |
| `rust → spec` | `rust.rs:529` | `leading_comment_block`, a text utility |
| `search → spec` | `search.rs:121, 210` | `cap_source_text`, `within_scope` |
| `lang → extract`, `lang → stack`, `imports → lang`, `extract → lang`, `resolve → lang` | | `lang.rs` mixes pack-neutral items (`FileRole`, `SourceKind`, `detect`) with TypeScript-only internals (`ts_parser`, `RESOLVER_EXTENSIONS`, `extract_symbols`) |
| `features → stack`, `spec → stack`, `mcp → stack` | | by-name pack lookup (kept by design, see PR 1 note) |

### Dropped after checking

| Item from the first pass | Why dropped |
|---|---|
| Fold the four document parsers (`parse`, `parse_feature`, `parse_rollup`, `parse_system`) into one | They share ~12 lines (the opening and closing `---`). Their section handling really differs, and the file format nests per-symbol hashes. A shared helper would be more code than the duplication. |
| "291-line" and "188-line" functions in `mcp.rs` | Artefacts of my line-counting script. The real longest function is `get_spec` at ~192 lines including a nested helper. Not debt. |
| `StackPack` trait too wide | Mostly drop; one piece survives as PR 1 step 1d (default bodies for the two flow-edge methods). |
| Five production `expect`s in `spec.rs` | Each guards a filter a few lines above it. Leave. |
| Long functions (`next_task` 151, `submit` 106, `merge_inherent_impls` 211) | They move as a side effect of PR 2; not worth separate work. |
| Generic modules' tests build Rust fixtures | Small; leave. |

### Checked and healthy

- Concurrency: each `mcp` handler takes one snapshot of the graph, a single watcher thread rebuilds, and the graph is swapped whole (`ArcSwap`). No change needed.
- Only five `#[allow]`s, all `too_many_arguments` (four) or `only_used_in_recursion` (one).
- No TODO/FIXME comments. No pack-name branching in generic production code.

## The plan: three PRs, each on its own branch from `master`

Each PR gets `/code-review` before Sidd is asked to merge. Each commit compiles and passes `utility/check.sh`.

### PR 1 — layering and correctness (branch `pack-registry-layering`)

TDD: the registry test in 1b is written first.

| Step | Change | Removes |
|---|---|---|
| 1a | Move `leading_comment_block`, `cap_source_text` and `within_scope` into `text.rs` | `rust → spec`, `search → spec` |
| 1b | **One pack registry** in `stack.rs`, read by both `detect` and `for_name`. The empty name (test graphs) still means TypeScript; any other unknown name becomes an error, not a fallback. Failing test first: every registered pack round-trips through `for_name(pack.name())`, and an unknown name is rejected. | F1 |
| 1c | Move `ResolvedImport` and `ResolvedDefaultImport` into `graph.rs`. Replace `Graph::file_role` with a free function next to the registry. | `graph → resolve`, `graph → stack`, `graph → lang` |
| 1d | Move the TypeScript-only parts of `lang.rs` next to the TypeScript pack; `lang.rs` keeps the pack-neutral items. Give `extract_flow_edges` and `resolve_flow_edge` default bodies; this deletes four stub methods in the Rust and Java packs. | TypeScript code in the shared module |
| 1e | `spec::write` writes a temp file and renames it (reuse `write_replacing`). Failing test first if one can be written without fault injection; otherwise a test that no `.tmp` file is left behind. | F5 |

Acceptance:
- Re-run `moddeps.py`; report the remaining cycle members and say why each remains.
- Expect `features ↔ stack` to remain, because `StackPack::feature_model` names `FeatureModel` and `feature_model_for` looks the pack up by name. Removing it means passing `&dyn StackPack` through every spec function, which reverses an earlier deliberate choice. Keep it unless Sidd wants it revisited.
- `examples/graph_stats.rs` still shows 134 unresolved imports or fewer on CodeOwl itself.
- `FORMAT_VERSION`: bump only if a pack's output changes. None is expected, so check by diffing a built graph before and after.

### PR 2 — split `spec.rs` into `spec/` (branch `split-spec-module`, pure move)

| Module | Old lines (approx.) | Contents |
|---|---|---|
| `layout` | 33–197 | granularity rules, spec paths, spec-bearing predicates |
| `format` | 198–408, 1358–1545, 2176–2218 | `HashPair`, `FileSpec`, render/parse/read/write, `extract_section`, `upsert*`, `reorder_symbols` |
| `smells` | 3183–3280 | `prose_smells`, `body_smells`, `file_spec_smells`, moved below the write path (F4) |
| `staleness` | 1097–1357 | dependency hashes, `diff_hash_lists`, `symbol_changes`, `file_changes` |
| `task_input` | 409–1096 | the file-task ladder, paging, stand-ins, source capping |
| `file_task` | 1546–1875 | `SpecTask`, `next_task`, `submit` |
| `feature`, `rollup`, `system` | 2219–2554, 2555–2839 (+2840–2903 shared enumeration), 2904–3146 | one module per document kind |
| `coverage` | 3147–4046 minus smells | status, summary, orphans, prioritisation |
| `migrate` | 1876–2135 | the two one-off migrations (F6) |

Rules for the move:
- `spec/mod.rs` re-exports the public API **by explicit name, never with globs.** CodeOwl's own Rust resolver follows a flat `pub use` list but not a glob, so CodeOwl keeps indexing itself correctly.
- About 16 private helpers become `pub(super)`; fewer once `upsert*`/`reorder` are co-located with `file_task`.
- Tests move with the code they test. Shared fixtures go to `spec/test_support.rs`.
- No behaviour change: the passed-test count is identical before and after, and `utility/check.sh` is green on every commit.
- The external callers (`mcp.rs`, `main.rs`, `rust.rs`, `search.rs`, `explain.rs`, and tests under `tests/`) keep compiling through the re-exports. Nothing outside needs editing except paths already moved in PR 1.
- No `FORMAT_VERSION` bump (no pack output changes).

### PR 3 — optional: wire types out of `mcp.rs` (branch `split-mcp-wire`)

Move the ~770 lines of request/response types and their `From` impls to `mcp/wire.rs`, leaving the handlers in `mcp.rs`. Tidying only. Do it only if PR 2 makes the same treatment attractive; skip otherwise.

## Order and why

1. PR 1 first: it holds the correctness fixes (F1, F5), and moving the helpers out of `spec.rs` before splitting it avoids moving them twice.
2. PR 2 second: large but mechanical.
3. PR 3 only on request.

## Open decisions for Sidd

| # | Decision | Recommendation |
|---|---|---|
| D1 | Approve PR 1 as written, including that an unknown pack name becomes an error instead of the TypeScript fallback (1b) | Yes |
| D2 | Keep the by-name pack lookup (so `features ↔ stack` stays a cycle) | Keep |
| D3 | Do PR 3 at all | Skip for now |

## Not in this plan

Regenerating specs, the `talentTrail` fingerprint rules, the call-graph question (q18), and any new language pack. Phase 2's second language is helped by PR 1, which gives packs a single place to register.
