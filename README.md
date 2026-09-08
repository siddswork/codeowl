<pre>
   ,___,
   (o,o)
   ("v")
  ---"---
  CodeOwl
  -------


 #####                          #######
#     #   ####   #####   ###### #     #  #    #  #
#        #    #  #    #  #      #     #  #    #  #
#        #    #  #    #  #####  #     #  #    #  #
#        #    #  #    #  #      #     #  # ## #  #
#     #  #    #  #    #  #      #     #  ##  ##  #
 #####    ####   #####   ###### #######  #    #  ######

</pre>

**A structural graph of your codebase with LLM-authored prose specs layered
on top — kept honest by content hashing, served over MCP.**

---

Every time a coding agent touches a brownfield codebase, it rebuilds its
mental model from scratch — grep, read files, guess how things connect —
and hallucinates the edges it can't see (a `fetch()` to an API route, a
`.from("table")` query). Slow, token-expensive, unreliable.

CodeOwl extracts a structural graph once — symbols, containment, resolved
imports, framework "flow" edges — and serves prose specs over that graph
via MCP: one per symbol, file, directory, "feature" (a cross-cutting flow
the import graph alone can't see), and the whole system. The specs are
LLM-written but hash-invalidated against the graph, so they go stale the
instant the code moves. Dual audience: an agent skips exploration; a human
(BA, new dev) gets documentation that doesn't rot.

Two rules shape everything: CodeOwl **never calls an LLM** — it assembles
context, the calling agent writes the prose, CodeOwl persists and
invalidates — and it **never hashes prose** for invalidation, only the
deterministic graph.

## Why it holds up

- **Specs don't rot.** Every spec's staleness key is computed from the
  graph — a symbol's signature hash, its resolved dependencies' interface
  hashes — never from the prose. Change the code and the specs it touches
  flip to `stale` immediately, naming exactly what moved.
- **No LLM in the loop, ever.** CodeOwl holds no API keys and makes no
  model calls. It's a deterministic index; the calling agent writes spec
  text through `get_next_spec_task` → `submit_spec`. Nothing to hallucinate
  in the parts CodeOwl owns.
- **Sees what grep can't.** Framework conventions become real edges — a
  `fetch("/api/x")` resolves to its route file, a `.from("payments")`
  resolves to the `CREATE TABLE`, a `<Form/>` resolves to its component.
  `get_callers` on a database table lists the app code that queries it.
- **One artifact, two readers.** A feature spec is a numbered,
  BA-followable narrative of a capability end to end; the same document
  gives an agent the exact files, dependencies, and tables it needs
  without re-exploring.
- **Committed to git.** Specs live in `docs/specs/` as plain Markdown —
  reviewable in a PR, browsable on GitHub, versioned with the code.

## What it supports today

Written in **Rust**; ships as a single self-contained binary that runs as
an MCP server (`codeowl serve <repo>`). CodeOwl picks one *stack* per repo
automatically.

| Stack | What it extracts |
|---|---|
| **TypeScript / TSX** | functions, classes, methods, consts; `import`/re-export resolution via `oxc_resolver`; Next.js App Router features (`app/**/page.tsx`, orphan API routes); `fetch("/api/…")` → route, Supabase `.from("table")` → schema, React `<Component/>` → file flow edges |
| **SQL schema** | `CREATE TABLE` → table nodes with column lists, resolved against the app code that queries them |
| **Rust** | `fn` / `struct` / `enum` / `trait` / `impl` / `mod` / `const` / `macro_rules!`; `use crate::…` / `super::` / `pub use` module-tree resolution; `///` + `//!` docs; `#[derive(…)]` / attribute markers. No feature layer — a library or CLI has no routes to enumerate. |

Java (a plain library, then Quarkus microservices with heterogeneous
entry points and cross-service edges) is next — see `ROADMAP.md`'s
Phase 2.

## Docs

- **What & why:** [`REQUIREMENTS.md`](REQUIREMENTS.md)
- **How it's built:** [`ARCHITECTURE.md`](ARCHITECTURE.md)
- **Build sequence:** [`ROADMAP.md`](ROADMAP.md)
- **Wire it into your own repo:** [`setup/README.md`](setup/README.md)
- **Use it day to day:** [`setup/USAGE.md`](setup/USAGE.md)

## License

MIT — see [`LICENSE`](LICENSE).
