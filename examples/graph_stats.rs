//! Structural statistics for the graph CodeOwl builds today: node and edge
//! counts, degree distributions, hubs, connected components, cycles, and
//! how far apart files sit. Run with
//! `cargo run --example graph_stats -- <repo> [<repo> ...]`.
//!
//! File-level view: every resolved import becomes one directed edge
//! `importing file -> file that owns the target symbol`, deduplicated, with
//! self-edges dropped. That is the graph the feature layer and the
//! staleness cascade actually walk.

use codeowl::graph::{FlowTarget, Graph};
use codeowl::index::RepoIndex;
use codeowl::symbol::SymbolKind;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;

fn pct(sorted: &[usize], p: f64) -> usize {
    if sorted.is_empty() {
        return 0;
    }
    sorted[(((sorted.len() - 1) as f64) * p).round() as usize]
}

fn top(label: &str, deg: &HashMap<usize, usize>, names: &[String], n: usize) {
    let mut v: Vec<_> = deg.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1).then(names[*a.0].cmp(&names[*b.0])));
    println!("  top {label}:");
    for (i, d) in v.into_iter().take(n) {
        println!("    {d:>4}  {}", names[*i]);
    }
}

/// Tarjan's strongly connected components, iterative so a deep import chain
/// can't overflow the stack. Returns each component's node list.
fn sccs(adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adj.len();
    let (mut index, mut low) = (vec![usize::MAX; n], vec![0usize; n]);
    let (mut on, mut stack, mut out) = (vec![false; n], Vec::new(), Vec::new());
    let mut counter = 0;
    for root in 0..n {
        if index[root] != usize::MAX {
            continue;
        }
        let mut work = vec![(root, 0usize)];
        while let Some(&(v, ci)) = work.last() {
            if ci == 0 {
                index[v] = counter;
                low[v] = counter;
                counter += 1;
                stack.push(v);
                on[v] = true;
            }
            if ci < adj[v].len() {
                work.last_mut().unwrap().1 += 1;
                let w = adj[v][ci];
                if index[w] == usize::MAX {
                    work.push((w, 0));
                } else if on[w] {
                    low[v] = low[v].min(index[w]);
                }
            } else {
                work.pop();
                if let Some(&(p, _)) = work.last() {
                    low[p] = low[p].min(low[v]);
                }
                if low[v] == index[v] {
                    let mut comp = Vec::new();
                    loop {
                        let w = stack.pop().unwrap();
                        on[w] = false;
                        comp.push(w);
                        if w == v {
                            break;
                        }
                    }
                    out.push(comp);
                }
            }
        }
    }
    out
}

fn report(label: &str, root: &Path) {
    let Ok((_index, graph, _)) = RepoIndex::open(root) else {
        println!("== {label}: failed to open ==");
        return;
    };
    println!("== {label} ({}) ==", graph.pack_name());
    nodes_and_containment(&graph);
    let names: Vec<String> = graph.files().map(|f| f.id.clone()).collect();
    let ix: HashMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    file_graph(&graph, &names, &ix);
}

fn nodes_and_containment(graph: &Graph) {
    let files = graph.files().count();
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_raw: BTreeMap<String, usize> = BTreeMap::new();
    for s in graph.symbols() {
        *by_kind.entry(format!("{:?}", s.kind)).or_default() += 1;
        *by_raw.entry(s.raw.clone()).or_default() += 1;
    }
    let symbols: usize = by_kind.values().sum();
    println!("  nodes: {} files + {symbols} symbols", files);
    println!("  by kind: {by_kind:?}");
    let mut raw: Vec<_> = by_raw.into_iter().collect();
    raw.sort_by_key(|r| std::cmp::Reverse(r.1));
    println!("  by raw (top 8): {:?}", &raw[..raw.len().min(8)]);

    // Containment: children per container, and how deep nesting goes.
    let mut kids: Vec<usize> = graph
        .symbols()
        .filter(|s| s.kind == SymbolKind::Container)
        .map(|s| s.children.len())
        .collect();
    kids.sort_unstable();
    let per_file: Vec<usize> = {
        let mut v: Vec<usize> = graph.files().map(|f| f.children.len()).collect();
        v.sort_unstable();
        v
    };
    println!(
        "  top-level symbols per file: median {}, p90 {}, max {}",
        pct(&per_file, 0.5),
        pct(&per_file, 0.9),
        per_file.last().copied().unwrap_or(0)
    );
    println!(
        "  members per container: median {}, p90 {}, max {} (n={})",
        pct(&kids, 0.5),
        pct(&kids, 0.9),
        kids.last().copied().unwrap_or(0),
        kids.len()
    );
    let exported = graph.symbols().filter(|s| s.is_exported).count();
    println!("  exported symbols: {exported} of {symbols}");
}

fn file_graph(graph: &Graph, names: &[String], ix: &HashMap<&str, usize>) {
    let raw_imports = graph.imports().len();
    let unresolved = graph
        .imports()
        .iter()
        .filter(|e| e.target.is_none())
        .count();
    println!("  import statements: {raw_imports} ({unresolved} external/unresolved)");
    let mut flow: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for e in graph.flow_edges() {
        let ent = flow.entry(e.kind.clone()).or_default();
        ent.0 += 1;
        if matches!(e.target, FlowTarget::Node(_)) {
            ent.1 += 1;
        }
    }
    if !flow.is_empty() {
        println!("  flow edges (kind: total, resolved): {flow:?}");
    }

    let n = names.len();
    let mut edges: HashSet<(usize, usize)> = HashSet::new();
    for e in graph.imports() {
        let Some(t) = e.target else { continue };
        let (Some(&a), Some(&b)) = (
            ix.get(e.from_file.as_str()),
            ix.get(graph.owning_file_id(t)),
        ) else {
            continue;
        };
        if a != b {
            edges.insert((a, b));
        }
    }
    let mut adj = vec![Vec::new(); n];
    let mut und = vec![Vec::new(); n];
    let (mut fan_in, mut fan_out): (HashMap<usize, usize>, HashMap<usize, usize>) =
        (HashMap::new(), HashMap::new());
    for &(a, b) in &edges {
        adj[a].push(b);
        und[a].push(b);
        und[b].push(a);
        *fan_out.entry(a).or_default() += 1;
        *fan_in.entry(b).or_default() += 1;
    }
    let density = if n > 1 {
        edges.len() as f64 / (n * (n - 1)) as f64
    } else {
        0.0
    };
    println!(
        "  file graph: {n} nodes, {} directed edges, density {density:.4}, mean degree {:.2}",
        edges.len(),
        if n == 0 {
            0.0
        } else {
            edges.len() as f64 / n as f64
        }
    );
    let isolated = (0..n).filter(|i| und[*i].is_empty()).count();
    println!("  isolated files (no edges either way): {isolated}");

    let mut fi: Vec<usize> = (0..n).map(|i| *fan_in.get(&i).unwrap_or(&0)).collect();
    let mut fo: Vec<usize> = (0..n).map(|i| *fan_out.get(&i).unwrap_or(&0)).collect();
    fi.sort_unstable();
    fo.sort_unstable();
    println!(
        "  fan-in : median {}, p90 {}, max {}  | fan-out: median {}, p90 {}, max {}",
        pct(&fi, 0.5),
        pct(&fi, 0.9),
        fi.last().copied().unwrap_or(0),
        pct(&fo, 0.5),
        pct(&fo, 0.9),
        fo.last().copied().unwrap_or(0)
    );
    // How concentrated is the graph: share of all edges pointing at the top
    // 10% most-imported files.
    let tenth = (n / 10).max(1);
    let top_share: usize = fi.iter().rev().take(tenth).sum();
    if !edges.is_empty() {
        println!(
            "  top 10% most-imported files receive {:.0}% of all edges",
            100.0 * top_share as f64 / edges.len() as f64
        );
    }
    top("fan-in (most imported)", &fan_in, names, 5);
    top("fan-out (imports most)", &fan_out, names, 5);

    // Weakly connected components.
    let mut comp = vec![usize::MAX; n];
    let mut sizes = Vec::new();
    for s in 0..n {
        if comp[s] != usize::MAX {
            continue;
        }
        let id = sizes.len();
        let mut q = VecDeque::from([s]);
        comp[s] = id;
        let mut size = 0;
        while let Some(v) = q.pop_front() {
            size += 1;
            for &w in &und[v] {
                if comp[w] == usize::MAX {
                    comp[w] = id;
                    q.push_back(w);
                }
            }
        }
        sizes.push(size);
    }
    let mut sorted = sizes.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    let multi = sorted.iter().filter(|s| **s > 1).count();
    println!(
        "  connected components: {} total, {multi} with >1 file, largest {:?}",
        sizes.len(),
        &sorted[..sorted.len().min(5)]
    );

    // Distance inside the largest component (undirected, BFS from every node).
    if let Some((big, &bs)) = sizes
        .iter()
        .enumerate()
        .max_by_key(|(_, s)| **s)
        .filter(|&(_, &s)| s > 1 && s <= 2500)
    {
        let members: Vec<usize> = (0..n).filter(|i| comp[*i] == big).collect();
        let (mut total, mut pairs, mut diameter) = (0usize, 0usize, 0usize);
        for &s in &members {
            let mut dist = HashMap::from([(s, 0usize)]);
            let mut q = VecDeque::from([s]);
            while let Some(v) = q.pop_front() {
                let d = dist[&v];
                for &w in &und[v] {
                    if let Entry::Vacant(e) = dist.entry(w) {
                        e.insert(d + 1);
                        q.push_back(w);
                    }
                }
            }
            for (&t, &d) in &dist {
                if t != s {
                    total += d;
                    pairs += 1;
                    diameter = diameter.max(d);
                }
            }
        }
        println!(
            "  largest component ({bs} files): average shortest path {:.2} hops, diameter {diameter}",
            total as f64 / pairs.max(1) as f64
        );
    }

    // Cycles: strongly connected components of size > 1.
    let cyc: Vec<Vec<usize>> = sccs(&adj).into_iter().filter(|c| c.len() > 1).collect();
    let in_cycles: usize = cyc.iter().map(|c| c.len()).sum();
    let mut cs: Vec<usize> = cyc.iter().map(|c| c.len()).collect();
    cs.sort_unstable_by(|a, b| b.cmp(a));
    println!(
        "  import cycles: {} strongly connected groups, {in_cycles} files involved, sizes {:?}",
        cyc.len(),
        &cs[..cs.len().min(5)]
    );
    if let Some(big) = cyc.iter().max_by_key(|c| c.len()) {
        let mut m: Vec<&str> = big.iter().map(|i| names[*i].as_str()).collect();
        m.sort_unstable();
        println!("    largest cycle group: {:?}", &m[..m.len().min(8)]);
    }
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        report("CodeOwl itself", Path::new("."));
        return;
    }
    for a in &args {
        report(a, Path::new(a));
    }
}
