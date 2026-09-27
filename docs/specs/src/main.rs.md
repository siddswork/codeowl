---
kind: file
source_paths: [src/main.rs]
file: { source_hash: 82633b3bee38e13b522582fc5117d10c75c30c947b43b36ee1566c1feeff770f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 00ddf409275920e3c873d382f2ef544c91c89299e60da0e3e45e4e9d666807f2 }
symbols:
  src/main.rs::Cli: { source_hash: a9e859449295587528a9bfb37cc3574f636cc6f0ce47e8cd9bd480dd433912cd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: f06d4285746fc86abe4f632825ea0205ef6a74f624325cc8da17378f80b0e1dd }
  src/main.rs::Command: { source_hash: 20f721df0a580d5fda2813ab531db5d29ca8e832e4d51b75acbc819f4e536a73, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c295340364202d1a8162d7b5baf67cb9bae8c48fb802be9a886d8623ee4b24dd }
  src/main.rs::main: { source_hash: 5662d9c849a463d00eb827feb11c4a2adcf9427b1f72529c2e32897859387475, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 25ac4862bea2bd329a0794ea0358dbd712a5168d2604eb0f308eb0fd47accc5f }
  src/main.rs::canonical_root: { source_hash: 118b754d0b23da9200b1c11dbf9ad945af8276777f91c432fb9bbb96a15cc7f3, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 43ac277e71253b08025075455066b22405845df75285e35540ba43e195721321 }
---
# src/main.rs
## Summary
This is the command-line entry point for the whole program: it defines the two subcommands a user can run (`extract`, a one-shot dump of everything found in a repo as JSON, and `serve`, which starts the long-running MCP server that editors and agents talk to), parses whichever one was typed, and dispatches to it. It also owns the small bit of path handling every run needs first — resolving whatever path the user gave into an absolute, symlink-free location — since getting that wrong silently breaks import resolution later on rather than raising an error. `serve`'s three numeric flags exist purely to keep individual responses small enough for a given MCP client to handle without spilling to a temp file, and default to sensible values most users never need to touch.

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
The program's entry point — it parses whatever command the user typed and runs the matching branch above.
### Behavior
Parses the command line into a `Cli` (via `clap`'s `parse()`), then matches on which `Command` variant came back:

- **`Extract`**: resolves `path` to an absolute, symlink-free location (`canonical_root`), builds a fresh `RepoIndex` from scratch and runs its `rebuild()` step, then converts every symbol in the resulting graph into a `SymbolView` (a flattened, serializable view of one symbol plus its resolved edges) and prints the whole list as pretty-printed JSON. This path always re-extracts from nothing — there's no reuse of a prior run's cache.

- **`Serve`**: also canonicalizes `path`, but opens the index instead of forcing a rebuild — `RepoIndex::open` reuses a saved `.codeowl` cache if one exists and only re-parses files that changed since (the `caught` return value lists what got "caught up" this way). It prints how many of the graph's named imports resolved to an actual symbol (`resolved N/M`) and, if any files needed catching up, how many. It then builds a `CodeOwlServer` around the graph, applying the three byte limits from the `Serve` flags, spawns a background file watcher (`codeowl::watch::spawn`) that keeps re-indexing edited files for as long as the process runs — the watcher's handle is bound to `_watcher` specifically so it stays alive until `main` returns, since dropping it stops the watcher — and starts serving the MCP protocol over stdio. `main` then blocks on `running.waiting()` until the server exits, which is what keeps the process alive for the whole session instead of returning immediately after startup.

Both branches propagate errors via `?` and `anyhow`'s `.context(...)`, so a failure at any step (a bad path, a parse error, the watcher failing to start) surfaces with a description of which step failed rather than a bare error.
### Depends on
- externals: anyhow, codeowl

## `canonical_root`
`fn canonical_root(path: &Path) -> Result<PathBuf>`
### Summary
Turns whatever path the user typed (say, `.` or `../my-repo`) into an absolute, symlink-free path before anything else uses it.
### Behavior
Calls `Path::canonicalize`, which resolves the path relative to the current directory, follows any symlinks, and returns an absolute `PathBuf` — or an error, wrapped with `.context(...)` naming the path that failed, if the path doesn't exist or can't be resolved.

This has to happen before extraction starts because import resolution later (in `resolve.rs`) strips this same root off of another library's (`oxc_resolver`'s) resolved import paths — which are always absolute — to recover a path relative to the repo. If the root passed in were left relative (e.g. `.`), that strip would silently fail for every import (a relative path is never a prefix of an absolute one), so every single import would resolve to nothing with no error raised anywhere. Canonicalizing up front is what keeps the whole graph's paths on one consistent (absolute) scheme regardless of how the user invoked the command.
### Depends on
- externals: anyhow, std
