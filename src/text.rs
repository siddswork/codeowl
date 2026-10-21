//! Small string-manipulation helpers shared verbatim across packs and the
//! spec and search layers — unlike `lang.rs` (mostly TypeScript-specific)
//! or `hash.rs` (hashing), this module exists purely so identical logic
//! isn't hand-copied into every pack's own extractor, and so a pack or
//! `search.rs` never has to import from `spec.rs` for a plain string
//! utility.

/// Cut `text` to at most `max_bytes`, on a `char` boundary, with **no**
/// marker appended — unlike `spec::cap_generation_text`, whose marker text
/// is meant to be read by an LLM assembling a prompt. `get_source` returns
/// what's meant to be read *as* source code; splicing an English sentence
/// into the middle of a function body would corrupt exactly the thing a
/// caller is trying to verify. The signal is the returned `bool`, which
/// `get_source`'s `truncated` field carries instead.
pub(crate) fn cap_source_text(text: String, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text, false);
    }
    let mut cut = max_bytes.min(text.len());
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    (text[..cut].to_string(), true)
}

/// Whether `path` (a file or directory) falls under `scope` — an exact
/// match, or a true subdirectory (`scope` plus a `/` boundary), never a
/// bare string-prefix match: `"lib/email"` must not also catch
/// `"lib/email.ts"` or `"lib/email-utils/"`. An empty `scope` matches
/// everything (files/rollups only — see `spec::coverage`'s own doc comment
/// on why an empty-but-`Some` scope still excludes features/system).
/// Shared with `search_code`'s `path` filter (`search.rs`) — the same
/// boundary discipline applies there for the same reason: a query scoped
/// to `"lib"` shouldn't also match `"library/foo.ts"`.
pub(crate) fn within_scope(path: &str, scope: &str) -> bool {
    scope.is_empty() || path == scope || path.starts_with(&format!("{scope}/"))
}

/// The comment block at the very top of `text`, as written: leading `//`,
/// `/* … */` and `#` comment lines up to the first blank or code
/// line. A Rust `#[…]` / `#![…]` attribute is not a comment. Empty when the
/// file starts with code. A Python module docstring is not recognised.
pub(crate) fn leading_comment_block(text: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let mut in_block = false;
    for line in text.lines() {
        let t = line.trim_start();
        if in_block {
            lines.push(line);
            if t.contains("*/") {
                in_block = false;
            }
            continue;
        }
        if t.is_empty() && lines.is_empty() {
            continue;
        }
        let is_hash_comment = t.starts_with('#') && !t.starts_with("#[") && !t.starts_with("#!");
        if t.starts_with("/*") {
            lines.push(line);
            in_block = !t.contains("*/");
        } else if t.starts_with("//") || is_hash_comment {
            lines.push(line);
        } else {
            break;
        }
    }
    lines.join("\n")
}

/// A field/const/attribute's stored `signature` carries its initializer
/// value verbatim (each pack's own fold comment explains why the value
/// isn't stripped at the source), so the `interface_hash` fold strips it
/// here instead, transiently, without touching the stored field. Finds the
/// real, top-level `=` by tracking bracket depth — a plain first-`=` split
/// would cut inside a leading annotation/decorator/call's own argument
/// (Java's `@Column(name = "x")`, Python's `Field(primary_key=True)`, a
/// Rust macro invocation with its own `=`), silently keeping only that
/// prefix instead of the field's real name+type. Best-effort for a
/// multi-declarator field sharing one declaration's text (Java's
/// `int a, b = 2;`): cuts at the first top-level `=`, which can leave a
/// later declarator's own fragment attached to an earlier one's fold
/// contribution — already true of `signature` itself before this fold
/// existed (both declarators already share identical text), not a new
/// imprecision.
pub(crate) fn strip_value_for_fold(signature: &str) -> &str {
    let mut depth = 0i32;
    for (i, c) in signature.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '=' if depth == 0 => return signature[..i].trim_end(),
            _ => {}
        }
    }
    signature
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_value_for_fold_cuts_at_the_first_top_level_equals() {
        assert_eq!(strip_value_for_fold("public int x = 5"), "public int x");
        assert_eq!(strip_value_for_fold("public int x"), "public int x");
    }

    #[test]
    fn cap_source_text_leaves_short_text_untouched_and_unflagged() {
        let text = "hello world".to_string();
        let (out, truncated) = cap_source_text(text.clone(), 32_000);
        assert_eq!(out, text);
        assert!(!truncated);
    }

    #[test]
    fn cap_source_text_cuts_and_flags_but_never_appends_prose() {
        // Unlike cap_generation_text, get_source's cap must never splice
        // an English marker into what's meant to be read as real source
        // code -- the `truncated` bool is the signal, not the text.
        let text = "x".repeat(1000);
        let (out, truncated) = cap_source_text(text, 50);
        assert_eq!(out.len(), 50);
        assert!(truncated);
        assert!(
            out.chars().all(|c| c == 'x'),
            "no marker text should appear in the capped output: {out}"
        );
    }

    #[test]
    fn cap_source_text_never_splits_a_multi_byte_char() {
        let text = "☃".repeat(1000); // 3 bytes each, well over any small cap
        let (out, truncated) = cap_source_text(text, 50);
        assert!(out.len() <= 50);
        assert!(truncated);
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
    }

    #[test]
    fn within_scope_respects_path_boundaries_not_bare_string_prefixes() {
        // A real bug caught dogfooding against the pilot repo: "lib/email"
        // as a bare string prefix also matches "lib/email.ts", an
        // unrelated sibling file that merely shares the prefix.
        assert!(within_scope("lib/email", "lib/email"));
        assert!(within_scope("lib/email/config.ts", "lib/email"));
        assert!(!within_scope("lib/email.ts", "lib/email"));
        assert!(!within_scope("lib/email-utils/x.ts", "lib/email"));
        assert!(within_scope("anything", ""));
    }

    #[test]
    fn leading_comment_block_takes_the_top_comment_and_stops_at_code() {
        let text = "//! Module doc line one.\n//! Line two.\n\nuse std::io;\n// later\n";
        assert_eq!(
            leading_comment_block(text),
            "//! Module doc line one.\n//! Line two."
        );
        let block = "/* header\n * more\n */\nexport const A = 1;\n";
        assert_eq!(leading_comment_block(block), "/* header\n * more\n */");
        assert_eq!(leading_comment_block("export const A = 1;\n"), "");
        // A line starting with `*` is a comment only inside a block comment.
        assert_eq!(leading_comment_block("*ptr = 1;\nfn a() {}\n"), "");
        // A Rust attribute is not a comment.
        assert_eq!(
            leading_comment_block("#![allow(dead_code)]\nfn a() {}\n"),
            ""
        );
    }

    #[test]
    fn strip_value_for_fold_ignores_an_equals_inside_a_bracketed_argument() {
        // A naive first-'=' split would cut inside the leading
        // annotation/decorator/call's own argument list, silently
        // keeping only that prefix and dropping the field's real
        // name+type.
        assert_eq!(
            strip_value_for_fold("@Column(name = \"x\")\n\tpublic int y = 5"),
            "@Column(name = \"x\")\n\tpublic int y"
        );
        assert_eq!(
            strip_value_for_fold("id: int = Field(primary_key=True)"),
            "id: int"
        );
    }
}
