//! Which Cargo crate a Rust file belongs to, and which crate names a `use`
//! can start with. Rust import resolution (`rust.rs`) needs both: `crate::`
//! means "the root of *this file's* crate", and `use some_crate::x` names a
//! library crate by its name, whether that crate is a workspace sibling or
//! the very package a binary, test or example sits in.
//!
//! Everything here is derived from the walked file set plus the
//! `Cargo.toml` files that sit above those files, and is deterministic: the
//! directories are visited in sorted order, so two crates claiming one name
//! always resolve the same way.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use crate::imports::FileImports;

/// Importable library crates in the repo (crate name -> the directory that
/// holds the crate's root file), plus the package directories and library
/// root directories `crate_root` falls back on.
#[derive(Debug, Default)]
pub struct CrateMap {
    libs: HashMap<String, String>,
    /// Directories holding a `Cargo.toml` with a `[package]`.
    package_dirs: BTreeSet<String>,
    /// Directories holding a library crate root, including a `[lib] path`
    /// whose file is not named `lib.rs`.
    lib_dirs: BTreeSet<String>,
}

/// `"crates/a/src/x.rs"` -> `"crates/a/src"`; a top-level file -> `""`.
fn parent_dir(path: &str) -> String {
    path.rsplit_once('/')
        .map_or(String::new(), |(dir, _)| dir.to_string())
}

fn join(dir: &str, rel: &str) -> String {
    let rel = rel.strip_prefix("./").unwrap_or(rel);
    if dir.is_empty() {
        rel.to_string()
    } else {
        format!("{dir}/{rel}")
    }
}

impl CrateMap {
    /// Read every `Cargo.toml` that sits in a directory containing (or
    /// above) a walked file. A package contributes a name only if it has a
    /// library root among the walked files: a binary-only package can't be
    /// imported. The name is `[lib] name` when set, else `[package] name`,
    /// with hyphens turned into underscores as Cargo does. A manifest that
    /// is missing or does not parse is skipped, never an error.
    pub fn load(root: &Path, files: &HashMap<String, FileImports>) -> Self {
        let mut dirs: BTreeSet<String> = BTreeSet::new();
        for file in files.keys() {
            let mut dir = parent_dir(file);
            loop {
                if !dirs.insert(dir.clone()) || dir.is_empty() {
                    break;
                }
                dir = parent_dir(&dir);
            }
        }

        let mut libs = HashMap::new();
        let mut package_dirs = BTreeSet::new();
        let mut lib_dirs = BTreeSet::new();
        for dir in dirs {
            let Ok(text) = std::fs::read_to_string(root.join(&dir).join("Cargo.toml")) else {
                continue;
            };
            let Ok(doc) = text.parse::<toml::Table>() else {
                continue;
            };
            let Some(pkg) = doc
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
            else {
                continue; // a workspace-only manifest has no package
            };
            package_dirs.insert(dir.clone());
            let lib = doc.get("lib");
            let name = lib
                .and_then(|l| l.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or(pkg)
                .replace('-', "_");
            let lib_file = join(
                &dir,
                lib.and_then(|l| l.get("path"))
                    .and_then(|p| p.as_str())
                    .unwrap_or("src/lib.rs"),
            );
            if files.contains_key(&lib_file) {
                lib_dirs.insert(parent_dir(&lib_file));
                libs.entry(name).or_insert_with(|| parent_dir(&lib_file));
            }
        }
        Self {
            libs,
            package_dirs,
            lib_dirs,
        }
    }

    /// The directory holding the root of library crate `name`, if the repo
    /// has such a crate. `None` for `std`, `serde` and every other
    /// dependency that isn't part of this repo.
    pub fn lib_root(&self, name: &str) -> Option<&str> {
        self.libs.get(name).map(String::as_str)
    }

    /// The directory `crate::` means for `file`: the nearest enclosing
    /// directory that holds a `lib.rs` or `main.rs`, or that a manifest
    /// declares as a library root (`[lib] path`). That makes a binary with
    /// its own modules (`src/bin/tool/main.rs` plus siblings) resolve inside
    /// the binary, and two workspace crates each inside themselves.
    ///
    /// A file under no such directory (a `tests/` or `examples/` file)
    /// falls back to the `src` of the package that owns it, never another
    /// package's; with no `Cargo.toml` above it, to a top-level `src`.
    pub fn crate_root(&self, file: &str, files: &HashMap<String, FileImports>) -> String {
        let mut dir = parent_dir(file);
        loop {
            if self.lib_dirs.contains(&dir)
                || ["lib.rs", "main.rs"]
                    .iter()
                    .any(|root| files.contains_key(&join(&dir, root)))
            {
                return dir;
            }
            if dir.is_empty() {
                break;
            }
            dir = parent_dir(&dir);
        }
        // Nearest enclosing package: walk up again, stop at the first
        // directory that holds a `[package]` manifest.
        let mut dir = parent_dir(file);
        loop {
            if self.package_dirs.contains(&dir) {
                return join(&dir, "src");
            }
            if dir.is_empty() {
                return "src".to_string();
            }
            dir = parent_dir(&dir);
        }
    }
}
