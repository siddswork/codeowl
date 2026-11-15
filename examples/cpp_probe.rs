//! Prints what the C++ pack reads from a repository, as tab-separated lines
//! for `utility/cpp_accuracy/compare.py`. Run with
//! `cargo run --release --example cpp_probe -- <repo>`.
//!
//! Every C++ file under the repository is read (`.git` skipped), so the
//! comparison can follow include chains through files outside its scope.
//!
//! | Line | Fields |
//! |---|---|
//! | `F` | file, parse errors, line count |
//! | `I` | file, `q` (quotes) or `a` (angle brackets), the include path |
//! | `S` | file, kind (`class` `enum` `func` `proto` `macro` `test` `alias` `var`), name, line, 1 if at namespace scope |
//! | `N` | file, name of a top-level enum, one of its enumerators |
//! | `E` | the pack's own edge: file, the file of the target, the target symbol's name |
//!
//! The `E` lines come from the same functions the index runs
//! (`cpp::includes::extract`, then `cpp::edges::resolve` over a graph of the
//! folded symbols), called directly because `abseil`, `fmt` and `json` have
//! Python scripts and `detect` refuses a repository with two stacks.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use codeowl::graph::{FileExtraction, Graph};
use codeowl::hash::hash_text;
use codeowl::imports::FileImports;
use codeowl::symbol::SymbolKind;

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // `file_type` does not follow a symlink, so a link back up the tree
        // cannot make the walk loop or read a file twice.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if path.file_name().is_some_and(|n| n != ".git") {
                collect(root, &path, out);
            }
        } else if let Ok(rel) = path.strip_prefix(root) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            if codeowl::cpp::is_cpp_file(&rel) {
                out.push(rel);
            }
        }
    }
}

/// `#include "x"` / `#include <x>` lines: (form, path).
fn includes(source: &str) -> Vec<(char, String)> {
    let mut found = Vec::new();
    for line in source.lines() {
        let Some(rest) = line.trim_start().strip_prefix('#') else {
            continue;
        };
        let Some(inc) = rest.trim_start().strip_prefix("include") else {
            continue;
        };
        let inc = inc.trim();
        if let Some(p) = inc.strip_prefix('"').and_then(|x| x.split('"').next()) {
            found.push(('q', p.to_string()));
        } else if let Some(p) = inc.strip_prefix('<').and_then(|x| x.split('>').next()) {
            found.push(('a', p.to_string()));
        }
    }
    found
}

fn main() {
    let root = std::env::args().nth(1).expect("usage: cpp_probe <repo>");
    let root = Path::new(&root);
    let mut files = Vec::new();
    collect(root, root, &mut files);
    files.sort();

    let mut extractions: Vec<FileExtraction> = Vec::new();
    let mut file_imports: HashMap<String, FileImports> = HashMap::new();
    for rel in files {
        let Ok(bytes) = std::fs::read(root.join(&rel)) else {
            continue;
        };
        let source = String::from_utf8_lossy(&bytes);
        extractions.push(FileExtraction {
            rel_path: rel.clone(),
            source_hash: hash_text(&source),
            symbols: codeowl::cpp::extract_file(&source, &rel),
        });
        file_imports.insert(rel.clone(), codeowl::cpp::includes::extract(&source));
        let (symbols, errors) = codeowl::cpp::extract_unfolded_counted(&source, &rel);
        println!("F\t{rel}\t{errors}\t{}", source.lines().count());
        for (form, path) in includes(&source) {
            println!("I\t{rel}\t{form}\t{path}");
        }

        let raw_of: HashMap<&str, &str> = symbols
            .iter()
            .map(|s| (s.id.as_str(), s.raw.as_str()))
            .collect();
        let prefix = format!("{rel}::");
        for sym in &symbols {
            let name = match &sym.parent {
                Some(p) => sym
                    .id
                    .strip_prefix(&format!("{p}::"))
                    .unwrap_or(&sym.id)
                    .to_string(),
                None => sym.id.strip_prefix(&prefix).unwrap_or(&sym.id).to_string(),
            };
            let top = sym.parent.is_none();
            let declared = sym.markers.iter().any(|m| m == "declaration");
            let kind = match (sym.kind, sym.raw.as_str()) {
                (SymbolKind::Container, "enum") => "enum",
                (SymbolKind::Container, _) => "class",
                (_, "macro") => "macro",
                (_, "test") => "test",
                (_, "enumerator") => {
                    if let Some(p) = &sym.parent
                        && raw_of.get(p.as_str()) == Some(&"enum")
                        && let Some(owner) = p.strip_prefix(&prefix)
                        && !owner.contains("::")
                    {
                        println!("N\t{rel}\t{owner}\t{name}");
                    }
                    if top { "var" } else { continue }
                }
                (SymbolKind::Callable, _) if declared => "proto",
                (SymbolKind::Callable, _) => "func",
                (_, "typedef" | "using" | "concept") => "alias",
                _ => "var",
            };
            println!(
                "S\t{rel}\t{kind}\t{name}\t{}\t{}",
                sym.lines[0],
                u8::from(top)
            );
        }
    }

    // The pack's own edges, as the index would build them.
    let graph = Graph::build(extractions);
    let mut edges: BTreeSet<(String, String, String)> = BTreeSet::new();
    for e in codeowl::cpp::edges::resolve(&file_imports, &graph) {
        let Some(target) = e.target else { continue };
        let id = graph.string_id(target);
        let file = graph.owning_file_id(target);
        let name = id.strip_prefix(&format!("{file}::")).unwrap_or(id);
        edges.insert((e.from_file, file.to_string(), name.to_string()));
    }
    for (from, file, name) in edges {
        println!("E\t{from}\t{file}\t{name}");
    }
}
