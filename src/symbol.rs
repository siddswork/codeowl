use serde::Serialize;

/// What a declaration represents.
///
/// M1 only extracts top-level function/class/const declarations plus class
/// methods — nested closures, interfaces, and type aliases are out of scope
/// for now (see `ROADMAP.md`'s M1 entry). `Method` exists so a class's
/// `children` has something to point at; it isn't one of the "function/
/// class/const" kinds named in scope, but a class with no visible members
/// wouldn't exercise the containment tree at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
/// M2 is what introduces the real `SymbolId` arena (see `CLAUDE.md`'s Rust
/// conventions); M1 has no graph to index into yet.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Symbol {
    pub id: String,
    pub kind: SymbolKind,
    pub file: String,
    /// 1-indexed `[start_line, end_line]`, inclusive.
    pub lines: [usize; 2],
    pub signature: String,
    pub docstring: Option<String>,
    pub parent: Option<String>,
    pub children: Vec<String>,
}
