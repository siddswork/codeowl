---
description: Generate (or refresh) the spec for one file or feature entry point via CodeOwl's MCP tools
argument-hint: <repo-relative-file-path>
---

Generate the spec for `$ARGUMENTS`, using CodeOwl's MCP tools
(`get_next_spec_task`, `submit_spec`, `get_spec`). This command is the
*client-side* half of CodeOwl's generation loop: CodeOwl itself never
calls an LLM or writes prose — it only assembles context and persists
whatever you write. You are the one writing the spec text.

`$ARGUMENTS` is always a repo-relative file path — a plain file (e.g.
`lib/utils.ts`) or a feature entry point (a page like
`app/submit/page.tsx`, or an API route with no page referencing it, like
a webhook). You don't need to know in advance which one it is: the loop
below walks bottom-up (a file's symbols, then the file, then — only if
this file is also a recognized feature entry point — the feature) and
just tells you what's next each time. If `$ARGUMENTS` is empty, ask the
user which file to generate rather than guessing.

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

1. Call `get_next_spec_task` with `target` set to `$ARGUMENTS`.
2. If the result is `null`, stop — everything on this target already has
   a current spec. Report that and end the command.
3. Otherwise you'll get a task shaped one of three ways:
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
4. Call `submit_spec` with `id` set to the task's `id` and `content` set
   to what you just wrote.
5. Go back to step 1.

## Termination and reporting

`get_next_spec_task` is stateless and safe to call repeatedly — if a
symbol's (or a feature's participant's) source hasn't changed since it
was last generated, it's skipped automatically (no LLM call happens for
it), so re-running this command on an already-current target is a fast
no-op that reports nothing left to do.

When the loop ends, report concisely: which symbols/file/feature got a
spec written or refreshed, and where it landed (`docs/specs/<path>.md`
for a file, `docs/specs/_features/<slug>.md` for a feature — see
`ARCHITECTURE.md`'s "Spec document format"). If `get_next_spec_task`
never returns anything at all on the first call, say why: either nothing
changed since it was last generated, or the target doesn't qualify for a
spec at all (a barrel file with no exported function/class, and not a
feature entry point either — see the granularity rules in
`ARCHITECTURE.md`).
