# Wiring CodeOwl into a repository

CodeOwl runs as an MCP server that a coding agent (Claude Code, and later
other MCP clients) queries for accurate structural facts and generated
specs about a target repository, instead of re-exploring it on every task.
This folder has everything needed to set that up against your own repo.

**Phase 1 scope:** the extractor handles TypeScript / TSX, and feature
detection assumes Next.js **App Router** conventions (`app/**/page.tsx`,
`app/**/route.ts`, `fetch("/api/...")`). Other stacks degrade to
"symbols + file specs, no feature layer" — see `REQUIREMENTS.md`.

---

## 1. Build the binary

Needs a Rust toolchain (`rustup`, stable).

```sh
git clone https://github.com/siddswork/codeowl
cd codeowl
cargo build --release
# -> ./target/release/codeowl
```

The binary is self-contained; note its absolute path for the next step.

## 2. Point an MCP server at your repo

Copy `setup/mcp.json` to `.mcp.json` at the root of the repo you want
CodeOwl to serve, and replace both paths with absolute ones:

```json
{
  "mcpServers": {
    "codeowl": {
      "command": "/absolute/path/to/codeowl/target/release/codeowl",
      "args": ["serve", "/absolute/path/to/your/repo"]
    }
  }
}
```

Restart Claude Code (or re-add the MCP server) in that repo. On startup
CodeOwl walks the tree, builds the graph, and — on later runs —
hash-checks every file against its cache and re-parses only what changed.
For the rest of the session an in-process file watcher keeps the graph in
step with your edits (re-parsed within ~1s, no restart).

Verify: ask the agent to call `get_spec_coverage` — it should report a
count of files, most of them `missing`.

## 3. Install the generation command

`setup/codeowl-generate.md` is a Claude Code slash command that drives the
spec-writing loop. CodeOwl never calls an LLM itself — it assembles
context and persists whatever the agent writes; this command is the
client-side half.

Copy it to whichever scope you want:

```sh
# just this repo:
mkdir -p /path/to/your/repo/.claude/commands
cp setup/codeowl-generate.md /path/to/your/repo/.claude/commands/

# or every repo on this machine:
cp setup/codeowl-generate.md ~/.claude/commands/
```

Re-copy it when this file changes upstream — it's the single source of
truth, kept versioned here rather than duplicated.

## 4. Generate the first specs

```
/codeowl-generate --all --budget=20      # prioritized batch: system spec,
                                          # then features, then files by
                                          # import fan-in
/codeowl-generate lib/utils.ts            # one file (+ its symbols)
/codeowl-generate app/submit/page.tsx     # a feature entry point (+ its feature spec)
/codeowl-generate system                  # the whole repo, bottom-up
```

Specs land in `docs/specs/` as Markdown, mirroring the source tree
(`docs/specs/<path>.md` for a file, `docs/specs/_features/<slug>.md` for a
feature, `docs/specs/<dir>/_index.md` for a directory rollup,
`docs/specs/_index.md` for the system spec).

## 5. Commit, and tell agents to use it

- **Commit `docs/specs/` to git.** The hashes in each file's frontmatter
  are the staleness signal; they only work if the specs are versioned
  alongside the code.
- **Gitignore `.codeowl/`.** It's the local graph + index cache, rebuilt
  on demand — never commit it. Add `/.codeowl` to your repo's
  `.gitignore`.
- **Regenerate as its own commit**, not folded into a feature PR — a
  `/codeowl-generate` run mid-feature otherwise drags unrelated spec
  diffs into the diff.
- **Add a line to your repo's `CLAUDE.md`** so agents reach for CodeOwl
  first:

  > This repo has a CodeOwl MCP server. Before exploring the codebase,
  > check `get_spec` / `get_spec_coverage` for an existing spec, and use
  > `get_callers` / `get_callees` for impact questions.

---

## Day-to-day use

**Reading (most sessions):** `get_spec`, `get_symbol`, `get_callers`,
`get_callees`, `search_code`. The agent asks CodeOwl "how does X work" /
"what calls Y" instead of grepping.

**Refreshing (occasional):** when `get_spec` returns `stale` (code moved
since the spec was written) or reports `smells` (a deterministic quality
check distrusts the prose), re-run `/codeowl-generate` for that target.
`get_spec_coverage` lists everything outstanding in priority order.

**`stale` vs `smelly`:** `stale` is hash-based — an input moved.
`smelly` is content-based — the prose is a cop-out ("see the source") or
too thin to be useful, regardless of hashes. A spec can be `current` and
`smelly` at once; both are reasons to regenerate.
