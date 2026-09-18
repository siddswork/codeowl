# CodeOwl — Launch Talk Narrative

> First public reveal of CodeOwl to the org. Format: **6 slides + a live demo**,
> ~12 minutes of talking plus demo, then Q&A. Audience: ~90% devs/technical,
> ~10% senior management — so every slide carries one line a manager can repeat
> in a hallway, but the meat is written for engineers who've never touched
> compiler theory and shouldn't need to.
>
> This file is **content, not slides** — narrative script, bullet points, and
> rough ASCII diagrams you can redraw properly (or drop as-is into a code-block
> slide — they render fine monospaced). No jargon is used without being
> explained the sentence before it's first used.

---

## The one-sentence pitch

**CodeOwl reads your codebase once, builds a map of how it actually fits
together, and keeps a set of plain-English docs about it that update
themselves — and it hands that map to your AI coding tools so they stop
re-guessing it from scratch every single time.**

If you only remember one line for the hallway: *"It's living documentation
that can't go stale, because a robot checks it against the code every time
anyone asks."*

---

## Slide 1 — The Problem: your codebase has a memory problem

**Say this:**
> "Every one of us — and every AI coding assistant we use — has to rebuild
> the same mental model of this codebase, over and over. What calls what.
> Which button hits which API. Which API writes to which table. We `grep`,
> we read files, we guess the connections that aren't written down anywhere
> explicit — and the moment that session ends, that understanding is gone.
> The next person, or the next AI session, starts from zero again."

**Slide bullets:**
- Documentation, where it exists, was written once and rotted since — nobody
  trusts it, so nobody reads it, so nobody updates it. Vicious circle.
- The code is the only reliable source of truth — but reading it fluently
  end-to-end is a skill, and it's slow even for people who have it.
- Every dev, every BA, every QA engineer, and **every AI agent** re-derives
  the same understanding independently. That's wasted human time *and*
  wasted AI tokens (agents pay per token to re-explore).
- Some of the most important connections in a codebase aren't even written
  as code — they're strings. A URL, a table name. No amount of `grep`-ing
  reliably finds all of those.

**Diagram — the same discovery work, repeated forever:**

```
 Every session, every person, every AI agent starts here:

   Dev A            Dev B            AI Agent C           New hire D
     |                |                  |                    |
     v                v                  v                    v
   grep, read files, guess how things connect, build a mental model
     |                |                  |                    |
     v                v                  v                    v
  understanding    understanding     understanding         understanding
  (in one head,    (in one head,     (thrown away          (starts from
   never shared)    never shared)     when session ends)     zero, again)
```

**One line for management:** *"Right now, understanding how our own systems
work lives only in people's heads and gets re-earned, at a cost, every time
someone needs it — including by the AI tools we're paying to make us
faster."*

---

## Slide 2 — The Idea: a shared brain for the codebase

**Say this:**
> "CodeOwl builds that shared understanding once, and keeps it current
> automatically. Think of it in two layers. First, it reads every file and
> builds a *structural map* — which function calls which, which file imports
> which, which button click actually reaches which database table. That part
> is pure fact-checking: no AI involved, no guessing, just parsing. Second,
> on top of that map, it generates plain-English write-ups — specs — at every
> level: this function, this file, this whole directory, this whole feature,
> the whole system. Any AI assistant, or any human, can ask CodeOwl a
> question instead of re-exploring the repo. One exception worth naming: a
> pure library or CLI never gets feature specs — there's no user-facing
> route to hang one off — so it just gets function/file/directory/system
> specs instead, and that's expected, not a gap."

**Slide bullets:**
- Two layers: a **structural graph** (deterministic facts, extracted by
  parsing — never guessed) and **prose specs** (LLM-written English,
  layered on top of the graph, one per function/file/directory/feature/system).
- Not every codebase gets every level — a pure library or CLI has no
  feature specs at all (no user-facing route to hang one off), just
  function/file/directory/system.
- Specs are **committed to git** — reviewable in a pull request, browsable on
  GitHub, versioned right alongside the code they describe.
- Served over **MCP** (Model Context Protocol — the same plumbing Claude
  Code, VS Code, and other AI tools already speak) so any AI assistant in
  the room can query it directly.
- Dual audience by design: an AI agent gets exact files and dependencies
  without exploring; a human (new hire, BA, QA) gets a document that reads
  like a person explaining the feature to them.

**Diagram — CodeOwl as the shared brain everyone queries instead of re-deriving:**

```
     Dev A        Dev B        AI coding agent        BA / QA / new hire
       \             |               |                     /
        \            |               |                    /
         v            v              v                   v
        ┌─────────────────────────────────────────────────┐
        │                 CodeOwl  (MCP server)             │
        │                "the shared brain"                 │
        └───────────────────────┬────────────────────────────┘
                                 │
                 ┌───────────────┴────────────────┐
                 │      Structural Graph            │  <- facts, extracted
                 │  (functions, files, imports,     │     once by parsing,
                 │   calls, hidden string links)     │     never guessed
                 └───────────────┬────────────────┘
                                 │
                 ┌───────────────┴────────────────┐
                 │        Prose Specs (.md)         │  <- written once,
                 │  one per symbol / file / feature  │     reused by everyone,
                 │  / system — kept honest by        │     auto-flagged the
                 │  checking against the graph        │     instant code moves
                 └──────────────────────────────────┘
```

**One line for management:** *"It's a self-updating knowledge base for our
codebase — and it plugs straight into the AI tools our devs already use, so
adoption doesn't require anyone to learn a new tool."*

---

## Slide 3 — How it works, without the compiler theory

**Say this:**
> "You don't need to know how a compiler works to get this, so here's the
> plain version. Step one, CodeOwl reads each file's *shape* — its
> functions, classes, database tables — the same first step a compiler takes
> before it runs your code, except CodeOwl stops right there; it never
> executes anything. Step two, it connects the dots: normal imports, but also
> the *hidden* connections — a frontend doing `fetch('/api/checkout')` has no
> import statement linking it to the backend route that handles it, but
> that's a real connection a developer cares about, and CodeOwl finds it by
> matching the string. Step three, an AI fills in the plain-English
> description for each piece, and CodeOwl stitches the facts it already knows
> — signatures, dependency lists — around that prose, so the AI never has to
> restate what's already known for certain. Step four, all of that is served
> back out over MCP to whoever asks."

**Slide bullets:**
1. **Read the grammar** — parse every file into its functions, classes,
   tables. Pure syntax, no cross-file knowledge yet.
2. **Connect the dots** — resolve real imports *and* the hidden
   string-carried links (a URL, a table name) that plain code search can't
   see.
3. **Write the story** — an AI writes the plain-English description; CodeOwl
   fills in the facts (signatures, dependencies) itself, so nothing the
   graph already knows gets left to chance or hallucination.
4. **Serve it** — over MCP to AI assistants, and as plain Markdown in git for
   humans.

**Diagram — the pipeline:**

```
 YOUR CODE                     1. READ THE GRAMMAR
 ┌────────────┐                (parse each file's shape —
 │  *.ts/.tsx │                 no execution, ever)
 │  *.py      │  ─────────────────────────────────>  Symbols
 │  *.java    │                                       (functions, classes,
 │  *.rs      │                                        tables, ...)
 └────────────┘                                           │
                                                            │ 2. CONNECT THE DOTS
                                                            │  - real imports
                                                            │  - hidden string links,
                                                            │    e.g. fetch("/api/x")
                                                            │    or .from("table")
                                                            v
                                                    ┌───────────────┐
                                                    │  Dependency    │
                                                    │  Graph         │
                                                    └───────┬───────┘
                                                            │ 3. WRITE THE STORY
                                                            │  AI writes the prose;
                                                            │  CodeOwl fills in the
                                                            │  facts it already knows
                                                            v
                                                    ┌───────────────┐
                                                    │  Specs (.md)   │
                                                    │  committed to  │
                                                    │  git           │
                                                    └───────┬───────┘
                                                            │ 4. SERVE IT
                                                            v
                                         MCP  ──►  Claude Code / VS Code / Kiro
                                              ──►  Humans, as Markdown in the repo
```

**One line for management:** *"Step one and two are just fact-checking — no
AI, nothing to hallucinate. The AI only ever writes the English paragraph on
top of facts CodeOwl has already verified."*

---

## Slide 4 — Why you can actually trust it

**Say this:**
> "Two things make this different from every internal wiki that's ever died
> of neglect. First: CodeOwl fingerprints the code. Every spec is stamped
> with a fingerprint of exactly what it was written from. The moment the
> underlying code changes, that fingerprint no longer matches — and the next
> time anyone asks for that spec, CodeOwl says so immediately: 'this is out
> of date, and here's exactly what moved.' Nobody has to remember to update
> docs; the drift is caught mechanically, every single time. Second:
> CodeOwl itself never calls an AI model and never holds an API key. It
> assembles the facts and hands the writing job to whichever AI assistant
> you're already using in your session. That means no new vendor
> relationship, no new place your code gets sent — it's exactly the same
> trust boundary as the coding assistant you already use today."

**Slide bullets:**
- **Specs can't silently rot.** Every spec is fingerprinted against the
  code it describes. Code moves → fingerprint mismatch → spec is instantly
  flagged *stale*, with the exact thing that changed named, not just "this
  might be old."
- **CodeOwl never calls an AI model itself and holds no credentials.** It
  assembles context and hands the writing to whatever AI assistant is
  already in your session — same trust boundary you already have, no new
  vendor, nothing new to secure.
- **It sees connections `grep` can't** — the string-carried links (API
  calls, database table names) that a plain text search misses because
  there's no `import` statement to follow.
- **One document, two readers** — the same feature write-up a business
  analyst can read top to bottom is the exact document an AI agent uses to
  find the right files without re-exploring.

**Diagram — the fingerprint check (why staleness can't hide):**

```
  BEFORE anyone touches the code:          AFTER someone edits the function:

  ┌───────────────────┐                    ┌───────────────────┐
  │ spec says:         │                    │ spec says:         │
  │  "validates the    │                    │  "validates the    │
  │   email, then      │                    │   email, then      │
  │   saves the row"   │                    │   saves the row"   │
  │                     │                    │                     │
  │ fingerprint: A1     │                    │ fingerprint: A1     │ <- unchanged,
  └───────────────────┘                    └───────────────────┘    it's the OLD spec
  code's fingerprint: A1                    code's fingerprint: B7  <- code changed!

        MATCH                                      MISMATCH
   → spec served as CURRENT              → spec instantly flagged STALE
                                            (old text still shown, plus
                                             exactly what moved)
```

**Diagram — a connection plain text search misses, that CodeOwl catches:**

```
 frontend/CheckoutForm.tsx                   backend/routes/checkout.ts
 ┌─────────────────────────┐                ┌─────────────────────────┐
 │ fetch("/api/checkout")  │── (string, ───►│ export async function    │
 │                          │   no import     │   POST(...) { ... }      │
 └─────────────────────────┘   statement)    └────────────┬─────────────┘
                                                            │ .from("orders")
                                                            │  (also a string!)
                                                            v
                                                 ┌─────────────────────┐
                                                 │ CREATE TABLE          │
                                                 │  orders ( ... )       │
                                                 └─────────────────────┘

 grep across these 3 files: sees no connection at all.
 CodeOwl: sees one complete feature — click → route → database write —
          and writes the single narrative that explains all of it.
```

**One line for management:** *"This isn't a wiki someone has to remember to
update — it's checked against the real code, automatically, every time it's
read. If it's wrong, it says so instead of lying to you."*

---

## Slide 5 — Where this stands today, and where it's going

**Say this:**
> "This isn't a concept — it's a working tool, self-hosted, written in Rust
> as a single binary. It already understands TypeScript/Next.js, Rust, Java,
> and Python/FastAPI codebases end to end, including database schemas. It's
> already running against real repos, including CodeOwl's own source code —
> we used it to document itself. What's next is broadening it: a Java
> framework called Quarkus, then support for repos that mix languages, like
> a Python backend next to a React frontend in the same repo, which is
> extremely common here. I'm looking for a couple of real teams to pilot
> this against their own repos next."

**Slide bullets:**
- **Live today:** TypeScript/Next.js, Rust, Java, Python/FastAPI — including
  each stack's database layer, resolved down to real table columns.
- **Proven on a real corpus**, not a toy demo — including dogfooding it on
  CodeOwl's own codebase.
- **Next up:** a Java framework (Quarkus), then repos that mix more than one
  language in one place — a very common real-world shape here.
- **The ask:** pilot teams. Point it at a real repo, generate specs for one
  feature end to end, tell us where it's wrong.

**Diagram — coverage today, and what's next:**

```
   SHIPPED                                           IN PROGRESS   NEXT
 ┌───────────┬───────────┬───────────┬───────────┐  ┌───────────┐ ┌──────────┐
 │TypeScript │   Rust     │   Java    │  Python +  │  │  Java +    │ │ Repos that│
 │+ Next.js  │           │           │  FastAPI   │  │  Quarkus   │ │ mix multi-│
 │(+ SQL     │           │           │  (+ schema │  │            │ │ ple stacks│
 │ schema)   │           │           │   models)  │  │            │ │ in one    │
 └───────────┴───────────┴───────────┴───────────┘  └───────────┘ └──────────┘
```

**One line for management:** *"It already works on four real stacks today —
this is a maturity and coverage roadmap, not a research bet."*

---

## Slide 6 — "Isn't this just Graphify?"

**Say this:**
> "A few of you have already asked me this, so let's address it head-on
> instead of leaving it for Q&A: how is this different from Graphify? Short
> answer — Graphify maps your codebase. CodeOwl writes and maintains its
> documentation. Those sound similar but they're different jobs. Graphify
> builds a graph and tags every connection as confirmed or inferred — that's
> genuinely useful, and it's honest about what it knows versus guesses. But
> nothing about that graph persists an *answer*. If you ask 'how does
> checkout work end to end,' Graphify hands you a subgraph and you
> read it yourself, every single time you ask — its own generated report is
> a whole-repo highlights reel, not a per-feature write-up. CodeOwl's
> feature spec *is* that write-up: a real document that exists whether or
> not anyone's currently asking, checked against the code, so the next
> person who asks the same question gets an instant, verified answer
> instead of redoing the reading. One more precise point, because I don't
> want to overclaim: Graphify's own code parsing is also local, no LLM,
> same as us — that's not the difference. The real one is narrower and
> still true: CodeOwl never calls an LLM for anything, at all, ever.
> Graphify does, for its docs/PDF/image passes. We're not trying to out-build
> a funded platform at everything it does. We're solving the one thing it
> doesn't."

**Slide bullets:**
- **Graphify maps. CodeOwl documents.** A graph of confirmed/inferred
  connections is genuinely useful — but nobody's written down the *answer*
  to "how does this work," so you re-derive it from the subgraph every time
  you ask. A feature spec is that answer, persisted, checked against the
  code, ready before anyone asks.
- **Not a feature fight.** Graphify is broader on purpose — 40+ languages,
  docs/PDFs/video, PR triage, community detection. CodeOwl isn't trying to
  match that surface. It's narrower and deeper on one specific problem:
  documentation that doesn't rot.
- **A precise privacy claim, not an inflated one.** Graphify's code
  extraction is local too — that's not the distinction. The real one:
  CodeOwl never calls an LLM for *anything*, ever; Graphify does for its
  non-code passes.

**Diagram — same question, two different answers:**

```
 "How does checkout work, end to end?"

 Graphify:                                 CodeOwl:
 ┌─────────────────────────────┐           ┌─────────────────────────────┐
 │ returns a subgraph:          │           │ returns a feature spec:      │
 │  Page --uses--> Form          │           │  "Checkout" (.md)            │
 │  Form --EXTRACTED--> Route    │           │  a written, numbered         │
 │  Route --INFERRED--> Table    │           │  narrative -- persisted,     │
 │                                │           │  checked against the code,   │
 │ you read it and write the      │           │  existed before you asked    │
 │ narrative yourself, again      │           │                               │
 └─────────────────────────────┘           └─────────────────────────────┘
      re-derived, every time                    written once, reused, verified
```

**One line for management:** *"We're not trying to out-build a funded
platform at everything it does. We're solving the one thing it doesn't:
documentation a human can trust and an agent can act on, that never goes
silently out of date."*

---

## Live demo script (~5 minutes)

Keep this tight — the goal is to make Slide 4's trust claim *tangible*, not
to tour every MCP tool.

1. **Ask a real question, get a real answer.** Open the repo in Claude Code
   (or whatever assistant you're demoing with) with CodeOwl connected. Ask
   something feature-shaped: *"How does \[a real feature in the demo repo]
   work end to end?"* Show the answer coming back instantly from a cached
   feature spec — not from the assistant re-reading a dozen files live.

2. **Prove the fingerprint isn't decorative.** Pick a small, safe, visible
   edit — rename a parameter, change a return type, anything that shows up
   in the spec's own text. Make the edit. Ask for the same spec again on the
   spot. Show it flip to **stale**, naming exactly what moved, without
   anyone having told it to check.

3. **Show a connection `grep` would miss.** Pick a database table or an API
   route in the demo repo. Ask CodeOwl who calls/touches it. Show it
   resolving a `fetch("/api/…")` or a `.from("table")` string — a link a
   plain-text search across the repo would not find — to the real file on
   the other end.

4. **(Optional, if time allows) Show the spec as a human document.** Open
   the actual `docs/specs/**/*.md` file in the repo on GitHub. Point out
   it's plain Markdown, sitting in git, reviewable in a PR like any other
   change — not a separate system to trust or maintain.

**If something breaks live:** fall back to a pre-recorded terminal capture
of steps 1–3 rather than debugging live in front of the room — the point of
the demo is the trust story in Slide 4, not a flawless live query.

---

## Q&A cheat sheet (not slide content — prep for the room)

- **"Does this send our code to an external AI?"** Only exactly as much as
  your coding assistant already does today. CodeOwl itself holds no API key
  and makes no model calls — it hands the writing job to whatever assistant
  you're already using and already trust with this code.
- **"Isn't this just another doc site that'll rot like the last one?"** No —
  that's the whole design point. Every spec is checked against a fingerprint
  of the real code on every read; if the code moved, the spec says so
  immediately instead of quietly going wrong.
- **"What languages does it support today?"** TypeScript/Next.js, Rust,
  Java, and Python/FastAPI, each including their database-schema layer.
  Quarkus and multi-language repos are actively being built next.
- **"What's the catch?"** It's a young, actively developed tool — coverage
  is real but still growing stack by stack, and generation still needs an
  AI assistant in the loop to actually write the prose (CodeOwl won't do
  that part on its own, by design).
- **"Where do the docs actually live?"** In git, as plain Markdown, in a
  mirrored `docs/specs/` tree — reviewable in a normal pull request, no new
  system to log into.
