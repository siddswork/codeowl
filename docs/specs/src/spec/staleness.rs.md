---
kind: file
source_paths: [src/spec/staleness.rs]
file: { source_hash: , deps_hash: , spec_hash:  }
symbols:
  src/spec/staleness.rs::interface_or_source_hash: { source_hash: 571280a0b439c44f0f00513509b285dceac15ef3cfdbdf188beaa1b2836754ae, deps_hash: 1905c59a809cb146e2d62da82589212debb7a142f8d907564aa5b0da29373eae, spec_hash: 6a18764e9cce21f23f5f5c3f7614fe36107392d3b1381253114eca83e533d416 }
  src/spec/staleness.rs::dependency_hash: { source_hash: c5ccd3e6b6e8af68b991f12fe9884516dd1853247273ff922b539c137b17168a, deps_hash: 43f1c124e9646bc90f5c80d49d1555450df5957531f3ebcb7bac0dac0dab7275, spec_hash: 9f65ff68a21c6593208ce95aed889de921c755c87c29a8a98ee7e285950f4d9b }
  src/spec/staleness.rs::file_dependency_hash: { source_hash: 6eaff6c968f0ed0595601bdacaa3c97b7ba3f8431113afda2ea624c33158b14c, deps_hash: fda77667fa29aea1764dc67165133760f12c815928b2368ab995f813913d1cea, spec_hash: 6cab31f51cf7c879f396858215426a29864d9d8a072063ed3046c4f1de5c4d28 }
  src/spec/staleness.rs::diff_hash_lists: { source_hash: ad9b61a855f22bab85c7c2a88d9f2209948d8a5b13b71b6fd5ed157e01b21751, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: bc4a2eab77077f4c0322ddb613ec6fd792637ae779f3bb36c92434bc8c9b73f7 }
  src/spec/staleness.rs::symbol_changes: { source_hash: 7d8003b8528f55319c103df648fafb49171b358847f12c164dc1f4c694e3aa9a, deps_hash: 7139611c2449214bfc7bb552592931223583fc488491fce545e8c11a9b6ec247, spec_hash: a3285a4415f3cefefcdb03321a68ee80c89ba819da35aa7c784fc8139db4f89e }
  src/spec/staleness.rs::file_changes: { source_hash: f2692ac71a88fca03fef928545116e2f7ef05825cd3875e09924071aa5d193a2, deps_hash: 1905c59a809cb146e2d62da82589212debb7a142f8d907564aa5b0da29373eae, spec_hash: fe9368c161858328dc19ee9cc2f839933657ed107a10c5559f403f07e207dcdc }
dep_targets:
  src/spec/staleness.rs::interface_or_source_hash -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/staleness.rs::interface_or_source_hash -> src/symbol.rs::SymbolId: 6733b5c62e75
  src/spec/staleness.rs::symbol_changes -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/staleness.rs::symbol_changes -> src/symbol.rs::Symbol: 2bab71782e46
  src/spec/staleness.rs::symbol_changes -> src/symbol.rs::SymbolId: 6733b5c62e75
  src/spec/staleness.rs::file_changes -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/staleness.rs::file_changes -> src/symbol.rs::SymbolId: 6733b5c62e75
---
# src/spec/staleness.rs
## Summary


## `interface_or_source_hash`
`fn interface_or_source_hash(graph: &Graph, id: SymbolId) -> String`
### Summary
Gives the fingerprint of something a spec depends on, used to detect when that dependency changed in a way that matters to its users. Prefers the fingerprint of the dependency's public shape, so internal edits don't mark dependents out of date.
### Behavior
Looks the id up as a symbol. If found, it returns the symbol's `interface_hash` (its public shape, see `GLOSSARY.md`), and falls back to the symbol's whole `source_hash` when it has none, which makes dependents go stale on any edit to that symbol. If the id is not a symbol but a file, it returns the file's `source_hash`. If neither lookup succeeds it returns an empty string. It never uses the text of a spec: rewording a dependency's prose must not make its consumers stale. The fallback to the whole-source hash is a coarse case that can over-mark dependents stale, and it is why giving more kinds of symbols a real `interface_hash` reduces unnecessary staleness.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph

## `dependency_hash`
`fn dependency_hash(graph: &Graph, root: &Path, file_id: SymbolId, sym: &Symbol) -> String`
### Summary
Computes the "have my dependencies changed shape?" fingerprint for one function or type. It is saved when a spec is written and recomputed on every freshness check, so a difference means something this symbol uses changed.
### Behavior
Reads the symbol's own source text from disk and selects the file's imports whose names appear in it as whole words, the same narrowing the "Depends on" list uses, skipping any import that targets the symbol itself. Only imports that resolved to a target inside the repo are kept, because CodeOwl cannot observe the shape of an outside package. The targets go through `hash_dependency_targets`. It returns an empty string, not an error, if the id is not a file node or the source cannot be read, which means such a symbol compares as unchanged against another empty value. The scan uses the full source text, not a shortened outline.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph
- `src/symbol.rs::Symbol` — crate::symbol
- externals: std

## `file_dependency_hash`
`pub(super) fn file_dependency_hash(graph: &Graph, file_id: SymbolId) -> String`
### Summary
Computes the "have my dependencies changed shape?" fingerprint for a whole file. It is the file-level counterpart of the per-symbol version and is used to judge whether the file's own summary is still current.
### Behavior
Collects every import in the file that resolved to a target inside the repo and hashes them with `hash_dependency_targets`. Unlike `dependency_hash`, it does not narrow by what each symbol mentions, because a file summary is about the file as a whole, so any dependency changing shape can matter. Unresolved imports are ignored since their shape cannot be observed. If the id is not a file node it returns an empty string rather than failing.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph

## `diff_hash_lists`
`pub fn diff_hash_lists(current: &[(String, String)], stored: &[(String, String)]) -> Vec<String>`
### Summary
Compares a document's currently-computed set of dependencies against what was stored when it was last generated, producing a human-readable list of exactly what changed — shared by both feature specs (their `participants` map) and directory rollups (their `files` map), since the staleness rule is identical for both: any entry whose hash moved, any new entry, or any entry that disappeared counts as a real change.
### Behavior
For each `(id, hash)` in the current set, checks whether it exists in the stored set with a matching hash (no change, nothing recorded), a matching id but a different hash (`"changed:<id>"`), or no matching id at all (`"added:<id>"` — a new dependency appeared since generation). Then, for each stored entry with no corresponding current entry, records `"removed:<id>"`. The final list is sorted for deterministic output. An empty result means the document is fully current; each string in a non-empty result deterministically names one specific thing that moved — computed purely off the graph, with no LLM involved in deciding *that* something changed, only in deciding what to write once it has.
### Depends on
- (none)

## `symbol_changes`
`pub fn symbol_changes(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    sym: &Symbol,
    stored: &HashPair,
) -> Vec<String>`
### Summary
Reports what has changed for one function or type since its spec was written. An empty result means the spec is still current, and otherwise it names what moved: the code itself, the things it depends on, or both.
### Behavior
Builds two lists of (name, hash) pairs with the fixed keys `source` and `dependencies`. The current list uses the symbol's present `source_hash` and a freshly computed `dependency_hash`. The stored list uses the hashes saved in the `HashPair`. Both go to `diff_hash_lists`, which returns the keys whose values differ. Using the same two keys each time lets `get_spec` tell a caller which of the two moved. The stored `spec_hash` is not involved, since editing prose never makes a spec stale. If the source cannot be read, `dependency_hash` returns an empty string, which will not match a stored non-empty value and so shows as a `dependencies` change.
### Depends on
- `src/spec/format.rs::HashPair` — super::format
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph
- `src/symbol.rs::Symbol` — crate::symbol
- externals: std

## `file_changes`
`pub fn file_changes(graph: &Graph, file_id: SymbolId, stored: &HashPair) -> Vec<String>`
### Summary
Reports what has changed for a whole file since its spec was written. An empty result means the file's spec is current.
### Behavior
Works like `symbol_changes` but at file level. The current side is the file's present `source_hash` (an empty string if the id is not a file) and `file_dependency_hash`. The stored side comes from the `HashPair`. `diff_hash_lists` returns whichever of the keys `source` and `dependencies` differ. It needs no disk access, because the file's source hash is already in the graph, so it takes no `root` argument. A file that vanished from the graph compares as having an empty hash and therefore shows as changed.
### Depends on
- `src/spec/format.rs::HashPair` — super::format
- `src/graph.rs::Graph` — crate::graph
- `src/symbol.rs::SymbolId` — crate::graph
