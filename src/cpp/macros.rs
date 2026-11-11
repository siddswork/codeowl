//! Macro blanking: make C++ source parseable by tree-sitter without running
//! the preprocessor.
//!
//! tree-sitter reads the text as written, so a macro standing where the
//! grammar expects nothing breaks the parse: `class LEVELDB_EXPORT DB { … }`
//! reads as a *function* named `DB` (`experiments/exp-08-cpp-tree-sitter-or-
//! compiler.md`). [`blank_macros`] returns a copy of the source in which such
//! macros are replaced by spaces **of the same length**, so every byte offset
//! and line number is unchanged: the tree is built from the blanked copy and
//! every span, signature and hash is still read from the original text.
//!
//! The rules are positional. They know where a macro sits, never what it
//! expands to (that is the macro table, a later step; `DECISIONS.md` question
//! 28). A "macro-looking" name is all capitals with an underscore
//! (`GUARDED_BY`, `FMT_API`); the one rule whose position is unambiguous
//! (between `class` and the class name) accepts any all-capitals name.
//!
//! The same pass is a small syntax shim for constructs newer than the
//! grammar: C++20 `export` / `module` / `import`, the C++23 explicit object
//! parameter, and C++26 `= delete("reason")` and contract clauses.

/// One lexical token. `text` borrows from the source; `line` / `end_line`
/// are 0-based rows of its first and last byte.
#[derive(Debug, Clone, Copy)]
struct Tok<'a> {
    kind: Kind,
    text: &'a str,
    start: usize,
    end: usize,
    line: usize,
    end_line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// An identifier or keyword.
    Ident,
    /// Punctuation, one token per operator (`::`, `->`, `[[`, `&&` are kept
    /// whole; everything else is a single character).
    Punct,
    /// A string or character literal.
    Str,
    Number,
    /// A whole preprocessor directive, continuation lines included. It acts
    /// as a statement boundary for the rules, and is blanked in one case
    /// only: a conditional that splits a declaration (`split_conditional`).
    Preproc,
}

/// A copy of `source` with grammar-breaking macros replaced by spaces. The
/// result has the same length in bytes and the same line breaks.
pub fn blank_macros(source: &str) -> String {
    let mut bytes = source.as_bytes().to_vec();
    let mut toks = lex(source);
    // A blanked macro can expose another (two annotations in a row), so the
    // rules run again on what is left. Four passes is far more than any real
    // declaration needs; the bound only guarantees termination.
    for _ in 0..4 {
        let ranges = find_ranges(&toks);
        if ranges.is_empty() {
            break;
        }
        for &(start, end) in &ranges {
            for b in &mut bytes[start..end] {
                if *b != b'\n' && *b != b'\r' {
                    *b = b' ';
                }
            }
        }
        toks.retain(|t| !ranges.iter().any(|&(s, e)| t.start >= s && t.end <= e));
    }
    // Only whole tokens are overwritten, and each byte of one becomes an
    // ASCII space, so the buffer is still valid UTF-8.
    String::from_utf8(bytes).unwrap_or_else(|_| source.to_string())
}

/// Split `source` into tokens, dropping whitespace and comments.
fn lex<'a>(source: &'a str) -> Vec<Tok<'a>> {
    let b = source.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    let mut line = 0;
    // True until the first token of a line: a `#` here starts a directive.
    let mut at_line_start = true;

    // The closure names `'a` so the pushed slice is tied to `source`, not to
    // a lifetime the closure would otherwise invent for its own argument.
    let push = |toks: &mut Vec<Tok<'a>>, kind, start: usize, end: usize, line: usize| {
        let end_line = line + b[start..end].iter().filter(|&&c| c == b'\n').count();
        toks.push(Tok {
            kind,
            text: &source[start..end],
            start,
            end,
            line,
            end_line,
        });
        end_line
    };

    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            at_line_start = true;
            i += 1;
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let start = i;
            i += 2;
            while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                i += 1;
            }
            i = (i + 2).min(b.len());
            line += b[start..i].iter().filter(|&&c| c == b'\n').count();
        } else if c == b'#' && at_line_start {
            let start = i;
            while i < b.len() && b[i] != b'\n' {
                // A backslash at the end of the line continues the directive.
                if b[i] == b'\\' && b.get(i + 1) == Some(&b'\n') {
                    i += 1;
                } else if b[i] == b'\\'
                    && b.get(i + 1) == Some(&b'\r')
                    && b.get(i + 2) == Some(&b'\n')
                {
                    i += 2;
                }
                i += 1;
            }
            let i_end = i.min(b.len());
            line = push(&mut toks, Kind::Preproc, start, i_end, line);
            i = i_end;
        } else if c == b'"' {
            let start = i;
            i = skip_quoted(b, i, b'"');
            line = push(&mut toks, Kind::Str, start, i, line);
            at_line_start = false;
        } else if c == b'\'' {
            let start = i;
            i = skip_quoted(b, i, b'\'');
            line = push(&mut toks, Kind::Str, start, i, line);
            at_line_start = false;
        } else if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            let start = i;
            i += 1;
            // Digits, letters (suffixes, hex, exponents), dots, and the
            // C++14 digit separator `1'000` when a digit or letter follows.
            while i < b.len()
                && (b[i].is_ascii_alphanumeric()
                    || b[i] == b'.'
                    || b[i] == b'_'
                    || (b[i] == b'\'' && b.get(i + 1).is_some_and(u8::is_ascii_alphanumeric)))
            {
                i += 1;
            }
            line = push(&mut toks, Kind::Number, start, i, line);
            at_line_start = false;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = &source[start..i];
            // `R"delim( … )delim"`: a raw string, whatever its prefix.
            if b.get(i) == Some(&b'"') && matches!(word, "R" | "LR" | "uR" | "UR" | "u8R") {
                i = skip_raw_string(b, i);
                line = push(&mut toks, Kind::Str, start, i, line);
            } else {
                line = push(&mut toks, Kind::Ident, start, i, line);
            }
            at_line_start = false;
        } else if !c.is_ascii() {
            // A non-ASCII character outside a literal or comment: step over
            // the whole code point so a token never splits one.
            i += 1;
            while i < b.len() && (b[i] & 0xC0) == 0x80 {
                i += 1;
            }
            at_line_start = false;
        } else {
            let two = b.get(i + 1).map(|&n| [c, n]);
            let len = match two {
                Some([b':', b':'] | [b'-', b'>'] | [b'[', b'['] | [b']', b']'] | [b'&', b'&']) => 2,
                _ => 1,
            };
            line = push(&mut toks, Kind::Punct, i, i + len, line);
            i += len;
            at_line_start = false;
        }
    }
    toks
}

/// The index just past a `"…"` or `'…'` literal starting at `i`. A literal
/// never spans a line (an unterminated one stops at the line end).
fn skip_quoted(b: &[u8], mut i: usize, quote: u8) -> usize {
    i += 1;
    while i < b.len() && b[i] != quote && b[i] != b'\n' {
        if b[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    (i + 1).min(b.len())
}

/// The index just past a raw string whose opening `"` is at `i`.
fn skip_raw_string(b: &[u8], i: usize) -> usize {
    let delim_start = i + 1;
    let mut j = delim_start;
    while j < b.len() && b[j] != b'(' && b[j] != b'\n' {
        j += 1;
    }
    if j >= b.len() || b[j] != b'(' {
        return skip_quoted(b, i, b'"');
    }
    let delim = &b[delim_start..j];
    let mut k = j + 1;
    while k < b.len() {
        if b[k] == b')'
            && b[k + 1..].starts_with(delim)
            && b.get(k + 1 + delim.len()) == Some(&b'"')
        {
            return k + 2 + delim.len();
        }
        k += 1;
    }
    b.len()
}

/// All capitals, digits and underscores, starting with a capital, at least
/// three characters. Enough for the one position that is unambiguous.
fn is_caps(t: &Tok) -> bool {
    t.kind == Kind::Ident
        && t.text.len() >= 3
        && t.text.starts_with(|c: char| c.is_ascii_uppercase())
        && t.text
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}

/// [`is_caps`] with at least one underscore: how a macro is told from an
/// ordinary all-capitals name (`FILE`, `DB`, `HANDLE`) where position alone
/// does not settle it.
fn is_macro_name(t: &Tok) -> bool {
    is_caps(t) && t.text.contains('_')
}

fn is_punct(t: Option<&Tok>, text: &str) -> bool {
    t.is_some_and(|t| t.kind == Kind::Punct && t.text == text)
}

fn is_word(t: Option<&Tok>, text: &str) -> bool {
    t.is_some_and(|t| t.kind == Kind::Ident && t.text == text)
}

/// Keywords that can only begin, or continue, a declaration. A macro-looking
/// name directly in front of one of these is not a type.
const DECL_KEYWORDS: &[&str] = &[
    "static",
    "inline",
    "constexpr",
    "consteval",
    "constinit",
    "virtual",
    "explicit",
    "friend",
    "extern",
    "typedef",
    "using",
    "template",
    "class",
    "struct",
    "union",
    "enum",
    "namespace",
    "void",
    "bool",
    "char",
    "int",
    "long",
    "short",
    "unsigned",
    "signed",
    "float",
    "double",
    "auto",
    "const",
    "volatile",
    "typename",
    "mutable",
    "thread_local",
    "operator",
    "decltype",
    "wchar_t",
    "char8_t",
    "char16_t",
    "char32_t",
];

/// Words that are never a declared name, so `word MACRO(args);` is a
/// statement and not an annotated member.
const NON_NAME_KEYWORDS: &[&str] = &[
    "return",
    "throw",
    "case",
    "else",
    "do",
    "new",
    "delete",
    "goto",
    "co_return",
    "co_yield",
    "co_await",
    "sizeof",
    "typedef",
    "using",
    "const",
    "static",
    "inline",
    "constexpr",
    "void",
    "bool",
    "char",
    "int",
    "long",
    "short",
    "unsigned",
    "signed",
    "float",
    "double",
    "auto",
    "volatile",
    "extern",
    "mutable",
    "virtual",
    "explicit",
    "typename",
    "operator",
];

/// What may follow a declarator's `)` before a trailing annotation.
const AFTER_PARAMS: &[&str] = &["const", "noexcept", "override", "final", "volatile"];

/// If `toks[i]` opens a parenthesised group, the index of its closing `)`.
fn matching_paren(toks: &[Tok], i: usize) -> Option<usize> {
    if !is_punct(toks.get(i), "(") {
        return None;
    }
    let mut depth = 0usize;
    for (k, t) in toks.iter().enumerate().skip(i) {
        if t.kind == Kind::Punct && t.text == "(" {
            depth += 1;
        } else if t.kind == Kind::Punct && t.text == ")" {
            depth -= 1;
            if depth == 0 {
                return Some(k);
            }
        } else if t.kind == Kind::Preproc {
            return None;
        }
    }
    None
}

/// The index of the last token of the macro use starting at `i`: the name
/// itself, or the `)` of its argument list.
fn macro_end(toks: &[Tok], i: usize) -> usize {
    matching_paren(toks, i + 1).unwrap_or(i)
}

/// Byte ranges to blank, found in one pass over `toks`.
fn find_ranges(toks: &[Tok]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        if t.kind == Kind::Preproc {
            split_conditional(toks, i, &mut out);
            i += 1;
            continue;
        }
        if t.kind != Kind::Ident {
            i += 1;
            continue;
        }
        let prev = i.checked_sub(1).map(|p| &toks[p]);
        // Two rules blank a neighbour of the token along with it, so they
        // report a byte range of their own.
        if let Some((range, last)) =
            template_parameter_macro(toks, i, prev).or_else(|| unnamed_enum_base(toks, i))
        {
            out.push(range);
            i = last + 1;
            continue;
        }
        // C++26 `= delete("reason")`: the one rule that blanks what follows
        // the token and keeps the token itself.
        // (A let-chain: the `if let` and the plain conditions in one `if`,
        // stable since the 2024 edition.)
        if t.text == "delete"
            && is_punct(prev, "=")
            && let Some(close) = matching_paren(toks, i + 1)
        {
            out.push((toks[i + 1].start, toks[close].end));
            i = close + 1;
            continue;
        }
        // Order matters once: `if MACRO (cond)` must be tried before the
        // rules that would read `(cond)` as the macro's own arguments.
        let found = shim(toks, i, prev)
            .or_else(|| macro_after_if(toks, i, prev))
            .or_else(|| class_macro(toks, i, prev))
            .or_else(|| annotation_after_params(toks, i, prev))
            .or_else(|| annotation_after_member(toks, i, prev))
            .or_else(|| macro_alone_on_line(toks, i, prev))
            .or_else(|| macro_before_declaration(toks, i, prev));
        match found {
            Some(last) => {
                out.push((t.start, toks[last].end));
                i = last + 1;
            }
            None => i += 1,
        }
    }
    out
}

/// The word after a directive's `#`: `if`, `ifdef`, `else`, `endif`, …
///
/// The result borrows from the source text (`'a`), not from the token: with
/// two references in the signature Rust cannot guess which one the returned
/// `&str` follows, so it has to be said.
fn directive<'a>(t: &Tok<'a>) -> &'a str {
    let rest = t.text.trim_start_matches('#').trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(rest.len());
    &rest[..end]
}

/// A conditional that starts in the middle of a declaration:
///
/// ```text
/// Limiter(int n)
///     :
/// #if !defined(NDEBUG)
///       max_(n),
/// #endif
///       allowed_(n) {}
/// ```
///
/// The grammar reads directives only between declarations, so this one
/// would break the constructor around it. Its `#if` and `#endif` lines are
/// blanked, and so is everything from an `#else` on: the declaration is read
/// with its first branch. (A conditional *between* declarations is left
/// alone, and both of its branches are extracted.)
///
/// The same is done for a conditional whose first branch opens a brace or a
/// parenthesis it does not close (or closes one it did not open): the
/// matching half is under another `#if` further down, and the grammar cannot
/// nest that. `#ifdef __cplusplus` / `extern "C" {` / `#endif` is the
/// everyday case.
fn split_conditional(toks: &[Tok], i: usize, out: &mut Vec<(usize, usize)>) {
    if !matches!(directive(&toks[i]), "if" | "ifdef" | "ifndef") {
        return;
    }
    let prev = i.checked_sub(1).map(|p| &toks[p]);
    let before = i.checked_sub(2).map(|p| &toks[p]);
    let mid_declaration = prev.is_some_and(|prev| match prev.kind {
        // After `,` `(` `=` or an operator nothing has ended yet. A `:` is a
        // constructor's initialiser list only when it follows a `)`;
        // `public:` ends a label.
        Kind::Punct => {
            // Inside braces a comma separates entries of an enum or an
            // initialiser, which the grammar reads whole with every branch.
            (prev.text == "," && enclosing_opener(toks, i) != Some("{"))
                || matches!(prev.text, "(" | "=" | "&&" | "|" | "+")
                || (prev.text == ":" && is_punct(before, ")"))
        }
        Kind::Ident => matches!(prev.text, "class" | "struct" | "union"),
        _ => false,
    });
    let mut depth = 0usize;
    let mut first_else = None;
    let mut end = None;
    for (k, t) in toks.iter().enumerate().skip(i + 1) {
        if t.kind != Kind::Preproc {
            continue;
        }
        match directive(t) {
            "if" | "ifdef" | "ifndef" => depth += 1,
            "endif" if depth == 0 => {
                end = Some(k);
                break;
            }
            "endif" => depth -= 1,
            "else" | "elif" | "elifdef" | "elifndef" if depth == 0 && first_else.is_none() => {
                first_else = Some(k);
            }
            _ => {}
        }
    }
    let Some(end) = end else {
        return;
    };
    let first_branch_end = first_else.unwrap_or(end);
    // Net braces and parentheses opened by the first branch.
    let mut net = [0i32; 2];
    // A conditional nested in the first branch is counted by its first
    // branch only: its `#else` side would otherwise be added on top.
    let mut nested = 0usize;
    let mut skipping = false;
    for t in &toks[i + 1..first_branch_end] {
        if t.kind == Kind::Preproc {
            match directive(t) {
                "if" | "ifdef" | "ifndef" => nested += 1,
                "else" | "elif" | "elifdef" | "elifndef" if nested > 0 => skipping = true,
                "endif" if nested > 0 => {
                    nested -= 1;
                    if nested == 0 {
                        skipping = false;
                    }
                }
                _ => {}
            }
            continue;
        }
        if skipping {
            continue;
        }
        if t.kind == Kind::Punct {
            match t.text {
                "{" => net[0] += 1,
                "}" => net[0] -= 1,
                "(" => net[1] += 1,
                ")" => net[1] -= 1,
                _ => {}
            }
        }
    }
    if !(mid_declaration || net != [0, 0]) {
        return;
    }
    out.push((toks[i].start, toks[i].end));
    out.push((toks[first_branch_end].start, toks[end].end));
}

/// The nearest bracket opened before `toks[i]` and not yet closed.
fn enclosing_opener(toks: &[Tok], i: usize) -> Option<&'static str> {
    let mut depth = [0usize; 3];
    for t in toks[..i].iter().rev() {
        if t.kind != Kind::Punct {
            continue;
        }
        let (idx, opens) = match t.text {
            ")" => (0, false),
            "(" => (0, true),
            "]" => (1, false),
            "[" => (1, true),
            "}" => (2, false),
            "{" => (2, true),
            _ => continue,
        };
        if !opens {
            depth[idx] += 1;
        } else if depth[idx] > 0 {
            depth[idx] -= 1;
        } else {
            return Some(["(", "[", "{"][idx]);
        }
    }
    None
}

/// `template <typename T, MACRO(cond)>`: a macro standing for a template
/// parameter (`FMT_ENABLE_IF`). It goes together with the comma that
/// separates it, so no empty parameter is left behind.
fn template_parameter_macro(
    toks: &[Tok],
    i: usize,
    prev: Option<&Tok>,
) -> Option<((usize, usize), usize)> {
    if !is_macro_name(&toks[i]) {
        return None;
    }
    let close = matching_paren(toks, i + 1)?;
    let next = toks.get(close + 1)?;
    let after_comma = is_punct(prev, ",");
    let before_comma = is_punct(Some(next), ",");
    if !(after_comma || is_punct(prev, "<")) || !(before_comma || is_punct(Some(next), ">")) {
        return None;
    }
    // Walk back to the `<` that opens this list; it must follow `template`.
    let mut depth = 0usize;
    let mut k = i;
    let limit = i.saturating_sub(300);
    let in_template_list = loop {
        let Some(p) = k.checked_sub(1) else {
            break false;
        };
        if p < limit {
            break false;
        }
        k = p;
        let t = &toks[k];
        if t.kind == Kind::Preproc {
            break false;
        }
        if t.kind != Kind::Punct {
            continue;
        }
        match t.text {
            ">" => depth += 1,
            "<" if depth == 0 => {
                break k
                    .checked_sub(1)
                    .is_some_and(|p| is_word(toks.get(p), "template"));
            }
            "<" => depth -= 1,
            ";" | "{" | "}" => break false,
            _ => {}
        }
    };
    if !in_template_list {
        return None;
    }
    let range = if after_comma {
        (prev?.start, toks[close].end)
    } else if before_comma {
        (toks[i].start, next.end)
    } else {
        (toks[i].start, toks[close].end)
    };
    Some((
        range,
        if after_comma {
            close
        } else if before_comma {
            close + 1
        } else {
            close
        },
    ))
}

/// `enum : unsigned { … }`: the grammar does not read an underlying type on
/// an enum with no name. The `: type` goes; the enumerators stay.
fn unnamed_enum_base(toks: &[Tok], i: usize) -> Option<((usize, usize), usize)> {
    if toks[i].text != "enum" || !is_punct(toks.get(i + 1), ":") {
        return None;
    }
    let brace = (i + 2..toks.len().min(i + 10)).find(|&k| is_punct(toks.get(k), "{"))?;
    let all_type_words = toks[i + 2..brace]
        .iter()
        .all(|t| t.kind == Kind::Ident || (t.kind == Kind::Punct && t.text == "::"));
    (brace > i + 2 && all_type_words).then(|| ((toks[i + 1].start, toks[brace - 1].end), brace - 1))
}

/// `class MACRO Name` / `class MACRO(args) Name`: the macro between the
/// keyword and the name. Any all-capitals name counts here. `class FOO final`
/// is left alone: there `FOO` is the class.
fn class_macro(toks: &[Tok], i: usize, prev: Option<&Tok>) -> Option<usize> {
    let prev = prev?;
    if !(prev.kind == Kind::Ident && matches!(prev.text, "class" | "struct" | "union")) {
        return None;
    }
    if !is_caps(&toks[i]) {
        return None;
    }
    let last = macro_end(toks, i);
    let name = toks.get(last + 1)?;
    if name.kind != Kind::Ident || name.text == "final" {
        return None;
    }
    let after = toks.get(last + 2)?;
    let follows = (after.kind == Kind::Ident && after.text == "final")
        || (after.kind == Kind::Punct && matches!(after.text, ":" | "{" | "<" | ";"));
    follows.then_some(last)
}

/// `void f() MACRO(args);`: an annotation after a declarator's parameter
/// list (and any `const` / `noexcept` / `override`).
fn annotation_after_params(toks: &[Tok], i: usize, _prev: Option<&Tok>) -> Option<usize> {
    if !is_macro_name(&toks[i]) {
        return None;
    }
    // Walk back over `const`, `noexcept`, … to the `)`.
    let mut k = i;
    loop {
        k = k.checked_sub(1)?;
        let t = &toks[k];
        if t.kind == Kind::Ident && AFTER_PARAMS.contains(&t.text) {
            continue;
        }
        if t.kind == Kind::Punct && matches!(t.text, "&" | "&&") {
            continue;
        }
        if t.kind == Kind::Punct && t.text == ")" {
            break;
        }
        return None;
    }
    let last = macro_end(toks, i);
    let next = toks.get(last + 1)?;
    let follows = match next.kind {
        Kind::Punct => matches!(next.text, ";" | "{" | "=" | "->" | ":" | "[["),
        Kind::Ident => {
            matches!(
                next.text,
                "override" | "final" | "noexcept" | "const" | "try"
            ) || is_macro_name(next)
        }
        _ => false,
    };
    follows.then_some(last)
}

/// `int x_ MACRO(args);`: an annotation after a data member's name. The
/// name must itself follow a type, which is what separates this from a
/// variable built with constructor arguments (`std::string DEFAULT_DIR("/")`).
fn annotation_after_member(toks: &[Tok], i: usize, prev: Option<&Tok>) -> Option<usize> {
    if !is_macro_name(&toks[i]) {
        return None;
    }
    let last = matching_paren(toks, i + 1)?;
    let name = prev?;
    let is_name = (name.kind == Kind::Ident && !NON_NAME_KEYWORDS.contains(&name.text))
        || (name.kind == Kind::Punct && name.text == "]");
    if !is_name {
        return None;
    }
    let before_name = toks.get(i.checked_sub(2)?)?;
    let after_type = match before_name.kind {
        Kind::Ident => {
            !NON_NAME_KEYWORDS.contains(&before_name.text) || {
                matches!(
                    before_name.text,
                    "bool"
                        | "char"
                        | "int"
                        | "long"
                        | "short"
                        | "unsigned"
                        | "signed"
                        | "float"
                        | "double"
                        | "auto"
                )
            }
        }
        Kind::Punct => matches!(before_name.text, ">" | "*" | "&" | "&&" | "]"),
        _ => false,
    };
    if !after_type {
        return None;
    }
    let next = toks.get(last + 1)?;
    (next.kind == Kind::Punct && matches!(next.text, ";" | "=" | "{" | ",")).then_some(last)
}

/// A line that is nothing but a macro use with no semicolon
/// (`FMT_BEGIN_NAMESPACE`, `Q_OBJECT`, `ABSL_NAMESPACE_END`).
fn macro_alone_on_line(toks: &[Tok], i: usize, prev: Option<&Tok>) -> Option<usize> {
    let last = macro_end(toks, i);
    // With arguments and no semicolon the position is clear enough that the
    // name need not have an underscore (`COMPOP(>, <, AtLeast)`).
    if !(is_macro_name(&toks[i]) || (is_caps(&toks[i]) && last != i)) {
        return None;
    }
    let next = toks.get(last + 1);
    let alone = prev.is_none_or(|p| p.end_line < toks[i].line)
        && next.is_none_or(|n| n.line > toks[last].end_line);
    if !alone {
        return None;
    }
    // It must stand where a declaration could: after `;`, `}`, `{`, `:` or a
    // directive. After a `,` it is the last entry of a list.
    let starts_statement = prev.is_none_or(|p| {
        p.kind == Kind::Preproc
            || (p.kind == Kind::Punct && matches!(p.text, ";" | "}" | "{" | ":"))
    });
    // What follows must begin something new. A `{` would make this the head
    // of a block (`TEST_F(A, B)` with the brace on the next line).
    let ends_statement = next.is_none_or(|n| match n.kind {
        Kind::Preproc | Kind::Ident => true,
        Kind::Punct => matches!(n.text, "}" | "[[" | "~"),
        _ => false,
    });
    // `{ ONLY_ONE }`: the single entry of an enum or an initialiser.
    let sole_entry = is_punct(prev, "{") && is_punct(next, "}");
    (starts_statement && ends_statement && !sole_entry).then_some(last)
}

/// `MACRO Status f();` / `MACRO(args) void f();`: a macro in front of a
/// declaration. Two names in a row normally mean "type, then declared name",
/// so a bare macro-looking name is blanked only when what follows it is
/// clearly still a type and not the declared name.
fn macro_before_declaration(toks: &[Tok], i: usize, prev: Option<&Tok>) -> Option<usize> {
    if !is_macro_name(&toks[i]) {
        return None;
    }
    // A member access or a qualified name is an ordinary use.
    if prev.is_some_and(|p| p.kind == Kind::Punct && matches!(p.text, "::" | "." | "->")) {
        return None;
    }
    let last = macro_end(toks, i);
    let has_args = last != i;
    let t1 = toks.get(last + 1)?;
    if is_punct(Some(t1), "[[") || is_punct(Some(t1), "~") {
        return Some(last);
    }
    if t1.kind != Kind::Ident || matches!(t1.text, "final" | "override") {
        return None;
    }
    if has_args || DECL_KEYWORDS.contains(&t1.text) {
        return Some(last);
    }
    // `MACRO Name(params) : member(x) {}` and `explicit MACRO Name(params)`:
    // a constructor, so the macro is not its return type. A plain
    // `MY_TYPE make(int)` has neither mark and is kept.
    if let Some(close) = matching_paren(toks, last + 2) {
        let is_constructor = is_word(prev, "explicit") || is_punct(toks.get(close + 1), ":");
        return is_constructor.then_some(last);
    }
    // `MACRO Type name…`: what follows `t1` decides. `MY_TYPE value;`,
    // `MY_TYPE f()`, `MY_TYPE a, b` keep the name as a type.
    let t2 = toks.get(last + 2)?;
    let still_a_type = match t2.kind {
        Kind::Ident => !matches!(t2.text, "final" | "override"),
        Kind::Punct => matches!(t2.text, "::" | "<" | "*" | "&" | "&&"),
        _ => false,
    };
    still_a_type.then_some(last)
}

/// `if MACRO (cond)`: `if FMT_CONSTEXPR20 (…)`.
fn macro_after_if(toks: &[Tok], i: usize, prev: Option<&Tok>) -> Option<usize> {
    (is_word(prev, "if") && is_macro_name(&toks[i]) && is_punct(toks.get(i + 1), "(")).then_some(i)
}

/// Syntax newer than the grammar. Each arm blanks the smallest piece that
/// lets the rest of the declaration parse.
fn shim(toks: &[Tok], i: usize, prev: Option<&Tok>) -> Option<usize> {
    let t = &toks[i];
    let next = toks.get(i + 1);
    let statement_start = prev.is_none_or(|p| {
        p.kind == Kind::Preproc || (p.kind == Kind::Punct && matches!(p.text, ";" | "}" | "{"))
    });
    // The `;` that ends the statement starting at `i`, on the same line.
    let semicolon = || {
        toks.iter()
            .enumerate()
            .skip(i)
            .take_while(|(_, x)| x.line == t.line)
            .find(|(_, x)| x.kind == Kind::Punct && x.text == ";")
            .map(|(k, _)| k)
    };
    match t.text {
        // C++20: `export module m;` and `export import m;` go whole; in
        // front of a declaration only the keyword goes.
        "export" => {
            if is_word(next, "module") || is_word(next, "import") {
                semicolon()
            } else {
                Some(i)
            }
        }
        // C++20: `module;`, `module m:part;`, `import m;`, `import <v>;`.
        // Both are ordinary identifiers elsewhere, hence the checks.
        "module" | "import" if statement_start => {
            let n = next?;
            let is_module_line = match n.kind {
                Kind::Ident | Kind::Str => true,
                Kind::Punct => matches!(n.text, ";" | ":" | "<"),
                _ => false,
            };
            if is_module_line { semicolon() } else { None }
        }
        // C++23: the explicit object parameter, `void f(this Self self)`.
        "this"
            if prev.is_some_and(|p| p.kind == Kind::Punct && matches!(p.text, "(" | ","))
                && next.is_some_and(|n| n.kind == Kind::Ident) =>
        {
            Some(i)
        }
        _ => None,
    }
    .or_else(|| contract_clause(toks, i))
}

/// C++26 contracts: `pre(cond)` / `post(r: cond)` after a declarator.
fn contract_clause(toks: &[Tok], i: usize) -> Option<usize> {
    if !matches!(toks[i].text, "pre" | "post") {
        return None;
    }
    let last = matching_paren(toks, i + 1)?;
    let mut k = i;
    loop {
        k = k.checked_sub(1)?;
        let t = &toks[k];
        if t.kind == Kind::Ident && AFTER_PARAMS.contains(&t.text) {
            continue;
        }
        return (t.kind == Kind::Punct && t.text == ")").then_some(last);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Blank, check the invariants every result must keep, and return it.
    fn blank(src: &str) -> String {
        let out = blank_macros(src);
        assert_eq!(out.len(), src.len(), "length must not change");
        let rows = |s: &str| s.bytes().filter(|&b| b == b'\n').count();
        assert_eq!(rows(&out), rows(src), "line breaks must not change");
        out
    }

    /// `src` with the first occurrence of each given piece replaced by
    /// spaces (line breaks inside a piece are kept).
    fn spaced(src: &str, macros: &[&str]) -> String {
        let mut out = src.to_string();
        for m in macros {
            assert!(out.contains(m), "test bug: `{m}` not in source");
            let blank: String = m.chars().map(|c| if c == '\n' { c } else { ' ' }).collect();
            out = out.replacen(m, &blank, 1);
        }
        out
    }

    fn unchanged(src: &str) {
        assert_eq!(blank(src), src);
    }

    #[test]
    fn export_macro_between_class_and_name_is_blanked() {
        let src = "class LEVELDB_EXPORT DB {\n public:\n  virtual ~DB();\n};\n";
        assert_eq!(blank(src), spaced(src, &["LEVELDB_EXPORT"]));
    }

    #[test]
    fn class_macro_without_underscore_and_with_arguments() {
        let src = "struct EXPORT Slice : Base {};\nclass ATTR(\"x\") Mutex final {};\n";
        assert_eq!(blank(src), spaced(src, &["EXPORT", "ATTR(\"x\")"]));
    }

    #[test]
    fn exported_forward_declaration_is_blanked() {
        let src = "class LEVELDB_EXPORT Env;\n";
        assert_eq!(blank(src), spaced(src, &["LEVELDB_EXPORT"]));
    }

    #[test]
    fn an_all_capitals_class_name_is_kept() {
        unchanged("class FOO final {};\nclass DB : public Base {};\nstruct FILE_INFO {};\n");
    }

    #[test]
    fn annotation_after_a_parameter_list() {
        let src = "void Lock() EXCLUSIVE_LOCK_FUNCTION(mu_);\n\
                   void Run() const\n    EXCLUSIVE_LOCKS_REQUIRED(mutex_) {\n}\n";
        assert_eq!(
            blank(src),
            spaced(
                src,
                &[
                    "EXCLUSIVE_LOCK_FUNCTION(mu_)",
                    "EXCLUSIVE_LOCKS_REQUIRED(mutex_)"
                ]
            )
        );
    }

    #[test]
    fn two_annotations_in_a_row() {
        let src = "bool Try() ABSL_ATTRIBUTE_COLD ABSL_LOCKS_EXCLUDED(mu_);\n";
        assert_eq!(
            blank(src),
            spaced(src, &["ABSL_ATTRIBUTE_COLD", "ABSL_LOCKS_EXCLUDED(mu_)"])
        );
    }

    #[test]
    fn annotation_after_a_member_name() {
        let src = "port::Mutex mu_;\nint count_ GUARDED_BY(mu_);\nchar* buf_ PT_GUARDED_BY(mu_) = nullptr;\n";
        assert_eq!(
            blank(src),
            spaced(src, &["GUARDED_BY(mu_)", "PT_GUARDED_BY(mu_)"])
        );
    }

    #[test]
    fn a_variable_built_with_arguments_is_kept() {
        unchanged(
            "static const std::string DEFAULT_DIR(\"/tmp\");\n\
             Mutex GLOBAL_LOCK(kInit);\n\
             void f() { return CHECK_OK(x); }\n",
        );
    }

    #[test]
    fn macro_in_front_of_a_declaration() {
        let src = "LEVELDB_EXPORT Status DestroyDB(const std::string& name);\n\
                   LEVELDB_EXPORT leveldb_t* leveldb_open(const char* name);\n\
                   FMT_CONSTEXPR inline int width();\n\
                   ABSL_MUST_USE_RESULT bool Ok();\n\
                   template <typename T> FMT_API auto arg(T t) -> int;\n\
                   ABSL_DEPRECATED(\"use New\") void Old();\n";
        assert_eq!(
            blank(src),
            spaced(
                src,
                &[
                    "LEVELDB_EXPORT",
                    "LEVELDB_EXPORT",
                    "FMT_CONSTEXPR",
                    "ABSL_MUST_USE_RESULT",
                    "FMT_API",
                    "ABSL_DEPRECATED(\"use New\")"
                ]
            )
        );
    }

    #[test]
    fn an_all_capitals_type_name_is_kept() {
        unchanged(
            "SECURITY_ATTRIBUTES sa;\n\
             MY_TYPE make();\n\
             void f(MY_TYPE a, OTHER_TYPE b);\n\
             MY_TYPE x = 1, y;\n\
             MY_TYPE* p;\n\
             int v = FLAG_ONE | FLAG_TWO;\n\
             auto n = ns::MAX_SIZE;\n",
        );
    }

    #[test]
    fn a_macro_alone_on_its_line() {
        let src = "FMT_BEGIN_NAMESPACE\nnamespace detail {\nint f();\n}\nFMT_END_NAMESPACE\n";
        assert_eq!(
            blank(src),
            spaced(src, &["FMT_BEGIN_NAMESPACE", "FMT_END_NAMESPACE"])
        );
    }

    #[test]
    fn a_namespace_macro_before_a_closing_brace() {
        let src = "namespace absl {\nABSL_NAMESPACE_BEGIN\nint f();\nABSL_NAMESPACE_END\n}  // namespace absl\n";
        assert_eq!(
            blank(src),
            spaced(src, &["ABSL_NAMESPACE_BEGIN", "ABSL_NAMESPACE_END"])
        );
    }

    #[test]
    fn list_entries_alone_on_a_line_are_kept() {
        unchanged(
            "enum Color {\n  COLOR_RED,\n  COLOR_BLUE\n};\n\
             enum One {\n  ONLY_ONE\n};\n\
             int t[] = {\n  SIZE_A,\n  SIZE_B\n};\n",
        );
    }

    #[test]
    fn a_test_macro_with_its_brace_on_the_next_line_is_kept() {
        unchanged("TEST_F(DBTest, Empty)\n{\n  ASSERT_TRUE(true);\n}\n");
    }

    #[test]
    fn macro_between_if_and_its_condition() {
        let src = "int f() { if FMT_CONSTEXPR20 (sizeof(int) != 1) return 1; return 0; }\n";
        assert_eq!(blank(src), spaced(src, &["FMT_CONSTEXPR20"]));
    }

    #[test]
    fn a_constructor_behind_a_macro() {
        let src = "struct V {\n  FMT_CONSTEXPR V(const char* s) : data_(s) {}\n\
                   \x20 constexpr FMT_INLINE V() : data_(nullptr) {}\n\
                   \x20 explicit FMT_API V(int n);\n  MY_TYPE make(int a);\n};\n";
        assert_eq!(
            blank(src),
            spaced(src, &["FMT_CONSTEXPR", "FMT_INLINE", "FMT_API"])
        );
    }

    #[test]
    fn a_macro_standing_for_a_template_parameter() {
        let src = "template <typename T, FMT_ENABLE_IF(is_int<T>::value)>\nvoid f(T);\n\
                   template <FMT_ENABLE_IF(true), typename U> void g(U);\n";
        assert_eq!(
            blank(src),
            spaced(
                src,
                &[", FMT_ENABLE_IF(is_int<T>::value)", "FMT_ENABLE_IF(true),"]
            )
        );
        unchanged("bool r = a < b, MY_CHECK(x) > c;\n");
    }

    #[test]
    fn an_all_capitals_call_alone_on_its_line() {
        let src = "COMPOP(>, <, AtLeast)\nCOMPOP(<=, >=, AtLeast)\nint x;\n";
        assert_eq!(
            blank(src),
            spaced(src, &["COMPOP(>, <, AtLeast)", "COMPOP(<=, >=, AtLeast)"])
        );
        // No arguments and no underscore: it could be anything.
        unchanged("int a;\nFOO\nint b;\n");
    }

    #[test]
    fn underlying_type_of_an_unnamed_enum() {
        let src = "struct S { enum : unsigned { kA = 1 }; enum : std::uint8_t { kB }; };\n";
        assert_eq!(blank(src), spaced(src, &[": unsigned", ": std::uint8_t"]));
        unchanged("enum class M : int { A };\nenum E : char { B };\n");
    }

    #[test]
    fn a_conditional_that_splits_a_declaration() {
        let src = "struct L {\n  L(int n)\n      :\n#if !defined(NDEBUG)\n        max_(n),\n\
                   #endif  // !defined(NDEBUG)\n        allowed_(n) {}\n};\n";
        assert_eq!(
            blank(src),
            spaced(
                src,
                &["#if !defined(NDEBUG)", "#endif  // !defined(NDEBUG)"]
            )
        );

        // With an `#else`, the declaration is read with its first branch.
        let src = "int size =\n#if BIG\n    64;\n#else\n    32;\n#endif\nint next;\n";
        assert_eq!(
            blank(src),
            spaced(src, &["#if BIG", "#else\n    32;\n#endif"])
        );

        let src = "class\n#if defined(WIDE)\n    alignas(16)\n#endif\n    Wide {};\n";
        assert_eq!(blank(src), spaced(src, &["#if defined(WIDE)", "#endif"]));
    }

    #[test]
    fn a_conditional_whose_branch_leaves_a_brace_open() {
        // The `{` opened under the first `#ifdef` is closed under the second.
        let src = "#ifdef __cplusplus\nextern \"C\" {\n#endif\nint c_api(int);\n\
                   #ifdef __cplusplus\n}\n#endif\n";
        assert_eq!(
            blank(src),
            spaced(
                src,
                &[
                    "#ifdef __cplusplus",
                    "#endif",
                    "#ifdef __cplusplus",
                    "#endif"
                ]
            )
        );

        // Two openings, one per branch: read with the first.
        let src = "#if A\nvoid f(int a) {\n#else\nvoid f(long a) {\n#endif\n  g(a);\n}\n";
        assert_eq!(
            blank(src),
            spaced(src, &["#if A", "#else\nvoid f(long a) {\n#endif"])
        );
    }

    #[test]
    fn a_nested_conditional_does_not_count_its_else_side() {
        // The inner `#if` / `#else` pair adds two braces if both sides are
        // counted, which would make the include guard look unbalanced.
        let src = "#ifndef H_\n#define H_\n#if A\nvoid f() {\n#else\nvoid f(int) {\n#endif\n}\nint g();\n#endif\n";
        let out = blank(src);
        assert!(out.starts_with("#ifndef H_\n#define H_\n"), "{out}");
        assert!(out.trim_end().ends_with("#endif"), "{out}");
    }

    #[test]
    fn an_enumerator_list_with_branches_is_kept_whole() {
        unchanged("enum E {\n  kA,\n#if X\n  kB,\n#else\n  kC,\n#endif\n  kD\n};\n");
    }

    #[test]
    fn a_conditional_between_declarations_is_kept() {
        unchanged(
            "class C {\n public:\n#if A\n  int a();\n#else\n  int b();\n#endif\n};\n\
             int x;\n#ifdef B\nint y;\n#endif\n",
        );
    }

    #[test]
    fn preprocessor_lines_are_never_touched() {
        unchanged(
            "#define GUARDED_BY(x) __attribute__((guarded_by(x)))\n\
             #  define FMT_API MY_EXPORT int\n\
             #define LONG_ONE(a) \\\n  FMT_API a \\\n  FMT_API a\n\
             #if defined(MY_FLAG) && MY_OTHER > 1\n#endif\n",
        );
    }

    #[test]
    fn comments_and_strings_are_never_touched() {
        unchanged(
            "// class FOO_EXPORT A {\n\
             /* FMT_BEGIN_NAMESPACE\n   LEVELDB_EXPORT int f(); */\n\
             const char* s = \"class FOO_EXPORT A { FMT_API int x; }\";\n\
             const char* r = R\"x(\nFMT_BEGIN_NAMESPACE\n)x\";\n\
             char c = '\"'; int big = 1'000'000;\n",
        );
    }

    #[test]
    fn a_blanked_range_keeps_non_ascii_text_valid() {
        let src = "ABSL_DEPRECATED(\"utilisé — non\") void Old();\n";
        let out = blank(src);
        assert!(out.trim_start().starts_with("void Old();"), "{out:?}");
    }

    #[test]
    fn modules_export_keyword_and_module_lines() {
        let src = "module;\n#include <string>\nexport module shapes:circle;\nimport :base;\n\
                   import <vector>;\nexport class Circle {};\nexport template <typename T> T id(T t);\n";
        assert_eq!(
            blank(src),
            spaced(
                src,
                &[
                    "module;",
                    "export module shapes:circle;",
                    "import :base;",
                    "import <vector>;",
                    "export",
                    "export"
                ]
            )
        );
    }

    #[test]
    fn module_and_import_as_ordinary_names_are_kept() {
        unchanged("int module = 1;\nvoid f() { module = 2; import(x); module->run(); }\n");
    }

    #[test]
    fn explicit_object_parameter() {
        let src = "struct S { void f(this S self); int g() { return h(this, 1); } };\n";
        assert_eq!(blank(src), spaced(src, &["this"]));
    }

    #[test]
    fn deleted_function_with_a_reason() {
        let src = "struct D { D() = delete(\"use make()\"); };\nvoid g() { delete (p); }\n";
        assert_eq!(blank(src), spaced(src, &["(\"use make()\")"]));
    }

    #[test]
    fn contract_clauses() {
        let src =
            "int f(int x) pre(x > 0) post(r: r > 0) { return x; }\nint pre(int); int y = pre(1);\n";
        assert_eq!(blank(src), spaced(src, &["pre(x > 0)", "post(r: r > 0)"]));
    }
}
