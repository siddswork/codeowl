---
description: Generate (or refresh) specs via CodeOwl's MCP tools -- one target, the whole repo, or a budgeted batch
argument-hint: <repo-relative-file-path> | system | . | --all [--budget=N]
---

Generate spec(s) for `$ARGUMENTS`, using CodeOwl's MCP tools
(`get_next_spec_task`, `submit_spec`, `get_spec`, `get_spec_coverage`).
This command is the *client-side* half of CodeOwl's generation loop:
CodeOwl itself never calls an LLM or writes prose — it only assembles
context and persists whatever you write. You are the one writing the
spec text.

The context a task hands you always reflects the current working tree:
CodeOwl's in-session file watcher re-parses edited files within about a
second, so if you changed code earlier in this session you can run this
command straight away — the `source`/`dependencies` you get back are
already up to date, no server restart.

`$ARGUMENTS` is one of:
- A repo-relative file path (e.g. `lib/utils.ts`), a feature entry point
  (a page like `app/submit/page.tsx`, or an API route with no page
  referencing it, like a webhook), or a directory path (e.g. `lib`) with
  at least two spec-bearing files in it.
- `system` or `.` — the whole repo: every module directory, every
  feature, then the one system spec, all in one bottom-up sweep.
- `--all`, optionally with `--budget=N` — a *prioritized* batch instead of
  a plain sweep: system spec, then feature specs, then files by
  descending import fan-in, then everything else. See "Batch mode" below.

You don't need to know in advance which single-target shape applies: the
loop below walks bottom-up (a file's symbols, then the file, then — only
if this file is also a recognized feature entry point — the feature; for
a directory, each of its files' own ladder, then the directory's own
rollup; for `system`/`.`, every module's and every feature's own ladder,
then the system spec) and just tells you what's next each time. If
`$ARGUMENTS` is empty, ask the user what to generate rather than
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

Repeat the following until `get_next_spec_task` returns `null`:

1. Call `get_next_spec_task` with `target` set to `$ARGUMENTS` (or, in
   batch mode, the current item's `id` — see "Batch mode" below).
2. If the result is `null`, stop — everything on this target already has
   a current spec. Report that and end the command.
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
     narrative usually spans more than one), and `dependencies` (each
     already has a summary-or-stub — read, don't re-derive). This is the
     one document a BA should be able to read start to finish and
     understand the capability without opening any source file — write
     for that reader, with file references as an aside for devs, not the
     other way around.
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

   **Reconciliation (`prior`/`prior_summary`/`prior_behavior`, when
   present and non-null):** the source changed *and* a human had hand-
   edited this exact spec since it was last machine-written — the value
   is their edit. Preserve whatever in it is still accurate; change only
   what the actual source diff affects. Don't silently discard a human's
   correction just because you're rewriting the section — read it first,
   the same way you'd read `source`.

   **You may see the same target offered again even though nothing in
   `source`/`dependencies` looks different from last time, and `prior` is
   `null`.** That means the previously-submitted content failed a
   deterministic quality check (a "see the source" cop-out, or prose too
   short to be real) — not that the source changed again. Write genuinely
   better content this time: more specific, actually describing behavior.
   Resubmitting similarly thin content will just get offered back to you
   again.
4. Call `submit_spec` with `id` set to the task's `id` and `content` set
   to what you just wrote.
5. Go back to step 1.

## Batch mode (`--all` / `--all --budget=N`)

Use this when `$ARGUMENTS` starts with `--all`, instead of the single-
target loop above:

1. Call `get_spec_coverage` (no `scope`, unless the user named one) to
   get `pending`: every non-current document, already in priority order
   (system spec, then feature specs, then files by descending fan-in,
   then everything else).
2. If `--budget=N` was given, you have `N` **generations** to spend —
   count every `get_next_spec_task` call that returns a real task (not
   `null`) toward that budget, not every item in `pending` (a single file
   with three uncovered symbols costs four generations: three symbols
   plus the file itself). Without `--budget`, spend as many as it takes
   to exhaust `pending` entirely.
3. Walk `pending` in order. For each item's `id`, run the single-target
   loop above (steps 1–5) against it — but stop the *whole* batch the
   moment your generation count would exceed the budget, even mid-item;
   don't finish an in-progress item "for free."
4. When you stop (budget exhausted or `pending` fully drained), report
   concisely: how many generations you spent, on which documents, and —
   if budget-capped — call `get_spec_coverage` once more and tell the
   user how much is still pending so they know whether another budgeted
   run is worth it.

## Termination and reporting

`get_next_spec_task` is stateless and safe to call repeatedly — if a
symbol's (or a feature's participant's) source hasn't changed since it
was last generated, it's skipped automatically (no LLM call happens for
it), so re-running this command on an already-current target is a fast
no-op that reports nothing left to do.

When the loop ends, report concisely: which symbols/file/feature/rollup/
system spec got a spec written or refreshed, and where it landed
(`docs/specs/<path>.md` for a file, `docs/specs/_features/<slug>.md` for
a feature, `docs/specs/<dir>/_index.md` for a directory rollup,
`docs/specs/_index.md` for the system spec — see `ARCHITECTURE.md`'s
"Spec document format"). If `get_next_spec_task` never returns anything
at all on the first call, say why: either nothing changed since it was
last generated, or the target doesn't qualify for a spec at all (a barrel
file with no exported function/class and not a feature entry point
either, or a directory with fewer than two spec-bearing files — see the
granularity rules in `ARCHITECTURE.md`).
