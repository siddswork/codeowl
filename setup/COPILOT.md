# Wiring CodeOwl into VS Code + GitHub Copilot

[`README.md`](README.md) covers the Claude Code path. This is the same
setup for **VS Code with GitHub Copilot (Agent mode)** — an MCP server
for the structural queries, a prompt file for the generation loop, and a
custom-instructions line so the agent reaches for CodeOwl first.

Everything CodeOwl-side is client-neutral: the MCP tools, the graph, the
`docs/specs/` corpus. Only the three wiring files differ from Claude Code.

| piece | Claude Code | VS Code + Copilot |
|---|---|---|
| MCP server | `.mcp.json` (`mcpServers`) | `.vscode/mcp.json` (`servers`) |
| generation command | `~/.claude/commands/codeowl-generate.md` | `.github/prompts/codeowl-generate.prompt.md` |
| "use CodeOwl first" nudge | a line in `CLAUDE.md` | `.github/copilot-instructions.md` |

Requires **Agent mode** (Copilot Chat → mode dropdown → *Agent*) and a
recent VS Code — MCP support and prompt files are both GA as of 2025.
This page and the prompt file's frontmatter follow the current VS Code
docs ([MCP servers](https://code.visualstudio.com/docs/copilot/customization/mcp-servers),
[prompt files](https://code.visualstudio.com/docs/copilot/customization/prompt-files));
those pages move quickly, so if a field or command name doesn't match
what your VS Code expects, check them.

---

## 1. Build the binary

Same as the Claude Code path — needs a Rust toolchain (`rustup`, stable):

```sh
git clone https://github.com/siddswork/codeowl
cd codeowl
cargo build --release
# -> ./target/release/codeowl
```

Note its absolute path. (No pre-built binaries yet — if you're setting
CodeOwl up on a machine without a Rust toolchain, `curl --proto '=https'
--tlsv1.2 -sSf https://sh.rustup.rs | sh` then `cargo install --git
https://github.com/siddswork/codeowl` puts `codeowl` on `~/.cargo/bin`.)

## 2. Add the MCP server — `.vscode/mcp.json`

Open the config from the Command Palette (**⌘⇧P** / **Ctrl+Shift+P**) —
there's no default keybinding, but these commands exist:

| command | opens / does |
|---|---|
| **MCP: Open Workspace Folder Configuration** | creates/opens `.vscode/mcp.json` for this repo — paste the block below |
| **MCP: Add Server** | guided flow: pick `stdio`, paste command + args, name it `codeowl`, choose **Workspace** or **Global** |
| **MCP: Open User Configuration** | the user-level `mcp.json` (e.g. `~/Library/Application Support/Code/User/mcp.json` on macOS) — server available in every repo |
| **MCP: List Servers** | Start / Stop / Restart / Show Output / Show Configuration for each |

The 🛠️ **Configure Tools** button in Agent mode's chat box also has an
**Add MCP Server** entry that runs the same guided flow.

The block to paste (from `setup/mcp-vscode.json`), with the binary path
filled in:

```json
{
  "servers": {
    "codeowl": {
      "type": "stdio",
      "command": "/absolute/path/to/codeowl/target/release/codeowl",
      "args": ["serve", "${workspaceFolder}"]
    }
  }
}
```

`${workspaceFolder}` resolves to the repo root, so only the binary path
is machine-specific.

After saving, VS Code shows a **Start** codelens above the server block —
click it (or *MCP: List Servers* → *Start*). Once running, the tools
appear in Agent mode's 🛠️ tool picker. VS Code prompts to trust the
server the first time and whenever the config changes.

On startup CodeOwl walks the tree, builds the graph, and — on later runs
— hash-checks every file against its cache and re-parses only what
changed. An in-process file watcher then keeps the graph in step with
your edits (~1s, no restart).

**Verify:** in an Agent chat, "call get_spec_coverage" — it should report
a file count, most of them `missing`.

## 3. Install the generation prompt — `.github/prompts/`

```sh
mkdir -p /path/to/your/repo/.github/prompts
cp setup/codeowl-generate.prompt.md /path/to/your/repo/.github/prompts/
```

Then in Copilot Chat type `/codeowl-generate` and append the target — the
same targets the Claude Code command takes:

```
/codeowl-generate --all --budget=5
/codeowl-generate --all --stale --budget=30
/codeowl-generate src/util.rs
/codeowl-generate system
```

Its frontmatter declares `tools: ['codeowl/*']` (every tool from the
`codeowl` MCP server), which also puts the prompt in **agent mode** — so
you don't have to switch modes first. Prompt files load automatically for
the Local agent; there's no enable-setting to flip in current VS Code.
(They are *not* loaded by Agent Host / cloud-agent sessions — a local
VS Code + Copilot Chat is the supported setup here.)

`setup/codeowl-generate.prompt.md` is a port of
`setup/codeowl-generate.md` — the loop body is identical; only the
frontmatter and the argument token (`$ARGUMENTS` → `${input:target}`)
differ. **When one changes upstream, the other must too.**

## 4. The "use CodeOwl first" nudge — `.github/copilot-instructions.md`

Copilot auto-applies this to every chat request in the repo, the way
Claude Code loads `CLAUDE.md`. Create it (or append to it):

```markdown
This repo has a CodeOwl MCP server. Before exploring the codebase, check
`get_spec` / `get_spec_coverage` for an existing spec, and use
`get_callers` / `get_callees` for impact questions. The prose specs in
`docs/specs/` are readable directly, too.
```

## 5. Commit, same as Claude Code

- **Commit `docs/specs/`** — the frontmatter hashes are the staleness
  signal and only work if the specs are versioned with the code.
- **Gitignore `.codeowl/`** (the local graph/index cache — add
  `/.codeowl`).
- `.vscode/mcp.json` has a machine-specific binary path — either
  gitignore it, or commit it with `"command": "codeowl"` (assuming the
  binary is on `PATH`) so it's portable.
- **Regenerate as its own commit**, not folded into a feature PR.

---

## Known rough edges with Copilot

CodeOwl was built and tuned against Claude Code. The read tools
(`get_symbol`, `get_callers`, `get_callees`, `get_spec`,
`get_spec_coverage`, `search_code`) work the same anywhere. The
**generation loop** has two Copilot-specific frictions:

1. **Large-file tool output.** `get_next_spec_task` returns a symbol's
   full source. For a large file (a multi-thousand-line utility class) the
   payload can exceed VS Code's tool-result size limit and get truncated —
   Claude Code spills it to a file instead. Until CodeOwl trims that
   payload (a planned change), generation of the biggest files may be
   unreliable in Copilot; everything else, and all the read tools, are
   fine.

2. **The prompt is Claude-tuned.** Copilot's agent may not follow the
   multi-step loop as faithfully — reading *every* `core_sources` file,
   the budget arithmetic, the reconciliation rules. Run
   `/codeowl-generate --all --budget=5` in small batches and review each
   before committing.

## Reading specs without any of this

`docs/specs/*.md` are plain committed Markdown. A teammate who only wants
to *read* the specs needs nothing installed — open the files, or
`#file:` / `@workspace` them in Copilot Chat. The MCP server adds the
live graph queries and the generation loop on top.
