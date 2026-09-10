# Using CodeOwl day to day

Setup (build, `.mcp.json`, installing `/codeowl-generate`) is in
[`README.md`](README.md). This is what to do once it's wired in.

CodeOwl gives you two things about the repo it serves:

1. **A structural map** — every symbol, file, and import edge, kept in
   step with your working tree as you edit (re-parsed within ~1s, no
   restart). Plus, for the stacks that have them, string-carried edges the
   import graph can't see (a Next.js `fetch("/api/...")`, a Supabase
   `.from("table")`) and SQL `CREATE TABLE` nodes.
2. **Prose specs** — LLM-authored descriptions of how the code works, at
   symbol / file / feature / directory / system level, committed to
   `docs/specs/` and content-hashed so they show as `stale` when the code
   moves underneath them.

CodeOwl never calls an LLM. It assembles context and stores results; your
agent writes the spec prose.

**One stack per repo, auto-detected.** The extractor picks a `StackPack`
from what it walks — TypeScript + Next.js + SQL, or Rust. A CLI or library
stack (no routes, no framework entry points) gets symbols, file specs, and
rollups but no feature layer; its system spec leads with a `## Key flows`
section instead.

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

You can also **drive the tools yourself in chat** — this is a normal
workflow, not just something the agent does invisibly: *"ask codeowl who
calls `SymbolKind`"*, *"get_callees for `src/spec.rs`"*, *"call
get_spec_coverage"*, *"search_code for `TODO`"*.

The *"how does X work?"* questions above are **reading** questions, asked
of specs that already exist. They are not how you generate — see below.

### How the structural tools actually behave

The four read tools are a thin layer over the resolved graph — precise,
but Phase-1 literal. Worth knowing before you rely on an answer:

- **`get_symbol(id)`** — one symbol's record: signature, line range,
  docstring, `kind`/`raw`, and (for a type/class) every method it
  contains. The fast "where is this, what's its shape" lookup.
- **`get_callers(id)`** — the files that reference `id` through a
  **resolved `import` / `use` edge**, plus (for a SQL table) the files
  with a `.from("table")` that resolves to it. It is *not* a call graph.
  Two consequences: query the **type or the free function**, not a method
  (`Graph`, not `Graph::build` — a method is never imported by name, so
  it always comes back empty); and a name that's shadowed by a local of
  the same name is still counted (whole-word match, not scope analysis).
- **`get_callees(id)`** — what the **file containing `id`** imports.
  File-level, not per-symbol: naming any symbol in `spec.rs` returns
  `spec.rs`'s whole import list. `resolved_id` is set for intra-repo
  targets, null for external packages.
- **`search_code(query)`** — plain regex over every tracked file
  (`.gitignore`-respecting, capped, no index). The same as running `rg`
  yourself.

The tools give you the **edges** cheaply and reliably — things grep can't
see (a resolved re-export, an import vs. a same-named local) or can't
distinguish. The **narrative** ("this enum is load-bearing, these files
move together") is always the agent's synthesis on top; where a current
spec exists, `get_spec` on each result feeds that synthesis a paragraph
instead of a file.

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
- **Features** — discovered from your stack's entry-point convention, not
  from you. For the Next.js pack: every `app/**/page.tsx`, plus every
  `app/api/**/route.ts` that no `fetch("/api/...")` call reaches (webhooks,
  cron targets). Each gets a mechanical slug (`app/submit/page.tsx` →
  `submit`); the human-readable title is written by the agent. A feature
  is written once its entry point's file spec is done. **A CLI/library
  stack has no feature layer** — this list is empty and that's expected.
- **System** — one whole-repo spec, written last, composed from every
  feature and directory spec (or, with no feature layer, from the
  directory rollups plus a `## Key flows` section the agent writes).

A CodeOwl "feature" is **route-shaped** — one entry point plus whatever it
reaches through a `fetch()` literal. A capability you'd name in
conversation ("artwork evaluation") may span several pages and routes, so
it can end up as several feature specs, tied together by the directory
rollups and the system spec.

**`--all` order — you don't need to know the repo.** CodeOwl computes it
from the import graph. `get_spec_coverage` returns everything still
missing in this order, and a budgeted `--all` run spends down that list:

1. **the shared-code tier** — the ~8 most-imported files (shared clients,
   auth, core utils). Just those, not the whole `lib/`, and not
   `components/ui/` primitives however widely they're imported — capping
   the tier is what keeps features a couple of runs away, not a dozen.
   Once these 8 are done the tier is empty for good; it doesn't refill
   with the next batch. Imports from test files don't count toward fan-in.
2. **feature specs** — the BA-facing narratives, now written with real
   dependency context for the shared tier.
3. the **long tail** — every other file, by fan-in.
4. **directory rollups**.
5. **test code** — `e2e/`, `__tests__/`, `*.test.*`, `*.spec.*`. Gets
   file specs, but always after the product code and never a rollup or a
   line in the system spec. Documenting the test harness is rarely the
   point; when it is, it's safe to leave for last.
6. the **system spec** — last; it composes over everything above, so it's
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

`get_spec_coverage` reports **`generations_remaining`** — the exact
`--budget=N` a complete run over that scope would need, counting uncovered
symbols, not just documents (so "15 missing files" might be 240
generations). Each `pending` entry carries its own `generations` share, so
you can see which files are the expensive ones before you start.

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
- **A stale-only refresh** — `/codeowl-generate --all --stale --budget=N`
  regenerates only the specs the code moved under and leaves
  never-generated files alone. Cheaper than a full pass, lower-risk (it's
  editing existing prose, not writing new), and the natural thing to run
  after a batch of code changes when you want the committed corpus to stay
  honest but aren't ready to document the gaps.
- **As its own commit** — never fold a generation run into a feature PR,
  or unrelated spec diffs ride along. Regenerate, review, commit
  `docs/specs/` on its own.

`stale` vs `smelly`: `stale` = a hash moved, the code changed. `smelly` =
the prose itself is weak, regardless of hashes. A spec can be `current`
and `smelly` at once; both are reasons to regenerate.

---

## Splitting the work across a team

Documenting a large repo is a divide-and-conquer job, and CodeOwl is
built for it — the specs are committed content-hashed Markdown, so
independently generated pieces merge like code and every piece is
validated the same way regardless of who wrote it.

**The workflow:**

- Each person clones the repo and runs their own MCP server. Their
  `.codeowl/` cache is built locally and is per-machine (it's gitignored
  — see "What CodeOwl stores" below); nobody shares it.
- Divide by directory. Ask your agent: *"run `get_spec_coverage` scoped
  to `src/lib`, then generate what's missing there."* Or target a subtree
  directly — `/codeowl-generate src/lib --budget=20`. `get_spec_coverage`
  takes a `scope` (a directory prefix) precisely so two people can see
  non-overlapping slices of what's left.
- Each person commits their slice — `docs/specs/src/lib/…` — as its own
  PR, separate from any feature work.
- Merges are clean: different directories touch different `.md` files. If
  two people do land on the same file's spec, git merges the Markdown; a
  leftover conflict is resolved by hand or by re-running
  `/codeowl-generate <that-file>` once on the merged result.
- On any machine, `get_spec_coverage` reports the union of what's
  committed plus what that person has generated locally — so anyone can
  check what's left and claim a slice.

Feature and system specs are the exception to "divide by directory" —
they cross files, so one person should own each. `/codeowl-generate
system` is the last thing anyone runs, once every file and rollup is
current.

## What CodeOwl stores

Two places, with opposite lifecycles:

| | committed? | rebuilt? | safe to delete? |
|---|---|---|---|
| `docs/specs/**.md` | **yes** — reviewed in PRs, this is the product | no — the LLM writes it | no |
| `.codeowl/graph` | no — gitignore it | yes, from source | **yes** |
| `.codeowl/index` | no — gitignore it | yes, from source | **yes** |

- **`.codeowl/graph`** — the built structural graph (every symbol, every
  import edge), kept as plain JSON so you can `cat` / `jq` it.
- **`.codeowl/index`** — the per-file input cache: each file's extracted
  symbols, imports, and a hash of its text. On a fresh `serve`, CodeOwl
  hash-checks this against what's on disk and re-parses only the files
  that changed while nothing was running.
- Both are **per-machine, cheap to rebuild, and never shared**. A
  missing, unreadable, or format-outdated cache just triggers a full walk
  on the next startup (a few seconds on a laptop-scale repo). Deleting
  `.codeowl/` and restarting the server is the correct fix for any cache
  weirdness.
- `.codeowl/` carries a `format_version` stamp (see `GLOSSARY.md`); when
  CodeOwl's on-disk format changes, an older cache is discarded whole
  rather than migrated.

`ARCHITECTURE.md`'s "Storage" section is the design-level version of this.

---

## What Phase 1 does *not* do

- **Two stacks so far** — TypeScript + Next.js + SQL, or Rust. One per
  repo, auto-detected; a repo that looks like both is rejected rather than
  guessed. Other languages get no symbols. Java (plain, then Quarkus) is
  the next stack in.
- **The feature layer is stack-specific.** For the Next.js pack it assumes
  App Router (`app/**/page.tsx`, `app/**/route.ts`). The Rust pack has no
  feature layer at all.
- **`get_callers` is import-edge, not call-graph** — see "How the
  structural tools actually behave" above.
- **`search_code` is plain regex** — no semantic / embedding search.
- **SQL is `CREATE TABLE` only** — no views, column types, or foreign-key
  edges (a `.from("view_name")` won't resolve).
- **No headless generation** — `/codeowl-generate` runs interactively in
  your session; there's no CI/cron runner yet.
