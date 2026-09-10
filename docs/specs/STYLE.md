# Spec style for this repo

`/codeowl-generate` reads this before writing any spec here. It sets
**audience and tone** — not structure. The headings, the "base it only on
`source`" rule, and everything else in the command still apply.

## Who the reader is

A **regular application developer** who has never taken a compilers
course. CodeOwl's own code is rooted in parser / AST / graph-theory
territory, and that vocabulary is a wall for most of its audience. Write
so that reader can follow along.

## Rules

1. **The `## Summary` / `### Summary` must be jargon-free.** Someone who
   doesn't know what an AST or an arena is should still understand what
   this file or function is *for*. Lead with the effect ("figures out
   which file an `import` points at"), then the mechanism if needed.

2. **Gloss a term the first time it appears, inline, in parentheses.**
   "the arena (the one flat list that owns every graph node)",
   "resolution (matching an `import` to the actual symbol it names)". A
   reader shouldn't have to leave the page to parse the first sentence.

3. **For a term you genuinely can't avoid repeating** — `AST`,
   `tree-sitter`, `Merkle fold`, `SymbolId`, `interface_hash`,
   `containment edge` vs `reference edge`, `fan-in` — gloss it once, then
   add "(see `GLOSSARY.md`)" and use it freely after that.

4. **A concrete example beats an abstract sentence.** Show the input and
   the output: `import { Graph } from './graph'` → `src/graph.rs::Graph`.
   `app/submit/page.tsx` → slug `submit`.

5. **`### Behavior` is the developer-facing half — it may be more
   technical.** Control flow, edge cases, the `Result` variants, the
   invariants. But still spell out an acronym on first use even here.

6. **Don't describe CodeOwl's *domain* (static analysis) as if the reader
   already lives in it.** "walks the parse tree" needs "(the structure
   tree-sitter produces from the source text)" the first time; "top-level
   symbol" needs "(a declaration not nested inside a class or function)".

7. **Keep it tight.** Glossing every term makes prose longer — compensate
   by cutting hedge words and restating nothing the signature already
   says.

## Quick test

Read your `## Summary` out loud imagining a frontend developer who's
never written a parser. If a sentence would make them stop and re-read,
rewrite it or add the parenthetical.
