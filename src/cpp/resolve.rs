//! Which repository file an `#include` means, and everything a file reaches
//! through its includes. Step 2 of `docs/cpp-step2-plan.md`, part B. Pure
//! functions over a list of paths: nothing here reads the disk.
//!
//! A compiler looks for `"x.h"` beside the including file first and for
//! `<x.h>` only on its search path, so the two forms resolve by different
//! rules. Without the compiler's real search path (`compile_commands.json`,
//! a later step) the pack tries the usual roots, then a path-suffix match
//! that must be unique: a guess that is wrong is worse than none, because a
//! wrong edge makes unrelated files stale.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::imports::FileImports;

/// The repository's C++ files, indexed for include lookup.
pub struct FileSet {
    files: BTreeSet<String>,
    /// Every path-suffix that starts at a `/` boundary (and the whole path)
    /// to the files that end with it: `a/b/c.h` is under `a/b/c.h`, `b/c.h`
    /// and `c.h`.
    by_suffix: HashMap<String, Vec<String>>,
}

impl FileSet {
    pub fn new<'a>(paths: impl IntoIterator<Item = &'a str>) -> Self {
        let files: BTreeSet<String> = paths.into_iter().map(str::to_string).collect();
        let mut by_suffix: HashMap<String, Vec<String>> = HashMap::new();
        for path in &files {
            let mut rest = path.as_str();
            loop {
                by_suffix
                    .entry(rest.to_string())
                    .or_default()
                    .push(path.clone());
                match rest.split_once('/') {
                    Some((_, tail)) => rest = tail,
                    None => break,
                }
            }
        }
        FileSet { files, by_suffix }
    }

    /// The repository file `specifier` (an include as `includes::extract`
    /// returns it, with its quotes or angle brackets) means, written in
    /// `from_file`. `None` when no file matches or several do.
    ///
    /// | Form | Tried in order |
    /// |---|---|
    /// | `"p"` | beside `from_file`; the root; `include/`; `src/`; a unique suffix match |
    /// | `<p>` | the root; `include/`; `src/`; a unique suffix match, only if `p` has a directory |
    ///
    /// A bare `<time.h>` is almost always a system header, and measured on
    /// `abseil` a single rule for both forms resolved it to
    /// `absl/time/time.h` four times.
    pub fn resolve(&self, from_file: &str, specifier: &str) -> Option<&str> {
        let (quoted, path) = if let Some(p) = specifier.strip_prefix('"') {
            (true, p.strip_suffix('"')?)
        } else {
            (false, specifier.strip_prefix('<')?.strip_suffix('>')?)
        };
        if path.is_empty() {
            return None;
        }
        let dir = from_file.rsplit_once('/').map_or("", |(d, _)| d);
        let mut roots: Vec<&str> = Vec::new();
        if quoted {
            roots.push(dir);
        }
        roots.extend(["", "include", "src"]);
        for root in roots {
            let joined = if root.is_empty() {
                path.to_string()
            } else {
                format!("{root}/{path}")
            };
            if let Some(found) = normalize(&joined).and_then(|n| self.files.get(&n)) {
                return Some(found);
            }
        }
        // Last resort: a path that ends the same way, if exactly one does.
        // A bare name in angle brackets does not get this (see above).
        if !quoted && !path.contains('/') {
            return None;
        }
        let key = normalize(path)?;
        match self.by_suffix.get(&key).map(Vec::as_slice) {
            Some([only]) => Some(only),
            _ => None,
        }
    }
}

/// `a/./b/../c.h` as `a/c.h`; `None` if it climbs out of the root.
fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The resolved includes of every file, and what each file reaches.
#[derive(Debug, Default)]
pub struct IncludeGraph {
    /// File to the repository files it includes directly, sorted, no
    /// duplicates, itself left out.
    direct: BTreeMap<String, Vec<String>>,
    /// Includes that named no repository file: (file, specifier). System
    /// headers are most of them.
    pub unresolved: Vec<(String, String)>,
}

impl IncludeGraph {
    /// Resolve every include of every file in `file_imports` (the keys are
    /// the repository's C++ files). Deterministic: files in path order.
    pub fn build(file_imports: &HashMap<String, FileImports>) -> Self {
        let set = FileSet::new(file_imports.keys().map(String::as_str));
        let mut graph = IncludeGraph::default();
        let mut paths: Vec<&String> = file_imports.keys().collect();
        paths.sort();
        for path in paths {
            let mut direct: BTreeSet<String> = BTreeSet::new();
            for import in &file_imports[path].imports {
                match set.resolve(path, &import.specifier) {
                    Some(target) if target != path => {
                        direct.insert(target.to_string());
                    }
                    Some(_) => {}
                    None => graph
                        .unresolved
                        .push((path.clone(), import.specifier.clone())),
                }
            }
            graph
                .direct
                .insert(path.clone(), direct.into_iter().collect());
        }
        graph
    }

    /// The files `file` includes directly.
    pub fn includes(&self, file: &str) -> &[String] {
        self.direct.get(file).map_or(&[], Vec::as_slice)
    }

    /// Every file reachable from `file` through includes, sorted, `file`
    /// itself left out. Includes form cycles in real code (a header and the
    /// header it forward-declares into), and a visited set ends them.
    pub fn reach(&self, file: &str) -> Vec<String> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut stack: Vec<&str> = self.includes(file).iter().map(String::as_str).collect();
        while let Some(next) = stack.pop() {
            if next == file || !seen.insert(next) {
                continue;
            }
            stack.extend(self.includes(next).iter().map(String::as_str));
        }
        seen.into_iter().map(str::to_string).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imports::ImportRef;

    fn set(paths: &[&str]) -> FileSet {
        FileSet::new(paths.iter().copied())
    }

    fn q(p: &str) -> String {
        format!("\"{p}\"")
    }

    fn a(p: &str) -> String {
        format!("<{p}>")
    }

    #[test]
    fn a_quoted_include_is_tried_beside_the_file_then_the_roots_in_order() {
        let all = set(&["a/x.h", "x.h", "include/x.h", "src/x.h", "lib/deep/x.h"]);
        assert_eq!(all.resolve("a/b.cc", &q("x.h")), Some("a/x.h"));
        // Without the sibling: the root, then include/, then src/.
        let no_sibling = set(&["x.h", "include/x.h", "src/x.h", "lib/deep/x.h"]);
        assert_eq!(no_sibling.resolve("a/b.cc", &q("x.h")), Some("x.h"));
        let no_root = set(&["include/x.h", "src/x.h", "lib/deep/x.h"]);
        assert_eq!(no_root.resolve("a/b.cc", &q("x.h")), Some("include/x.h"));
        let no_include = set(&["src/x.h", "lib/deep/x.h"]);
        assert_eq!(no_include.resolve("a/b.cc", &q("x.h")), Some("src/x.h"));
        // Then a unique suffix match.
        let only_deep = set(&["lib/deep/x.h", "a/b.cc"]);
        assert_eq!(only_deep.resolve("a/b.cc", &q("x.h")), Some("lib/deep/x.h"));
    }

    #[test]
    fn an_include_with_a_directory_resolves_against_each_root() {
        let all = set(&["db/db.h", "include/db/db.h"]);
        assert_eq!(all.resolve("main.cc", &q("db/db.h")), Some("db/db.h"));
        let inc = set(&["include/db/db.h"]);
        assert_eq!(
            inc.resolve("main.cc", &q("db/db.h")),
            Some("include/db/db.h")
        );
        // Beside the including file: `src/net/conn.cc` writes "wire.h".
        let beside = set(&["src/net/wire.h", "other/wire.h"]);
        assert_eq!(
            beside.resolve("src/net/conn.cc", &q("wire.h")),
            Some("src/net/wire.h")
        );
    }

    #[test]
    fn an_ambiguous_suffix_resolves_to_nothing() {
        let two = set(&["a/util/x.h", "b/util/x.h"]);
        assert_eq!(two.resolve("main.cc", &q("util/x.h")), None);
        assert_eq!(two.resolve("main.cc", &a("util/x.h")), None);
        // A prefix that is not at a directory boundary is no match.
        let boundary = set(&["lib/myutil/x.h"]);
        assert_eq!(boundary.resolve("main.cc", &q("util/x.h")), None);
    }

    #[test]
    fn a_missing_file_resolves_to_nothing() {
        let s = set(&["a.h"]);
        assert_eq!(s.resolve("main.cc", &q("nope.h")), None);
        assert_eq!(s.resolve("main.cc", &a("nope/nope.h")), None);
        assert_eq!(s.resolve("main.cc", "\"\""), None);
        assert_eq!(s.resolve("main.cc", "garbage"), None);
    }

    #[test]
    fn a_bare_angle_include_is_a_system_header_not_a_repository_file() {
        // The measured failure: `<time.h>` taken for `absl/time/time.h`.
        let s = set(&["x/time.h", "include/mylib.h"]);
        assert_eq!(s.resolve("main.cc", &a("time.h")), None);
        assert_eq!(s.resolve("main.cc", &q("time.h")), Some("x/time.h"));
        assert_eq!(s.resolve("main.cc", &a("x/time.h")), Some("x/time.h"));
        // A bare angle include still finds a file at a root.
        assert_eq!(s.resolve("main.cc", &a("mylib.h")), Some("include/mylib.h"));
    }

    #[test]
    fn an_angle_include_is_not_looked_for_beside_the_including_file() {
        let s = set(&["a/local.h", "a/b.cc"]);
        assert_eq!(s.resolve("a/b.cc", &q("local.h")), Some("a/local.h"));
        assert_eq!(s.resolve("a/b.cc", &a("local.h")), None);
    }

    #[test]
    fn dot_segments_are_followed_and_cannot_leave_the_repository() {
        let s = set(&["a/x.h", "b/y.h", "z.h"]);
        assert_eq!(s.resolve("a/c.cc", &q("../b/y.h")), Some("b/y.h"));
        assert_eq!(s.resolve("a/c.cc", &q("./x.h")), Some("a/x.h"));
        assert_eq!(s.resolve("a/c.cc", &q("../../z.h")), None);
    }

    fn imports(specs: &[&str]) -> FileImports {
        FileImports {
            imports: specs
                .iter()
                .map(|s| ImportRef {
                    specifier: (*s).to_string(),
                    imported_name: "*".to_string(),
                })
                .collect(),
            ..FileImports::default()
        }
    }

    fn graph(files: &[(&str, &[&str])]) -> IncludeGraph {
        let map = files
            .iter()
            .map(|(f, specs)| ((*f).to_string(), imports(specs)))
            .collect();
        IncludeGraph::build(&map)
    }

    #[test]
    fn direct_includes_are_sorted_distinct_and_never_the_file_itself() {
        let g = graph(&[
            (
                "m.cc",
                &["\"b.h\"", "\"a.h\"", "<a.h>", "\"m.cc\"", "<vector>"],
            ),
            ("a.h", &[]),
            ("b.h", &[]),
        ]);
        assert_eq!(g.includes("m.cc"), ["a.h", "b.h"]);
        assert_eq!(g.unresolved, [("m.cc".to_string(), "<vector>".to_string())]);
    }

    #[test]
    fn reach_follows_a_chain() {
        let g = graph(&[
            ("a.cc", &["\"b.h\""]),
            ("b.h", &["\"c.h\""]),
            ("c.h", &["\"d.h\""]),
            ("d.h", &[]),
        ]);
        assert_eq!(g.reach("a.cc"), ["b.h", "c.h", "d.h"]);
        assert_eq!(g.reach("c.h"), ["d.h"]);
        assert!(g.reach("d.h").is_empty());
    }

    #[test]
    fn reach_in_a_diamond_lists_each_file_once() {
        let g = graph(&[
            ("a.cc", &["\"b.h\"", "\"c.h\""]),
            ("b.h", &["\"d.h\""]),
            ("c.h", &["\"d.h\""]),
            ("d.h", &[]),
        ]);
        assert_eq!(g.reach("a.cc"), ["b.h", "c.h", "d.h"]);
    }

    #[test]
    fn reach_ends_in_a_cycle_and_leaves_the_file_out() {
        let g = graph(&[
            ("a.h", &["\"b.h\""]),
            ("b.h", &["\"c.h\""]),
            ("c.h", &["\"a.h\"", "\"b.h\""]),
        ]);
        assert_eq!(g.reach("a.h"), ["b.h", "c.h"]);
        assert_eq!(g.reach("b.h"), ["a.h", "c.h"]);
    }

    #[test]
    fn reach_of_an_unknown_file_is_empty() {
        let g = graph(&[("a.h", &[])]);
        assert!(g.reach("nope.h").is_empty());
        assert!(g.includes("nope.h").is_empty());
    }
}
