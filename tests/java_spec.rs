//! Integration (M16): a Java file goes through extraction → `Graph` → the
//! spec `next_task`/`submit` loop → `render`, and comes out with the class
//! as one `## `Type`` section — its methods and a nested `enum` folded in,
//! never offered as their own top-level tasks. Plus: a two-file package
//! resolves its same-package (no-`import`) reference edge end to end through
//! `RepoIndex`. The end-to-end version of `java.rs`'s unit tests, on the
//! path `/codeowl generate` drives.

use codeowl::graph::{FileExtraction, Graph};
use codeowl::hash::hash_text;
use codeowl::index::RepoIndex;
use codeowl::spec::{SpecTask, next_task, read_file_spec, render, submit};

const PATH: &str = "src/main/java/org/apache/commons/lang3/StringUtils.java";

const SRC: &str = "\
package org.apache.commons.lang3;\n\
\n\
/**\n\
 * Operations on {@link String} that are null safe.\n\
 */\n\
public class StringUtils {\n\
\n\
    public static boolean isEmpty(final CharSequence cs) {\n\
        return len(cs) == 0;\n\
    }\n\
\n\
    public static boolean isBlank(final CharSequence cs) {\n\
        return isEmpty(cs);\n\
    }\n\
\n\
    private static int len(final CharSequence cs) {\n\
        return cs == null ? 0 : cs.length();\n\
    }\n\
\n\
    /** Which side a pad lands on. */\n\
    public enum Pad {\n\
        LEFT, RIGHT;\n\
    }\n\
}\n";

fn java_graph(rel_path: &str, src: &str) -> Graph {
    Graph::build(vec![FileExtraction {
        rel_path: rel_path.to_string(),
        source_hash: hash_text(src),
        symbols: codeowl::java::extract_file(src, rel_path),
    }])
}

#[test]
fn a_class_renders_as_one_section_with_methods_and_a_nested_enum_folded_in() {
    let dir = std::env::temp_dir().join(format!("codeowl-java-spec-{}-cls", std::process::id()));
    std::fs::create_dir_all(dir.join("src/main/java/org/apache/commons/lang3")).unwrap();
    std::fs::write(dir.join(PATH), SRC).unwrap();

    let graph = java_graph(PATH, SRC);
    let file_id = graph.find(PATH).unwrap();

    // The `private` helper is a member, not exported — it never becomes a
    // reference-edge target, but it stays in the graph.
    let len = graph
        .get_symbol(graph.find(&format!("{PATH}::StringUtils::len")).unwrap())
        .unwrap();
    assert!(!len.is_exported, "a private method is not exported");

    // The generate loop offers exactly one symbol task — the class itself.
    // isEmpty / isBlank / len are members; Pad is a nested type. The prose
    // below is deliberately >= 4 words per section so it clears
    // `prose_smells` and the task isn't re-offered (the cap is a guard
    // against that turning into a hang).
    let mut offered = Vec::new();
    for _ in 0..20 {
        let Some(task) = next_task(&graph, &dir, file_id).unwrap() else {
            break;
        };
        match task {
            SpecTask::Symbol { id, docstring, .. } => {
                if id == format!("{PATH}::StringUtils") {
                    assert_eq!(
                        docstring.as_deref(),
                        Some("Operations on {@link String} that are null safe."),
                        "the Javadoc is carried into the task"
                    );
                }
                offered.push(id.clone());
                submit(
                    &graph,
                    &dir,
                    &id,
                    "### Summary\nNull-safe checks over a possibly-null CharSequence argument.\n\
                     ### Behavior\nEach public check funnels its length work through the \
                     private len helper, which treats null as length zero.\n",
                )
                .unwrap();
            }
            SpecTask::File { id, .. } => {
                submit(
                    &graph,
                    &dir,
                    &id,
                    "Null-safe string predicates plus a nested padding-side enum.",
                )
                .unwrap();
            }
        }
    }

    assert_eq!(
        offered,
        vec![format!("{PATH}::StringUtils")],
        "only the top-level class is offered — not its methods or the nested enum"
    );

    let spec = read_file_spec(&dir, PATH).unwrap().unwrap();
    let rendered = render(&graph, &dir, file_id, &spec);

    assert!(
        rendered.contains("## `StringUtils`"),
        "class section missing"
    );
    assert!(
        !rendered.contains("## `Pad`") && !rendered.contains("## `StringUtils.Pad`"),
        "the nested enum must not render as its own section:\n{rendered}"
    );
    assert_eq!(
        rendered.matches("\n## `").count(),
        1,
        "exactly one top-level section — the class:\n{rendered}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_same_package_reference_resolves_end_to_end_through_repo_index() {
    let dir = std::env::temp_dir().join(format!("codeowl-java-spec-{}-idx", std::process::id()));
    let pkg = dir.join("src/main/java/com/foo");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(
        pkg.join("A.java"),
        "package com.foo;\npublic class A {\n    int use() { return B.value(); }\n}\n",
    )
    .unwrap();
    std::fs::write(
        pkg.join("B.java"),
        "package com.foo;\npublic class B {\n    static int value() { return 1; }\n}\n",
    )
    .unwrap();

    // `open` picks the Java pack via `detect`, walks, and rebuilds.
    let (_index, graph, _catch_up) = RepoIndex::open(&dir).unwrap();

    let edge = graph
        .imports()
        .iter()
        .find(|e| e.from_file == "src/main/java/com/foo/A.java" && e.imported_name == "B")
        .expect("A references B in the same package with no import");
    assert_eq!(
        graph.string_id(edge.target.expect("the same-package edge resolves")),
        "src/main/java/com/foo/B.java::B"
    );
    // B never names A -> no phantom reverse edge.
    assert!(
        !graph
            .imports()
            .iter()
            .any(|e| e.from_file.ends_with("B.java") && e.imported_name == "A"),
        "the scan is directional"
    );

    std::fs::remove_dir_all(&dir).ok();
}
