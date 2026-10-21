# Experiment 7: can a restructuring re-key the specs it orphans?

Status (2026-10-21): **dry-run done; no kill criterion triggered; the write step is built and was run on a copy of the repository (see "The write step on a copy"); decision recorded in `DECISIONS.md` question 26.** The protocol below was written before any result.

## Question

After a restructuring (code moved, split, merged, renamed) every spec for the moved code is orphaned at its old id and the new location reads as missing, so a regeneration writes it all again. Most of the old prose is still true, because the code it describes did not change. Can CodeOwl find, deterministically and without an LLM, the old prose that still applies, and re-key it to the new location?

## Why now

CodeOwl's own repo was restructured on 2026-10-21 (`spec.rs` into `spec/`, a directory per language pack; `DECISIONS.md` question 25). The stored specs are untouched, so the orphans are still on disk: a ready-made experiment. They must be measured **before** the full regeneration, which would overwrite them.

A first, rough probe on that corpus (no dependency check): of 322 orphaned symbols with written prose, 275 had a symbol with identical source text in the new tree (256 with a single same-named successor, 19 ambiguous), 46 had only a same-named symbol with different text, 1 had no successor.

## Instrument

`spec::adopt::plan`, a read-only planner (the first half of the feature if it is built; on the experiment branch, deleted if it is not), reported by `codeowl adopt-moved-specs <path>` without any write option. For every orphaned symbol that has written prose it reports:

1. **A successor**, by source hash. Exactly one current symbol with the same source hash and the same short name, or, when several, exactly one whose file path lies under the old file's stem (`src/java.rs` to `src/java/mod.rs`). Otherwise *ambiguous*. Same name but different text is *text differs*; nothing is *no successor*.
2. **A dependency relation**, comparing the interface hashes (not the ids; `deps_hash` hashes ids, so it cannot match after a dependency moves) of the dependencies the old spec recorded with those of the successor now: *equal*, *only additions* (every recorded hash still present; extras allowed, which is what a split produces when a same-file call becomes a cross-file import), *changed* (a recorded hash is gone), or *unknown* (the old spec recorded no dependency hashes and the successor has some).
3. **Moved paths named in the prose**: file-like names (`something.rs`, `src/x/y.ts`) in the old summary or behavior that match no current file.

## Measures and kill criteria

| Measure | How | Stop if |
|---|---|---|
| Reach | Symbols adoptable (one successor, dependency relation *equal* or *only additions*) over orphaned symbols with prose | Under 50 % |
| Saving | Generations avoided (one per adoptable symbol) at the measured 1.5k tokens per cycle (`NORTHSTAR.md` K7), against the full regeneration | Not material |
| Correctness | Adoptable specs whose prose names a path that no longer exists; a hand check of a sample against the code | Many are wrong in a way the checks above do not catch |
| Quality | About ten adopted specs read against what a fresh generation says for the same code | Clearly worse |

Reported separately, because they decide the design: how many adoptable symbols fall under each dependency relation (does *only additions* matter?), and how many orphaned file-level summaries there are (file, rollup and system summaries describe a whole and are not adopted).

## Not measured

Whether an LLM reviser, given the old prose as `previous`, would do better than a straight carry-over for the not-adoptable remainder. That is option B of the design discussion and needs its own run.

## Results

Run on 2026-10-21 against CodeOwl's own corpus at `d2094d4` (the restructuring merged, the stored specs untouched): `codeowl adopt-moved-specs .`. **No kill criterion was triggered.**

| Outcome for the 322 orphaned symbols with written prose | Count | Share |
|---|---|---|
| **Adoptable as current**: one successor with the same text, dependencies unchanged, prose naming no vanished file | **211** | **66 %** |
| of which dependencies equal | 194 | |
| of which only additions (a same-file helper that became an import) | 17 | |
| Same text and dependencies, but the prose names a file this repo no longer has: a draft, not adopted as current | 5 | 2 % |
| A dependency changed shape | 60 | 19 % |
| Same name elsewhere, text differs | 45 | 14 % |
| of which exactly one candidate | 40 | |
| No successor | 1 | 0.3 % |
| Several identical successors, none preferred | 0 | (8 before the lineage vote) |
| Several orphans claim one successor (merged helpers) | 0 | (the check was added after review; none here) |
| The successor already has written prose | 0 | (likewise) |

Ten orphaned file documents sit behind these; their summaries are not adopted.

### Against the protocol

| Measure | Result | Verdict |
|---|---|---|
| **Reach** | 66 % adoptable as current. Counting the 60 whose only blocker is a changed dependency as "re-key and mark stale with the old prose as a draft", 276 of 322 (86 %) have a same-text successor. | Pass (bar: 50 %) |
| **Saving** | The regeneration of `src` needs 494 generations (`get_spec_coverage`); adoption removes 211 of them, 43 %, at the measured 1.5k tokens per cycle (`NORTHSTAR.md` K7) at most, so up to roughly 315k tokens, and about 170k at a lower per-symbol cost. A further 105 symbols (5 naming a vanished file, 60 changed dependency, 40 single text-differs candidate) become revisions of a draft instead of writes from nothing. | Pass |
| **Correctness** | 5 of the 216 symbols with the same text and unchanged dependencies (2.3 %) have prose that names a file this repo no longer has, so they are drafts and none of the 211 adopted as current does (a stale aside such as "the same wrapper `extract.rs`, `features.rs` and `rust.rs` define"); the check is exact, because it only counts names that are an orphaned spec document. Of the 20 the broad heuristic first flagged, 15 were illustrations (`lib/utils.ts`, `app/page.tsx`, `a/b/C.java`, `crud.py`) and are fine. A sample of 8 (every 27th of the 216) read against the current code: 8 of 8 accurate, one of them carrying a stale file name the check catches. | Pass |
| **Quality** | Limited: not the planned ten. For the three symbols I had also written fresh in the same session (`join`, `parent_dir`, `is_crud_module`) the old and the new prose agree; the fresh text states edge cases the old does not (only one leading `./` is stripped; a top-level `crud/` directory does not match), because the current prompt asks for limits. The old prose is a little less precise, not wrong. | Pass, thin evidence |

### What the numbers also say

- **The dependency check must compare interface hashes, not ids.** `deps_hash` hashes the target ids, so it cannot match once a dependency moves; a test (`a_dependency_that_only_moved_keeps_the_same_hash_and_still_adopts`) pins this.
- **A split produces only additions.** 17 symbols called a helper that used to be in the same file and is now an import; strict equality would have rejected all of them for a harmless reason.
- **59 of the 60 "dependency changed" are one type.** `src/graph.rs::Graph` changed shape in the same restructuring (a method moved out of it), so those specs would have been stale without any move. The rule is working as designed, but it is conservative: the prose of, say, `render` does not care that `Graph` lost a method.
- **The lineage vote was worth building.** Eight helpers duplicated across the packs (`text`, `field_text`, `node_lines`, `disambiguate_colliding_slugs`) had several identical successors; sending each to the file where the rest of its old file went resolved all eight, where path affinity (`java.rs` to `java/mod.rs`) resolved none of them, since `extract.rs` became `typescript/mod.rs`.
- **The 40 single-candidate "text differs" are moves with a small edit** (a visibility change, a trimmed comment): the natural source of a draft.

### A defect found on the way

63 stray `</content>` tags in 5 of the 39 stored spec files, left by an earlier generation at the end of a section, and not caught by the prose checks. Any carry-over would copy them, and a new `prose_smells` entry would stop a new one being submitted.

### Review changes

The first version of the planner counted 216 as adoptable. A review of the pull request found three things the headline had to respect: an orphan whose prose names a vanished file is a draft by the design rule (it was in the 216); two orphans can resolve to one successor, and only one spec can live there (none do here); and a successor that already has written prose must not be overwritten (none here). All three are now buckets of their own, with tests; the first moved 5 symbols out of the headline, 216 to 211. The report also now counts the rollup and feature orphans it does not adopt (0 here), so it does not read as the whole orphan population.

## Decision

Build the feature as agreed (`DECISIONS.md` question 26): the planner stays, and gains a write step that re-keys what is safe and a prune of what is left. The rules the measurements support:

1. Adopt as **current**: a single successor that has no written prose of its own and that no other orphan claims, dependencies equal or only additions, prose naming no vanished file.
2. Adopt as **stale with the old prose as `previous`** (the writer revises): a dependency changed, or the prose names a vanished file, or the text differs and there is exactly one same-name candidate.
3. Never adopt: nothing, several candidates after the lineage vote, a hand-edited spec (its prose hash does not match).
4. File, rollup, system and feature summaries are regenerated, not adopted.
5. Strip or refuse a stray `</content>` before carrying prose over.

## The write step on a copy

`codeowl adopt-moved-specs <copy> --write` on a copy of the repository (the stored specs and the code as they stand on the branch), nothing written to the real corpus. Before and after, from `coverage()`:

| | Before | After |
|---|---|---|
| File specs current / stale / missing | 1 / 24 / 31 | 1 / 51 / 4 |
| Generations still needed | 566 | **358** |
| Orphaned documents | 10 | 5 |
| Spec files on disk | 39 | 61 |

The fall of 208 generations is exactly the 208 carried as current. The 103 carried as drafts still need a generation each, but a revision of given prose and not a first writing. Carried: 208 current, 103 drafts; not carried: 5 naming a vanished file and 5 with several same-name candidates (their documents kept), 1 with no successor (nothing to carry). Removed: 5 orphaned documents and 44 sections from files that still exist. All 52 file specs parse afterwards; no staging file is left; the next run reports the 5 documents it kept and nothing else.

The planner's 211 adoptable became 208 here because three of the moved functions (`prose_smells`, `symbol_prose_is_human_edited`, `file_prose_is_human_edited`) were edited on the same branch, so their old prose describes text that is no longer there. That is the source-hash match being exact, and the reason the real run belongs after the code is final.
