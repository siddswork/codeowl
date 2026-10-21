//! The order a budgeted generation run spends its calls in, and which stale files matter most.

use super::coverage::CoverageItem;
use crate::graph::Graph;
use crate::lang::FileRole;
use crate::stack::file_role;

/// The `n` file-kind items most worth regenerating, ranked by blast radius
/// (`fan_in`) rather than by `prioritize`'s generate-order tiering — "which
/// stale documents would hurt the most if left wrong" instead of "which
/// order should a budgeted run spend on." Same "needs attention" criterion
/// as `pending` (non-current, or current but smelly); only `kind == "file"`
/// carries real fan-in (see `CoverageItem::fan_in`), so rollups/features/
/// system never appear here even if stale.
pub fn top_stale_by_impact(items: &[CoverageItem], n: usize) -> Vec<CoverageItem> {
    let mut candidates: Vec<CoverageItem> = items
        .iter()
        .filter(|i| i.kind == "file" && (i.status != "current" || !i.smells.is_empty()))
        .cloned()
        .collect();
    candidates.sort_by_key(|i| std::cmp::Reverse(i.fan_in));
    candidates.truncate(n);
    candidates
}

/// The "shared infrastructure" tier that gets documented before features
/// is the `SHARED_CODE_MAX_FILES` files with the highest import fan-in
/// across the whole repo (only counting those imported at least
/// `SHARED_CODE_FAN_IN` times, and only [`FileRole::Domain`] files — a UI
/// primitive or generated file is excluded however high its fan-in). It's
/// an *absolute* set anchored to the whole repo, not the top-N of
/// whatever's left to generate — otherwise a budgeted `--all` run keeps
/// promoting the next batch into tier 0 and never reaches a feature.
/// Laptop-scale heuristics, not tuned values.
const SHARED_CODE_FAN_IN: usize = 3;
const SHARED_CODE_MAX_FILES: usize = 8;

/// `coverage`'s items that still need attention, ordered the way
/// `/codeowl generate --all`/`--budget=N` should spend a limited budget
/// (see `ARCHITECTURE.md`'s "Generation priority"):
///
/// 1. the shared-code tier — the top `SHARED_CODE_MAX_FILES` files by
///    import fan-in (`fan_in >= SHARED_CODE_FAN_IN`), whose real summaries
///    upgrade every dependent spec's context for free
/// 2. feature specs — the BA-facing payoff, now written with real
///    dependency summaries rather than stubs
/// 3. the long tail of files (everything not in tier 1)
/// 4. directory rollups (need their files first)
/// 5. test code (`is_test_path`) — file specs only, always after the
///    product, whatever its fan-in
/// 6. the system spec — the capstone, only writable once everything it
///    composes from is current, so always last
///
/// Within a tier, an honestly `"missing"` or `"stale"` document outranks a
/// merely smelly-but-`"current"` one, then descending fan-in, then `id`
/// for a stable, reproducible order.
///
/// Includes a `"current"` item when it has a quality smell — hash-based
/// staleness only verifies a spec's *inputs* haven't moved, never that
/// its prose was ever meaningful (a "see the source" stub, once written,
/// stays hash-`"current"` forever unless something else flags it). Never
/// filtering on smells too would mean a `--all --budget=N` run could run
/// to completion while silently leaving known-bad content in place.
pub fn prioritize(items: Vec<CoverageItem>, graph: &Graph) -> Vec<CoverageItem> {
    // The fan-in floor for the shared-code tier: the higher of
    // SHARED_CODE_FAN_IN and the Nth-highest fan-in among *all* candidate
    // files in the repo — computed from `items` before the `pending`
    // filter, on purpose. Using `pending` instead would recompute the
    // cutoff every run as the top files get generated, quietly promoting
    // the next batch into tier 0 forever; anchoring it to the whole repo
    // means once the real top-N are current, tier 0 is empty and features
    // lead. (Ties at the cutoff can nudge the count slightly over N.)
    let shared_cutoff = {
        let mut fan_ins: Vec<usize> = items
            .iter()
            .filter(|i| i.kind == "file" && file_role(graph, &i.id) == FileRole::Domain)
            .map(|i| i.fan_in)
            .filter(|&f| f >= SHARED_CODE_FAN_IN)
            .collect();
        fan_ins.sort_unstable_by(|a, b| b.cmp(a));
        fan_ins
            .get(SHARED_CODE_MAX_FILES - 1)
            .copied()
            .unwrap_or(SHARED_CODE_FAN_IN)
            .max(SHARED_CODE_FAN_IN)
    };

    let mut pending: Vec<CoverageItem> = items
        .into_iter()
        .filter(|i| i.status != "current" || !i.smells.is_empty())
        .collect();

    pending.sort_by(|a, b| {
        let tier = |item: &CoverageItem| -> u8 {
            if item.kind == "file" {
                return match file_role(graph, &item.id) {
                    FileRole::Test => 5,
                    FileRole::Domain if item.fan_in >= shared_cutoff => 0,
                    // Below the fan-in cutoff, or a primitive file at any
                    // fan-in: the long tail, after features.
                    FileRole::Domain | FileRole::Primitive => 2,
                    // Unreachable in practice, kept as its own arm rather
                    // than folded into the tier above (code review,
                    // 2026-09-20): `file_is_spec_bearing` (M19 commit 2)
                    // excludes every `FileRole::Generated` file before it
                    // can ever reach the coverage inventory this sort
                    // runs over, so a reader must not infer from this
                    // match that a generated file is still prioritized.
                    FileRole::Generated => 2,
                };
            }
            match item.kind.as_str() {
                "feature" => 1,
                "rollup" => 3,
                "system" => 6,
                _ => 7,
            }
        };
        fn urgency(status: &str) -> u8 {
            match status {
                "missing" => 0,
                "stale" => 1,
                _ => 2, // "current" but smelly -- the only other way in
            }
        }
        tier(a)
            .cmp(&tier(b))
            .then(urgency(&a.status).cmp(&urgency(&b.status)))
            .then(b.fan_in.cmp(&a.fan_in))
            .then(a.id.cmp(&b.id))
    });
    pending
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::build_graph_from_sources;
    use crate::spec::coverage::{CoverageItem, coverage, summarize};
    use crate::spec::feature::submit_feature;
    use crate::spec::file_task::submit;
    use crate::spec::rollup::submit_rollup;
    use crate::spec::system::submit_system;
    use crate::spec::test_support::{
        SYSTEM_FIXTURE, build_feature_fixture, plant_smelly_symbol_spec,
    };

    #[test]
    fn coverage_reports_everything_missing_then_everything_current() {
        let (graph, dir) = build_feature_fixture(SYSTEM_FIXTURE, "coverage1");

        let items = coverage(&graph, &dir, None).unwrap();
        let summary = summarize(&items);
        assert_eq!(
            summary.missing, 8,
            "5 files + 1 rollup + 1 feature + 1 system"
        );
        assert_eq!(summary.current, 0);
        assert_eq!(summary.stale, 0);

        let pending = prioritize(items, &graph);
        let ids: Vec<&str> = pending.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "feature:submit",
                "lib/supabase.ts",
                "app/api/submit-artwork/route.ts",
                "app/submit/page.tsx",
                "lib/one.ts",
                "lib/two.ts",
                "rollup:lib",
                "system",
            ],
            "no file here clears the shared-code fan-in bar, so: features, \
             then files by descending fan-in (lib/supabase.ts is imported \
             by both page.tsx and route.ts), then rollups, then the system \
             spec last"
        );
        assert_eq!(
            pending
                .iter()
                .find(|i| i.id == "lib/supabase.ts")
                .unwrap()
                .fan_in,
            2
        );

        // Generate everything, then confirm coverage agrees nothing is
        // pending.
        for (sym_id, file_id) in [
            ("lib/supabase.ts::getSupabase", "lib/supabase.ts"),
            ("lib/one.ts::one", "lib/one.ts"),
            ("lib/two.ts::two", "lib/two.ts"),
        ] {
            submit(
                &graph,
                &dir,
                sym_id,
                "### Summary\nDoes one small, specific job.\n### Behavior\nRuns without any side effects.\n",
            )
            .unwrap();
            submit(&graph, &dir, file_id, "A small file of helper functions.").unwrap();
        }
        submit_rollup(
            &graph,
            &dir,
            "lib",
            "Small shared helper functions for this module.",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "app/submit/page.tsx::Page",
            "### Summary\nRenders the artwork submission form.\n### Behavior\nSubmits data to the API on save.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "app/submit/page.tsx",
            "The artwork submission page for competitors.",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "app/api/submit-artwork/route.ts::POST",
            "### Summary\nAccepts an artwork submission request.\n### Behavior\nPersists the submission to storage.\n",
        )
        .unwrap();
        submit(
            &graph,
            &dir,
            "app/api/submit-artwork/route.ts",
            "The API route that accepts artwork submissions.",
        )
        .unwrap();
        submit_feature(
            &graph,
            &dir,
            "submit",
            "# Artwork submission\n## Summary\nLets an artist submit artwork for judging.\n",
        )
        .unwrap();
        submit_system(
            &graph,
            &dir,
            "# Acme\n## Summary\nA platform for running art competitions.\n",
        )
        .unwrap();

        let items = coverage(&graph, &dir, None).unwrap();
        let summary = summarize(&items);
        assert_eq!(summary.current, 8);
        assert_eq!(summary.stale, 0);
        assert_eq!(summary.missing, 0);
        assert_eq!(summary.generations_remaining, 0);
        assert!(prioritize(items, &graph).is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn coverage_never_counts_a_generated_file_as_missing_or_pending() {
        // M19: a generated interface with real exported methods must not
        // inflate get_spec_coverage's missing count or appear in its
        // pending worklist -- it's reference-only, never spec-bearing,
        // regardless of how many methods it exports.
        let dir =
            std::env::temp_dir().join(format!("codeowl-spec-test-{}-gen2", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let path = "target/generated-sources/foo/HeroesResource.java";
        let mut graph = build_graph_from_sources(&[(
            path,
            "export function getAllHeroes(): void {}\n\
             export function getRandomHero(): void {}\n",
        )]);
        graph.set_pack_name("java");

        let items = coverage(&graph, &dir, None).unwrap();
        let pending = prioritize(items, &graph);
        assert!(
            !pending.iter().any(|i| i.kind == "file"),
            "the generated file must never appear as a pending file document: {pending:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn coverage_includes_a_current_but_smelly_file_in_pending() {
        let (graph, dir) = build_feature_fixture(SYSTEM_FIXTURE, "coverage3");
        // A hash-current symbol spec with a cop-out Behavior — makes the
        // whole file spec smelly without anything having moved.
        plant_smelly_symbol_spec(
            &graph,
            &dir,
            "lib/supabase.ts::getSupabase",
            "Builds and returns the shared Supabase client.",
            "See the source for details.",
        );
        submit(
            &graph,
            &dir,
            "lib/supabase.ts",
            "A thin wrapper module around the Supabase browser SDK.",
        )
        .unwrap();

        let items = coverage(&graph, &dir, Some("lib/supabase.ts")).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "current");
        assert!(!items[0].smells.is_empty());

        let summary = summarize(&items);
        assert_eq!(summary.current, 1);
        assert_eq!(summary.smelly, 1);

        let pending = prioritize(items, &graph);
        assert_eq!(
            pending.len(),
            1,
            "a current-but-smelly document must still show up as pending"
        );
        assert_eq!(pending[0].status, "current");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prioritize_puts_shared_code_first_features_next_system_last() {
        fn item(id: &str, kind: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: "missing".into(),
                fan_in,
                smells: Vec::new(),
                generations: 0,
            }
        }
        let items = vec![
            item("system", "system", 0),
            item("feature:checkout", "feature", 0),
            item("lib/db.ts", "file", 12), // shared infra — many importers
            item("app/page.tsx", "file", 0), // a leaf
            item("rollup:lib", "rollup", 0),
        ];
        let graph = crate::fixtures::build_graph_from_sources(&[]);
        let ids: Vec<String> = prioritize(items, &graph)
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(
            ids,
            vec![
                "lib/db.ts",        // high-fan-in files first: their real
                "feature:checkout", // summaries upgrade downstream specs
                "app/page.tsx",     // then the long tail of leaf files
                "rollup:lib",       // then rollups
                "system",           // the system spec is always last
            ]
        );
    }

    #[test]
    fn prioritize_sinks_test_code_below_the_product_whatever_its_fan_in() {
        fn item(id: &str, kind: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: "missing".into(),
                fan_in,
                smells: Vec::new(),
                generations: 0,
            }
        }
        let items = vec![
            item("e2e/helpers/api-client.ts", "file", 71), // huge fan-in, but test code
            item("lib/db.ts", "file", 4),
            item("feature:checkout", "feature", 0),
            item("app/dashboard/card.test.tsx", "file", 0),
            item("system", "system", 0),
        ];
        let graph = crate::fixtures::build_graph_from_sources(&[]);
        let ids: Vec<String> = prioritize(items, &graph)
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(
            ids,
            vec![
                "lib/db.ts",                   // real shared code
                "feature:checkout",            // features
                "e2e/helpers/api-client.ts",   // test code — after the product,
                "app/dashboard/card.test.tsx", // fan-in only orders within the tier
                "system",                      // system still dead last
            ]
        );
    }

    #[test]
    fn top_stale_by_impact_ranks_file_items_by_fan_in_descending() {
        fn item(id: &str, status: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: "file".into(),
                status: status.into(),
                fan_in,
                smells: Vec::new(),
                generations: usize::from(status != "current"),
            }
        }
        let items = vec![
            item("quiet.ts", "stale", 1),
            item("db.ts", "stale", 40),
            item("mid.ts", "missing", 10),
            item("clean.ts", "current", 99), // clean -- not "needing attention"
        ];
        let top = top_stale_by_impact(&items, 5);
        let ids: Vec<&str> = top.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["db.ts", "mid.ts", "quiet.ts"],
            "highest fan-in first; a clean current item never qualifies regardless of fan-in"
        );
    }

    #[test]
    fn top_stale_by_impact_truncates_to_n() {
        fn item(id: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: "file".into(),
                status: "stale".into(),
                fan_in,
                smells: Vec::new(),
                generations: 1,
            }
        }
        let items: Vec<CoverageItem> = (0..10).map(|i| item(&format!("f{i}.ts"), i)).collect();
        assert_eq!(top_stale_by_impact(&items, 3).len(), 3);
        assert_eq!(top_stale_by_impact(&items, 3)[0].id, "f9.ts");
    }

    #[test]
    fn top_stale_by_impact_includes_current_but_smelly_items() {
        let smelly_current = CoverageItem {
            id: "smelly.ts".into(),
            kind: "file".into(),
            status: "current".into(),
            fan_in: 7,
            smells: vec!["cop_out_phrase".to_string()],
            generations: 1,
        };
        let top = top_stale_by_impact(&[smelly_current], 5);
        let ids: Vec<&str> = top.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["smelly.ts"],
            "current but smelly still needs attention, same as pending's own criterion"
        );
    }

    #[test]
    fn top_stale_by_impact_excludes_non_file_kinds() {
        let feature = CoverageItem {
            id: "feature:checkout".into(),
            kind: "feature".into(),
            status: "stale".into(),
            fan_in: 0,
            smells: Vec::new(),
            generations: 1,
        };
        assert!(top_stale_by_impact(&[feature], 5).is_empty());
    }

    #[test]
    fn prioritize_caps_the_shared_code_tier_so_features_are_reachable() {
        fn item(id: &str, kind: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: "missing".into(),
                fan_in,
                smells: Vec::new(),
                generations: 0,
            }
        }
        // 12 files, fan-in 20 down to 9 — all well over SHARED_CODE_FAN_IN.
        let mut items = vec![item("feature:x", "feature", 0)];
        for fan in (9..=20).rev() {
            items.push(item(&format!("lib/f{fan:02}.ts"), "file", fan));
        }
        let graph = crate::fixtures::build_graph_from_sources(&[]);
        let ids: Vec<String> = prioritize(items, &graph)
            .into_iter()
            .map(|i| i.id)
            .collect();

        // Only the top SHARED_CODE_MAX_FILES (fan-in 20..13) come before
        // the feature; the rest (12..9) fall to the long tail after it.
        assert_eq!(ids[0], "lib/f20.ts");
        assert_eq!(ids[SHARED_CODE_MAX_FILES - 1], "lib/f13.ts");
        assert_eq!(ids[SHARED_CODE_MAX_FILES], "feature:x");
        assert_eq!(ids[SHARED_CODE_MAX_FILES + 1], "lib/f12.ts");
        assert_eq!(ids.last().unwrap(), "lib/f09.ts");
    }

    #[test]
    fn shared_cutoff_is_anchored_to_the_whole_repo_not_whats_left() {
        fn item(id: &str, kind: &str, status: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: kind.into(),
                status: status.into(),
                fan_in,
                smells: Vec::new(),
                generations: 0,
            }
        }
        // The repo's 8 highest-fan-in files are already current. What's
        // left is a mid-fan-in file and a feature. The mid file must NOT be
        // treated as shared code just because it's now the top of pending.
        let mut items = vec![
            item("lib/mid.ts", "file", "missing", 9),
            item("feature:x", "feature", "missing", 0),
        ];
        for fan in (12..=19).rev() {
            items.push(item(&format!("lib/hot{fan}.ts"), "file", "current", fan));
        }
        let graph = crate::fixtures::build_graph_from_sources(&[]);
        let ids: Vec<String> = prioritize(items, &graph)
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(
            ids,
            vec!["feature:x", "lib/mid.ts"],
            "cutoff is set by the 8 current files (fan-in 12), so lib/mid.ts (9) is long tail"
        );
    }

    #[test]
    fn ui_primitives_stay_out_of_the_shared_tier_even_at_high_fan_in() {
        fn item(id: &str, fan_in: usize) -> CoverageItem {
            CoverageItem {
                id: id.into(),
                kind: "file".into(),
                status: "missing".into(),
                fan_in,
                smells: Vec::new(),
                generations: 0,
            }
        }
        let items = vec![
            item("components/ui/button.tsx", 40), // imported everywhere
            item("lib/db.ts", 12),
            CoverageItem {
                id: "feature:x".into(),
                kind: "feature".into(),
                status: "missing".into(),
                fan_in: 0,
                smells: Vec::new(),
                generations: 0,
            },
        ];
        let graph = crate::fixtures::build_graph_from_sources(&[]);
        let ids: Vec<String> = prioritize(items, &graph)
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(
            ids,
            vec!["lib/db.ts", "feature:x", "components/ui/button.tsx"]
        );
    }
}
