//! Pairing a definition with its declaration across files. Step 2b, part B of
//! `docs/cpp-step2b-plan.md`.
//!
//! A C++ class is declared in a header and implemented in a source file. The
//! extractor reads each file alone, so `int DBImpl::Get() { … }` in
//! `db_impl.cc` is a top-level symbol there while the class is in
//! `db_impl.h`. This pass moves the definition into the symbol it defines:
//! the definition stops being a symbol of its file, and the declaration owns
//! its span (`Symbol::other_file_spans`), so one spec describes the class from
//! both files.

use std::collections::{HashMap, HashSet};

use super::resolve::IncludeGraph;
use crate::graph::FileExtraction;
use crate::hash::hash_text;
use crate::imports::FileImports;
use crate::symbol::SymbolKind;

/// Whether `sym` is a definition this pass may move: a top-level symbol that
/// can be reached from another file, written with a qualified name (a member
/// or a namespace function) or a plain function with a body (whose prototype
/// may be in a header). A `static` helper or anything in an anonymous
/// namespace is a different entity from a header's of the same name.
fn is_pairable(sym: &crate::symbol::ExtractedSymbol) -> bool {
    if sym.parent.is_some() || !sym.is_exported {
        return false;
    }
    let qualified = matches!(
        sym.raw.as_str(),
        "qualified_function" | "qualified_variable"
    );
    let free = sym.raw == "function" && !sym.markers.iter().any(|m| m == "declaration");
    qualified || free
}

/// The ids, after the file, that a definition may name, innermost namespace
/// first.
///
/// A definition's id is the namespaces around it followed by the name as
/// written, and the extractor records how many segments are the namespaces
/// (`chain:N`). C++ looks the first written name up from the innermost
/// enclosing namespace outward, so `absl::Status::X` written inside
/// `namespace absl` (id `absl::absl::Status::X`) is `absl::Status::X`: the
/// ids to try are the written name behind the first `N`, `N - 1`, … down to 0
/// of the enclosing namespaces. A definition with no record (a plain
/// function) can only name its own id.
fn names_to_try(def: &crate::symbol::ExtractedSymbol, suffix: &str) -> Vec<String> {
    let chain = def
        .markers
        .iter()
        .find_map(|m| m.strip_prefix("chain:")?.parse::<usize>().ok());
    let segments: Vec<&str> = suffix.split("::").collect();
    match chain {
        Some(n) if n <= segments.len() => (0..=n)
            .rev()
            .map(|k| {
                segments[..k]
                    .iter()
                    .chain(&segments[n..])
                    .copied()
                    .collect::<Vec<_>>()
                    .join("::")
            })
            .collect(),
        _ => vec![suffix.to_string()],
    }
}

/// What a pass did, for the accuracy probe and the tests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PairingReport {
    /// Definitions moved into the declaration of the member or function they
    /// define.
    pub paired: usize,
    /// Of `paired`, the free functions (a body written plain, its prototype
    /// in a header), as opposed to a definition written with a qualified name.
    pub free_paired: usize,
    /// Definitions moved into their class, which declares no member of that
    /// name (a member a macro declared, say).
    pub to_class: usize,
    /// Qualified definitions with several candidates: left where they are.
    pub ambiguous: usize,
    /// Qualified definitions with no declaration in any file they reach.
    pub unmatched: usize,
}

/// What a definition is moved into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// The declaration of the member or function it defines.
    Member,
    /// The prototype of the plain function it implements.
    Free,
    /// The class, which declares no member of that name.
    Class,
}

/// Where a definition goes: the file and symbol id of its target.
#[derive(Debug)]
struct Move {
    from_file: usize,
    definition: String,
    to_file: usize,
    target: String,
    kind: Kind,
}

/// Pair every out-of-file definition with its declaration. See the module
/// doc.
///
/// **The rule.** C++ requires an out-of-class definition to be written in a
/// namespace that encloses its declaration, with the qualification supplying
/// the rest, so the definition's id after its file (`leveldb::DBImpl::Get`,
/// whether it is written inside `namespace leveldb` or out in full) is
/// exactly its declaration's id after its file. A definition pairs with the
/// one symbol of that name, in a file its own file reaches through includes,
/// of the right kind: a member or a namespace function (with a body already
/// or not, as a member can be defined inline for one configuration) for a
/// qualified definition, a bare prototype for a plain function, a field for
/// a static data member. When the
/// class declares no member of that name (a member a macro declared), the
/// class is the target. Several candidates, or none: the definition stays
/// where it is and is counted.
///
/// A definition in a source file that is not exported (`static`, in an
/// anonymous namespace) is a different entity from a header's of the same
/// name and is never paired.
///
/// **Fresh copies.** `files` is the per-file extraction `rebuild` copies from
/// the index's cache on every graph build, never the cache itself. A
/// definition that was moved is no longer in its file, so a second pass over
/// the same extractions finds nothing and changes nothing; but it must not be
/// given extractions in which a paired definition has been put back, which it
/// would move a second time (the span twice, the hash folded twice).
pub fn pair(
    files: &mut [FileExtraction],
    file_imports: &HashMap<String, FileImports>,
) -> PairingReport {
    let mut report = PairingReport::default();
    let includes = IncludeGraph::build(file_imports);
    let mut order: Vec<usize> = (0..files.len()).collect();
    order.sort_by(|&a, &b| files[a].rel_path.cmp(&files[b].rel_path));
    let by_path: HashMap<&str, usize> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.rel_path.as_str(), i))
        .collect();

    // Declarations and classes by the part of their id after the file.
    let mut declarations: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut classes: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    for (fi, file) in files.iter().enumerate() {
        let prefix = format!("{}::", file.rel_path);
        for (si, sym) in file.symbols.iter().enumerate() {
            let Some(suffix) = sym.id.strip_prefix(&prefix) else {
                continue;
            };
            // A member or a free function of that name, with or without a
            // body already: a class can define a member inline in the header
            // for one configuration and in a source file for another. Not
            // another qualified definition, which belongs to something else.
            let declares = (sym.kind == SymbolKind::Callable && !sym.raw.starts_with("qualified_"))
                || (sym.kind == SymbolKind::Value && sym.raw == "field");
            if declares {
                declarations
                    .entry(suffix.to_string())
                    .or_default()
                    .push((fi, si));
            }
            if sym.kind == SymbolKind::Container
                && matches!(sym.raw.as_str(), "class" | "struct" | "union")
            {
                classes
                    .entry(suffix.to_string())
                    .or_default()
                    .push((fi, si));
            }
        }
    }

    let mut moves: Vec<Move> = Vec::new();
    for &fi in &order {
        let file = &files[fi];
        // The files this one reaches through includes: worked out only for a
        // file that has a definition to pair, which most do not.
        let reach: HashSet<usize> = if file.symbols.iter().any(is_pairable) {
            includes
                .reach(&file.rel_path)
                .iter()
                .filter_map(|p| by_path.get(p.as_str()).copied())
                .collect()
        } else {
            continue;
        };
        let prefix = format!("{}::", file.rel_path);
        for def in file.symbols.iter().filter(|s| is_pairable(s)) {
            let qualified = matches!(
                def.raw.as_str(),
                "qualified_function" | "qualified_variable"
            );
            let Some(suffix) = def.id.strip_prefix(&prefix) else {
                continue;
            };
            let wanted_field = def.raw == "qualified_variable";
            let reachable = |&&(g, _): &&(usize, usize)| reach.contains(&g);
            // The ids this definition may name, innermost namespace first. The
            // first one with a declaration decides, as C++ lookup would.
            let names = names_to_try(def, suffix);
            let candidates: Vec<(usize, usize)> = names
                .iter()
                .map(|name| {
                    declarations
                        .get(name.as_str())
                        .map(|v| {
                            v.iter()
                                .filter(reachable)
                                .filter(|&&(g, s)| {
                                    let cand = &files[g].symbols[s];
                                    if (cand.raw == "field") != wanted_field {
                                        return false;
                                    }
                                    // A plain function with a body is the
                                    // implementation of a bare prototype.
                                    // Another function with a body is no
                                    // prototype (an overload, or a second
                                    // definition).
                                    qualified
                                        || cand.raw == "field"
                                        || cand.markers.iter().any(|m| m == "declaration")
                                })
                                .copied()
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .find(|found| !found.is_empty())
                .unwrap_or_default();
            match candidates.as_slice() {
                [(g, s)] => {
                    moves.push(Move {
                        from_file: fi,
                        definition: def.id.clone(),
                        to_file: *g,
                        target: files[*g].symbols[*s].id.clone(),
                        kind: if qualified { Kind::Member } else { Kind::Free },
                    });
                    continue;
                }
                [] => {}
                _ => {
                    report.ambiguous += 1;
                    continue;
                }
            }
            // No declaration of that name. A qualified definition may still
            // belong to a class that declares no such member.
            if !qualified {
                continue;
            }
            let owners: Vec<(usize, usize)> = names
                .iter()
                .filter_map(|name| name.rsplit_once("::").map(|(class, _)| class))
                .map(|class| {
                    classes
                        .get(class)
                        .map(|v| v.iter().filter(reachable).copied().collect::<Vec<_>>())
                        .unwrap_or_default()
                })
                .find(|found| !found.is_empty())
                .unwrap_or_default();
            match owners.as_slice() {
                [(g, s)] => {
                    moves.push(Move {
                        from_file: fi,
                        definition: def.id.clone(),
                        to_file: *g,
                        target: files[*g].symbols[*s].id.clone(),
                        kind: Kind::Class,
                    });
                }
                [] => report.unmatched += 1,
                _ => report.ambiguous += 1,
            }
        }
    }

    // What was actually done, not what was planned.
    for (mv, done) in moves.iter().zip(apply(files, &moves)) {
        if !done {
            continue;
        }
        match mv.kind {
            Kind::Member => report.paired += 1,
            Kind::Free => {
                report.paired += 1;
                report.free_paired += 1;
            }
            Kind::Class => report.to_class += 1,
        }
    }
    report
}

/// Carry out the moves: the definition leaves its file, its target owns its
/// lines, and the hashes of the target and of every class around it fold the
/// body in. Returns, for each move, whether it was made.
///
/// A move is made only if both ends exist, and never into a target that is
/// itself being moved (the body would go with it and be lost); a definition
/// whose move is not made stays where it is. Definitions are removed from
/// their files after every move has been looked at, so the positions used
/// while moving stay valid, and each file's id map is built once.
fn apply(files: &mut [FileExtraction], moves: &[Move]) -> Vec<bool> {
    let index: Vec<HashMap<String, usize>> = files
        .iter()
        .map(|f| {
            f.symbols
                .iter()
                .enumerate()
                .map(|(i, s)| (s.id.clone(), i))
                .collect()
        })
        .collect();
    let being_moved: HashSet<(usize, &str)> = moves
        .iter()
        .map(|mv| (mv.from_file, mv.definition.as_str()))
        .collect();

    let mut applied = vec![false; moves.len()];
    let mut removals: Vec<HashSet<&str>> = vec![HashSet::new(); files.len()];
    // What each target (and each class around it) takes in: the hashes of the
    // bodies. Sorted before use, so the order bodies are written in, and the
    // order files are visited in, moves nothing.
    let mut bodies: HashMap<(usize, String), Vec<String>> = HashMap::new();
    for (k, mv) in moves.iter().enumerate() {
        if being_moved.contains(&(mv.to_file, mv.target.as_str())) {
            continue;
        }
        let (Some(&d), Some(&t)) = (
            index[mv.from_file].get(&mv.definition),
            index[mv.to_file].get(&mv.target),
        ) else {
            continue;
        };
        let from_path = files[mv.from_file].rel_path.clone();
        let def = files[mv.from_file].symbols[d].clone();
        let target = &mut files[mv.to_file].symbols[t];
        target.other_file_spans.push((from_path.clone(), def.lines));
        target.other_file_spans.extend(
            def.extra_spans
                .iter()
                .map(|span| (from_path.clone(), *span)),
        );
        target
            .other_file_spans
            .extend(def.other_file_spans.iter().cloned());
        // It has a body now.
        target.markers.retain(|m| m != "declaration");
        if target.raw == "prototype" {
            target.raw = "function".to_string();
        }
        target.docstring = target.docstring.take().or(def.docstring);
        // The body's hash goes into the target and every class around it.
        let mut at = Some(t);
        while let Some(i) = at {
            let sym = &files[mv.to_file].symbols[i];
            bodies
                .entry((mv.to_file, sym.id.clone()))
                .or_default()
                .push(def.source_hash.clone());
            at = sym
                .parent
                .as_ref()
                .and_then(|p| index[mv.to_file].get(p).copied());
        }
        removals[mv.from_file].insert(mv.definition.as_str());
        applied[k] = true;
    }
    for ((fi, id), mut hashes) in bodies {
        hashes.sort();
        if let Some(&i) = index[fi].get(&id) {
            let sym = &mut files[fi].symbols[i];
            sym.source_hash = hash_text(&format!("{}\n{}", sym.source_hash, hashes.join("\n")));
        }
    }
    for (file, gone) in files.iter_mut().zip(&removals) {
        if !gone.is_empty() {
            file.symbols.retain(|s| !gone.contains(s.id.as_str()));
        }
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpp::{extract_file, includes};
    use crate::hash::hash_text;
    use crate::symbol::ExtractedSymbol;

    fn build(files: &[(&str, &str)]) -> (Vec<FileExtraction>, HashMap<String, FileImports>) {
        let extractions = files
            .iter()
            .map(|(path, text)| FileExtraction {
                rel_path: path.to_string(),
                source_hash: hash_text(text),
                symbols: extract_file(text, path),
            })
            .collect();
        let imports = files
            .iter()
            .map(|(path, text)| (path.to_string(), includes::extract(text)))
            .collect();
        (extractions, imports)
    }

    /// Pair `files` and return them with the report.
    fn paired(files: &[(&str, &str)]) -> (Vec<FileExtraction>, PairingReport) {
        let (mut extractions, imports) = build(files);
        let report = pair(&mut extractions, &imports);
        (extractions, report)
    }

    #[track_caller]
    fn find<'a>(files: &'a [FileExtraction], id: &str) -> &'a ExtractedSymbol {
        files
            .iter()
            .flat_map(|f| &f.symbols)
            .find(|s| s.id == id)
            .unwrap_or_else(|| {
                let all: Vec<&str> = files
                    .iter()
                    .flat_map(|f| f.symbols.iter().map(|s| s.id.as_str()))
                    .collect();
                panic!("no symbol `{id}`; have {all:?}")
            })
    }

    fn ids<'a>(files: &'a [FileExtraction], path: &str) -> Vec<&'a str> {
        files
            .iter()
            .find(|f| f.rel_path == path)
            .unwrap()
            .symbols
            .iter()
            .map(|s| s.id.as_str())
            .collect()
    }

    fn has_marker(s: &ExtractedSymbol, marker: &str) -> bool {
        s.markers.iter().any(|m| m == marker)
    }

    const DB_H: &str = "class DB {\n public:\n  int Get(int k);\n  int Put(int k);\n};\n";
    const DB_CC: &str = "#include \"db.h\"\nint DB::Get(int k) {\n  return k;\n}\nint DB::Put(int k) {\n  return k + 1;\n}\n";

    #[test]
    fn a_method_body_in_the_source_file_moves_into_the_member_it_defines() {
        let (before, _) = build(&[("db.h", DB_H), ("db.cc", DB_CC)]);
        let (files, report) = paired(&[("db.h", DB_H), ("db.cc", DB_CC)]);
        assert_eq!(report.paired, 2);

        // The definitions are gone from the source file, which now has no
        // symbol of its own.
        assert!(
            ids(&files, "db.cc").is_empty(),
            "{:?}",
            ids(&files, "db.cc")
        );
        // Each member owns its body's lines, labelled with the file.
        let get = find(&files, "db.h::DB::Get");
        assert_eq!(get.other_file_spans, [("db.cc".to_string(), [2, 4])]);
        assert_eq!(
            find(&files, "db.h::DB::Put").other_file_spans,
            [("db.cc".to_string(), [5, 7])]
        );
        // It is still the class's member, now with a body.
        assert_eq!(get.raw, "method");
        assert_eq!(get.parent.as_deref(), Some("db.h::DB"));
        assert!(!has_marker(get, "declaration"), "{:?}", get.markers);
        assert_eq!(get.signature, "int Get(int k)", "the declaration's");
        // The class gathers its members' spans when read; it holds none itself.
        assert!(find(&files, "db.h::DB").other_file_spans.is_empty());

        // A body edit moves the member's and the class's source hash and
        // neither interface hash: users of the class stay current.
        let old_get = find(&before, "db.h::DB::Get");
        assert_ne!(get.source_hash, old_get.source_hash);
        let class = find(&files, "db.h::DB");
        let old_class = find(&before, "db.h::DB");
        assert_ne!(class.source_hash, old_class.source_hash);
        assert_eq!(class.interface_hash, old_class.interface_hash);
        assert_eq!(get.interface_hash, old_get.interface_hash);
    }

    #[test]
    fn editing_a_body_moves_the_hashes_and_reordering_two_bodies_moves_nothing() {
        let a = paired(&[("db.h", DB_H), ("db.cc", DB_CC)]).0;
        let edited = DB_CC.replace("return k + 1;", "return k + 2;");
        let b = paired(&[("db.h", DB_H), ("db.cc", &edited)]).0;
        assert_ne!(
            find(&a, "db.h::DB").source_hash,
            find(&b, "db.h::DB").source_hash,
            "a body edit moves the class"
        );
        assert_ne!(
            find(&a, "db.h::DB::Put").source_hash,
            find(&b, "db.h::DB::Put").source_hash
        );
        assert_eq!(
            find(&a, "db.h::DB::Get").source_hash,
            find(&b, "db.h::DB::Get").source_hash,
            "the other member's body did not change"
        );
        assert_eq!(
            find(&a, "db.h::DB").interface_hash,
            find(&b, "db.h::DB").interface_hash
        );
        // The same two bodies written in the other order, same lines.
        let swapped = "#include \"db.h\"\nint DB::Put(int k) {\n  return k + 1;\n}\nint DB::Get(int k) {\n  return k;\n}\n";
        let c = paired(&[("db.h", DB_H), ("db.cc", swapped)]).0;
        assert_eq!(
            find(&a, "db.h::DB").source_hash,
            find(&c, "db.h::DB").source_hash
        );
    }

    #[test]
    fn a_header_that_holds_some_bodies_pairs_only_the_rest() {
        let h = "class A {\n public:\n  int f() { return 1; }\n  int g();\n};\n";
        let cc = "#include \"a.h\"\nint A::g() { return 2; }\n";
        let (files, report) = paired(&[("a.h", h), ("a.cc", cc)]);
        assert_eq!(report.paired, 1);
        assert!(find(&files, "a.h::A::f").other_file_spans.is_empty());
        assert_eq!(
            find(&files, "a.h::A::g").other_file_spans,
            [("a.cc".to_string(), [2, 2])]
        );
    }

    #[test]
    fn a_body_below_its_class_in_the_same_file_is_left_to_the_fold() {
        let h = "class A {\n public:\n  int f();\n};\nint A::f() { return 1; }\n";
        let (files, report) = paired(&[("a.h", h)]);
        assert_eq!(report, PairingReport::default());
        // One symbol, not paired: the same-id fold already made it one.
        assert_eq!(ids(&files, "a.h"), ["a.h::A", "a.h::A::f"]);
        assert!(find(&files, "a.h::A::f").other_file_spans.is_empty());
    }

    #[test]
    fn a_source_file_with_its_own_classes_and_functions_is_left_alone() {
        let cc = "namespace {\nclass Helper { public: int f() { return 1; } };\n}\nstatic int util() { return 2; }\nint Public() { return util(); }\n";
        let (files, report) = paired(&[("a.cc", cc)]);
        assert_eq!(report, PairingReport::default());
        assert_eq!(ids(&files, "a.cc").len(), 4, "{:?}", ids(&files, "a.cc"));
    }

    #[test]
    fn a_free_function_defined_in_the_source_file_moves_into_its_prototype() {
        let h = "// Builds a table.\nint BuildTable(int n);\n";
        let cc = "#include \"b.h\"\nint BuildTable(int n) {\n  return n;\n}\n";
        let (files, report) = paired(&[("b.h", h), ("b.cc", cc)]);
        assert_eq!(report.paired, 1);
        assert!(ids(&files, "b.cc").is_empty());
        let proto = find(&files, "b.h::BuildTable");
        assert_eq!(proto.other_file_spans, [("b.cc".to_string(), [2, 4])]);
        // It has a body now: no longer a bare prototype.
        assert_eq!(proto.raw, "function");
        assert!(!has_marker(proto, "declaration"));
        assert!(proto.docstring.is_some(), "the prototype's own comment");
    }

    #[test]
    fn a_free_function_pairs_only_with_a_bare_prototype_not_with_another_body() {
        // `f` is defined in the header and again, as an overload, in the
        // source file: two bodies, no prototype. It is not the header's
        // function's implementation.
        let h = "inline int f(int a) { return a; }\n";
        let cc = "#include \"f.h\"\nint f(double a) { return 1; }\n";
        let (files, report) = paired(&[("f.h", h), ("f.cc", cc)]);
        assert_eq!(report.paired, 0, "{report:?}");
        assert_eq!(report.ambiguous, 0);
        assert_eq!(ids(&files, "f.cc"), ["f.cc::f"]);
        assert!(find(&files, "f.h::f").other_file_spans.is_empty());
    }

    #[test]
    fn a_static_helper_is_never_paired_with_a_prototype_of_the_same_name() {
        let h = "int Helper(int x);\n";
        let cc = "#include \"b.h\"\nstatic int Helper(int x) { return x; }\n";
        let (files, report) = paired(&[("b.h", h), ("b.cc", cc)]);
        assert_eq!(report.paired, 0);
        assert_eq!(ids(&files, "b.cc"), ["b.cc::Helper"]);
        assert!(find(&files, "b.h::Helper").other_file_spans.is_empty());
    }

    #[test]
    fn a_class_implemented_across_two_source_files_owns_every_body_labelled_by_file() {
        let h = "class Env {\n public:\n  int Now();\n};\n";
        let posix = "#include \"env.h\"\nint Env::Now() { return 1; }\n";
        let win = "#include \"env.h\"\n\nint Env::Now() { return 2; }\n";
        let (files, report) = paired(&[
            ("env.h", h),
            ("env_posix.cc", posix),
            ("env_windows.cc", win),
        ]);
        assert_eq!(report.paired, 2);
        assert_eq!(
            find(&files, "env.h::Env::Now").other_file_spans,
            [
                ("env_posix.cc".to_string(), [2, 2]),
                ("env_windows.cc".to_string(), [3, 3])
            ]
        );
    }

    #[test]
    fn a_member_that_already_has_a_body_in_the_header_still_receives_the_source_files() {
        // abseil's `CycleClock::Now` is defined inline in the header for one
        // configuration and in the `.cc` for another: a member with a body in
        // each file. The header's own fold dropped its `declaration` marker,
        // and the `.cc` body belongs to the member all the same.
        let h = "class C {\n public:\n  static int Now();\n};\ninline int C::Now() { return 1; }\n";
        let cc = "#include \"c.h\"\nint C::Now() { return 2; }\n";
        let (files, report) = paired(&[("c.h", h), ("c.cc", cc)]);
        assert_eq!(report.paired, 1, "{report:?}");
        assert_eq!(report.to_class, 0);
        assert_eq!(
            find(&files, "c.h::C::Now").other_file_spans,
            [("c.cc".to_string(), [2, 2])]
        );
        assert!(find(&files, "c.h::C").other_file_spans.is_empty());
    }

    #[test]
    fn an_operator_of_a_template_class_pairs_although_its_definition_names_the_arguments() {
        // `btree<P>::operator=` carries the class's template arguments in its
        // qualification; the member's id is `btree::operator=`.
        let h = "template <class P>\nclass btree {\n public:\n  btree& operator=(const btree& o);\n  bool operator()(int a) const;\n};\n";
        let cc = "#include \"b.h\"\ntemplate <class P>\nbtree<P>& btree<P>::operator=(const btree& o) { return *this; }\ntemplate <class P>\nbool btree<P>::operator()(int a) const { return a > 0; }\n";
        let (files, report) = paired(&[("b.h", h), ("b.cc", cc)]);
        assert_eq!(report.paired, 2, "{report:?}");
        assert_eq!(report.unmatched, 0);
        assert!(ids(&files, "b.cc").is_empty(), "{:?}", ids(&files, "b.cc"));
        assert_eq!(
            find(&files, "b.h::btree::operator=").other_file_spans.len(),
            1
        );
        assert_eq!(
            find(&files, "b.h::btree::operator()")
                .other_file_spans
                .len(),
            1
        );
    }

    #[test]
    fn a_name_written_with_the_enclosing_namespace_again_still_pairs() {
        // abseil's status.cc: inside `namespace absl`, `absl::Status::Make`.
        // C++ finds `absl` by looking outward from the innermost namespace,
        // so the entity is `absl::Status::Make`, not `absl::absl::Status::Make`.
        let h = "namespace absl {\nclass Status {\n public:\n  static int Make(int m);\n};\n}\n";
        let cc =
            "#include \"s.h\"\nnamespace absl {\nint absl::Status::Make(int m) { return m; }\n}\n";
        let (files, report) = paired(&[("s.h", h), ("s.cc", cc)]);
        assert_eq!(report.paired, 1, "{report:?}");
        assert_eq!(report.unmatched, 0);
        assert!(ids(&files, "s.cc").is_empty(), "{:?}", ids(&files, "s.cc"));
        assert_eq!(
            find(&files, "s.h::absl::Status::Make").other_file_spans,
            [("s.cc".to_string(), [3, 3])]
        );
    }

    #[test]
    fn the_repeated_namespace_may_be_an_inner_one() {
        // Inside `namespace a { namespace b {`, `b::C::f` is `a::b::C::f`.
        let h = "namespace a {\nnamespace b {\nclass C {\n public:\n  int f();\n};\n}\n}\n";
        let cc =
            "#include \"c.h\"\nnamespace a {\nnamespace b {\nint b::C::f() { return 1; }\n}\n}\n";
        let (files, report) = paired(&[("c.h", h), ("c.cc", cc)]);
        assert_eq!(report.paired, 1, "{report:?}");
        assert_eq!(find(&files, "c.h::a::b::C::f").other_file_spans.len(), 1);
    }

    #[test]
    fn the_innermost_namespace_that_holds_the_name_wins() {
        // `a::Status` is found before `::Status` from inside `namespace a`:
        // only the inner one gets the body, and it is not ambiguous.
        let outer = "class Status {\n public:\n  int f();\n};\n";
        let inner = "namespace a {\nclass Status {\n public:\n  int f();\n};\n}\n";
        let cc =
            "#include \"o.h\"\n#include \"i.h\"\nnamespace a {\nint Status::f() { return 1; }\n}\n";
        let (files, report) = paired(&[("o.h", outer), ("i.h", inner), ("c.cc", cc)]);
        assert_eq!(report.paired, 1, "{report:?}");
        assert_eq!(report.ambiguous, 0);
        assert_eq!(find(&files, "i.h::a::Status::f").other_file_spans.len(), 1);
        assert!(find(&files, "o.h::Status::f").other_file_spans.is_empty());
    }

    #[test]
    fn a_namespace_qualified_function_pairs_with_the_prototype_in_its_namespace() {
        let h = "namespace absl {\nnamespace base_internal {\nvoid SpinLockWake(int* w);\n}\n}\n";
        // Written inside the namespaces, and written out in full.
        let inside = "#include \"s.h\"\nnamespace absl {\nnamespace base_internal {\nvoid SpinLockWake(int* w) { }\n}\n}\n";
        let outside = "#include \"s.h\"\nvoid absl::base_internal::SpinLockWake(int* w) { }\n";
        for cc in [inside, outside] {
            let (files, report) = paired(&[("s.h", h), ("s.cc", cc)]);
            assert_eq!(report.paired, 1, "{cc}");
            assert!(ids(&files, "s.cc").is_empty(), "{cc}");
            assert_eq!(
                find(&files, "s.h::absl::base_internal::SpinLockWake")
                    .other_file_spans
                    .len(),
                1
            );
        }
    }

    #[test]
    fn a_definition_written_inside_the_namespace_pairs_with_a_member_of_a_namespaced_class() {
        let h = "namespace n {\nclass A {\n public:\n  int f();\n};\n}\n";
        let inside = "#include \"a.h\"\nnamespace n {\nint A::f() { return 1; }\n}\n";
        let outside = "#include \"a.h\"\nint n::A::f() { return 1; }\n";
        for cc in [inside, outside] {
            let (files, report) = paired(&[("a.h", h), ("a.cc", cc)]);
            assert_eq!(report.paired, 1, "{cc}");
            assert_eq!(
                find(&files, "a.h::n::A::f").other_file_spans.len(),
                1,
                "{cc}"
            );
        }
    }

    #[test]
    fn a_static_data_member_defined_in_the_source_file_moves_into_its_field() {
        let h = "class C {\n public:\n  static int count;\n};\n";
        let cc = "#include \"c.h\"\nint C::count = 0;\n";
        let (files, report) = paired(&[("c.h", h), ("c.cc", cc)]);
        assert_eq!(report.paired, 1);
        assert_eq!(
            find(&files, "c.h::C::count").other_file_spans,
            [("c.cc".to_string(), [2, 2])]
        );
        assert!(ids(&files, "c.cc").is_empty());
    }

    #[test]
    fn a_member_of_a_nested_class_pairs_through_its_whole_path() {
        let h = "class O {\n public:\n  class I {\n   public:\n    void f();\n  };\n};\n";
        let cc = "#include \"o.h\"\nvoid O::I::f() { }\n";
        let (files, report) = paired(&[("o.h", h), ("o.cc", cc)]);
        assert_eq!(report.paired, 1);
        assert_eq!(find(&files, "o.h::O::I::f").other_file_spans.len(), 1);
        // The outer class's hash moves too: the body is inside it.
        let (before, _) = build(&[("o.h", h), ("o.cc", cc)]);
        assert_ne!(
            find(&files, "o.h::O").source_hash,
            find(&before, "o.h::O").source_hash
        );
    }

    #[test]
    fn a_definition_of_a_member_the_class_does_not_declare_moves_into_the_class() {
        // A member a macro declared is not a symbol; the class is the home.
        let h = "class A {\n public:\n  int x;\n};\n";
        let cc = "#include \"a.h\"\nint A::made_by_a_macro() { return 1; }\n";
        let (files, report) = paired(&[("a.h", h), ("a.cc", cc)]);
        assert_eq!(report.to_class, 1);
        assert_eq!(
            find(&files, "a.h::A").other_file_spans,
            [("a.cc".to_string(), [2, 2])]
        );
        assert!(ids(&files, "a.cc").is_empty());
    }

    #[test]
    fn two_reachable_declarations_of_one_name_leave_the_definition_where_it_is() {
        let a = "namespace x {\nclass S {\n public:\n  void f();\n};\n}\n";
        let b = "namespace x {\nclass S {\n public:\n  void f();\n};\n}\n";
        let cc = "#include \"a.h\"\n#include \"b.h\"\nvoid x::S::f() { }\n";
        let (files, report) = paired(&[("a.h", a), ("b.h", b), ("c.cc", cc)]);
        assert_eq!(report.ambiguous, 1);
        assert_eq!(report.paired, 0);
        assert_eq!(ids(&files, "c.cc"), ["c.cc::x::S::f"]);
        assert!(find(&files, "a.h::x::S::f").other_file_spans.is_empty());
        assert!(find(&files, "b.h::x::S::f").other_file_spans.is_empty());
    }

    #[test]
    fn a_definition_whose_header_the_file_does_not_reach_stays_unpaired() {
        let (files, report) = paired(&[
            ("db.h", DB_H),
            ("db.cc", &DB_CC.replace("#include \"db.h\"\n", "")),
        ]);
        assert_eq!(report.paired, 0);
        assert_eq!(report.unmatched, 2);
        assert_eq!(ids(&files, "db.cc").len(), 2);
    }

    #[test]
    fn a_definition_of_something_outside_the_repository_stays_and_is_counted() {
        let cc = "int std::foo::bar() { return 1; }\n";
        let (files, report) = paired(&[("a.cc", cc)]);
        assert_eq!(report.unmatched, 1);
        assert_eq!(ids(&files, "a.cc").len(), 1);
    }

    #[test]
    fn a_header_reached_through_another_header_is_found() {
        let cc = "#include \"mid.h\"\nint DB::Get(int k) { return k; }\n";
        let (files, report) = paired(&[
            ("db.h", DB_H),
            ("mid.h", "#include \"db.h\"\n"),
            ("db.cc", cc),
        ]);
        assert_eq!(report.paired, 1);
        assert_eq!(find(&files, "db.h::DB::Get").other_file_spans.len(), 1);
    }

    #[test]
    fn the_definitions_comment_is_kept_when_the_declaration_has_none() {
        let cc = "#include \"db.h\"\n\n// Looks the key up.\nint DB::Get(int k) { return k; }\n";
        let (files, _) = paired(&[("db.h", DB_H), ("db.cc", cc)]);
        let get = find(&files, "db.h::DB::Get");
        assert!(
            get.docstring
                .as_deref()
                .is_some_and(|d| d.contains("Looks the key up")),
            "{:?}",
            get.docstring
        );
    }

    #[test]
    fn a_move_whose_target_is_gone_leaves_the_definition_in_place_and_is_not_counted() {
        // Moves are planned before any is made; one that cannot be carried
        // out must not take the definition out of its file and lose it.
        let (mut files, _) = build(&[("db.h", DB_H), ("db.cc", DB_CC)]);
        let mv = Move {
            from_file: 1,
            definition: "db.cc::DB::Get".to_string(),
            to_file: 0,
            target: "db.h::DB::NoSuchMember".to_string(),
            kind: Kind::Member,
        };
        let before = files.clone();
        assert_eq!(apply(&mut files, &[mv]), [false]);
        assert_eq!(files, before, "nothing changed");
    }

    #[test]
    fn a_body_is_never_moved_into_a_symbol_that_is_itself_being_moved() {
        // `t.h::f` has a body and is moved into the prototype `p.h::f`;
        // `c.cc::f` was planned to go into `t.h::f`. Whichever is handled
        // first, the body must not end up on a symbol that is removed.
        let (mut files, _) = build(&[
            ("c.cc", "void f() { }\n"),
            ("p.h", "void f();\n"),
            ("t.h", "inline void f() { }\n"),
        ]);
        let moves = [
            Move {
                from_file: 0,
                definition: "c.cc::f".to_string(),
                to_file: 2,
                target: "t.h::f".to_string(),
                kind: Kind::Member,
            },
            Move {
                from_file: 2,
                definition: "t.h::f".to_string(),
                to_file: 1,
                target: "p.h::f".to_string(),
                kind: Kind::Free,
            },
        ];
        assert_eq!(apply(&mut files, &moves), [false, true]);
        assert_eq!(ids(&files, "c.cc"), ["c.cc::f"], "kept, not lost");
        assert!(ids(&files, "t.h").is_empty());
        assert_eq!(
            find(&files, "p.h::f").other_file_spans,
            [("t.h".to_string(), [1, 1])]
        );
    }

    #[test]
    fn pairing_twice_gives_what_pairing_once_does() {
        // `rebuild` runs on the cached extractions every time; a second pass
        // over an already-paired set must be a no-op, not a second move.
        let (mut files, imports) = build(&[("db.h", DB_H), ("db.cc", DB_CC)]);
        pair(&mut files, &imports);
        let once = files.clone();
        let again = pair(&mut files, &imports);
        assert_eq!(again, PairingReport::default());
        assert_eq!(files, once);
    }
}
