//! The record types a language pack's import reader returns, shared by every
//! pack: a file's named imports, re-exports and default imports, plus the
//! fully-qualified references some languages reach another module through.
//! Reading a file's import statements is each pack's own job (for TypeScript,
//! `typescript::extract_imports`); this module holds only the shapes they
//! all fill in, and which resolution and the graph consume.

use serde::{Deserialize, Serialize};

/// One named import: `import { <imported_name> } from '<specifier>'`. A
/// local alias (`import { Foo as Bar }`), if any, is purely a local rename
/// — irrelevant to resolving what it points at, so it isn't tracked here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRef {
    pub specifier: String,
    pub imported_name: String,
}

/// One named re-export: `export { <source_name> as <exported_as> } from
/// '<specifier>'` — how a barrel forwards a name it doesn't declare itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReExport {
    pub exported_as: String,
    pub specifier: String,
    pub source_name: String,
}

/// One default import: `import <local_name> from '<specifier>'` (or the
/// default half of `import <local_name>, { ... } from '<specifier>'`).
/// Tracked for the M11 rendered-component resolver — React components are
/// almost always default exports, so the named-import list alone can't
/// resolve `<Component/>` to a file. Resolution is file-level only (the
/// target file, not a specific symbol); see `typescript::resolve_default_imports`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultImport {
    pub local_name: String,
    pub specifier: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileImports {
    pub imports: Vec<ImportRef>,
    pub re_exports: Vec<ReExport>,
    #[serde(default)]
    pub default_imports: Vec<DefaultImport>,
    /// A fully-qualified same-crate/package reference with no `use`/
    /// `import` statement at all (`ARCHITECTURE.md` open question 14,
    /// `ROADMAP.md` M21.h) -- Rust's `crate::a::b::Item` and Java's
    /// cross-package FQN both reach another module without ever naming it
    /// at the top of the file, so `imports`/`re_exports` alone miss them.
    /// Kept as its own field rather than folded into `imports`: Java's
    /// same-package scan (`java.rs::same_package_edges`) treats an
    /// explicit `imports` entry for a simple name as shadowing a
    /// same-package guess for that name, and a fully-qualified reference
    /// to a *different* same-named type must not trigger that shadow
    /// (`FightApiMapper`'s two distinct `Fight` classes -- the real case
    /// this exists for). TypeScript and Python never populate this: ES
    /// modules and Python modules have no dotted global-path syntax that
    /// reaches another module without naming it at the use site.
    #[serde(default)]
    pub qualified_refs: Vec<ImportRef>,
}
