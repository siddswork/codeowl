---
description: Generate (or refresh) the spec for one file via CodeOwl's MCP tools
argument-hint: <repo-relative-file-path>
---

Generate the spec for the file `$ARGUMENTS`, using CodeOwl's MCP tools
(`get_next_spec_task`, `submit_spec`, `get_spec`). This command is the
*client-side* half of CodeOwl's generation loop: CodeOwl itself never
calls an LLM or writes prose — it only assembles context and persists
whatever you write. You are the one writing the spec text.

If `$ARGUMENTS` is empty, ask the user which file to generate rather than
guessing.

## Loop

Repeat the following until `get_next_spec_task` returns `null`:

1. Call `get_next_spec_task` with `target` set to `$ARGUMENTS`.
2. If the result is `null`, stop — everything on this file already has a
   current spec. Report that and end the command.
3. Otherwise you'll get a task shaped one of two ways:
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
4. Call `submit_spec` with `id` set to the task's `id` and `content` set
   to what you just wrote.
5. Go back to step 1.

## Termination and reporting

`get_next_spec_task` is stateless and safe to call repeatedly — if a
symbol's source hasn't changed since it was last generated, it's skipped
automatically (no LLM call happens for it), so re-running this command on
an already-current file is a fast no-op that reports nothing left to do.

When the loop ends, report concisely: which symbols/file got a spec
written or refreshed, and where it landed (`docs/specs/<path>.md`, per
`ARCHITECTURE.md`'s "Spec document format"). If `get_next_spec_task`
never returns anything at all on the first call, say why: either nothing
about this file changed since it was last generated, or the file doesn't
qualify for a spec at all (barrel files and files with no exported
function/class don't get one — see the granularity rules in
`ARCHITECTURE.md`).
