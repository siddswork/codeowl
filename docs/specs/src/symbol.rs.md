---
kind: file
source_paths: [src/symbol.rs]
file: { source_hash: 746dff979be7a41f0b0612a5bb91f0d92453056fc2d26f6135a015f6f483dedf, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 23ac758132e751b7ca3328c51a0697f8ffd9a3c307b15b6d4f1b5bc05bcf653e }
symbols:
  src/symbol.rs::SymbolId: { source_hash: cab1efc8ff00e70ad0c6359952a0420cae2340e091c301f98b90ac8e001b5c9f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 9b39ef07757da2546307644d89bbbf944473e6d63e4855a63faeb48e16412626 }
  src/symbol.rs::SymbolKind: { source_hash: 705c2902ac4016a078c0207dc562b429c21c755363ceab9dfc3608825ebc7fab, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3aaa3a5c23a61c4d6c074cf71da5ce649410d1c66e26997bbfd1dc21f4248cdb }
  src/symbol.rs::Symbol: { source_hash: 88f53fca6a62bd30841a69a0271c293ff3bdbf1d1f6d077e3114d48f33a648e0, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b6049042cacbe4f7b2cf040f5a3b48b7f6f36fbd18e6de3103684971beafe214 }
  src/symbol.rs::ExtractedSymbol: { source_hash: 9fb194a436a02228480ad52aa3687bdc04d4f195e80be377fd6e023ab9e10249, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: ca8004af72a0baea77572b6f4a1f4270abf8613dd55ea2afc5040ca484dc4d3b }
---
# src/symbol.rs
## Summary
Defines the vocabulary the whole graph is made of. `SymbolId` is a small wrapper around a number: a node's position in the graph's arena (the one flat list holding every node), used for every parent and child link. It is only meaningful inside the graph that produced it, which is why anything that leaves the process uses the text id instead. `SymbolKind` is the four stack-neutral categories (`Container`, `Callable`, `Value`, `Schema`) that the rules for which things get a spec are written against, so those rules work the same for every language. `Symbol` is one declaration as it lives in a built graph, and `ExtractedSymbol` is the same declaration straight out of a single-file parse, with its links still as plain text until the graph is built. Everything here is owned data with no parser lifetimes attached, so it can be cached and serialised freely.

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
One declaration (a function, type, constant or table) as it lives in the built graph, with everything CodeOwl knows about it: where it is, its signature, its fingerprints, and how it connects to its parent and children.
### Behavior
Identity and place: `id` is a stable text handle, `<file>::<name>`, or `<file>::<class>.<method>` for a method, and is what MCP replies and spec files use, because a `SymbolId` (the arena index) only means something inside the graph that produced it. `file` and `lines` (1-based, inclusive) say where it sits. `kind` is the stack-neutral category and `raw` the language's own word for it (`fn`, `struct`, `class`, `table`, ...), kept for display and for pack-internal logic; the generic core never branches on `raw`. `signature` and `docstring` come from extraction.

Fingerprints: `source_hash` covers the whole span and, for a container, folds in its members in order, so any edit inside changes it. `interface_hash` covers only the public shape, so edits to a body leave it alone. It is `None` for anything not exported, since nothing outside the file can import it. Note that for some TypeScript type declarations the shape hashed is the whole declaration's tokens, not just the signature line.

Other fields: `is_exported` says whether another file can import it (always false for methods, which are reached through their class). `markers` holds language decorations verbatim (`#[derive(...)]`, `@Path`, `@app.route`) for the pack to read back. `extra_spans` holds extra line ranges, such as folded `impl` blocks. `parent` and `children` are arena ids for containment; a top-level declaration's parent is its file. The older fields `raw`, `markers` and `extra_spans` default when absent so old caches still load, and a format-version bump forces a rebuild anyway.
### Depends on
- (none)

## `ExtractedSymbol`
`pub struct ExtractedSymbol`
### Summary
The form a declaration takes straight after a single file has been parsed, before the repo-wide graph exists. It carries the same facts as `Symbol` except that its links to other symbols are still plain text names.
### Behavior
Extraction looks at one file at a time and cannot know about other files, so it has no arena (the graph's flat list of nodes) to hand out positions from. `parent` and `children` are therefore string ids. A `parent` of `None` means "top-level in this file, not inside another symbol", not "has no container at all". `Graph::build` later turns every string id into a real `SymbolId` and gives top-level symbols their file as parent, once all the files are known together. The other fields (`id`, `kind`, `raw`, `file`, `lines`, `signature`, `docstring`, `is_exported`, `source_hash`, `interface_hash`, `markers`) have the same meaning as on `Symbol` and are carried across unchanged. `extra_spans` holds additional line ranges that belong to the symbol but lie outside its own range and its children's. The case today is each `impl` block folded into a Rust type, so the `impl Trait for Type` header, the one place the trait is named, is scanned for dependencies and shown as source. It is empty for every other symbol. `raw`, `markers` and `extra_spans` default when absent so older cached data still loads. The struct is plain data, serialised to the cache.
### Depends on
- (none)
