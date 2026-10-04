---
kind: file
source_paths: [tests/incremental.rs]
file: { source_hash: 1ad5a485214e403dcaec67a871efd35f4bfdae7e700e40bf0f430500c38948f5, deps_hash: ad8f991c443fc9d8b732781de3051224e6b4521e821e5f8b04fb5480ecd1267b, spec_hash: 5579561847c62b04b36cb74eac37a3db67661c53917463e9d1858b3ae4771766 }
symbols:
  tests/incremental.rs::tempdir: { source_hash: 6fa89551f32e496fc5be22bf0782865b4e364177b627c2ac0a438cfae40f7b1a, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 0b23cf7fe783477732954f91432c836c0ef4c5d95445cc43af78e2e8c3551cc6 }
  tests/incremental.rs::source_hash: { source_hash: 4c89c0d474dfbe2318dc471004542c647436ecad1c7a31eaf5967fa1984135be, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: ddd9f177ae3c31a3778ec923567ab64616014d756bb4140a1cea2cdeb3c8d4fd }
  tests/incremental.rs::watcher_reindexes_an_edited_file_without_restart: { source_hash: ad9e66f72d19a8c05e0956dccbd14181779178b6b7246bd9ce7e1604925aa0d8, deps_hash: ad8f991c443fc9d8b732781de3051224e6b4521e821e5f8b04fb5480ecd1267b, spec_hash: 53edb5e5c44dfb3a42ae95268c30a5448acc6f117f4da336fde1c8ac1b0134d9 }
  tests/incremental.rs::watcher_picks_up_a_newly_created_file: { source_hash: 827b31b45f251ac8520d0ce47f20fb306fa0b2231c8b9ce5a1509bef559462e4, deps_hash: ad8f991c443fc9d8b732781de3051224e6b4521e821e5f8b04fb5480ecd1267b, spec_hash: d0e6ad151e132fc8a3f96f0620b5ed2a4cac7f9a46c93f123c4c5db3c12d89f0 }
  tests/incremental.rs::a_fresh_spawn_reuses_the_persisted_index: { source_hash: 3156f31cb90f4d1cf899104e4136257be61e4dc1a269e3fb8c550770e417b916, deps_hash: 454bac184e033d6d0f341c4aa6393dccebb041ea2e972089f7985c7b7f001acd, spec_hash: aa540e829223a7b975f17155df8b3be7e7ef1eddf45b218314ed0bb7dbe9b9a2 }
dep_targets:
  file -> src/index.rs::RepoIndex: 481419547059
  file -> src/mcp.rs::CodeOwlServer: 874e569c4376
  tests/incremental.rs::watcher_reindexes_an_edited_file_without_restart -> src/index.rs::RepoIndex: 481419547059
  tests/incremental.rs::watcher_reindexes_an_edited_file_without_restart -> src/mcp.rs::CodeOwlServer: 874e569c4376
  tests/incremental.rs::watcher_picks_up_a_newly_created_file -> src/index.rs::RepoIndex: 481419547059
  tests/incremental.rs::watcher_picks_up_a_newly_created_file -> src/mcp.rs::CodeOwlServer: 874e569c4376
  tests/incremental.rs::a_fresh_spawn_reuses_the_persisted_index -> src/index.rs::RepoIndex: 481419547059
---
# tests/incremental.rs
## Summary
End-to-end tests for keeping the index current without restarting, covering both moments that matter. Two tests check the live file watcher while a server runs: one edits an existing file and waits for the published graph to show a changed fingerprint, and the other creates a new file that imports an existing function and waits for that import to resolve in the graph. Both poll with a five-second deadline, because they depend on real file-system events. A third test covers starting up again: it opens the index once so it is saved to disk, edits one of two files while nothing is running, and opens again, asserting the catch-up report lists only the edited file as modified, with nothing added or removed, and that the graph still contains both files' declarations. Together they show that only changed files are re-read, that new files are noticed, and that the saved index is reused. Helpers create collision-proof temporary folders and read a symbol's fingerprint.

## `tempdir`
`fn tempdir(tag: &str) -> std::path::PathBuf`
### Summary
A test helper that creates a fresh, empty temporary folder for one test to use, with a name that cannot collide with another test's, even when tests run at the same time.
### Behavior
Builds a folder name from the prefix `codeowl-m9-`, the caller's tag, the process id, and a counter that increases on every call, under the system temporary directory. Because tests run concurrently in one process, the shared counter is what guarantees two tests never get the same folder, even with the same tag. It creates the folder and any missing parents and returns its path. It panics if creation fails, which is fine in test code, and it does not delete the folder afterwards.
### Depends on
- externals: std

## `source_hash`
`fn source_hash(graph: &codeowl::Graph, id: &str) -> String`
### Summary
A test helper that looks up a symbol by its id in a graph and returns its current whole-content fingerprint, so a test can compare it before and after an edit.
### Behavior
Finds the id in the graph, panicking with the message "symbol present" if there is none, fetches the symbol, and returns a copy of its `source_hash`. The fingerprint changes when anything inside the symbol changes, which is what the incremental-update tests use to show that an edited file was really re-read and an untouched one was not. It panics if the id is absent or is a file, which is acceptable in test code.
### Depends on
- (none)

## `watcher_reindexes_an_edited_file_without_restart`
`fn watcher_reindexes_an_edited_file_without_restart()`
### Summary
An end-to-end test that checks the live file watcher notices an edit to a file while the server is running and publishes an updated graph, with no restart.
### Behavior
Creates a temporary folder with a one-function TypeScript file, `f` returning 1, opens the index for it, and records the function's fingerprint. It creates a server from the graph and starts the file watcher with the server's shared graph cell, relying on the fact that `spawn` registers its folder watches before it returns, so an edit made immediately afterwards is guaranteed to be seen. It then rewrites the file so `f` returns 42.

It polls the shared cell every 50 milliseconds for up to five seconds, loading the current graph each time. The test passes as soon as the graph contains `a.ts::f` with a fingerprint different from the recorded one, which shows the watcher re-read the file and swapped in a new graph. If the deadline passes it fails with "watcher never republished the edited graph". The test depends on real file-system events and timing, so the generous deadline allows for debounce and slow machines.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index
- `src/mcp.rs::CodeOwlServer` — codeowl::mcp
- externals: std

## `watcher_picks_up_a_newly_created_file`
`fn watcher_picks_up_a_newly_created_file()`
### Summary
An end-to-end test that checks the live file watcher notices a brand-new file created while the server is running, and links it into the graph, not just edits to files that already existed.
### Behavior
Creates a temporary folder with one file, `lib.ts`, exporting a function `helper`, opens the index, wraps the graph in a server and starts the file watcher. It then writes a new file `app.ts` that imports `helper` from `./lib`. It polls the shared graph every 50 milliseconds for up to five seconds. The test passes as soon as the graph contains `lib.ts::helper` and at least one resolved import points to it, which shows the new file was picked up and its import resolved to the existing declaration. If the deadline passes it fails with "watcher never resolved the new importer's edge". It depends on real file-system events, so the deadline is generous.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index
- `src/mcp.rs::CodeOwlServer` — codeowl::mcp
- externals: std

## `a_fresh_spawn_reuses_the_persisted_index`
`fn a_fresh_spawn_reuses_the_persisted_index()`
### Summary
A test that checks starting CodeOwl a second time reuses the saved index and re-reads only the files that changed while nothing was running.
### Behavior
Creates a temporary folder with two files, `a.ts` and `b.ts`, each exporting one constant. A first `RepoIndex::open` stands in for the first run of the program, and the test asserts it created the saved `.codeowl/index` file. While nothing is running it then rewrites `b.ts` with a different value. A second `open` stands in for a fresh start. The test asserts the catch-up report lists exactly `b.ts` as modified, with nothing added and nothing removed, which shows the unchanged file was not re-read and the changed one was found by comparing fingerprints with the saved copy. It also asserts that the graph still contains both `a.ts::a` and `b.ts::b`, so reusing the saved index lost nothing.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index
