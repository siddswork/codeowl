//! Walks a single TypeScript/TSX source file with tree-sitter and pulls out
//! its top-level `Symbol`s.
//!
//! **TypeScript + Next.js pack — Phase 2 seam here.** The tree-sitter
//! grammar and every `node.kind()` string below are TypeScript-specific; a
//! second language needs its own extractor. See `ROADMAP.md`'s "Stack
//! modularization".
//!
//! Deliberately *not* a recursive walk of every node in the tree: we only
//! look at `program`'s direct children (unwrapping `export` statements) and,
//! for classes, one level into `class_body`. Nested closures, callbacks
//! passed to `useEffect`, etc. are implementation detail, not declarations
//! CodeOwl would ever generate a spec for — see `CLAUDE.md`'s note that
//! generation only recurses containment edges between *symbols*, not every
//! syntax node.

use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use crate::hash::hash_text;
use crate::imports::{bare_export_clause, export_specs};
use crate::lang::ts_parser;
use crate::symbol::{ExtractedSymbol, SymbolKind};

/// Parse `source` (the contents of `rel_path`) and extract its symbols.
///
/// `rel_path`'s extension picks the grammar: `.tsx` gets JSX support,
/// everything else parses as plain TypeScript.
pub fn extract_file(source: &str, rel_path: &str) -> Vec<ExtractedSymbol> {
    let mut parser = ts_parser(rel_path);

    // `tree` owns the arena the whole `Node<'_>` chain below borrows from.
    // We never let a `Node` outlive this function — every value we push
    // onto `out` is an owned `String`/`usize`, extracted while `tree` is
    // still alive. That's the "don't let Node<'a> escape the parse
    // function" rule from CLAUDE.md: it's not that Node is unsafe to use,
    // it's that storing one ties whatever holds it to this Tree's lifetime.
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let root = tree.root_node();
    let exported = clause_exported_names(root, source);
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        visit_top_level(child, source, rel_path, &exported, &mut out);
    }
    let mut folded = fold_same_id(out);
    // A type declaration computes its `interface_hash` whether or not it is
    // exported, so that `fold_same_id` can combine a private half with an
    // exported one (see there). Only an exported symbol keeps it.
    for sym in &mut folded {
        if !sym.is_exported {
            sym.interface_hash = None;
        }
    }
    folded
}

/// The local names a bare export clause exports: `function Button() {}`
/// followed by `export { Button, buttonVariants };` exports `Button` just as
/// `export function Button() {}` would. Only the clause with no `from`
/// counts (`export { a } from './x'` forwards another module's `a`, and says
/// nothing about a local one), and only an unrenamed name: `export { a as b }`
/// publishes `b`, which is not this declaration's id, so importers could not
/// find it by name anyway (deliberately not handled; no case in the repos
/// measured). A name that is not declared in this file is harmless here.
fn clause_exported_names(root: Node, source: &str) -> HashSet<String> {
    let mut cursor = root.walk();
    root.children(&mut cursor)
        .filter_map(bare_export_clause)
        .flat_map(|clause| export_specs(clause, source))
        .filter(|spec| spec.alias.is_none())
        .map(|spec| spec.name.to_string())
        .collect()
}

/// Is a top-level declaration called `name`, written as (or inside) `outer`,
/// exported: either it carries the `export` keyword itself, or a bare export
/// clause names it. The one place that decides, so no visitor can forget the
/// clause half.
fn is_declaration_exported(outer: Node, name: &str, clause_exported: &HashSet<String>) -> bool {
    outer.kind() == "export_statement" || clause_exported.contains(name)
}

/// Handle one direct child of `program`: unwrap an `export` wrapper if
/// present, dispatch on the declaration kind, and push whatever symbols it
/// produces onto `out`. `exported` is the set of names a bare export clause
/// exports (`clause_exported_names`); a declaration is exported if it carries
/// the keyword itself or is named there.
fn visit_top_level(
    node: Node,
    source: &str,
    file: &str,
    exported: &HashSet<String>,
    out: &mut Vec<ExtractedSymbol>,
) {
    let decl = if node.kind() == "export_statement" {
        match node.child_by_field_name("declaration") {
            Some(d) => d,
            // `export { X } from './y'`, `export * from './y'`, or an
            // *anonymous* `export default <expr>` (an arrow function, an
            // identifier, a literal) — nothing new is *declared* here for
            // M1 to extract. Barrel files are exactly this case. A
            // *named* default export (`export default function Page()
            // {}`, `export default class Foo {}`) does have a
            // `declaration` field and falls through to the match below
            // like any other declaration — Next.js page/route components
            // routinely take this shape, so M5's feature layer relies on
            // it being extracted, not skipped.
            None => return,
        }
    } else {
        node
    };

    match decl.kind() {
        "function_declaration" => visit_function(decl, node, source, file, exported, out),
        "class_declaration" => visit_class(decl, node, source, file, exported, out),
        "lexical_declaration" => visit_lexical(decl, node, source, file, exported, out),
        // Type-level declarations. They carry no runtime behavior, but an
        // importer depends on their shape exactly as it does on a
        // function's signature -- without a symbol here, `import { User }`
        // resolved to nothing and a change to `User` never staled anyone.
        "type_alias_declaration" => {
            visit_type_decl(decl, node, "type", source, file, exported, out)
        }
        "interface_declaration" => {
            visit_type_decl(decl, node, "interface", source, file, exported, out)
        }
        "enum_declaration" => visit_type_decl(decl, node, "enum", source, file, exported, out),
        _ => {}
    }
}

/// One `type X = …` / `interface X {…}` / `enum X {…}` as a single `Value`
/// symbol -- a leaf, like a plain `const`, so it is an import target with an
/// `interface_hash` but gets no spec section of its own (spec-bearing is
/// `Callable`/`Container` only).
///
/// Unlike a function, whose contract is its signature and not its body, a
/// type's contract *is* its body: add, rename or retype a member and an
/// importer can break. So `interface_hash` covers the whole declaration,
/// as its tokens (see `declaration_tokens`: no comments, no formatting),
/// while `signature` stays the short header (`interface User`) that
/// `get_symbol` shows.
fn visit_type_decl(
    decl: Node,
    outer: Node,
    raw: &str,
    source: &str,
    file: &str,
    exported: &HashSet<String>,
    out: &mut Vec<ExtractedSymbol>,
) {
    let Some(name_node) = decl.child_by_field_name("name") else {
        return;
    };
    let name = text(name_node, source);
    // The header is everything before the body: `interface User extends
    // Base` stops at the `{`. A type alias has no body, only a `value`
    // after the `=`, so its header ends with its name or its type
    // parameters (`type Box<T = {}>`) -- cut there, not at the `=`, which
    // a comment or a `=` inside the parameters would otherwise confuse.
    let signature = match decl.child_by_field_name("body") {
        Some(body) => signature_text(decl, body, source),
        None => {
            if decl.child_by_field_name("value").is_none() {
                return;
            }
            let header_end = decl
                .child_by_field_name("type_parameters")
                .unwrap_or(name_node)
                .end_byte();
            source[decl.start_byte()..header_end].to_string()
        }
    };
    let is_exported = is_declaration_exported(outer, name, exported);
    out.push(ExtractedSymbol {
        id: format!("{file}::{name}"),
        kind: SymbolKind::Value,
        raw: raw.to_string(),
        file: file.to_string(),
        lines: node_lines(decl),
        source_hash: hash_text(text(decl, source)),
        // Always computed here; `extract_file` drops it for a symbol that
        // ends up not exported.
        interface_hash: Some(hash_text(&declaration_tokens(decl, source))),
        signature,
        docstring: leading_doc(outer, source),
        is_exported,
        markers: Vec::new(),
        extra_spans: Vec::new(),
        parent: None,
        children: Vec::new(),
    });
}

/// `node`'s tokens -- its leaf nodes' text, comments left out -- joined by
/// single spaces. Hashing this instead of the raw text means neither a
/// comment edit nor a reformat (indentation, line wrapping, a comment that
/// used to sit between two tokens) moves the hash; only a change to the
/// tokens themselves does. It works on the syntax tree, not a regex, so a
/// `//` inside a string literal type is a token, not a comment.
fn declaration_tokens(node: Node, source: &str) -> String {
    let mut tokens = Vec::new();
    collect_tokens(node, source, &mut tokens);
    tokens.join(" ")
}

fn collect_tokens<'a>(node: Node, source: &'a str, out: &mut Vec<&'a str>) {
    if node.kind() == "comment" {
        return;
    }
    if node.child_count() == 0 {
        let token = text(node, source);
        if !token.is_empty() {
            out.push(token);
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_tokens(child, source, out);
    }
}

/// Merge top-level symbols that share an id into one.
///
/// TypeScript lets several declarations share a name: a `const` and a
/// `type` (`export const Status = {…} as const; export type Status = …`),
/// two `interface`s that merge, or an `interface` merged into a `class`.
/// Two symbols with one id would overwrite each other in the graph's id
/// map, so they fold -- the same move the Rust pack makes for an `impl`
/// block. The declaration with the strongest kind (`Container`, then
/// `Callable`, then `Value`) supplies the kind, signature and span, so a
/// class merged with an interface stays a spec-bearing `Container`; ties go
/// to the first. The others add their ranges to `extra_spans`, their hashes
/// into both hashes (a private half counts: it is part of the merged type),
/// and their children. The folded symbol is exported if any declaration is,
/// and keeps whichever doc comment exists.
fn fold_same_id(symbols: Vec<ExtractedSymbol>) -> Vec<ExtractedSymbol> {
    fn strength(kind: &SymbolKind) -> u8 {
        match kind {
            SymbolKind::Container => 2,
            SymbolKind::Callable => 1,
            SymbolKind::Value | SymbolKind::Schema => 0,
        }
    }

    let mut folded: Vec<ExtractedSymbol> = Vec::with_capacity(symbols.len());
    let mut index: HashMap<String, usize> = HashMap::new();
    for mut sym in symbols {
        let Some(&at) = index.get(&sym.id) else {
            index.insert(sym.id.clone(), folded.len());
            folded.push(sym);
            continue;
        };
        let first = &mut folded[at];
        // The stronger declaration becomes the survivor; the other is
        // folded into it below, whichever was written first.
        if strength(&sym.kind) > strength(&first.kind) {
            std::mem::swap(first, &mut sym);
        }
        first.docstring = first.docstring.take().or(sym.docstring);
        first.source_hash = hash_text(&format!("{}\n{}", first.source_hash, sym.source_hash));
        first.interface_hash = match (first.interface_hash.take(), sym.interface_hash) {
            (Some(a), Some(b)) => Some(hash_text(&format!("{a}\n{b}"))),
            (a, b) => a.or(b),
        };
        first.is_exported |= sym.is_exported;
        first.extra_spans.push(sym.lines);
        first.extra_spans.extend(sym.extra_spans);
        first.children.extend(sym.children);
    }
    folded
}

fn visit_function(
    decl: Node,
    outer: Node,
    source: &str,
    file: &str,
    exported: &HashSet<String>,
    out: &mut Vec<ExtractedSymbol>,
) {
    let Some(body) = decl.child_by_field_name("body") else {
        return;
    };
    let name = field_text(decl, "name", source).unwrap_or("<anonymous>");
    let signature = signature_text(decl, body, source);
    let is_exported = is_declaration_exported(outer, name, exported);
    out.push(ExtractedSymbol {
        id: format!("{file}::{name}"),
        kind: SymbolKind::Callable,
        raw: "function".to_string(),
        file: file.to_string(),
        lines: node_lines(decl),
        // `decl`, not `outer` — signature text skips `export`/`export
        // default` for consistency with the const/arrow-function case
        // below, where it's cheaper to just not include it. Whether a
        // symbol is exported is now its own `is_exported` field instead.
        source_hash: hash_text(text(decl, source)),
        interface_hash: is_exported.then(|| hash_text(&signature)),
        signature,
        docstring: leading_doc(outer, source),
        is_exported,
        markers: Vec::new(),
        extra_spans: Vec::new(),
        parent: None,
        children: Vec::new(),
    });
}

fn visit_class(
    decl: Node,
    outer: Node,
    source: &str,
    file: &str,
    exported: &HashSet<String>,
    out: &mut Vec<ExtractedSymbol>,
) {
    let Some(body) = decl.child_by_field_name("body") else {
        return;
    };
    let name = field_text(decl, "name", source).unwrap_or("<anonymous>");
    let class_id = format!("{file}::{name}");

    let mut member_ids = Vec::new();
    let mut member_source_hashes = Vec::new();
    let mut member_symbols = Vec::new();
    let mut cursor = body.walk();
    for member in body.children(&mut cursor) {
        match member.kind() {
            "method_definition" => {
                let Some(m_body) = member.child_by_field_name("body") else {
                    continue;
                };
                let m_name = field_text(member, "name", source).unwrap_or("<anonymous>");
                let m_id = dedupe_member_id(format!("{class_id}.{m_name}"), &member_ids);
                let m_source_hash = hash_text(text(member, source));
                member_symbols.push(ExtractedSymbol {
                    id: m_id.clone(),
                    kind: SymbolKind::Callable,
                    raw: "method".to_string(),
                    file: file.to_string(),
                    lines: node_lines(member),
                    signature: signature_text(member, m_body, source),
                    docstring: leading_doc(member, source),
                    // A method isn't independently exported/imported —
                    // see the doc comment on Symbol::is_exported.
                    is_exported: false,
                    source_hash: m_source_hash.clone(),
                    interface_hash: None,
                    markers: Vec::new(),
                    extra_spans: Vec::new(),
                    parent: Some(class_id.clone()),
                    children: Vec::new(),
                });
                member_ids.push(m_id);
                member_source_hashes.push(m_source_hash);
            }
            // One `Value` member per declared class field — TS's `#priv`
            // JS-private fields land here too (grammar-wise identical to a
            // plain one, just `name`'s own node kind differs), same as
            // `private`-modified ones: a member is never independently
            // exported regardless of its own visibility.
            "public_field_definition" => {
                let Some(f_name) = field_text(member, "name", source) else {
                    continue;
                };
                let f_id = dedupe_member_id(format!("{class_id}.{f_name}"), &member_ids);
                let f_source_hash = hash_text(text(member, source));
                member_symbols.push(ExtractedSymbol {
                    id: f_id.clone(),
                    kind: SymbolKind::Value,
                    raw: "property".to_string(),
                    file: file.to_string(),
                    lines: node_lines(member),
                    signature: field_signature_text(member, source),
                    docstring: leading_doc(member, source),
                    is_exported: false,
                    source_hash: f_source_hash.clone(),
                    interface_hash: None,
                    markers: Vec::new(),
                    extra_spans: Vec::new(),
                    parent: Some(class_id.clone()),
                    children: Vec::new(),
                });
                member_ids.push(f_id);
                member_source_hashes.push(f_source_hash);
            }
            _ => {}
        }
    }

    let signature = signature_text(decl, body, source);
    let is_exported = is_declaration_exported(outer, name, exported);

    // Merkle rollup: the class's own source_hash folds in each member's
    // source_hash (methods and, as of M21, fields), in declaration order
    // (reordering members is a real change too). This is what makes the
    // ancestor-chain hash-propagation validation in ROADMAP.md's M2 entry
    // hold: edit one member, and both that member's and the class's
    // source_hash move.
    //
    // source_hash's own rollup, built here; interface_hash's separate
    // rollup (which *does* fold in public field signatures, per M21.b
    // below) is built further down, once member_symbols exists.
    let mut rollup_input = signature.clone();
    for h in &member_source_hashes {
        rollup_input.push('\n');
        rollup_input.push_str(h);
    }

    // M21.b: interface_hash folds in each *public* field's own signature
    // (name+type, no docstring, no value -- exactly `signature`, per
    // ROADMAP.md's decision table). M21.i extends the same fold to a
    // public method's signature (`ARCHITECTURE.md` open question 13) --
    // `is_pub_field_signature` is name-misleading now but behaviorally
    // exactly right unchanged: "not `private`, not a `#`-private name"
    // is the correct public check for a method too, and a method's
    // `signature` is already body-free (`signature_text`'s own "up to
    // the first `{`" cut), so no stripping is needed for it the way a
    // field's default value once needed `strip_value_for_fold` -- no
    // normalization at all, per open question 13's own lean. Skipped
    // entirely for a non-exported class (code-review finding: it was
    // built unconditionally, then discarded by `.then()` below) --
    // wasted allocation and iteration on every reparse otherwise, and
    // the watcher reparses on every save.
    let mut iface_rollup = signature.clone();
    if is_exported {
        for m in &member_symbols {
            if matches!(m.kind, SymbolKind::Value | SymbolKind::Callable)
                && is_pub_field_signature(&m.signature)
            {
                iface_rollup.push('\n');
                iface_rollup.push_str(&m.signature);
            }
        }
    }

    out.push(ExtractedSymbol {
        id: class_id,
        kind: SymbolKind::Container,
        raw: "class".to_string(),
        file: file.to_string(),
        lines: node_lines(decl),
        source_hash: hash_text(&rollup_input),
        interface_hash: is_exported.then(|| hash_text(&iface_rollup)),
        signature,
        docstring: leading_doc(outer, source),
        is_exported,
        markers: Vec::new(),
        extra_spans: Vec::new(),
        parent: None,
        children: member_ids,
    });
    out.extend(member_symbols);
}

fn visit_lexical(
    decl: Node,
    outer: Node,
    source: &str,
    file: &str,
    exported: &HashSet<String>,
    out: &mut Vec<ExtractedSymbol>,
) {
    // Only `const` is in scope — `let` (and `var`, a different node kind
    // entirely) aren't declarations CodeOwl generates specs for.
    if field_text(decl, "kind", source) != Some("const") {
        return;
    }

    let mut cursor = decl.walk();
    for declarator in decl.children(&mut cursor) {
        if declarator.kind() != "variable_declarator" {
            continue;
        }
        let Some(name_node) = declarator.child_by_field_name("name") else {
            continue;
        };
        // `const { a, b } = ...` / `const [a, b] = ...` bind several names
        // at once. Each becomes its own plain `Value` (below): the names
        // are what an importer asks for -- `export const { auth } =
        // NextAuth(…)` is imported as `auth`, never as the pattern -- and
        // nothing here knows each one's real type, so, like any `const`
        // with a computed value, only its name is its interface.
        let plain = name_node.kind() == "identifier";
        let names = if plain {
            vec![name_node]
        } else {
            bound_names(name_node)
        };
        for name_node in names {
            let name = text(name_node, source);
            let id = format!("{file}::{name}");
            let value = declarator.child_by_field_name("value").filter(|_| plain);

            let (kind, raw, signature) = match value {
                Some(v) if v.kind() == "arrow_function" || v.kind() == "function_expression" => {
                    let body = v.child_by_field_name("body").unwrap_or(v);
                    (
                        SymbolKind::Callable,
                        "function",
                        format!("const {name} = {}", signature_text(v, body, source)),
                    )
                }
                // A plain const's "shape" is its declared type, if any — e.g.
                // `const PI: number = 3.14` — not its literal value, which
                // interface_hash should ignore (below) exactly like a
                // function's body. Include the type annotation's own text
                // (which already carries a leading ": "), so a type change is
                // a real interface_hash change and a value-only edit isn't.
                _ => {
                    // Not for a destructured name: the annotation types the
                    // whole pattern (`const { a }: Props = x`), not `a`.
                    let type_text = declarator
                        .child_by_field_name("type")
                        .filter(|_| plain)
                        .map(|t| text(t, source))
                        .unwrap_or("");
                    (
                        SymbolKind::Value,
                        "const",
                        format!("const {name}{type_text}"),
                    )
                }
            };

            let is_exported = is_declaration_exported(outer, name, exported);
            out.push(ExtractedSymbol {
                id,
                kind,
                raw: raw.to_string(),
                file: file.to_string(),
                lines: node_lines(declarator),
                source_hash: hash_text(text(declarator, source)),
                interface_hash: is_exported.then(|| hash_text(&signature)),
                signature,
                docstring: leading_doc(outer, source),
                is_exported,
                markers: Vec::new(),
                extra_spans: Vec::new(),
                parent: None,
                children: Vec::new(),
            });
        }
    }
}

/// The identifier nodes a destructuring pattern binds, in source order:
/// `{ a, b: c, d = 1, e: { f }, ...g }` binds `a`, `c`, `d`, `f`, `g`, and
/// `[x, , y, ...z]` binds `x`, `y`, `z`. Only what the pattern *binds* counts
/// -- a property key (`b` in `b: c`) and a default value (`1`) don't.
fn bound_names(pattern: Node) -> Vec<Node> {
    let mut names = Vec::new();
    collect_bound_names(pattern, &mut names);
    names
}

fn collect_bound_names<'a>(node: Node<'a>, names: &mut Vec<Node<'a>>) {
    match node.kind() {
        "identifier" | "shorthand_property_identifier_pattern" => names.push(node),
        // `key: pattern` binds whatever the right-hand side binds.
        "pair_pattern" => {
            if let Some(value) = node.child_by_field_name("value") {
                collect_bound_names(value, names);
            }
        }
        // `name = default`: the default is an expression, not a binding.
        "object_assignment_pattern" | "assignment_pattern" => {
            if let Some(left) = node.child_by_field_name("left") {
                collect_bound_names(left, names);
            }
        }
        "comment" => {}
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_bound_names(child, names);
            }
        }
    }
}

/// Slice `source` from `node`'s start up to (not including) `body`'s start,
/// trimmed. This is the "everything before the `{`" trick: it picks up
/// modifiers, name, type parameters, params, and return type without having
/// to name each of those fields individually — and it works identically for
/// `function_declaration`, `method_definition`, `class_declaration`, and
/// arrow functions (whose "body" is an expression when there are no braces).
fn signature_text(node: Node, body: Node, source: &str) -> String {
    let start = node.start_byte();
    let end = body.start_byte().max(start);
    source[start..end].trim_end().to_string()
}

/// If `candidate` is already in `existing`, append a `#2`/`#3`/… suffix
/// until it's unique. Code-review finding: a class field and a method can
/// share a bare name in real, syntactically valid code (tree-sitter parses
/// it regardless of what `tsc` would say), and both naturally compute to
/// the same `{class}.{name}` id under this extractor's scheme -- silently
/// colliding would let one member's arena node overwrite the other's in
/// `Graph::build`'s `by_id` map. Whichever member is declared first keeps
/// the plain id (this scheme's pre-existing, already-relied-upon
/// contract); a later collision backs off.
fn dedupe_member_id(candidate: String, existing: &[String]) -> String {
    if !existing.contains(&candidate) {
        return candidate;
    }
    let mut n = 2;
    loop {
        let next = format!("{candidate}#{n}");
        if !existing.contains(&next) {
            return next;
        }
        n += 1;
    }
}

/// Is a field's own stored `signature` text publicly visible? TS fields
/// are public by default -- no keyword needed, unlike Rust's `pub`
/// -required convention -- so this only excludes the two ways a field
/// opts *out*: an explicit `private` modifier, or a JS `#`-private name
/// (needs no `private` keyword at all, the `#` sigil alone does it, and
/// the signature text starts with the sigil directly since it's part of
/// the name).
fn is_pub_field_signature(signature: &str) -> bool {
    match signature.split_whitespace().next() {
        Some(first) => first != "private" && !first.starts_with('#'),
        None => false,
    }
}

/// A `public_field_definition`'s declaration text with any initializer
/// stripped — the "everything before the `=`" version of `signature_text`'s
/// "everything before the `{`" trick, since a field has no body node to
/// anchor on. Falls back to the whole node (trailing `;` trimmed) when
/// there's no initializer at all.
fn field_signature_text(node: Node, source: &str) -> String {
    let start = node.start_byte();
    let end = node
        .child_by_field_name("value")
        .map(|v| v.start_byte())
        .unwrap_or_else(|| node.end_byte())
        .max(start);
    source[start..end]
        .trim_end()
        .trim_end_matches('=')
        .trim_end()
        .trim_end_matches(';')
        .trim_end()
        .to_string()
}

/// Walk backward over `node`'s immediately preceding siblings, collecting a
/// contiguous run of `comment` nodes with no blank line between them (or
/// between the last comment and `node` itself). Returns `None` if the
/// nearest preceding sibling isn't a comment, or isn't adjacent.
///
/// The adjacency check matters: `prev_sibling()` finds the nearest sibling
/// regardless of blank lines in between, so without it a comment several
/// paragraphs above an unrelated declaration would get misattributed as
/// its docstring.
fn leading_doc(node: Node, source: &str) -> Option<String> {
    let mut comments = Vec::new();
    let mut expected_end_row = node.start_position().row;
    let mut cursor = node.prev_sibling();

    while let Some(n) = cursor {
        if n.kind() == "comment" && n.end_position().row + 1 == expected_end_row {
            expected_end_row = n.start_position().row;
            cursor = n.prev_sibling();
            comments.push(n);
        } else {
            break;
        }
    }

    if comments.is_empty() {
        return None;
    }
    comments.reverse();
    let lines: Vec<String> = comments
        .iter()
        .flat_map(|n| clean_comment(text(*n, source)))
        .collect();
    Some(lines.join("\n"))
}

/// Strip comment syntax (`//`, `/* */`, JSDoc's leading `*`) down to content
/// lines, dropping ones that are blank once stripped.
fn clean_comment(raw: &str) -> Vec<String> {
    let inner = raw
        .strip_prefix("/**")
        .or_else(|| raw.strip_prefix("/*"))
        .and_then(|s| s.strip_suffix("*/"))
        .or_else(|| raw.strip_prefix("//"));

    let Some(inner) = inner else {
        return Vec::new();
    };

    inner
        .lines()
        .map(|line| line.trim().trim_start_matches('*').trim())
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn field_text<'a>(node: Node, field: &str, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name(field).map(|n| text(n, source))
}

fn text<'a>(node: Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or_default()
}

fn node_lines(node: Node) -> [usize; 2] {
    [node.start_position().row + 1, node.end_position().row + 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_declaration_basic() {
        let src = "export function double(x: number): number {\n    return x * 2;\n}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        let s = &symbols[0];
        assert_eq!(s.id, "a.ts::double");
        assert_eq!(s.kind, SymbolKind::Callable);
        assert_eq!(s.raw, "function");
        assert_eq!(s.file, "a.ts");
        assert_eq!(s.lines, [1, 3]);
        assert_eq!(s.signature, "function double(x: number): number");
        assert_eq!(s.docstring, None);
        assert_eq!(s.parent, None);
        assert!(s.children.is_empty());
        assert!(s.is_exported);
        assert!(s.interface_hash.is_some());
        assert!(!s.source_hash.is_empty());
    }

    #[test]
    fn non_exported_declarations_have_no_interface_hash() {
        let src = "function helper(): void {}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        assert!(!symbols[0].is_exported);
        assert_eq!(symbols[0].interface_hash, None);
        // source_hash is still computed — non-exported code still needs a
        // staleness signal, it just can't be a reference-edge target.
        assert!(!symbols[0].source_hash.is_empty());
    }

    #[test]
    fn body_only_edit_changes_source_hash_but_not_interface_hash() {
        // This is the direct regression test for gap 2: rewriting a
        // function's implementation must never invalidate its importers.
        let before = extract_file(
            "export function add(a: number, b: number): number {\n    return a + b;\n}\n",
            "a.ts",
        );
        let after = extract_file(
            "export function add(a: number, b: number): number {\n    let sum = a + b;\n    return sum;\n}\n",
            "a.ts",
        );
        assert_ne!(before[0].source_hash, after[0].source_hash);
        assert_eq!(before[0].interface_hash, after[0].interface_hash);
    }

    #[test]
    fn signature_edit_changes_interface_hash() {
        let before = extract_file(
            "export function add(a: number, b: number): number {\n    return a + b;\n}\n",
            "a.ts",
        );
        let after = extract_file(
            "export function add(a: number, b: number, c: number): number {\n    return a + b;\n}\n",
            "a.ts",
        );
        assert_ne!(before[0].interface_hash, after[0].interface_hash);
        assert_ne!(before[0].source_hash, after[0].source_hash);
    }

    #[test]
    fn jsdoc_block_comment_is_docstring() {
        let src = "/**\n * Doubles a number.\n */\nexport function double(x: number): number {\n    return x * 2;\n}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].docstring.as_deref(), Some("Doubles a number."));
    }

    #[test]
    fn consecutive_line_comments_join_into_docstring() {
        let src = "// Adds two numbers.\n// Simple helper.\nexport function add(a: number, b: number): number {\n  return a + b;\n}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(
            symbols[0].docstring.as_deref(),
            Some("Adds two numbers.\nSimple helper.")
        );
    }

    #[test]
    fn comment_separated_by_blank_line_is_not_attached() {
        let src = "// unrelated\n\nexport function noop(): void {}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].docstring, None);
    }

    #[test]
    fn arrow_function_const_is_a_callable() {
        let src = "export const double = (x: number): number => x * 2;\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        let s = &symbols[0];
        assert_eq!(s.id, "a.ts::double");
        assert_eq!(s.kind, SymbolKind::Callable);
        assert_eq!(s.raw, "function");
        assert_eq!(s.signature, "const double = (x: number): number =>");
    }

    #[test]
    fn plain_const_is_a_value() {
        let src = "export const PI = 3.14;\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1);
        let s = &symbols[0];
        assert_eq!(s.id, "a.ts::PI");
        assert_eq!(s.kind, SymbolKind::Value);
        assert_eq!(s.raw, "const");
        assert_eq!(s.signature, "const PI");
    }

    #[test]
    fn let_declaration_is_ignored() {
        let src = "export let counter = 0;\n";
        let symbols = extract_file(src, "a.ts");
        assert!(symbols.is_empty());
    }

    fn ids(symbols: &[ExtractedSymbol]) -> Vec<&str> {
        symbols.iter().map(|s| s.id.as_str()).collect()
    }

    fn only(src: &str) -> ExtractedSymbol {
        let mut symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 1, "expected one symbol from {src:?}");
        symbols.remove(0)
    }

    // --- Question 21: type-level declarations and destructured exports.
    // An import of one of these names used to resolve to nothing (no
    // symbol to point at), so its importers never went stale.

    #[test]
    fn type_alias_is_a_value_symbol() {
        let s = only("export type Status = 'open' | 'closed';\n");
        assert_eq!(s.id, "a.ts::Status");
        assert_eq!(s.kind, SymbolKind::Value);
        assert_eq!(s.raw, "type");
        assert_eq!(s.signature, "type Status");
        assert!(s.is_exported);
        assert!(s.interface_hash.is_some());
        assert!(s.parent.is_none() && s.children.is_empty());
    }

    #[test]
    fn interface_is_a_value_symbol() {
        let s = only("export interface User {\n  id: string;\n  name: string;\n}\n");
        assert_eq!(s.id, "a.ts::User");
        assert_eq!(s.kind, SymbolKind::Value);
        assert_eq!(s.raw, "interface");
        assert_eq!(s.signature, "interface User");
        assert_eq!(s.lines, [1, 4]);
        assert!(s.is_exported);
        assert!(s.interface_hash.is_some());
    }

    #[test]
    fn interface_signature_keeps_generics_and_extends() {
        let s = only("export interface Box<T> extends Base {\n  value: T;\n}\n");
        assert_eq!(s.signature, "interface Box<T> extends Base");
    }

    #[test]
    fn enum_is_a_value_symbol() {
        let s = only("export enum Role {\n  Admin,\n  User,\n}\n");
        assert_eq!(s.id, "a.ts::Role");
        assert_eq!(s.kind, SymbolKind::Value);
        assert_eq!(s.raw, "enum");
        assert_eq!(s.signature, "enum Role");
        assert!(s.interface_hash.is_some());
    }

    #[test]
    fn const_enum_is_extracted_too() {
        let s = only("export const enum Flag { On, Off }\n");
        assert_eq!(s.id, "a.ts::Flag");
        assert_eq!(s.raw, "enum");
    }

    #[test]
    fn non_exported_type_declarations_have_no_interface_hash() {
        for src in [
            "type T = string;\n",
            "interface T { a: string }\n",
            "enum T { A }\n",
        ] {
            let s = only(src);
            assert!(!s.is_exported, "{src}");
            assert_eq!(s.interface_hash, None, "{src}");
            assert!(!s.source_hash.is_empty(), "{src}");
        }
    }

    #[test]
    fn default_exported_interface_is_extracted_by_name() {
        let s = only("export default interface Props { a: string }\n");
        assert_eq!(s.id, "a.ts::Props");
        assert!(s.is_exported);
    }

    #[test]
    fn a_type_declaration_keeps_its_leading_doc_comment() {
        let s = only("/** A user. */\nexport interface User { id: string }\n");
        assert_eq!(s.docstring.as_deref(), Some("A user."));
    }

    #[test]
    fn interface_member_edits_move_interface_hash() {
        let before = only("export interface User {\n  id: string;\n}\n");
        for after in [
            "export interface User {\n  id: string;\n  email: string;\n}\n",
            "export interface User {\n  id: number;\n}\n",
            "export interface User {\n  uid: string;\n}\n",
            "export interface User {\n  id?: string;\n}\n",
        ] {
            let after = only(after);
            assert_ne!(before.interface_hash, after.interface_hash, "{after:?}");
            assert_ne!(before.source_hash, after.source_hash);
        }
    }

    #[test]
    fn type_alias_and_enum_edits_move_interface_hash() {
        let a = only("export type S = 'open' | 'closed';\n");
        let b = only("export type S = 'open' | 'closed' | 'draft';\n");
        assert_ne!(a.interface_hash, b.interface_hash);
        let a = only("export enum R { A, B }\n");
        let b = only("export enum R { A, B, C }\n");
        assert_ne!(a.interface_hash, b.interface_hash);
    }

    #[test]
    fn comments_inside_a_type_body_do_not_move_interface_hash() {
        let plain = only("export interface User {\n  id: string;\n  name: string;\n}\n");
        for commented in [
            // a new comment line
            "export interface User {\n  // the key\n  id: string;\n  name: string;\n}\n",
            // a trailing comment
            "export interface User {\n  id: string; // the key\n  name: string;\n}\n",
            // a block comment on its own lines
            "export interface User {\n  /** The key. */\n  id: string;\n  name: string;\n}\n",
        ] {
            let c = only(commented);
            assert_eq!(plain.interface_hash, c.interface_hash, "{commented}");
        }
        // The raw source did change, so the symbol's own spec still goes
        // stale; only its importers are spared.
        let c = only("export interface User {\n  // the key\n  id: string;\n  name: string;\n}\n");
        assert_ne!(plain.source_hash, c.source_hash);
    }

    #[test]
    fn editing_an_existing_comment_in_a_type_body_does_not_move_interface_hash() {
        let a = only("export type T = {\n  a: string; // one\n};\n");
        let b = only("export type T = {\n  a: string; // two\n};\n");
        assert_eq!(a.interface_hash, b.interface_hash);
    }

    #[test]
    fn a_comment_marker_inside_a_string_literal_type_is_not_stripped() {
        let a = only("export type U = 'http://a';\n");
        let b = only("export type U = 'http://b';\n");
        assert_ne!(a.interface_hash, b.interface_hash);
    }

    #[test]
    fn destructured_object_const_yields_one_symbol_per_name() {
        let symbols = extract_file("export const { a, b } = getStuff();\n", "a.ts");
        assert_eq!(ids(&symbols), ["a.ts::a", "a.ts::b"]);
        for s in &symbols {
            assert_eq!(s.kind, SymbolKind::Value);
            assert_eq!(s.raw, "const");
            assert!(s.is_exported);
            assert!(s.interface_hash.is_some());
            assert_eq!(s.lines, [1, 1]);
        }
        assert_eq!(symbols[0].signature, "const a");
        assert_eq!(symbols[1].signature, "const b");
    }

    #[test]
    fn destructured_const_binds_the_local_name_not_the_property_key() {
        let symbols = extract_file("export const { a: renamed, b = 1 } = x;\n", "a.ts");
        assert_eq!(ids(&symbols), ["a.ts::renamed", "a.ts::b"]);
    }

    #[test]
    fn destructured_const_handles_nested_rest_and_array_patterns() {
        let symbols = extract_file(
            "export const { a: { b }, ...rest } = x;\nexport const [first, , third, ...others] = y;\n",
            "a.ts",
        );
        assert_eq!(
            ids(&symbols),
            [
                "a.ts::b",
                "a.ts::rest",
                "a.ts::first",
                "a.ts::third",
                "a.ts::others"
            ]
        );
    }

    #[test]
    fn destructured_const_that_is_not_exported_has_no_interface_hash() {
        let symbols = extract_file("const { a } = x;\n", "a.ts");
        assert_eq!(ids(&symbols), ["a.ts::a"]);
        assert!(!symbols[0].is_exported);
        assert_eq!(symbols[0].interface_hash, None);
    }

    #[test]
    fn destructured_names_share_a_declaration_but_not_an_id() {
        let src = "export const { a, b } = f(1);\n";
        let one = extract_file(src, "a.ts");
        let two = extract_file("export const { a, b } = f(2);\n", "a.ts");
        // A value edit moves each name's source_hash, never its
        // interface_hash -- the same rule a plain `const` follows.
        assert_ne!(one[0].source_hash, two[0].source_hash);
        assert_eq!(one[0].interface_hash, two[0].interface_hash);
        assert_eq!(one[1].interface_hash, two[1].interface_hash);
    }

    #[test]
    fn a_const_and_a_type_of_the_same_name_fold_into_one_symbol() {
        // The common `as const` + derived-type pattern. Two symbols with
        // one id would overwrite each other in the graph's id map.
        let src = "export const Status = { Open: 'open' } as const;\nexport type Status = typeof Status[keyof typeof Status];\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(ids(&symbols), ["a.ts::Status"]);
        let s = &symbols[0];
        assert!(s.is_exported);
        assert_eq!(s.extra_spans, [[2, 2]]);

        // Either half changing moves the folded symbol's hashes.
        let edited_type = extract_file(
            "export const Status = { Open: 'open' } as const;\nexport type Status = keyof typeof Status;\n",
            "a.ts",
        );
        assert_ne!(s.interface_hash, edited_type[0].interface_hash);
        assert_ne!(s.source_hash, edited_type[0].source_hash);
    }

    #[test]
    fn two_interfaces_of_the_same_name_merge_into_one_symbol() {
        let src = "export interface W { a: string }\nexport interface W { b: string }\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(ids(&symbols), ["a.ts::W"]);
        assert_eq!(symbols[0].extra_spans, [[2, 2]]);
        let one = only("export interface W { a: string }\n");
        assert_ne!(one.interface_hash, symbols[0].interface_hash);
    }

    #[test]
    fn a_fold_is_exported_if_any_declaration_is() {
        let symbols = extract_file(
            "interface W { a: string }\nexport interface W { b: string }\n",
            "a.ts",
        );
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].is_exported);
        assert!(symbols[0].interface_hash.is_some());
    }

    // --- A declaration exported by a bare `export { … }` clause, not by an
    // `export` keyword on itself (the shadcn `function Button() {}` …
    // `export { Button }` shape). It is exported just the same, and without
    // an `interface_hash` its importers hash its whole source instead.

    #[test]
    fn a_declaration_exported_by_a_clause_is_exported_with_an_interface_hash() {
        for (src, id) in [
            ("function f(a: number): void {}\nexport { f };\n", "a.ts::f"),
            ("class C { m(): void {} }\nexport { C };\n", "a.ts::C"),
            ("const V = 1;\nexport { V };\n", "a.ts::V"),
            (
                "const g = (a: number): void => {};\nexport { g };\n",
                "a.ts::g",
            ),
            ("type T = string;\nexport { T };\n", "a.ts::T"),
            ("interface I { a: string }\nexport type { I };\n", "a.ts::I"),
            ("enum E { A }\nexport { E };\n", "a.ts::E"),
        ] {
            let symbols = extract_file(src, "a.ts");
            let s = symbols
                .iter()
                .find(|s| s.id == id)
                .unwrap_or_else(|| panic!("{id}"));
            assert!(s.is_exported, "{src}");
            assert!(s.interface_hash.is_some(), "{src}");
        }
    }

    #[test]
    fn a_clause_export_hashes_like_the_same_declaration_with_an_export_keyword() {
        for (clause, keyword) in [
            (
                "function f(a: number): void {}\nexport { f };\n",
                "export function f(a: number): void {}\n",
            ),
            (
                "class C { m(): void {} }\nexport { C };\n",
                "export class C { m(): void {} }\n",
            ),
            (
                "const V: number = 1;\nexport { V };\n",
                "export const V: number = 1;\n",
            ),
            (
                "type T = string;\nexport { T };\n",
                "export type T = string;\n",
            ),
        ] {
            let a = extract_file(clause, "a.ts");
            let b = extract_file(keyword, "a.ts");
            assert_eq!(a[0].interface_hash, b[0].interface_hash, "{clause}");
        }
    }

    #[test]
    fn a_body_only_edit_to_a_clause_exported_function_does_not_move_interface_hash() {
        let before = only("function f(a: number): number { return a; }\nexport { f };\n");
        let body = only("function f(a: number): number { return a + 1; }\nexport { f };\n");
        let signature =
            only("function f(a: number, b: number): number { return a; }\nexport { f };\n");
        assert_ne!(before.source_hash, body.source_hash);
        assert_eq!(before.interface_hash, body.interface_hash);
        assert_ne!(before.interface_hash, signature.interface_hash);
    }

    #[test]
    fn only_the_names_in_the_clause_are_exported() {
        let symbols = extract_file(
            "function a(): void {}\nfunction b(): void {}\nfunction c(): void {}\nexport {\n  a,\n  c,\n};\n",
            "a.ts",
        );
        let exported: Vec<_> = symbols
            .iter()
            .map(|s| (s.id.as_str(), s.is_exported))
            .collect();
        assert_eq!(
            exported,
            [("a.ts::a", true), ("a.ts::b", false), ("a.ts::c", true)]
        );
        assert_eq!(symbols[1].interface_hash, None);
    }

    #[test]
    fn a_clause_before_the_declaration_still_exports_it() {
        let s = only("export { f };\nfunction f(): void {}\n");
        assert!(s.is_exported);
        assert!(s.interface_hash.is_some());
    }

    #[test]
    fn a_reexport_clause_with_a_source_exports_nothing_local() {
        // `export { a } from './x'` forwards *another module's* `a`; a local
        // `a` of the same name stays private.
        let s = only("function a(): void {}\nexport { a } from './x';\n");
        assert!(!s.is_exported);
        assert_eq!(s.interface_hash, None);
    }

    #[test]
    fn an_alias_equal_to_the_name_is_not_a_rename() {
        // `export { a as a }` publishes `a` under its own name.
        let s = only("function a(): void {}\nexport { a as a };\n");
        assert!(s.is_exported);
        assert!(s.interface_hash.is_some());
    }

    #[test]
    fn a_renamed_clause_export_is_not_treated_as_an_export_of_the_local_name() {
        // Deliberately not handled: importers would ask for `b`, which is
        // not this symbol's id, so marking `a` exported would change
        // nothing for them (zero cases in the repos measured).
        let s = only("function a(): void {}\nexport { a as b };\n");
        assert!(!s.is_exported);
    }

    // --- Code-review findings on the above.

    #[test]
    fn an_interface_and_a_class_of_the_same_name_keep_the_class_as_a_container() {
        // Declaration merging. Whichever order they are written in, the
        // folded symbol must stay a `Container`: a `Value` would make the
        // file stop being spec-bearing and the class would lose its spec.
        for src in [
            "export interface Foo { a: string }\nexport class Foo { m(): void {} }\n",
            "export class Foo { m(): void {} }\nexport interface Foo { a: string }\n",
        ] {
            let symbols = extract_file(src, "a.ts");
            let foo = symbols.iter().find(|s| s.id == "a.ts::Foo").unwrap();
            assert_eq!(foo.kind, SymbolKind::Container, "{src}");
            assert_eq!(foo.raw, "class", "{src}");
            assert_eq!(foo.children, ["a.ts::Foo.m"], "{src}");
            assert_eq!(foo.extra_spans.len(), 1, "{src}");
            assert_eq!(
                ids(&symbols).iter().filter(|i| **i == "a.ts::Foo").count(),
                1
            );
        }
    }

    #[test]
    fn a_fold_keeps_a_doc_comment_from_whichever_declaration_has_one() {
        let s =
            only("interface W { a: string }\n/** Merged. */\nexport interface W { b: string }\n");
        assert_eq!(s.docstring.as_deref(), Some("Merged."));
    }

    #[test]
    fn a_private_half_of_a_folded_type_still_moves_the_interface_hash() {
        let before = only("interface W { a: string }\nexport interface W { b: string }\n");
        let after = only("interface W { a: number }\nexport interface W { b: string }\n");
        assert_ne!(before.interface_hash, after.interface_hash);
    }

    #[test]
    fn reformatting_a_type_does_not_move_interface_hash() {
        let compact = only("export interface U { id: string; tags: string[]; }\n");
        let spread = only("export interface U {\n    id: string;\n    tags: string[];\n}\n");
        let indented = only("export interface U {\n\tid: string;\n\n\ttags:\n\t\tstring[];\n}\n");
        assert_eq!(compact.interface_hash, spread.interface_hash);
        assert_eq!(compact.interface_hash, indented.interface_hash);
        // A real edit still does.
        let edited = only("export interface U { id: string; tags: number[] }\n");
        assert_ne!(compact.interface_hash, edited.interface_hash);
    }

    #[test]
    fn a_comment_after_the_equals_sign_does_not_move_interface_hash() {
        let plain = only("export type A = 'x';\n");
        let commented = only("export type A = // legacy\n  'x';\n");
        assert_eq!(plain.interface_hash, commented.interface_hash);
    }

    #[test]
    fn a_type_alias_signature_is_just_its_header() {
        assert_eq!(
            only("export type A = /* legacy */ 'x' | 'y';\n").signature,
            "type A"
        );
        assert_eq!(
            only("export type Box<T = {}> = { v: T };\n").signature,
            "type Box<T = {}>"
        );
    }

    #[test]
    fn folding_does_not_touch_a_symbol_with_a_unique_name() {
        let symbols = extract_file(
            "export interface A { a: string }\nexport type B = number;\n",
            "a.ts",
        );
        assert_eq!(ids(&symbols), ["a.ts::A", "a.ts::B"]);
        assert!(symbols.iter().all(|s| s.extra_spans.is_empty()));
    }

    #[test]
    fn barrel_file_yields_no_symbols() {
        let src = "export { Foo } from './foo';\nexport * from './bar';\n";
        let symbols = extract_file(src, "a.ts");
        assert!(symbols.is_empty());
    }

    #[test]
    fn a_method_colliding_with_a_field_of_the_same_name_gets_a_distinct_id() {
        // tree-sitter parses this regardless of whether tsc would accept
        // it -- a syntax tree, not a type checker -- so CodeOwl still
        // needs to index it without corrupting the graph.
        let src = "export class Config {\n    validate: boolean = false;\n    validate() { return this.validate; }\n}\n";
        let syms = extract_file(src, "a.ts");
        let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            ids.len(),
            "a real id collision survived: {ids:?}"
        );

        let field = syms
            .iter()
            .find(|s| s.kind == SymbolKind::Value)
            .expect("the field");
        assert_eq!(field.id, "a.ts::Config.validate");

        let method = syms
            .iter()
            .find(|s| s.kind == SymbolKind::Callable)
            .expect("the method");
        assert_ne!(method.id, field.id);
        assert!(method.id.starts_with("a.ts::Config.validate#"));

        let class = &syms[0];
        assert_eq!(class.children.len(), 2);
        assert_ne!(class.children[0], class.children[1]);
    }

    #[test]
    fn class_with_methods_builds_containment_tree() {
        let src = "export class Foo<T> extends Bar implements Baz {\n    /** ctor doc */\n    constructor(private x: number) {}\n\n    // plain method\n    async doThing(y: T): Promise<void> {\n        console.log(y);\n    }\n}\n";
        let symbols = extract_file(src, "a.ts");
        assert_eq!(symbols.len(), 3);

        let class = &symbols[0];
        assert_eq!(class.id, "a.ts::Foo");
        assert_eq!(class.kind, SymbolKind::Container);
        assert_eq!(class.raw, "class");
        assert_eq!(class.signature, "class Foo<T> extends Bar implements Baz");
        assert_eq!(class.parent, None);
        assert_eq!(
            class.children,
            vec!["a.ts::Foo.constructor", "a.ts::Foo.doThing"]
        );
        assert!(class.is_exported);
        assert!(class.interface_hash.is_some());

        let ctor = &symbols[1];
        assert_eq!(ctor.id, "a.ts::Foo.constructor");
        assert_eq!(ctor.kind, SymbolKind::Callable);
        assert_eq!(ctor.raw, "method");
        assert_eq!(ctor.parent.as_deref(), Some("a.ts::Foo"));
        assert_eq!(ctor.docstring.as_deref(), Some("ctor doc"));
        // Methods are never independently exported — see the doc comment
        // on Symbol::is_exported.
        assert!(!ctor.is_exported);
        assert_eq!(ctor.interface_hash, None);

        let method = &symbols[2];
        assert_eq!(method.id, "a.ts::Foo.doThing");
        assert_eq!(method.kind, SymbolKind::Callable);
        assert_eq!(method.raw, "method");
        assert_eq!(method.parent.as_deref(), Some("a.ts::Foo"));
        assert_eq!(method.docstring.as_deref(), Some("plain method"));
        assert!(
            method
                .signature
                .starts_with("async doThing(y: T): Promise<void>")
        );
    }

    #[test]
    fn a_public_by_default_fields_type_change_moves_the_classs_interface_hash() {
        // TS default visibility is public -- no keyword needed, unlike
        // Rust's `pub`-required convention.
        let a = extract_file("export class P {\n    x: number;\n}\n", "a.ts");
        let b = extract_file("export class P {\n    x: string;\n}\n", "a.ts");
        assert_ne!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_private_keyword_fields_type_change_does_not_move_interface_hash() {
        let a = extract_file("export class P {\n    private x: number;\n}\n", "a.ts");
        let b = extract_file("export class P {\n    private x: string;\n}\n", "a.ts");
        assert_eq!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_js_hash_private_fields_type_change_does_not_move_interface_hash() {
        // `#priv` needs no `private` keyword -- the `#` sigil alone makes
        // it JS-private, and the signature text starts with it directly.
        let a = extract_file("export class P {\n    #x: number;\n}\n", "a.ts");
        let b = extract_file("export class P {\n    #x: string;\n}\n", "a.ts");
        assert_eq!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_fields_docstring_change_does_not_move_interface_hash() {
        let a = extract_file(
            "export class P {\n    /** old doc */\n    x: number;\n}\n",
            "a.ts",
        );
        let b = extract_file(
            "export class P {\n    /** totally different */\n    x: number;\n}\n",
            "a.ts",
        );
        assert_eq!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn class_fields_are_extracted_as_value_members() {
        let src = "export class FileNode {\n    /** Repo-relative path. */\n    id: string;\n    /** Hash of the file's raw text. */\n    private source_hash: string = \"\";\n    method() {}\n}\n";
        let symbols = extract_file(src, "a.ts");
        let class = &symbols[0];
        assert_eq!(
            class.children,
            vec![
                "a.ts::FileNode.id",
                "a.ts::FileNode.source_hash",
                "a.ts::FileNode.method"
            ]
        );

        let id_field = symbols
            .iter()
            .find(|s| s.id == "a.ts::FileNode.id")
            .unwrap();
        assert_eq!(id_field.kind, SymbolKind::Value);
        assert_eq!(id_field.raw, "property");
        assert_eq!(id_field.parent.as_deref(), Some("a.ts::FileNode"));
        assert_eq!(id_field.docstring.as_deref(), Some("Repo-relative path."));
        assert_eq!(id_field.signature, "id: string");
        // A member is never independently imported -- same stance as a
        // method (see Symbol::is_exported's own doc comment).
        assert!(!id_field.is_exported);
        assert_eq!(id_field.interface_hash, None);

        let hash_field = symbols
            .iter()
            .find(|s| s.id == "a.ts::FileNode.source_hash")
            .unwrap();
        assert_eq!(hash_field.kind, SymbolKind::Value);
        // The initializer is stripped from the signature, same as
        // visit_lexical already does for a plain top-level const.
        assert_eq!(hash_field.signature, "private source_hash: string");
    }

    #[test]
    fn field_reorder_moves_the_classs_source_hash_but_unrelated_edit_does_not() {
        let a = extract_file("class P {\n    x = 1;\n    y = 2;\n}\n", "a.ts");
        let reordered = extract_file("class P {\n    y = 2;\n    x = 1;\n}\n", "a.ts");
        assert_ne!(a[0].source_hash, reordered[0].source_hash);

        let unrelated_edit = extract_file(
            "// a comment that doesn't touch the class\nclass P {\n    x = 1;\n    y = 2;\n}\n",
            "a.ts",
        );
        assert_eq!(a[0].source_hash, unrelated_edit[0].source_hash);
    }

    #[test]
    fn method_body_edit_propagates_source_hash_to_class_but_not_interface_hash() {
        let before = extract_file(
            "export class Foo {\n    bar(): void {\n        console.log('a');\n    }\n}\n",
            "a.ts",
        );
        let after = extract_file(
            "export class Foo {\n    bar(): void {\n        console.log('b');\n    }\n}\n",
            "a.ts",
        );
        let (before_method, before_class) = (&before[1], &before[0]);
        let (after_method, after_class) = (&after[1], &after[0]);

        // The ancestor chain: a leaf method's source_hash changes...
        assert_ne!(before_method.source_hash, after_method.source_hash);
        // ...and that propagates up to the containing class's source_hash...
        assert_ne!(before_class.source_hash, after_class.source_hash);
        // ...but the class's *interface* didn't change (M2 doesn't yet
        // fold method signatures into a class's interface_hash — see the
        // doc comment on visit_class's interface_hash computation).
        assert_eq!(before_class.interface_hash, after_class.interface_hash);
    }

    #[test]
    fn tsx_file_parses_with_jsx_grammar() {
        let src = "export const Widget = ({ label }: { label: string }) => {\n    return <span>{label}</span>;\n};\n";
        let symbols = extract_file(src, "a.tsx");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Callable);
        assert_eq!(symbols[0].raw, "function");
    }
}
