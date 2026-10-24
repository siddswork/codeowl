---
type: file
title: "src/spec/smells.rs"
description: "Specification for `src/spec/smells.rs`."
generated: { by: CodeOwl }
resource: "../../../../src/spec/smells.rs"
source_paths: [src/spec/smells.rs]
file: { source_hash: , deps_hash: , spec_hash:  }
symbols:
  src/spec/smells.rs::prose_smells: { source_hash: 1b2d28ae52695f99a7f5f2c73756c63488932350e97a51e65780fe9b6a82284a, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 26b86568c835c132867da8e456451e621fa21632e6c8887df187f7d56eb2d6ee }
  src/spec/smells.rs::file_spec_smells: { source_hash: e2051fcf2c4327cf2233c2fc5a222569a4a7d686bc66ae68dd4fa84994f0bfd2, deps_hash: 1905c59a809cb146e2d62da82589212debb7a142f8d907564aa5b0da29373eae, spec_hash: 2d0c62f86cc75e7b8607878981b10c4b2e1a7efe909023c625d99a6f072145b5 }
  src/spec/smells.rs::body_smells: { source_hash: c6428938dfe924cd3ea028067177273c93bc2e26ba1b8b9c31a2ae1d7eda1efa, deps_hash: c378d364baed498bdbd2549c482c4a36fa77a0c5383c0ae74334ff13ee349263, spec_hash: c1f157cf9df1e48cd3e6d2f5e8471df629373eb7dfb00eb07c4d91e8d2506a9f }
  src/spec/smells.rs::reject_if_smelly: { source_hash: 10553f50cddbaa0d779991367e1c16afbb2df35b39eb320f0ca6d725023c2509, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a2487f1ddcaa91ae9cd315a32ef5c900c4b52c489346e54592410ed0aa5a192e }
dep_targets:
  src/spec/smells.rs::file_spec_smells -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/smells.rs::file_spec_smells -> src/symbol.rs::SymbolId: 6733b5c62e75
  src/spec/smells.rs::body_smells -> src/spec/format.rs::extract_section: 2f6754bf9a83
---
# src/spec/smells.rs
> Generated using [CodeOwl](https://github.com/siddswork/codeowl)
## Summary


## `prose_smells`
`pub fn prose_smells(text: &str) -> Vec<String>`
### Summary
A deterministic, non-LLM check for prose that looks like a stub or an evasion rather than genuine content — independent of hash-based staleness entirely, since a spec can have perfectly matching hashes and still contain worthless prose. This is what catches the two failure modes called out by name in `CLAUDE.md` and the generation prompt: a phrase that defers the reader elsewhere instead of actually explaining the behavior, and prose so short it can only be a placeholder — both found concretely in real pre-quality-check specs during an audit (things like a one-clause non-answer or a two-word deflection).
### Behavior
Checks the (lowercased) text against a small, explicitly named denylist of deflecting phrases that punt the reader back to reading the code themselves instead of actually describing what it does, flagging one smell if any of them appear anywhere in the text. Separately, flags a second smell if the text has fewer than four whitespace-separated words — too short to be a real explanation of anything. Deliberately not exhaustive or clever: a small fixed list plus a word-count floor, both easy to reason about and cheap to extend if a new failure mode is ever found in practice, rather than an attempt at a general-purpose quality classifier.
### Depends on
- (none)

## `file_spec_smells`
`pub fn file_spec_smells(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    spec: &FileSpec,
) -> Vec<String>`
### Summary
Scans a whole file spec for signs of poor quality and returns a list of problem labels, empty if it looks fine. The coverage report uses this to flag a spec that is up to date by hash but whose prose should not be trusted.
### Behavior
Collects problems from three places, then sorts and de-duplicates them. The file summary is checked with `prose_smells` only once it has actually been written; an empty `spec_hash` means the file entry was never generated, and a blank summary in that state is a mid-generation gap rather than a cop-out. Each symbol's summary and behavior are checked the same way. Finally, when the spec has at least two symbols, it computes each symbol's resolved in-repo dependency lines (leaving out the `externals:` line, because many symbols sharing a standard-library dependency is normal) and, if every symbol has the same non-empty list, adds the label `identical_dependencies_across_symbols`. That pattern is the signature of an older bug that attributed all of a file's imports to every symbol, so it flags specs written before the fix that were never regenerated. If a symbol id cannot be resolved the lists are not all comparable and the identical-dependencies check is skipped. It never fails.
### Depends on
- [`src/spec/format.rs::FileSpec`](format.rs.md) — super::format
- [`src/spec/format.rs::dependency_lines`](format.rs.md) — super::format
- [`src/graph.rs::Graph`](../graph.rs.md) — crate::graph
- [`src/symbol.rs::SymbolId`](../symbol.rs.md) — crate::graph
- externals: anyhow, std

## `body_smells`
`pub fn body_smells(body: &str) -> Vec<String>`
### Summary
The same coarse quality signal as `file_spec_smells`, but for the single-blob body of a feature, rollup, or system spec, checked against just the LLM-written `## Summary` portion of it.
### Behavior
Attempts to extract a `## Summary` section out of the body; if none is found (as is already the case for a rollup or system spec's `body`, which stores only the summary text to begin with, unlike a feature spec's `body`, which is the whole document including its title), the whole `body` text is used as a fallback. Either way, the resulting text is checked via `prose_smells`.
### Depends on
- [`src/spec/format.rs::extract_section`](format.rs.md) — super::format

## `reject_if_smelly`
`pub(super) fn reject_if_smelly(what: &str, smells: Vec<String>) -> Result<()>`
### Summary
A small guard that turns a non-empty list of quality-check failures into an error, refusing to let low-quality spec prose be persisted. Storing prose that fails the deterministic quality check isn't harmless: in the "source unchanged, not human-edited" case, a quality failure is exactly what makes `next_task` re-offer the same document for regeneration — so if a generate loop kept resubmitting equally weak prose, it would never terminate. Erroring at submit time turns that silent non-termination risk into a clear message naming exactly what to fix.
### Behavior
If `smells` is empty, returns `Ok(())` — nothing to reject. Otherwise, fails with an error naming `what` (a description of the content being checked) and the comma-joined list of specific smells found, along with a note that `get_next_spec_task` will keep re-offering this spec until its prose passes the check — telling the caller both what's wrong and what happens if they don't fix it.
### Depends on
- externals: anyhow
