//! Deterministic quality checks on LLM-written spec prose. They look only at the text, never at the hashes.

use std::path::Path;

use anyhow::{Result, bail};

use super::format::{FileSpec, dependency_lines, extract_section};
use crate::graph::{Graph, SymbolId};

/// A deterministic, non-LLM check for prose that looks like a stub or a
/// cop-out rather than real content — independent of hash-based
/// staleness. Catches the two failure modes `CLAUDE.md`/the generate
/// prompt already call out by name: a "see the source" cop-out that
/// defeats the whole point of the document, and prose so short it can
/// only be a template placeholder (found, concretely, in two real
/// pre-M5 specs during a quality audit: `"<name> does its job."` /
/// `"See source."`). Deliberately not exhaustive or clever — a small,
/// named denylist plus a word-count floor, both easy to reason about and
/// cheap to extend if a new failure mode turns up.
pub fn prose_smells(text: &str) -> Vec<String> {
    const COP_OUT_PHRASES: &[&str] = &[
        "see the source",
        "see source",
        "see the route handler",
        "see the handler",
        "see the file for details",
        "see the code for details",
        "refer to the source",
        "check the implementation",
        "check the source",
    ];
    let mut smells = Vec::new();
    let lower = text.to_lowercase();
    if COP_OUT_PHRASES.iter().any(|phrase| lower.contains(phrase)) {
        smells.push("cop_out_phrase".to_string());
    }
    if text.split_whitespace().count() < 4 {
        smells.push("suspiciously_short".to_string());
    }
    // A generator's closing tag left in the text (an earlier run wrote 63
    // of them into five stored specs). Real prose never contains one.
    if text.contains("</content>") {
        smells.push("stray_markup".to_string());
    }
    // A code span opened and never closed: markdown shows the text after it
    // wrongly, and in practice it means the prose is broken (a fragment that
    // began mid-sentence with a stray backtick sat in a spec marked current).
    if has_unclosed_code_span(text) {
        smells.push("unbalanced_code_span".to_string());
    }
    smells
}

/// Whether `text` has a run of backticks that opens a code span and never gets
/// the matching closer. CommonMark's rule: a run of N backticks opens a span
/// only if a later run of exactly N backticks in the same block closes it;
/// a run with no partner is left as literal text. A backslash outside a span
/// escapes the next character (so `\`` is a literal backtick), but inside a span
/// it is plain text, which is why a span written as `` `\`name\`` `` comes out
/// broken. A block ends at a blank line or where something that can interrupt
/// a paragraph starts (see [`interrupts_a_paragraph`]); fenced and indented
/// code blocks are not prose and are cut out.
///
/// The rule is deliberately stricter than the renderer: markdown shows a lone
/// backtick as plain text, but in a spec it is almost always a mistake (a
/// fragment cut mid-sentence, a span that lost its closer), and the cost of
/// being refused is one rewording (a literal backtick can be written inside a
/// double-backtick span). Where the check has to guess about markdown structure
/// it leans toward not flagging, because refusing good prose is the worse error.
fn has_unclosed_code_span(text: &str) -> bool {
    let mut fence: Option<(char, usize)> = None;
    let mut block = String::new();
    for line in text.lines() {
        if let Some(open) = fence {
            if closes_fence(line, open) {
                fence = None;
            }
            continue;
        }
        if let Some(open) = opens_fence(line) {
            if span_is_unclosed(&block) {
                return true;
            }
            block.clear();
            fence = Some(open);
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            if span_is_unclosed(&block) {
                return true;
            }
            block.clear();
            continue;
        }
        // Four spaces of indent at the start of a block make it code.
        if block.is_empty() && line.len() - trimmed.len() >= 4 {
            continue;
        }
        if !block.is_empty() && interrupts_a_paragraph(trimmed) {
            if span_is_unclosed(&block) {
                return true;
            }
            block.clear();
        }
        block.push_str(line);
        block.push('\n');
    }
    span_is_unclosed(&block)
}

/// The fence a line opens, as (character, length): at least three backticks or
/// tildes after at most three spaces of indent. A backtick fence cannot have a
/// backtick in its info string, which is what keeps a line that starts with an
/// inline triple-backtick span from being taken for one.
fn opens_fence(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start();
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let marker = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let length = trimmed.chars().take_while(|&c| c == marker).count();
    if length < 3 || (marker == '`' && trimmed[length..].contains('`')) {
        return None;
    }
    Some((marker, length))
}

/// Whether `line` closes the fence `(marker, length)`: the same character, at
/// least as many of them, and nothing but whitespace after.
fn closes_fence(line: &str, (marker, length): (char, usize)) -> bool {
    let trimmed = line.trim_start();
    if line.len() - trimmed.len() > 3 {
        return false;
    }
    let run = trimmed.chars().take_while(|&c| c == marker).count();
    run >= length && trimmed[run..].trim().is_empty()
}

/// Whether a line can interrupt a paragraph and start a new block: a bullet, a
/// quote, a heading (`#` to `######` then a space), or an ordered item that
/// starts at 1. Anything else, such as a wrapped line that begins `2. ` or
/// `#[derive(...)]`, is only a continuation of the sentence.
fn interrupts_a_paragraph(line: &str) -> bool {
    if ["- ", "* ", "+ ", ">"]
        .iter()
        .any(|marker| line.starts_with(marker))
    {
        return true;
    }
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && (line.len() == hashes || line[hashes..].starts_with(' ')) {
        return true;
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    digits > 0
        && line[..digits].parse::<u32>() == Ok(1)
        && (line[digits..].starts_with(". ") || line[digits..].starts_with(") "))
}

/// The span rule on one block of prose; see [`has_unclosed_code_span`].
fn span_is_unclosed(block: &str) -> bool {
    let chars: Vec<char> = block.chars().collect();
    let run_at = |start: usize| chars[start..].iter().take_while(|&&c| c == '`').count();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            // Outside a span a backslash escapes whatever follows it.
            '\\' => i += 2,
            '`' => {
                let open = run_at(i);
                let mut j = i + open;
                let mut closed = false;
                while j < chars.len() {
                    if chars[j] == '`' {
                        let run = run_at(j);
                        j += run;
                        if run == open {
                            i = j;
                            closed = true;
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                if !closed {
                    return true;
                }
            }
            _ => i += 1,
        }
    }
    false
}

/// The union of every quality smell anywhere in a file spec — the file's
/// own summary, every symbol's summary/behavior, and a whole-file check
/// for the pre-M5 file-wide-dependency-attribution bug's signature (every
/// symbol sharing one identical, non-empty dependency list). That bug is
/// structurally impossible in a freshly-generated spec after M5's
/// per-symbol scoping fix, but this stays as a detector for specs that
/// predate it and were never regenerated since. This is the coarse,
/// whole-document signal `get_spec_coverage` needs ("does this document
/// need another look at all"); `get_spec` on one specific symbol id
/// checks `prose_smells` directly against just that symbol's own prose
/// instead, for a narrower answer.
pub fn file_spec_smells(
    graph: &Graph,
    root: &Path,
    file_id: SymbolId,
    spec: &FileSpec,
) -> Vec<String> {
    // Only smell-check the file summary once it's actually been written.
    // `submit_spec` on a symbol lazily creates a `FileSpec` with a blank
    // summary (`FileSpec::blank`); an empty `spec_hash` is that "never
    // generated" signal (the same one `next_task` reads), and an empty
    // summary in that state is a mid-generation gap, not a short cop-out.
    let mut smells = if spec.file.spec_hash.is_empty() {
        Vec::new()
    } else {
        prose_smells(&spec.file_summary)
    };
    for (_, prose) in &spec.sections {
        smells.extend(prose_smells(&prose.summary));
        smells.extend(prose_smells(&prose.behavior));
    }
    if spec.symbols.len() >= 2 {
        // Compare only the *resolved* intra-repo lines — a shared
        // `externals: …` line (every symbol in a small module returning
        // `anyhow::Result`, say) is normal, not a scoping failure.
        let dep_lists: Vec<Vec<String>> = spec
            .symbols
            .iter()
            .filter_map(|(id, _)| {
                let sym_id = graph.find(id)?;
                let sym = graph.get_symbol(sym_id)?;
                Some(
                    dependency_lines(graph, root, file_id, sym)
                        .into_iter()
                        .filter(|line| !line.starts_with("externals:"))
                        .collect(),
                )
            })
            .collect();
        if dep_lists.len() == spec.symbols.len()
            && !dep_lists[0].is_empty()
            && dep_lists.iter().all(|d| *d == dep_lists[0])
        {
            smells.push("identical_dependencies_across_symbols".to_string());
        }
    }
    smells.sort();
    smells.dedup();
    smells
}

/// The same coarse signal as `file_spec_smells`, for a feature/rollup/
/// system spec's single-blob body — checked against just the LLM-written
/// `## Summary` section (a `RollupSpec`/`SystemSpec`'s `body` already
/// *is* just that; a `FeatureSpec`'s `body` is the whole document, title
/// included, so this extracts the `## Summary` section out of it first).
pub fn body_smells(body: &str) -> Vec<String> {
    let summary = extract_section(body, "## Summary", "\n## ").unwrap_or_else(|| body.to_string());
    prose_smells(&summary)
}

/// Refuse to persist prose that trips the deterministic quality check
/// (`prose_smells`) `get_next_spec_task` also runs. Storing smelly prose
/// isn't harmless: a smell in the "source unchanged, not human-edited"
/// case is exactly what makes `next_task` re-offer the document, so a
/// generate loop that keeps resubmitting equally-thin prose never
/// terminates. Erroring at submit time turns that silent non-termination
/// into a message naming what to fix — the same stance `submit` already
/// takes on content missing a `### Summary`.
pub(super) fn reject_if_smelly(what: &str, smells: Vec<String>) -> Result<()> {
    if smells.is_empty() {
        return Ok(());
    }
    // "Expand it" is the right advice for thin prose and the wrong advice for a
    // broken code span, which needs fixing, not lengthening. Say what each
    // smell asks for, so a text with both is told both.
    let mut advice = Vec::new();
    if smells.iter().any(|s| !s.ends_with("unbalanced_code_span")) {
        advice.push("expand it");
    }
    if smells.iter().any(|s| s.ends_with("unbalanced_code_span")) {
        advice.push(
            "close the code span (to show backticks inside one, wrap it in a \
             double-backtick span: ``a `b` c``; a backslash does not escape \
             inside a span)",
        );
    }
    let advice = advice.join(", and ");
    bail!(
        "{what} fails a quality check ({}) — {advice}; `get_next_spec_task` \
         re-offers a spec until its prose clears the check",
        smells.join(", ")
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::build_graph_from_sources;
    use crate::hash::hash_text;
    use crate::spec::format::{FileSpec, HashPair, SymbolProse};

    use std::path::Path;

    #[test]
    fn prose_smells_flags_markup_a_generator_left_at_the_end_of_the_text() {
        // 63 of these sat in five stored specs, written by an earlier run.
        assert_eq!(
            prose_smells("Formats a date into a readable form and returns it.</content>"),
            vec!["stray_markup"]
        );
        assert!(
            prose_smells("Formats a date into a readable form and returns it.").is_empty(),
            "prose without the tag is untouched"
        );
    }

    #[test]
    fn prose_smells_flags_a_code_span_that_never_closes() {
        // The slip that went through unchecked: a code span whose own text
        // holds backticks, written with backslashes. Markdown does not honour a
        // backslash inside a code span, so the first span closes early and a
        // double-backtick run is left with no partner.
        let slip = "Then, for each symbol, a `## \\`name\\`` heading (from `short_name`), \
                    the symbol's current signature.";
        assert_eq!(prose_smells(slip), vec!["unbalanced_code_span"]);
        // A stored fragment that began mid-sentence with a stray backtick (it sat in
        // a spec marked current, because nothing could see it).
        assert_eq!(
            prose_smells("` and so on) back out of rendered markdown."),
            vec!["unbalanced_code_span"]
        );
        assert_eq!(
            prose_smells("Returns the `value without ever closing it, as callers expect."),
            vec!["unbalanced_code_span"]
        );
    }

    #[test]
    fn prose_smells_accepts_every_well_formed_code_span() {
        let fine = |text: &str| {
            assert!(prose_smells(text).is_empty(), "wrongly flagged: {text}");
        };
        fine("Uses `a` and `b` to build the result for each caller.");
        // A double-backtick span may hold single backticks.
        fine("Writes ``[`src/hash.rs`](../x.rs)`` as the title line of the page.");
        fine("The delimiter is `` ` `` in the header block of every spec.");
        // A backslash-escaped backtick outside any span is a literal character.
        fine("Prints a literal \\` character whenever the caller asks for one.");
        // A span may run across a line break inside one paragraph.
        fine("Reads `the value\nthat spans two lines` and returns the result to callers.");
        // Whatever sits in a fenced block is not prose.
        fine("An example follows.\n```\nlet x = `;\n```\nThe example above is only a sketch.");
        fine("Plain prose with no backticks at all, in several words of text.");
    }

    #[test]
    fn a_code_span_cannot_run_across_blocks() {
        // CommonMark: a span ends at the end of its paragraph, so an opener in
        // one paragraph and a closer in the next are both left unmatched.
        assert_eq!(
            prose_smells(
                "Opens `a span here and goes on.\n\nCloses it here` in another paragraph."
            ),
            vec!["unbalanced_code_span"]
        );
        // The same for two list items that each hold one lone backtick.
        assert_eq!(
            prose_smells(
                "- the first item has a lone ` backtick in it\n- the second one has a lone ` too"
            ),
            vec!["unbalanced_code_span"]
        );
    }

    #[test]
    fn a_wrapped_line_that_only_looks_like_a_block_start_does_not_cut_a_span() {
        // Inside a paragraph only a bullet, a quote, a heading or "1." starts a
        // new block. These wrapped lines do not, so the span below stays whole.
        let fine = |text: &str| {
            assert!(prose_smells(text).is_empty(), "wrongly flagged: {text}");
        };
        fine("Reads `foo bar\n2. baz` and returns the result to callers.");
        fine("Uses `the attribute\n#[derive(Debug)] shown` in the example for readers.");
        fine("Fills `the row\n| with cells` the way the example for readers shows.");
        // "1." does interrupt a paragraph, so a span cannot cross it.
        assert_eq!(
            prose_smells("Reads `foo bar\n1. baz` and returns the result to callers."),
            vec!["unbalanced_code_span"]
        );
        // A heading is `#` then a space; `#!` is not one.
        fine("Reads `foo\n#!bar` and returns the result to callers of it.");
    }

    #[test]
    fn a_fence_is_found_by_its_character_and_length() {
        let fine = |text: &str| {
            assert!(prose_smells(text).is_empty(), "wrongly flagged: {text}");
        };
        // A line that starts with an inline triple-backtick span is not a fence...
        assert_eq!(
            prose_smells(
                "```code``` is used here for a sample.\nA second line with a lone ` in prose of the text."
            ),
            vec!["unbalanced_code_span"],
            "the lone backtick on line two must not be swallowed as 'code'"
        );
        // ...a four-backtick fence may hold a three-backtick line...
        fine(
            "Text before it.\n````\n```\nlet x = `;\n```\n````\nMore text after the example here.",
        );
        // ...and a tilde line cannot close a backtick fence.
        fine("```\nlet x = `;\n~~~\nstill code `\n```\nDone and dusted after the example here.");
    }

    #[test]
    fn an_indented_code_block_is_not_prose() {
        let text = "Prose before the sample, in a few words.\n\n    let x = `a;\n\nProse after the sample, in a few words.";
        assert!(prose_smells(text).is_empty(), "{:?}", prose_smells(text));
    }

    #[test]
    fn the_rejection_names_the_fix_for_every_smell_it_reports() {
        let only_span = reject_if_smelly("Behavior", vec!["Behavior: unbalanced_code_span".into()])
            .unwrap_err()
            .to_string();
        assert!(only_span.contains("double-backtick"), "{only_span}");
        assert!(!only_span.contains("expand it"), "{only_span}");

        let only_thin = reject_if_smelly("Behavior", vec!["Behavior: suspiciously_short".into()])
            .unwrap_err()
            .to_string();
        assert!(only_thin.contains("expand it"), "{only_thin}");
        assert!(!only_thin.contains("double-backtick"), "{only_thin}");

        // Both at once: the agent is told both things, not only the first.
        let both = reject_if_smelly(
            "Behavior",
            vec![
                "Behavior: suspiciously_short".into(),
                "Behavior: unbalanced_code_span".into(),
            ],
        )
        .unwrap_err()
        .to_string();
        assert!(both.contains("expand it"), "{both}");
        assert!(both.contains("double-backtick"), "{both}");
    }

    #[test]
    fn body_smells_sees_it_in_the_summary_section_of_a_feature() {
        let body = "# Title\n## Summary\nThe page lets a user `submit something and nothing more.\n\
                    ## How it works\nA fine `paired` span.\n";
        assert_eq!(body_smells(body), vec!["unbalanced_code_span"]);
    }

    #[test]
    fn prose_smells_flags_cop_out_phrases_and_short_text_but_not_real_prose() {
        assert_eq!(
            prose_smells("Returns the cached client. See the source for details."),
            vec!["cop_out_phrase"]
        );
        assert_eq!(prose_smells("Does one thing."), vec!["suspiciously_short"]);
        assert!(
            prose_smells(
                "Formats a date string into a human-readable form, returning \
                 a fallback for invalid input."
            )
            .is_empty()
        );
    }

    #[test]
    fn file_spec_smells_catches_the_real_pre_m5_stub_pattern() {
        // The exact shape found auditing two real pre-M5 specs: every
        // symbol's prose is a template stub ("<name> does its job." /
        // "See source."), and every symbol shares an identical,
        // non-empty dependency list (the file-wide-attribution bug).
        // dependency_lines needs each symbol's own source text on disk
        // (see read_symbol_text), so this needs a real temp dir, the same
        // pattern depends_on_is_scoped_to_what_each_symbol_actually_uses
        // uses.
        let dir = std::env::temp_dir().join(format!(
            "codeowl-spec-test-{}-smell-fixture",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let a_content = "import { helper } from './util';\n\nexport function one() {\n  helper();\n}\n\nexport function two() {\n  helper();\n}\n";
        let util_content = "export function helper(): void {}\n";
        for (rel, content) in [("a.ts", a_content), ("util.ts", util_content)] {
            std::fs::write(dir.join(rel), content).unwrap();
        }
        let extractions = vec![
            crate::fixtures::extract_and_hash("a.ts", a_content),
            crate::fixtures::extract_and_hash("util.ts", util_content),
        ];
        let mut graph = Graph::build(extractions);
        let file_imports = crate::typescript::extract_imports(a_content, "a.ts");
        let resolved = file_imports
            .imports
            .iter()
            .map(|imp| crate::graph::ResolvedImport {
                from_file: "a.ts".to_string(),
                specifier: imp.specifier.clone(),
                imported_name: imp.imported_name.clone(),
                target: graph.find(&format!(
                    "{}.ts::{}",
                    imp.specifier.trim_start_matches("./"),
                    imp.imported_name
                )),
            })
            .collect();
        graph.set_resolved_imports(resolved);
        let file_id = graph.find("a.ts").unwrap();

        let spec = FileSpec {
            source_path: "a.ts".to_string(),
            file: HashPair::default(),
            symbols: vec![
                ("a.ts::one".to_string(), HashPair::default()),
                ("a.ts::two".to_string(), HashPair::default()),
            ],
            file_summary: "a.ts summary.".to_string(),
            sections: vec![
                (
                    "a.ts::one".to_string(),
                    SymbolProse {
                        summary: "one does its job.".to_string(),
                        behavior: "See source.".to_string(),
                    },
                ),
                (
                    "a.ts::two".to_string(),
                    SymbolProse {
                        summary: "two does its job.".to_string(),
                        behavior: "See source.".to_string(),
                    },
                ),
            ],
        };

        let mut smells = file_spec_smells(&graph, &dir, file_id, &spec);
        smells.sort();
        assert_eq!(
            smells,
            vec![
                "cop_out_phrase",
                "identical_dependencies_across_symbols",
                "suspiciously_short"
            ]
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_spec_smells_is_empty_for_real_looking_prose_with_distinct_dependencies() {
        let graph = build_graph_from_sources(&[(
            "a.ts",
            "export function one(): void {}\nexport function two(): void {}\n",
        )]);
        let file_id = graph.find("a.ts").unwrap();
        let spec = FileSpec {
            source_path: "a.ts".to_string(),
            file: HashPair::default(),
            symbols: vec![
                ("a.ts::one".to_string(), HashPair::default()),
                ("a.ts::two".to_string(), HashPair::default()),
            ],
            file_summary: "Two small, unrelated utility functions used across the app.".to_string(),
            sections: vec![
                (
                    "a.ts::one".to_string(),
                    SymbolProse {
                        summary: "Does the first specific thing this file needs.".to_string(),
                        behavior: "Runs synchronously with no side effects at all.".to_string(),
                    },
                ),
                (
                    "a.ts::two".to_string(),
                    SymbolProse {
                        summary: "Does a second, unrelated specific thing.".to_string(),
                        behavior: "Also runs synchronously with no side effects.".to_string(),
                    },
                ),
            ],
        };
        assert!(file_spec_smells(&graph, Path::new("/nonexistent"), file_id, &spec).is_empty());
    }

    #[test]
    fn a_file_whose_summary_was_never_written_is_not_smelly_for_being_short() {
        // Mid-generation state: `submit_spec` on a symbol lazily creates a
        // `FileSpec` with a blank summary (via `FileSpec::blank`). An empty
        // summary that was never generated must not read as a
        // `suspiciously_short` cop-out — nothing was written to smell.
        let graph = build_graph_from_sources(&[(
            "a.ts",
            "export function one(): void {}\nexport function two(): void {}\n",
        )]);
        let file_id = graph.find("a.ts").unwrap();
        let spec = FileSpec {
            source_path: "a.ts".to_string(),
            file: HashPair::default(), // spec_hash empty -> file summary never written
            symbols: vec![("a.ts::one".to_string(), HashPair::default())],
            file_summary: String::new(),
            sections: vec![(
                "a.ts::one".to_string(),
                SymbolProse {
                    summary: "Does the first specific thing this file needs.".to_string(),
                    behavior: "Runs synchronously with no side effects at all.".to_string(),
                },
            )],
        };
        assert!(file_spec_smells(&graph, Path::new("/nonexistent"), file_id, &spec).is_empty());

        // But once the summary *has* been written (spec_hash set), a short
        // one is still flagged.
        let written = FileSpec {
            file: HashPair {
                spec_hash: hash_text(""),
                ..HashPair::default()
            },
            file_summary: "Two helpers.".to_string(),
            ..spec
        };
        assert_eq!(
            file_spec_smells(&graph, Path::new("/nonexistent"), file_id, &written),
            vec!["suspiciously_short"]
        );
    }

    #[test]
    fn body_smells_checks_only_the_summary_section_of_a_feature_or_rollup_body() {
        assert_eq!(
            body_smells(
                "# Title\n## Summary\nSee the route handler for details.\n## How it works\n1. Does something else entirely, at length, in this other section.\n"
            ),
            vec!["cop_out_phrase"]
        );
        assert!(
            body_smells(
                "# Title\n## Summary\nLets a registered competitor submit their artwork \
                 for judging in a competition.\n## How it works\n1. Step one.\n"
            )
            .is_empty()
        );
    }
}
