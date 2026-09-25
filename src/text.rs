//! Small string-manipulation helpers shared verbatim across packs — unlike
//! `lang.rs` (mostly TypeScript-specific) or `hash.rs` (hashing), this
//! module exists purely so identical logic isn't hand-copied into every
//! pack's own extractor.

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
