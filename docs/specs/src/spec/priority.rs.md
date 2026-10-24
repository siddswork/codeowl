---
type: file
title: "src/spec/priority.rs"
description: "Specification for `src/spec/priority.rs`."
generated: { by: CodeOwl }
resource: "../../../../src/spec/priority.rs"
source_paths: [src/spec/priority.rs]
file: { source_hash: , deps_hash: , spec_hash:  }
symbols:
  src/spec/priority.rs::top_stale_by_impact: { source_hash: 622de3ce0d5be17752b92292f6e183e69c8f9e42ea7b0cc919f240ab8266f44f, deps_hash: 9d7a969e9481e619e3424d74485b90a959580831f90fea67bbf19d14f116942c, spec_hash: 6f4a87a8b3a4743782b50bfb1be1f126a201b669971fbcf4ff3c90a1214f0cb8 }
  src/spec/priority.rs::prioritize: { source_hash: f3a338392280b2c5ef15b327bd74052a28806b24515046a6ab24d5cf1939f85c, deps_hash: 9fb02c79a8164951423437955d4f2844446258dd7a532d2624d4b24258b27c06, spec_hash: 966dc52b3920632846afd1be6ef27b4bf350905bf1138db41ac373dafbce5a54 }
dep_targets:
  src/spec/priority.rs::top_stale_by_impact -> src/spec/coverage.rs::CoverageItem: 490a22c5cc59
  src/spec/priority.rs::prioritize -> src/graph.rs::Graph: 13dbdfa88b0e
  src/spec/priority.rs::prioritize -> src/lang.rs::FileRole: 2e8bc021ccde
---
# src/spec/priority.rs
> Generated using [CodeOwl](https://github.com/siddswork/codeowl)
## Summary


## `top_stale_by_impact`
`pub fn top_stale_by_impact(items: &[CoverageItem], n: usize) -> Vec<CoverageItem>`
### Summary
Picks the files whose out-of-date specs would do the most harm, ranked by how many other files import them. It answers "what should I fix first" and is separate from the order a budgeted generation run follows.
### Behavior
Keeps only file items that need attention, meaning the status is not `current` or the spec is current but has quality problems. Folder summaries, features and the system spec are left out since only files have a real fan-in. The candidates are sorted by `fan_in`, highest first, and cut to the first `n`. The sort is stable, so files with equal fan-in stay in the order the coverage list gave them (path order). It returns clones and has no failure mode; `n` of 0 gives an empty list.
### Depends on
- [`src/spec/coverage.rs::CoverageItem`](coverage.rs.md) — super::coverage

## `prioritize`
`pub fn prioritize(items: Vec<CoverageItem>, graph: &Graph) -> Vec<CoverageItem>`
### Summary
Takes the full coverage list, keeps only the documents that still need work, and puts them in the order a limited generation budget should be spent. Widely used shared code comes first, because its written summaries make every later spec better, and the system summary comes last, because it can only be written once everything below it is done.
### Behavior
First it fixes the cutoff for the top tier: the larger of `SHARED_CODE_FAN_IN` and the `SHARED_CODE_MAX_FILES`-th highest fan-in among all domain files in the whole repo. It is computed before filtering on purpose. Computed from only the pending items, it would move on every run as the top files got finished and quietly promote the next batch forever; anchored to the whole repo, the top tier empties once the real top files are current. Ties at the cutoff can make the tier slightly larger.

Items are kept when their status is not `current`, or when they are current but have quality problems, because matching hashes never prove the prose was meaningful and filtering them out would leave known-bad text in place after a "complete" run.

Sorting uses a tier number, then urgency, then fan-in, then id. Tiers: 0 shared domain files at or above the cutoff, 1 feature specs, 2 other files (the long tail, including low-level helper files at any fan-in), 3 folder summaries, 5 test code (always after product code whatever its fan-in), 6 the system spec. A generated-code file is mapped to tier 2 but never reaches here, because such files are excluded earlier. Within a tier a `missing` item comes before a `stale` one, which comes before a merely smelly one; then higher fan-in first; then id for a stable, repeatable order.
### Depends on
- [`src/spec/coverage.rs::CoverageItem`](coverage.rs.md) — super::coverage
- [`src/graph.rs::Graph`](../graph.rs.md) — crate::graph
- [`src/lang.rs::FileRole`](../lang.rs.md) — crate::lang
- [`src/stack.rs::file_role`](../stack.rs.md) — crate::stack
