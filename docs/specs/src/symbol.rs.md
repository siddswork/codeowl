---
kind: file
source_paths: [src/symbol.rs]
file: { source_hash: de25bd0e2225ae1bd8aa539ad35d10320f2cfff71c214ed2acd067ea53c13356, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 14d0ad65af303877f03964f52020458875c7a8edb1bc331e1431e02b810d1585 }
symbols:
  src/symbol.rs::SymbolId: { source_hash: cab1efc8ff00e70ad0c6359952a0420cae2340e091c301f98b90ac8e001b5c9f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 9b39ef07757da2546307644d89bbbf944473e6d63e4855a63faeb48e16412626 }
  src/symbol.rs::SymbolKind: { source_hash: 705c2902ac4016a078c0207dc562b429c21c755363ceab9dfc3608825ebc7fab, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3aaa3a5c23a61c4d6c074cf71da5ce649410d1c66e26997bbfd1dc21f4248cdb }
  src/symbol.rs::Symbol: { source_hash: d82ceb579f7fd83d883bf9d1caa0000a477d38a090cd9dba730c676936321cf4, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: e685a1c6502fd8954805fda0869a5ae1df62e5d2d450c8ce9641fa0929efa977 }
  src/symbol.rs::ExtractedSymbol: { source_hash: 96029ac8433be50559b951023304717ee398ff259d7d3efddc3366b8ef694c49, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 20fb63bdbc56052f18a2eddd1379c3edbfe10cd09dcd3b7c9916aee2342ed255 }
---
# src/symbol.rs
## Summary
Defines the graph's node vocabulary. `SymbolId` is the arena-index newtype every containment link uses; `SymbolKind` is the stack-neutral `Container | Callable | Value | Schema` set the granularity rules key on; `Symbol` is one declaration as it lives in a built `Graph`; `ExtractedSymbol` is the same shape a pack's extractor produces per-file before a `Graph` exists, with string containment links `Graph::build` later resolves to `SymbolId`s. Together these are the boundary between "parsed a file" and "built the arena" — every type here is owned data with no tree-sitter lifetimes escaping.

## `SymbolId`
`pub struct SymbolId`
### Summary
A lightweight pointer to one node — a symbol (a function, class, etc.) or a file — inside CodeOwl's graph (the in-memory structure that holds every extracted symbol from the codebase, see `GLOSSARY.md`). It's how one part of the graph refers to another without copying data around.
### Behavior
`SymbolId` wraps a plain `u32` index into the graph's arena (the one flat list that owns every node). It's built via the crate-private `new(index)` and read back via `index()` — both `pub(crate)`, so code outside this crate can only ever pass an existing `SymbolId` around, never mint one from a raw number. The id is meaningful only relative to the specific `Graph` that produced it: nothing pins a given index to the same node across a re-extraction (a file changes, the graph gets rebuilt), so a `SymbolId` from an old graph must never be looked up in a new one. That's a distinct problem from a node's *string* id (e.g. `"src/graph.rs::Graph"`), which stays stable across re-extractions and is what CodeOwl actually persists to disk between runs.
### Depends on
- (none)

## `SymbolKind`
`pub enum SymbolKind`
### Summary
The stack-neutral vocabulary the generic core uses to reason about a declaration: `Container` (has members that get their own specs), `Callable` (a function/method/constructor), `Value` (a data-carrier folded into its file spec), `Schema` (a SQL table). Each pack maps its concrete grammar kinds onto these four and records the real keyword in `ExtractedSymbol::raw`.
### Behavior
A `Copy` enum, serialized `snake_case` (`"container"`, `"callable"`, …) into `codeowl extract` output, the `.codeowl/` cache, and the `get_symbol` MCP response; `JsonSchema` is derived for the MCP tool schema. `spec.rs`'s granularity rule keys off it: a top-level `Container` or `Callable` gets its own spec document, a `Value` or `Schema` does not, and a method — a `Callable` whose parent is a `Container` — is covered inside the container's section rather than separately. Introduced in M14's design-decision-1 reshape, replacing the earlier `Function | Class | Method | Const | Table` set; the grammar detail that lost carries on `raw`.
### Depends on
- (none)

## `Symbol`
`pub struct Symbol`
### Summary
The record for one declaration (a function, class, method, struct, etc.) once it's been placed into the graph (CodeOwl's in-memory structure holding every extracted symbol — see the `SymbolId` spec). It carries everything CodeOwl knows about that declaration: where it lives, its signature, its documentation, and the content hashes that drive staleness detection.
### Behavior
`id` is a deterministic string like `"src/graph.rs::Graph"` (or `"src/graph.rs::Graph.build"` for a method) — the stable handle used everywhere outside the graph's arena (an MCP tool response, a spec file's frontmatter, a human re-running extraction), since a `SymbolId` is only valid for the specific `Graph` that produced it.

`kind` is CodeOwl's own generic classification (roughly: callable, container, or value); `raw` is the pack's own grammar-level label for the same declaration (`"function"` / `"class"` / `"struct"`, `"table"` for a SQL table, and so on) — kept only for display and pack-internal logic, never branched on by the generic core.

`file` and `lines` locate the declaration in its source file (`lines` is a 1-indexed `[start, end]` pair, inclusive). `signature` and `docstring` are pulled straight from the source text.

`is_exported` says whether another file could `import` this declaration at all — always `false` for a method, since a method is only reached through an already-imported class, never imported on its own.

`source_hash` is a content hash over the declaration's entire span (signature and body); it changes on any edit inside it, and rolls up Merkle-style for a container (a class's hash folds in each of its methods' hashes, in order — see `GLOSSARY.md`'s "Merkle fold"). `interface_hash` is a narrower hash over just the exported *shape* — the signature, never the docstring or body — so an implementation-only edit leaves it unchanged; it's `None` when `is_exported` is `false`, since nothing outside the file could depend on its shape yet. Together these two hashes let CodeOwl distinguish "this file changed" from "this file's public contract changed" (see `ARCHITECTURE.md`'s "Caching and invalidation").

`markers` holds any stack-specific decorations verbatim, exactly as the pack's extractor recorded them — a Rust `#[derive(...)]`, a Java `@Entity`, a Python `@app.route`. The generic core never interprets these itself; a `StackPack` reads its own back out later.

`parent` and `children` place this symbol in the containment tree using `SymbolId` (not the string `id`) — every symbol has a parent now that files are graph nodes too. A top-level declaration's parent is its file; only a file with no directory node above it has no parent at all.
### Depends on
- (none)

## `ExtractedSymbol`
`pub struct ExtractedSymbol`
### Summary
The same declaration record as `Symbol`, but in the form a pack's extractor produces straight off the syntax tree (the structure `tree-sitter` builds from the source text), before a `Graph` exists to place it in the arena (the one flat list that owns every graph node).
### Behavior
`ExtractedSymbol` carries the same fields as `Symbol` (id, kind, raw, file, lines, signature, docstring, is_exported, source_hash, interface_hash, markers), except containment is expressed with plain strings rather than `SymbolId`: `parent: Option<String>` and `children: Vec<String>` instead of `SymbolId`-typed fields. This is deliberate — extraction runs one file at a time with no cross-file knowledge (see `ARCHITECTURE.md`'s "Extractors"), so there's no arena yet to hand out numeric positions from.

`parent: None` here means "top-level in this file, not contained by another *symbol*" — it does **not** mean "has no container at all." `Graph::build` is the step that resolves every string id into a real `SymbolId` and gives top-level symbols their containing file as a parent, once every file's symbols (and the files themselves) are known together and an arena can exist.
### Depends on
- (none)
