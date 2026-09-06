use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A node's position in a `Graph`'s arena (a `Symbol` or a `FileNode` — see
/// `graph.rs`'s `Node`). Only meaningful relative to the `Graph` that
/// produced it — not stable across a re-extraction, since nothing pins a
/// given node to the same index next time (that stability is what a node's
/// string id is for). Lives here, not in `graph.rs`, because `Symbol`
/// itself needs to name the type for `parent`/`children` below; `graph.rs`
/// re-exports it so callers don't need to know that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SymbolId(u32);

impl SymbolId {
    pub(crate) fn new(index: u32) -> Self {
        Self(index)
    }

    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }
}

/// What a declaration represents.
///
/// M1 only extracts top-level function/class/const declarations plus class
/// methods — nested closures, interfaces, and type aliases are out of scope
/// for now (see `ROADMAP.md`'s M1 entry). `Method` exists so a class's
/// `children` has something to point at; it isn't one of the "function/
/// class/const" kinds named in scope, but a class with no visible members
/// wouldn't exercise the containment tree at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Const,
}

/// A single extracted declaration, resolved into a `Graph`'s arena.
///
/// `id` stays a deterministic string (`<file>::<name>`, or
/// `<file>::<class>.<method>` for methods) — that's the stable handle
/// everything outside the arena (MCP responses, spec frontmatter, a human
/// re-running `codeowl extract`) uses, since a `SymbolId` is only valid for
/// the `Graph` that produced it. `parent`/`children` *do* use `SymbolId`:
/// this is `Graph::build`'s output, not `extract_file`'s (see
/// `ExtractedSymbol` below for the pre-arena shape) — every symbol has a
/// container now that files are arena nodes too (a top-level declaration's
/// parent is its file; only a file with no directory node above it, which
/// is every file in M4, has no parent of its own).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub kind: SymbolKind,
    pub file: String,
    /// 1-indexed `[start_line, end_line]`, inclusive.
    pub lines: [usize; 2],
    pub signature: String,
    pub docstring: Option<String>,
    /// Whether this declaration is reachable via `import` from another
    /// file — i.e. whether it's a valid target for the reference-edge
    /// resolution M2 wires in next. `false` for every `Method`: a method
    /// isn't imported on its own, only reached through an instance of an
    /// already-imported class, which is call-level resolution M2 defers.
    pub is_exported: bool,
    /// Content hash over this symbol's *entire* span (signature and body),
    /// rolled up Merkle-style for container symbols (a `Class`'s hash folds
    /// in its methods' hashes, in order) — see `ARCHITECTURE.md`'s
    /// "Caching and invalidation". Changes on *any* edit inside the symbol.
    pub source_hash: String,
    /// Content hash over just the exported *shape* — `signature`, never
    /// `docstring` or body — so implementation-only edits leave it
    /// unchanged. `None` when `is_exported` is `false`: nothing outside
    /// this file could resolve to it, so it isn't a fixed reference-edge
    /// invalidation key that needs tracking yet. This is gap 2's fix (see
    /// `CLAUDE.md`'s hard invariants).
    pub interface_hash: Option<String>,
    pub parent: Option<SymbolId>,
    pub children: Vec<SymbolId>,
}

/// What `extract_file` produces directly off the syntax tree, before a
/// `Graph` exists to assign arena positions.
///
/// Containment here is still a *string*, not a `SymbolId` — deliberately:
/// extraction is per-file and has no cross-file knowledge (see
/// `ARCHITECTURE.md`'s "Extractors"), so it has no arena to hand out
/// positions from yet. `parent: None` means "top-level in this file, not
/// contained by another *symbol*" — it does *not* mean "has no container at
/// all". `Graph::build` is what resolves every string id into a real
/// `SymbolId` and gives top-level symbols their file as a parent, once
/// every file's symbols (and the files themselves) are known together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedSymbol {
    pub id: String,
    pub kind: SymbolKind,
    pub file: String,
    pub lines: [usize; 2],
    pub signature: String,
    pub docstring: Option<String>,
    pub is_exported: bool,
    pub source_hash: String,
    pub interface_hash: Option<String>,
    pub parent: Option<String>,
    pub children: Vec<String>,
}
