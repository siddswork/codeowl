---
kind: file
source_paths: [src/text.rs]
file: { source_hash: 1c77c1ee16b4008d3decc29776d4c4bc40cf9e52829f03e7c977fcff9ad0eb1e, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c437c0d8eec1915dd71b33d9fac761b3a17b3a7e5e04bacbb3c2ff380a49e994 }
symbols:
  src/text.rs::strip_value_for_fold: { source_hash: dc96e3f0335b34de327e8f6086ecaf55d0a3233df141b1b851a48c8711186c4e, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: dad5170662677c8ea8b5fd8730c8ea3898c3d755994e9bd8bb9260840743bace }
---
# src/text.rs
## Summary
This file holds small string-manipulation helpers shared verbatim across every stack pack. Unlike `lang.rs` (mostly TypeScript-specific logic) or `hash.rs` (hashing), it exists purely so identical text-processing logic isn't hand-copied into each pack's own extractor separately — currently just `strip_value_for_fold`, the bracket-depth-aware helper that cuts a field's initializer value off its signature before that signature gets folded into a container's `interface_hash`.

## `strip_value_for_fold`
`pub(crate) fn strip_value_for_fold(signature: &str) -> &str`
### Summary
Cuts a field's or constant's stored signature text down to just its name and type, dropping the initializer value — used when folding a member's signature into its container's `interface_hash` (the hash that tracks a container's public shape), since a member's *value* is deliberately not part of that public surface.
### Behavior
Scans the signature character by character looking for the real, top-level `=` — the one that actually separates the declaration from its initializer, not one buried inside something else. It tracks bracket depth (`(`, `[`, `{` increment; their closing counterparts decrement) so it never mistakes an `=` nested inside a leading annotation, decorator, or macro call for the real one — a naive first-`=` split would wrongly cut Java's `@Column(name = "x")` or Python's `Field(primary_key=True)` right at that inner `=`, keeping only a useless prefix instead of the field's actual name and type. Once it finds a top-level `=`, it returns everything before it, trimmed of trailing whitespace; if no top-level `=` exists at all, it returns the whole signature unchanged (there's no value to strip).

This is best-effort, not perfect, for a multi-declarator line sharing one signature string (Java's `int a, b = 2;`): it cuts at the first top-level `=`, which can leave a later declarator's fragment still attached. That imprecision already existed in the stored `signature` itself before this function was written — both declarators already shared identical text — so this doesn't make anything worse, it just doesn't fix a pre-existing edge case either.
### Depends on
- (none)
