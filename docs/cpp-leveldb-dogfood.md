# C++ dogfood on `leveldb` (step 8, first half)

Written 2026-11-20 before the run, approved by the owner the same day (a fresh session in the copy; budget 25). The first time a C++ spec is written from what the pack hands the writer. Results go in section 6 after the run.

## 1. Why, and what it can show

Every number so far (K11, pairing share, ids) measures the pack against the compiler. None shows what a spec built on it is like, and specs are the product. `leveldb` is the safe place: C++ only (it indexes without step 2c), 100 % of its qualified definitions pair, no id lacks its namespace, so later extraction work will not orphan what is written here. A spec reveals three things the numbers cannot: whether the text the writer is handed is enough (above all for a class whose behaviour is its bodies), whether the dependency list is right, and whether pairing put the right bodies with the right declaration.

## 2. Setup (done)

| | |
|---|---|
| The copy | `../dogfood/leveldb`, cloned from the accuracy check's copy at `7ee830d` (its pinned commit), so the check's clone stays clean. Its `.codeowl/`, `.claude/`, `.mcp.json` and the brief are excluded from git there |
| The server | `target/release/codeowl serve .`, built from `master` at `2b7a7c0` (no pack change since the controls were baselined at `a7c6ea0`) |
| The writer | A fresh Claude Code session opened in the copy, with `/codeowl-generate` copied from `setup/` |
| Baseline | 124 coverage items, **841 generations remaining** (116 files, 7 rollups, the system spec), nothing written |

## 3. The run: nine targets, 25 generations

`--all --budget=25` would spend the budget on the two highest-fan-in headers (`dbformat.h` 11, `filename.h` 13) and never reach a class worth reading. So named targets, one `/codeowl-generate <file>` each, in this order, chosen to cover what the pack does differently:

| # | Target | Gens | Why it is here |
|---|---|---|---|
| 1 | `db/db_impl.h` | 3 | `DBImpl`: 48,970 bytes with its bodies, about 6 times the 8,000-byte cap, so the task is the outline. **The one that matters most** |
| 2 | `db/version_set.h` | 6 | `VersionSet` (26,758 bytes) and `Version`: the second large paired class, and several symbols in one file |
| 3 | `db/builder.h` | 2 | A free function with its prototype in the header and its body in a `.cc` (the free-function pairing rule) |
| 4 | `db/log_reader.h` | 2 | A medium class (11,656 bytes) whose bodies are in the `.cc` |
| 5 | `db/memtable.h` | 2 | A medium class, bodies split between the header and the `.cc` |
| 6 | `include/leveldb/status.h` | 2 | Small and clean; the public API's value type |
| 7 | `table/block.h` | 2 | Small class with a nested iterator class |
| 8 | `util/arena.h` | 2 | Small; bodies partly inline |
| 9 | `include/leveldb/slice.h` | 4 | A class whose bodies are all in the header: the control, nothing paired |
| | **Total** | **25** | |

(25 generations is about 3 % of the 841 the whole repository needs; the first plan wrongly said 20 %.)

## 4. What the owner reads, and what I check beside it

The owner reads about ten of the specs written (all of 1, 2 and 3, and a few of the rest), asking of each: **would this help an engineer who has not read the code, and is anything in it false?**

| Check | By whom | Method |
|---|---|---|
| Every submission passes the deterministic quality check | the writer, recorded | The result of each `submit_spec` |
| Claims in the `DBImpl` spec against the code | me | Each behavioural claim checked against `get_source` on the member it names (a sample of at least 15) |
| Whether the outline-only task was enough | the owner and me | Did the writer ask for `get_source` or read bodies to fill gaps (count the calls)? Would the spec have been wrong or thin without them? |
| The dependency list | me | Each listed dependency used by a body, and each obvious one missing |
| A pairing that put a body with the wrong declaration | me | Any spec describing a signature the header does not declare (the 13 % overload limit) |
| Cost | the writer, recorded | Tool calls per generation, pages followed, the largest task |

## 5. How the result is judged

A table of findings, each marked **pack** (what the writer was handed: a missing body, a wrong overload, a missing or false dependency, a truncated source) or **writer** (a claim the text did not support). Pack findings go to the plan with a proposed fix; writer findings go to `setup/codeowl-generate.md`'s prompt only if they repeat. The owner's verdict decides whether the next step is step 3 (the macro table), step 4 (precision) or step 2c. **Nothing in the pack changes during the run**, so the controls are measured once, afterwards (`utility/cpp_accuracy/run.sh --controls`), against the baseline of 2026-11-20.

**Rules for the session:** do not edit `leveldb`'s code (the specs must stay current); do not hand-edit a spec; write what the task gives you and no more; note, never fix, anything the pack does that looks wrong; stop after the nine targets whatever the budget says.

## 6. Results

(To be written after the run.)
