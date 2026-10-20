---
kind: file
source_paths: [src/rust_crates.rs]
file: { source_hash: d8ef26c21d633a7516d5065397f681b5dc0a928ffb9462a928720e8a50a56fd0, deps_hash: baf806a55f27b576723e1a9bc9913fd5657a443ce71840b90578afc60daa8162, spec_hash: 4d9c479b0ec2aa5d113cd6b39804533923218700c91512d92146907228338817 }
symbols:
  src/rust_crates.rs::CrateMap: { source_hash: f6e77727f3b6b0983dbe8bcde4dbff3680ab3911335a751ed719268f8bd2b672, deps_hash: baf806a55f27b576723e1a9bc9913fd5657a443ce71840b90578afc60daa8162, spec_hash: d34659ec7926decddac26e2e0cc669a3fef37eeef34abd91fda8dab318248df5 }
  src/rust_crates.rs::parent_dir: { source_hash: 49a05a4c58fc3ce114cce587fc428abf0ffcfd6f5ee6dd23a2f5855edbe7b2bf, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c74a2c391544e98bf03f5eab36d3760543cf4f83e53c3260b1843fd2f8e75320 }
  src/rust_crates.rs::join: { source_hash: 4206c372ea95a10c667ed752248cf276c12d4bca01dab72d96cc6dad0460f7b9, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3fd583fd0b82a57b3288cb2881ac4564c4cbdff8ba1361514dbb2cccbe838c9a }
dep_targets:
  file -> src/imports.rs::FileImports: 7a834189d6b3
  src/rust_crates.rs::CrateMap -> src/imports.rs::FileImports: 7a834189d6b3
---
# src/rust_crates.rs
## Summary
This file works out which Cargo crate a Rust file belongs to and which crate names a `use` statement can begin with, which Rust import resolution needs in two places. In `crate::x`, `crate` means the root of this file's own crate, not the whole repo. In `use some_crate::x`, `some_crate` names a library crate, which may be a sibling in a multi-crate workspace or the very package a binary, test or example sits in. It reads the `Cargo.toml` files above the indexed source files to learn each package's library name and where its root file is, skipping any manifest that is missing or does not parse. It answers two questions: `lib_root(name)` returns the folder of a library crate that is part of the repo, and `crate_root(file)` returns the folder `crate::` refers to for a file, found by walking up to the nearest folder holding a `lib.rs`, a `main.rs` or a declared library root. Files in no such folder, such as `tests/` and `examples/`, fall back to their own package's `src`. Everything is computed from the indexed files and manifests and visited in sorted order, so two crates claiming one name always resolve the same way. The two small text helpers, `parent_dir` and `join`, handle repo-relative paths with forward slashes.

## `CrateMap`
`pub struct CrateMap`
### Summary
Keeps track of which Rust library crates exist inside the repo and where each one's root lives, so a `use` statement can be matched to the right file, even in a repo made of several crates.
### Behavior
Holds three things: `libs` maps a library crate's name to the directory holding its root file, `package_dirs` lists directories with a `Cargo.toml` that has a `[package]`, and `lib_dirs` lists directories holding a library root, including a `[lib] path` whose file is not named `lib.rs`.

`load(root, files)` reads every `Cargo.toml` in or above a directory that contains an indexed file. A package only contributes a name if it has a library root among the indexed files, since a binary-only package cannot be imported. The name is `[lib] name` if set, otherwise `[package] name`, with hyphens turned into underscores as Cargo does. A manifest that is missing or fails to parse is skipped, never an error. `lib_root(name)` returns the directory for a library crate in this repo, or `None` for `std`, `serde` or any dependency that is not part of the repo.

`crate_root(file, files)` gives the directory that `crate::` means for a file: the nearest enclosing directory that holds a `lib.rs` or `main.rs`, or that a manifest declares as a library root. So a binary with its own modules resolves inside itself, and two workspace crates each resolve inside themselves. A file under no such directory, such as one in `tests/` or `examples/`, falls back to the `src` of the package that owns it, never another package's, and to a top-level `src` when there is no `Cargo.toml` above it.
### Depends on
- `src/imports.rs::FileImports` — crate::imports
- externals: std

## `parent_dir`
`fn parent_dir(path: &str) -> String`
### Summary
Gives the folder part of a file path: `crates/a/src/x.rs` becomes `crates/a/src`, and a file at the top level gives an empty string.
### Behavior
Splits the path at its last `/` and returns everything before it as a new string. If the path has no `/` at all, it returns an empty string, which stands for the repo root. It works on text only, uses forward slashes, and does not touch the file system, so it never fails and does not check that the path exists. A path ending in `/` gives the path without that trailing slash. It is used by the crate-root search to walk upward one folder at a time.
### Depends on
- (none)

## `join`
`fn join(dir: &str, rel: &str) -> String`
### Summary
Joins a folder and a relative path into one repo-relative path, such as `crates/a` and `src/lib.rs` into `crates/a/src/lib.rs`, tolerating an empty folder and a leading `./`.
### Behavior
First removes a leading `./` from the relative part if present. If the folder is empty, meaning the repo root, it returns the relative part alone, so no leading slash is produced. Otherwise it returns the folder, a `/`, and the relative part. It only manipulates text with forward slashes, does not touch the file system, and does not collapse `..` segments or check that the result exists. It cannot fail. It is used when turning a manifest's `[lib] path` into a file path to look up, and when testing whether a folder holds a root file such as `lib.rs` or `main.rs`.
### Depends on
- (none)
