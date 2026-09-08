---
kind: file
source_paths: [src/hash.rs]
file: { source_hash: cc20920f6b3ad8a75b6f2a992f5def7182810a96edcd6818a7995014485694bd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 676a175ccd5d2ddca7ddb6bb5ba588f4f87f1131cbc9c806b0a0e659a4e4c56f }
symbols:
  src/hash.rs::hash_text: { source_hash: b68417d95d8759027bd745de13209e192545b78e88b031c08f4d4533a4ed0d16, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 23e2bd7c5f69f0ce1dbc23f9bf846094141e2f5653bbe7acbb07c0f90ab04811 }
---
# src/hash.rs
## Summary
Stable content hashing for every hash CodeOwl persists — `source_hash`, `interface_hash`, `deps_hash`, `spec_hash`. One function, `hash_text`, wrapping BLAKE3. Deliberately not `std::hash` / `DefaultHasher`, whose output is only guaranteed stable within a single build of a single Rust version — CodeOwl compares hashes across runs and toolchain upgrades, so it needs a fixed algorithm that gives the same digest on any machine.

## `hash_text`
`pub fn hash_text(input: &str) -> String`
### Summary
The one hashing primitive the whole codebase uses — turns any string into a stable lowercase-hex digest. Every `source_hash`, `interface_hash`, `deps_hash`, and `spec_hash` is a `hash_text` of some canonical string.
### Behavior
BLAKE3 of the UTF-8 bytes, rendered as its 64-char hex string. Deterministic and platform-independent, so a `.codeowl/` cache and a committed spec's frontmatter compare identically across machines. No salt, no truncation. Callers are responsible for building a canonical input (sorted lists, fixed separators) — this function just hashes whatever it's given.
### Depends on
- (none)
