---
kind: file
source_paths: [src/main.rs]
file: { source_hash: 82633b3bee38e13b522582fc5117d10c75c30c947b43b36ee1566c1feeff770f, deps_hash: ad8f991c443fc9d8b732781de3051224e6b4521e821e5f8b04fb5480ecd1267b, spec_hash: 8f17a509d9063c5f3bff3d4c7f5d994152fd7dea130c291d8bd9d3f78962351a }
symbols:
  src/main.rs::Cli: { source_hash: a9e859449295587528a9bfb37cc3574f636cc6f0ce47e8cd9bd480dd433912cd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: f06d4285746fc86abe4f632825ea0205ef6a74f624325cc8da17378f80b0e1dd }
  src/main.rs::Command: { source_hash: 20f721df0a580d5fda2813ab531db5d29ca8e832e4d51b75acbc819f4e536a73, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c295340364202d1a8162d7b5baf67cb9bae8c48fb802be9a886d8623ee4b24dd }
  src/main.rs::main: { source_hash: 5662d9c849a463d00eb827feb11c4a2adcf9427b1f72529c2e32897859387475, deps_hash: ad8f991c443fc9d8b732781de3051224e6b4521e821e5f8b04fb5480ecd1267b, spec_hash: a4a93ba94f6b641b37b8fbce75aa780da61e40ddb331088fb6c9d26c9e4ea961 }
  src/main.rs::canonical_root: { source_hash: 118b754d0b23da9200b1c11dbf9ad945af8276777f91c432fb9bbb96a15cc7f3, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 43ac277e71253b08025075455066b22405845df75285e35540ba43e195721321 }
---
# src/main.rs
## Summary
This is the command-line entry point of the whole program. It defines the two commands a user can run, `extract`, a one-shot dump of everything found in a repo as JSON, and `serve`, which starts the long-running server that editors and AI agents talk to over standard input and output, parses whichever was typed, and runs it. It also owns the small piece of path handling every run needs first: the path given is turned into an absolute, symlink-free location before anything else uses it. This matters because the import resolver recovers repo-relative paths by stripping that root off absolute paths, and given a relative root such as `.` the stripping silently fails for every import, so every import would resolve to nothing with no error at all. `serve` also takes three size limits, all with sensible defaults, that exist to keep individual responses small enough for a given AI client to handle without it saving them to a temporary file: the size above which a large class is shown as an outline, the hard ceiling on a spec-writing task, and a separate, larger ceiling on a source-reading response. In `serve` mode a short summary of how many imports resolved, and how many files changed since the last run, goes to standard error, since standard output carries only the protocol.

## `Cli`
`struct Cli`
### Summary
The top-level shape of every command this program accepts from the terminal — it's what turns `codeowl <subcommand> ...` into something the rest of the program can act on.
### Behavior
`Cli` has a single field, `command`, marked with `#[command(subcommand)]` so the `clap` command-line-parsing library knows this is a dispatch point rather than a plain flag: whatever the user types after the program name gets parsed into one variant of the `Command` enum. `Cli` itself holds no logic — it exists so `clap` has a single root type to parse the whole command line into, and the actual behavior lives in whichever `Command` variant comes back.
### Depends on
- (none)

## `Command`
`enum Command`
### Summary
The two things this program can be asked to do: a one-shot extraction that prints what it found, or starting the long-running server other tools talk to.
### Behavior
`Extract { path }` walks the repo at `path`, extracts symbols (the functions, types, and other declarations tree-sitter — the parsing library used to read source code — can find), resolves each `import` to the actual symbol it names, and prints the resulting list as JSON to stdout. It's a one-off inspection command, not the normal way this tool is used day to day.

`Serve { path, large_class_bytes, max_spec_task_bytes, max_source_bytes }` runs that same extraction and then keeps the process running, exposing the read/write surface over MCP (Model Context Protocol — how an editor or agent talks to this tool) via stdio. Its three numeric flags all exist to keep individual MCP responses small enough for a client to handle inline instead of spilling to a temp file:
- `large_class_bytes` controls when a big class/struct/interface is reduced to just its member signatures and docstrings (dropping method bodies) inside a `get_next_spec_task` response — tried first because it loses less information than a hard truncation.
- `max_spec_task_bytes` is the hard ceiling applied afterward to any single generation-task response (whether that reduction happened or not) — nothing returned from `get_next_spec_task` can exceed this, regardless of which path produced it.
- `max_source_bytes` is a separate, larger ceiling used only for `get_source` responses, since those exist to hand back real source for verification rather than a prompt-sized excerpt.

All three flags default to constants defined in `codeowl::spec` and only need overriding when a specific MCP client's own inline-context limit is smaller (or larger) than the default assumes.
### Depends on
- externals: std

## `main`
`async fn main() -> Result<()>`
### Summary
The program's entry point. It reads which command the user typed and runs it: `extract` prints everything found in a repo as JSON, and `serve` starts the long-running server that editors and AI agents talk to.
### Behavior
Parses the command line and branches on the command.

`extract <path>`: makes the path absolute and symlink-free, builds a fresh index of the whole repo, builds the graph, turns every symbol into its outside-safe form, and prints the list as pretty JSON to standard output. It does not use or write any saved cache beyond what the build step does.

`serve <path>`: makes the path absolute, then opens the index, which loads the saved copy and re-reads only files that changed since the last run. It prints to standard error how many named imports resolved out of the total, and, if any files changed while nothing was running, how many were re-indexed. It then creates the server with the three size limits taken from the command-line flags: the large-class threshold, the maximum task size, and the maximum source size. It starts the file watcher, which keeps the server's graph in step with edits and which stops when `serve` returns, so the handle is held for the whole session. Finally it serves over standard input and output and waits until the connection closes. Failures starting the watcher or the server are reported with context, and any error is returned as the program's exit error. Standard output carries only protocol traffic in `serve` mode, so all human-readable notices go to standard error.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index
- `src/mcp.rs::CodeOwlServer` — codeowl::mcp
- externals: anyhow

## `canonical_root`
`fn canonical_root(path: &Path) -> Result<PathBuf>`
### Summary
Turns whatever path the user typed (say, `.` or `../my-repo`) into an absolute, symlink-free path before anything else uses it.
### Behavior
Calls `Path::canonicalize`, which resolves the path relative to the current directory, follows any symlinks, and returns an absolute `PathBuf` — or an error, wrapped with `.context(...)` naming the path that failed, if the path doesn't exist or can't be resolved.

This has to happen before extraction starts because import resolution later (in `resolve.rs`) strips this same root off of another library's (`oxc_resolver`'s) resolved import paths — which are always absolute — to recover a path relative to the repo. If the root passed in were left relative (e.g. `.`), that strip would silently fail for every import (a relative path is never a prefix of an absolute one), so every single import would resolve to nothing with no error raised anywhere. Canonicalizing up front is what keeps the whole graph's paths on one consistent (absolute) scheme regardless of how the user invoked the command.
### Depends on
- externals: anyhow, std
