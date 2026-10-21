# exp-06 — Does a feature spec miss participants that sit two or more call hops out? (the q18 trigger)

**Status:** measured 2026-10-21 on Java (Quarkus), Python (FastAPI) and TypeScript (Next.js); see "What this does not cover".
**Feeds:** `DECISIONS.md` q18 (should CodeOwl build a call graph), `NORTHSTAR.md` measurement log.
**Not a decision doc.** The outcome is recorded in q18; where the two disagree, q18 wins.

---

## Why this was run

q18 (2026-09-29) logged "build a call graph?" as *not built* and named its own trigger for revisiting:

> on a real dogfooded repo, how many real feature specs are missing a participant that's two or more call hops from the entry point

The trigger was never measured. On 2026-10-21 a comparison against Graphify (a third-party code-graph tool, v0.9.93, `Graphify-Labs/graphify`) raised the question again, because Graphify records function-level `calls` edges that CodeOwl does not. This experiment runs the missing measurement, plus a second, separate measurement of the *consumer* side (what `get_callers` cannot see), because the Graphify run produced the data for it at no extra cost.

## The two questions, kept apart

| | Question | Who it matters to |
|---|---|---|
| A | Does `assemble_participants` leave out files that a real request path reaches two or more call hops from the entry point? | Feature-spec correctness (q18's trigger) |
| B | How much of the real call structure is invisible to the file-level `get_callers` / `get_callees`? | An agent asking "who calls this" |

## Method

**Subjects (A):** three repos, one per stack with a feature model. Java first: `quarkus-super-heroes` (`/home/sidd/dev/openSource/test-repos/quarkus-super-heroes`), four services plus a UI. The Java stack has a feature model, so `enumerate_entry_points` returns **39 entry points in 12 distinct entry files** (38 HTTP routes and 1 Kafka consumer). The entry points of one file share one participant set, because `assemble_participants` is keyed on the entry *file*.

**CodeOwl's answer:** a throwaway example, `probe_q18.rs` (untracked, deleted afterwards), called `RepoIndex::open`, `feature_model_for`, `enumerate_entry_points` and `assemble_participants`, and printed `core`, `dependencies` and `data` per entry point. Paths were normalised (ids carry `::Symbol` suffixes and a source-root prefix) before comparing.

**Two independent oracles for "what the request path really reaches".** Each has a blind spot, so both were used and every disagreement was read by hand.

| Oracle | How | Blind spot |
|---|---|---|
| 1. Graphify call graph | `graphify extract . --code-only` (`GRAPHIFY_BACKEND=none`, no LLM) on a `git archive` copy of the repo. Breadth-first over `calls` edges from every callable in the entry file; project each reached callable to its file; keep production sources (`/src/main/`) only. Hop count = call hops. | Name-matched, not type-resolved: misses calls through interfaces and injected fields it cannot match, and may add false edges. |
| 2. Textual reference closure | File A reaches file B if A's code (imports and `package` lines removed) contains B's class name as a whole word. Breadth-first from the entry file within the module's `src/main/java`. | Over-approximates: a class name inside a string, a comment or a same-named generated type counts as a reference. |

**Measurement B:** Graphify's graph of CodeOwl's own tracked files (1,652 nodes, 4,660 edges), compared with CodeOwl's resolved imports (`graph.imports()`, printed by a second throwaway probe as file pairs).

## Results

### A. Participants missed by feature specs

| Check | Result |
|---|---|
| Oracle 1 (Graphify), 12 entry files | **0 files missed** at any hop. Deepest reach: 4 call hops (`FightResource` to `FightService` to `HeroClient` to `HeroRestClient`), all present. |
| Oracle 2 (textual), 12 entry files | 5 reported misses on 4 files. Read by hand: **3 false positives, 1 real miss.** |
| The 3 false positives | `hero/Hero.java` and `villain/Villain.java`: the word appears only inside a log string (`"Hello Hero Resource"`). `narration/Fight.java`: the resource imports the *generated* `narration.api.model.Fight`, which CodeOwl does include; the domain class of the same name is unrelated. |
| The 1 real miss | `fight/client/NarrationImageGenerationRequest.java`, a record used as a DTO. Three hops from `FightResource` and `HelloFightResource` (resource, then `ImageGenerationRequestMapper` or `NarrationClient`, then the record). Both of those are `dependencies` (tracked by interface hash), and the walk does not go a hop past a dependency. It affects the 12 of 39 entry points that live in those two files. |
| Stored feature spec read for narrative quality | `http-get-api-fights-randomfighters.md` correctly narrates all four hops (resource, service, client, REST client, remote service) and the 404 recovery rule. `kafka-fights.md` was not checked beyond its participant list. |

**Reading:** the trigger in q18 is **not met**. One DTO, three hops out, in one of the five services; no service class, client or mapper was missed.

### A2. Python (FastAPI) and TypeScript (Next.js), same two oracles

Subjects: `full-stack-fastapi-template/backend` (23 routes in 5 route files) and the pilot `talentTrail` (79 entry points; 13 are test-only API routes under `app/api/test/` and were excluded, leaving 66 production entry files, one entry point per file). Oracle 1 is Graphify again. For TypeScript, oracle 2 (name-based) was too noisy to use (it flagged 62 of 66 files; most "misses" were shared helper names), so TypeScript rests on oracle 1 plus the code reading below.

| | Python (FastAPI) | TypeScript (Next.js) |
|---|---|---|
| Entry files measured | 5 | 66 |
| Oracle 1: entry files with at least one file missed | **0 of 5** | **36 of 66** |
| Oracle 1: (entry, file) pairs missed | 0 | 160 |
| Oracle 2 (name-based) | 7 files flagged; all name collisions (`config`, `main`, `init` matching `alembic/env.py` and `initial_data.py`); none real | not usable (see above) |

**Python:** the route files reach `crud`, `deps`, `db`, `config`, `security`, `models` and `utils`, and every one is a participant. No miss.

**TypeScript, by cause.** I split the 160 missed pairs by asking: would they be participants if every file a `core` file imports (in any import form) counted as a one-hop participant?

| Cause | Pairs | Distinct files | Reading |
|---|---|---|---|
| **A. Imported by a core file but dropped.** All 57 are imported with a *default* import (`import Header from "@/components/header"`), e.g. `Header`, `Footer`, `CompetitionsList`, `ResultsList`, `JudgeProgressTable`, `ArtworkGrid`, one hop from the page | 57 | not counted | A tier-rule gap. In `assemble_participants`, a default import is never an `imports()` edge; it only feeds the JSX walk, and a component that is not co-located or does no data work never reaches `core`. A target that is a *file* is skipped when building `dependencies` (the code comment says "feature-participant granularity for module imports is an M19 question"). So the file appears nowhere in the spec, not even as a stub. `ARCHITECTURE.md` says such components "stay one-hop stub dependencies"; the measurement shows they do not. Checked by hand on `app/about/page.tsx`: it renders `<Header />` and `<Footer />` and neither is a participant. |
| **B. Beyond the one-hop frontier (UI primitives and utilities)** | 53 | 10 | `lib/utils.ts` (`cn`), `components/ui/button|card|badge|dialog|alert.tsx`. Low documentation value. |
| **C. Beyond the frontier (telemetry)** | 6 | 2 | `hooks/use-analytics.ts`, `lib/analytics-utils.ts`. |
| **D. Beyond the frontier (domain)** | 44 | 20 | `lib/auth.ts` (reached through `getJudgeContext`), `lib/email/*` (config and templates behind the registration route), `lib/supabase-browser.ts`, `lib/date-utils.ts`, `lib/grade-utils.ts`, and domain components such as `components/judge/artwork-card.tsx` and `anonymous-code-badge.tsx`. Present in **21 of 66** entry files. |

**Reading for TypeScript:** by the letter of q18's trigger it *is* met (35 of 66 entry files miss at least one file two or more call hops out; 21 of 66 miss a domain file). But in TypeScript a call into another file requires an import, so every one of these files is already reachable through plain imports. What is missing is a depth and tier policy, not call information. A call graph would not have found anything the import graph does not already contain.

### A'. A different finding, not asked for: over-inclusion

| Entry file | Participants CodeOwl lists | Reached by a real call path (oracle 2) | Cited in the stored spec's prose |
|---|---|---|---|
| `FightResource.java` (7 routes) | 35 | 22 | 13 of 27 distinct class names (34 participant entries), spec for `GET /api/fights/randomfighters`; the count is a whole-word match, so a class named `Fight` may be matched by the plain word |

Every route in a resource file gets the whole file's participants. The `randomfighters` spec lists 34 participant entries (27 distinct class names) and names 13 of those. An edit to code that only the narration or image routes use still makes the `randomfighters` spec read stale. **Not measured:** how often that happens in practice. This is exposure, not a measured cost.

### B. What the file-level graph cannot see (CodeOwl's own repo)

| Number | Value |
|---|---|
| Graphify call edges | 1,893 |
| of which inside one file | **1,739 (92 %)** |
| across files | 154 |
| cross-file call edges whose (caller file, callee file) pair has a CodeOwl import edge | 138 of 154; **112 of 112** between `src/` files (the other 16 involve tests or examples) |

So at **file** granularity CodeOwl loses almost nothing. What it loses is everything below that: which function calls which, and every same-file call. That matches the documented limit of `get_callers` ("a method, an associated/static function or a same-file helper gets no edge").

### Precision of Graphify's call edges (exposure, not error rate)

| Repo | Call edges | Marked `INFERRED` | Edges whose callee's bare name is shared by more than one callable |
|---|---|---|---|
| CodeOwl (Rust) | 1,893 | 140 (7 %) | 488 (26 %) |
| quarkus-super-heroes (Java) | 952 | 507 (53 %) | 567 (60 %) |

Name sharing is an upper bound on how many edges *could* be wrong, not a count of wrong ones. In 1,893 CodeOwl edges, only 2 have a callee name that does not appear at the call-site line. Observed gap: Graphify gives the trait method `StackPack::file_fingerprint` no call edge to its implementations (trait dispatch).

## What this does not cover

- **One repo per stack, and one of them tiny.** FastAPI template: 5 route files. A larger Python or Java service might behave differently; Java's one-hop `@Inject` shape is the least tested outside Quarkus.
- **TypeScript oracle 1 was read, not fully verified.** The call chains it produced were printed and the `Header`/`Footer` case was confirmed in the source; the other 150-odd pairs were not each opened. Category B/C/D labels are by file path.
- **Rust has no feature model** (CLI), so question A does not apply to CodeOwl's own repo.
- **Oracle 1 is itself a name-matching graph.** A miss in both oracles would be invisible here. Oracle 2 narrows that risk but over-approximates.
- **Hops are measured on the file projection** of a method-level graph; a hop count of 3 means three call steps, not three import steps.
- **Only two stored feature specs exist** for this repo; their prose was read for one of them.
- **No measurement of the other three q18 benefits** (targeted invalidation, per-symbol `### Depends on` accuracy, whether specs would read better). Those stay as q18 argued them.

## What follows

1. **Verdict: do not build call edges.** Java: one DTO. Python: nothing. TypeScript: many misses, but all are reachable by imports, and the cause is a tier rule, not missing call data. The remaining case for call edges is B (agent navigation), which q18 already said an IDE's language server answers and which needs a viewer to be differentiated.
2. **Two findings that belong to the participant rules, not to a call graph** (candidate questions, not decisions, not built): (a) default-imported components that are not co-located and do no data work appear nowhere in a feature spec (57 TypeScript pairs; now `DECISIONS.md` q23); (b) participants are keyed on the entry file, so the routes of one file share an over-large set (the over-inclusion section below). A third, weaker one: whether the frontier should go one hop past a dependency for non-primitive files (the 44 domain pairs).
3. If call edges are ever built, the numbers above fix three design constraints: same-file calls are 92 % of the volume; trait/interface dispatch must be marked, not guessed; and an edge needs a confidence tag so the 26 to 60 % name-ambiguity exposure is not served as fact.

## Reproducing it

```
# Graphify (isolated; nothing touches the repo)
uv venv gfy-venv && uv pip install --python gfy-venv/bin/python ./graphify-src   # clone of Graphify-Labs/graphify, v0.9.93
mkdir copy && git -C <repo> archive HEAD | tar -x -C copy
cd copy && GRAPHIFY_BACKEND=none ../gfy-venv/bin/graphify extract . --code-only --no-cluster   # writes graphify-out/graph.json (keys: nodes, edges)

# CodeOwl participants (throwaway example, ~20 lines)
let (_i, graph, _) = RepoIndex::open(Path::new(&root))?;
let fm = feature_model_for(&graph)?;
for e in fm.enumerate_entry_points(&graph) { let p = assemble_participants(&graph, fm, &e); /* print core, dependencies, data */ }
```

Comparison rules: normalise ids to the part after `io/quarkus/sample/superheroes/` and drop `::Symbol`; keep Graphify callables as nodes whose label ends in `()`; keep production sources only.

**Cache check.** `RepoIndex::open` reuses a repo's `.codeowl/` cache when the format version matches, so the first runs could have read an old cache. The CodeOwl side was therefore rebuilt from copies of all three repos with `.codeowl/`, `node_modules/` and `.git/` removed, using the release build at `FORMAT_VERSION` 23. The participant lists were identical for all 141 entry points (79 TypeScript, 39 Java, 23 Python). Input difference that remains: Graphify read the committed files (`git archive HEAD`), CodeOwl read the working tree; for `talentTrail` that differs only in `CLAUDE.md` and an untracked `docs/`, and for Quarkus CodeOwl also sees generated sources that Graphify did not.
