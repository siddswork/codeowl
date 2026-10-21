//! Stack-neutral vocabulary every `StackPack` answers in: [`SourceKind`]
//! (what a pack says about a file it reads) and [`FileRole`] (what kind of
//! code a file holds). Which extension maps to which kind, and which path
//! is a test or generated, is each pack's decision, not this module's —
//! the TypeScript conventions live in `typescript.rs`, and pack selection
//! (`stack::detect`) next to the pack registry in `stack.rs`.

/// What a `StackPack::source_kind` says about a file it reads. Stack-
/// neutral vocabulary (like `SymbolKind`), but which *extension* maps to
/// which kind is each pack's decision, not this module's — `TypeScriptNextStack`
/// owns the `.sql` mapping, for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// A normal source file — parsed by the pack's grammar, and put
    /// through the import / flow-edge passes.
    Code,
    /// A dedicated schema file (the TS pack's `.sql`) — parsed for table
    /// declarations only; `index.rs` skips the import / flow-edge passes.
    /// An *in-language* ORM model is not this — it's a `Code` file whose
    /// model class `is_schema_symbol` retags to `SymbolKind::Schema` (M17).
    Schema,
}

/// What kind of code a file holds, for the purpose of deciding how its
/// spec is prioritised (`spec::prioritize`) and whether it counts as
/// shared infrastructure. "What's a UI primitive" and "what's generated"
/// are stack conventions, not universal truths, so each `StackPack`
/// supplies its own `classify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRole {
    /// Product code — the default. Eligible for the shared-code tier,
    /// counted in import fan-in, documented in normal priority order.
    Domain,
    /// A UI primitive (`components/ui/*`): imported nearly everywhere, so
    /// it dominates a fan-in ranking, but its summary tells a dependent
    /// feature spec nothing it couldn't guess — kept out of the
    /// shared-code tier. Still gets its own file spec, in the long tail.
    Primitive,
    /// Test code — an `e2e/`/`cypress/`/`playwright/` tree, a `__tests__/`
    /// directory, or a `.test.`/`.spec.` file. Stays in the graph so
    /// `get_callers` still shows "used by these tests", but its specs sort
    /// last and it's never treated as a product module.
    Test,
    /// Machine-generated code. A `StackPack` supplies the rule: the Java
    /// pack assigns it under `target/generated-sources` and
    /// `build/generated`, the Python pack to Alembic migration versions.
    /// Never spec-bearing, since a spec describes hand-written code.
    Generated,
}
