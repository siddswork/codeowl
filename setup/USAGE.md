# Using CodeOwl day to day

Setup (build, `.mcp.json`, installing `/codeowl-generate`) is in
[`README.md`](README.md). This is what to do once it's wired in.

CodeOwl gives you two things about the repo it serves:

1. **A structural map** — every symbol, file, import edge, route literal
   (`fetch("/api/...")`), and SQL table, kept in step with your working
   tree as you edit (re-parsed within ~1s, no restart).
2. **Prose specs** — LLM-authored descriptions of how the code works, at
   symbol / file / feature / directory / system level, committed to
   `docs/specs/` and content-hashed so they show as `stale` when the code
   moves underneath them.

CodeOwl never calls an LLM. It assembles context and stores results; your
agent writes the spec prose.

---

## Reading — you don't do anything

Once your repo's `CLAUDE.md` has the line from `README.md` step 5, the
agent queries CodeOwl on its own. You just ask normal questions and it
reaches for the index instead of grepping:

- *"How does artwork submission work end to end?"* → `get_spec feature:<slug>`
- *"What breaks if I change `getSupabase`'s signature?"* → `get_callers`
- *"What writes to the `payments` table?"* → `get_callers` on the table node
- *"Where's the retry logic for X?"* → `get_spec` / `search_code`

If a spec exists and is current, the agent gets an accurate answer without
opening a file. If it's `missing` or `stale`, the agent falls back to
reading source — CodeOwl never blocks it, it just doesn't help as much.

You can also query it directly in chat: *"ask codeowl for the spec of
`lib/utils.ts`"*, *"call get_spec_coverage"*.

---

## Generating and refreshing — you run `/codeowl-generate`

This is the only command you drive. It walks the
`get_next_spec_task → submit_spec` loop, writing specs into `docs/specs/`.

```
/codeowl-generate --all --budget=20     # prioritized batch across the repo
/codeowl-generate lib/utils.ts          # one file and its symbols
/codeowl-generate app/submit/page.tsx   # a feature entry point + its feature spec
/codeowl-generate system                # the whole repo, bottom-up
```

**When to run it:**

- **First time** — `/codeowl-generate --all --budget=N` in a few passes
  until `get_spec_coverage` stops reporting `missing`.
- **After a change** — when `get_spec` returns `stale` (an input moved) or
  lists `smells` (the prose is a cop-out or too thin — a content check,
  independent of hashes). `get_spec_coverage` lists everything outstanding
  in priority order.
- **As its own commit** — never fold a generation run into a feature PR,
  or unrelated spec diffs ride along. Regenerate, review, commit
  `docs/specs/` on its own.

`stale` vs `smelly`: `stale` = a hash moved, the code changed. `smelly` =
the prose itself is weak, regardless of hashes. A spec can be `current`
and `smelly` at once; both are reasons to regenerate.

---

## What Phase 1 does *not* do

- **TypeScript / TSX only.** Other languages get no symbols. The feature
  layer assumes Next.js App Router (`app/**/page.tsx`, `app/**/route.ts`).
- **`search_code` is plain regex** — no semantic / embedding search.
- **SQL is `CREATE TABLE` only** — no views, column types, or foreign-key
  edges (a `.from("view_name")` won't resolve).
- **No headless generation** — `/codeowl-generate` runs interactively in
  your session; there's no CI/cron runner yet.
