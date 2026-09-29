//! The cost-table + correctness-table measurement CodeOwl runs before
//! deciding whether a new kind of declaration should fold into a
//! container's `interface_hash` -- built for M21.a (`ARCHITECTURE.md` open
//! question 12, fields) and reused as-is for M21.i (open question 13,
//! method signatures). Kept as a runnable example rather than thrown away
//! after use (owner's call, 2026-09-29) because this is now a two-time
//! playbook, not a one-off: `SIGNAL` below is the one thing to change for
//! the next such question.
//!
//! Run with `cargo run --example measure_signature_fold` from the repo
//! root. Points at four fixed local paths (the same pilot repos M21.a
//! measured) -- edit `main` below if they've moved, or you're measuring a
//! different signal on different repos.
//!
//! **Cost table:** for every exported container with at least one real,
//! publicly-visible member matching `SIGNAL`, its current real importer
//! count -- exactly how many files would be newly invalidated per edit
//! under the folded rule, computed directly off the real resolved-import
//! graph, never simulated.
//!
//! **Correctness table:** of the files that already have a generated spec
//! and import one of those containers, what fraction actually name one of
//! the container's own qualifying member names in prose -- a *necessary*
//! precondition for that prose to go false under a rename/removal, not a
//! sufficient one (see the printed per-hit lines: read them before trusting
//! the percentage, the same correction M21.a's own writeup had to make).

use codeowl::graph::{Graph, SymbolId};
use codeowl::index::RepoIndex;
use codeowl::symbol::{Symbol, SymbolKind};
use std::collections::HashSet;
use std::path::Path;

/// The one thing to change for a different open question: which kind of
/// member to measure, and each pack's own "is this member's own signature
/// publicly visible" check. Not claiming byte-identical behavior to the
/// real extractor's own visibility helpers (Rust's `has_pub`, Java's
/// `has_public_or_protected`, TS's default-public/`private`/`#`-name rule,
/// Python's leading-underscore convention) -- a measurement script only
/// needs to be right often enough to produce an honest table, not to ship.
const SIGNAL: SymbolKind = SymbolKind::Callable;

fn is_qualifying_member(pack: &str, sym: &Symbol) -> bool {
    if sym.kind != SIGNAL {
        return false;
    }
    let sig = sym.signature.trim_start();
    match pack {
        "rust" => sig.starts_with("pub "),
        "java" => sig.starts_with("public ") || sig.contains(" public "),
        "typescript-next" => {
            let name = sym.id.rsplit("::").next().unwrap_or(&sym.id);
            !sig.starts_with("private") && !name.starts_with('#')
        }
        "python" => {
            let name = sym.id.rsplit("::").next().unwrap_or(&sym.id);
            !name.starts_with('_')
        }
        _ => false,
    }
}

/// Skip a member name too generic/short to be real signal if it showed up
/// in prose written about something else entirely -- the same noise filter
/// M21.a applied to field names (`id`/`kind`/`file`). M21.i found this list
/// needs to be *longer* for methods than it was for fields: a method name
/// collides with an ordinary parameter/local-variable name far more often
/// than a field name does (`parent_id` was the concrete case -- a real
/// `Graph` method, and also the near-universal parameter name every pack's
/// own extraction functions take).
const GENERIC_NAMES: &[&str] = &[
    "get",
    "new",
    "build",
    "open",
    "save",
    "load",
    "of",
    "len",
    "is_empty",
    "empty",
    "total",
    "set",
    "run",
    "init",
    "close",
    "to_string",
    "toString",
    "equals",
    "hashCode",
    "clone",
    "files",
    "file",
    "symbols",
    "imports",
    "children",
    "content",
    "name",
    "id",
    "value",
    "values",
    "data",
    "list",
    "size",
    "count",
    "type",
    "path",
    "read",
    "write",
    "process",
    "handle",
    "create",
    "update",
    "delete",
    "find",
    "search",
    "parse",
    "parent_id",
];

fn qualifying_member_names(graph: &Graph, pack: &str, container_id: SymbolId) -> Vec<String> {
    graph
        .symbols()
        .filter(|s| s.parent == Some(container_id) && is_qualifying_member(pack, s))
        .map(|s| s.id.rsplit("::").next().unwrap_or(&s.id).to_string())
        .filter(|n| !GENERIC_NAMES.contains(&n.as_str()) && n.len() > 2)
        .collect()
}

fn mentions_word(text: &str, word: &str) -> bool {
    let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(pos) = text[i..].find(word) {
        let start = i + pos;
        let end = start + word.len();
        let before_ok = start == 0 || !is_ident(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_ident(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        i = start + 1;
        if i >= bytes.len() {
            break;
        }
    }
    false
}

/// `SIGNAL == Callable` only: how often does a container fold a name onto
/// more than one real member? A field almost never collides this way; a
/// method does, wherever a language allows overloading (Java) -- and
/// that's exactly the shape that makes "one shared symbol id" ambiguous
/// about which of several real declarations a hash actually covers.
fn report_name_collisions(graph: &Graph) {
    if SIGNAL != SymbolKind::Callable {
        return;
    }
    for container in graph.symbols().filter(|s| s.kind == SymbolKind::Container) {
        let Some(cid) = graph.find(&container.id) else {
            continue;
        };
        let mut by_name: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for s in graph
            .symbols()
            .filter(|s| s.kind == SymbolKind::Callable && s.parent == Some(cid))
        {
            let name = s.id.rsplit("::").next().unwrap_or(&s.id);
            *by_name.entry(name).or_insert(0) += 1;
        }
        let collisions: Vec<(&str, usize)> = by_name.into_iter().filter(|(_, n)| *n > 1).collect();
        if !collisions.is_empty() {
            println!(
                "  {} has overloaded/colliding names: {collisions:?}",
                container.id
            );
        }
    }
}

fn correctness_check(
    root: &Path,
    label: &str,
    qualifying: &[(String, Vec<String>, HashSet<String>)],
) {
    let mut files_could_mention = 0usize;
    let mut files_actually_mention = 0usize;
    let mut checked: HashSet<String> = HashSet::new();
    for (_container_id, member_names, importers) in qualifying {
        if member_names.is_empty() {
            continue;
        }
        for from_file in importers {
            if !checked.insert(from_file.clone()) {
                continue; // a file importing >1 qualifying container is checked once
            }
            let spec_path = root.join("docs/specs").join(format!("{from_file}.md"));
            let Ok(text) = std::fs::read_to_string(&spec_path) else {
                continue;
            };
            files_could_mention += 1;
            let hits: Vec<&String> = member_names
                .iter()
                .filter(|m| mentions_word(&text, m))
                .collect();
            if !hits.is_empty() {
                files_actually_mention += 1;
                for m in &hits {
                    for line in text.lines().filter(|l| mentions_word(l, m)) {
                        println!("    [{label}] {from_file} names `{m}`: {}", line.trim());
                    }
                }
            }
        }
    }
    let rate = if files_could_mention == 0 {
        0.0
    } else {
        100.0 * files_actually_mention as f64 / files_could_mention as f64
    };
    println!(
        "  [{label}] files with a spec that could mention a qualifying member: {files_could_mention}, actually names one: {files_actually_mention} ({rate:.0}%)"
    );
}

fn measure(label: &str, root: &Path) {
    let Ok((_index, graph, _catch_up)) = RepoIndex::open(root) else {
        println!("== {label}: failed to open ==");
        return;
    };
    let pack = graph.pack_name().to_string();
    println!("== {label} ({pack}) ==");

    let mut qualifying: Vec<(String, usize)> = Vec::new(); // (container id, importer count)
    let mut for_correctness: Vec<(String, Vec<String>, HashSet<String>)> = Vec::new();
    let mut total_qualifying_containers = 0usize;

    for container in graph.symbols().filter(|s| s.kind == SymbolKind::Container) {
        let Some(container_id) = graph.find(&container.id) else {
            continue;
        };
        let has_qualifying_member = graph
            .symbols()
            .any(|s| s.parent == Some(container_id) && is_qualifying_member(&pack, s));
        if !has_qualifying_member {
            continue;
        }
        total_qualifying_containers += 1;

        let importers: HashSet<&str> = graph
            .imports()
            .iter()
            .filter(|e| e.target == Some(container_id))
            .map(|e| e.from_file.as_str())
            .collect();
        if !importers.is_empty() {
            qualifying.push((container.id.clone(), importers.len()));
            for_correctness.push((
                container.id.clone(),
                qualifying_member_names(&graph, &pack, container_id),
                importers.into_iter().map(str::to_string).collect(),
            ));
        }
    }

    qualifying.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("  containers w/ a real qualifying member: {total_qualifying_containers}");
    println!("  ...with real importers today: {}", qualifying.len());
    let top5: Vec<usize> = qualifying.iter().take(5).map(|(_, n)| *n).collect();
    println!("  folded-rule importers per edit (top 5): {top5:?}");
    let sum: usize = qualifying.iter().map(|(_, n)| n).sum();
    let avg = if qualifying.is_empty() {
        0.0
    } else {
        sum as f64 / qualifying.len() as f64
    };
    println!("  sum if every one had one member edited: {sum}");
    println!("  average: {avg:.1}");
    if let Some((id, n)) = qualifying.first() {
        println!("  worst single case: {id} ({n} importers)");
    }
    report_name_collisions(&graph);
    correctness_check(root, label, &for_correctness);
}

fn main() {
    measure("CodeOwl itself", Path::new("."));
    measure(
        "full-stack-fastapi-template",
        Path::new("/home/sidd/dev/openSource/test-repos/full-stack-fastapi-template/backend"),
    );
    measure(
        "quarkus-super-heroes",
        Path::new("/home/sidd/dev/openSource/test-repos/quarkus-super-heroes"),
    );
    measure(
        "talentTrail",
        Path::new("/home/sidd/dev/startup/talentTrail"),
    );
}
