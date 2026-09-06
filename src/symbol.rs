use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

/// A single extracted declaration.
///
/// `id` is a deterministic string (`<file>::<name>`, or
/// `<file>::<class>.<method>` for methods) rather than an arena index —
/// the real `SymbolId` arena (see `CLAUDE.md`'s Rust conventions) is a
/// later M2 step; this first M2 increment only adds hashing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    pub parent: Option<String>,
    pub children: Vec<String>,
}
