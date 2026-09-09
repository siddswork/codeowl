---
mode: agent
tools: ['codeowl']
description: Generate (or refresh) specs via CodeOwl's MCP tools -- one target, the whole repo, or a budgeted batch
---

<!-- VS Code / GitHub Copilot port of setup/codeowl-generate.md (the Claude
Code slash command). The loop body below is identical to that file — keep
the two in sync; the only differences are this frontmatter and `$ARGUMENTS`
-> `${input:target}`. See setup/COPILOT.md. -->

Generate spec(s) for `${input:target}`, using CodeOwl's MCP tools
(`get_next_spec_task`, `submit_spec`, `get_spec`, `get_spec_coverage`).
This prompt is the *client-side* half of CodeOwl's generation loop:
CodeOwl itself never calls an LLM or writes prose — it only assembles
context and persists whatever you write. You are the one writing the
spec text.

The context a task hands you always reflects the current working tree:
CodeOwl's in-session file watcher re-parses edited files within about a
second, so if you changed code earlier in this session you can run this
prompt straight away — the `source`/`dependencies` you get back are
already up to date, no server restart.

`${input:target}` is one of:
- A repo-relative file path (e.g. `src/util.rs`, `lib/utils.ts`), a
  feature entry point — one of your stack's entry points, whatever form
  they take (a Next.js page, an orphan API route / webhook with no page
  referencing it, and so on) — or a directory path (e.g. `src`, `lib`)
  with at least two spec-bearing files in it. A CLI or library stack has
  no feature entry points at all; that's expected, not a misconfiguration.
- `feature:<slug>` or `rollup:<dir>` — the same ids `get_spec_coverage`
  reports; equivalent to naming the feature's entry point / the directory.
- `system` or `.` — the whole repo: every module directory, every
  feature, then the one system spec, all in one bottom-up sweep.
- `--all`, optionally with `--budget=N` — a *prioritized* batch instead of
  a plain sweep: high-fan-in files first, then feature specs, then the
  long tail of files, then rollups, then the system spec last. See "Batch
  mode" below.

You don't need to know in advance which single-target shape applies: the
loop below walks bottom-up (a file's symbols, then the file, then — only
if this file is also a recognized feature entry point — the feature; for
a directory, each of its files' own ladder, then the directory's own
rollup; for `system`/`.`, every module's and every feature's own ladder,
then the system spec) and just tells you what's next each time. If
`${input:target}` is empty, ask the user what to generate rather than
guessing.

**Read before writing, every time — this is not a formality.** Every
piece of `source`/`core_sources` a task hands you exists to be read in
full before you write anything, not skimmed for a plausible-sounding
sentence. Two concrete failure modes to actively avoid, both observed in
real generated output:
- A feature task with several `core_sources` entries and the narrative
  only describing one of them, because the others were never actually
  read. If there are three core files, the document needs to account for
  all three — if you're not sure what one of them does, read it before
  writing, don't omit it.
- Writing "see the source/route handler for details" anywhere in a
  `### Behavior`, `## Rules & failure modes`, or similar section. That
  sentence defeats the document's entire purpose — whatever "the source"
  would tell the reader is exactly what belongs on the page instead. If
  you don't yet know what to say there, that's a signal to go read more,
  not to write that sentence.

## Loop

Repeat the following until `get_next_spec_task` returns `{"kind": "done"}`:

1. Call `get_next_spec_task` with `target` set to `${input:target}` (or, in
   batch mode, the current item's `id` — see "Batch mode" below).
2. If the result's `kind` is `"done"`, stop — everything on this target
   already has a current spec. Report that and end.
3. Otherwise you'll get a task shaped one of five ways:
   - **`kind: "symbol"`** — write markdown containing exactly these two
     headings, in this order, each with real prose under it:
     ```
     ### Summary
     <one or two sentences: what this does and why it exists>
     ### Behavior
     <what it actually does: control flow, side effects, error/edge cases,
     anything a caller needs to know that isn't obvious from the signature
     alone>
     ```
     Base this **only** on `source` (and `dependencies`, `docstring`,
     `signature` — all provided in the task). Do not invent behavior the
     source doesn't show. Do not restate the signature or the dependency
     list as prose — CodeOwl already writes those deterministically; your
     job is exactly the part it can't derive from the graph.
   - **`kind: "file"`** — write one short paragraph of plain prose (no
     headings needed) summarizing what the file as a whole is for, given
     everything you now know about its symbols from the tasks you already
     completed. This becomes the file's own `## Summary`.
   - **`kind: "feature"`** — write the whole feature document in one go,
     starting with a `# Title` line (a human-friendly title — CodeOwl
     never synthesizes one), then these four headings:
     ```
     # <Title>
     ## Summary
     <what this capability is, who uses it>
     ## How it works
     <numbered flow, BA-followable, file references inline for devs>
     ## Data touched
     <tables, storage, external services>
     ## Rules & failure modes
     <business rules, edge cases, what breaks and how>
     ```
     Base this on `entry_point`, every file in `core_sources` (the
     feature's own code — read all of them before writing anything: the
     narrative usually spans more than one), `dependencies` (each already
     has a summary-or-stub — read, don't re-derive), and `data` — the SQL
     tables this feature's code queries, each with its real column list
     (`table(col, col, …)`). Name those tables and columns in **## Data
     touched** rather than guessing from `.select()` calls. **If a
     `core_sources` file renders an imported component that isn't itself
     in `core_sources` (a `<Form/>` from a shared components directory,
     say) and that component carries real feature logic — form state, the
     submit/save handlers, validation — open and read its source before
     writing the flow.** CodeOwl pulls co-located and data-touching
     components into `core` automatically, but a component rendered
     indirectly (via a variable, a `.map()`, `React.createElement`) can
     still be missed. This is the one document a BA should be able to read
     start to finish and understand the capability without opening any
     source file — write for that reader, with file references as an aside
     for devs, not the other way around.
   - **`kind: "rollup"`** — write one short paragraph of plain prose (no
     headings needed) synthesizing what the directory as a whole is for,
     based on `files` (each entry is that file's own already-generated
     `## Summary` — read every one, don't re-derive from source, and don't
     re-read the files themselves: the whole point of a rollup is that it
     costs nothing beyond what's already been generated). This becomes the
     directory's own `## Summary`; CodeOwl fills in the per-file listing
     underneath it deterministically.
   - **`kind: "system"`** — write one short paragraph of plain prose (no
     headings needed) starting with a `# Title` line (a product name —
     CodeOwl never synthesizes one), synthesizing what the product as a
     whole does from `modules` (each entry is that module's own rollup
     summary) and `features` (each entry is that feature's own title +
     summary) — read every one, don't re-derive from source, don't
     re-read any of the underlying files. This is the top-level document
     a BA (or anyone new to the repo) should read first.

     **If `features` is empty** — the stack has no feature layer (a CLI, a
     library: no routes, no enumerable entry points) — a single paragraph
     rarely does the codebase justice, because the thing a reader most
     wants to know is how its major flows work end to end and no feature
     spec covers that. After the `# Title` line and the summary paragraph,
     add a `## Key flows` section: name the codebase's principal
     end-to-end flows (3–6 of them) and, for each, a short paragraph
     tracing which modules it crosses in order. You identify these
     yourself from the module summaries — CodeOwl can't enumerate them —
     so this section is closer to a `feature` narrative in spirit than to
     the usual one-paragraph system spec.

   **Reconciliation (`prior`/`prior_summary`/`prior_behavior`, when
   present and non-null):** the source changed *and* a human had hand-
   edited this exact spec since it was last machine-written — the value
   is their edit. Preserve whatever in it is still accurate; change only
   what the actual source diff affects. Don't silently discard a human's
   correction just because you're rewriting the section — read it first,
   the same way you'd read `source`.

   **You may see the same target offered again even though nothing in
   `source`/`dependencies` looks different from last time, and `prior` is
   `null`.** That means a *previously stored* spec (from an older run)
   failed a deterministic quality check (a "see the source" cop-out, or
   prose too short to be real) — not that the source changed again. Write
   genuinely better content this time: more specific, actually describing
   behavior.
4. Call `submit_spec` with `id` set to the task's `id` and `content` set
   to what you just wrote. **`submit_spec` rejects content that fails the
   same quality check** (Summary/Behavior under four words, or a "see the
   source" phrase) — the error names what tripped it; expand that section
   and submit again rather than moving on.
5. Go back to step 1.

## Batch mode (`--all` / `--all --budget=N`)

Use this when `${input:target}` starts with `--all`, instead of the
single-target loop above:

1. Call `get_spec_coverage` (no `scope`, unless the user named one) to
   get `pending`: every non-current document, already in the order a
   budgeted run should spend on — high-fan-in files first (their
   summaries feed every dependent spec), then feature specs, then the
   long tail of files, then rollups, then test-code file specs, then the
   system spec last. Each entry's `id` is ready to use as-is: a file
   path, `feature:<slug>`, `rollup:<dir>`, or `system` —
   `get_next_spec_task` accepts all of them.
2. If `--budget=N` was given, you have `N` **generations** to spend —
   count every `get_next_spec_task` call that returns a real task (`kind`
   is not `"done"`) toward that budget, not every item in `pending` (a
   single file with three uncovered symbols costs four generations: three
   symbols plus the file itself). Without `--budget`, spend as many as it
   takes to exhaust `pending` entirely.
3. Walk `pending` in order. For each item's `id`, run the single-target
   loop above (steps 1–5) against it — passing that `id` straight to
   `get_next_spec_task` as `target`, no translation. Stop the *whole*
   batch the moment your generation count would exceed the budget, even
   mid-item; don't finish an in-progress item "for free." (The `system`
   entry only becomes generable once everything above it is current, so
   on a partial run you'll stop before reaching it — that's expected.)
4. When you stop (budget exhausted or `pending` fully drained), report
   concisely: how many generations you spent and on what, broken down by
   kind — *N feature specs, M file specs, K rollups, system spec: yes/no*.
   If budget-capped, call `get_spec_coverage` once more and tell the user
   what's still pending (again by kind), so they know whether to run
   another batch or target something specific. If the system spec is the
   only thing left, say so and suggest running this prompt with `system`.

## Termination and reporting

`get_next_spec_task` is stateless and safe to call repeatedly — if a
symbol's (or a feature's participant's) source hasn't changed since it
was last generated, it's skipped automatically (no LLM call happens for
it), so re-running this prompt on an already-current target is a fast
no-op that reports nothing left to do.

When the loop ends, report concisely: which symbols/file/feature/rollup/
system spec got a spec written or refreshed, and where it landed
(`docs/specs/<path>.md` for a file, `docs/specs/_features/<slug>.md` for
a feature, `docs/specs/<dir>/_index.md` for a directory rollup,
`docs/specs/_index.md` for the system spec — see `ARCHITECTURE.md`'s
"Spec document format"). If `get_next_spec_task` returns `{"kind":
"done"}` on the very first call, say why: either nothing changed since it
was last generated, or the target doesn't qualify for a spec at all (a
barrel file with no exported function/class and not a feature entry point
either, or a directory with fewer than two spec-bearing files — see the
granularity rules in `ARCHITECTURE.md`).
