# Writing a `STYLE.md` for your repo

A `docs/STYLE.md` tells the agent that writes your specs who they are for. It
is optional. Without one, specs are accurate but written for a reader who
already knows the code. With one, they are written for the reader you name.

## What it is and where it goes

- The path is exactly `docs/STYLE.md`. `/codeowl-generate` reads it, if it
  exists, before it writes anything.
- It sits beside `docs/specs/`, not inside it, so every file in `docs/specs/`
  is a spec. It needs no frontmatter.
- Only `/codeowl-generate` reads it. CodeOwl itself never does, and nothing
  checks that a spec follows it. It is advice to the writing agent.

## What it can and cannot change

| It can change | It cannot change |
|---|---|
| Who the reader is and how much they know | The headings (`### Summary`, `### Behavior`, and so on) |
| Which terms to explain in passing | The rule that a spec is based only on the code it was given |
| Whether to lead with an example | The quality check: a section needs real prose, and a "see the source" cop-out is refused |
| House habits (for example, no internal ticket numbers) | The signature and dependency lines, which CodeOwl writes itself |

Changing `STYLE.md` does not make any existing spec stale, because a spec goes
stale when its code changes. A new style reaches specs as they are written or
rewritten. To restyle a spec that is already current, delete its file and run
`/codeowl-generate` again; a current spec is never offered.

## What a good one looks like

1. **Name the reader in one sentence.** "A support engineer who has never
   written Rust" gives the agent something to aim at. "Developers" does not.
2. **List the vocabulary to explain.** Say which terms of your domain the
   reader will not know, and to explain each one in parentheses the first time
   it appears. If you keep a glossary, say to point to it for terms that
   cannot be avoided.
3. **Ask for an example when it helps.** "Show the input and the output" turns
   an abstract sentence into one a reader can check.
4. **Say where it may get technical.** Many repos want `### Summary` in plain
   language and `### Behavior` free to be exact.
5. **Add your house rules.** For example, no names of tickets or milestones in
   a spec.
6. **Keep it short.** The agent reads it on every run, and a long file is read
   less carefully. This repo's own `docs/STYLE.md` is about sixty lines; thirty
   is plenty to start.
7. **Include a way to check.** A one-line test the writer can apply to its own
   text ("read the summary aloud as the reader") beats three more rules.

Leave out anything about structure, facts about a particular file (the graph
already knows them) and instructions the agent cannot act on.

## A template to copy

Save this as `docs/STYLE.md`, then replace the bracketed parts.

```markdown
# Spec style for this repo

`/codeowl-generate` reads this before writing any spec here. It sets the
audience and tone, not the structure.

## Who the reader is

[One sentence: their job, and what they do not know.]

## Rules

1. `### Summary` must make sense to that reader. Lead with what the code does
   for them, then how.
2. Explain a [domain] term in parentheses the first time it appears, for
   example "[term] ([plain meaning])".
3. For a term you cannot avoid repeating, explain it once, then add "(see
   `[GLOSSARY.md]`)" and use it freely after that.
4. Prefer a concrete example: show an input and the output it gives.
5. `### Behavior` may be more technical. Spell out an acronym on first use.
6. Keep it tight: cut hedge words, and do not restate the signature.
7. [Any house rule, for example: never mention ticket or milestone numbers.]

## Quick test

Read your `### Summary` aloud as [the reader]. If a sentence would make them
stop and re-read, rewrite it or add the parenthetical.
```

## An example of the difference

The function below is made up for this guide, and so are the two specs. They
show the shape of the change, not real output.

```rust
pub fn margin_shortfall(positions: &[Position], collateral: f64, haircut: f64) -> f64 {
    let required: f64 = positions
        .iter()
        .map(|p| p.notional.abs() * p.initial_margin_rate)
        .sum();
    let usable = collateral * (1.0 - haircut);
    (required - usable).max(0.0)
}
```

**Without a `STYLE.md`.** Correct, and written for someone who trades:

```markdown
### Summary
Computes the margin shortfall for a set of positions: required initial margin
less haircut-adjusted collateral, floored at zero.

### Behavior
Required margin is the sum over positions of absolute notional times the
position's initial margin rate. Usable collateral is the collateral times one
minus the haircut. The result is the required margin less the usable
collateral, and is zero when collateral covers it. Longs and shorts both
count, because notional is taken as an absolute value, so offsetting positions
are not netted.
```

**With this `docs/STYLE.md`.** The reader is "an operations analyst who knows
the spreadsheets but not the trading vocabulary"; explain each trading term
the first time, and show a worked number:

```markdown
### Summary
Works out how much more money a client must put up to cover their open trades,
or zero if they have already put up enough. Each trade needs a deposit (the
"initial margin"), and the money the client has posted counts for slightly
less than its face value (the "haircut", a discount applied because the posted
assets could lose value). For example, trades needing 50,000 against 40,000
posted with a 10% haircut leave a shortfall of 14,000.

### Behavior
Each position's deposit is its size, ignoring whether it is a buy or a sell,
times that position's margin rate; the deposits are added up. The posted
collateral is multiplied by one minus the haircut (40,000 becomes 36,000 at
10%). The result is the total deposit less that figure, and never below zero.
Because a buy and a sell are both counted at full size, opposite trades do not
cancel each other out.
```

Both are true to the code. The second names the reader's missing vocabulary,
explains it once, and gives a number to check against.

## Checking yours

Generate two or three specs and read the summaries as the reader you named. If
one still stops you, the fix is usually a missing term in your list, or a rule
that is too vague to act on. Edit `docs/STYLE.md`, delete those specs, and
generate them again.

This repository's own [`docs/STYLE.md`](../docs/STYLE.md) is a complete working
example.
