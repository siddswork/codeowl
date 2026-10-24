//! Which documents the granularity rules say should exist, and where each lives: the mirrored `docs/specs/` paths, which files and directories are spec-bearing, and the generated-source summary.

use std::path::{Path, PathBuf};

use crate::graph::{Graph, SymbolId};
use crate::lang::FileRole;
use crate::symbol::SymbolKind;

/// A repo-relative path's [`FileRole`] under the stack that built `graph`
/// — `RustStack` calls `tests/` a test root, `TypeScriptNextStack` uses
/// the JS conventions. Goes through the graph's pack, not the TypeScript
/// rules directly, so prioritisation isn't hard-wired to one stack.
fn classify_in(graph: &Graph, path: &str) -> FileRole {
    crate::stack::file_role(graph, path)
}

/// Where a file's spec lives, mirrored under `docs/specs/` — never strips
/// the source extension: `lib/utils.ts` -> `docs/specs/lib/utils.ts.md`.
pub fn spec_path(root: &Path, source_path: &str) -> PathBuf {
    root.join("docs")
        .join("specs")
        .join(spec_relative_path(source_path))
}

/// A source file's spec, relative to `docs/specs/`: its path plus `.md`. The
/// one place that says how a spec is named; [`spec_path`] and [`spec_link`]
/// both follow it.
fn spec_relative_path(source_path: &str) -> String {
    format!("{source_path}.md")
}

/// The file part of a symbol id: everything before `::`, or the whole id when
/// it already names a file.
pub fn symbol_file(id: &str) -> &str {
    id.split("::").next().unwrap_or(id)
}

/// A link, relative to the spec of `from_file`, to the spec of `target_file`.
/// Both are repo-relative source paths, and their specs sit under
/// `docs/specs/` as [`spec_relative_path`] says, so the link is the path
/// between those two places. Relative, not rooted at `/`: on GitHub a leading
/// `/` means the repository root, not `docs/specs/`. Characters that would
/// end a markdown link destination (or start a fragment or query) are
/// percent-escaped.
pub fn spec_link(from_file: &str, target_file: &str) -> String {
    let mut from: Vec<&str> = from_file.split('/').collect();
    from.pop();
    let target_path = spec_relative_path(target_file);
    let target: Vec<&str> = target_path.split('/').collect();
    let target_dirs = &target[..target.len() - 1];
    let common = from
        .iter()
        .zip(target_dirs)
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    parts.extend(&target[common..]);
    let mut link = String::new();
    for c in parts.join("/").chars() {
        match c {
            '%' => link.push_str("%25"),
            ' ' => link.push_str("%20"),
            '(' => link.push_str("%28"),
            ')' => link.push_str("%29"),
            '#' => link.push_str("%23"),
            '?' => link.push_str("%3F"),
            '<' => link.push_str("%3C"),
            '>' => link.push_str("%3E"),
            '\\' => link.push_str("%5C"),
            c => link.push(c),
        }
    }
    link
}

/// Whether a repo-relative path is test code — an `e2e/` tree, a
/// `__tests__/` directory, a `cypress/`/`playwright/` tree, or a
/// `.test.`/`.spec.` file. Test code stays in the graph (so `get_callers`
/// still shows "used by these tests"), but its specs sort to the very
/// bottom of a `--all` run and it's never treated as a product module —
/// documenting the test harness is rarely the point of a spec corpus, and
/// whenever it is, it's safe to leave for last. The path is classified as
/// given: a `rollup:`/`feature:` coverage id is not stripped here.
pub fn is_test_path(graph: &Graph, path: &str) -> bool {
    matches!(classify_in(graph, path), FileRole::Test)
}

/// A file is spec-bearing iff it declares at least one `Callable` or
/// `Container` among its top-level symbols — barrel files (zero top-level
/// declarations at all — see `barrel_file_is_not_spec_bearing`) and
/// const-only/metadata-only boilerplate get no document (see
/// `ARCHITECTURE.md`'s granularity rules).
///
/// **Deliberately not gated on `is_exported` (design correction,
/// 2026-09-25).** An earlier version additionally required the top-level
/// symbol to be exported, reasoning it would also catch barrel files —
/// but a real barrel file already has zero top-level `Callable`/
/// `Container` symbols at all (a re-export isn't a declaration), so that
/// gate caught nothing a barrel file needed. Its only actual effect was
/// making a file with substantial, real, *entirely private* code —
/// `tests/*.rs`-style integration-test files with no `pub` anywhere are
/// the common real case — permanently invisible to `get_spec_coverage`:
/// not merely deprioritized (`FileRole::Test` already exists for that),
/// literally never offered a spec, ever. A top-level declaration being
/// private only means nothing *outside this file* can import it; it says
/// nothing about whether the file itself is worth documenting.
///
/// **`FileRole::Generated` is never spec-bearing, regardless of what it
/// exports (M19).** A build-generated interface (an OpenAPI-codegen'd
/// JAX-RS resource, a `.proto` stub) can export plenty of real methods —
/// confirmed against a real `mvn compile` run, `HeroesResource`
/// alone has 8 — but it's read-only structural reference, never the
/// hand-written code a spec describes, per the "only spec the
/// hand-written code" principle `ARCHITECTURE.md` open question 11
/// states. Checked first so it short-circuits before the symbol scan
/// below runs at all.
pub fn file_is_spec_bearing(graph: &Graph, file_id: SymbolId) -> bool {
    if matches!(
        classify_in(graph, graph.string_id(file_id)),
        FileRole::Generated
    ) {
        return false;
    }
    graph.children_ids(file_id).iter().any(|&id| {
        graph
            .get_symbol(id)
            .is_some_and(|s| matches!(s.kind, SymbolKind::Callable | SymbolKind::Container))
    })
}

/// How many of `graph`'s files sit under the active pack's declared
/// generated-source directories (M19), for `get_spec_coverage` to surface
/// to the driving LLM. `None` for any pack whose `generated_source_dirs()`
/// is empty (every pack but Java today) — the concept doesn't apply, so
/// there's nothing honest to report, not a zero. `found: 0` for a pack
/// that *does* declare them is the actionable signal: this could be a
/// Quarkus-shaped service with build-generated entry points, and none
/// were found — has `mvn compile` (or equivalent) been run locally? See
/// `setup/codeowl-generate.md` for why `mvn generate-sources` alone
/// isn't enough.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedSourcesSummary {
    pub checked_dirs: Vec<String>,
    pub found: usize,
}

pub fn generated_sources_summary(graph: &Graph) -> Option<GeneratedSourcesSummary> {
    let pack = crate::stack::for_graph(graph);
    let dirs = pack.generated_source_dirs();
    if dirs.is_empty() {
        return None;
    }
    let found = graph
        .files()
        .filter(|f| matches!(classify_in(graph, &f.id), FileRole::Generated))
        .count();
    Some(GeneratedSourcesSummary {
        checked_dirs: dirs.iter().map(|d| d.to_string()).collect(),
        found,
    })
}

/// The top-level symbols a file spec gives their own section — both
/// exported and unexported `Callable`s/`Container`s (the point is
/// describing how the file works, not only its public API). Top-level
/// `Value`s (a `const`) get no subsection of their own; a container's
/// members are covered inside the container's own section, not separately,
/// matching how M2 already treats a class as one containment unit.
pub(crate) fn spec_bearing_children(graph: &Graph, file_id: SymbolId) -> Vec<SymbolId> {
    graph
        .children_ids(file_id)
        .iter()
        .copied()
        .filter(|&id| {
            graph
                .get_symbol(id)
                .is_some_and(|s| matches!(s.kind, SymbolKind::Callable | SymbolKind::Container))
        })
        .collect()
}

/// A directory gets a rollup (`_index.md`) iff at least two of its
/// *immediate* files are themselves spec-bearing — Next.js route trees are
/// full of single-file directories (`app/api/<route>/route.ts`), and an
/// `_index.md` for each of those would be pure noise. `dir_path` is a
/// repo-relative directory path with no trailing slash (`""` for the repo
/// root, though see `next_rollup_task`'s guard on that case).
pub fn directory_is_spec_bearing(graph: &Graph, dir_path: &str) -> bool {
    spec_bearing_files_in(graph, dir_path).len() >= 2
}

/// Every file directly in `dir_path` (repo-relative, no trailing slash) —
/// spec-bearing or not. `dir_path` compares against each file's own parent
/// path, so this is one directory level, not a recursive subtree walk;
/// nested subdirectories get their own rollup instead of being folded into
/// this one.
pub(super) fn files_in(graph: &Graph, dir_path: &str) -> Vec<SymbolId> {
    let mut files: Vec<SymbolId> = graph
        .files()
        .filter(|f| {
            Path::new(&f.id)
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                == Some(dir_path.to_string())
        })
        .filter_map(|f| graph.find(&f.id))
        .collect();
    files.sort_by_key(|&id| graph.string_id(id).to_string());
    files
}

/// `dir_path`'s own spec-bearing files, in a stable (path-sorted) order —
/// unlike a file's symbols, a directory's files have no declaration order
/// to preserve.
pub(super) fn spec_bearing_files_in(graph: &Graph, dir_path: &str) -> Vec<SymbolId> {
    files_in(graph, dir_path)
        .into_iter()
        .filter(|&id| file_is_spec_bearing(graph, id))
        .collect()
}

/// The short name a section heading uses — the part of a top-level
/// symbol's stable id after `file::`.
pub(super) fn short_name(id: &str) -> &str {
    id.rsplit("::").next().unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::build_graph_from_sources;

    #[test]
    fn the_file_part_of_a_symbol_id_is_everything_before_the_double_colon() {
        assert_eq!(symbol_file("src/graph.rs::Graph"), "src/graph.rs");
        assert_eq!(symbol_file("src/graph.rs"), "src/graph.rs");
        assert_eq!(symbol_file("a.ts::f"), "a.ts");
    }

    #[test]
    fn a_link_to_a_spec_is_relative_to_the_spec_that_holds_it() {
        // Relative, not `/`-rooted: on GitHub a leading `/` means the repo
        // root, not `docs/specs/`, so a rooted link would not open.
        assert_eq!(
            spec_link("src/spec/format.rs", "src/graph.rs"),
            "../graph.rs.md"
        );
        assert_eq!(
            spec_link("src/hash.rs", "src/spec/format.rs"),
            "spec/format.rs.md"
        );
        assert_eq!(
            spec_link("src/people.rs", "src/greeter.rs"),
            "greeter.rs.md"
        );
        assert_eq!(spec_link("a.ts", "lib/b.ts"), "lib/b.ts.md");
        assert_eq!(spec_link("lib/a.ts", "b.ts"), "../b.ts.md");
        assert_eq!(spec_link("src/a/b/c.rs", "src/x/y.rs"), "../../x/y.rs.md");
        // The link is the path between the two places `spec_path` gives.
        let root = Path::new("/r");
        let from = spec_path(root, "src/spec/format.rs");
        let linked = from
            .parent()
            .unwrap()
            .join(spec_link("src/spec/format.rs", "src/graph.rs"));
        assert_eq!(
            normalise(&linked),
            normalise(&spec_path(root, "src/graph.rs"))
        );
    }

    /// `path` with its `..` components folded in, without touching the disk.
    fn normalise(path: &Path) -> PathBuf {
        let mut out = PathBuf::new();
        for part in path.components() {
            if part.as_os_str() == ".." {
                out.pop();
            } else {
                out.push(part);
            }
        }
        out
    }

    #[test]
    fn characters_that_would_end_a_link_destination_are_escaped() {
        assert_eq!(
            spec_link("a.ts", "my dir/x (1).ts"),
            "my%20dir/x%20%281%29.ts.md"
        );
        assert_eq!(spec_link("a.ts", "lib/c#/util.ts"), "lib/c%23/util.ts.md");
        assert_eq!(spec_link("a.ts", "what?/x.ts"), "what%3F/x.ts.md");
        assert_eq!(spec_link("a.ts", "<a>/x.ts"), "%3Ca%3E/x.ts.md");
        assert_eq!(spec_link("a.ts", "back\\slash.ts"), "back%5Cslash.ts.md");
        assert_eq!(spec_link("a.ts", "100%/x.ts"), "100%25/x.ts.md");
        // Square brackets are fine in a destination and are kept.
        assert_eq!(
            spec_link("a.ts", "app/api/[id]/route.ts"),
            "app/api/[id]/route.ts.md"
        );
    }

    #[test]
    fn barrel_file_is_not_spec_bearing() {
        let graph = build_graph_from_sources(&[(
            "a.ts",
            "export { Foo } from './foo';\nexport * from './bar';\n",
        )]);
        let file_id = graph.find("a.ts").unwrap();
        assert!(!file_is_spec_bearing(&graph, file_id));
    }

    #[test]
    fn file_with_exported_function_is_spec_bearing() {
        let graph = build_graph_from_sources(&[("a.ts", "export function double(x: number) {}\n")]);
        let file_id = graph.find("a.ts").unwrap();
        assert!(file_is_spec_bearing(&graph, file_id));
    }

    #[test]
    fn a_private_top_level_function_is_still_spec_bearing() {
        // Design correction, 2026-09-25: `file_is_spec_bearing` used to
        // additionally require the top-level symbol to be *exported*
        // (`is_exported`). A real barrel file is already excluded
        // independently -- it declares zero top-level symbols at all,
        // see `barrel_file_is_not_spec_bearing` -- so the export check's
        // only actual effect was hiding a file whose real, substantial
        // code happens to be entirely private, like a `tests/*.rs`
        // integration-test file with no `pub` anywhere: structurally in
        // the graph, permanently invisible to `get_spec_coverage`, with
        // literally no spec obtainable for it, ever. A top-level
        // function being private only means nothing *outside this file*
        // can import it -- it says nothing about whether the file is
        // worth documenting.
        let graph = build_graph_from_sources(&[("a.ts", "function helper(): void {}\n")]);
        let file_id = graph.find("a.ts").unwrap();
        assert!(file_is_spec_bearing(&graph, file_id));
    }

    #[test]
    fn a_generated_file_is_never_spec_bearing_even_with_exported_symbols() {
        // M19: FileRole::Generated must exclude a file from spec
        // generation regardless of what it exports -- a real
        // build-generated JAX-RS interface (quarkus-openapi-generator-
        // server output) has plenty of exported methods but must stay
        // reference-only, per the "only spec the hand-written code"
        // principle. `classify_in` reads the graph's own `pack_name`, so
        // this sets it to "java" directly rather than re-deriving it
        // through `stack::detect` on a throwaway fixture repo.
        let path = "target/generated-sources/foo/HeroesResource.java";
        let mut graph =
            build_graph_from_sources(&[(path, "export function getAllHeroes(): void {}\n")]);
        graph.set_pack_name("java");
        let file_id = graph.find(path).unwrap();
        assert!(!file_is_spec_bearing(&graph, file_id));
    }

    #[test]
    fn generated_sources_summary_counts_java_generated_files_only() {
        let mut graph = build_graph_from_sources(&[
            (
                "src/main/java/org/acme/App.java",
                "export function ignored(): void {}\n",
            ),
            (
                "target/generated-sources/foo/HeroesResource.java",
                "export function getAllHeroes(): void {}\n",
            ),
        ]);
        graph.set_pack_name("java");

        let summary = generated_sources_summary(&graph).expect("Java declares generated dirs");
        assert_eq!(
            summary.checked_dirs,
            vec!["target/generated-sources", "build/generated"]
        );
        assert_eq!(
            summary.found, 1,
            "only the file under a generated dir counts"
        );
    }

    #[test]
    fn generated_sources_summary_is_none_for_a_pack_with_no_generated_dir_convention() {
        let graph = build_graph_from_sources(&[("a.ts", "export function f(): void {}\n")]);
        assert_eq!(generated_sources_summary(&graph), None);
    }

    #[test]
    fn const_only_file_is_not_spec_bearing() {
        // Excluded by *kind* (`Value`, never `Callable`/`Container`), not
        // by export status -- renamed from `non_exported_only_file_...`
        // to stop reading as the export-status case, which is now
        // `a_private_top_level_function_is_still_spec_bearing` above and
        // asserts the opposite.
        let graph = build_graph_from_sources(&[("a.ts", "const helper = 1;\n")]);
        let file_id = graph.find("a.ts").unwrap();
        assert!(!file_is_spec_bearing(&graph, file_id));
    }

    #[test]
    fn single_file_directory_is_not_spec_bearing() {
        let graph = build_graph_from_sources(&[(
            "app/api/submit/route.ts",
            "export function GET(): void {}\n",
        )]);
        assert!(!directory_is_spec_bearing(&graph, "app/api/submit"));
    }

    #[test]
    fn directory_with_two_spec_bearing_files_is_spec_bearing() {
        let graph = build_graph_from_sources(&[
            ("lib/one.ts", "export function one(): void {}\n"),
            ("lib/two.ts", "export function two(): void {}\n"),
        ]);
        assert!(directory_is_spec_bearing(&graph, "lib"));
    }

    #[test]
    fn directory_with_one_spec_bearing_and_one_barrel_file_is_not_spec_bearing() {
        let graph = build_graph_from_sources(&[
            ("lib/one.ts", "export function one(): void {}\n"),
            ("lib/index.ts", "export { one } from './one';\n"),
        ]);
        assert!(!directory_is_spec_bearing(&graph, "lib"));
    }

    #[test]
    fn is_test_path_examples() {
        // An empty graph -> pack_name "" -> the TypeScript pack's classify.
        let g = crate::fixtures::build_graph_from_sources(&[]);
        assert!(is_test_path(&g, "e2e/helpers/api-client.ts"));
        assert!(is_test_path(&g, "src/components/__tests__/button.ts"));
        assert!(is_test_path(&g, "lib/utils.test.ts"));
        assert!(is_test_path(&g, "app/page.spec.tsx"));
        assert!(is_test_path(&g, "rollup:e2e/helpers"));
        assert!(!is_test_path(&g, "lib/utils.ts"));
        assert!(!is_test_path(&g, "app/api/attest/route.ts")); // "test" substring, not a test file
    }
}
