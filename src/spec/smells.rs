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
    smells
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
    bail!(
        "{what} fails a quality check ({}) — expand it; `get_next_spec_task` \
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
