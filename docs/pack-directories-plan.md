# One directory per language pack: plan

Status (2026-10-21): **approved by Sidd; PR A (`typescript/`) in progress.** Follows `docs/architecture-refactor-plan.md` (PRs #102 to #104). Pure moves: no logic changes.

## Why

After the architecture review, `spec/` is a directory of layered modules, but the packs are not laid out alike. `rust.rs`, `java.rs` and `python.rs` each hold their pack's extractor and resolver in one file, while the TypeScript pack is spread across five generically named files (`extract.rs`, `imports.rs`, `resolve.rs`, `schema.rs`, `features.rs`) next to a small `typescript.rs` of helpers. A reader cannot tell from the name that `extract.rs` is the TypeScript extractor, and `imports.rs` holds the import record types every language shares inside a TypeScript-specific file. The same applies to the framework models that belong to a pack (`rust_crates.rs`, `quarkus.rs`, `fastapi.rs`).

## Decisions (Sidd, 2026-10-21)

- One directory per pack, named for the language. The framework or build helpers that belong to a pack go inside it.
- The TypeScript extractor, today `extract.rs`, becomes the pack's main file, `typescript/mod.rs`.
- The helper file that is `typescript.rs` today is renamed `typescript/conventions.rs` (which files are source, test or UI primitive, which parser to load, which extensions the resolver tries).
- The shared import record types stay in a neutral `src/imports.rs`, which shrinks to types only. The TypeScript reading of import statements moves to `typescript/imports.rs`.
- `features.rs` is split: the `FeatureModel` trait, `EntryPoint`, `feature_model_for` and participant assembly stay; the Next/Supabase extractors and `TypeScriptNextFeatureModel` move to `typescript/features.rs`.
- Within `rust/`, `java/` and `python/` the current file becomes `mod.rs` and is not split further. That waits until a pack earns it.

## Target layout

```
src/typescript/  mod.rs (the extractor)  conventions.rs  imports.rs  resolve.rs  schema.rs  features.rs
src/rust/        mod.rs (today's rust.rs)      crates.rs  (today's rust_crates.rs)
src/java/        mod.rs (today's java.rs)      quarkus.rs
src/python/      mod.rs (today's python.rs)    fastapi.rs
src/imports.rs   the shared import record types only
src/features.rs  the generic feature layer only
```

## PRs

| PR | What | Checks |
|---|---|---|
| A | `typescript/`: the five moves, the `imports.rs` split, the `features.rs` split | `graph_stats` identical on master and the branch across the nine test repos; test count unchanged; CodeOwl's own index resolves every crate-relative import; the public names in `lib.rs` (`extract_file`, `extract_imports`, `build_resolver`, `resolve_imports`) keep working |
| B | `rust/`, `java/`, `python/` with their companions | the same checks |

Both land before the full spec regeneration, because moving files makes freshly generated specs wrong at once.

## Rules to keep (from `spec/`)

- Re-exports are explicit `pub use self::module::{...}` lists: CodeOwl's own resolver follows a named re-export but not a glob, and does not resolve a bare child-module path (`DECISIONS.md` q24).
- A pack never imports from `spec/`; `graph.rs` never names a pack.
- No `FORMAT_VERSION` bump: nothing a pack produces changes.
