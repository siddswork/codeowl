---
kind: file
source_paths: [tests/feature_components.rs]
file: { source_hash: 05559633a59efc94b982786afaa20540ead46b6a43fe4bd476273c323c0a4e48, deps_hash: dcd610b167fad1e8e0a2001ef3e19bdedc1c1481e981a771bcc324d95fc550f9, spec_hash: 6473b3f73450cfdcae2d9349f72df3f6b54828d41dbc4733131b985ec019d488 }
symbols:
  tests/feature_components.rs::tempdir: { source_hash: dd13c893d5eb86f72b251b5fa2699b0a27027e81e47b10555044cf9c5d8b8ad1, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 5b47a1addbc8100ac12254f2e526c2a369c7345b7cf1f16fd1c263f3bf6ce4ee }
  tests/feature_components.rs::build: { source_hash: 38ee617bdd3534412ebff811b80cbe5f8d21484787887e00438a4e3ff6a80d83, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: b8819df8c1dfb2c92101ea1bfc22ca522e33075ff1735dea9878814120d9a127 }
  tests/feature_components.rs::core_follows_the_rendered_component_subtree_into_its_fetches: { source_hash: ef835f61ddd4d14d45e22ade11fdc6693a3c717641f3e4060ce37cbcd8a8167d, deps_hash: dea08ec8ede72ecf86938bbf61525ab84aaacbc816c65bf4ef42916d467ba76d, spec_hash: 960d8177d2ef3172a3ffdb4fffca48a71ec89670ce89d82f77e9f42f43784154 }
dep_targets:
  file -> src/features.rs::assemble_participants: a6e5500838a2
  file -> src/index.rs::RepoIndex: 481419547059
  tests/feature_components.rs::build -> src/index.rs::RepoIndex: 481419547059
  tests/feature_components.rs::core_follows_the_rendered_component_subtree_into_its_fetches -> src/features.rs::assemble_participants: a6e5500838a2
---
# tests/feature_components.rs
## Summary
A test that checks a page's feature follows the components it renders, not only the web calls it makes directly. Without that, a page that is a thin server wrapper around a client component would produce a feature description written from the wrapper alone, with the real flow, including state, the calls and validation, invisible. It builds a small Next.js-style project in a temporary folder: a page that renders a client component placed beside it and also a shared button, the client component renders a form from another folder, and the form calls an API route. It then asserts the feature's core files include the page, the neighbouring client component, the form (included because it does data work even though it is not beside the page), and the API route reached through the form's call, while the presentational button, which does no data work and is not beside the page, stays out. The helper functions create a collision-proof temporary folder and build the graph from the sample files.

## `tempdir`
`fn tempdir(tag: &str) -> std::path::PathBuf`
### Summary
A test helper that creates a fresh, empty temporary folder for one test to build its fixture repo in, with a name that cannot collide with another test's, even when tests run at the same time.
### Behavior
Builds a folder name from the prefix `codeowl-fc-`, the tag the caller gives, the process id, and a counter that goes up by one on every call, under the system's temporary directory. Tests run concurrently inside one process, so the counter, which is shared across the whole run, is what guarantees two tests never get the same folder even with the same tag. It creates the folder and any missing parents, and returns its path. It panics if creation fails, which is acceptable in test code. It does not delete the folder afterwards; callers clean up.
### Depends on
- externals: std

## `build`
`fn build(dir: &std::path::Path) -> codeowl::Graph`
### Summary
A test helper that writes a small Next.js-style project into a folder and builds the full graph for it, so the feature tests have a realistic project to examine.
### Behavior
Writes five files into the given folder, creating any missing subfolders: a page at `app/judge/evaluate/page.tsx`, a client component next to it, a form component under `components/judge`, a UI button under `components/ui`, and an API route at `app/api/judge/evaluations/submit/route.ts`. The contents come from constants defined elsewhere in the test file. It then builds the index for the folder and rebuilds the graph from it, which resolves imports and flow edges as in normal use, and returns that graph. It panics on any failure, since a broken fixture should stop the test. The layout reproduces a real shape: a page renders a client component, which renders a form that calls the API route, and everything also uses the shared button.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index

## `core_follows_the_rendered_component_subtree_into_its_fetches`
`fn core_follows_the_rendered_component_subtree_into_its_fetches()`
### Summary
A test that checks a page's feature includes the components it shows and the API route they call, but leaves out a purely visual component, so the feature description covers real behavior and not layout.
### Behavior
Builds the sample project in a fresh temporary folder, finds the feature entry point whose file is the evaluation page, and assembles that feature's participants, taking the core file list. It asserts the core contains all four expected files, with a failure message showing the actual core if any is missing: the page itself, the client component next to it (included because it sits beside the entry file), the form component in a different folder (included even though it is not beside the page, because it makes a `fetch` call), and the API route (reached through that call). It then asserts the shared UI button is not in the core, because a presentational component that does no data work must stay out. Together this checks both admission rules: nearness, and doing data work.
### Depends on
- `src/features.rs::assemble_participants` — codeowl::features
