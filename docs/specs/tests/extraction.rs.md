---
kind: file
source_paths: [tests/extraction.rs]
file: { source_hash: f1bc37633ed94502e1e481bd61ba807b3a1f1514a8284247ce7771990524842e, deps_hash: 4c0a824e87206f229e91a85487956d98ae6c40d484fe2d4cecfef6a84733b7ad, spec_hash: da119a3ae8293525cd33b7511f3343cbd7ff3d4f40edc9fb365cd6703dbf2c9c }
symbols:
  tests/extraction.rs::react_component_with_hooks_and_generics: { source_hash: 83a97a6c030790e83d21d2ab34c38a45e6d425a058ae351e2a5e4ec49583ab5f, deps_hash: 4c0a824e87206f229e91a85487956d98ae6c40d484fe2d4cecfef6a84733b7ad, spec_hash: c56037b9e805ef39245dccb0455e42b42bb4d0c897123b432b0d9d1a4a83a20c }
  tests/extraction.rs::barrel_file_has_no_declarations: { source_hash: a6cb147b50fb900a835e84be70c7ebf045a9558c9a2bc9a8e89fe6f5124a927d, deps_hash: a65fd4a58cea4f4c11f1e2fd8d648b47cb091b15cf89775a510aac9dea4c2ba5, spec_hash: 380acc544f60ee2273e0a15e93742d5e86268ee4e2e4bf4cad4bfcde279ba0de }
---
# tests/extraction.rs
## Summary
Integration tests for the TypeScript extractor, run against realistic fixture files and covering the hardest real cases the first extraction milestone called for. One test uses a React component with hooks and a generic function, and checks that exactly the three top-level functions are found, that nothing nested inside the component, such as callbacks and inner arrow functions, leaks out as a symbol of its own, and that the doc comment and generic type parameter are captured. The other uses a barrel file that only re-exports names from other files and checks that it produces no symbols at all. Together they pin down the extractor's rules about what counts as a declaration.

## `react_component_with_hooks_and_generics`
`fn react_component_with_hooks_and_generics()`
### Summary
A test that checks the TypeScript extractor handles a realistic React component file: it should find exactly the three top-level functions and nothing from inside the component's body.
### Behavior
Reads the fixture `fixtures/component.tsx` into the extractor as the file `component.tsx`. It asserts the symbol ids, in order, are exactly `UserBadge`, `findFirst` and `fetchUser`, so nothing nested inside `UserBadge`, such as the callback passed to the effect hook or its inner arrow functions, appears as a symbol of its own. It asserts that every symbol is a callable with raw kind `function`. It then checks that `UserBadge` has the doc comment "Fetches and displays a user's display name." and a signature containing its name, and that `findFirst`'s signature contains the generic type parameter `<T>`, failing with the actual signature in the message if not. The test fails on any mismatch in count, order, kind, docstring or signature.
### Depends on
- `src/symbol.rs::SymbolKind` — codeowl
- `src/extract.rs::extract_file` — codeowl

## `barrel_file_has_no_declarations`
`fn barrel_file_has_no_declarations()`
### Summary
A test that checks a barrel file, one that only re-exports names from other files, produces no symbols, since it declares nothing of its own.
### Behavior
Reads the fixture `fixtures/barrel.ts` into the TypeScript extractor as the file `barrel.ts` and asserts the returned list of symbols is empty. This guards the rule that forwarding statements such as `export { x } from './y'` and `export * from './y'` declare nothing new, which is also why a barrel file is later not given a spec. It fails if any symbol comes back.
### Depends on
- `src/extract.rs::extract_file` — codeowl
