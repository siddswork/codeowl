# Cold-agent MCP tool description test

A reusable methodology (not a script — the eval itself needs an LLM's
judgment, not a deterministic assertion) for answering one repeatable
question: **are CodeOwl's live `tools/list` descriptions enough for an
agent with zero CodeOwl codebase access to use the tools correctly and
reach accurate conclusions?**

First run: 2026-09-25, against CodeOwl's own repo, found 6 real
description gaps (all fixed same session — see `src/mcp.rs` commits
`c8f37ae`, `31ba65c`, `0ec353c`). The method below generalizes that
run so it can be repeated against any repo without hand-picking new
"gotcha" targets each time.

## When to re-run this

- Any time a `#[tool(description = ...)]` string in `src/mcp.rs`
  changes.
- Any time a tool's response shape changes (new field, changed
  semantics of an existing one).
- Before a milestone ships that changes what an existing tool returns
  — e.g. M21's field extraction will change `get_symbol`'s `children`
  contents, which is exactly the kind of change this test is built to
  catch.
- Periodically against a repo in a different stack than the one just
  tested — a gap that's invisible on a Rust repo (this project) can be
  the first thing a cold agent trips on in a Python/Java/TS one.

## How to launch it

Fill in `{REPO_PATH}` below (an absolute path to any repo CodeOwl has
already indexed — a fresh `.codeowl/` build isn't part of what this
tests) and launch it via the `Agent` tool, `subagent_type:
"general-purpose"`, with the filled-in text below as the prompt.
Nothing about this requires being inside the CodeOwl repo itself —
the subagent only ever touches `{REPO_PATH}` through the MCP tools.

After it reports back:

1. **Check it actually stayed in bounds.** It self-reports which
   tools it used; there's no harder sandbox than the instruction
   itself, so treat a suspiciously clean or suspiciously detailed
   answer (more specific than a tool response could plausibly supply)
   as a reason to look closer, not proof of a violation — but don't
   just accept the self-report uncritically either.
2. **Judge the findings, don't just relay them.** Sanity-check
   surprising claims about tool behavior against the actual
   `src/mcp.rs` handler code before treating them as real gaps — the
   first run caught the subagent making at least one unverified
   inferential leap (claiming three tools shared a behavior when only
   two actually did). A finding is real when the source confirms it,
   not just when the subagent asserts it confidently.
3. **Fix what's real, re-run only what changed** — don't re-run the
   whole test after every single-line description tweak; the
   trigger list above is for material changes.

## The prompt

```
You are simulating a coding agent that has JUST connected to a
repo's MCP server for the very first time. You have zero prior
knowledge of this specific tool ("CodeOwl") beyond what its own MCP
tool descriptions tell you when you call them.

HARD CONSTRAINT, do not violate it: for this entire task, you may
ONLY use tools whose name starts with `mcp__codeowl__`. Do NOT use
Read, Grep, Glob, Bash, or any other file-inspection tool at any
point, even to "double check" something — if a tool description
doesn't give you enough to answer confidently, say so in your report
rather than reaching outside the allowed toolset.

The repo you're working against is at: {REPO_PATH}

Do the following, in order, logging each tool call and its raw
result as you go:

1. Use search_code and/or get_spec_coverage to find your bearings —
   you don't know this repo's shape yet. Pick 3-4 real, distinct
   symbols to use for the rest of this task: at least one exported
   function or type, at least one thing that looks like a
   method/member (not a top-level export), and at least one file
   with real internal or external imports.

2. For one of your chosen symbols, call get_symbol, then get_callers
   on it. Before looking at the result, predict in one sentence what
   an empty `callers` list would mean. Then call it, and state
   whether your prediction changes anything about what you'd tell a
   user who asked "what else in this codebase uses this?"

3. For a container type (class/struct/interface — whatever this
   stack calls it) among your chosen symbols, call get_symbol and
   try to answer "what fields/members does this have?" purely from
   the response. State your confidence, then decide whether you need
   another tool call to be sure, and make it if so.

4. For your chosen file with imports, call get_callees. For each
   entry with `resolved_id: null`, state what you believe that
   means, and whether the description gave you enough to distinguish
   between the possible reasons.

5. Call get_spec on any one of your chosen symbols or files. Explain
   what `status` and `smells` (if present) each independently tell
   you, and whether a caller could safely trust the content if
   `smells` is non-empty but `status` is "current" (or vice versa).

6. Pick one thing you were NOT explicitly asked about above and
   demonstrate using get_source to verify a real behavioral claim
   about it, the way an agent would when it doesn't trust a summary
   at face value.

Then write a report with:
- Your answer to each of the 5 numbered items above.
- For each: did the tool description alone get you to a correct,
  confident answer, or did you have to guess, cross-check with
  another tool, or flag genuine uncertainty? Be specific about which.
- Any tool description that told you something that turned out to be
  incomplete or misleading once you saw the actual tool result.
- An honest self-assessment: where would a less careful agent than
  you have gotten this wrong and not known it?
```
