//! What a C++ file says about other files: the headers it includes and the
//! words it uses. Step 2 of `docs/cpp-step2-plan.md`; resolution (which repo
//! file an include means, and which symbols the words reach) is the next part.
//!
//! Both come from the lexer in `macros.rs`, which already skips comments and
//! string literals and returns each preprocessor directive whole.

use std::collections::BTreeSet;

use super::macros::{Kind, lex};
use crate::imports::{FileImports, ImportRef};

/// `imported_name` of an include: a file, not a name (the convention the
/// Java pack uses for `import a.b.*;`). No edge is ever drawn with it.
pub const INCLUDE_NAME: &str = "*";

/// Read one file's includes and the words it uses.
pub fn extract(source: &str) -> FileImports {
    let mut imports: Vec<ImportRef> = Vec::new();
    let mut words: BTreeSet<String> = BTreeSet::new();
    for tok in lex(source) {
        match tok.kind {
            Kind::Ident => {
                words.insert(tok.text.to_string());
            }
            Kind::Preproc => {
                // A backslash-newline joins lines before anything is read.
                let joined = tok.text.replace("\\\r\n", " ").replace("\\\n", " ");
                if let Some(spec) = include_specifier(&joined) {
                    let import = ImportRef {
                        specifier: spec.to_string(),
                        imported_name: INCLUDE_NAME.to_string(),
                    };
                    if !imports.contains(&import) {
                        imports.push(import);
                    }
                } else {
                    // An include names a file, not a symbol: no words.
                    words_in_directive(&joined, &mut words);
                }
            }
            Kind::Punct | Kind::Str | Kind::Number => {}
        }
    }
    FileImports {
        imports,
        identifiers: words.into_iter().collect(),
        ..FileImports::default()
    }
}

/// The path of an `#include`, with its delimiters (`"a/b.h"` or `<a/b.h>`),
/// or `None` for any other directive and for a computed include
/// (`#include MACRO`). The delimiters stay because the two forms resolve by
/// different rules.
fn include_specifier(directive: &str) -> Option<&str> {
    let rest = directive.strip_prefix('#')?.trim_start();
    let rest = rest
        .strip_prefix("include_next")
        .or_else(|| rest.strip_prefix("include"))?;
    // `#includefoo` is not an include; the keyword must end.
    if rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let rest = rest.trim_start();
    let close = match rest.chars().next()? {
        '"' => '"',
        '<' => '>',
        _ => return None,
    };
    let end = rest[1..].find(close)? + 2;
    Some(&rest[..end])
}

/// Add the words of one directive: its identifiers, without the directive's
/// own keyword (`define`, `if`, …) and without those inside a comment or a
/// string or character literal.
fn words_in_directive(directive: &str, words: &mut BTreeSet<String>) {
    let b = directive.as_bytes();
    // Past the `#`, the blanks and the keyword.
    let mut i = 0;
    while i < b.len() && (b[i] == b'#' || b[i].is_ascii_whitespace()) {
        i += 1;
    }
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
        i += 1;
    }
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => break,
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                    i += 1;
                }
                i += 2;
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            // A character literal is short (`'Z'`, `'\\x41'`). An apostrophe
            // with no closing quote near it (`don't`) is only an apostrophe.
            b'\'' => {
                let close = b[i + 1..].iter().take(6).position(|&c| c == b'\'');
                i += close.map_or(1, |n| n + 2);
            }
            c if c.is_ascii_digit() => {
                // A number, suffixes and digit separators and all: `0x1F`,
                // `1e5`, `10UL`, `1'000`.
                while i < b.len()
                    && (b[i].is_ascii_alphanumeric()
                        || b[i] == b'_'
                        || (b[i] == b'\'' && b.get(i + 1).is_some_and(u8::is_ascii_alphanumeric)))
                {
                    i += 1;
                }
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                words.insert(directive[start..i].to_string());
            }
            _ => i += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specs(src: &str) -> Vec<String> {
        extract(src)
            .imports
            .into_iter()
            .map(|i| i.specifier)
            .collect()
    }

    fn has(src: &str, word: &str) -> bool {
        extract(src).identifiers.iter().any(|w| w == word)
    }

    #[test]
    fn both_forms_are_read_with_their_delimiters() {
        let src = "#include \"db/db.h\"\n#include <vector>\n";
        assert_eq!(specs(src), ["\"db/db.h\"", "<vector>"]);
        for import in extract(src).imports {
            assert_eq!(import.imported_name, "*");
        }
    }

    #[test]
    fn spacing_around_the_hash_and_the_keyword_does_not_matter() {
        let src = "  #  include   \"a.h\"\n#\tinclude<b.h>\n#include_next <c.h>\n";
        assert_eq!(specs(src), ["\"a.h\"", "<b.h>", "<c.h>"]);
    }

    #[test]
    fn an_include_inside_a_conditional_is_read_whatever_the_branch() {
        let src = "#if defined(_WIN32)\n#include <windows.h>\n#else\n#include <unistd.h>\n#endif\n#if 0\n#include \"dead.h\"\n#endif\n";
        assert_eq!(specs(src), ["<windows.h>", "<unistd.h>", "\"dead.h\""]);
    }

    #[test]
    fn a_computed_include_and_other_directives_are_not_includes() {
        let src = "#include MACRO_NAME\n#include FOO(bar)\n#includes \"x.h\"\n#define INC \"y.h\"\n#if __has_include(<z.h>)\n#endif\n#pragma once\n";
        assert!(specs(src).is_empty());
    }

    #[test]
    fn an_include_in_a_comment_or_a_string_is_not_an_include() {
        let src =
            "// #include \"a.h\"\n/* #include \"b.h\"\n*/\nconst char* s = \"#include <c.h>\";\n";
        assert!(specs(src).is_empty());
    }

    #[test]
    fn a_repeated_include_is_listed_once() {
        let src = "#include \"a.h\"\n#if X\n#include \"a.h\"\n#endif\n#include <a.h>\n";
        assert_eq!(specs(src), ["\"a.h\"", "<a.h>"]);
    }

    #[test]
    fn a_trailing_comment_does_not_hide_the_path() {
        assert_eq!(specs("#include \"a.h\" // for A\n"), ["\"a.h\""]);
        assert_eq!(specs("#include <a.h> /* for A */\n"), ["<a.h>"]);
    }

    #[test]
    fn a_continued_directive_still_reads() {
        assert_eq!(specs("#include \\\n  \"a.h\"\n"), ["\"a.h\""]);
    }

    #[test]
    fn words_come_from_code_not_comments_or_strings() {
        let src = "// Alpha\n/* Beta */\nint Gamma = 1; const char* s = \"Delta\"; char c = 'E';\n";
        assert!(has(src, "Gamma"));
        for absent in ["Alpha", "Beta", "Delta", "E"] {
            assert!(!has(src, absent), "{absent}");
        }
    }

    #[test]
    fn words_in_preprocessor_lines_are_read() {
        let src = "#define FAST(x) Impl(x)\n#if defined(HAVE_SNAPPY) && Kind == 3\n#endif\n#ifdef Other // Note\n#endif\n";
        for word in ["FAST", "Impl", "HAVE_SNAPPY", "Kind", "Other"] {
            assert!(has(src, word), "{word}");
        }
        assert!(!has(src, "Note"));
        // A number and its suffix are not words.
        assert!(!has("#if V >= 0x1FUL\n#endif\n", "x1FUL"));
        assert!(!has("#if V >= 0x1FUL\n#endif\n", "UL"));
    }

    #[test]
    fn a_string_in_a_directive_hides_its_words() {
        let src = "#error \"Hidden words\"\n#define MSG \"Also hidden\"\n";
        assert!(has(src, "MSG"));
        assert!(!has(src, "Hidden"));
        assert!(!has(src, "Also"));
    }

    #[test]
    fn words_are_distinct_and_sorted() {
        let src = "int b; int a; int b;\n";
        assert_eq!(extract(src).identifiers, ["a", "b", "int"]);
    }

    #[test]
    fn the_directive_keyword_and_an_include_path_are_not_words() {
        let src = "#include <vector>\n#include \"db/db.h\"\n#include <Foo/Foo.h> // note\n#define Name 1\n#if defined(X)\n#endif\n";
        let fi = extract(src);
        for absent in [
            "include", "define", "if", "endif", "vector", "db", "h", "Foo", "note",
        ] {
            assert!(!fi.identifiers.iter().any(|w| w == absent), "{absent}");
        }
        // What a directive uses is still a word.
        for word in ["Name", "defined", "X"] {
            assert!(fi.identifiers.iter().any(|w| w == word), "{word}");
        }
    }

    #[test]
    fn a_digit_separator_or_an_apostrophe_does_not_hide_the_rest() {
        assert!(has("#if A > 1'000'000 && Tail\n#endif\n", "Tail"));
        assert!(has("#error don't stop Here\n", "Here"));
        // A real character literal is still skipped.
        assert!(!has("#if C == 'Z' && Rest\n#endif\n", "Z"));
        assert!(has("#if C == 'Z' && Rest\n#endif\n", "Rest"));
    }

    #[test]
    fn a_block_comment_that_runs_on_from_a_directive_hides_its_words() {
        let src = "#define X 1 /* start\n Hidden words */\nint after;\n";
        let fi = extract(src);
        assert!(!has(src, "Hidden") && !has(src, "words"));
        assert!(fi.identifiers.iter().any(|w| w == "after"));
        // A comment marker inside a string is not a comment.
        let s2 = "#define S \"/*\"\nint Visible;\n";
        assert!(has(s2, "Visible"));
        // Two strings on one line: the second is not mistaken for a gap.
        let s3 = "#define T \"a\" \"/*\"\nint Seen;\n";
        assert!(has(s3, "Seen"));
    }

    #[test]
    fn a_header_with_no_includes_has_none() {
        let fi = extract("class A {};\n");
        assert!(fi.imports.is_empty());
        assert!(fi.re_exports.is_empty() && fi.default_imports.is_empty());
        assert!(fi.qualified_refs.is_empty());
    }
}
