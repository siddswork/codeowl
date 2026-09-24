# exp-04 — What makes a coding agent actually rely on CodeOwl

**Status:** spike, on paper. Written before M20/M21 freeze any tool signature or extraction shape.
**Feeds:** M20 (`get_source` + `search_code` ergonomics), M21 (member-level extraction), `ARCHITECTURE.md` open question 12, and the reframed `tantivy`/ONNX deferral in `ROADMAP.md`.
**Not a decision doc** — its conclusions fold into `ARCHITECTURE.md` (§1, §4, §7, open question 12) and each milestone's own plan as it starts. Where this file and those disagree, they win.

---

## Why this spike exists

Every milestone through M19 optimizes one loop: **generate a spec corpus, serve it to a reader.** That loop is built, validated, and working.

This spike starts from the other end — not "is the corpus good" but **"when an agent is mid-task in a repo with CodeOwl connected and current, does it actually use it?"** — and the honest answer, observed directly rather than assumed, is: not much.

The observation, 2026-09-24, working inside this repo: with the CodeOwl MCP server connected and the graph live, the agent kept reaching for `grep` and file reads instead of the eight tools sitting right there. The owner noticed and asked directly why.

This matters because the MCP server's own instruction string already tells the agent not to do that (`mcp.rs`):

> *CodeOwl is a read-only structural index of this repository — reach for it before grep/file-reading on "how is this wired" questions.*

The instruction exists and is ignored. So the interesting question isn't "how do we word the instruction better." It's **whether the instruction is ignorable because it's wrong** — because for the lookups actually being made, there is no CodeOwl answer to reach for.

Checked case by case, mostly yes.

---

## Q1 — Which lookups actually sent the agent to `grep`, and why?

Not a survey — the real calls made in one working session, each classified by whether CodeOwl *could* have answered it.

### The ones where CodeOwl genuinely had no answer

**1. "Does `resolve_imports` ever resolve to a bare file, or only to a named symbol?"**

A claim about a function's **behavior**, being checked before it got written into a document. `get_symbol` returns `signature` (the text *before* the body), `docstring`, `lines`, hashes, `children`. The body is not in the graph and no tool serves it. → `Read`.

`get_spec` is not the substitute, and this is the distinction the whole of M20 turns on. A spec is LLM-written prose *about* a symbol — the right answer to *"explain this to me"*, the wrong answer to *"verify this before I assert it."* It may be `stale`. It may carry `smells`. It may be `missing`. Under any of the three, an agent that has to be **correct** about behavior cannot substitute the prose for the code — and it can't know in advance which of the three it's about to get.

**2. "What fields does `FileNode` carry, and what does each field's doc comment say?"**

`get_symbol("src/graph.rs::FileNode")` answers `children: []`. Not because the type has no members — it has three — but because the Rust pack never walks a `struct`'s body. → `Read`.

**3. "Does `calledBy` appear anywhere in `src/`?"**

An existence sweep, to confirm a call-graph field was never implemented. `search_code` does exactly this. The agent used `Bash` + `rg` anyway — because `search_code` takes `query` alone: no path filter, no case flag, no context lines. → `Bash`, unnecessarily.

### The ones where the fallback was correct

Reading `ARCHITECTURE.md`/`ROADMAP.md` **prose** to check a design claim. Not code, not in the graph. `search_code` can find a line in them (see Q3), but reading a whole argument is a file read, correctly.

### The finding

The three misses have nothing in common technically — a missing body, a missing member kind, a thin parameter list. What they share is the *effect on a caller*:

> **An agent will not rely on a tool that answers some of its questions and silently under-answers the rest.**

One `children: []` that means *"this pack doesn't look"* rather than *"there are none"* is enough to teach an agent to open the file every time. And once it opens the file by default, it does so for the queries CodeOwl **would** have answered well, too. Reliance is all-or-nothing in a way coverage is not — which is why this is worth its own track rather than three scattered improvements.

---

## Q2 — Two things believed at the start of this spike that turned out to be false

Both were asserted confidently in conversation before being checked, both were checked against source, both were wrong. Recorded because each changes what gets built.

### "CodeOwl doesn't extract struct fields at all." — False

It extracts them in **one of four packs**:

| Pack | Container members extracted | Fields? |
| --- | --- | --- |
| Java (`java.rs`) | methods **and** fields — `field_declaration`/`constant_declaration` → one `SymbolKind::Value` per declared name, `raw: "field"`/`"constant"`; the `int a, b;` multi-declarator case handled, with a test | yes |
| TypeScript (`extract.rs`) | `class_body` filtered to `method_definition` only | no |
| Rust (`rust.rs`) | `struct_item`/`enum_item`/`union_item` go through a leaf push that never walks the body | no |
| Python (`python.rs`) | module-level assignments → `Value`, `raw: "assignment"`, `parent: None` | no (class attributes) |

**Why this changes the work.** "Build field extraction" is a feature request. "Three of four packs disagree with the fourth, and the disagreement makes `children: []` ambiguous" is a **defect report** — and it reframes M21's exit test from "fields appear" to "a caller can trust a negative answer."

It also means the `interface_hash` question (Q4 below) isn't hypothetical or future — Java has been extracting fields since M16, so whatever rule is in force is *already* being applied to a real corpus, just unnoticed.

### "`search_code` can't search the `.md` design docs." — False

`search.rs` walks every file under the repo root with no type filter. Confirmed live: `search_code("Merkle")` returned hits in `ROADMAP.md`, `GLOSSARY.md`, `setup/USAGE.md`, and `docs/specs/*.md`.

But that same call surfaced something real — see Q3.

---

## Q3 — The payload bug, observed rather than predicted

`search.rs` pushes **whole untruncated lines** into `SearchMatch.text`. `MAX_RESULTS: usize = 200` caps the match **count**, not the response **size**.

Against committed spec prose — which this project's design produces by construction, one paragraph per line — a single line runs 1–3 KB. The live `search_code("Merkle")` call above returned roughly **15 KB for about 20 matches**, most of it `ROADMAP.md` and `docs/specs/*.md` paragraphs. A broader regex against a repo with a complete corpus returns hundreds of KB.

M22's own ROADMAP entry had already called this and filed it as not-yet-urgent:

> *"`search_code`'s `matches` on a broad regex across a big repo is a secondary candidate for the same treatment, still deferred — lower priority since it hasn't been observed failing yet."*

It has now been observed, by accident, on the first live call made in this spike.

**This is the third instance of one failure mode**, which is what makes it worth naming as a class rather than a bug:

| Instance | Shape | Fix |
| --- | --- | --- |
| God-class `get_next_spec_task` (PR #37) | one oversized **blob** | reduce + hard-cap |
| `get_spec_coverage`'s `pending` (shipped 2026-09-20) | an oversized **list** | cursor + 50-item page |
| `search_code`'s `matches` (M20, not yet built) | oversized **lines** | per-line truncation + flag |

Three different shapes, one root cause: **no MCP response in this codebase has a size budget by construction.** Each has been fixed reactively after a real failure. Worth considering, though not scoped into M20: a response-size assertion in the test harness, so the fourth instance is caught by CI rather than by a user.

---

## Q4 — Does an exported field belong in `interface_hash`?

The one genuinely undecided question in the whole track. Promoted to `ARCHITECTURE.md` open question 12 rather than answered here.

**The rule today.** `extract.rs` folds member `source_hash`es into a container's `source_hash`, but deliberately does **not** fold member signatures into its `interface_hash`. The recorded reasoning: M2 resolves file-to-file edges, so nothing watches a class's members. That holds for **methods** — a method is never independently imported, which is why `get_callers` on one always returns empty.

**Why fields break the reasoning.** A `pub` field is public surface in exactly the sense a method signature is: change its type and every consumer breaks, identically to a changed return type. But a consumer's `deps_hash` keys on the target's `interface_hash`, so under today's rule that break **moves no hash and invalidates nothing**. A consumer's spec keeps describing a field shape that no longer exists, and `freshness` reports it current. That is the one class of silent wrongness the entire hashing design exists to prevent.

**Why it isn't simply fixed.** `interface_hash` moving is what propagates staleness across reference edges — deliberately one hop, never transitive. Widening what moves it widens how much of a corpus goes stale per edit. A struct whose fields churn routinely would invalidate every direct referrer every time, where today it invalidates none. Whether that is *correctness finally arriving* or *a `freshness` number rendered useless* is an empirical question about real repos, and nobody has measured the cascade.

**Why M21 is the right moment.** It's the first point where exported fields exist in the graph at all, so the cascade can be measured on CodeOwl's own repo and the pilot instead of argued about. Logged now so it gets decided deliberately, rather than defaulted into by whichever way the extraction happens to get written.

---

## Q5 — Does any of this actually need semantic search?

Raised explicitly by the owner: *"if that also needs us to have a semantic search capability, raise this as a point."*

**Answer: needed for one of three lookup kinds — and less than it first looks.**

Working backwards from what sends an agent to `grep`, repo lookups split three ways:

1. **"Show me this exact thing."** The id is already in hand. `get_symbol`/`get_source` answer it. No search involved at all.
2. **"Where is the thing called X"** — a name, a distinctive literal. Regex already answers it. At laptop scale, speed is not the bottleneck, so what an index would really add is **ranking**: ripgrep returns walk order, so a match inside a committed spec document outranks the actual definition purely by accident of directory order. Real gap — but the cheap fix is to rank on data CodeOwl already holds and `rg` structurally *cannot* see (`FileRole`: `Domain` before `Test` before `Generated`; prefer a line the graph knows is a declaration line). That's arguably a better differentiator than embeddings, and it needs no new dependency.
3. **"Find the code that does X"**, with no name to search for — *"where do we handle the debounce"*, *"what validates the order total."* **Only semantics answers this.** It's also exactly the question an agent has when it is **new to a repo** — the moment CodeOwl is supposed to be most valuable, and the moment it currently helps least.

### The reframe: index the corpus, not the code

CodeOwl holds something a generic code-embedding tool does not: **a corpus of LLM-written, hash-checked prose describing every symbol at several granularities.** Embeddings are good at prose and weak on raw source.

So the higher-value build is not "semantic search over code." It's:

```
search_specs(query) -> [ { spec_section, symbol_id, score } ]
```

— semantic search over `docs/specs/**`, returning spec sections **plus the symbol ids they describe**, which the agent then feeds straight into `get_source` / `get_callers` / `get_spec`. The tools **compose into one workflow** (find by meaning → read the real code → find who depends on it) rather than duplicating each other.

### Two constraints to carry into that decision

- **Coverage honesty.** Results are bounded by corpus coverage, so the response must state what it searched — *"340 specs, 61% of eligible nodes"* — rather than silently under-returning. Same tiered-degradation discipline open question 8 applies to schema coverage. A semantic miss on an uncovered region is indistinguishable from "nothing matches," which is the same ambiguity defect as Q1's `children: []`, in a new place.
- **Distribution cost — the actual decision.** `ort` plus a model file (~90 MB for all-MiniLM-L6-v2) cuts directly against the single-self-contained-binary property that `ARCHITECTURE.md` §9 names as the **first** reason CodeOwl is written in Rust. That tension — not embedding quality — is what has to be weighed.

### Sequencing

Build M20 and M21 first: no new dependencies, no distribution cost, no model file. **Then measure** whether kind-3 lookups are still the observed gap before committing. That's the same "wait for real evidence" policy open questions 8, 10 and 11 each already apply — and this spike is itself evidence for why it's the right policy, since two of its own confident starting assumptions (Q2) turned out to be false when checked.

---

## What this spike concludes

1. **Reliance is the metric, not coverage.** A structural tool that under-answers a class of question loses the questions it answers well too. That's the case for closing the three gaps together as a track rather than opportunistically.
2. **`get_source` is mostly existing machinery** — `spec::symbol_span_text` and `spec::cap_generation_text` already do the work; what's new is a cap, a flag, and a tool signature. The one design point worth building in rather than discovering: the watcher's 300ms debounce means an agent's edit-then-verify loop can read the *current* file at a *pre-edit* span, so `graph_in_sync` is required, computed against the **file's** `source_hash` (a container's own is a Merkle fold, not a hash of its span text).
3. **Field extraction is a consistency defect, not a missing feature** — and fixing it forces a deliberate, total `source_hash` invalidation plus a `FORMAT_VERSION` bump. Taking that invalidation is correct; engineering around it would produce a hash that lies about field reordering, which in Rust is a real layout change.
4. **Semantic search is worth one of three lookup kinds**, and the version worth building indexes the spec corpus rather than the source — deferred pending measurement, with binary size as the real cost.
5. **MCP response size has no budget by construction** — three reactive fixes in three shapes so far. Worth a systemic answer eventually.

## Open, deliberately

- **`interface_hash` and exported fields** (Q4) — decide during M21, with the cascade measurable rather than estimated. `ARCHITECTURE.md` open question 12.
- **A response-size budget in the test harness** (Q3) — named here, not scoped into M20. The fourth instance should be caught by CI, not by a user.
- **Whether `get_source` should ever return a *resolved* view** (a symbol plus its one-hop dependencies' signatures inline, one call instead of N) — not designed. Plausibly the next reliance gap once M20 lands, plausibly over-engineering; no evidence either way yet, so not guessed at here.
