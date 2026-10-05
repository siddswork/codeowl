# Plan: the file-task ladder (exp-05 step 4)

Status: **approved by the owner 2026-10-05 (numbers as proposed; lifecycle-test rewrite moved to step 5). Nothing is built.** Branch `file-task-ladder`. Tests first.
Decided already (owner): see `DECISIONS.md` q22 and `experiments/exp-05-file-summary-input.md`. Items marked **PROPOSED** are mine and need approval.

## Goal
A file summary stops being written from the first 8,000 bytes. `get_next_spec_task` for a file hands the writer the **whole file**, in pages, or a deterministic stand-in for it when the file is too big.

## What the code does today (verified 2026-10-05)
- `spec::next_task` (`src/spec.rs`) walks the file's `spec_bearing_children` (`Callable` and `Container` only; `Value` and `Schema` symbols get no spec) and returns the first symbol that is missing, stale, smelly or human-edited. **It reaches the `SpecTask::File` arms only after every spec-bearing symbol is current.** So decision 2 (summaries rung only when every symbol spec is current) already holds by construction and needs a test, not new logic.
- `mcp.rs` (`resolve_next_task`, `SpecTask::File` arm, ~line 1038) reads the whole file and applies `cap_generation_text(source, max_spec_task_bytes)` (8,000 bytes). `SpecTaskResponse::File { id, source, prior }`. `GenerateTaskRequest` has only `target`.
- The Rust extractor skips `#[cfg(test)]` modules (`rust.rs::is_cfg_test`), so test code is never in the graph, but **the raw file text still contains it**.
- The server holds only `graph.pack_name()`, not a pack object.
- Staleness of a file is `file_changes` against `source_hash` of the whole file. **Unchanged in this step.**

## Design

### Input kinds (the response says which one it is)
| `input_kind` | When | Content |
|---|---|---|
| `raw` | file text (after Rust test stripping) fits in `P` pages or fewer | the file text, paged |
| `summaries` | more than `P` pages and the file has at least `S` spec-bearing symbols | per symbol: signature, then its stored `### Summary`; paged |
| `outline` | more than `P` pages and the file is dominated by one `Container` | the file's top comment, the container's signature and docstring, then every member's signature and docstring in file order (same data `maybe_reduce_container_source` already uses); paged if still large |
| `raw` (fallback) | more than `P` pages, too few symbols for `summaries`, no dominant container | raw, paged, still complete (no cut) |

### Numbers (**PROPOSED**)
- Page size `G` = the existing `max_spec_task_bytes` (8,000 by default), cut on a **line boundary** (a single line longer than a page is cut on a char boundary). Reuses the existing flag.
- `P` = 12 pages (decided).
- `S` = 8 spec-bearing symbols. Exp-05 does not give a number; its summaries files had 23 to 46. Revisit with the first real run.
- Dominant container: one `Container` whose span is at least 70 % of the file's bytes.

### Paging protocol (decided: one writer reads all pages, submits once)
- `GenerateTaskRequest` gains optional `cursor: Option<String>`.
- `SpecTaskResponse::File` gains `input_kind`, `page` (1-based), `pages`, `next_cursor: Option<String>`; `source` is the current page. `prior` only on page 1.
- **Cursor** (**PROPOSED**): the page number as a decimal string plus a short hash of the full input, `"<page>:<hash12>"`. If the input changed between calls (file edited, symbol spec regenerated) the hash differs and the call returns an error telling the writer to restart from page 1. Stateless.
- A cursor on any non-file target is an error. Page 1 = no cursor.
- Files that fit in one page behave as today minus the truncation marker: `page 1`, `pages 1`, `next_cursor null`.

### Rust test stripping (decided: drop the `#[cfg(test)]` module before paging)
- The pack, not `mcp.rs`, owns it. `StackPack` gains `fn strip_test_code(&self, rel_path: &str, source: &str) -> String` with a default of returning the source unchanged; `RustStack` removes `#[cfg(test)]` module items (tree-sitter, reusing `is_cfg_test`).
- **Open wiring detail** (**PROPOSED**): the server has only `pack_name()`. Smallest change: the server resolves the pack by name with a small lookup next to `Graph::build`'s callers, or `Graph` stores the pack's function. I will check which is smaller when writing the first test.
- Only the writer's input changes; the file's `source_hash` still covers the whole file (the fingerprint is step 5).

### Not in this step
Fingerprint, `FORMAT_VERSION`, migration (steps 5 and 0d), the `setup/codeowl-generate.md` and `.prompt.md` change (step 6), regenerating the 17 stale specs.

`FORMAT_VERSION` does **not** change: nothing in the graph or any pack's extract/resolve output changes. `strip_test_code` only feeds the generation task.

## Tests first (each is written failing, then made to pass)
1. `next_task` returns a symbol task, never the file task, while any spec-bearing symbol is missing or stale (decision 2). May already pass; it pins the guarantee.
2. A file at or under one page returns `raw`, `page 1 of 1`, no cursor, no truncation marker.
3. A file of N pages returns `page k of N` for each cursor, the pages concatenate to the full text exactly, and the last has `next_cursor: null`. Multi-byte text is never split.
4. Pages cut on line boundaries; an over-long single line is cut on a char boundary.
5. A cursor whose hash no longer matches the input errors with a "restart from page 1" message. A cursor on a symbol or directory target errors.
6. `prior` appears on page 1 only.
7. Rust: a file whose text is over the cap only because of a `#[cfg(test)]` module returns the stripped text (fewer pages); other packs are untouched.
8. More than `P` pages with at least `S` spec-bearing symbols returns `summaries`: each symbol's signature and stored summary, no top-level file summary.
9. More than `P` pages with one dominant container returns `outline` with every member's signature and docstring in file order (Java fixture).
10. More than `P` pages with neither returns paged `raw` (complete).
11. The lifecycle test `leaf_body_edit_stales_exactly_its_file_and_containing_rollup` keeps passing unchanged. **See the correction below.**

## Lifecycle test (resolved 2026-10-05)
The rewrite of `leaf_body_edit_stales_exactly_its_file_and_containing_rollup` moved to exp-05 step 5 (it depends on the file fingerprint). Here it must keep passing unchanged.

## Commit sequence (each compiles and passes `utility/check.sh`)
1. Tests 1 to 6 and the paging helper (`spec.rs`), `GenerateTaskRequest.cursor`, the new `File` response fields, `raw` kind. Replaces the cap for files only; symbol tasks keep `cap_generation_text`.
2. `StackPack::strip_test_code` and the Rust implementation (test 7).
3. `summaries` rung (test 8) and the `outline` rung (test 9), with the fallback (test 10).
4. Docs: `ARCHITECTURE.md` (file task shape, tool description for `get_next_spec_task`, no milestone numbers in agent-facing strings), `GLOSSARY.md` (page, rung, outline), `DECISIONS.md` and exp-05 status lines.

## Risks
- Raw text of 12 pages is about 70k tokens in one agent's context (decided, accepted).
- The writer ignoring `next_cursor` writes from page 1 only. Until step 6 changes the command, existing copies of `/codeowl-generate` do not page. **Do not run a generation between steps 4 and 6.**
- `S` and the 70 % rule are guesses; they need a first real run.
