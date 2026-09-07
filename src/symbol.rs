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

/// What a declaration represents, in **stack-neutral** terms — the generic
/// core reasons about these four; the pack records the concrete grammar
/// kind (`"function"`, `"struct"`, `"macro_rules"`, `"@interface"`, …) in
/// [`ExtractedSymbol::raw`] for display and its own logic (M13 design
/// decision 1).
///
/// - `Container` — has members that get their own specs: a TS/Java class,
///   a Rust `struct` / `enum` / `trait` / `impl` / `mod`.
/// - `Callable` — a function, method, or constructor: the unit a caller
///   invokes. TS `function`/`method`, Rust `fn`, later Java methods.
/// - `Value` — a data-carrier with no spec of its own: a TS/Rust `const`
///   or `static`, a plain-data record. Rolls up into its file spec.
/// - `Schema` — M10's SQL `CREATE TABLE` (see `schema.rs`); M18 widens the
///   source off the `.sql`-file assumption. Resolvable and queryable but
///   never spec-bearing.
///
/// `spec.rs`'s granularity rule generates a document for a *top-level*
/// `Container` or `Callable` (a method is a `Callable` whose parent is a
/// `Container`, so it stays inside the container's section) — pack-agnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Container,
    Callable,
    Value,
    Schema,
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
    /// The pack's concrete grammar kind for this declaration —
    /// `"function"` / `"method"` / `"class"` / `"const"` for TS, `"table"`
    /// for SQL, `"fn"` / `"struct"` / `"trait"` / `"macro_rules"` / … from
    /// M14. The generic core never branches on it; it's for display and
    /// pack-internal logic (M13 design decision 1). `#[serde(default)]` for
    /// pre-v4 caches, which `FORMAT_VERSION` forces a rebuild of anyway.
    #[serde(default)]
    pub raw: String,
    pub file: String,
    /// 1-indexed `[start_line, end_line]`, inclusive.
    pub lines: [usize; 2],
    pub signature: String,
    pub docstring: Option<String>,
    /// Whether this declaration is reachable via `import` from another
    /// file — i.e. whether it's a valid target for the reference-edge
    /// resolution M2 wires in next. `false` for every method (a `Callable`
    /// with a `Container` parent): a method isn't imported on its own, only
    /// reached through an instance of an already-imported class, which is
    /// call-level resolution M2 defers.
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
    /// Stack-specific syntactic decorations on this declaration, verbatim
    /// as the pack's extractor chose to record them — a Rust `#[tool]` /
    /// `#[derive(...)]`, a Java `@Path` / `@Entity`, a Python `@app.route`.
    /// The generic core never interprets these; a `StackPack` reads its
    /// own back out (M13-pre spike / M13 design decision 9 — the Java
    /// feature and schema model is entirely annotation-driven, and
    /// `signature` string-matching is not a substitute). Empty for the
    /// TypeScript+Next pack in M13; populated from M14 on.
    #[serde(default)]
    pub markers: Vec<String>,
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
    /// See [`Symbol::raw`]. Set by the pack's extractor; `Graph::build`
    /// carries it straight through to the arena `Symbol`.
    #[serde(default)]
    pub raw: String,
    pub file: String,
    pub lines: [usize; 2],
    pub signature: String,
    pub docstring: Option<String>,
    pub is_exported: bool,
    pub source_hash: String,
    pub interface_hash: Option<String>,
    /// See [`Symbol::markers`]. Set by the pack's extractor; `Graph::build`
    /// carries it straight through to the arena `Symbol`.
    #[serde(default)]
    pub markers: Vec<String>,
    pub parent: Option<String>,
    pub children: Vec<String>,
}
