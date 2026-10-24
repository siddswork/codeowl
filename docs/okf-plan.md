# Plan: make CodeOwl specs conform to OKF v0.2

OKF (Open Knowledge Format) is a Google Cloud spec for giving agents knowledge as a directory of markdown files with YAML frontmatter (`okf/SPEC.md` in `GoogleCloudPlatform/knowledge-catalog`, v0.2). A bundle is conformant if every non-reserved `.md` file has parseable frontmatter with a non-empty `type`, and any `index.md` / `log.md` follow their formats. Everything else is optional. `docs/specs/` is the bundle; a concept's ID is its path without `.md`.

Status (2026-10-23): decided with Sidd; reasoning in `DECISIONS.md` q27. PR 1 is merged (#113: renderers, `migrate-to-okf`, `STYLE.md` moved, docs). PR 3 (the migration run on `docs/specs/`) is done (#114, and #115 for the provenance line). PR 2 (links) is built: relative links, not the `/`-rooted ones written below, because on GitHub a leading `/` means the repository root (see q27).

## Decisions

| Area | Decision |
|---|---|
| `type` | Replaces `kind`, same values: `file`, `rollup`, `feature`, `system`. Checked: no code reads `kind` back (`parse` ignores it), and it is not in any hash. |
| `generated` | `generated: { by: CodeOwl }`. No `at` (the spec does not require it, and a rewrite on every regeneration would claim a change that did not happen). No link in the value: a markdown link inside a YAML flow map does not parse, and `by` is an actor name. |
| `title` | The path that is already the `# ` heading (system: the repo name). |
| `description` | A fixed sentence from path and type, never copied from the summary, so it never changes. File: "Specification for `src/hash.rs`." Rollup: "Overview of the `src/spec` directory." Feature and system similar. 23 of 59 file specs have no file summary yet, and a copied sentence would drift. |
| `resource` | File: relative path to the source file. Rollup: the directory. Absent for feature and system (abstract). |
| `sources` | Feature and system only: the files they cover. |
| Provenance line | `> Generated using [CodeOwl](...)` directly under the title, not hashed (first kept as "by", then changed to "using"; see q27). |
| Links | `### Depends on` lines become markdown links to the dependency's file spec (see PR 2). |
| `STYLE.md` | Moves to `docs/STYLE.md` (a file without frontmatter inside the bundle breaks conformance). |
| Skipped | `tags` (stack and file role are derivable from the graph, but the vocabulary would be ours and nothing reads it; Google's material says nothing about vector stores, so revisit only if one is actually planned, and then also rethink `description`), `verified` (we cannot tell who checked), `stale_after` (staleness is hashes, not dates), the computation family, `index.md`, `log.md`, `okf_version`, `usage_*`, footnote `sources`, and `status`. `status` was considered: only `draft` (the carried-over draft from `adopt-moved-specs`) could be written truthfully, and ordinary staleness is computed from hashes, so a stored value would go out of date; the drafts are about a hundred and the regeneration rewrites most of them. |
| Open | None. |

## Build order

1. **PR 1: frontmatter conformance.** Tests first: each renderer emits `type` and `generated`; an old `kind` file still parses; no hash moves. Change the four renderers. Add `codeowl migrate-to-okf <repo> [--write]` (dry run by default, idempotent, first built to edit the frontmatter only: `kind` to `type`, adds the new keys; never prose or hashes. It later also adds the provenance line under the title and links the `Depends on` entries; see q27). Move `STYLE.md` and update every path that names it (generate command and its port, `setup/README.md`, the orphan exemption). Docs: `DECISIONS.md` q27 plus its index row, the `ARCHITECTURE.md` format section and its four example blocks, a `GLOSSARY.md` entry, `setup/USAGE.md`. No `FORMAT_VERSION` bump (it covers graph extraction, not spec files).
2. **PR 2: links.** A new `linked_dependency_lines`, used only when a spec is rendered, writes `- [`src/graph.rs::Graph`](../graph.rs.md) — crate::graph` instead of plain text; `dependency_lines` stays plain for the quality check and the task. (The plan first said `/src/graph.rs.md`; the link is relative, see the status line.) Link whenever the target file will ever have a spec (decided from the graph by `file_is_spec_bearing`, not from the disk), even if it has none yet: OKF tolerates broken links, and linking only when the file exists would make the output depend on disk. A file that never gets one (constants-only, schema, barrel, generated) and a dependency in the same file stay plain text, since a link would be permanently dead or circular. Link to the file, no heading anchor (anchors collide). Checked: nothing reads `### Depends on` back (only `render` writes it), and staleness uses `deps_hash` from the graph, so this cannot cause false staleness; a test renders a spec with links and asserts it still reads as current and not smelly. Existing specs: first left to the regeneration, which was wrong, because a current spec is never offered for regeneration; `migrate-to-okf` now takes the graph and rewrites the existing entries (see q27).
3. **PR 3: run it on our specs.** `migrate-to-okf --write` on `docs/specs/`, then check all files with a throwaway script (frontmatter parses, `type` present).
4. **Resume the regeneration** (309 generations left), so the remaining specs are written in OKF with links.

## Why links

OKF links are the only built-in way to connect concepts. With them the folder becomes a graph a reader or tool can walk, and a future conversion of the specs into linked HTML pages (as GitHub renders markdown) gets its navigation for free.

## Not doing

Nothing for hypothetical consumers (Sidd is the only user). No conformance checker ships; the spec has three rules and a script verifies them once.
