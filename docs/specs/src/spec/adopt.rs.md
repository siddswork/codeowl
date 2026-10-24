---
type: file
title: "src/spec/adopt.rs"
description: "Specification for `src/spec/adopt.rs`."
generated: { by: CodeOwl }
resource: "../../../../src/spec/adopt.rs"
source_paths: [src/spec/adopt.rs]
file: { source_hash: , deps_hash: , spec_hash:  }
symbols:
  src/spec/adopt.rs::short_name: { source_hash: ef9a268dff165086ce076268e9df3420ebfd4dc6b2e4855a68278c361517d09f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 313085351bd39d105d914df6646b03be0d1d1398f2d4cbf94f7b89840dcaf18e }
---
# src/spec/adopt.rs
> Generated using [CodeOwl](https://github.com/siddswork/codeowl)
## Summary


## `short_name`
`fn short_name(id: &str) -> &str`
### Summary
Extracts the plain, human-readable name from a symbol's full stable id, for use as a section heading in a rendered spec document.
### Behavior
Splits the id on `::` and returns the last segment — so `src/graph.rs::Graph::build` becomes `build`, the part a reader actually recognizes as the symbol's own name, dropping the file and containment prefix.
### Depends on
- (none)
