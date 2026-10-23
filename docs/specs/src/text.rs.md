---
type: file
title: "src/text.rs"
description: "Specification for `src/text.rs`."
generated: { by: CodeOwl }
resource: "../../../src/text.rs"
source_paths: [src/text.rs]
file: { source_hash: 1c77c1ee16b4008d3decc29776d4c4bc40cf9e52829f03e7c977fcff9ad0eb1e, fingerprint: 70ed67bae98cf918ad9b0146ff9a62a327a90725324e92fae5a0d6a255205256, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: c437c0d8eec1915dd71b33d9fac761b3a17b3a7e5e04bacbb3c2ff380a49e994 }
symbols:
  src/text.rs::cap_source_text: { source_hash: 9c71efceb9b6d94527aeb30c8e94cd6ccae8ac5856cc9d0898dac922863db52f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 2e5a14a17bf4bc747b23635756c6f6220ed3ecfd10a3ccab87c591cd539e37b7 }
  src/text.rs::within_scope: { source_hash: 115118d8ff0fd7a3fd0f74e616b2e4fdfcfc3f9ab09abff139c2345e1b52c3fb, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 185995f6230182a04b7f1a032b80801f79b7dd0c320ac0c97bcc33d31a65ab0a }
  src/text.rs::strip_value_for_fold: { source_hash: dc96e3f0335b34de327e8f6086ecaf55d0a3233df141b1b851a48c8711186c4e, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: dad5170662677c8ea8b5fd8730c8ea3898c3d755994e9bd8bb9260840743bace }
---
# src/text.rs
## Summary
This file holds small string-manipulation helpers shared verbatim across every stack pack. Unlike `lang.rs` (mostly TypeScript-specific logic) or `hash.rs` (hashing), it exists purely so identical text-processing logic isn't hand-copied into each pack's own extractor separately — currently just `strip_value_for_fold`, the bracket-depth-aware helper that cuts a field's initializer value off its signature before that signature gets folded into a container's `interface_hash`.

## `cap_source_text`
`pub(crate) fn cap_source_text(text: String, max_bytes: usize) -> (String, bool)`
### Summary
Shortens source code to a byte limit without writing anything into it, and reports whether it cut. It is the variant used when the text is meant to be read as real code.
### Behavior
Returns the text and `false` untouched when it is within `max_bytes`. Otherwise it cuts at the limit, moving back to the nearest character boundary so a multi-byte character is never split, and returns the shortened text with `true`. Unlike `cap_generation_text`, no marker sentence is appended, since an English note in the middle of a function body would corrupt the code a caller is trying to check. The `bool` is how truncation is signalled, and `get_source` passes it on as its `truncated` field. A limit of 0 gives an empty string and `true` for any non-empty text.
### Depends on
- (none)

## `within_scope`
`pub(crate) fn within_scope(path: &str, scope: &str) -> bool`
### Summary
Tells whether a file or folder path lies inside a given folder, matching whole path pieces and not just starting letters. It is used to narrow coverage reports and code searches to one directory.
### Behavior
Returns true if `scope` is empty (everything matches), if `path` equals `scope`, or if `path` starts with `scope` followed by a `/`. The slash is what keeps matching honest: scope `lib/email` matches `lib/email` and `lib/email/send.ts` but not `lib/email.ts` or `lib/email-utils/x.ts`, which a plain string-prefix test would wrongly accept. A trailing slash on `scope` is not handled specially, so `lib/` would only match paths that begin with `lib//`. The coverage code and `search_code` both use it so their boundaries agree. Pure function, no failure mode.
### Depends on
- (none)

## `strip_value_for_fold`
`pub(crate) fn strip_value_for_fold(signature: &str) -> &str`
### Summary
Cuts a field's or constant's stored signature text down to just its name and type, dropping the initializer value — used when folding a member's signature into its container's `interface_hash` (the hash that tracks a container's public shape), since a member's *value* is deliberately not part of that public surface.
### Behavior
Scans the signature character by character looking for the real, top-level `=` — the one that actually separates the declaration from its initializer, not one buried inside something else. It tracks bracket depth (`(`, `[`, `{` increment; their closing counterparts decrement) so it never mistakes an `=` nested inside a leading annotation, decorator, or macro call for the real one — a naive first-`=` split would wrongly cut Java's `@Column(name = "x")` or Python's `Field(primary_key=True)` right at that inner `=`, keeping only a useless prefix instead of the field's actual name and type. Once it finds a top-level `=`, it returns everything before it, trimmed of trailing whitespace; if no top-level `=` exists at all, it returns the whole signature unchanged (there's no value to strip).

This is best-effort, not perfect, for a multi-declarator line sharing one signature string (Java's `int a, b = 2;`): it cuts at the first top-level `=`, which can leave a later declarator's fragment still attached. That imprecision already existed in the stored `signature` itself before this function was written — both declarators already shared identical text — so this doesn't make anything worse, it just doesn't fix a pre-existing edge case either.
### Depends on
- (none)
