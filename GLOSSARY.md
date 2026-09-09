# Glossary

The vocabulary `ARCHITECTURE.md`, `ROADMAP.md`, the code comments, and the
generated specs use. Most of it is borrowed from **static-analysis /
compiler tooling** or **graph theory**; a handful of terms are CodeOwl's
own. If a doc uses a word that isn't in your everyday app-dev vocabulary,
it's probably explained here.

Each entry leads with the plain idea, then fills in the precise version and
why CodeOwl cares.

---

## Code structure — from static analysis

**Symbol**
: One named thing declared in the source code: a function, a method, a
  class, a struct, an enum, a trait, a `const`, a type alias, or a SQL
  table. Anything you could "go to definition" on in an editor.

  CodeOwl reads every source file and pulls out its symbols; those symbols
  are the building blocks of everything else it does. Each symbol becomes
  one node in CodeOwl's graph, carrying an `id`, a `kind` (function? class?
  table?), its signature, the line range it occupies, and links to the
  things it contains and the things it refers to.

  The word is borrowed from compilers and linkers — a linker's "symbol
  table" is exactly this: a list of names and what each one points at.
  (`src/symbol.rs`)

**`id` (also: stable id, string id)**
: A symbol's permanent, human-readable name — `"<file>::<name>"`. Examples:
  `src/graph.rs::Graph` (the `Graph` type in that file),
  `src/graph.rs::Graph::build` (its `build` method).

  This is the name that's safe to write into a spec file, hand to an MCP
  client, or paste into a chat. It doesn't change when CodeOwl re-parses
  the repo, so a reference to `src/graph.rs::Graph` written yesterday still
  points at the same thing today. Contrast `SymbolId` (below), which is the
  opposite.

**`SymbolId`**
: A number — a plain `u32` — that says *which slot* a symbol occupies in
  CodeOwl's in-memory node array (see **Arena**).

  It's tiny and fast to copy, which is why the internal code passes these
  around instead of the long string `id`. But it's only meaningful inside
  the one graph that produced it, and it changes every time the repo is
  re-parsed (symbol #42 today might be symbol #58 tomorrow). So a
  `SymbolId` never leaves the process — it's never written to a file or
  sent to a client.

  Mental model: a `SymbolId` is an *array index*, the string `id` is a
  *name*. You look things up by name across sessions; you use the index for
  speed within one.

**Arena**
: One big list (`Vec`) that owns every node in the graph. Nodes don't point
  at each other with real pointers or reference-counted boxes — they refer
  to each other by `SymbolId`, i.e. by position in this list.

  Why do it this way? A code graph is full of cycles (file A imports file
  B which imports file A), and Rust's ownership rules make cyclic pointer
  structures painful. Storing everything in one flat list and linking by
  index sidesteps that entirely — it's the standard Rust answer for this
  shape of data. Bonus: saving the graph to disk is then just serializing
  one list.

  Background: Niko Matsakis, ["Modeling graphs in Rust using vector
  indices"](https://smallcultfollowing.com/babysteps/blog/2015/04/06/modeling-graphs-in-rust-using-vector-indices/)
  — the canonical write-up of exactly this technique. (`ARCHITECTURE.md`
  "The graph is arena-indexed")

**`kind` vs `raw`**
: Two labels on every symbol, at two levels of abstraction.

  `kind` is the **generic bucket** — one of just four values that the
  language-independent parts of CodeOwl reason about:
  - `Container` — something with members inside it: a class, struct, enum,
    trait, interface.
  - `Callable` — a function or method.
  - `Value` — a plain data holder with no members worth documenting on
    their own: a `const`, a `static`.
  - `Schema` — a database table.

  `raw` is the **exact keyword the parser actually saw** — `"class"`,
  `"struct"`, `"fn"`, `"interface"`, `"macro_rules"`, `"@interface"`,
  `"record"`, and so on.

  The split lets the shared pipeline stay language-neutral (it only ever
  checks `kind`), while `raw` is kept around for display ("this is a
  `struct`, not just a `Container`") and for language-specific logic inside
  a pack. (M14 / `src/symbol.rs`)

**Signature**
: The type-level shape of a callable — its parameter list and return type
  — or the declared header of a type. For example, `pub fn build(files:
  Vec<FileExtraction>) -> Self`.

  CodeOwl extracts this straight from the source and writes it into the
  spec itself. It never asks the LLM to restate a signature, because a
  signature is a fact the graph already knows, and a hallucinated one would
  be worse than none.

**Docstring**
: The documentation comment written directly above a declaration — `///`
  or `//!` in Rust, `/** … */` in TypeScript and Java, `"""…"""` in
  Python. CodeOwl pulls this out and shows it both in the finished spec and
  in the context it hands the LLM when asking for a spec.

**Marker**
: An annotation or attribute stuck on a declaration — `#[derive(Debug)]`,
  `#[tool]`, `@Entity`, `@Path("/users")`, `@Deprecated`.

  CodeOwl records these as free-form strings and the generic core never
  tries to interpret them — but a language pack reads its own back out.
  This matters a lot for Java: a Quarkus service's entire notion of "what's
  an HTTP endpoint" and "what's a database entity" is expressed through
  annotations (`@Path`, `@Entity`), so the Java pack's feature and schema
  models are almost entirely marker-driven (M17).

**Extraction**
: The first pass CodeOwl runs on each file: parse it with tree-sitter, walk
  the parse tree, and collect the symbols. This pass is **syntax only** —
  it looks at one file at a time and knows nothing about what any other
  file contains. Its output is a list of symbols whose parent/child links
  are still just strings, not resolved graph edges.
  (`src/extract.rs`, `src/rust.rs`, `src/java.rs`, `src/schema.rs`)

**Resolution**
: The second pass, which needs the whole repo in view: take an `import` or
  `use` statement — say `import { Graph } from './graph'` — and figure out
  exactly which symbol in which file it actually refers to. This means
  following path aliases (`@/lib/...`), re-export chains ("this file just
  forwards `Graph` from that other file"), and each language's
  module-to-file convention. The output turns the string links from
  extraction into real graph edges.
  (`src/resolve.rs`, `rust::resolve_imports`, `java::resolve_imports`)

**tree-sitter**
: The parser library CodeOwl is built on. It has one grammar per language
  (TypeScript, Rust, Java, …), each compiled directly into the CodeOwl
  binary. Given source text it produces a *concrete syntax tree* — every
  token and bracket represented as a node. CodeOwl walks that tree, copies
  out the data it wants as plain owned values, and then throws the tree
  away.

**AST walk**
: Recursively visiting the nodes of a parse tree. CodeOwl does two kinds:
  - **Shallow walk** — only the file's top-level declarations, plus one
    level down into class and `impl` bodies to catch methods. This is how
    it finds symbols; it deliberately doesn't descend into function bodies.
  - **Full walk** — every node, however deeply nested. This is how it finds
    things like a `fetch("/api/...")` call, which can appear anywhere
    inside any function.

---

## Relationships — from graph theory

**Node / edge**
: The graph is made of **nodes** (files and symbols) joined by **edges**
  (relationships). CodeOwl deliberately keeps two *kinds* of edge separate,
  because they have completely different shapes and get used completely
  differently — the next two entries.

**Containment edge** (`parent` / `children`)
: "X is inside Y." A method is inside exactly one class; that class is
  inside exactly one file; that file is inside exactly one directory.

  Structurally this forms a **tree** — every node has exactly one parent,
  and following parents always terminates (at the repo root). Spec
  generation *recurses* along these edges: a file's spec is built up from
  its functions' specs, a directory's rollup from its files' summaries.
  Because it's a finite tree, that recursion is guaranteed to finish.

**Reference edge** (`imports`; later also `calls`)
: "X uses / depends on / points at Y." File A imports a helper from file B.

  Structurally this is a **dense, tangled, cyclic graph** — almost every
  file imports the same handful of shared utilities, and two files
  importing each other is completely normal. If spec generation tried to
  recurse along these ("to document A, first document everything A imports,
  and everything *those* import…") it would loop forever and never scale.
  So generation only ever *reads* reference edges, never recurses across
  them. `get_callers` and `get_callees` are how you query them.

**Fan-in**
: How many *other* files import something from this file. In graph terms,
  its in-degree in the reference graph.

  A file with high fan-in — a shared HTTP client, a core utilities module —
  is documented **first**, on purpose. Once its summary exists, every spec
  that depends on it can be handed that summary as context, so the
  dependent specs come out better. (The opposite direction, **fan-out** —
  how many things *this* file imports — is rarely what CodeOwl cares
  about.)

**Flow edge**
: A "this file reaches that thing" connection that the import graph
  **physically cannot see**, because it's carried by a string, not by an
  `import` statement.

  Examples: a Next.js frontend does `fetch("/api/submit")` — that string
  connects the UI to an API route handler, but there's no `import` linking
  them. A Supabase call `.from("payments")` connects code to a database
  table. A JSX `<SubmitForm/>` tag renders a component.

  CodeOwl's language pack pulls these out as raw unresolved strings, then
  matches each one against the built graph to find its target. This is the
  entire reason **feature specs** exist: the interesting hops in a real
  user flow — click a button, hit an endpoint, write a row — are almost
  always flow edges, invisible to a plain import analysis.
  (`src/features.rs`, `graph::FlowEdge`)

**Merkle fold** (also: rollup hash)
: A way of hashing a container so that a change anywhere inside it changes
  the container's hash too.

  Concretely: a class's `source_hash` isn't just a hash of the class's own
  text — it's a hash of that text *plus* every method's `source_hash`. So
  editing one line in one method moves that method's hash, which moves the
  class's hash, which (if the class is nested) moves the outer type's hash,
  all the way up.

  This is exactly the trick Git uses for its tree objects. It lets CodeOwl
  answer "did anything under here change?" with a single hash comparison,
  and it makes a change propagate up the containment tree in a completely
  deterministic way.

---

## Specs — CodeOwl's own vocabulary

**Spec**
: A prose description of a piece of code — what it does, why it exists, how
  it behaves, how it fails — **written by an LLM**, not by CodeOwl.

  This is the main thing CodeOwl produces. Specs are saved into `docs/specs/`
  as Markdown files and committed to git alongside the code. CodeOwl's job
  is to assemble the right context and then persist whatever prose the
  calling agent writes; the loop is `get_next_spec_task` (CodeOwl hands you
  a unit of code + its context) → you write the prose → `submit_spec`
  (CodeOwl stores it).

**Spec-bearing**
: A piece of code that gets **its own spec section or file**, rather than
  being folded into its parent's.

  The rules:
  - A **top-level** function or type is spec-bearing.
  - A **method** is not — it's described inside its class's section.
  - A **`const`** is not — it's mentioned in the file's summary.
  - A **file** is spec-bearing only if it has at least one exported
    function or class. A file that just re-exports things from elsewhere (a
    "barrel") is not.
  - A **directory** is spec-bearing (it gets a rollup) only if it contains
    at least two spec-bearing files.

  (`src/spec.rs`, `ARCHITECTURE.md` "granularity rules")

**Granularity rules**
: The fixed, no-LLM rules that decide *which specs should exist at all* —
  the spec-bearing tests just above.

  These matter because `get_spec_coverage` measures progress against this
  list. If coverage were measured against "every file," it could never
  reach 100% — there'd always be barrel files and trivial directories
  dragging it down. Measuring against "every file that's *supposed* to have
  a spec" means 100% is a real, reachable target.

**Document kinds** (there are five)
: - **Symbol spec** — a `### Summary` + `### Behavior` block for one
    function or type, living inside its file's `.md`.
  - **File spec** — `docs/specs/<path>.md`: a `## Summary` for the file,
    followed by one section per spec-bearing symbol in it.
  - **Rollup** (a.k.a. directory spec) — `docs/specs/<dir>/_index.md`:
    written by combining the directory's file summaries, *not* by
    re-reading source.
  - **Feature spec** — `docs/specs/_features/<slug>.md`: a "how does X work,
    end to end" narrative that crosses *sideways* between files along flow
    edges. Written to be readable by a business analyst, not only a
    developer.
  - **System spec** — `docs/specs/_index.md`: one per repo, the
    whole-product overview. Written last, because it's assembled from all
    the others.

**Stub**
: The automatic fallback shown wherever a real spec doesn't exist yet: just
  the symbol's signature plus its inline docstring, read straight off the
  graph — no LLM involved.

  Stubs are what make partial coverage usable. If spec A refers to symbol
  B and B has no spec, the reader (or the LLM writing A) still gets B's
  stub — enough to know what B is.

**Participant / core / dependencies / data**
: A feature spec's inputs, organised in three tiers of "how closely does
  this belong to the feature":

  - **core** — the feature's *own* code: its entry point plus every file it
    reaches through an admitted flow edge. Tracked by `source_hash`, so
    changing a line in any core file counts as changing the feature.
  - **dependencies** — the symbols that core code imports directly (one hop
    only). Tracked by `interface_hash` — only a change to a dependency's
    *public shape* matters, not its internals.
  - **data** — the SQL tables that core code queries (one hop only).
    Tracked by `source_hash`.

  "One hop only" is the key constraint: CodeOwl lists what core directly
  touches and stops there. It never expands a dependency's dependencies.
  (`src/features.rs::Participants`)

**Entry point**
: A place where the outside world calls into the system and a distinct
  capability starts. A Next.js page or an orphan API route; a Java
  `@Path`-annotated REST resource; a Kafka message listener; a CLI
  subcommand.

  A stack's `FeatureModel` enumerates its entry points, and each one
  becomes a feature spec. A pure library or a simple CLI has no entry
  points in this sense, and that's fine — it just gets symbol, file, and
  rollup specs.

**Reconciliation**
: The special case where **both** things happened since a spec was last
  machine-written: the *source changed* **and** a *human hand-edited the
  spec*.

  Naively regenerating would throw away the human's edit. Instead, CodeOwl
  hands the regeneration the human's version as a "prior," with
  instructions: keep whatever the human wrote that's still accurate, and
  only change the parts the actual source diff forces you to change. (M8)

---

## Hashes and status

To decide when a spec has gone out of date, CodeOwl compares hashes. There
are **four**, and all of them are computed the same boring way — `blake3`
of a canonical string — so they're fully deterministic and never involve
an LLM.

Why four and not one? Because "the spec is stale" has several different
causes, and CodeOwl wants to tell them apart — a change to a function's
internals shouldn't invalidate the specs of everything that *calls* it, but
a change to its signature should.

**`source_hash`**
: Hash of a symbol's (or file's) own source text. Moves on *any* edit to
  that text. For a container, it's the Merkle fold described above (own
  text + each member's `source_hash`), so it also moves when any member
  changes.

**`interface_hash`**
: Hash of a symbol's *public shape only* — its signature, and nothing else
  (not the body, not the docstring).

  This is what a **reference edge** keys on. If module B changes how a
  function works internally but the signature is identical, then everything
  that imports that function is unaffected — its `interface_hash` didn't
  move, so no dependent spec needs regenerating. `None` for a symbol that
  isn't exported (nothing outside its file can refer to it).

**`deps_hash`**
: Hash of the list of `(dependency id, that dependency's current
  interface_hash)` pairs, for everything a symbol or file imports.

  It moves when a *dependency* changed its shape, even though this symbol's
  own text is untouched. Example: your `submit()` function is byte-for-byte
  the same, but a type it imports gained a field — `deps_hash` catches
  that, so `submit()`'s spec gets flagged for a look. (M7)

**`spec_hash`**
: Hash of the **LLM-written prose only** — deliberately *not* including the
  signature and dependency lines that CodeOwl writes into the spec itself.

  This is how CodeOwl tells "a human edited this prose by hand" (the hash
  won't match any more) apart from "CodeOwl refreshed the auto-generated
  signature line" (which shouldn't look like a human edit). The former
  triggers reconciliation; the latter is silent.

**Spec status** — the word `get_spec` / `get_spec_coverage` attaches to
each document:
: - **`missing`** — no spec has ever been written for this. A stub is
    returned instead.
  - **`current`** — a spec exists and every input hash still matches. Good.
  - **`stale`** — a spec exists, but the code moved underneath it
    (`source_hash` changed, or a dependency's `interface_hash` did). The
    last known-good text is still returned, along with a `changed` field
    saying what moved.
  - **`smelly`** — the hashes all match, but a deterministic quality check
    (`prose_smells`) doesn't trust the prose: it contains a "see the
    source" cop-out, or it's too short to be a real description. A document
    can be `current` *and* `smelly` at the same time.

---

## Stacks and the pluggability work (Phase 2)

**Stack**
: The whole toolchain shape of a repo — its language, its framework, its
  database/schema convention, and its import-resolution rules, taken
  together. Not just "the language."

  "TypeScript + Next.js + SQL + Supabase" is one stack. "Rust" is another.
  "Java + Maven" is another. CodeOwl serves **exactly one stack per repo**,
  and it figures out which one automatically by looking at the files.

**`StackPack`**
: The plug-in interface that every stack implements: how to extract
  symbols, how to resolve imports, how to classify a file (source? test?
  generated?), and — optionally — a feature model.

  The point of the trait is that the generic machinery around it
  (`spec.rs`, `mcp.rs`, `graph.rs`, `index.rs`) never mentions a
  stack-specific convention. Adding support for a new language is writing a
  new `StackPack`, not editing the core. (`src/stack.rs`)

**`FeatureModel`**
: The *optional* part of a `StackPack` — the part that models "what counts
  as a feature here." It knows how to list a stack's entry points and how
  to decide which nearby files belong to a feature's core.

  A framework-shaped stack (Next.js, Quarkus) has one. A library or a plain
  CLI (a pure Rust crate, Apache commons-lang) returns `None` — it still
  gets symbol / file / rollup / system specs, just no feature layer.
  (`src/features.rs::FeatureModel`)

**"The socket held" / "no trait leak"**
: Shorthand for "the abstraction actually held up."

  When a genuinely different stack was added (Rust after TypeScript, then
  Java), the real test was: did any of the generic files — `spec.rs`,
  `mcp.rs` — need a stack-specific `if` added to them? Or did everything
  new fit behind the `StackPack` trait? Each time it fit, so: "the socket
  held," "no trait shape leaks." A leak would mean the trait was drawn in
  the wrong place.

**Stack-neutral / doc neutralization**
: Code or documentation that doesn't quietly assume one particular stack.

  "Doc neutralization" is the ongoing job of rewriting `ARCHITECTURE.md`
  and friends so that Next.js-specific ideas (routes, pages, `fetch`) are
  presented as *one pack's* choices rather than as universal truths about
  how CodeOwl works.

**`FORMAT_VERSION`**
: A single integer stamped into the `.codeowl/` cache on disk. It's bumped
  whenever the saved data structure changes in a way that would make an old
  cache give *wrong* answers rather than just missing ones. On startup, if
  the cache's version isn't the current one, CodeOwl throws the whole cache
  away and rebuilds from scratch — safer than trying to migrate it.

---

## Process vocabulary

**Generation**
: One full `get_next_spec_task` → write prose → `submit_spec` cycle — i.e.
  producing one spec.

  This is the unit `/codeowl-generate --budget=N` counts. A file with 20
  undocumented symbols costs 21 generations (one per symbol, then one for
  the file's own summary), not 1. `get_spec_coverage`'s
  `generations_remaining` field is the total number of these needed to
  fully document the repo.

**The pilot**
: The main reference repo CodeOwl is tested against — a real production
  TypeScript + Next.js + Supabase app (internally, `talentTrail`). Every
  Phase-2 refactor has to be shown "inert on the pilot" (it produces
  byte-identical output before and after) before it can merge.

**Dogfooding / self-corpus**
: Running CodeOwl on its own source code. The "self-corpus" is the set of
  specs CodeOwl writes *about itself* — used in M14/M15 to check that the
  `StackPack` machinery produces something coherent for a non-web,
  non-framework codebase.

**`structural_sweep.py`**
: The tool that proves a refactor changed nothing a consumer can observe.
  It builds two versions of CodeOwl (before and after the change), runs
  both against the pilot repo, and byte-compares every MCP read —
  `codeowl extract`, `get_spec_coverage`, `get_next_spec_task`,
  `get_callers`, `get_callees`, `get_symbol`. Any difference is a
  regression.

---

## Acronyms

**MCP** (Model Context Protocol)
: The protocol CodeOwl speaks. It runs as an MCP *server*; a coding agent
  (Claude Code, VS Code + Copilot, …) is the MCP *client* that calls
  CodeOwl's tools (`get_spec`, `get_callers`, `get_next_spec_task`, …).

**LLM** (Large Language Model)
: The model doing the writing — Claude, GPT, etc. Worth stating plainly
  because CodeOwl's central rule is that **CodeOwl never calls one**. It
  assembles context and stores results; the client's LLM writes every word
  of spec prose.

**BA** (Business Analyst)
: The non-developer reader a **feature spec** is written for — someone who
  needs to understand *what a capability does and what rules govern it*
  without reading code.

**AST** (Abstract Syntax Tree)
: The tree structure a parser produces from source code. See **AST walk**.
