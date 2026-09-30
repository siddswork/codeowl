//! Walks a single Rust source file with tree-sitter and pulls out its
//! top-level items — the Rust pack's counterpart to `extract.rs` (M14).
//!
//! Deliberately shallow, like `extract.rs`: `source_file`'s direct
//! children, plus one level into `impl` / `trait` / inline `mod` / `struct`
//! bodies for the callables and (M21) named fields they contain. A struct's
//! *named* fields become `Value` members (`field_declaration_list`); a
//! tuple struct's positional fields (`ordered_field_declaration_list`, no
//! `name` node in the grammar at all) are not yet extracted — see
//! `visit_item`'s own comment. Function bodies, enum variants, and nested
//! `fn`s remain implementation detail, not declarations CodeOwl generates
//! a spec for.
//!
//! Kind mapping onto the stack-neutral [`SymbolKind`] (M14 / design
//! decision 1): `fn` -> `Callable`, `struct`/`enum`/`union`/`trait`/`impl`/
//! `mod` -> `Container`, `const`/`static` -> `Value`, `macro_rules!` ->
//! `Callable`. The concrete keyword lands in `raw`.
//!
//! An **inherent `impl Foo` block is folded into `Foo`'s own symbol** after
//! the walk (M15 / `merge_inherent_impls`): Rust splits a type's data
//! (`struct Foo`) from its methods (`impl Foo`) as a matter of syntax, but a
//! spec reader wants one `Foo`, the same containment unit a TS/Java `class`
//! already is. Trait impls (`impl Trait for Foo`) stay their own symbols —
//! "how `Foo` satisfies `Trait`" is a distinct concern, and other languages
//! express it separately too (`class Foo implements Trait`).

use tree_sitter::{Node, Parser};

use crate::hash::hash_text;
use crate::symbol::{ExtractedSymbol, SymbolKind};
use crate::text::strip_value_for_fold;

/// Parse `source` (the contents of `rel_path`, a `.rs` file) and extract
/// its top-level items plus one level of `impl`/`trait`/`mod` members.
pub fn extract_file(source: &str, rel_path: &str) -> Vec<ExtractedSymbol> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("bundled tree-sitter-rust grammar should always load");

    // `tree` owns the arena every `Node<'_>` below borrows from — nothing
    // escapes this function (CLAUDE.md's "don't let Node<'a> escape" rule).
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        visit_item(child, None, source, rel_path, &mut out);
    }
    let opaque_types = top_level_types_with_unextracted_fields(root, source, rel_path);
    merge_inherent_impls(out, &opaque_types)
}

/// Top-level struct ids whose real field data isn't reflected in
/// `children` at all — specifically a tuple struct's positional fields
/// (`ordered_field_declaration_list`, deliberately not extracted as
/// members yet — see `tuple_struct_fields_are_not_yet_extracted_as_members`
/// below). Code-review finding on M21.e: `merge_inherent_impls`'s "zero
/// fields" check via `children.is_empty()` can't otherwise tell a genuine
/// zero-field marker type from `struct UserId(u64);`, whose one real field
/// this extractor just doesn't capture as a member — scanned separately,
/// over `root.children()` only, since `merge_inherent_impls` only ever
/// considers top-level types in the first place.
fn top_level_types_with_unextracted_fields(
    root: Node,
    source: &str,
    file: &str,
) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() != "struct_item" {
            continue;
        }
        let Some(body) = child.child_by_field_name("body") else {
            continue;
        };
        if body.kind() == "ordered_field_declaration_list"
            && body.named_child_count() > 0
            && let Some(name) = field_text(child, "name", source)
        {
            out.insert(format!("{file}::{name}"));
        }
    }
    out
}

/// The last `::`-separated segment of a stable id — `a.rs::Counter` ->
/// `Counter`, `a.rs::impl Counter::new` -> `new`. Only sound for the ids
/// `merge_inherent_impls` actually inspects (top-level types and the
/// methods of an *inherent* impl); a trait-impl id like
/// `a.rs::impl Debug for S` is never passed here.
fn short_id(id: &str) -> &str {
    id.rsplit("::").next().unwrap_or(id)
}

/// Fold every inherent `impl Foo { … }` block into the `Foo` symbol this
/// file defines: reparent its methods onto `Foo` (rewriting their ids from
/// `<file>::impl Foo::m` to `<file>::Foo::m`), Merkle-fold the block's
/// `source_hash` into `Foo`'s, union the block's attribute `markers`, then
/// drop the block. A type's several inherent impls all collapse into it, in
/// source order. Impls on a type this file doesn't define are left
/// untouched.
///
/// **A trait impl (`impl Trait for Foo`) also folds, but only in one
/// narrow, mechanical shape** (M21.e, `ARCHITECTURE.md` open question 16's
/// degenerate case — the real trigger was `stack.rs::JavaStack`, whose
/// entire content was one `impl StackPack for JavaStack` on a zero-field
/// marker struct): `Foo` has zero fields of its own, zero inherent-impl
/// methods (not itself a target above), and this is the *only* trait impl
/// targeting it in the file. In that shape there is nothing left for a
/// standalone `Foo` document to say beyond a guess at the very behavior
/// the trait impl already states, and — since Rust's grammar gives the
/// two symbols no edge between them at all — no other target invalidates a
/// bare `Foo` document if the trait impl's own behavior later changes.
/// Any other shape (real fields, an inherent impl, or more than one trait
/// impl) leaves every trait impl in its own section, unchanged — that
/// general case is deliberately left open, not decided here.
///
/// **Known limitation, logged rather than built (M21.e code review):**
/// "the only trait impl targeting it" is checked *within this file only*
/// — `merge_inherent_impls` runs per-file, on one file's own
/// `Vec<ExtractedSymbol>`, with no visibility into any other file. A type
/// with a second trait impl living in a *different* file (a real, if
/// uncommon, Rust organization style — impls split into their own module
/// per trait) is invisible to `trait_target_counts` here, so that type
/// still folds as if it had exactly one, silently reintroducing the exact
/// unlinked-node gap this fold exists to close, just for the impl this
/// file can't see. Closing it for real needs a whole-graph second pass
/// after every file is extracted, not a per-file check — deliberately not
/// built ahead of a real repo actually hitting it, the same "wait for
/// evidence" policy this project applies elsewhere (open questions 8, 10,
/// 11's remaining manifestations).
fn merge_inherent_impls(
    syms: Vec<ExtractedSymbol>,
    opaque_types: &std::collections::HashSet<String>,
) -> Vec<ExtractedSymbol> {
    use std::collections::{HashMap, HashSet};

    // Short name -> index of a top-level type this file defines.
    let type_ix: HashMap<&str, usize> = syms
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.parent.is_none() && matches!(s.raw.as_str(), "struct" | "enum" | "union")
        })
        .map(|(i, s)| (short_id(&s.id), i))
        .collect();

    // Inherent-impl block index -> the index of the type it merges into.
    // Unconditional, as always.
    let mut merge_into: HashMap<usize, usize> = syms
        .iter()
        .enumerate()
        .filter(|(_, s)| s.parent.is_none() && s.raw == "impl")
        .filter_map(|(i, s)| {
            let target = inherent_impl_target(&s.signature)?;
            Some((i, *type_ix.get(target.as_str())?))
        })
        .collect();

    // Trait-impl block index -> its target type index, for every trait
    // impl on a locally-defined type. Collected separately: eligibility
    // here is conditional on the whole file's shape, not unconditional the
    // way an inherent impl's fold is.
    let trait_targets: Vec<(usize, usize)> = syms
        .iter()
        .enumerate()
        .filter(|(_, s)| s.parent.is_none() && s.raw == "impl")
        .filter_map(|(i, s)| {
            let target = trait_impl_target(&s.signature)?;
            Some((i, *type_ix.get(target.as_str())?))
        })
        .collect();

    let mut trait_target_counts: HashMap<usize, usize> = HashMap::new();
    for &(_, ti) in &trait_targets {
        *trait_target_counts.entry(ti).or_insert(0) += 1;
    }
    let inherent_targets: HashSet<usize> = merge_into.values().copied().collect();
    for (i, ti) in trait_targets {
        let is_degenerate = !inherent_targets.contains(&ti)
            && syms[ti].children.is_empty()
            && !opaque_types.contains(&syms[ti].id)
            && trait_target_counts.get(&ti) == Some(&1);
        if is_degenerate {
            merge_into.insert(i, ti);
        }
    }

    if merge_into.is_empty() {
        return syms;
    }

    // Per target type: what its inherent impl blocks contribute — the
    // reparented method symbols, the block `source_hash`es to Merkle-fold
    // in, and the attribute markers to union. Accumulated in source order
    // (we sweep `syms` front to back).
    #[derive(Default)]
    struct Folded {
        methods: Vec<ExtractedSymbol>,
        block_hashes: Vec<String>,
        markers: Vec<String>,
        // M21.i: the signatures that fold into the type's interface_hash,
        // decided per block while the block's own kind is still known --
        // an inherent impl's `pub` methods, or every method of a trait
        // impl folded here by the degenerate-case rule above (a trait
        // impl's methods never carry `pub`; they're public because the
        // trait is).
        public_sigs: Vec<String>,
        // Each folded block's own full line range, so the `impl ... {`
        // header (the only line naming the trait, or an inherent impl's
        // generic bounds) stays visible to dependency scoping and
        // `get_source` after the block symbol itself is dropped.
        block_spans: Vec<[usize; 2]>,
    }
    let mut folded: HashMap<usize, Folded> = HashMap::new();
    let mut dropped = vec![false; syms.len()];

    for i in 0..syms.len() {
        let Some(&ti) = merge_into.get(&i) else {
            continue;
        };
        dropped[i] = true;
        let block_id = syms[i].id.clone();
        let type_id = syms[ti].id.clone();
        let block_hash = syms[i].source_hash.clone();
        let block_markers = syms[i].markers.clone();

        // Ids already claimed under this type -- its own pre-merge members
        // (struct fields, from visit_container's earlier pass) plus every
        // method already reparented onto it by an earlier impl block in
        // this same sweep (`several_inherent_impls_all_fold_into_the_one_type`).
        // Code-review finding: Rust allows a field and a method to share a
        // bare name (`self.x` vs `self.x()` — a `pub validate: bool` field
        // alongside `fn validate(&self)` is real, valid, idiomatic getter
        // code, not a hypothetical), and reparenting a method onto the
        // same id a field already has would silently overwrite the
        // field's arena slot in Graph::build's by_id map -- confirmed via
        // a reproduction before this fix existed.
        let mut claimed: std::collections::HashSet<String> =
            syms[ti].children.iter().cloned().collect();
        if let Some(already) = folded.get(&ti) {
            claimed.extend(already.methods.iter().map(|m| m.id.clone()));
        }

        // This block's methods are the contiguous run right after it whose
        // `parent` is the block (`visit_container` inserts the container,
        // then its members follow directly).
        let mut methods = Vec::new();
        for (j, m) in syms.iter().enumerate().skip(i + 1) {
            if m.parent.as_deref() != Some(block_id.as_str()) {
                break;
            }
            dropped[j] = true;
            let mut method = m.clone();
            let mut candidate = format!("{type_id}::{}", short_id(&m.id));
            if claimed.contains(&candidate) {
                // Disambiguate deterministically rather than collide --
                // the field (or an earlier-merged method) keeps the plain
                // name, since that's the id scheme's pre-existing,
                // already-relied-upon contract; the newly reparented
                // method backs off.
                let mut n = 2;
                loop {
                    let next = format!("{candidate}#{n}");
                    if !claimed.contains(&next) {
                        candidate = next;
                        break;
                    }
                    n += 1;
                }
            }
            claimed.insert(candidate.clone());
            method.id = candidate;
            method.parent = Some(type_id.clone());
            methods.push(method);
        }

        let is_trait_impl = trait_impl_target(&syms[i].signature).is_some();
        let entry = folded.entry(ti).or_default();
        entry.public_sigs.extend(
            methods
                .iter()
                .filter(|m| is_trait_impl || is_pub_field_signature(&m.signature))
                .map(|m| m.signature.clone()),
        );
        entry.methods.extend(methods);
        entry.block_hashes.push(block_hash);
        entry.block_spans.push(syms[i].lines);
        entry.markers.extend(block_markers);
    }

    let mut out = Vec::with_capacity(syms.len());
    for (i, s) in syms.into_iter().enumerate() {
        if dropped[i] {
            continue;
        }
        let Some(f) = folded.remove(&i) else {
            out.push(s);
            continue;
        };
        let mut ty = s;
        let mut rollup = ty.source_hash.clone();
        for h in &f.block_hashes {
            rollup.push('\n');
            rollup.push_str(h);
        }
        ty.source_hash = hash_text(&rollup);
        // M21.i: a merged method's public signature is the type's own
        // public surface too, folded into `interface_hash` the same
        // self-chaining way `source_hash` just was above -- these methods
        // don't exist yet when `visit_container` first computes the type's
        // `interface_hash` (they're still a separate impl block's members
        // at that point), so they can only be folded in here, once merged.
        // Which ones count was decided per block above (`public_sigs`).
        // `None` stays `None`: an unexported type has no public surface to
        // extend regardless of what its methods are. No
        // `strip_value_for_fold` -- Rust has no default-parameter-value
        // syntax to strip, and no normalization beyond that (open question
        // 13's own lean).
        if let Some(existing) = ty.interface_hash.take() {
            let mut iface_rollup = existing;
            for sig in &f.public_sigs {
                iface_rollup.push('\n');
                iface_rollup.push_str(sig);
            }
            ty.interface_hash = Some(hash_text(&iface_rollup));
        }
        for marker in f.markers {
            if !ty.markers.contains(&marker) {
                ty.markers.push(marker);
            }
        }
        ty.children.extend(f.methods.iter().map(|m| m.id.clone()));
        ty.extra_spans.extend(f.block_spans);
        out.push(ty);
        out.extend(f.methods);
    }
    out
}

/// Handle one item. `parent_id` is the container this item is nested in
/// (an `impl`/`trait`/`mod`), or `None` at file top level.
fn visit_item(
    node: Node,
    parent_id: Option<&str>,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    match node.kind() {
        "function_item" => {
            let raw = if parent_id.is_some() { "method" } else { "fn" };
            push_leaf(
                node,
                SymbolKind::Callable,
                raw,
                parent_id,
                source,
                file,
                out,
            );
        }
        "function_signature_item" => {
            // A trait method with no default body — still part of the
            // trait's surface.
            push_leaf(
                node,
                SymbolKind::Callable,
                "method",
                parent_id,
                source,
                file,
                out,
            );
        }
        "struct_item" => {
            let name = field_text(node, "name", source)
                .unwrap_or("<struct>")
                .to_string();
            visit_container(node, "struct", name, parent_id, source, file, out)
        }
        // One symbol per named field. Deliberately not a tuple struct's
        // positional fields (`ordered_field_declaration_list`'s children
        // have no `name` field at all in the grammar) -- out of scope for
        // this pass, falls through to the catch-all below and is skipped
        // gracefully rather than guessed at ahead of a real repo needing it.
        "field_declaration" => push_leaf(
            node,
            SymbolKind::Value,
            "field",
            parent_id,
            source,
            file,
            out,
        ),
        "enum_item" => push_leaf(
            node,
            SymbolKind::Container,
            "enum",
            parent_id,
            source,
            file,
            out,
        ),
        "union_item" => push_leaf(
            node,
            SymbolKind::Container,
            "union",
            parent_id,
            source,
            file,
            out,
        ),
        "const_item" => push_leaf(
            node,
            SymbolKind::Value,
            "const",
            parent_id,
            source,
            file,
            out,
        ),
        "static_item" => push_leaf(
            node,
            SymbolKind::Value,
            "static",
            parent_id,
            source,
            file,
            out,
        ),
        "type_item" => push_leaf(
            node,
            SymbolKind::Value,
            "type",
            parent_id,
            source,
            file,
            out,
        ),
        "macro_definition" => push_leaf(
            node,
            SymbolKind::Callable,
            "macro_rules",
            parent_id,
            source,
            file,
            out,
        ),
        "trait_item" => visit_container(
            node,
            "trait",
            trait_name(node, source),
            parent_id,
            source,
            file,
            out,
        ),
        "impl_item" => {
            let name = impl_header(node, source);
            visit_container(node, "impl", name, parent_id, source, file, out)
        }
        "mod_item" if node.child_by_field_name("body").is_some() && !is_cfg_test(node, source) => {
            let name = field_text(node, "name", source)
                .unwrap_or("<mod>")
                .to_string();
            visit_container(node, "mod", name, parent_id, source, file, out)
        }
        _ => {}
    }
}

/// A `#[cfg(test)] mod tests { … }` block — unit tests colocated in a
/// source file. Skipped entirely (the whole subtree), the same call
/// `classify` makes for a `tests/` file: test code isn't spec-bearing and
/// shouldn't pad a domain file's symbol list.
fn is_cfg_test(node: Node, source: &str) -> bool {
    attributes(node, source)
        .iter()
        .any(|a| a.replace(' ', "").contains("cfg(test)"))
}

/// A `Container` with a body — recurse one level for the callables (and,
/// as of M21, the fields) it holds, then Merkle-fold their `source_hash`es
/// into the container's own (an edit to any member is an edit to the
/// container), matching how `extract.rs` treats a class.
fn visit_container(
    node: Node,
    raw: &str,
    name: String,
    parent_id: Option<&str>,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    let id = match parent_id {
        Some(p) => format!("{p}::{name}"),
        None => format!("{file}::{name}"),
    };
    let before = out.len();

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            visit_item(child, Some(&id), source, file, out);
        }
    }

    // Direct children only — a nested container recursed above has
    // already pushed its own members into this same flat `out` range, so
    // filtering by `parent == id` here (not just "anything after
    // `before`") avoids treating a grandchild as this container's own
    // direct member. Matches `java.rs::visit_container`'s identical guard.
    let direct: Vec<&ExtractedSymbol> = out[before..]
        .iter()
        .filter(|s| s.parent.as_deref() == Some(id.as_str()))
        .collect();
    let member_ids: Vec<String> = direct.iter().map(|s| s.id.clone()).collect();
    let sig = signature_before_body(node, source);
    let mut rollup = sig.clone();
    for s in &direct {
        rollup.push('\n');
        rollup.push_str(&s.source_hash);
    }

    // M21.b: interface_hash folds in each *public* Value member's own
    // signature, value stripped (name+type is public surface, a value
    // assignment isn't). Methods stay excluded, unchanged -- this is
    // specifically about fields/consts being public surface the same
    // way a method's own params/return type already are. A private
    // member never contributes (not public surface, under either
    // regime). `SymbolKind::Value` covers struct fields (raw "field", no
    // inline value syntax at all), const/static items nested in this
    // container (raw "const"/"static", which do carry a value) --
    // strip_value_for_fold handles both uniformly, a no-op for a field's
    // already-value-free text -- and type aliases (raw "type"), which
    // are the one real exception: code-review finding, a type alias's
    // `= Target` is not a discardable value the way a const's is, it IS
    // the alias's entire public meaning, so it's never stripped.
    let is_exported = has_pub(node, source);
    // Skip building the fold at all for a non-exported container
    // (code-review finding: it was computed unconditionally, then
    // discarded by `.then()` below) -- wasted allocation and iteration
    // on every reparse otherwise, and the watcher reparses on every save.
    let mut iface_rollup = sig.clone();
    if is_exported {
        for s in &direct {
            match s.kind {
                SymbolKind::Value if is_pub_field_signature(&s.signature) => {
                    iface_rollup.push('\n');
                    if s.raw == "type" {
                        iface_rollup.push_str(&s.signature);
                    } else {
                        iface_rollup.push_str(strip_value_for_fold(&s.signature));
                    }
                }
                // M21.i: a trait method's signature is public surface too
                // (`raw == "trait"` here -- a trait's methods are its
                // whole contract). Reaches only a *trait's own* methods:
                // an inherent impl's methods are folded separately, in
                // `merge_inherent_impls`, since they're a different
                // symbol at this point in the walk (the impl block, not
                // yet merged into its type). A trait method never carries
                // an explicit `pub` -- it's public because the trait
                // itself is, the same JLS-style implicit-visibility case
                // Java's interface methods have -- so this checks the
                // container kind, not the member's own text, unlike the
                // field case above. No `strip_value_for_fold`: a
                // `fn`/`function_signature_item`'s stored signature has
                // no default-parameter-value syntax in Rust to strip in
                // the first place, and no normalization beyond that
                // (`ARCHITECTURE.md` open question 13's own lean).
                SymbolKind::Callable if raw == "trait" => {
                    iface_rollup.push('\n');
                    iface_rollup.push_str(&s.signature);
                }
                _ => {}
            }
        }
    }

    // Insert the container *before* its members so declaration order in
    // `out` stays parent-then-children, like `extract.rs`.
    out.insert(
        before,
        ExtractedSymbol {
            id,
            kind: SymbolKind::Container,
            raw: raw.to_string(),
            file: file.to_string(),
            lines: node_lines(node),
            signature: sig.clone(),
            docstring: leading_doc(node, source),
            is_exported,
            source_hash: hash_text(&rollup),
            interface_hash: is_exported.then(|| hash_text(&iface_rollup)),
            markers: attributes(node, source),
            extra_spans: Vec::new(),
            parent: parent_id.map(str::to_string),
            children: member_ids,
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn push_leaf(
    node: Node,
    kind: SymbolKind,
    raw: &str,
    parent_id: Option<&str>,
    source: &str,
    file: &str,
    out: &mut Vec<ExtractedSymbol>,
) {
    let Some(name) = field_text(node, "name", source) else {
        return;
    };
    let id = match parent_id {
        Some(p) => format!("{p}::{name}"),
        None => format!("{file}::{name}"),
    };
    let sig = signature_before_body(node, source);
    // A member (inside an impl/trait) is never independently imported —
    // same stance as `extract.rs` for a class method.
    let is_exported = parent_id.is_none() && has_pub(node, source);
    out.push(ExtractedSymbol {
        id,
        kind,
        raw: raw.to_string(),
        file: file.to_string(),
        lines: node_lines(node),
        signature: sig.clone(),
        docstring: leading_doc(node, source),
        is_exported,
        source_hash: hash_text(text(node, source)),
        interface_hash: is_exported.then(|| hash_text(&sig)),
        markers: attributes(node, source),
        extra_spans: Vec::new(),
        parent: parent_id.map(str::to_string),
        children: Vec::new(),
    });
}

/// `impl Graph` / `impl Display for Graph` / `impl<T> Foo<T>` — the source
/// text of everything before the body, `impl ` prefix kept so an `impl`
/// symbol's id can't collide with the `struct` it's for.
fn impl_header(node: Node, source: &str) -> String {
    signature_before_body(node, source)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn trait_name(node: Node, source: &str) -> String {
    field_text(node, "name", source)
        .unwrap_or("<trait>")
        .to_string()
}

/// Shared preamble both [`inherent_impl_target`] and [`trait_impl_target`]
/// need before they diverge on whether/where a ` for ` appears
/// (code-review finding on M21.e: this used to be duplicated in both,
/// the same triplication risk `strip_value_for_fold` was consolidated into
/// `text.rs` to avoid): normalize whitespace, strip the leading `impl`
/// keyword (guarding against an identifier that merely starts with
/// "impl"), and drop everything from a `where` clause onward. `None` for
/// anything that isn't an `impl` header at all.
fn impl_header_before_where(header: &str) -> Option<String> {
    let header: String = header.split_whitespace().collect::<Vec<_>>().join(" ");
    let rest = header.strip_prefix("impl")?;
    if !rest.is_empty() && !rest.starts_with(|c: char| c.is_whitespace() || c == '<') {
        return None;
    }
    let rest = rest.trim_start();
    Some(rest.split(" where ").next().unwrap_or(rest).to_string())
}

/// The bare type name at the front of `path` — any leading generics
/// stripped, then the last `::`-segment taken. The shared tail both
/// [`inherent_impl_target`] and [`trait_impl_target`] reduce to once
/// they've isolated the type path itself. `strip_leading_generics` is a
/// no-op when `path` has no leading `<...>`, so this is safe to call on
/// `trait_impl_target`'s `after_for` (never itself generics-prefixed) as
/// well as `inherent_impl_target`'s own `before_where`.
fn bare_type_name(path: &str) -> Option<String> {
    let after_generics = strip_leading_generics(path);
    let type_path = after_generics
        .split(|c: char| c.is_whitespace() || c == '<')
        .next()
        .unwrap_or("");
    let name = type_path.rsplit("::").next().unwrap_or(type_path);
    (!name.is_empty()).then(|| name.to_string())
}

/// The bare type name an *inherent* `impl` header is for, or `None` for a
/// trait impl or an unparseable header. `impl Graph` -> `Graph`,
/// `impl<T> Wrapper<T>` -> `Wrapper`, `impl ResolveCtx<'_>` -> `ResolveCtx`,
/// `impl foo::Bar` -> `Bar`. A ` for ` before any `where` clause means a
/// trait impl (`impl Debug for S`) — left for its own spec section.
fn inherent_impl_target(header: &str) -> Option<String> {
    let before_where = impl_header_before_where(header)?;
    if before_where.split(' ').any(|w| w == "for") {
        return None;
    }
    bare_type_name(&before_where)
}

/// The bare type name a *trait* `impl` header targets — the mirror image of
/// [`inherent_impl_target`]. `impl Greeter for EnglishGreeter` -> `EnglishGreeter`,
/// `impl<T> From<T> for Wrapper<T>` -> `Wrapper`, `impl fmt::Debug for S` ->
/// `S`. `None` for an inherent impl (no ` for `) or an unparseable header.
/// Finding `for` as a plain word-split, before stripping the impl's own
/// leading generics, is safe the same way `inherent_impl_target`'s own
/// existence-check already relies on: `for` is a reserved keyword, so it can
/// never appear as a trait/type identifier inside a generic bound, however
/// many spaces that bound itself contains (`<T: Iterator<Item = u8>>`).
fn trait_impl_target(header: &str) -> Option<String> {
    let before_where = impl_header_before_where(header)?;
    let words: Vec<&str> = before_where.split(' ').collect();
    let for_pos = words.iter().position(|&w| w == "for")?;
    let after_for = words[for_pos + 1..].join(" ");
    bare_type_name(&after_for)
}

/// Drop a leading `<…>` generic-parameter list (angle-bracket balanced, so
/// `<T: Iterator<Item = u8>>` is handled), returning the rest trimmed. `s`
/// with no leading `<` is returned unchanged.
fn strip_leading_generics(s: &str) -> &str {
    if !s.starts_with('<') {
        return s;
    }
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return s[i + 1..].trim_start();
                }
            }
            _ => {}
        }
    }
    s
}

/// Everything from the item's start up to its `body` (or its terminating
/// `;` / end) — modifiers, keyword, name, generics, params, return type,
/// `where` clause. The "text before the `{`" trick from `extract.rs`.
fn signature_before_body(node: Node, source: &str) -> String {
    let start = node.start_byte();
    let end = node
        .child_by_field_name("body")
        .map(|b| b.start_byte())
        .unwrap_or_else(|| node.end_byte())
        .max(start);
    source[start..end]
        .trim_end()
        .trim_end_matches(';')
        .trim_end()
        .to_string()
}

/// Contiguous run of `///` (or `//!`, or a `/** */` block) doc comments
/// immediately above `node`. `#[...]` attributes and plain `//` comments
/// between the doc and the item are stepped over (they belong to the
/// item); anything else, or a blank line, ends the run — so a comment
/// paragraphs above an unrelated item isn't misattributed.
fn leading_doc(node: Node, source: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    // The line the current run starts on; a preceding node is adjacent iff
    // it ends where this row begins (nothing, not even a blank line, in
    // between). Comment nodes include their trailing newline, so an
    // end column of 0 means "ends at the start of the next line".
    let mut expected_row = node.start_position().row;
    let mut cursor = node.prev_sibling();

    while let Some(n) = cursor {
        let end = n.end_position();
        let end_row_exclusive = end.row + usize::from(end.column > 0);
        if end_row_exclusive != expected_row {
            break;
        }
        match n.kind() {
            "line_comment" | "block_comment" => {
                if let Some(content) = doc_comment_body(text(n, source)) {
                    lines.extend(content.into_iter().rev());
                }
                // a plain `//` comment is stepped over, not collected
            }
            "attribute_item" | "inner_attribute_item" => {}
            _ => break,
        }
        expected_row = n.start_position().row;
        cursor = n.prev_sibling();
    }

    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    Some(lines.join("\n"))
}

/// `Some(content_lines)` for a doc comment (`///` / `//!` / `/** */`),
/// `None` for an ordinary `//` / `/* */` comment (skipped, not a break).
fn doc_comment_body(raw: &str) -> Option<Vec<String>> {
    if let Some(rest) = raw.strip_prefix("///").or_else(|| raw.strip_prefix("//!")) {
        return Some(vec![rest.trim().to_string()]);
    }
    if let Some(inner) = raw
        .strip_prefix("/**")
        .or_else(|| raw.strip_prefix("/*!"))
        .and_then(|s| s.strip_suffix("*/"))
    {
        return Some(
            inner
                .lines()
                .map(|l| l.trim().trim_start_matches('*').trim().to_string())
                .filter(|l| !l.is_empty())
                .collect(),
        );
    }
    None
}

/// The `#[...]` attribute items directly above `node`, verbatim — this is
/// what a `RustStack` reads back for `#[tool]` / `#[derive(...)]` / a test
/// attribute (M14 / design decision 9). Adjacency isn't required: an
/// attribute always abuts its item.
fn attributes(node: Node, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = node.prev_sibling();
    while let Some(n) = cursor {
        match n.kind() {
            "attribute_item" | "inner_attribute_item" => out.push(text(n, source).to_string()),
            "line_comment" | "block_comment" => {}
            _ => break,
        }
        cursor = n.prev_sibling();
    }
    out.reverse();
    out
}

/// A `pub` / `pub(crate)` / `pub(super)` visibility modifier on the item.
fn has_pub(node: Node, source: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| c.kind() == "visibility_modifier" && text(c, source).starts_with("pub"))
}

/// Does a field's own stored `signature` text (`"pub x: u32"` / `"x: u32"`
/// / `"pub(crate) x: u32"`) start with a real `pub` visibility keyword? A
/// bare prefix check (`signature.starts_with("pub")`) would false-positive
/// on a private field literally named e.g. `pub_key` -- checking the first
/// whitespace-separated token for an exact `"pub"` or a `"pub("` prefix
/// (for `pub(crate)`/`pub(super)`, one token, no internal space) avoids
/// that.
fn is_pub_field_signature(signature: &str) -> bool {
    match signature.split_whitespace().next() {
        Some(first) => first == "pub" || first.starts_with("pub("),
        None => false,
    }
}

// A struct field never has inline value syntax in Rust (that's not
// valid -- defaults come via `impl Default`, never in the field
// declaration itself), but `const`/`static` items do, and their own
// `signature` (via `signature_before_body`'s fallback, since neither
// has a `body` field to cut at) carries that value verbatim -- so the
// interface_hash fold strips it via `crate::text::strip_value_for_fold`,
// transiently, without touching the stored field.

fn field_text<'a>(node: Node, field: &str, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name(field).map(|n| text(n, source))
}

fn text<'a>(node: Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or_default()
}

fn node_lines(node: Node) -> [usize; 2] {
    [node.start_position().row + 1, node.end_position().row + 1]
}

// ---------------------------------------------------------------------------
// Imports — `use` statements and their resolution against the module tree.
// The Rust pack's counterpart to `imports.rs` + `resolve.rs`. Reference
// edges only: a `use` names an *item* (type / fn / trait / const / module),
// never a method, so every target is a top-level symbol like the extractor
// produces. `mod foo;` is structural, not a symbol reference — skipped.
// ---------------------------------------------------------------------------

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::graph::{Graph, SymbolId};
use crate::imports::{FileImports, ImportRef, ReExport};
use crate::resolve::ResolvedImport;

/// Parse a `.rs` file's `use` declarations. A `pub use` is a re-export
/// (the barrel pattern — `lib.rs` is all of these); a plain `use` is an
/// import. Glob (`use x::*`), `use x::{self, ..}`'s `self`, and nested
/// lists (`use a::{b::C}`) are skipped — a glob names no single item, and
/// the others are rare enough to defer.
pub fn extract_imports(source: &str, _rel_path: &str) -> FileImports {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("bundled tree-sitter-rust grammar should always load");
    let Some(tree) = parser.parse(source, None) else {
        return FileImports::default();
    };

    let mut fi = FileImports::default();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        if node.kind() != "use_declaration" {
            continue;
        }
        let mut dc = node.walk();
        let kids: Vec<Node> = node.children(&mut dc).collect();
        let is_reexport = kids.iter().any(|c| c.kind() == "visibility_modifier");
        let Some(arg) = kids
            .iter()
            .find(|c| !matches!(c.kind(), "use" | "visibility_modifier" | ";"))
        else {
            continue;
        };
        let arg = *arg;
        for (specifier, name) in use_targets(arg, source) {
            if is_reexport {
                fi.re_exports.push(ReExport {
                    exported_as: name.clone(),
                    specifier,
                    source_name: name,
                });
            } else {
                fi.imports.push(ImportRef {
                    specifier,
                    imported_name: name,
                });
            }
        }
    }
    extract_qualified_refs(root, source, &mut fi.qualified_refs);
    fi
}

/// `ARCHITECTURE.md` open question 14 / `ROADMAP.md` M21.h: a same-crate
/// fully-qualified reference (`crate::a::b::Item`) needs no `use`
/// statement, so the loop above -- which only reads top-level
/// `use_declaration`s -- never sees it. Walks the *whole* tree (every
/// expression and type position, not just the top level) looking for a
/// `crate`-rooted `scoped_identifier` / `scoped_type_identifier`, skipping
/// `use_declaration` subtrees (already covered above) and not recursing
/// into a matched node's own children (its path text is already fully
/// consumed as the one reference it represents). Comments and string
/// literals are never visited at all -- they're leaf tokens, not
/// expression nodes -- so this can't repeat the Javadoc/`import static`
/// false-positive shape the M21.h measurement phase hit for a text scan.
fn extract_qualified_refs(node: Node, source: &str, out: &mut Vec<ImportRef>) {
    if node.kind() == "use_declaration" {
        return;
    }
    if matches!(node.kind(), "scoped_identifier" | "scoped_type_identifier") {
        if let Some((specifier, name)) = crate_qualified_target(node, source) {
            out.push(ImportRef {
                specifier,
                imported_name: name,
            });
        }
        return; // this node's own path/name is already consumed either way
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_qualified_refs(child, source, out);
    }
}

/// A `crate`-rooted qualified node -> `(module-path, item-name)`, or `None`
/// if it isn't rooted at `crate` (an external crate, or `self::`/`super::`
/// -- out of scope for this phase; only `crate::` was actually measured).
/// `scoped_identifier` whose own `path` is itself a `scoped_type_identifier`
/// is the "one segment short" shape measured in `mcp.rs`/`quarkus.rs`
/// (`crate::symbol::SymbolKind::Callable`, an enum-variant/associated-item
/// access) -- the real dependency is the *type* one level up
/// (`SymbolKind`), not `Callable`, which isn't a symbol of its own.
fn crate_qualified_target(node: Node, source: &str) -> Option<(String, String)> {
    let path = node.child_by_field_name("path")?;
    let name = node.child_by_field_name("name")?;
    if node.kind() == "scoped_identifier" && path.kind() == "scoped_type_identifier" {
        let inner_path = path.child_by_field_name("path")?;
        let inner_name = path.child_by_field_name("name")?;
        return crate_rooted(inner_path, source).then(|| {
            (
                text(inner_path, source).to_string(),
                text(inner_name, source).to_string(),
            )
        });
    }
    crate_rooted(path, source).then(|| {
        (
            text(path, source).to_string(),
            text(name, source).to_string(),
        )
    })
}

/// Whether a qualified path's prefix is rooted at `crate` -- a node's own
/// text span is always the exact source substring it covers, so checking
/// the leading word is enough; no need to walk the nested `path` fields
/// down to the `crate` leaf by hand.
fn crate_rooted(path: Node, source: &str) -> bool {
    let t = text(path, source);
    t == "crate" || t.starts_with("crate::")
}

/// `(module-path, item-name)` pairs for one `use` argument node.
fn use_targets(arg: Node, source: &str) -> Vec<(String, String)> {
    match arg.kind() {
        // `use a::b::Item;`
        "scoped_identifier" => split_last(text(arg, source)).into_iter().collect(),
        // `use a::b::Item as Alias;` — the alias is a local rename, track
        // the source name like `imports.rs` does for TS.
        "use_as_clause" => arg
            .child_by_field_name("path")
            .or_else(|| arg.named_child(0))
            .and_then(|p| split_last(text(p, source)))
            .into_iter()
            .collect(),
        // `use a::b::{X, Y, Z};`
        "scoped_use_list" => {
            let mut cursor = arg.walk();
            let kids: Vec<Node> = arg.children(&mut cursor).collect();
            let prefix = kids
                .first()
                .map(|n| text(*n, source).to_string())
                .unwrap_or_default();
            let Some(list) = kids.iter().find(|c| c.kind() == "use_list") else {
                return Vec::new();
            };
            let mut lc = list.walk();
            list.children(&mut lc)
                .filter(|c| c.kind() == "identifier")
                .map(|c| (prefix.clone(), text(c, source).to_string()))
                .collect()
        }
        _ => Vec::new(), // use_wildcard, bare identifier, etc.
    }
}

/// `"crate::graph::Graph"` -> `("crate::graph", "Graph")`. `None` for a
/// single-segment path (`use foo;` — a whole-module import, no item).
fn split_last(path: &str) -> Option<(String, String)> {
    path.rsplit_once("::")
        .map(|(pre, name)| (pre.to_string(), name.to_string()))
}

/// Resolve every file's `use` imports (and `pub use` re-exports) against
/// the module tree. A module path maps to a file by Rust's filesystem
/// convention — `crate::a::b` -> `<src>/a/b.rs` or `<src>/a/b/mod.rs` —
/// not by reading `mod` declarations; then the item is looked up as a
/// top-level symbol in that file, following one hop of `pub use`.
pub fn resolve_imports(
    root: &Path,
    file_imports: &HashMap<String, FileImports>,
    graph: &Graph,
) -> Vec<ResolvedImport> {
    let src = crate_src_dir(file_imports);
    let mut sorted: Vec<_> = file_imports.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    let mut out = Vec::new();
    for (from_file, fi) in sorted {
        let each = fi
            .imports
            .iter()
            .map(|i| (&i.specifier, &i.imported_name))
            .chain(fi.re_exports.iter().map(|r| (&r.specifier, &r.source_name)));
        let mut seen_targets: HashSet<SymbolId> = HashSet::new();
        for (specifier, name) in each {
            let target = resolve_one(&src, from_file, specifier, name, file_imports, graph);
            if let Some(id) = target {
                seen_targets.insert(id);
            }
            out.push(ResolvedImport {
                from_file: from_file.clone(),
                specifier: specifier.clone(),
                imported_name: name.clone(),
                target,
            });
        }
        // ARCHITECTURE.md open question 14 / M21.h: only add a qualified-ref
        // edge that resolves to a *real, new* target -- unlike an explicit
        // `use`, an unresolved qualified ref is noise, not a declared intent
        // worth recording; a same-file target was never a blind spot at all
        // (the `bat` false start from measurement); and a target already
        // covered by an explicit `use` above would otherwise duplicate in
        // `get_callees`, which does no dedup of its own.
        for qr in &fi.qualified_refs {
            let Some(id) = resolve_one(
                &src,
                from_file,
                &qr.specifier,
                &qr.imported_name,
                file_imports,
                graph,
            ) else {
                continue;
            };
            if graph.owning_file_id(id) == from_file.as_str() || !seen_targets.insert(id) {
                continue;
            }
            out.push(ResolvedImport {
                from_file: from_file.clone(),
                specifier: qr.specifier.clone(),
                imported_name: qr.imported_name.clone(),
                target: Some(id),
            });
        }
        out.extend(same_file_trait_impl_edges(from_file, graph));
    }
    let _ = root; // resolution is off the walked file set, not the FS
    out
}

/// `ARCHITECTURE.md` open question 16's general case (`ROADMAP.md` M21.g):
/// `impl Trait for Type` never needs a `use` statement for `Type` when
/// both live in the same file, so `graph.imports()` has no edge for it —
/// without one, a change to `Type`'s own fields never invalidates the
/// impl block's document, even though the impl's own text is what
/// actually depends on that shape (`self.field` inside its methods). This
/// synthesizes exactly the edge an explicit `use` would have produced,
/// scoped to trait impls whose target is defined in the same file --
/// `resolve_one` above already handles the cross-file case once a real
/// `use` names the type, so this only fills the gap `use` can't reach.
/// One edge per trait impl, `impl -> type`; no edge links two sibling
/// impls of the same type to each other, since neither's document says
/// anything about the other's behavior.
fn same_file_trait_impl_edges(file: &str, graph: &Graph) -> Vec<ResolvedImport> {
    let Some(file_node) = graph.find(file).and_then(|id| graph.get_file(id)) else {
        return Vec::new();
    };
    file_node
        .children
        .iter()
        .filter_map(|&id| graph.get_symbol(id))
        .filter(|s| s.raw == "impl")
        .filter_map(|s| {
            let target_name = trait_impl_target(&s.signature)?;
            let target_id = graph.find(&format!("{file}::{target_name}"))?;
            Some(ResolvedImport {
                from_file: file.to_string(),
                // Every other resolved import's specifier is a real
                // module path (`crate::stats`, a Java package); this one
                // has no `use` statement to draw that from, but leaving
                // it empty renders a malformed `` `target` — `` line with
                // nothing after the dash wherever a dependency is
                // displayed (the rendered `### Depends on` section,
                // `get_callees`'s response) -- named plainly instead.
                specifier: "same file".to_string(),
                imported_name: target_name,
                target: Some(target_id),
            })
        })
        .collect()
}

fn resolve_one(
    src: &str,
    from_file: &str,
    specifier: &str,
    name: &str,
    file_imports: &HashMap<String, FileImports>,
    graph: &Graph,
) -> Option<SymbolId> {
    let target_file = module_path_to_file(src, from_file, specifier, file_imports)?;
    if let Some(id) = graph.find(&format!("{target_file}::{name}")) {
        return Some(id);
    }
    // One hop through a `pub use` in the target module.
    let rx = file_imports
        .get(&target_file)?
        .re_exports
        .iter()
        .find(|r| r.exported_as == name)?;
    let hop = module_path_to_file(src, &target_file, &rx.specifier, file_imports)?;
    graph.find(&format!("{hop}::{}", rx.source_name))
}

/// The directory the crate root lives in — `"src"` for the usual layout,
/// `""` for a crate rooted at the repo root. Taken from the walked file
/// set so it needs no filesystem access.
fn crate_src_dir(file_imports: &HashMap<String, FileImports>) -> String {
    for f in file_imports.keys() {
        if let Some(dir) = f
            .strip_suffix("/lib.rs")
            .or_else(|| f.strip_suffix("/main.rs"))
        {
            return dir.to_string();
        }
        if f == "lib.rs" || f == "main.rs" {
            return String::new();
        }
    }
    "src".to_string()
}

/// The module a file *is* — `src/graph.rs` is module `crate::graph`, and a
/// `self::x` from it means `src/graph/x.rs`. `mod.rs` / crate roots are
/// the module named by their directory.
fn module_dir_of(file: &str) -> String {
    if file.ends_with("/mod.rs")
        || file.ends_with("/lib.rs")
        || file.ends_with("/main.rs")
        || file == "lib.rs"
        || file == "main.rs"
    {
        return file
            .rsplit_once('/')
            .map_or(String::new(), |(d, _)| d.to_string());
    }
    file.strip_suffix(".rs").unwrap_or(file).to_string()
}

/// A `crate::` / `self::` / `super::` module path -> the repo-relative
/// file that module's items live in, or `None` for an external crate or an
/// unresolvable path.
fn module_path_to_file(
    src: &str,
    from_file: &str,
    specifier: &str,
    file_imports: &HashMap<String, FileImports>,
) -> Option<String> {
    let segs: Vec<&str> = specifier.split("::").collect();
    let exists = |p: &str| file_imports.contains_key(p);

    let (base, rest): (String, &[&str]) = match *segs.first()? {
        "crate" => (src.to_string(), &segs[1..]),
        "self" => (module_dir_of(from_file), &segs[1..]),
        "super" => {
            let ups = segs.iter().take_while(|s| **s == "super").count();
            let mut dir = module_dir_of(from_file);
            for _ in 0..ups {
                dir = dir
                    .rsplit_once('/')
                    .map_or(String::new(), |(d, _)| d.to_string());
            }
            (dir, &segs[ups..])
        }
        // An external crate (`anyhow`, `std`, `tree_sitter`) — or, in 2018
        // style, a bare top-level module. Try the latter under `src`; if
        // no such file was walked, it's external -> unresolved.
        _ => (src.to_string(), &segs[..]),
    };

    let joined = if rest.is_empty() {
        base.clone()
    } else {
        let mut p = base.clone();
        for s in rest {
            if !p.is_empty() {
                p.push('/');
            }
            p.push_str(s);
        }
        p
    };

    let prefixed = |name: &str| {
        if joined.is_empty() {
            name.to_string()
        } else {
            format!("{joined}/{name}")
        }
    };
    [
        format!("{joined}.rs"),
        prefixed("mod.rs"),
        prefixed("lib.rs"),
        prefixed("main.rs"),
    ]
    .into_iter()
    .find(|cand| exists(cand))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_free_function() {
        let src = "/// Doubles it.\npub fn double(x: i64) -> i64 {\n    x * 2\n}\n";
        let syms = extract_file(src, "src/math.rs");
        assert_eq!(syms.len(), 1);
        let s = &syms[0];
        assert_eq!(s.id, "src/math.rs::double");
        assert_eq!(s.kind, SymbolKind::Callable);
        assert_eq!(s.raw, "fn");
        assert_eq!(s.signature, "pub fn double(x: i64) -> i64");
        assert_eq!(s.docstring.as_deref(), Some("Doubles it."));
        assert!(s.is_exported);
        assert!(s.parent.is_none());
    }

    #[test]
    fn private_function_is_not_exported() {
        let syms = extract_file("fn helper() {}\n", "a.rs");
        assert_eq!(syms.len(), 1);
        assert!(!syms[0].is_exported);
        assert_eq!(syms[0].interface_hash, None);
    }

    #[test]
    fn a_method_colliding_with_a_field_of_the_same_name_gets_a_distinct_id() {
        // Real, valid Rust (a common getter idiom): `self.validate` (the
        // field) and `self.validate()` (the method) coexist fine, but
        // both naturally compute to the same id string under this
        // extractor's `{parent}::{name}` scheme. Code-review finding:
        // before this test existed, the method's reparented id silently
        // collided with the field's, and Graph::build's by_id map would
        // keep only whichever won the last insert -- the other's arena
        // node became permanently unreachable by any lookup.
        let src = "pub struct Config {\n    pub validate: bool,\n}\n\nimpl Config {\n    pub fn validate(&self) -> bool {\n        self.validate\n    }\n}\n";
        let syms = extract_file(src, "a.rs");
        let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        // No two symbols share an id.
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
        assert_eq!(
            field.id, "a.rs::Config::validate",
            "the field keeps the plain name"
        );

        let method = syms
            .iter()
            .find(|s| s.kind == SymbolKind::Callable && s.raw == "method")
            .expect("the method");
        assert_ne!(method.id, field.id);
        assert!(
            method.id.starts_with("a.rs::Config::validate#"),
            "method got a disambiguated id: {}",
            method.id
        );

        // Config's own `children` lists both real, distinct ids -- not
        // the same string twice.
        let config = &syms[0];
        assert_eq!(config.children.len(), 2);
        assert_ne!(config.children[0], config.children[1]);
        assert!(config.children.contains(&field.id));
        assert!(config.children.contains(&method.id));
    }

    #[test]
    fn inherent_impl_folds_into_its_type() {
        let src = "\
pub struct Counter {\n    n: u64,\n}\n\n\
impl Counter {\n\
    /// Start at zero.\n\
    pub fn new() -> Self {\n        Self { n: 0 }\n    }\n\
    fn bump(&mut self) {\n        self.n += 1;\n    }\n\
}\n";
        let syms = extract_file(src, "src/counter.rs");
        let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "src/counter.rs::Counter",
                "src/counter.rs::Counter::new",
                "src/counter.rs::Counter::bump",
                "src/counter.rs::Counter::n",
            ],
            "the `impl Counter` block should be gone, its methods reparented onto Counter -- \
             `n` trails the flat list here (an artifact of the fold pass appending an already- \
             extracted field's index after the impl's methods), but Counter's own `children` \
             below keeps the true field-then-methods order"
        );

        let the_struct = &syms[0];
        assert_eq!(the_struct.kind, SymbolKind::Container);
        assert_eq!(the_struct.raw, "struct");
        assert_eq!(
            the_struct.children,
            vec![
                "src/counter.rs::Counter::n",
                "src/counter.rs::Counter::new",
                "src/counter.rs::Counter::bump"
            ]
        );

        let new = &syms[1];
        assert_eq!(new.raw, "method");
        assert_eq!(new.parent.as_deref(), Some("src/counter.rs::Counter"));
        assert_eq!(new.docstring.as_deref(), Some("Start at zero."));
        assert!(!new.is_exported);

        let n = &syms[3];
        assert_eq!(n.id, "src/counter.rs::Counter::n");
        assert_eq!(n.kind, SymbolKind::Value);
        assert_eq!(n.raw, "field");
    }

    #[test]
    fn several_inherent_impls_all_fold_into_the_one_type() {
        let src = "\
struct S;\n\
impl S {\n    fn a(&self) {}\n}\n\
impl S {\n    fn b(&self) {}\n}\n";
        let syms = extract_file(src, "a.rs");
        let s = syms.iter().find(|s| s.id == "a.rs::S").unwrap();
        assert_eq!(s.children, vec!["a.rs::S::a", "a.rs::S::b"]);
        assert!(
            !syms.iter().any(|s| s.raw == "impl"),
            "no impl-block symbol should survive"
        );
    }

    #[test]
    fn folded_type_source_hash_moves_with_a_method_body_edit() {
        let a = extract_file(
            "struct S;\nimpl S {\n    fn f(&self) { let x = 1; }\n}\n",
            "a.rs",
        );
        let b = extract_file(
            "struct S;\nimpl S {\n    fn f(&self) { let x = 2; }\n}\n",
            "a.rs",
        );
        let s_a = a.iter().find(|s| s.id == "a.rs::S").unwrap();
        let s_b = b.iter().find(|s| s.id == "a.rs::S").unwrap();
        assert_ne!(s_a.source_hash, s_b.source_hash);
    }

    #[test]
    fn trait_impl_is_left_as_its_own_symbol() {
        let src = "\
struct S;\n\
impl S {\n    fn own(&self) {}\n}\n\
impl std::fmt::Debug for S {\n    fn fmt(&self) {}\n}\n";
        let syms = extract_file(src, "a.rs");
        // The inherent `impl S` folded in; the trait impl did not.
        assert_eq!(
            syms.iter().find(|s| s.id == "a.rs::S").unwrap().children,
            vec!["a.rs::S::own"]
        );
        assert!(
            syms.iter()
                .any(|s| s.id == "a.rs::impl std::fmt::Debug for S" && s.raw == "impl")
        );
    }

    #[test]
    fn inherent_impl_target_parses_headers() {
        assert_eq!(inherent_impl_target("impl Graph").as_deref(), Some("Graph"));
        assert_eq!(
            inherent_impl_target("impl ResolveCtx<'_>").as_deref(),
            Some("ResolveCtx")
        );
        assert_eq!(
            inherent_impl_target("impl<T> Wrapper<T>").as_deref(),
            Some("Wrapper")
        );
        assert_eq!(
            inherent_impl_target("impl<T: Iterator<Item = u8>> Foo<T>").as_deref(),
            Some("Foo")
        );
        assert_eq!(inherent_impl_target("impl std::fmt::Debug for S"), None);
        assert_eq!(inherent_impl_target("impl<T> From<T> for S"), None);
    }

    #[test]
    fn trait_impl_target_parses_headers() {
        assert_eq!(
            trait_impl_target("impl Greeter for EnglishGreeter").as_deref(),
            Some("EnglishGreeter")
        );
        assert_eq!(
            trait_impl_target("impl std::fmt::Debug for S").as_deref(),
            Some("S")
        );
        assert_eq!(
            trait_impl_target("impl<T> From<T> for Wrapper<T>").as_deref(),
            Some("Wrapper")
        );
        assert_eq!(
            trait_impl_target("impl<T: Iterator<Item = u8>> Trait for Foo<T>").as_deref(),
            Some("Foo")
        );
        // An inherent impl has no target -- the mirror image of
        // `inherent_impl_target` returning `None` for a trait impl.
        assert_eq!(trait_impl_target("impl Graph"), None);
        assert_eq!(trait_impl_target("impl<T> Wrapper<T>"), None);
    }

    #[test]
    fn a_tuple_structs_trait_impl_does_not_fold_in_despite_children_being_empty() {
        // Code-review finding on M21.e: `children.is_empty()` is also true
        // for a tuple struct's real positional fields, since this
        // extractor doesn't capture them as members at all yet (see
        // `tuple_struct_fields_are_not_yet_extracted_as_members` above).
        // Without a separate check, `UserId` (one real field, just not one
        // this pass extracts) would be mistaken for a content-free marker
        // type and its Display impl folded in -- the exact over-merge
        // M21.e exists to avoid for any type with real content.
        let syms = extract_file(
            "pub struct UserId(u64);\n\n\
             impl std::fmt::Display for UserId {\n\
                 fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n\
                     write!(f, \"{}\", self.0)\n    }\n\
             }\n",
            "a.rs",
        );
        let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert!(
            ids.contains(&"a.rs::impl std::fmt::Display for UserId"),
            "a tuple struct's trait impl must stay its own section, not \
             fold into the type just because its real field isn't \
             extracted as a member yet: {ids:?}"
        );
    }

    #[test]
    fn derive_and_attributes_land_in_markers() {
        let src = "#[derive(Debug, Clone)]\n#[serde(rename_all = \"snake_case\")]\npub enum Kind {\n    A,\n}\n";
        let syms = extract_file(src, "a.rs");
        let e = &syms[0];
        assert_eq!(e.raw, "enum");
        assert_eq!(
            e.markers,
            vec![
                "#[derive(Debug, Clone)]",
                "#[serde(rename_all = \"snake_case\")]"
            ]
        );
    }

    #[test]
    fn const_and_static_are_values() {
        let syms = extract_file(
            "pub const MAX: usize = 8;\nstatic NAME: &str = \"x\";\n",
            "a.rs",
        );
        assert_eq!(syms[0].kind, SymbolKind::Value);
        assert_eq!(syms[0].raw, "const");
        assert_eq!(syms[1].kind, SymbolKind::Value);
        assert_eq!(syms[1].raw, "static");
    }

    #[test]
    fn a_pub_fields_type_change_moves_the_structs_interface_hash() {
        // M21.b: the whole point of the fold -- a public field's type is
        // public surface exactly like a method's return type is.
        let a = extract_file("pub struct P {\n    pub x: u32,\n}\n", "a.rs");
        let b = extract_file("pub struct P {\n    pub x: u64,\n}\n", "a.rs");
        assert_ne!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_pub_fields_value_only_edit_never_applies_here_but_reorder_still_moves_it() {
        // Table row 6 (M21.b decision record, ROADMAP.md): reordering two
        // pub fields does move interface_hash under the plain fold
        // (mirrors source_hash's existing order-sensitive treatment),
        // even though nothing about the public contract changed --
        // deliberately not engineered around, confirmed here so the
        // behavior is at least tested, not silently assumed.
        let a = extract_file(
            "pub struct P {\n    pub x: u32,\n    pub y: u32,\n}\n",
            "a.rs",
        );
        let reordered = extract_file(
            "pub struct P {\n    pub y: u32,\n    pub x: u32,\n}\n",
            "a.rs",
        );
        assert_ne!(a[0].interface_hash, reordered[0].interface_hash);
    }

    #[test]
    fn a_private_fields_type_change_does_not_move_interface_hash() {
        // Table row 11: private isn't public surface, under either regime.
        let a = extract_file("pub struct P {\n    x: u32,\n}\n", "a.rs");
        let b = extract_file("pub struct P {\n    x: u64,\n}\n", "a.rs");
        assert_eq!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_pub_fields_docstring_change_does_not_move_interface_hash() {
        // Table row 8: a doc comment never contributes to interface_hash,
        // for anything -- a field's own text (what gets folded) never
        // included its docstring to begin with.
        let a = extract_file(
            "pub struct P {\n    /// old doc\n    pub x: u32,\n}\n",
            "a.rs",
        );
        let b = extract_file(
            "pub struct P {\n    /// totally different doc\n    pub x: u32,\n}\n",
            "a.rs",
        );
        assert_eq!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_pub_inherent_methods_signature_change_moves_the_structs_interface_hash() {
        // M21.i: the fold extended to methods -- an inherent impl's
        // public method signature is public surface too, folded in only
        // once `merge_inherent_impls` reparents the method onto the type
        // (it isn't the type's own member before that merge happens).
        let a = extract_file(
            "pub struct P;\n\nimpl P {\n    pub fn touch(&self, x: u32) {}\n}\n",
            "a.rs",
        );
        let b = extract_file(
            "pub struct P;\n\nimpl P {\n    pub fn touch(&self, x: u64) {}\n}\n",
            "a.rs",
        );
        assert_ne!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_private_inherent_methods_signature_change_does_not_move_interface_hash() {
        // Same reasoning as a private field: not public surface, under
        // either regime.
        let a = extract_file(
            "pub struct P;\n\nimpl P {\n    fn touch(&self, x: u32) {}\n}\n",
            "a.rs",
        );
        let b = extract_file(
            "pub struct P;\n\nimpl P {\n    fn touch(&self, x: u64) {}\n}\n",
            "a.rs",
        );
        assert_eq!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_pub_traits_method_signature_change_moves_the_traits_interface_hash() {
        // M21.i: a trait method never carries an explicit `pub` -- it's
        // public because the trait itself is -- so this has to be gated
        // on the container's own kind (`raw == "trait"`), not the
        // member's own text, the same JLS-implicit-visibility shape
        // Java's interface methods have.
        let a = extract_file(
            "pub trait Greeter {\n    fn greet(&self, loud: bool);\n}\n",
            "a.rs",
        );
        let b = extract_file(
            "pub trait Greeter {\n    fn greet(&self, loud: bool, times: u32);\n}\n",
            "a.rs",
        );
        assert_ne!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_degenerate_trait_impls_merged_method_signature_moves_the_types_interface_hash() {
        // Code-review finding on the M21.i PR: M21.e folds a zero-field
        // type's single trait impl into the type itself (one document),
        // but that impl's methods carry no `pub` -- their visibility comes
        // from the trait -- so an explicit-`pub` check silently dropped
        // them from the type's interface_hash even though they're now its
        // documented surface.
        fn src(params: &str) -> String {
            format!(
                "pub trait Greeter {{\n    fn greet(&self{params});\n}}\n\n\
                 pub struct P;\n\n\
                 impl Greeter for P {{\n    fn greet(&self{params}) {{}}\n}}\n"
            )
        }
        let find_p = |syms: &[ExtractedSymbol]| {
            syms.iter()
                .find(|s| s.id == "a.rs::P")
                .expect("P must still exist")
                .interface_hash
                .clone()
        };
        let a = extract_file(&src(", loud: bool"), "a.rs");
        let b = extract_file(&src(", loud: bool, times: u32"), "a.rs");
        assert!(
            b.iter().any(|s| s.id == "a.rs::P::greet"),
            "sanity: the trait impl must actually have folded into P"
        );
        assert_ne!(find_p(&a), find_p(&b));
    }

    #[test]
    fn a_pub_consts_value_only_edit_inside_a_pub_mod_does_not_move_interface_hash() {
        // Code-review finding: the fold loop includes every SymbolKind
        // ::Value direct member, not just struct fields -- a pub mod's
        // pub const/static items are also Value, and (unlike struct
        // fields, which have no inline value syntax in Rust at all)
        // const/static's own `signature` does carry its initializer
        // value, so a value-only edit was spuriously moving the mod's
        // interface_hash.
        let a = extract_file("pub mod config {\n    pub const X: i32 = 1;\n}\n", "a.rs");
        let b = extract_file("pub mod config {\n    pub const X: i32 = 2;\n}\n", "a.rs");
        assert_eq!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_pub_consts_type_change_inside_a_pub_mod_moves_interface_hash() {
        let a = extract_file("pub mod config {\n    pub const X: i32 = 1;\n}\n", "a.rs");
        let b = extract_file(
            "pub mod config {\n    pub const X: &str = \"1\";\n}\n",
            "a.rs",
        );
        assert_ne!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_new_pub_field_moves_interface_hash_but_a_new_private_one_does_not() {
        let base = extract_file("pub struct P {\n    pub x: u32,\n}\n", "a.rs");
        let plus_pub = extract_file(
            "pub struct P {\n    pub x: u32,\n    pub y: u32,\n}\n",
            "a.rs",
        );
        let plus_private =
            extract_file("pub struct P {\n    pub x: u32,\n    y: u32,\n}\n", "a.rs");
        assert_ne!(base[0].interface_hash, plus_pub[0].interface_hash);
        assert_eq!(base[0].interface_hash, plus_private[0].interface_hash);
    }

    #[test]
    fn a_pub_type_aliass_target_change_moves_interface_hash() {
        // Code-review finding: unlike const/static, a type alias's "=" is
        // not a discardable value -- the aliased type IS its entire
        // public meaning, so strip_value_for_fold must never touch it.
        let a = extract_file("pub mod config {\n    pub type Id = u64;\n}\n", "a.rs");
        let b = extract_file("pub mod config {\n    pub type Id = String;\n}\n", "a.rs");
        assert_ne!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn a_private_field_literally_named_pub_key_is_not_mistaken_for_public() {
        // is_pub_field_signature's whole reason to exist: a bare
        // signature.starts_with("pub") would false-positive here.
        let a = extract_file("pub struct P {\n    pub_key: u32,\n}\n", "a.rs");
        let b = extract_file("pub struct P {\n    pub_key: u64,\n}\n", "a.rs");
        assert_eq!(a[0].interface_hash, b[0].interface_hash);
    }

    #[test]
    fn struct_fields_are_extracted_as_value_members() {
        // The exact FileNode shape (src/graph.rs) that sent an agent to
        // Read mid-session (M21.a's own exit test, per ROADMAP.md).
        let src = "\
pub struct FileNode {\n\
    /// Repo-relative path.\n\
    pub id: String,\n\
    /// Hash of the file's raw text.\n\
    pub source_hash: String,\n\
    /// Top-level symbols, in declaration order.\n\
    pub children: Vec<SymbolId>,\n\
}\n";
        let syms = extract_file(src, "a.rs");
        assert_eq!(syms[0].id, "a.rs::FileNode");
        assert_eq!(syms[0].kind, SymbolKind::Container);
        assert_eq!(
            syms[0].children,
            vec![
                "a.rs::FileNode::id",
                "a.rs::FileNode::source_hash",
                "a.rs::FileNode::children"
            ]
        );

        let fields = &syms[1..4];
        let ids: Vec<&str> = fields.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "a.rs::FileNode::id",
                "a.rs::FileNode::source_hash",
                "a.rs::FileNode::children"
            ]
        );
        for f in fields {
            assert_eq!(f.kind, SymbolKind::Value);
            assert_eq!(f.raw, "field");
            assert_eq!(f.parent.as_deref(), Some("a.rs::FileNode"));
            // A member is never independently imported -- same stance as
            // a method (push_leaf's own rule), so this is never exported
            // on its own even though the field itself is `pub`.
            assert!(!f.is_exported);
            assert_eq!(f.interface_hash, None);
        }
        assert_eq!(fields[0].docstring.as_deref(), Some("Repo-relative path."));
        assert_eq!(
            fields[1].docstring.as_deref(),
            Some("Hash of the file's raw text.")
        );
        assert_eq!(
            fields[2].docstring.as_deref(),
            Some("Top-level symbols, in declaration order.")
        );
    }

    #[test]
    fn a_private_field_is_still_extracted_as_a_member() {
        // Matches java.rs's precedent: a member is extracted regardless
        // of its own visibility, same as a private method already is.
        let syms = extract_file("pub struct C {\n    n: u64,\n}\n", "a.rs");
        assert_eq!(syms.len(), 2);
        assert_eq!(syms[1].id, "a.rs::C::n");
        assert_eq!(syms[1].raw, "field");
    }

    #[test]
    fn field_reorder_moves_the_structs_source_hash_but_unrelated_edit_does_not() {
        let a = extract_file("pub struct P {\n    x: u32,\n    y: u32,\n}\n", "a.rs");
        let reordered = extract_file("pub struct P {\n    y: u32,\n    x: u32,\n}\n", "a.rs");
        assert_ne!(a[0].source_hash, reordered[0].source_hash);

        let unrelated_edit = extract_file(
            "// a comment that doesn't touch the struct\npub struct P {\n    x: u32,\n    y: u32,\n}\n",
            "a.rs",
        );
        assert_eq!(a[0].source_hash, unrelated_edit[0].source_hash);
    }

    #[test]
    fn unit_struct_with_no_body_has_no_members() {
        let syms = extract_file("pub struct Marker;\n", "a.rs");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].children, Vec::<String>::new());
    }

    #[test]
    fn a_nested_structs_fields_are_not_leaked_onto_its_enclosing_module() {
        // Regression: visit_container's member_ids used to be "everything
        // after `before`" -- correct back when a nested container's own
        // members were always leaf callables with no children of their
        // own, but a struct with real field members now recurses and
        // pushes grandchildren into that same flat range too.
        let src = "pub mod outer {\n    pub struct Foo {\n        x: i32,\n    }\n}\n";
        let syms = extract_file(src, "a.rs");
        let outer = syms.iter().find(|s| s.id == "a.rs::outer").unwrap();
        assert_eq!(outer.children, vec!["a.rs::outer::Foo"]);
        let foo = syms.iter().find(|s| s.id == "a.rs::outer::Foo").unwrap();
        assert_eq!(foo.children, vec!["a.rs::outer::Foo::x"]);
        assert_eq!(foo.parent.as_deref(), Some("a.rs::outer"));
    }

    #[test]
    fn tuple_struct_fields_are_not_yet_extracted_as_members() {
        // Deliberately out of scope for this pass (see rust.rs's module
        // doc): a tuple field has no `name` field in the grammar at all
        // (`ordered_field_declaration_list`, not `field_declaration_list`),
        // so it falls through visit_item's catch-all and is skipped --
        // gracefully, not silently wrong, just not a member yet.
        let syms = extract_file("pub struct Point(pub f64, pub f64);\n", "a.rs");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].children, Vec::<String>::new());
    }

    #[test]
    fn macro_rules_is_a_callable() {
        let syms = extract_file("macro_rules! shout {\n    () => {};\n}\n", "a.rs");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].id, "a.rs::shout");
        assert_eq!(syms[0].raw, "macro_rules");
        assert_eq!(syms[0].kind, SymbolKind::Callable);
    }

    #[test]
    fn inline_module_nests_its_items() {
        let src = "pub mod parse {\n    pub fn run() {}\n    struct Inner;\n}\n";
        let syms = extract_file(src, "a.rs");
        let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["a.rs::parse", "a.rs::parse::run", "a.rs::parse::Inner"]
        );
        assert_eq!(syms[0].raw, "mod");
        assert_eq!(syms[1].parent.as_deref(), Some("a.rs::parse"));
    }

    #[test]
    fn mod_declaration_without_a_body_is_skipped() {
        // `mod foo;` is an import-graph edge (commit 4), not a symbol.
        let syms = extract_file("mod foo;\npub fn g() {}\n", "a.rs");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].id, "a.rs::g");
    }

    #[test]
    fn cfg_test_module_is_skipped_whole() {
        let src = "\
pub fn real() {}\n\n\
#[cfg(test)]\n\
mod tests {\n\
    use super::*;\n\
    #[test]\n\
    fn a_test() { real(); }\n\
}\n";
        let syms = extract_file(src, "a.rs");
        assert_eq!(
            syms.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["a.rs::real"]
        );
    }

    #[test]
    fn extracts_this_crate_s_own_graph_module() {
        // The M14 dogfood check: parse a real file from this repo.
        let src = std::fs::read_to_string("src/graph.rs").unwrap();
        let syms = extract_file(&src, "src/graph.rs");
        let ids: std::collections::HashSet<&str> = syms.iter().map(|s| s.id.as_str()).collect();
        assert!(
            ids.contains("src/graph.rs::Graph"),
            "missing the Graph type"
        );
        assert!(
            ids.contains("src/graph.rs::FORMAT_VERSION"),
            "missing the FORMAT_VERSION const"
        );
        assert!(
            !syms.iter().any(|s| s.id == "src/graph.rs::impl Graph"),
            "the inherent `impl Graph` block should have folded into Graph"
        );
        assert!(
            syms.iter()
                .any(|s| s.raw == "method" && s.id == "src/graph.rs::Graph::build"),
            "missing Graph::build, reparented onto Graph"
        );
        // Every symbol carries a concrete keyword.
        assert!(syms.iter().all(|s| !s.raw.is_empty()));
    }

    // --- imports ---

    #[test]
    fn parses_plain_grouped_aliased_and_pub_use() {
        let src = "\
use crate::graph::Graph;\n\
use crate::graph::{Node, SymbolId};\n\
use crate::hash::hash_text as h;\n\
use anyhow::Result;\n\
use crate::features::*;\n\
pub use crate::stack::RustStack;\n";
        let fi = extract_imports(src, "src/x.rs");

        let names: Vec<(&str, &str)> = fi
            .imports
            .iter()
            .map(|i| (i.specifier.as_str(), i.imported_name.as_str()))
            .collect();
        assert_eq!(
            names,
            vec![
                ("crate::graph", "Graph"),
                ("crate::graph", "Node"),
                ("crate::graph", "SymbolId"),
                ("crate::hash", "hash_text"), // alias dropped, source name kept
                ("anyhow", "Result"),
                // the glob `use crate::features::*` is not tracked
            ]
        );
        assert_eq!(fi.re_exports.len(), 1);
        assert_eq!(fi.re_exports[0].source_name, "RustStack");
        assert_eq!(fi.re_exports[0].specifier, "crate::stack");
    }

    /// A fixture module tree, resolved end to end.
    fn resolved(files: &[(&str, &str)]) -> Vec<ResolvedImport> {
        let extractions: Vec<crate::graph::FileExtraction> = files
            .iter()
            .map(|(p, src)| crate::graph::FileExtraction {
                rel_path: (*p).to_string(),
                source_hash: crate::hash::hash_text(src),
                symbols: extract_file(src, p),
            })
            .collect();
        let graph = Graph::build(extractions);
        let file_imports: HashMap<String, FileImports> = files
            .iter()
            .map(|(p, src)| ((*p).to_string(), extract_imports(src, p)))
            .collect();
        resolve_imports(Path::new("/unused"), &file_imports, &graph)
    }

    #[test]
    fn crate_relative_use_resolves_to_a_top_level_item() {
        let edges = resolved(&[
            ("src/lib.rs", "pub mod graph;\npub mod app;\n"),
            ("src/graph.rs", "pub struct Graph;\n"),
            (
                "src/app.rs",
                "use crate::graph::Graph;\npub fn run(_g: Graph) {}\n",
            ),
        ]);
        let e = edges
            .iter()
            .find(|e| e.from_file == "src/app.rs" && e.imported_name == "Graph")
            .unwrap();
        assert!(e.target.is_some(), "crate::graph::Graph should resolve");
    }

    #[test]
    fn external_crate_and_missing_item_stay_unresolved() {
        let edges = resolved(&[(
            "src/lib.rs",
            "use anyhow::Result;\nuse crate::nope::Gone;\npub fn f() {}\n",
        )]);
        assert!(edges.iter().all(|e| e.target.is_none()));
    }

    #[test]
    fn super_and_pub_use_hop_resolve() {
        let edges = resolved(&[
            (
                "src/lib.rs",
                "pub mod inner;\npub struct Root;\npub use inner::Leaf;\n",
            ),
            (
                "src/inner.rs",
                "use super::Root;\npub struct Leaf;\npub fn g(_r: Root) {}\n",
            ),
            ("src/other.rs", "use crate::Leaf;\npub fn h(_l: Leaf) {}\n"),
        ]);
        // `super::Root` from src/inner.rs -> src/lib.rs::Root
        assert!(edges.iter().any(|e| e.from_file == "src/inner.rs"
            && e.imported_name == "Root"
            && e.target.is_some()));
        // `use crate::Leaf` -> follows lib.rs's `pub use inner::Leaf`
        assert!(edges.iter().any(|e| e.from_file == "src/other.rs"
            && e.imported_name == "Leaf"
            && e.target.is_some()));
    }

    #[test]
    fn resolve_imports_on_this_repo_produces_real_edges() {
        // The M14 dogfood: parse + resolve every src/*.rs, and expect the
        // internal `use crate::…` graph to be substantially resolved.
        let files: Vec<(String, String)> = std::fs::read_dir("src")
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("rs"))
            .map(|p| {
                let rel = format!("src/{}", p.file_name().unwrap().to_str().unwrap());
                (rel, std::fs::read_to_string(&p).unwrap())
            })
            .collect();
        let refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(p, s)| (p.as_str(), s.as_str()))
            .collect();
        let edges = resolved(&refs);

        let crate_edges: Vec<_> = edges
            .iter()
            .filter(|e| e.specifier.starts_with("crate::"))
            .collect();
        let hit = crate_edges.iter().filter(|e| e.target.is_some()).count();
        assert!(
            crate_edges.len() >= 30,
            "expected a lot of crate:: imports, got {}",
            crate_edges.len()
        );
        assert!(
            hit * 100 / crate_edges.len() >= 80,
            "expected >=80% of crate:: imports resolved, got {hit}/{}",
            crate_edges.len()
        );
        // A specific one we know exists.
        assert!(edges.iter().any(|e| e.imported_name == "Graph"
            && e.specifier == "crate::graph"
            && e.target.is_some()));
    }

    #[test]
    fn fully_qualified_crate_call_with_no_use_gets_an_edge() {
        // ARCHITECTURE.md open question 14 / M21.h: `crate::rust::extract_file`
        // called with no `use crate::rust;` -- exactly `stack.rs`'s own real
        // shape, measured pre-build. No `use` means the ordinary import scan
        // (`extract_imports`) sees nothing; this is the blind spot.
        let edges = resolved(&[
            ("src/rust.rs", "pub fn extract_file() -> u32 { 0 }\n"),
            (
                "src/stack.rs",
                "pub fn go() -> u32 { crate::rust::extract_file() }\n",
            ),
        ]);
        let e = edges
            .iter()
            .find(|e| e.from_file == "src/stack.rs" && e.imported_name == "extract_file")
            .expect("fully-qualified call should produce an edge");
        assert!(
            e.target.is_some(),
            "crate::rust::extract_file should resolve"
        );
        assert_eq!(e.specifier, "crate::rust");
    }

    #[test]
    fn fully_qualified_type_one_segment_short_resolves_to_the_type_not_the_member() {
        // The measured "one segment short" shape: `crate::symbol::SymbolKind::Callable`
        // (an enum-variant / associated-item access) -- the real dependency is
        // `SymbolKind`, not a `Callable` item that doesn't exist as its own symbol.
        let edges = resolved(&[
            ("src/symbol.rs", "pub enum SymbolKind { Callable, Value }\n"),
            (
                "src/mcp.rs",
                "pub fn k() -> crate::symbol::SymbolKind { crate::symbol::SymbolKind::Callable }\n",
            ),
        ]);
        let e = edges
            .iter()
            .find(|e| e.from_file == "src/mcp.rs" && e.imported_name == "SymbolKind")
            .expect("qualified reference should target the type, not the variant");
        assert!(e.target.is_some());
        assert_eq!(e.specifier, "crate::symbol");
        assert!(
            !edges.iter().any(|e| e.imported_name == "Callable"),
            "no edge should target the nonexistent `Callable` item"
        );
    }

    #[test]
    fn same_file_fully_qualified_reference_is_not_a_blind_spot() {
        // The `bat` false start from measurement: a same-file fully-qualified
        // reference never needed a `use` in the first place, so it's not a
        // gap -- must not produce an edge (would be a spurious self-file dep).
        let edges = resolved(&[(
            "src/vscreen.rs",
            "pub struct Attributes;\nimpl Attributes {\n    \
             pub fn new() -> crate::vscreen::Attributes { Attributes }\n}\n",
        )]);
        assert!(
            edges.iter().all(|e| e.imported_name != "Attributes"),
            "a same-file qualified reference must not produce an edge"
        );
    }

    #[test]
    fn fully_qualified_reference_already_use_imported_is_not_duplicated() {
        // `get_callees` does no dedup of its own (mcp.rs) -- an explicit
        // `use` for a name must shadow the qualified-ref scan for the same
        // target, or the caller sees the same dependency listed twice.
        let edges = resolved(&[
            ("src/graph.rs", "pub struct Graph;\n"),
            (
                "src/app.rs",
                "use crate::graph::Graph;\n\
                 pub fn run() -> crate::graph::Graph { crate::graph::Graph }\n",
            ),
        ]);
        let hits: Vec<_> = edges
            .iter()
            .filter(|e| e.from_file == "src/app.rs" && e.imported_name == "Graph")
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "expected exactly one Graph edge, got {hits:?}"
        );
    }

    #[test]
    fn a_same_file_trait_impl_gets_a_synthetic_dependency_edge_to_its_type() {
        // M21.g's actual gap: `impl Add for Stats` never has a `use`
        // statement naming `Stats` -- same file, no import needed -- so
        // without a synthetic edge nothing in `graph.imports()` would ever
        // let a change to `Stats`'s own fields invalidate this impl's spec.
        // `Stats` has a real field, so this is the general case (M21.e's
        // fold leaves it alone; two documents is the correct end state).
        let edges = resolved(&[(
            "src/stats.rs",
            "pub struct Stats {\n    pub elapsed: u64,\n}\n\n\
             impl std::ops::Add for Stats {\n    \
             type Output = Stats;\n    \
             fn add(self, other: Stats) -> Stats {\n        \
             Stats { elapsed: self.elapsed + other.elapsed }\n    \
             }\n}\n",
        )]);
        let e = edges
            .iter()
            .find(|e| e.from_file == "src/stats.rs" && e.imported_name == "Stats")
            .expect("expected a synthetic dependency edge from the trait impl to its type");
        assert!(
            e.target.is_some(),
            "Stats should resolve to its own symbol in the same file"
        );
    }

    #[test]
    fn a_cross_file_trait_impl_already_resolves_via_its_own_use_statement() {
        // The other half of the same gap, already covered by the existing
        // `use`-based resolution -- no new code needed here, this only
        // confirms it. `impl Add for Stats` here names `Stats` through an
        // ordinary `use crate::stats::Stats;`, which `resolve_imports`
        // already turns into a real edge -- `dependency_hash` (spec.rs)
        // then finds it the same way it finds any other resolved import.
        let edges = resolved(&[
            (
                "src/stats.rs",
                "pub struct Stats {\n    pub elapsed: u64,\n}\n",
            ),
            (
                "src/add.rs",
                "use crate::stats::Stats;\n\n\
                 impl std::ops::Add for Stats {\n    \
                 type Output = Stats;\n    \
                 fn add(self, other: Stats) -> Stats {\n        \
                 Stats { elapsed: self.elapsed + other.elapsed }\n    \
                 }\n}\n",
            ),
        ]);
        let e = edges
            .iter()
            .find(|e| e.from_file == "src/add.rs" && e.imported_name == "Stats")
            .expect("expected the ordinary use-based edge from add.rs to Stats");
        assert!(e.target.is_some(), "Stats should resolve across files");
    }
}
