# CodeOwl — North Star

> How CodeOwl compares with tools that solve the same problem, and the
> measurable numbers that say whether it is getting better. Companion to
> `REQUIREMENTS.md` (what and for whom) and `ROADMAP.md` (build sequence):
> a milestone that moves a KPI cites it by id ("moves K2").

## How to read this document

Two different kinds of content live here, and they must not be confused:

- **The matrix (§2, §3) is opinion.** Scores of 1 to 5 are a judgment made on
  a stated date from public information, except where a row cites a published
  number. It is re-scored deliberately, as a whole, never edited cell by cell.
- **The KPIs (§4) are measurements.** Each has a method that anyone can
  re-run, a baseline, and a target. KPIs drive the work; the matrix only
  explains why those KPIs were chosen.

Measurements are appended to the log (§5), never overwritten, so progress is
history rather than a number that silently changed.

Rules for the competitor content, since this repo is public: state only
sourced facts or clearly labelled judgments about other products, cite the
source, and date the snapshot. Vendor claims are marked as vendor claims.

## 1. The problem being compared

Accurate, current, human-readable documentation of how a brownfield codebase
works, for the people who work on it (devs, BAs, QA, SREs). Tools that solve
a neighbouring problem (code retrieval for coding agents) appear only where
they share a parameter.

## 2. Parameters

Each parameter carries a stance, so the matrix never pulls against a decision
already made in `REQUIREMENTS.md`:

- **pursue**: a gap we intend to close; has KPIs.
- **defend**: a strength the design depends on; has KPIs so it does not erode.
- **deferred**: a real gap, but scheduled for Phase 2; tracked, not chased.
- **non-goal**: deliberately not competed on; scored for honesty only.

| # | Parameter | Definition (what a 5 means) | Stance | KPIs |
|---|---|---|---|---|
| P1 | Verifiable freshness | A reader can tell mechanically, per section, whether a doc still matches the code, and what changed | defend | K1a, K1b, K2, K3 |
| P2 | Measured accuracy | Doc quality is measured on a public benchmark or a repeatable human check, not asserted | pursue | K4, K5 |
| P3 | Language breadth | Works on any mainstream language and on polyglot repos | pursue | K6 |
| P4 | Cross-cutting flows | Features that cross files and layers (UI → API → DB) are described from real edges, not guessed | defend | K6 |
| P5 | Reading experience for non-developers | A BA can browse, search and follow diagrams without an IDE | deferred (Phase 2 web viewer) | — |
| P6 | Privacy / locality | No new place source code is sent; runs on the user's machine | defend | — (a design invariant: CodeOwl never calls an LLM) |
| P7 | Cost | Low first-run and upkeep cost in tokens, money and time | pursue | K7 |
| P8 | Setup effort | From nothing to a first useful spec in a few steps | pursue | K8 |
| P9 | Scale / multi-repo | Large repos and many repos per team | deferred (Phase 2) | K9 (tracked) |
| P10 | Maturity / adoption | Used by people other than its author | pursue | K10 (tracked, no target) |
| P11 | Docs in git | Docs are plain files, versioned with the code, reviewed in PRs | defend | — (holds by design) |
| P12 | Use as agent context | Agents demonstrably do better with it | non-goal (no claim made; see `REQUIREMENTS.md` "Users / consumers") | — |

## 3. Competitor snapshot — 2026-10-02

Scored by Claude Opus 5.5 with the owner, from public information on
2026-10-02. **Judgment unless the cell is marked †** (published number,
see the table below). "?" means not enough information to score.

| Parameter | CodeOwl | DeepWiki | Google Code Wiki | Swimm | Mintlify / DocuWriter | OSS generators (CodeWiki, deepwiki-open) | Graph MCPs (GitNexus, CodeGraphContext) |
|---|---|---|---|---|---|---|---|
| P1 Verifiable freshness | 4 | 2 | 3 | 4 | 3 | 1 | n/a |
| P2 Measured accuracy | 2 | 3 † | ? | 4 (human-written) | ? | 2–3 † | n/a |
| P3 Language breadth | 2 | 5 | 5 | 5 | 4 | 4 | 4 |
| P4 Cross-cutting flows | 4 | 3 | 3 | 2 | 1 | 3 | 3 |
| P5 Reading experience | 2 | 5 | 5 | 4 | 5 | 3 | 2 |
| P6 Privacy / locality | 5 | 2 | 2 | 3 | 2 | 4–5 | 5 |
| P7 Cost | 4 | 4 | 5 | 3 | 2 | 4 | 4 |
| P8 Setup effort (5 = least) | 2 | 5 | 5 | 3 | 3 | 3 | 4 |
| P9 Scale / multi-repo | 2 | 4 | 4 | 4 | 4 | 3 | 3 |
| P10 Maturity / adoption | 1 | 5 | 3 | 4 | 4 | 2 | 3 |
| P11 Docs in git | 5 | 1 | 1 | 4 | 4 | 2 | n/a |
| P12 Use as agent context | 3 | 4 | 2 | ? | ? | 2 | 5 |

### Why CodeOwl scored what it did

- **P1 = 4, not 5.** Staleness keys on `interface_hash` / `deps_hash` per
  symbol, never on prose, and a stale spec names what moved. Competitors
  either regenerate everything (Code Wiki: fresh but unproven), have an LLM
  judge the diff (Mintlify, DocuWriter), or track code snippets inside
  human-written docs (Swimm, the closest analogue). Held back by two things:
  an unresolved import is no edge, so its importers never go stale (K2), and a
  dependency's behaviour change without an interface change does not stale its
  dependents (a deliberate trade-off, `ARCHITECTURE.md` "Caching and
  invalidation").
- **P2 = 2.** The design keeps graph facts out of LLM prose, and the
  deterministic smell check flags weak text, but the only quality evidence is
  the owner's sample read. Prose quality also depends on the calling agent.
- **P3 = 2.** Four stacks, one stack per repo, polyglot repos not yet
  supported.
- **P4 = 4.** `fetch` → route, `.from("t")` → `CREATE TABLE`, FastAPI
  `Depends()`, Quarkus Kafka and REST-client edges are deterministic, where
  competitors infer flows with an LLM. Only for frameworks with a feature
  model.
- **P5 = 2.** Markdown in `docs/specs/`; no web UI, diagrams or chat yet.
- **P6 = 5.** No credentials, no model calls, no new egress.
- **P7 = 4.** Free, MIT, runs on an existing subscription, incremental
  regeneration. The first full generation is expensive (unmeasured, K7).
- **P8 = 2.** Build from source, wire MCP, drive generation from an agent;
  Linux and macOS only.
- **P9 = 2.** One repo on one machine, validated on mid-size repos.
- **P10 = 1.** First commit 2026-09-05; one user.
- **P11 = 5.** Plain Markdown under `docs/specs/`, diffable in a PR.
- **P12 = 3.** Nine MCP tools over graph and specs; benefit unmeasured and not
  claimed.

### Published numbers behind the † cells and the context

| Source | Finding |
|---|---|
| CodeWikiBench, [arXiv 2510.24428](https://arxiv.org/html/2510.24428v5) (docs scored against rubrics derived from projects' official docs) | CodeWiki 68.79 %, DeepWiki 64.06 %, deepwiki-open 50.05 %, OpenDeepWiki 47.13 %. All weaker on C/C++. Staleness is listed as future work. |
| SWD-Bench, [arXiv 2604.06793](https://arxiv.org/abs/2604.06793) (QA tasks derived from real PRs) | Best documentation raised SWE-Agent's issue-solving rate by 20 %. Tools not named. |
| [2026 code-graph MCP survey](https://wal.sh/research/2026-code-graph-mcp-survey/) | GitNexus on the Emacs tree: 114× the index time of `etags`, 222× the index size, 0 Lisp symbols. PolyForm-NonCommercial license. |
| Vendor claims, not independently verified | Swimm: 2,000+ teams, Team plan $11/user/month ([toolradar](https://toolradar.com/tools/swimm/calculator)). DeepWiki: 50k+ public repos pre-indexed ([guide](https://codersera.com/blog/deepwiki-complete-guide-2026/amp/)). Augment: Context Engine as MCP, "+70 %" agent performance ([blog](https://www.augmentcode.com/blog/context-engine-mcp-now-live)). |

No benchmark found measures whether generated docs *stay* correct as code
changes. K1a is designed to be the deterministic half of that number
(dependency tracking, checked against a compiler); K1b, whether the prose
itself is still true, needs a judge and comes later.

Product references: [DeepWiki](https://codersera.com/blog/deepwiki-complete-guide-2026/amp/),
[Google Code Wiki](https://infoq.com/news/2025/11/google-code-wiki/),
[Swimm auto-sync](https://docs.swimm.io/features/keep-docs-updated-with-auto-sync/),
[Mintlify workflows](https://www.mintlify.com/blog/docs-on-autopilot),
[DocuWriter](https://docuwriter.ai/),
[deepwiki-open](https://dev.co/devops/open-source/deepwiki-open),
[GitNexus](https://cdn.jsdelivr.net/gh/abhigyanpatwari/GitNexus@main/README.md),
[CodeGraphContext](https://glama.ai/mcp/servers/@Shashankss1205/CodeGraphContext/blob/5cf5aa8eaf2d6bd6f407dca9dfa7203b157d30ba/README.md).

## 4. KPIs

"Unmeasured" means the method exists on paper but has not been run, or needs
tooling that does not exist yet. A baseline is never guessed.

| KPI | Parameter | Metric | Method | Baseline | Target |
|---|---|---|---|---|---|
| **K1a** Dependency-tracking recall / precision | P1 | When an interface changes, the fraction of the files that truly depend on it that CodeOwl marks stale (recall), and the fraction of the files it marks stale that truly depend on it (precision). Measures *dependency tracking*, not prose correctness: a body-only change breaks nothing and correctly stales no dependent, yet a caller's prose may still be wrong | Fully deterministic, no LLM and no judgment. Replay a reference repo's commits (or apply scripted mutations: a signature change, a rename, a body-only edit, a dependency's interface change) and take as ground truth the files a compiler or type-checker rejects afterwards (`cargo check`, `tsc`, `javac`, `mypy`), which is independent of CodeOwl's own hashes. Dynamic edges (`fetch("/api/x")`, `.from("table")`, `@Incoming`) break no compile and need hand-written expected edges per fixture, or stay unmeasured. Needs a harness; Rust first, on CodeOwl's own history | unmeasured | recall ≥ 95 %, precision ≥ 80 % |
| **K1b** Prose-truth recall | P1 | Of the specs whose *claims* a commit made false, the fraction CodeOwl flags stale | Needs a judgment of whether each spec's claims still hold: an LLM judge audited by a human on a sample. Not reproducible and costs tokens per run, hence separate from K1a. Deferred until K1a exists | unmeasured | none until a method exists |
| **K2** Unresolved imports | P1 | Per stack, the fraction of imports that point at a walked file but resolve to no symbol | Extend `examples/graph_stats.rs`: count unresolved imports whose path suffix-matches a walked file | TypeScript (`talentTrail`): 141 / 858 = 16.4 %. Java (`commons-lang`, `quarkus-super-heroes`), Python: 0 real misses. Rust: not yet counted this way | < 2 % on every stack |
| **K3** False isolated files | P1 | Files with no import edges in or out that do import, or are imported | `examples/graph_stats.rs` on the reference repos, then check each isolated file by hand | CodeOwl: 0 isolated (was 12 before the crate-root fix). `ripgrep`: 54 file edges, stable across runs | 0 on every reference repo |
| **K4** Benchmark doc quality | P2 | CodeWikiBench overall score of CodeOwl's generated corpus | Generate the corpus for CodeWikiBench repos in a supported stack, run its rubric evaluation | unmeasured | ≥ 64 % (DeepWiki's published score) |
| **K5** Self-corpus freshness | P2 | `get_spec_coverage` on CodeOwl's own `src`: freshness, and fan-in-weighted freshness | Run `get_spec_coverage` with `scope: "src"` | 3 current, 20 stale, 1 missing; freshness 13 %, weighted 5 % | 100 % current at every release |
| **K6** Stack and feature-model coverage | P3, P4 | Number of supported stacks; number of framework feature models; polyglot repos supported (yes/no) | Count `StackPack` and feature-model implementations | 4 stacks (TypeScript, Rust, Java, Python), 3 feature models (Next.js, FastAPI, Quarkus), polyglot: no | Polyglot: yes. Further stacks per `ROADMAP.md` |
| **K7** Bootstrap cost | P7 | Tokens and wall time per generated spec, and for a full `--all` on a reference repo | Log tokens and time across a budgeted generation run | unmeasured (pilot generation was stopped at about 15 % coverage) | set once measured |
| **K8** Time to first spec | P8 | Steps and minutes from a fresh clone to one current spec | Follow `setup/README.md` on a clean machine, timed | unmeasured | ≤ 3 steps, ≤ 10 minutes |
| **K9** Index time and size | P9 | Cold index time, `.codeowl/` size, and size of the largest repo validated | Time `codeowl` on each reference repo | unmeasured | tracked, no target until Phase 2 |
| **K10** Adoption | P10 | External users; GitHub stars | Count | 0 external users; 2 stars | tracked, no target |

## 5. Measurement log

Append a row for every measurement, even an unchanged one. Never edit an
earlier row; a wrong row gets a correcting row below it.

| Date | KPI | Value | Commit | How measured |
|---|---|---|---|---|
| 2026-10-01 | K2 | TypeScript 141 / 858 unresolved (`talentTrail`); Java and Python 0 real misses | during PR #79 (exact commit not recorded) | Throwaway counting script (deleted), same method as K2 |
| 2026-10-01 | K3 | CodeOwl 0 isolated (from 12); `ripgrep` 54 edges, stable (from a random 3 to 9) | PR #79 | `examples/graph_stats.rs` |
| 2026-10-02 | K2 | TypeScript (`talentTrail`): unresolved imports that name a walked file: **4 of 721 after the fix (0.6 %); 141 of 858 (16.4 %) before**. All 4 are one name, `VIEW_AS_JUDGE_COOKIE`, a local `export { X }` re-export | branch `ts-type-symbols` | Throwaway probe (path-suffix match against walked files), deleted |
| 2026-10-02 | K3 | `talentTrail`: file edges 478 to 574, isolated files 56 to 42. Rust, Python, Java statistics byte-identical before and after | branch `ts-type-symbols` | `examples/graph_stats.rs`, master vs branch |
| 2026-10-02 | K2 | TypeScript (`talentTrail`): unresolved imports naming a walked file **0** of 717 (0 %); was 4 of 721 after the type-symbol fix, 141 of 858 originally | branch `ts-local-reexport` | Throwaway probe (path-suffix match against walked files), deleted |
| 2026-10-02 | K3 | `talentTrail`: file edges 574 to 577; resolved imports whose target has no `interface_hash` (importers hash the whole source) 131 to 0; Rust, Python, Java statistics identical | branch `ts-local-reexport` | `examples/graph_stats.rs` and a throwaway probe |
| 2026-10-02 | K5 | 3 current / 20 stale / 1 missing; freshness 13 %, weighted 5 % | `4f72f1f` | `get_spec_coverage`, scope `src` |
| 2026-10-02 | K6 | 4 stacks, 3 feature models, no polyglot | `4f72f1f` | Count of implementations |
| 2026-10-02 | K10 | 0 external users, 2 stars | `4f72f1f` | GitHub |

## 6. Re-scoring

Re-score §3 as a whole when a stance changes, when a KPI crosses its target,
or at least once per phase. Keep the previous snapshot below this line with
its date, so the trend stays visible.
