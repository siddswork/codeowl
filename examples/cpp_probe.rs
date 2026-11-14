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

use std::collections::HashMap;
use std::path::Path;

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

    for rel in files {
        let Ok(bytes) = std::fs::read(root.join(&rel)) else {
            continue;
        };
        let source = String::from_utf8_lossy(&bytes);
        let (symbols, errors) = codeowl::cpp::extract_counted(&source, &rel);
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
            // A folded symbol stands for several declarations; the compiler
            // counts each, so the comparison gets one line per range.
            let starts = std::iter::once(sym.lines[0]).chain(sym.extra_spans.iter().map(|s| s[0]));
            for line in starts {
                println!("S\t{rel}\t{kind}\t{name}\t{line}\t{}", u8::from(top));
            }
        }
    }
}
