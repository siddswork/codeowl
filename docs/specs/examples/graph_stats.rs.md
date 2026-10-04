---
kind: file
source_paths: [examples/graph_stats.rs]
file: { source_hash: 3a9635e32089bc73606089db63a79383531541691cf5715d304149a35f8fe7ee, deps_hash: bfc3e45e10445c8906eaa6b40d2ae4352846ce28ee32def3316a690807103aac, spec_hash: 9db3f004080f625d4409ca2c4c9ffd1b4f3ff49b966c36d8fdd4c978b7a91186 }
symbols:
  examples/graph_stats.rs::pct: { source_hash: abc2d4b7a291d7362788341fe88657752b0e776613b93c5d63c27dda526eed98, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3f362a8c6048701fd3b8c5929ba106744a380e63e5cd83aca5f3209e0b99349c }
  examples/graph_stats.rs::top: { source_hash: c6a78fcdc1e2314e1fb51b1b514607a48ef54c5302dc1276265d836fb4c970b2, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 22d0b265dfb26f323c63b3a8a673800bc8ecb5a7584c4a7190131e4f94ac51f5 }
  examples/graph_stats.rs::sccs: { source_hash: 4f2218e0174dda0d74e51a0b14bc7c32c3e206e660563dab08c1662d57f0bc67, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 0623f37c5368cb659abef181a5128d2f28208e690fa4719b53373ea07d3a0f98 }
  examples/graph_stats.rs::report: { source_hash: ff03030576d93825df9cb123a39a3ae48a746cbf13b6fb7a0a3ff1ecf7c36735, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: da92426d45f28494866e6b60e4c458b9542476d318418e2c4b25b9d85fb5b0fb }
  examples/graph_stats.rs::nodes_and_containment: { source_hash: cfca28a8bb4846d924a7a14eb47c6c136f8eb93b271a7e51f5fe6dd99f0e2194, deps_hash: c26f9d6a3e6974e2d667c28f724f82f43d77dfdb43054409f1e8e28bc6038944, spec_hash: bb0a15e923c686309f3d186976b19bfe41fd623066e61fccb2ef60affdb4ce61 }
  examples/graph_stats.rs::file_graph: { source_hash: 651bdaf37e8d98a1622cf9a6f352eb028c78f34e3dbb1e69cddc696875edd081, deps_hash: 308f45a79583deb699bf85014e3802b6dfdd23e12396d5348fbbaa03681c31db, spec_hash: 11b38d041454a4035fda18808d44d77f697b594f9587572bf4a258f8f078e387 }
  examples/graph_stats.rs::main: { source_hash: ac916d1504e59bb80736f0589eb8d258a67f807004cffe60ab06112ad4729865, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 0dea4e08bfbdf354b1eb35cb484282a625a758f8dbdbecdc253959e493c9eb33 }
dep_targets:
  file -> src/graph.rs::FlowTarget: fec70e24a6f3
  file -> src/graph.rs::Graph: 13dbdfa88b0e
  file -> src/index.rs::RepoIndex: 481419547059
  file -> src/symbol.rs::SymbolKind: 264efecb34de
  examples/graph_stats.rs::report -> src/index.rs::RepoIndex: 481419547059
  examples/graph_stats.rs::nodes_and_containment -> src/graph.rs::Graph: 13dbdfa88b0e
  examples/graph_stats.rs::nodes_and_containment -> src/symbol.rs::SymbolKind: 264efecb34de
  examples/graph_stats.rs::file_graph -> src/graph.rs::FlowTarget: fec70e24a6f3
  examples/graph_stats.rs::file_graph -> src/graph.rs::Graph: 13dbdfa88b0e
---
# examples/graph_stats.rs
## Summary
A runnable tool that prints statistics about the graph CodeOwl builds for one or more repos, run with `cargo run --example graph_stats -- <repo> [<repo> ...]`, or with no arguments to inspect CodeOwl itself. For each repo it reports how many files and declarations there are and how they nest, then looks at the file-level import graph, where every resolved import becomes one directed link from the importing file to the file that owns the target, de-duplicated and with links from a file to itself dropped. That is the graph the feature layer and the staleness cascade actually walk. The numbers it prints cover resolved versus unresolved imports, how connected the files are, which files are imported the most or import the most, how many files are isolated, how many separate groups of files there are, how far apart files sit, and where import cycles are. Its main use is checking whether a change to extraction or import resolution made the graph more complete, for example by watching isolated files and unresolved imports fall. It only reads and prints: opening a repo may refresh that repo's local cache, but nothing else is written. A repo that fails to open prints a notice and the run continues.

## `pct`
`fn pct(sorted: &[usize], p: f64) -> usize`
### Summary
Picks a percentile (for example the median or the 90th percentile) out of a list of numbers that is already sorted from smallest to largest.
### Behavior
Returns 0 for an empty list. Otherwise it takes the last index (length minus one), multiplies it by `p` (a fraction between 0 and 1), rounds to the nearest whole number, and returns the value at that position. So `p = 0.0` gives the smallest value, `1.0` gives the largest, and `0.5` gives the middle one, rounding to the nearer index when it falls between two. This is the "nearest rank" style and does not blend neighbouring values. The input must already be sorted, which is not checked, and a `p` outside 0 to 1 would index out of bounds and panic.
### Depends on
- (none)

## `top`
`fn top(label: &str, deg: &HashMap<usize, usize>, names: &[String], n: usize)`
### Summary
Prints a short ranked list of the files with the highest counts, such as the files imported by the most others, under a heading.
### Behavior
`deg` maps a file's position in the `names` list to its count, and `names` holds the file paths. The function collects the entries, sorts them by count from highest to lowest, with equal counts ordered alphabetically by file name so the output is stable between runs, and prints a line `  top <label>:` followed by the first `n` entries. Each line shows the count right-aligned in four columns, then the file path. If there are fewer than `n` entries it prints them all. It prints to standard output and returns nothing. An index in `deg` outside `names` would panic, and the caller is responsible for keeping them consistent.
### Depends on
- externals: std

## `sccs`
`fn sccs(adj: &[Vec<usize>]) -> Vec<Vec<usize>>`
### Summary
Finds the groups of files that all depend on one another in a loop, directly or through each other (import cycles), by splitting a graph into its strongly connected components. A file with no loop through it forms a group of one.
### Behavior
`adj` is an adjacency list: `adj[v]` lists the nodes `v` points to. The function implements Tarjan's algorithm, written with an explicit work stack instead of recursion so a very deep import chain cannot overflow the call stack.

Each node gets an `index` (the order it was first reached) and a `low` value (the lowest index reachable from it through its descendants and any node still on the open stack). The walk starts from every node not yet visited. For the node on top of the work stack it examines the next neighbour: an unvisited one is pushed to be explored, and one already on the open stack lowers `low`. When a node's neighbours are exhausted it is popped, its parent's `low` is lowered by the child's, and if the node's `low` equals its own `index` it is the root of a component, so nodes are popped from the open stack down to it and collected as one group.

Returns the list of components, each a list of node numbers; every node appears in exactly one. The order of the components, and of nodes inside one, follows the finishing order of the walk and is not sorted. An empty graph gives an empty list. The `unwrap` calls cannot fail given the algorithm's invariants.
### Depends on
- (none)

## `report`
`fn report(label: &str, root: &Path)`
### Summary
Opens one repo with CodeOwl and prints its statistics under a heading: first the node counts and containment, then the file-level import graph.
### Behavior
Calls `RepoIndex::open(root)` to build or load the index. If that fails for any reason it prints `== <label>: failed to open ==` and returns, so one broken repo does not stop a run over several. Otherwise it prints a header with the label and the name of the language pack that handled it, then calls `nodes_and_containment` on the graph. Next it builds the list of file ids in the graph's order, plus a map from each file id to its position in that list, and hands both to `file_graph`, which does the import statistics. The function prints only; it returns nothing and does not modify the repo, though opening an index may create or refresh the repo's `.codeowl/` cache.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index
- externals: std

## `nodes_and_containment`
`fn nodes_and_containment(graph: &Graph)`
### Summary
Prints how big the graph is and how its declarations are nested: counts of files and declarations, a breakdown by kind, how many declarations each file and each container holds, and how many are exported.
### Behavior
Counts the files and, for every symbol, tallies two breakdowns: by the generic kind (container, callable, value, schema) and by the pack's own raw kind (`fn`, `struct`, `class`, and so on). It prints the totals, the kind breakdown, and the eight most common raw kinds, highest first. For nesting, it collects the number of children for every container, and the number of top-level symbols for every file, sorts each list, and uses `pct` to print median, 90th percentile and maximum for both (with the container count shown too). Finally it prints how many symbols are exported out of the total. The output is plain text on standard output and nothing is returned. An empty graph prints zeros without panicking, because `pct` handles an empty list and `max` falls back to 0. Ties in the raw-kind ranking keep the alphabetical order of the underlying sorted map.
### Depends on
- `src/graph.rs::Graph` — codeowl::graph
- `src/symbol.rs::SymbolKind` — codeowl::symbol
- externals: std

## `file_graph`
`fn file_graph(graph: &Graph, names: &[String], ix: &HashMap<&str, usize>)`
### Summary
Prints a statistical picture of how the files in a repo depend on one another: how many imports there are and how many resolved, how connected the files are, which files are most imported, whether there are isolated files, how long the paths between files are, and where import cycles exist. It exists to judge whether the dependency graph looks right, for example when a fix should have added missing links.
### Behavior
Everything is printed to standard output, and nothing is returned.

Imports and flow edges: counts the import statements and how many have no target (external or unresolved). It also groups the pack's flow edges by kind and prints a total and a resolved count per kind, if there are any.

Building the file graph: for each resolved import it takes the importing file and the file that owns the target, and records a directed edge between them, ignoring links from a file to itself and de-duplicating repeated pairs. From those it builds a directed adjacency list, an undirected one, and fan-in and fan-out counts (how many files import a file, and how many it imports). It prints node count, edge count, density (edges over the possible number), and mean degree, then the count of isolated files, which have no edge in either direction. It prints median, 90th percentile and maximum of fan-in and fan-out, and what share of all edges point at the top tenth of files by fan-in (at least one file), then the top five files by each.

Components: a breadth-first search over the undirected graph finds weakly connected groups, and it prints how many there are, how many have more than one file, and the five largest sizes. For the largest group, only when it has more than one file and at most 2500, it runs a breadth-first search from every member to print the average shortest path and the diameter, the longest of those shortest paths. The 2500 limit keeps this all-pairs step affordable.

Cycles: `sccs` finds strongly connected groups, and those with more than one file are import cycles. It prints their count, how many files they involve, the five largest sizes, and the first eight paths, sorted, of the largest group. A blank line ends the report. An empty graph gives zeros without a crash, with the percentage lines skipped when there are no edges.
### Depends on
- `src/graph.rs::FlowTarget` — codeowl::graph
- `src/graph.rs::Graph` — codeowl::graph
- externals: std

## `main`
`fn main()`
### Summary
The starting point of the graph statistics tool. Run with no arguments it reports on CodeOwl's own repo, and given one or more folder paths it reports on each in turn.
### Behavior
Reads the command-line arguments after the program name. With none, it calls `report` once with the label "CodeOwl itself" and the current directory. Otherwise it calls `report` for each argument, using the argument as both the label and the path, in the order given. Each repo is independent, and a repo that fails to open prints a notice from `report` and does not stop the rest. The tool is run with `cargo run --example graph_stats`, and there is no argument validation: a path that does not exist simply produces the failed-to-open notice. It returns nothing and exits normally.
### Depends on
- externals: std
