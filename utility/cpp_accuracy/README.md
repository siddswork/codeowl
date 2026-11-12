# C++ accuracy check

Measures the C++ pack against the compiler. Plan step 5 of
`docs/cpp-pack-plan.md`; the method and the first numbers are
`experiments/exp-08-cpp-tree-sitter-or-compiler.md`.

```
utility/cpp_accuracy/run.sh                 # all four repositories
utility/cpp_accuracy/run.sh leveldb fmt     # some
utility/cpp_accuracy/run.sh --check         # exit 1 if a target of the plan is missed
utility/cpp_accuracy/run.sh --refresh       # rebuild the compiler's reference
```

Results are printed and written to `target/cpp-accuracy/results.json`.

## What it needs

| Need | How it is met |
|---|---|
| `g++`, `git`, `python3` with `venv` | Installed on the machine; `run.sh` stops if `g++` or `git` is missing |
| `libclang` (Python package, pinned to 18.1.1) | `run.sh` makes a virtual environment in `target/cpp-accuracy/venv` the first time |
| The four repositories at the pinned commits | Cloned next to this repository (`../test-repos/<dir>`, `dir` as in `config.json`: `leveldb`, `abseil-cpp`, `fmt`, `json`; `CODEOWL_TEST_REPOS` overrides the folder). A checkout that already exists is used as it is, with a warning if it is not at the pinned commit; its real commit is part of the cache key |
| A built probe | `cargo build --release --example cpp_probe`, done by the script |

Nothing is written inside a clone: the one generated header `leveldb` needs
(`port/port_config.h`, from its CMake template, with every optional feature
off) goes to `target/cpp-accuracy/generated/`.

## What is in it

| File | Purpose |
|---|---|
| `config.json` | The repositories, their pinned commits, compiler settings, which files are in scope, and the targets of the plan |
| `truth.py` | The compiler's reading, through libclang: every class, enum and function definition, and every file-to-symbol dependency, in one build configuration |
| `../../examples/cpp_probe.rs` | What the pack reads: symbols, includes, parse errors, one line each |
| `compare.py` | The two put side by side |
| `run.py`, `run.sh` | The driver and its environment |

The compiler's reference takes minutes (about 45 s for `leveldb`, 2 to 3 min
each for `abseil` and `json`), so it is cached in `target/cpp-accuracy/truth/`,
keyed by the configuration, the commit actually checked out, the generated
headers' templates, the `g++` and `libclang` versions, and `truth.py` itself.
A run stops with an error, and does not report 100 %, when the reference is
empty. The pack's side takes
about a second. Re-running after a change to the pack is therefore cheap.

## What the numbers mean

* **Classes, enums, functions**: of the definitions the compiler sees in the
  files in scope, the share the pack also reports (same file, kind and name).
* **Edge recall and precision**: a file depends on a top-level symbol of
  another file. The pack has no edges of its own until plan step 2, so the
  check measures the method that step will use: follow `#include` transitively
  and link to each top-level symbol of an included file whose name is written
  in the including file. When step 2 lands, point `compare.py` at the pack's
  real edges instead.
* **Targets** are section 7 of the plan: on clean code (`leveldb`) 100 % of
  classes, 99 % of functions, 97 % recall, 98 % precision; on macro- and
  template-heavy code 95 %, 97 %, 93 %, 80 %.

## Limits

* **One build configuration.** The compiler sees what one invocation sees:
  `#ifdef` branches for other platforms and disabled features do not exist
  for it. The pack reads every branch, so it can legitimately report more.
* **Precision is understated on templates.** The compiler does not resolve
  names inside a template that is never instantiated, so an edge it lacks may
  be real.
* **Files that do not compile alone** (34 of 81 in `nlohmann/json`) are still
  parsed; the compiler's recovery decides what it records for them. This is
  the compiler's count of units with errors, not the pack's count of files it
  read only partly (`parse:` in the report); the two measure different things.
* **Declarations a macro stamps out** are counted separately in the
  reference (they share one location), so a macro-heavy repository's totals
  are not undercounted.
* **Four repositories, all libraries.** No application, no generated code, no
  heavy conditional compilation. Add one before calling the pack done.
* **`abseil` is an even sample** of 160 files (`limit` in the config).
* The reference needs the same compiler version to be reproducible; the
  Python package is pinned for that reason, and the system headers come from
  the machine's `g++`.
