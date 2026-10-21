# One directory per language pack: plan

Status (2026-10-21): **approved by Sidd. PR A (`typescript/`) is built and in review; PR B (`rust/`, `java/`, `python/`) is not started.** Follows `docs/architecture-refactor-plan.md` (PRs #102 to #104). Pure moves: no logic changes.

PR A as built, in three commits: (1) the five file moves and the `imports.rs` split; (2) the `features.rs` split, along a seam that was already clean (the generic walk referenced none of the TypeScript names; 15 of its 19 tests moved with the TypeScript half); (3) the `Graph` test helpers moved out of `graph.rs` into a test-only `src/fixtures.rs`, which the plan did not foresee: once the pack was a directory whose `mod.rs` re-exports its resolver, the graph's test-only call into the pack closed a loop in CodeOwl's own import graph. Measured: `graph_stats` identical on `master` and the branch for the seven repos that produce output, no crate-relative import unresolved in CodeOwl's own index, 581 tests unchanged. The accepted cycle now lists five modules (`typescript` joined it, having been hidden inside `features.rs`); `DECISIONS.md` q25 has the detail and one oddity noticed in `feature_slug`.

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
