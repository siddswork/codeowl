//! The ids the C++ pack gives one file, for `utility/cpp_accuracy/reduce/`.
//!
//! `cpp_ids <file>` prints `ERRORS<TAB>n` (the parse errors left after
//! blanking), then one `line<TAB>id` per top-level symbol, the id without the
//! file name. Macros are left out: a macro is global whatever namespace it is
//! written in, so it says nothing about a lost namespace.
fn main() {
    let path = std::env::args().nth(1).expect("usage: cpp_ids <file>");
    let src = std::fs::read_to_string(&path).expect("cannot read the file");
    let (syms, errors) = codeowl::cpp::extract_unfolded_counted(&src, "f.h");
    println!("ERRORS\t{errors}");
    for s in syms
        .iter()
        .filter(|s| s.parent.is_none() && s.raw != "macro")
    {
        let name = s.id.strip_prefix("f.h::").unwrap_or(&s.id);
        println!("{}\t{}", s.lines[0], name);
    }
}
