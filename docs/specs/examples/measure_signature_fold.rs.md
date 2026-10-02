---
kind: file
source_paths: [examples/measure_signature_fold.rs]
file: { source_hash: fadc95706043b18f00e3109413c468c258a0005d4d60d51ce9993337640da873, deps_hash: b6a3272c4057cc497094fdae34762d5fb193ee39662db690a6e0f5aae3be4a4f, spec_hash: 7318492554ac15ab6200c0a592bcadc1d50b78ec47b5291a6daf15aca1db1920 }
symbols:
  examples/measure_signature_fold.rs::is_qualifying_member: { source_hash: ec63724d7131281115dabfc727de555caa06013319b148773cd520eeb7b7316c, deps_hash: 34a95995527dc5f673d09639b70c27217984732249263b23f878b003a1bf1598, spec_hash: df1a378f8fc90c2ed804ba5c0703e11aa9b61d1c1b898b3c63adee6e2f58b6d1 }
  examples/measure_signature_fold.rs::qualifying_member_names: { source_hash: 8197b28be753c35e2effb95d03854e6e5f8708ee0669ec7629da022ce90a74f1, deps_hash: 1905c59a809cb146e2d62da82589212debb7a142f8d907564aa5b0da29373eae, spec_hash: 9f943d4ef6fc323ce425ef10401df593d2c605a7c23c5b3e3746676f80290b54 }
  examples/measure_signature_fold.rs::mentions_word: { source_hash: 1643d1d1e8545c7b1fca3be56eee5a8d63871946d4aafa8fd33239f2eb1ad2be, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 96f392f946055a273a6ae00dc960db168d658817bbee1874004b87f4c903a705 }
  examples/measure_signature_fold.rs::report_name_collisions: { source_hash: b09f3e5d6654fc956f3777c6cc81b637fe1cd4f5e61220f16b7c820d28a17ea5, deps_hash: c26f9d6a3e6974e2d667c28f724f82f43d77dfdb43054409f1e8e28bc6038944, spec_hash: 55a1008cb77a5a7bb293b713bfbc3dfb3a2a649a3d25a1f6feef6b3ba7f276d5 }
  examples/measure_signature_fold.rs::correctness_check: { source_hash: 8faed7869e85501ae0ac0c645948b1b26fb4e5c79b68f928dcb4650c6de63c2c, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 1a346d932a624df2ff3cf0eeef10c4e18ce455b6536ff9f10bc29eb25ed71e21 }
  examples/measure_signature_fold.rs::measure: { source_hash: ac9a81f020ef790a596f9fcff32f3768ab8eec985f372c9c50fbeb19894ca3d2, deps_hash: 1df444dd72a2653710a1e710b05bfc39ccb25af714e552f0cda93375b9bcc0ea, spec_hash: e932e793d0c131f55cf4c99192e3dfc5c130a366136c26ad7873431481f19e2e }
  examples/measure_signature_fold.rs::main: { source_hash: 3d9c57809712559eb08db7e84906679e9741d5c10fdc420d814a23ad2a30fa76, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: da7bc43933bcab6088de9d12eb9cb3cf358df8a7d792c13b201ba5f8028d6b29 }
---
# examples/measure_signature_fold.rs
## Summary
A runnable measurement tool used before deciding whether a new kind of declaration should count toward a type's public shape, which is the fingerprint dependents use to know they are out of date. It was built for the question of whether fields should count and reused unchanged for the question of whether method signatures should, and it is kept as an example because it is now a repeatable method. To ask a new question, change one constant, `SIGNAL`, which names the kind of member being measured (currently functions and methods). It produces two tables for each of four fixed repos. The cost table lists, for every type with at least one public member of that kind, how many files import it today, which is exactly how many files would newly be marked stale on each edit under the new rule, counted from the real resolved-import graph and not simulated. The correctness table asks, of the importing files that already have a written spec, what share actually name one of the type's members in their text. Naming a member is necessary for that prose to become false when the member changes, though not sufficient, so the matching lines are printed to be read before trusting the percentage. It also reports types with several methods of the same name, since overloads make a single symbol id ambiguous about which declaration a fingerprint covers. The repo paths are fixed local paths on the author's machine and must be edited elsewhere, and the per-language visibility rules are approximations, adequate for an honest table but not guaranteed to match the real extractors.

## `is_qualifying_member`
`fn is_qualifying_member(pack: &str, sym: &Symbol) -> bool`
### Summary
Decides whether a member of a class or type counts for this measurement: it must be of the kind being measured (currently functions and methods) and be publicly visible in the way that language defines it.
### Behavior
Returns false at once if the symbol's kind is not `SIGNAL`, a constant set to `SymbolKind::Callable` that is the one thing to change when measuring a different kind of member. Otherwise the visibility test depends on the language pack's name, using the start of the signature (leading whitespace removed) or the member's own name, which is the part of the id after the last `::`:
- `rust`: the signature starts with `pub `.
- `java`: the signature starts with `public ` or contains ` public `.
- `typescript-next`: the signature does not start with `private` and the name does not start with `#`, so members are public unless marked otherwise.
- `python`: the name does not start with an underscore.
- any other pack: false.

These are approximations written for a measurement script and are not guaranteed to match the real extractors' own visibility checks. They only need to be right often enough to give an honest table. It cannot fail.
### Depends on
- `src/symbol.rs::Symbol` — codeowl::symbol

## `qualifying_member_names`
`fn qualifying_member_names(graph: &Graph, pack: &str, container_id: SymbolId) -> Vec<String>`
### Summary
Lists the names of a container's public members of the measured kind, leaving out very common or very short names that would match too much text to be a useful signal.
### Behavior
Goes through every symbol in the graph and keeps those whose parent is the given container and that pass `is_qualifying_member` for the pack. For each it takes the member's bare name, the part of the id after the last `::`. It then drops a name that is in the `GENERIC_NAMES` list (words that appear in prose about almost anything) or that is two characters or fewer. The result is the remaining names in graph order, possibly with duplicates if two members share a name. It scans the whole symbol list for each call, which is acceptable for a one-off measurement. It cannot fail, and returns an empty list for a container with no qualifying members.
### Depends on
- `src/graph.rs::Graph` — codeowl::graph
- `src/symbol.rs::SymbolId` — codeowl::graph

## `mentions_word`
`fn mentions_word(text: &str, word: &str) -> bool`
### Summary
Checks whether a piece of text contains a given name as a whole word, so `parse` is found in "calls parse here" but not in "parser" or "reparse".
### Behavior
Searches for `word` repeatedly through the text. For each occurrence it looks at the byte just before and the byte just after, and counts the match only if neither is an ASCII letter, digit or underscore (or the match is at the very start or end). The first such match returns true. After a rejected match it moves one byte forward from where that match began and searches again, so overlapping candidates are considered. It returns false if none qualify. The boundary test looks at ASCII identifier characters only. Because it steps forward by one byte, a search word that begins with a non-ASCII character could land in the middle of a multi-byte character and make the slice panic; the names measured here are ordinary identifiers, so this is not expected in practice. It is case-sensitive.
### Depends on
- (none)

## `report_name_collisions`
`fn report_name_collisions(graph: &Graph)`
### Summary
Prints every class or type that has two or more methods sharing one name, such as overloaded Java methods. Methods are tracked by name, so two with the same name share one symbol id, and then it is unclear which declaration a fingerprint really covers.
### Behavior
Only runs when the measured kind (`SIGNAL`) is `Callable` and does nothing otherwise, since fields almost never collide this way. For each container in the graph it finds the container's id, then counts its direct children of kind `Callable` by their bare name (the part of the id after the last `::`). Any name counted more than once is a collision. If a container has any, it prints a line naming the container and the list of colliding names with their counts, for example `Foo has overloaded/colliding names: [("bar", 2)]`. Containers with none print nothing. The order of containers follows the graph and the order of names inside a line follows the hash map, so it is not sorted. It prints only and returns nothing.
### Depends on
- `src/graph.rs::Graph` — codeowl::graph
- `src/symbol.rs::SymbolKind` — codeowl::symbol

## `correctness_check`
`fn correctness_check(
    root: &Path,
    label: &str,
    qualifying: &[(String, Vec<String>, HashSet<String>)],
)`
### Summary
Measures how often the existing written specs of importing files actually mention the member names of a container they import. If a spec never names a member, renaming or removing that member could not make the spec wrong, so the share that do is a gauge of how many extra stale warnings would be justified.
### Behavior
`qualifying` is a list of entries, each a container id, its qualifying member names, and the set of files that import it. Containers with no member names are skipped. For each importing file it looks for that file's spec at `docs/specs/<path>.md` under the repo root; a file with no spec, or one that cannot be read, is ignored. A file is counted once even if it imports several qualifying containers, and when that happens only the first container's member names are checked for it, which can understate the count. Each spec found counts toward "could mention". If its text names any member as a whole word (`mentions_word`), it also counts toward "actually mentions", and every line of the spec naming that member is printed with the label, file and member so a person can read them before trusting the percentage.

At the end it prints a summary line with both counts and the percentage, with 0% when no specs were found, so there is no division by zero. A mention is only a necessary condition for the prose to become false, not a sufficient one, which is why the matching lines are printed. It reads files and prints; it writes nothing.
### Depends on
- externals: std

## `measure`
`fn measure(label: &str, root: &Path)`
### Summary
Runs the whole measurement on one repo and prints it: the cost (how many importing files would be newly marked stale per edit if member signatures counted toward a container's public shape) followed by the correctness check against the existing specs.
### Behavior
Opens the repo with `RepoIndex::open`; on failure it prints `== <label>: failed to open ==` and returns. It prints a header with the label and pack name, then goes through every container in the graph. A container qualifies if at least one of its direct children passes `is_qualifying_member` for the pack. For each, it collects the distinct files whose resolved imports target the container. Containers with importers are recorded with their importer count, and also, for the correctness step, with their qualifying member names and importer set.

It then prints: the number of qualifying containers, how many of them have importers today, the importer counts of the top five (the number of files newly invalidated per edit under the folded rule, computed from the real resolved-import graph and not simulated), the sum across all, the average (0 when there are none), and the single worst container with its importer count. After that it calls `report_name_collisions` and `correctness_check`. Importers are counted by exact container target, so imports resolved to a different symbol in the same file do not count. It prints only and returns nothing; opening the repo may refresh its local cache.
### Depends on
- `src/index.rs::RepoIndex` — codeowl::index
- `src/symbol.rs::SymbolKind` — codeowl::symbol
- externals: std

## `main`
`fn main()`
### Summary
The starting point of the measurement tool. It runs the same measurement on four fixed repos, CodeOwl itself and three test projects.
### Behavior
Calls `measure` in sequence with a label and a path for each: CodeOwl itself at the current directory, the `backend` folder of the FastAPI template, the Quarkus super-heroes repo, and `talentTrail`. The three outside paths are absolute and specific to the author's machine, so on any other machine they must be edited, or those repos will each print a failed-to-open notice. A failure to open one repo does not stop the others. The tool takes no command-line arguments, in contrast to the graph statistics tool, and returns nothing. To measure a different kind of member, change the `SIGNAL` constant, and to measure other repos, change the paths here.
### Depends on
- externals: std
