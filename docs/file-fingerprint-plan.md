# Plan: the file fingerprint and its migration (exp-05 steps 5 and 0d)

Status: **approved by the owner 2026-10-20, with all four open questions answered as proposed (below). Nothing is built.** Branch `file-fingerprint` (cut from `master` at the #91 merge). Tests first.
Decided already (owner, 2026-10-04; `DECISIONS.md` q22 point 5, exp-05 Conclusion 14): the rule is **shape + doc comments + body literals (rule L2) on every file**, regardless of which input rung the writer used. All items are now decided; see Decisions below.

## Goal
A file summary goes stale when something it could plausibly quote changed, not on any byte. A comment-only commit took the corpus from 100 % to 40.2 % current; under the chosen rule about 21 % of CodeOwl's own Rust edits would have spared their file summary (replay of 342 edits, round 7).

## What the code does today (verified 2026-10-20)
- A file's staleness key is `FileNode.source_hash`, the hash of the **whole raw text** (`file_changes`, `src/spec.rs`: it compares `source_hash` and the file's `deps_hash` against the stored `HashPair`).
- Other readers of that same raw hash, which must **not** move:
  - `get_source`'s freshness check (`graph_in_sync`, `mcp.rs`) and the incremental reindex (`index.rs`).
  - Feature specs: `current_participant_hashes` records each core file's raw hash, because a feature is written from the raw core sources.
  - `interface_or_source_hash` for a file used as an import target (the demoted import-hash fix).
- **Rollups and the system spec already follow the file spec, not the raw hash.** `current_file_hashes` returns a file's `spec_hash` only while `file_changes` is empty. So once a file summary stays current, its rollup and the system spec stay current with no extra work. One exception to fix: `next_rollup_task` compares `s.file.source_hash == file.source_hash` directly (`spec.rs`, ~line 2556).
- `FileInputs` in `index.rs` caches each file's `source_hash`, symbols and imports; `RepoIndex::rebuild` turns them into `FileExtraction`, then `Graph::build`. The graph and index caches are stamped by `FORMAT_VERSION` (20).
- `Symbol` already carries `signature`, `docstring`, `is_exported`, `kind`, children and a per-symbol `source_hash` (a `Value`'s text includes its value). `leading_comment_block` (added by the ladder) finds the module doc. `FileImports` is serialisable.

## Design

### The fingerprint (what is hashed)
One hash per file, `FileNode.fingerprint`, over, in file order:
1. **Shape:** for every symbol, recursively through members: id, kind, signature, export flag; for a `Value` symbol also its source hash (so a constant's value counts). The file's imports (`FileImports`, serialised).
2. **Doc comments:** every symbol's docstring, and the module doc (`leading_comment_block`).
3. **Literals (rule L2), per pack:** the numbers other than 0, 1 and 2, and the strings of 8 or more characters, written inside function bodies, outside any test module. Compared as a *sorted list* (a multiset), so moving code around does not count by itself; reordering the symbols still changes the shape part.
Everything else is ignored: ordinary comments, whitespace, local names, which calls a body makes.

### Where it lives
- **In memory:** `FileExtraction.fingerprint` and `FileNode.fingerprint`, computed by a new `StackPack::file_fingerprint(rel_path, source, symbols, imports) -> String`, cached per file in `FileInputs` so a load does not reparse.
- **Default for a pack with no literal rule yet** (decided): `hash_text(source)`, the whole text. That is today's behaviour, so it can only over-report staleness, never hide a change. Each pack switches to the real rule when its literal node kinds are added and tested.
- **On disk (decided, option B):** the file line gains a key: `file: { source_hash, fingerprint, deps_hash, spec_hash }`. `source_hash` stays (raw text at generation time, still useful: `explain_stale` can later say "the text changed but the fingerprint did not"). A spec with **no** `fingerprint` key is *legacy*: it reads as stale until it is migrated or regenerated. The alternative (reuse the `source_hash` key to hold the fingerprint) needs no parse change but makes the key a misnomer and makes "old or new?" undecidable; I recommend against it.
- `file_changes` compares `fingerprint` (its cause label stays `source`). `submit` for a file records it. `next_rollup_task` uses `file_changes` instead of the raw comparison.

### `FORMAT_VERSION` 21, then 22 (and one more bump per later pack rule)
The graph and index caches gain a field and a changed hash scheme. Bump to 21 with its entry in the constant's doc comment (the project rule), in the commit that adds the field. The Rust rule is a pack-logic change that moves cached fingerprints, so it bumps again to 22 in its own commit (the project rule); each later pack's rule does the same.

### The migration tool (exp-05 step 0d)
`codeowl migrate-file-fingerprint <path> [--write]`, a dry run unless `--write`, modelled on `backfill-dep-targets`:
- For each file spec **without** a `fingerprint` key: it is migrated only if it was **current under the old definition**, i.e. the stored `source_hash` equals the graph's raw hash **and** the recomputed `deps_hash` equals the stored one. Then `fingerprint: <current>` is added to its file line and nothing else changes.
- A spec that was already stale stays stale. A spec that already has the key is skipped (idempotent). Unreadable specs are counted and left alone. CRLF line endings are kept. No LLM, no tokens.
- Report: migrated, already migrated, stale (left), unreadable.
- It is run on CodeOwl's own `docs/specs/` as **its own commit**, like #90.

## Tests first (each written failing, then made to pass)
**Plumbing and cache**
1. A pack with no rule gives `hash_text(source)` as the fingerprint; the graph's `FileNode` carries it; it survives the index save and load.
2. `FORMAT_VERSION` is 21 and the doc comment has the entry (a test reading the constant's doc is overkill; the entry is checked in review).

**Frontmatter and staleness**
3. A file spec round-trips with and without `fingerprint`; a legacy spec (no key) parses.
4. `file_changes` reports `source` when the fingerprint differs, nothing when it matches, and reports `source` for a legacy spec.
5. `submit` for a file stores the current fingerprint.
6. A rollup stays current when a file's raw text changed but its fingerprint did not (the cascade benefit); `next_rollup_task` uses the same test.
7. The lifecycle test `leaf_body_edit_stales_exactly_its_file_and_containing_rollup` is **TypeScript, and TypeScript keeps the whole-text fingerprint** (decision 1), so its assertions still hold and it is not rewritten in this commit. The sparing behaviour is covered instead by a Rust lifecycle test added with the Rust rule (commit 3, in `tests/lifecycle_matrix.rs`'s style): a body edit that changes a long string stales the file and its folder; one that changes no literal stales only the symbol and leaves the file, folder and a sibling current. The TypeScript test is rewritten in the commit that gives TypeScript its rule. (Corrected 2026-10-20.)

**The Rust rule** (each a separate small test)
8. Spared (fingerprint unchanged): a changed ordinary comment; whitespace and formatting; a renamed local variable; a body change that adds or removes calls but no literal; a literal 0, 1 or 2 changed to another of those; a short string (7 characters or fewer) changed; an edit inside the `#[cfg(test)]` module.
9. Staled (fingerprint changed): a signature, an added or removed item, an export flag, a constant's value, an import; a doc comment or the module doc; a number other than 0, 1, 2 in a body; a string of 8+ characters in a body; reordering two functions.
10. Sorted-multiset: moving a literal from one function to another leaves the fingerprint unchanged (decided).

**Migration**
11. A current legacy spec gets its `fingerprint` line and no other change; a stale one is left; running it twice changes nothing; CRLF kept; an unreadable spec is counted; a dry run writes nothing.

## Commit sequence (each compiles and passes `utility/check.sh`)
1. Plumbing: `StackPack::file_fingerprint` (default), `FileExtraction`/`FileNode`/`FileInputs` field, `FORMAT_VERSION` 21 and its entry (tests 1).
2. Spec side: `HashPair.fingerprint`, parse and render, `file_changes`, `submit`, `next_rollup_task`, `explain.rs` (a legacy spec is explained as such, not as an edit) (tests 3 to 6). With the default fingerprint this is behaviour-neutral for fresh specs; legacy specs read stale until migrated.
3. The Rust rule (tests 8 to 10) and the Rust lifecycle test.
4. The migration command and its tests (test 11).
5. Run the migration on CodeOwl's own `docs/specs/`, as its own commit: only file-line `fingerprint` additions. I report the counts before committing.
6. Docs: `ARCHITECTURE.md` ("Caching and invalidation"), `GLOSSARY.md` (the fingerprint), `setup/USAGE.md` (the new command), `DECISIONS.md` q22, exp-05 status.
7. **Measurement, no tokens:** a throwaway replay probe (untracked, deleted after) over CodeOwl's Rust history to confirm the real implementation spares about 21 % (gate), and the comment-only scenario. Result recorded in `NORTHSTAR.md`'s measurement log if you want it there.

## Decisions (owner, 2026-10-20: all four as proposed)
1. **Other stacks:** Rust first. TypeScript, Java and Python keep the whole-text default (safe, no sparing) until each gets its literal node kinds in its own later commit with its own tests.
2. **On disk:** a separate `fingerprint` key on the file line; `source_hash` stays as the raw text hash. A spec without the key is legacy and reads stale until migrated.
3. **Literals** are compared as a sorted list (test 10 stays).
4. **Feature specs** keep the raw core-file hash and do not benefit.

## Risks
- **Accepted trade-off (owner, 2026-10-04; not a risk to re-raise):** a body edit with no new number or long string leaves the file summary current; the symbol's own spec still stales.
- **Legacy specs read stale** after merge until the migration is run on a repo. Run it before the next generation. CodeOwl's own corpus is migrated in this branch; the pilot repo (`talentTrail`) is not touched by it.
- **The literal rule is per pack.** A wrong node-kind list in a pack under-detects. Each pack's rule needs its own tests; Rust's is the only one backed by a replay.
- A fingerprint bug could hide staleness silently. The migration only ever adds a value for specs it can prove current, and test 4 pins the legacy path.
