# Glossary

The vocabulary `ARCHITECTURE.md`, `ROADMAP.md`, the code comments, and the
generated specs use. Most of it is borrowed from **static-analysis / compiler
tooling** or **graph theory**; a few terms are specific to CodeOwl. If a doc
uses a word that isn't in your normal app-dev vocabulary, it's probably here.

---

## Code structure — from static analysis

**Symbol**
: One named declaration extracted from source — a function, method, class,
  struct, enum, trait, `const`, type alias, or SQL table. The unit CodeOwl
  builds its graph out of. Same sense as a linker's "symbol" or a compiler's
  "symbol table" entry: a name that refers to a definition. In CodeOwl a
  symbol is a graph node with an `id`, a `kind`, a `signature`, a line
  range, and containment/reference links. (`src/symbol.rs`)

**`id` (stable id / string id)**
: A symbol's durable name, `"<file>::<name>"` — e.g. `src/graph.rs::Graph`
  or `src/graph.rs::Graph::build`. Stable across re-parsing, safe to put in
  a spec file or hand to an MCP client. Contrast `SymbolId`.

**`SymbolId`**
: An *index* into the graph's flat node array (the arena) — a `u32`
  wrapper. Cheap to copy and to serialize, but only meaningful inside the
  one graph that produced it and **not** stable across a re-parse. Internal
  only; it never leaves the process. Think array index, not pointer.

**Arena**
: A flat `Vec` that owns every graph node; nodes refer to each other by
  `SymbolId` (an index) rather than by Rust reference or `Rc<RefCell<…>>`.
  The standard way to represent a cyclic graph in Rust without fighting the
  borrow checker, and it makes the on-disk cache trivial to serialize.
  (`ARCHITECTURE.md` "The graph is arena-indexed")

**`kind` vs `raw`**
: `kind` is the **stack-neutral** bucket a symbol falls into — one of
  `Container` (has members: a class, struct, enum, trait), `Callable` (a
  function/method), `Value` (a `const`/`static`), `Schema` (a SQL table).
  `raw` is the **concrete grammar keyword** the pack saw — `"class"`,
  `"struct"`, `"fn"`, `"macro_rules"`, `"@interface"`. The generic pipeline
  reasons about `kind`; `raw` is for display and pack-internal logic.
  (M14 / `src/symbol.rs`)

**Signature**
: A callable's type-level shape — parameters and return type — or a type's
  declared header. Extracted deterministically from source; CodeOwl fills it
  into a spec itself and never asks the LLM to restate it.

**Docstring**
: The doc comment immediately above a declaration (`///` / `//!` in Rust,
  `/** */` in TS/Java, `"""` in Python). Extracted and shown in specs and
  task context.

**Marker**
: An annotation or attribute attached to a declaration — `#[derive(...)]`,
  `#[tool]`, `@Entity`, `@Path`. Pack-owned free-form strings; the generic
  core never interprets them. Java's feature and schema models (M17) are
  entirely marker-driven.

**Extraction**
: The per-file pass that parses source with tree-sitter and pulls out its
  symbols. Syntax-only, no cross-file knowledge. Output: a list of
  `ExtractedSymbol` with *string* parent/child links. (`src/extract.rs`,
  `src/rust.rs`, `src/schema.rs`)

**Resolution**
: The cross-file pass that turns an `import`/`use` specifier into the actual
  target symbol it names — following path aliases, re-export chains, and
  the module-file convention. Turns string links into `SymbolId` links.
  (`src/resolve.rs`, `rust::resolve_imports`)

**tree-sitter**
: The parser library CodeOwl uses. One grammar per language, compiled into
  the binary. Produces a concrete syntax tree; CodeOwl walks it and
  extracts owned data, then drops the tree.

**AST walk**
: Recursively visiting parse-tree nodes. "Shallow walk" = only the file's
  top-level items plus one level into class/impl bodies (what CodeOwl does
  for symbols). "Full walk" = every node (what it does to find `fetch()`
  calls, which can be nested anywhere).

---

## Relationships — from graph theory

**Node / edge**
: The graph is nodes (files and symbols) connected by edges. CodeOwl keeps
  two structurally different edge types explicitly, because they behave
  differently:

**Containment edge** (`parent` / `children`)
: A **tree** — a function belongs to exactly one file, a method to exactly
  one class, a file to exactly one directory. Terminates at leaves. Spec
  generation *recurses* across these (a file's spec is composed from its
  functions' specs).

**Reference edge** (`imports`, and later `calls`)
: A **dense, cyclic graph** — nearly everything imports a handful of shared
  utilities, and mutual imports are routine. Spec generation only *reads*
  these (it never recurses across them — that wouldn't terminate and
  wouldn't scale). `get_callers` / `get_callees` expose them.

**Fan-in**
: How many *other* files import something from this one — its in-degree in
  the reference graph. A high-fan-in file (a shared client, core utils) is
  documented first, because its summary then feeds every dependent spec's
  context. Fan-out would be the reverse (how many things this file
  imports); CodeOwl mostly cares about fan-in.

**Flow edge**
: A "this file reaches that thing" edge the **import graph structurally
  cannot see** because it's carried by a string literal, not an `import`:
  a Next.js `fetch("/api/submit")`, a Supabase `.from("payments")`, a
  rendered `<Component/>` tag. The pack extracts these unresolved (one raw
  string each) and resolves each against the built graph. This is the whole
  reason feature specs exist — the load-bearing hops in a real user flow
  are usually flow edges. (`src/features.rs`, `graph::FlowEdge`)

**Merkle fold / rollup hash**
: Computing a container's `source_hash` by hashing its own text *plus* each
  child's `source_hash`. Editing any method then moves both that method's
  hash and its class's — the same trick Git uses for tree objects. Lets a
  change propagate up the containment tree deterministically.

---

## Specs — CodeOwl's own vocabulary

**Spec**
: An LLM-authored prose description of a code unit — what it does, why it
  exists, its behavior and failure modes. The primary artifact CodeOwl
  serves. Committed to `docs/specs/` as Markdown, content-hashed. CodeOwl
  never writes spec prose itself — the calling agent does, via
  `get_next_spec_task` → write → `submit_spec`.

**Spec-bearing**
: A code unit that gets **its own spec document or section**, as opposed to
  being folded into its parent's. The rule: a **top-level** `Callable` or
  `Container` is spec-bearing; a method is not (it's described inside its
  class's section); a `const` is not (it rolls up into the file spec). A
  **file** is spec-bearing iff it has ≥1 exported function or class — a
  barrel/re-export file isn't. A **directory** is spec-bearing (gets a
  rollup) iff it has ≥2 spec-bearing files. (`src/spec.rs`,
  `ARCHITECTURE.md` "granularity rules")

**Granularity rules**
: The deterministic, no-LLM rules deciding *which* specs exist at all — the
  "spec-bearing" tests above. `get_spec_coverage` reports against this
  inventory, so coverage can actually reach 100% instead of being capped by
  files that were never meant to have a doc.

**Document kinds** (five)
: **Symbol spec** — a `### Summary` + `### Behavior` section for one
  function/type, inside its file's `.md`.
  **File spec** — `docs/specs/<path>.md`; a `## Summary` plus a section per
  spec-bearing symbol.
  **Rollup** (directory spec) — `docs/specs/<dir>/_index.md`; composed from
  its files' summaries, not from source.
  **Feature spec** — `docs/specs/_features/<slug>.md`; a "how does X work
  end to end" narrative crossing files sideways along flow edges. Written
  for a business analyst, not just a dev.
  **System spec** — `docs/specs/_index.md`; one per repo, the whole-product
  overview, written last.

**Stub**
: The deterministic fallback used where no spec exists yet — just the
  signature plus any inline docstring, read straight off the graph, no LLM.
  A reference from spec A to symbol B that has no spec gets B's stub.

**Participant / core / dependencies / data**
: A feature spec's inputs, in three tiers.
  **core** — the feature's own code (its entry point plus the files it
  reaches through admitted flow edges); tracked by `source_hash`, so a body
  change is a feature change.
  **dependencies** — the symbols `core` imports directly; tracked by
  `interface_hash` (only a public-surface change matters).
  **data** — the SQL tables `core` queries; tracked by `source_hash`.
  Dependencies and data are one hop only — never expanded further.
  (`src/features.rs::Participants`)

**Entry point**
: Where the outside world calls into the system and a distinct capability
  begins — a Next.js page or orphan API route, a Quarkus `@Path` resource,
  a Kafka listener. A `FeatureModel` enumerates these; each becomes a
  feature spec. A CLI or library has none.

**Reconciliation**
: What happens when *both* the source changed *and* a human hand-edited the
  spec since it was last machine-written. The regeneration is given the
  human's version as a prior with instructions to keep whatever correction
  is still accurate and change only what the source diff affects. (M8)

---

## Hashes and status

CodeOwl keys spec invalidation on four hashes, all `blake3` of a canonical
string, all deterministic and LLM-free:

**`source_hash`**
: Hash of a symbol's (or file's) own source text. Moves on any edit to that
  text. For a container, Merkle-folds its members.

**`interface_hash`**
: Hash of a symbol's *exported shape* only — its signature, never its body
  or docstring. This is what reference-edge staleness keys on: a dependency
  whose implementation changed but whose signature didn't should not force
  its dependents' specs to regenerate. `None` for a non-exported symbol.

**`deps_hash`**
: Hash of the `(target id, target's current interface_hash)` pairs for
  everything a symbol/file imports. Moves when a dependency changes *shape*
  even though this symbol's own text didn't. (M7)

**`spec_hash`**
: Hash of the **LLM-written prose only** — not the CodeOwl-written
  signature/dependency lines. Lets a human edit to the prose be detected
  (it won't match), while a deterministic refresh of the signature line
  doesn't spuriously look like a human edit.

**Spec status** — what `get_spec` / `get_spec_coverage` report:
: **`missing`** — no spec has ever been generated (a stub is returned).
  **`current`** — a spec exists and all its input hashes still match.
  **`stale`** — a spec exists but `source_hash` or a dependency's
  `interface_hash` moved since; the last-good text is still returned, with
  `changed` naming what moved.
  **`smelly`** — a deterministic quality check (`prose_smells`) distrusts
  the prose (a "see the source" cop-out, or too short to be real) *even
  though* the hashes match. Not exclusive with `current`.

---

## Stacks and the pluggability work (Phase 2)

**Stack**
: The whole toolchain shape of a repo — language + framework + schema
  convention + resolution rules. CodeOwl serves **one stack per repo**,
  auto-detected. "TypeScript + Next.js + SQL" is one stack; "Rust" is
  another. Not just a language — the Phase-1 pack already spans TS + SQL +
  Next + Supabase.

**`StackPack`**
: The trait every stack implements — extraction, resolution, file
  classification, optionally a feature model. The generic pipeline around
  it never names a stack convention. (`src/stack.rs`)

**`FeatureModel`**
: The optional part of a `StackPack` that models "what is a feature" —
  entry-point enumeration and `core` admission. A CLI/library stack returns
  `None` and has no feature layer. (`src/features.rs::FeatureModel`)

**"The socket held" / trait leak**
: Shorthand for "did the abstraction stay abstract." When a second stack
  (Rust) was added, the test was whether `spec.rs` / `mcp.rs` needed *any*
  stack-specific change or whether everything fit behind the `StackPack`
  trait. It fit — "the socket held," "no trait shape leaks."

**Stack-neutral / neutralization**
: Code or docs that don't assume a specific stack. "Doc neutralization" =
  rewriting `ARCHITECTURE.md` etc. so Next.js-isms are presented as *one
  pack's* choices, not universal truths.

**`FORMAT_VERSION`**
: A single integer stamped into the `.codeowl/` cache. Bumped whenever the
  serialized shape changes in a way that would silently give wrong answers
  on an old cache; a cache with any other value is discarded and rebuilt.

---

## Process vocabulary

**Generation**
: One `get_next_spec_task` → write prose → `submit_spec` cycle. What
  `/codeowl-generate --budget=N` counts — a file with 20 uncovered symbols
  costs 21 generations, not 1. `get_spec_coverage`'s `generations_remaining`
  is the total for a complete pass.

**The pilot**
: The reference test repo — a real TypeScript + Next.js + Supabase app
  (`talentTrail`). Every Phase-2 refactor is validated "inert on the pilot"
  before merge.

**Dogfooding / self-corpus**
: Running CodeOwl on its own source. The "self-corpus" is the set of specs
  CodeOwl generates about itself — the M14/M15 validation that the trait
  produces something coherent for a non-web stack.

**`structural_sweep.py`**
: The tool that proves a refactor changed nothing observable: runs two
  CodeOwl builds against the pilot and byte-diffs every MCP read
  (`extract`, coverage, `get_next_spec_task`, `get_callers`, …).
