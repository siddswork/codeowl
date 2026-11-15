//! Namespaces in C++ ids (step 2b, part 0): two symbols of one file can end
//! in the same name (`big::FromHost`, `little::FromHost`). A spec finds each
//! section by its heading, so the heading must carry the namespaces or the
//! second section would be read from the first's block.

use codeowl::index::RepoIndex;
use codeowl::spec::{read_file_spec, submit};
use std::path::PathBuf;

struct Repo(PathBuf);

impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn prose(summary: &str, behavior: &str) -> String {
    format!("### Summary\n{summary}\n### Behavior\n{behavior}\n")
}

#[test]
fn two_same_named_symbols_of_different_namespaces_keep_their_own_prose() {
    let dir = std::env::temp_dir().join(format!("codeowl-cpp-sections-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let _guard = Repo(dir.clone());
    std::fs::write(
        dir.join("endian.h"),
        "namespace big { int FromHost(int x); }\nnamespace little { int FromHost(int x); }\n",
    )
    .unwrap();

    let (_index, graph, _) = RepoIndex::open(&dir).unwrap();
    submit(
        &graph,
        &dir,
        "endian.h::big::FromHost",
        &prose(
            "Converts a host integer to big endian order.",
            "Swaps the bytes on a little endian host and returns the value.",
        ),
    )
    .unwrap();
    submit(
        &graph,
        &dir,
        "endian.h::little::FromHost",
        &prose(
            "Converts a host integer to little endian order.",
            "Returns the value unchanged on a little endian host.",
        ),
    )
    .unwrap();

    let spec = read_file_spec(&dir, "endian.h").unwrap().expect("a spec");
    let text = std::fs::read_to_string(dir.join("docs/specs/endian.h.md")).unwrap();
    assert!(text.contains("## `big::FromHost`"), "{text}");
    assert!(text.contains("## `little::FromHost`"), "{text}");
    // Read back through the parser: each symbol gets its own prose, not the
    // first heading's.
    let read = |id: &str| {
        spec.sections
            .iter()
            .find(|(sid, _)| sid == id)
            .unwrap_or_else(|| panic!("no parsed section for {id}"))
            .1
            .summary
            .clone()
    };
    assert!(read("endian.h::big::FromHost").contains("big endian"));
    assert!(read("endian.h::little::FromHost").contains("little endian"));
    // And the rendered text carries each in its own section.
    let section = |id: &str| {
        let after = text
            .split(&format!("## `{id}`"))
            .nth(1)
            .unwrap_or_else(|| panic!("no section {id}"));
        after.split("\n## ").next().unwrap().to_string()
    };
    assert!(section("big::FromHost").contains("big endian order"));
    assert!(section("little::FromHost").contains("little endian order"));
}
