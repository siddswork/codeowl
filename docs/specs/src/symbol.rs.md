---
kind: file
source_paths: [src/symbol.rs]
file: { source_hash: de25bd0e2225ae1bd8aa539ad35d10320f2cfff71c214ed2acd067ea53c13356, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 14d0ad65af303877f03964f52020458875c7a8edb1bc331e1431e02b810d1585 }
symbols:
  src/symbol.rs::SymbolId: { source_hash: d18f54b79071c19e0e51299d4a93165109e9fcf01e1b7cd89e9415875223690c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 7427243474274f480d2fb47f7f974dee8b9dde773902d0cdd3c623020249605c }
  src/symbol.rs::SymbolKind: { source_hash: 705c2902ac4016a078c0207dc562b429c21c755363ceab9dfc3608825ebc7fab, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3aaa3a5c23a61c4d6c074cf71da5ce649410d1c66e26997bbfd1dc21f4248cdb }
  src/symbol.rs::Symbol: { source_hash: dc8ed31a654d14bd2417d298b4fdbf254b507c31c2d274577bf2db4100fbb013, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c1c98e9cc2b15d5fb12fc977520c2ce41224e064eeb31e475bc1710dd8acc9d5 }
  src/symbol.rs::ExtractedSymbol: { source_hash: def12a5c5c4a289f6fd6a7153c542a4cc6ae1ddd0b2127a1d4fba832d13e092d, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c851597f4fe4e0fed5d2f9e3d630cf85dd7a8b9929d01481f72be5178c15319a }
---
# src/symbol.rs
## Summary
Defines the graph's node vocabulary. `SymbolId` is the arena-index newtype every containment link uses; `SymbolKind` is the stack-neutral `Container | Callable | Value | Schema` set the granularity rules key on; `Symbol` is one declaration as it lives in a built `Graph`; `ExtractedSymbol` is the same shape a pack's extractor produces per-file before a `Graph` exists, with string containment links `Graph::build` later resolves to `SymbolId`s. Together these are the boundary between "parsed a file" and "built the arena" — every type here is owned data with no tree-sitter lifetimes escaping.

## `SymbolId`
`pub struct SymbolId`
### Summary
A newtype wrapper over a `u32` index into a `Graph`'s node arena, together with its two `pub(crate)` accessors. It's the internal handle every containment reference (`Symbol::parent`, `Symbol::children`, `FileNode::children`) uses to point at another node without a Rust reference or `Rc`, which is what keeps the graph a flat, cheaply-serializable structure.
### Behavior
Holds a single private `u32`. It is only valid for the exact `Graph` that produced it: re-extracting the repo can assign a different index to the same declaration, so a `SymbolId` must never be persisted or handed to an MCP caller — the node's stable string id serves that purpose instead. The two accessors are both `pub(crate)`: `new(index: u32)` wraps a raw arena index and is called only by `Graph::build` as it reserves one slot per node; `index(self)` takes `self` by value (`SymbolId` is `Copy`) and widens the stored `u32` to a `usize` for indexing into `Graph`'s node vector. Outside the crate a `SymbolId` is opaque — obtained from and passed back to `Graph` methods, never constructed or inspected directly. Defined in `symbol.rs` rather than `graph.rs` because `Symbol` needs to name the type for its `parent`/`children` fields; `graph.rs` re-exports it so callers don't depend on that placement.
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
One extracted declaration as it lives in a built `Graph`'s arena — the node type behind every `get_symbol` / `get_callers` / `get_callees` answer. Carries the declaration's identity, location, signature, docstring, two content hashes, pack markers, and its containment links to parent and children (all by `SymbolId`).
### Behavior
`id` is the stable string handle (`<file>::<name>`, or `<file>::<container>::<method>` for a member) — the thing that survives re-extraction and is safe to hand a caller, unlike a `SymbolId`. `parent`/`children` are `SymbolId`s filled in by `Graph::build` once every file is known; a top-level declaration's parent is its `FileNode`. `source_hash` covers the declaration's whole span and rolls up Merkle-style for a container (editing a method moves its own and its container's hash); `interface_hash` covers only the exported signature and is `None` when `is_exported` is false, so a private symbol never acts as a reference-edge invalidation key. `raw` and `markers` are pack-owned (empty for the TypeScript pack, populated from M14); the generic core never interprets them.
### Depends on
- (none)

## `ExtractedSymbol`
`pub struct ExtractedSymbol`
### Summary
The pre-arena shape a pack's extractor produces directly off a parsed file, before any `Graph` exists to assign arena positions. `Graph::build` consumes a `Vec<ExtractedSymbol>` per file and turns each into an arena `Symbol`.
### Behavior
Identical fields to `Symbol` except `parent`/`children` are `String` ids, not `SymbolId`s: extraction is per-file and has no cross-file knowledge, so it can't hand out arena slots yet. `parent: None` means "top-level in this file", not "has no container" — `Graph::build` gives every top-level symbol its file as a parent once all files are known together. `raw` and `markers` are `#[serde(default)]` and carried straight through to the `Symbol` untouched.
### Depends on
- (none)
