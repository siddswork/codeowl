# examples/

Developer and measurement tools that link against the `codeowl` crate and read the graph it builds. They are not usage examples, and nothing here ships in the release binary. `cargo test` and `utility/check.sh` compile them, so one that stops building after an API change fails the gate.

List them with `cargo run --example`. Run one with `cargo run --example <name> -- <args>`; add `--release` for big repos.

| Example | What it prints | Arguments |
|---|---|---|
| `graph_stats` | File-level graph statistics: counts by kind, edge counts, fan-in/fan-out and hubs, connected components, path length, import cycles | One or more repo paths; none means CodeOwl itself |
| `measure_signature_fold` | The cost and correctness tables used to decide whether a kind of member belongs in `interface_hash` | None: the four pilot-repo paths are hardcoded in `main`, edit them if the repos move. `SIGNAL` picks what is measured |

Opening a repo builds its `.codeowl/` cache there (gitignored).
