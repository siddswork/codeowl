//! The `/codeowl-generate` command exists in two copies, the Claude Code
//! slash command and its VS Code / Copilot port, and an agent executes
//! whichever it is given. A paged file task only works if the agent follows
//! its cursor, so both copies must say so, in the same words. These checks
//! keep the copies from drifting on the parts that matter.

use std::path::Path;

fn read(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("setup")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// The text with every run of whitespace collapsed to one space, so a phrase
/// that happens to wrap across lines still matches.
fn flat(name: &str) -> String {
    read(name).split_whitespace().collect::<Vec<_>>().join(" ")
}

const BOTH: [&str; 2] = ["codeowl-generate.md", "codeowl-generate.prompt.md"];

/// What every copy must tell the agent about a paged file task and about
/// writing from a stand-in for the file.
const REQUIRED: [(&str, &str); 8] = [
    ("`next_cursor`", "the field that says another page follows"),
    ("`cursor`", "the argument that fetches it"),
    (
        "same `target`",
        "the cursor goes with the target it came from",
    ),
    ("until the last page", "read every page before writing"),
    (
        "restart from page 1",
        "what to do when the cursor is refused",
    ),
    ("`input_kind`", "what the pages hold"),
    (
        "cover EVERY major part of the file",
        "the measured instruction for summaries and outlines",
    ),
    (
        "counts as one generation",
        "a paged file task is one unit of a --budget",
    ),
];

#[test]
fn both_copies_tell_the_agent_how_to_read_a_paged_file_task() {
    for name in BOTH {
        let text = flat(name);
        for (phrase, why) in REQUIRED {
            assert!(
                text.contains(phrase),
                "{name} must contain {phrase:?} ({why})"
            );
        }
    }
}

#[test]
fn both_copies_tell_the_writer_not_to_describe_its_input_as_cut_off() {
    for name in BOTH {
        let text = flat(name);
        assert!(
            text.contains("never describe the input as cut off"),
            "{name} must say not to describe the input as cut off"
        );
    }
}

#[test]
fn the_read_before_writing_section_is_word_for_word_the_same_in_both_copies() {
    let section = |text: &str| {
        let start = text.find("**Read before writing").expect("section start");
        let end = text.find("## Loop").expect("section end");
        text[start..end].to_string()
    };
    assert_eq!(
        section(&read(BOTH[0])),
        section(&read(BOTH[1])),
        "the two copies of the command have drifted apart"
    );
}
