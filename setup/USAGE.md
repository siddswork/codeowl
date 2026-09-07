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

The *"how does X work?"* questions above are **reading** questions, asked
of specs that already exist. They are not how you generate — see below.

---

## How generation works

You choose a **scope**, not a list of things to document. CodeOwl
enumerates the targets inside that scope itself and hands your agent one
task at a time; the agent reads the source CodeOwl pre-assembled for that
task, writes the prose, and submits it. The loop repeats until there's
nothing left uncovered in scope. Your job is to run the command and
review what the agent writes — you never name a feature or topic in
prose.

What CodeOwl enumerates:

- **Symbols** — every exported function and class. A file's symbols are
  documented before the file itself.
- **Files** — every extractable file, once its symbols are done.
- **Directories** — a rollup per directory with ≥2 spec-bearing files,
  once its files are done.
- **Features** — discovered from Next.js routing, not from you. Every
  `app/**/page.tsx`, plus every `app/api/**/route.ts` that no
  `fetch("/api/...")` call reaches (webhooks, cron targets). Each gets a
  mechanical slug (`app/submit/page.tsx` → `submit`); the human-readable
  title is written by the agent. A feature is written once its entry
  point's file spec is done.
- **System** — one whole-repo spec, written last, composed from every
  feature and directory spec.

A CodeOwl "feature" is **route-shaped** — one entry point plus whatever it
reaches through a `fetch()` literal. A capability you'd name in
conversation ("artwork evaluation") may span several pages and routes, so
it can end up as several feature specs, tied together by the directory
rollups and the system spec.

**`--all` order — you don't need to know the repo.** CodeOwl computes it
from the import graph. `get_spec_coverage` returns everything still
missing in this order, and a budgeted `--all` run spends down that list:

1. **high-fan-in files** — code imported by many others (shared clients,
   auth, utils). Documented first because a feature spec that depends on
   one gets its real summary instead of a bare signature.
2. **feature specs** — the BA-facing narratives, now written with real
   dependency context.
3. the **long tail** of lower-fan-in files.
4. **directory rollups**, then
5. the **system spec** — last; it composes over everything above, so it's
   only writable once they're all current. A budgeted run stops before
   reaching it; do it explicitly with `/codeowl-generate system` at the end.

So the normal workflow is just: `/codeowl-generate --all --budget=15`,
review, commit, repeat — the tool picks the order. You'd see feature
specs within the first batch or two. Target something directly only when
you want to jump ahead:

```
/codeowl-generate lib/supabase.ts       # one file and its symbols
/codeowl-generate app/register/page.tsx # one feature + its entry file
/codeowl-generate feature:register      # the same, by coverage id
/codeowl-generate system                # the final capstone
```

**`--budget=N` counts specs written, nothing else** — not tokens, not
dollars, not time. One symbol spec, one file spec, one feature spec, one
directory rollup, or the system spec each count as 1; because generation
is bottom-up, a file with three undocumented symbols costs 4 (the three
symbols, then the file). The run stops the moment the next spec would
exceed the budget — even partway through a file — and reports how many
documents are still `missing`. Without `--budget`, it runs until the
whole scope is covered.

Start with `--all --budget=N` (N≈15 on a fresh repo), read what the agent
wrote, commit, then run it again for the next batch — a small budget is
what keeps each pass reviewable.

---

## When to (re)generate — you run `/codeowl-generate`

CodeOwl never calls an LLM; the command is the client-side half of the
loop, and your agent writes every word. It writes specs into
`docs/specs/`.

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
