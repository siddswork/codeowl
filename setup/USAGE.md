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
from what it walks — TypeScript + Next.js + SQL, Rust, Java, or Python +
FastAPI. A CLI or library stack (no routes, no framework entry points)
gets symbols, file specs, and rollups but no feature layer; its system
spec leads with a `## Key flows` section instead.

---

## Reading — you don't do anything

Once your repo's `CLAUDE.md` has the line from `README.md` step 5, the
agent queries CodeOwl on its own. You just ask normal questions and it
reaches for the index instead of grepping.

### Try these right now, on CodeOwl's own repo

You already have this repo cloned and built — no second checkout, no
other setup. Each tool call and its result below is real: run live
against *this* repo's own MCP connection while writing this doc, and
re-verified live again before this section was split — reproduce any of
them yourself, right now, against the exact code you just built. Two
natural phrasings are given per question, since the exact wording is
never the point — an agent reads intent, not a script, and either one
of a pair gets you the same place:

**What does `Graph` actually contain before I touch it?**<br>
**What fields and methods does `Graph` have?**<br>
> `get_symbol` — one call: signature, all 24 methods, no file open.

**What breaks if I change `Graph`'s public API?**<br>
**Who depends on `Graph`?**<br>
> `get_callers` — 13 files import it directly. Ask the same on
> `Graph::build`, a method instead of a type, and you get back
> `{"callers":[]}` — the "not a call graph" caveat below, proven, not
> just asserted.

**What does `graph.rs` itself depend on?**<br>
**What does `graph.rs` import?**<br>
> `get_callees` — its own resolved imports, separate from who depends
> on it.

**Can I trust `graph.rs`'s spec, or do I need to read the source myself?**<br>
**Is the documentation for `graph.rs` still accurate?**<br>
> `get_spec` on the file id. Right now, on this repo: `stale`,
> `changed: ["changed:source"]` — real drift, caught live.

**New to `src/` — what's actually in here?**<br>
**Give me an overview of `src/`.**<br>
> `get_spec rollup:src` — a real directory narrative, not a file
> listing.

**What should I document first?**<br>
**Where are the documentation gaps?**<br>
> `get_spec_coverage` — `top_stale_by_impact` names `graph.rs` first
> here (fan-in 27): fix that one before the long tail.

**What does `search_code` actually do?**<br>
**Show me `search_code`'s real implementation, not just the signature.**<br>
> `get_source`. `get_symbol` on `search_code` (the function this exact
> tool wraps) stops at `pub fn search_code(root: &Path, query: &str,
> opts: &SearchOptions) -> Result<SearchResults>` — the declared shape,
> nothing else. `get_source` on the same id returns the whole 83-line
> body. Neither `get_symbol` nor a spec (which can be `stale`,
> `missing`, or just prose) answers "what does this actually do" —
> `get_source` is the only one reading the real thing.

**Where's `DEBOUNCE` actually implemented — not just mentioned?**<br>
**Find every real use of `DEBOUNCE` in `watch.rs`, with context.**<br>
> `search_code` with `path` and `context_lines`:
> `search_code("DEBOUNCE", path: "src/watch.rs", context_lines: 2)`
> returns the constant's declaration *and* the
> `rx.recv_timeout(DEBOUNCE)` call that actually uses it, each with its
> real doc comment attached — one call, not a grep plus a manual
> file open.

If a spec exists and is current, the agent gets an accurate answer without
opening a file. If it's `missing` or `stale`, the agent falls back to
reading source — CodeOwl never blocks it, it just doesn't help as much.

### What this looks like on your own repo, if it's shaped differently

CodeOwl's own repo is a Rust **CLI/library** — no routes, no SQL schema,
so two real capabilities have nothing to show here no matter how you ask.
The examples below aren't runnable against this repo (and the target
repos they're really from aren't part of this project — one is a
private production app, the others are external open-source projects
this project happens to dogfood against), but each is a **real, live
result**, captured the same way as the examples above, against a real
repo of that shape — so you know what to expect once you point CodeOwl
at your own.

**TypeScript / Next.js, a routed web app (e.g. with Supabase):**
- `search_code("createClient", path: "lib", context_lines: 1)` found
  every real client-construction call across the codebase, each with
  its JSDoc attached — not just the definition, every call site too.
- `get_source` on a found symbol returned the real function body — a
  cookie-handling Supabase client constructor, in this case — the same
  gap it closes on any stack.
- On a repo with real routes, `get_spec feature:<slug>` is the
  highest-value question CodeOwl answers in one call: a feature spec
  traces one route's entry point through to its data access, instead
  of a hand-traced UI→API→DB walk. CodeOwl's own repo — the one you
  just built — is a CLI/library with no routes, so it has no feature
  layer at all and no `feature:<slug>` to ask; this is what you gain
  the moment you point CodeOwl at a real routed service instead.

**Python / FastAPI, with a SQLModel schema:**
- `search_code("table=True", context_lines: 2)` — the exact schema
  signal CodeOwl's Python support looks for — found the real table
  classes immediately, correctly skipping a base class one level up
  that shares the same parent but isn't itself a table.
- `get_symbol` on one of those classes came back `"kind": "schema"` —
  confirmation the schema hook fired, not just a class definition.
  `get_source` on the same id returned its real field list.

**Java / Quarkus, a CDI service:**
- `search_code("@ApplicationScoped", path: "<one-module>",
  context_lines: 1)` — the exact annotation CodeOwl's Java support
  treats as "this class is part of the service's core," scoped to one
  module — found the real service and repository classes.
- `get_source` on one of those classes, with `context_lines: 2`,
  correctly widened the returned span to include the trailing Javadoc
  line sitting just above the class's own annotation — not just the
  class body, the real doc comment that explains it too.

You can also **drive the tools yourself in chat** — this is a normal
workflow, not just something the agent does invisibly: *"ask codeowl who
calls `SymbolKind`"*, *"get_callees for `src/mcp.rs`"*, *"call
get_spec_coverage"*, *"search_code for `TODO`"*.

The *"how does X work?"* questions above are **reading** questions, asked
of specs that already exist. They are not how you generate — see below.

### How the tools actually behave

CodeOwl's 10 MCP tools are a thin layer over the resolved graph and the
persisted spec store — precise, but Phase-1 literal. Worth knowing before
you rely on an answer:

- **`get_symbol(id)`** — one symbol's record: signature, line range,
  docstring, `kind`/`raw`, and (for a type/class) every method it
  contains. The fast "where is this, what's its shape" lookup.
- **`get_source(id, context_lines?)`** — the **actual source text**,
  read off disk at the span `get_symbol` recorded — the one thing
  `get_symbol` (signature only, stops before the body) and `get_spec`
  (LLM-written prose *about* the symbol, which can be `stale` or
  `missing`) never give you. Accepts a symbol id or a bare file path.
  `context_lines` (default 0, capped at 20) widens the returned span —
  pull in the imports a snippet depends on, or the enclosing block.
  `truncated: true` means `source` was cut at the byte cap; `lines`
  always reports the true span regardless, so you know exactly what's
  missing. `graph_in_sync: false` means the file changed on disk more
  recently than the index caught up (the watcher debounces ~300ms) —
  `lines` may not point at the right place until that settles. Pure
  read: never touches a spec.
- **`get_callers(id)`** — the files that reference `id` through a
  **resolved `import` / `use` edge**, plus (for a SQL table) the files
  with a `.from("table")` that resolves to it. It is *not* a call graph.
  Two consequences: query the **type or the free function**, not a method
  (`Graph`, not `Graph::build` — a method is never imported by name, so
  it always comes back empty); and a name that's shadowed by a local of
  the same name is still counted (whole-word match, not scope analysis).
- **`get_callees(id)`** — what the **file containing `id`** imports.
  File-level, not per-symbol: naming any symbol in `mcp.rs` returns
  `mcp.rs`'s whole import list. `resolved_id` is set for intra-repo
  targets, null for external packages.
- **`get_spec(id)`** — the spec for a symbol id, file id, `feature:<slug>`,
  `rollup:<dir_path>`, or `system`. A **pure read** — never triggers
  generation, that's `/codeowl-generate`'s job. `status` is `missing`
  (nothing generated yet), `current`, or `stale` (last-known-good content
  returned, plus `changed` naming what moved). `smells` flags weak prose
  independent of `status`.
- **`explain_stale(id?)`** — why a spec is stale. Give it an id (a symbol,
  a file, `feature:<slug>`, `rollup:<dir>` or `system`) and it names the
  causes: the file's own text changed, or a named dependency changed
  (and whether its interface moved or only its source text), or a child
  spec was rewritten. Leave the id out and it counts causes across the
  whole repo and lists the dependencies that stale the most specs. It only
  reads; it never writes a spec. A spec written before CodeOwl recorded
  its dependencies says `unknown` for the dependency cause until it is
  regenerated, or until you run `codeowl backfill-dep-targets <path>`
  (add `--write` to apply it; without that it only reports). A file spec
  written before the file fingerprint existed also says `unknown`, with
  the reason, until it is regenerated or migrated (see "After upgrading"
  below).
  Ask your agent: *"why is `src/mcp.rs` stale?"*
- **`get_spec_coverage(scope?, cursor?)`** — the repo's spec inventory,
  current/stale/missing/smelly, plus `coverage` (what fraction has *any*
  spec) and `freshness` (of what exists, what fraction still matches the
  code) as two separate axes. Returns `pending`, the priority-ordered
  worklist `--all` spends down, and `generations_remaining`, the real
  `--budget=N` a full run would cost. `scope` narrows the file/rollup
  portion to a directory prefix. **`pending` is paginated** — 50 entries
  per call; `next_cursor` (non-null when there's more) is what you pass
  back as `cursor` for the next page. If you ask conversationally ("call
  `get_spec_coverage`", "what's the coverage") rather than running
  `/codeowl-generate`, the agent answering you decides how much of
  `pending` to show — a short summary (coverage %, top-impact files) is
  often the right call, but it should say when there's more beyond what
  it's showing, not just quietly leave it out. On a Java repo it also
  returns `generated_sources` — `{checked_dirs, found}`, how many files
  sit under a known build-generated-source directory (Maven's
  `target/generated-sources`, Gradle's `build/generated`); `found: 0` is
  the signal to run a real build (`mvn compile`, not `generate-sources`
  alone — see `setup/codeowl-generate.md`) before generating, or entry
  points hidden behind a build-generated interface won't be found. `null`
  on every other stack. Every other field (`missing`, `by_kind`,
  `by_module`, `top_stale_by_impact`, `orphaned`, `generated_sources`) is
  already whole-repo regardless of pagination — only `pending` itself is
  paged.
- **`get_next_spec_task(target, cursor?)`** / **`submit_spec(id, content)`** —
  the generation loop's two halves: the first hands back the next
  uncovered unit with its source and dependency specs pre-assembled, the
  second persists what your agent wrote. A file too big for one response
  comes in pages (`cursor` fetches the next one; see "Big files" below),
  and a unit that already has a spec comes with the old text, so the agent
  revises it instead of starting over. `/codeowl-generate` drives both — see
  "How generation works" below; call them yourself only if you're
  building your own generation client.
- **`search_code(query, path?, ignore_case?, context_lines?,
  max_results?)`** — plain regex over every tracked file
  (`.gitignore`-respecting, no index) — the same as running `rg`
  yourself, now with `rg`'s own ergonomics: `path` scopes to a file or
  directory (a true boundary, not a bare prefix — `"lib"` never matches
  `"library/foo.ts"`); `ignore_case` for case-insensitive matching;
  `context_lines` (capped at 5) pulls surrounding lines into each
  match's `context_before`/`context_after`; `max_results` narrows the
  default 200-match cap, never widens it. Each match's `text` is capped
  at 500 bytes with `truncated: true` if cut (the match itself is kept,
  never the tail); `context_unavailable: true` on a match means context
  was asked for but a second read of that file failed in the moment
  between finding the match and re-reading it — distinct from context
  legitimately being empty. The whole response carries its own
  `truncated: true` when the match cap *or* a roughly 100 KB total
  response-size budget stopped the walk short — narrow `path` or the
  query itself to see the rest; the size budget isn't something you can
  raise from the request.

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
  cron targets). Each gets a mechanical slug (`app/checkout/page.tsx` →
  `checkout`); the human-readable title is written by the agent. A feature
  is written once its entry point's file spec is done. **A CLI/library
  stack has no feature layer** — this list is empty and that's expected.
- **System** — one whole-repo spec, written last, composed from every
  feature and directory spec (or, with no feature layer, from the
  directory rollups plus a `## Key flows` section the agent writes).

A CodeOwl "feature" is **route-shaped** — one entry point plus whatever it
reaches through a `fetch()` literal. A capability you'd name in
conversation ("checkout") may span several pages and routes, so it can
end up as several feature specs, tied together by the directory rollups
and the system spec.

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
/codeowl-generate lib/stripe.ts         # one file and its symbols
/codeowl-generate app/checkout/page.tsx # one feature + its entry file
/codeowl-generate feature:checkout      # the same, by coverage id
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

**Regenerating starts from the old text.** When a spec is stale because the
code changed and nobody edited it by hand, the task carries the last
version the agent wrote (`previous_summary` and `previous_behavior` for a
function or class, `previous` for a file's summary). The agent revises it:
it keeps every claim that is still true, re-checks each against the current
source, and drops nothing without a reason. A draft written from the
source alone can lose a caveat the old text had, such as "this matches by
text, so a name in a comment also counts", unless the agent finds it
again; starting from the old text keeps it. The old text is left out when
it fails the quality check (a "see the source" stub), and a hand-edited
spec is handled separately (see the FAQ on hand edits).
Review the diff of a regenerated spec the way you would a code change: a
caveat that disappeared is worth a second look.

### Big files: pages, and the three numbers you can change

A file summary is written from the whole file, so a big file arrives in
**pages** (each up to `--max-spec-task-bytes`, default 8,000 bytes). The
agent reads every page in order, then writes one summary. Past a certain
size CodeOwl stops handing over the raw text and hands over a stand-in: each
symbol's written summary, or an outline of the one big class that makes up
most of the file. These `codeowl serve` flags decide where those lines fall;
set them in the `args` of the MCP client's server entry (`.mcp.json` for
Claude Code, `.vscode/mcp.json` for VS Code), then reconnect the server:

| Flag | Default | What moving it does |
|---|---|---|
| `--max-file-pages` | 12 | Raise it to read more raw text before switching to a stand-in; lower it to switch sooner. |
| `--summaries-min-symbols` | 8 | The fewest top-level functions and classes a too-big file needs before its summary is written from their summaries. Lower it to use summaries sooner. |
| `--outline-container-percent` | 70 | The share of the file, in percent of its bytes, one class must cover to be written up from an outline. Raise it to use the outline less often. |

You only add a flag if you want a value other than the default; a config
with just `["serve", "."]` already runs with 12, 8 and 70. In Claude Code's
`.mcp.json` the repo is the folder the session starts in, written `.`:

```json
{
  "mcpServers": {
    "codeowl": {
      "command": "./target/release/codeowl",
      "args": ["serve", ".", "--max-file-pages", "12",
               "--summaries-min-symbols", "8",
               "--outline-container-percent", "70"]
    }
  }
}
```

In VS Code's `.vscode/mcp.json` the repo is `${workspaceFolder}` (that
placeholder is VS Code's; Claude Code does not expand it):

```json
"args": ["serve", "${workspaceFolder}", "--max-file-pages", "12",
         "--summaries-min-symbols", "8", "--outline-container-percent", "70"]
```

Claude Code reads `.mcp.json` from the folder the session was started in, so
start it inside the repo; from a parent folder `/mcp` will not list the
server. `command` is the binary you built: `target/release/codeowl` after
`utility/release.sh`, or `target/debug/codeowl` after a plain `cargo build`.
After rebuilding, reconnect the server (`/mcp`, or start a new session).

The defaults are starting values, not measured limits past 12 pages.

### After upgrading: migrate old file specs

A file summary used to go stale on any edit to the file, a comment included.
It now goes stale only when something a summary could quote changed (for
Rust: the file's shape, its doc comments, or a number or long string in a
function body; other stacks still use the whole text until their rule
lands). Specs written before that have no `fingerprint` key on the file line
of their frontmatter, so they read as stale. To keep the ones that were
current, run once:

```
codeowl migrate-file-fingerprint <path>            # dry run: reports only
codeowl migrate-file-fingerprint <path> --write    # add the key
```

It adds the key only to a spec that was current before the change (its stored
text hash and dependency hash still match), changes nothing else in the file,
and leaves specs that were already stale stale. Running it twice changes
nothing the second time. Run it before the next `/codeowl-generate`. It skips
a spec that already has the key, so if a later release gives your stack a
new fingerprint rule, specs migrated under the old one read stale and are
regenerated; on a large corpus in a stack that has no rule yet (TypeScript,
Java, Python), it is worth waiting for that rule before migrating.

### After moving or splitting code: carry the specs over

A spec is keyed by the path-based id of the code it describes, so when code is
moved, split or renamed its spec is left behind at the old id (reported as
`orphaned` by `get_spec_coverage`) and the new location reads as missing. A
regeneration would write the same prose again. If the code itself did not
change, the old prose is still true. Before regenerating, run:

```
codeowl adopt-moved-specs <path>             # dry run: reports only
codeowl adopt-moved-specs <path> --list      # also one line per symbol
codeowl adopt-moved-specs <path> --write     # carry the prose over
```

For each orphaned symbol that has written prose it finds the symbol with the
same text now, and compares what the two depend on (by what the dependencies
look like, not where they live). Then:

- Same text, same dependencies: the prose is written to the new place as
  **current**, with the new place's hashes. No generation is needed for it.
- Same text but a dependency changed, or the same name with edited text: the
  prose is written as a **stale draft**. `get_next_spec_task` then hands the
  writer the old prose as `previous_summary` and `previous_behavior` to revise.
- Left where it is, and its document kept: prose a person edited, prose that
  fails the quality check, prose that names a file that no longer exists,
  several equally good successors, two orphans claiming one successor, a
  successor that already has different prose, a successor that is not a
  top-level function or class.

It removes an orphaned file document only when everything in it was carried or
has no successor, and it drops a section from a file that still exists once
that section's prose has moved or its code is gone. File, directory and system
summaries describe a whole and are regenerated, never carried. Whatever it
leaves is still listed as orphaned. A leftover section in a file that still
exists is dropped the next time that file's spec is written (a regeneration of
that file does it); nothing removes it in the background.

It compares against the code as it is now, so run it once the move is final:
a later edit to a moved symbol turns its old prose into a draft. Running it
twice changes nothing the second time, and a run that was interrupted is
finished by running it again. Commit its result on its own, before the
regeneration. `get_spec` stays a pure read; specs change only with `--write`.

---

## The command line: what each command is for

`codeowl` is one binary with two faces. Your agent talks to `codeowl serve`
over MCP and never types a command. The other four commands are for you, in
a terminal, for the jobs that rewrite or inspect files on disk. They are
deliberately not MCP tools: a command that changes `docs/specs/` should run
because you ran it, not because an agent decided to.

| Command | Use it to | Changes `docs/specs/`? |
|---|---|---|
| `codeowl serve <repo>` | Serve the MCP tools to your agent. Your MCP config runs this; you rarely type it. | Not on its own. Specs are written only when your agent calls `submit_spec`. |
| `codeowl extract <repo>` | Print every symbol CodeOwl found, as JSON. A debugging and scripting aid. | No. |
| `codeowl adopt-moved-specs <repo>` | Keep the specs of code you moved, split or renamed. | Only with `--write`. |
| `codeowl migrate-file-fingerprint <repo>` | After upgrading, keep file specs that were current. | Only with `--write`. |
| `codeowl backfill-dep-targets <repo>` | Let `explain_stale` name the dependency behind an old spec. | Only with `--write`. |

What every command has in common:

- **One argument, the repo root.** It is the folder that holds `docs/specs/`.
  `.` works from inside the repo. A path that does not exist stops with
  `resolving <path> to an absolute path`.
- **A dry run is the default** for the three that change specs. Without
  `--write` they print what they would do and change nothing; with it they
  do it. Read the dry run first. Git is the undo: commit before you pass
  `--write`, and `git diff docs/specs` shows what changed.
- **The repo's stack is detected first**, the same way as for `serve`. In a
  folder CodeOwl does not recognise they stop with an error and touch
  nothing.
- **Every command, `serve` and `extract` included, writes a local cache
  folder, `.codeowl/`, in the repo** (so none of them runs on a read-only
  checkout). Each run reads the whole repo again and refreshes it. Keep that folder out of git. It holds nothing you
  cannot rebuild; deleting it only makes the next `serve` start slower.
- **Run them with the same `codeowl` binary your MCP config points at.** A
  run from an older build can apply an older rule.

### `codeowl serve <repo>`

Builds the index, keeps it in step with your working tree as you edit, and
answers MCP requests over stdio until the client disconnects. This is the
command in the `args` of `.mcp.json` (Claude Code) and `.vscode/mcp.json`
(VS Code); see [`README.md`](README.md). Running it by hand in a terminal
only waits for MCP input, which is not useful.

The first three flags cap how much text one response carries; the last three
choose how a file summary is built from a too-big file. All six are optional;
`["serve", "."]` alone runs with the defaults. After changing one, reconnect
the server (`/mcp` in Claude Code), because a running server keeps the values
it was started with.

| Flag | Default | What it limits |
|---|---|---|
| `--max-spec-task-bytes` | 8,000 | The hard ceiling on one `get_next_spec_task` response, and the size of one page of a big file. Set it below the size at which your client moves a response out of the conversation into a temp file (VS Code Copilot Chat does this above its own undocumented limit). |
| `--large-class-bytes` | 4,000 | A class (or struct, interface, enum with members) bigger than this is shown to the writer as its member signatures and docstrings, not every method body. |
| `--max-source-bytes` | 32,000 | The ceiling on one `get_source` response. Larger than the task cap on purpose: a verification read should get real code, not a prompt-sized fragment. |
| `--max-file-pages` | 12 | How many pages of raw text a file summary is written from before the stand-in takes over. |
| `--summaries-min-symbols` | 8 | The fewest top-level functions and classes a too-big file needs for the stand-in to be their written summaries. |
| `--outline-container-percent` | 70 | The share of a too-big file one class must cover for the stand-in to be an outline of that class. |

The last three are explained, with an example config, in
[Big files: pages, and the three numbers you can change](#big-files-pages-and-the-three-numbers-you-can-change).
Lower the first two if your client still spills responses to a temp file.

### `codeowl extract <repo>`

Walks the repo, parses every file, resolves the imports, and prints one JSON
array to stdout: one entry per symbol (function, class, constant, SQL
table). On CodeOwl's own repo that is about 1,260 entries and 0.9 MB. Each
entry has:

| Field | Meaning |
|---|---|
| `id` | The symbol's id, `<file>::<name>`, the same id the MCP tools take. |
| `kind`, `raw` | The general kind (`callable`, `container`, `value`, or `schema` for a SQL table) and the language's own word for it (`fn`, `class`, ...). |
| `file`, `lines` | Where it is: the repo-relative file and `[first, last]` line. |
| `signature`, `docstring` | The declaration line and its doc comment, if any. |
| `is_exported` | Whether code outside the file can import it. |
| `source_hash`, `interface_hash` | What spec staleness is judged by: the hash of its text, and the hash of its public shape. The second is `null` for anything that is not exported. |
| `markers` | The attributes or annotations written on it, as source text (`#[test]`, `@Override`, `@Order(DEFAULT_ORDER + 1)`). |
| `parent`, `children` | The symbol or file it sits in, and what sits inside it. |

It prints symbols, not import edges. Use it to check what CodeOwl sees in a
file, to feed a script, or to compare two runs:

```
codeowl extract . > symbols.json
jq -r '.[] | select(.is_exported) | .id' symbols.json | head
```

It never touches `docs/specs/`. Do not pipe it into a command that stops
early (`head`): `codeowl` reports "Broken pipe" and exits, which is harmless
but noisy; write it to a file first.

### `codeowl adopt-moved-specs <repo> [--list] [--write]`

Run it after you move, split or rename code and before you regenerate. A
spec belongs to the id of the code it describes, so moved code leaves its
spec behind (listed as `orphaned`) and the new place reads as missing. This
command finds, for each orphaned symbol, the symbol with the same text now,
and carries the prose over. Without `--write` it only reports; `--list` adds
one line per symbol and the reason a document stays. How it decides, what it
skips, and what it leaves for you are in
[After moving or splitting code: carry the specs over](#after-moving-or-splitting-code-carry-the-specs-over).

### `codeowl migrate-file-fingerprint <repo> [--write]`

Run it once after upgrading from a build that judged a file summary by its
whole text. It adds the new `fingerprint` key to file specs that were
current, so they stay current instead of reading stale. It changes nothing
else in the file and leaves stale specs stale. The dry run prints three
counts: how many it would migrate, and how many it skipped as already
migrated, already stale, or having no file summary yet. See
[After upgrading: migrate old file specs](#after-upgrading-migrate-old-file-specs).

### `codeowl backfill-dep-targets <repo> [--write]`

`explain_stale` answers "why is this spec stale" and, when a dependency is
the cause, names it. It can only do that for a spec that stored a record of
each dependency's hash at the time of writing (the `dep_targets:` block in
its frontmatter). Specs written before CodeOwl kept that record say
`unknown` for the dependency cause.

This command fills the record in for those old specs. It can only do so
honestly when the dependencies have not changed since the spec was written,
because the stored combined hash (`deps_hash`) must come out the same as
today's. So it:

- writes a record only where it reproduces the stored `deps_hash` exactly;
- skips a spec whose dependencies moved since, and says how many (those read
  `unknown` until regenerated);
- changes only the `dep_targets:` block, never prose or other keys.

The dry run prints how many owners (symbols and files) it would fill, in how
many spec files, and how many it skipped: moved, not checkable (the symbol
is gone or the source unreadable), already recorded, or having no
dependencies at all. The skipped specs are not lost: regenerating one writes
the record as part of the new spec. This is the least important of the
maintenance commands; skip it unless `explain_stale` keeps answering
`unknown` for specs you would rather not regenerate.

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

## Frequently asked questions

- **How does the graph know when I change code — is it on a timer, or does
  it rescan on every question?**

  Neither. An in-process file watcher reindexes incrementally the moment a
  file changes on disk (the ~1s figure above), plus a one-time catch-up
  pass on server start that hash-checks everything against
  `.codeowl/index` to cover changes made while nothing was running. The
  *graph* updates live; *specs* don't — regenerating is always the
  explicit `/codeowl-generate` step, never a side effect of reading.

- **What does `.codeowl/index` actually do — isn't `.codeowl/graph`
  enough on its own?**

  `.codeowl/index` is the raw, per-file parse cache the graph gets
  rebuilt from every time — not a second copy of the graph. One entry
  per file, keyed by its relative path:

  | field | what it holds |
  |---|---|
  | `source_hash` | a hash of that file's raw text — the diff key |
  | `symbols` | the file's parsed symbols, before cross-file resolution |
  | `imports` | the file's raw import statements, not yet resolved to a target |
  | `flow_edges` | raw, unresolved flow-edge strings (a `fetch(...)`, a `.from(...)`) |

  On a fresh server start, CodeOwl hash-checks every file on disk against
  the cached `source_hash` and re-parses only what's new or changed —
  an unchanged file reuses its cached symbols/imports/flow-edges, no
  re-parsing at all. The in-session file watcher does the same
  hash-check-and-re-extract, driven by filesystem events instead of a
  walk.

  `.codeowl/graph`, by contrast, is never diffed incrementally — it's
  thrown away and rebuilt from scratch out of whatever's currently in
  the index, every time. That's cheap because resolving imports and
  flow edges across already-parsed files is fast; it's the parsing
  itself (walking raw source) that's expensive. So only the index needs
  hash-diffing logic — the graph doesn't, because it's disposable and
  trivially regenerable from the index. Same safety net as the graph: a
  format or stack-pack mismatch discards the cache and falls back to a
  full walk, rather than trusting a partially-compatible one.

- **When several files change at once (e.g. a `git pull` while the
  server's already running), does each file trigger its own rebuild?**

  No — they're batched. The watcher collapses every filesystem event
  that lands within 300ms of the last one into a single pass, so a
  pull touching a dozen files becomes one rebuild, not a dozen. Within
  that batch, only files whose on-disk hash actually differs from
  what's cached get re-parsed; a file merely touched with identical
  content is skipped entirely. If nothing in the whole batch actually
  changed content, nothing happens — no rebuild, no write to disk.

  When something *did* change, the rebuild re-persists **both** files,
  not just the graph — `.codeowl/graph` and `.codeowl/index` are each
  fully rewritten to disk on every rebuild, not only the graph. The
  freshly rebuilt graph is also pushed into the running server's live
  view through an atomic pointer swap (`ArcSwap`), so a tool call
  that's already in flight — or arrives a moment later — always reads
  either the fully-old or fully-new graph, never something
  half-updated.

- **Once the graph is fresh, how does CodeOwl actually decide a
  *spec* has gone stale — against the graph, the index, or both?**

  Just the graph — the index never enters a staleness check; its only
  job is producing the graph, upstream of this. Every status check
  compares two values, computed fresh, against two values recorded in
  the spec's own frontmatter:

  | | computed fresh, right now | recorded in the spec's frontmatter |
  |---|---|---|
  | own text | `source_hash`, read straight off the graph node | `source_hash` |
  | dependencies | a hash walked over everything the symbol currently depends on, using their *current* `interface_hash` values | `deps_hash` |

  `deps_hash` is never a stored graph field on either side — the
  "current" side is recomputed from scratch on every single `get_spec`
  or `get_spec_coverage` call; it only gets *persisted* once, into the
  spec's frontmatter, when `submit_spec` last wrote it. So the real
  mechanism isn't "compare two cached numbers" — it's "recompute two
  values fresh from the current graph, and compare them against what
  the spec last recorded."

- **If I hand-edit a spec, does the next regeneration overwrite my fix?**

  No. Each spec carries two hashes — one for the source it describes, one
  for CodeOwl's own last-written prose. A mismatch on only the second
  means a human touched the text and the code didn't move, so the edit
  simply becomes the current spec. If the code changed too, your edit is
  fed back to the agent as the prior version to preserve and adjust, not
  silently discarded. If you did not edit it and only the code changed,
  the agent gets the last version it wrote instead, to revise.

- **Does this hold up on a large monorepo?**

  Phase 1 is scoped to laptop-scale — a fresh clone hash-checks and
  parses once, then every later change is incremental (see "What CodeOwl
  stores" above). A heavier, shared-instance mode (index a whole org's
  repo centrally, update on push/merge) is a later phase, not something
  Phase 1 pretends to already be.

- **How does `smelly` actually get caught, beyond hashes matching?**

  A deterministic, non-LLM check (`prose_smells`) — a denylist of cop-out
  phrases ("see the source") plus a word-count floor — runs on both ends:
  reading a spec surfaces it as `smells` independent of `status`, and
  `submit_spec` runs the same check as a write gate, so a resubmission
  loop of equally-thin prose is rejected outright.

- **Can CodeOwl hallucinate a signature or dependency list?**

  No, structurally — neither is LLM output. The signature comes from
  extraction, the dependency list from resolved graph edges; CodeOwl
  writes both into the document itself. The agent only ever writes
  purpose/behavior/side-effects/failure-mode prose, which is also why the
  staleness hash covers only that prose, never the deterministic lines.

- **If I delete a file, does its old spec just sit there looking current?**

  No — that's a third bucket, `orphaned`, distinct from `stale`. A spec
  goes `stale` when its target still exists but moved; it becomes
  `orphaned` when the target is gone entirely (a deleted file, a removed
  route). Orphaned specs are excluded from `coverage`/`freshness`/`pending`
  — flagged, never mistaken for a real gap. If the code only moved or was
  renamed, run `codeowl adopt-moved-specs` before deleting anything.

- **If I change one widely-shared utility, does that cascade into
  regenerating half the repo?**

  No — invalidation propagates exactly one hop, keyed on a symbol's
  public shape (`interface_hash`), not its implementation. Changing what
  a function *does* invalidates nothing downstream unless its signature
  also changed, and even then only its direct importers go stale, not
  theirs in turn. `/codeowl-generate` also enforces a hard cap on how
  many nodes one run will regenerate before it stops and reports instead
  of continuing silently.

- **Are all nodes in the graph the same shape, or are there different
  types?**

  Two shapes, not one. Every entry in `.codeowl/graph`'s `nodes` array is
  tagged as exactly one of two kinds — never both, never a third:

  - **Symbol** — a class, function, method, database table, or anything
    else actually declared in the code
  - **File** — the file itself, a lighter entry that just anchors the
    symbols inside it

  You can see the split yourself:
  `jq '.nodes | map(keys[0]) | unique' .codeowl/graph` returns
  `["File", "Symbol"]`.

  A **File** entry is deliberately thin — just three fields:

  | field | what it means | example |
  |---|---|---|
  | `id` | the file's path, relative to the repo root | `"src/graph.rs"` |
  | `source_hash` | a hash of the file's whole raw text — moves if even one character changes | changes the instant you save any edit to this file, no matter how small |
  | `children` | the top-level symbols this file declares, in the order they appear | `["src/graph.rs::Graph", "src/graph.rs::FileNode"]` |

  No signature, no docstring, no export flag — a file doesn't have any
  of those on its own.

  A **Symbol** entry carries the full record:

  | field | what it means | example |
  |---|---|---|
  | `id` | this symbol's stable, permanent name | `"src/graph.rs::Graph::build"` |
  | `kind` | which of 4 generic buckets it falls into — `Container`, `Callable`, `Value`, or `Schema` | `Callable`, since `build` is a function |
  | `raw` | the exact keyword the parser actually saw, kept only for display | `"fn"` in Rust, `"class"` in TypeScript/Java |
  | `file` | which file this symbol lives in | `"src/graph.rs"` |
  | `lines` | the start and end line numbers it occupies | `[45, 62]` |
  | `signature` | its parameter list and return type (for a function), or its declared shape (for a type) | `"fn build(files: Vec<FileExtraction>) -> Self"` |
  | `docstring` | the doc comment written directly above it, if any | `"Builds a Graph from every file's symbols."` |
  | `is_exported` | whether code outside this file could ever import it — always `false` for a method | `true` for a public struct, `false` for a private helper function |
  | `source_hash` | a hash of this symbol's own text — for a class, folded together with every method's hash too | moves the moment you edit so much as a comment inside the function body |
  | `interface_hash` | a hash of just the public shape — the signature only, never the body; empty if `is_exported` is false | stays the same if you rename a local variable inside the function, changes if you add a parameter |
  | `markers` | any annotations on the declaration, kept as plain text | `["@Path(\"/users\")"]` for a Java endpoint, or empty for a plain function with none |
  | `parent` | which symbol directly contains this one — its class, or its file if it's top-level | the `CheckoutHandler` class this method belongs to |
  | `children` | the symbols this one directly contains | a class's list of methods; empty for a plain function |

  Within `Symbol` entries there's a second, lighter distinction —
  `kind` is just a tag, not a different shape. The 4 values:
  **Container** (a class/struct/enum — has members), **Callable** (a
  function or method), **Value** (a `const`/`static` — no spec of its
  own), **Schema** (a SQL table — resolvable, never spec-bearing). A
  Rust `struct` and a Java `class` both land as `kind: Container` with
  a different `raw` — the pipeline only ever branches on `kind`, never
  `raw`.

- **A container's `source_hash` "folds in" its members — what does that
  actually mean, and does `interface_hash` work the same way?**

  Picture a big box labeled `ShoppingCart` containing three smaller
  boxes: `addItem`, `removeItem`, `checkout`. Each small box has a
  sticker — a fingerprint of what's inside it. The big box's own
  sticker isn't just about its own wrapping paper; it's computed by
  gluing the three small boxes' stickers together and fingerprinting
  *that*:

  ```
  addItem's sticker:    A1
  removeItem's sticker: B2
  checkout's sticker:   C3
  ShoppingCart's sticker = fingerprint("A1" + "B2" + "C3")  →  X9
  ```

  Edit `checkout()`'s body (say, add a discount calculation) and its
  own sticker changes — `C3` becomes `C3-NEW`. Nobody touched `addItem`
  or `removeItem`, so their stickers stay the same. But `ShoppingCart`'s
  sticker was glued together *from* all three — one ingredient just
  changed, so `ShoppingCart`'s sticker changes too, even though its own
  outer code never moved:

  ```
  ShoppingCart's sticker = fingerprint("A1" + "B2" + "C3-NEW")  →  X9-NEW
  ```

  That's the whole trick — this "sticker" is `source_hash`, and a
  change anywhere inside ripples upward through every container above
  it, one level at a time. CodeOwl's own name for this is a **Merkle
  fold** (also called a rollup hash — see `GLOSSARY.md`), after Ralph
  Merkle's 1979 "Merkle tree": the same construction Git uses for its
  tree objects and Bitcoin uses for a block's transactions. It's why
  CodeOwl can answer "did anything change under here?" by checking one
  hash instead of opening every method individually.

  **Order counts too.** Reorder `addItem` and `removeItem` with zero
  logic changes — each method's own sticker is identical, but the
  *glued-together order* is different (`"B2"+"A1"+"C3"` instead of
  `"A1"+"B2"+"C3"`), so `ShoppingCart`'s sticker still changes.

  **`interface_hash` is a different sticker that deliberately skips
  this.** `source_hash` asks *"did anything change at all, even
  something invisible from outside the box?"*; `interface_hash` asks
  *"did what this box promises to the outside world change?"* — only
  the box's own label (what `checkout()` accepts and returns), never
  what's inside the smaller boxes. Rewrite `checkout()`'s internals
  without touching its parameters or return type, and `source_hash`
  moves but `interface_hash` doesn't — which is exactly why a function
  calling `ShoppingCart.checkout()` never goes stale over an internal
  rewrite; it only cares about the promise, and the promise didn't
  change.

  One thing worth flagging rather than assuming: `ARCHITECTURE.md`'s
  design notes also describe a *second*, file-level fold — a file's
  `interface_hash` as the hash of its exported children's
  `interface_hash`es. That one isn't actually implemented; a file node
  has no `interface_hash` field at all.

  What *is* implemented goes a step further than the `checkout()`
  example above: a container's `interface_hash` doesn't stop at its own
  signature, it also folds in every **public field**'s own signature
  (name + type, never its value/default, never a method). Add a field
  to `ShoppingCart`, or change an existing public field's type, and
  `ShoppingCart`'s `interface_hash` moves — which is exactly why a
  function that constructs a `ShoppingCart { total: … }` goes stale over
  that edit, the same way it already did for `checkout()`'s own
  signature. Methods are still excluded from the fold entirely (a method
  body *or* signature edit only ever moves `source_hash`, matching the
  `checkout()` example above unchanged), and a **private** field never
  contributes regardless of what it is — only a field reachable from
  outside the type counts as part of the promise.

---

## What Phase 1 does *not* do

- **Four stacks so far** — TypeScript + Next.js + SQL, Rust, Java, or
  Python + FastAPI. One per repo, auto-detected; a repo that looks like
  more than one is rejected rather than guessed (a polyglot mode — several
  stacks in one repo, mixed freely — is planned, not yet started). Other
  languages get no symbols.
- **The feature layer is stack-specific, and optional.** The Next.js pack
  assumes App Router (`app/**/page.tsx`, `app/**/route.ts`); FastAPI
  assumes route decorators and `Depends()`; the Java pack recognizes
  Quarkus's heterogeneous entry points (JAX-RS `@Path`, Kafka, `@Scheduled`,
  gRPC) when they're there. Rust and a plain Java library have no feature
  layer at all — a library has no routes to enumerate.
- **`get_callers` is import-edge, not call-graph** — see "How the
  structural tools actually behave" above.
- **`search_code` is plain regex** — no semantic / embedding search.
- **SQL is `CREATE TABLE` only** — no views, column types, or foreign-key
  edges (a `.from("view_name")` won't resolve).
- **No headless generation** — `/codeowl-generate` runs interactively in
  your session; there's no CI/cron runner yet.
