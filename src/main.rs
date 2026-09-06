use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use codeowl::graph::extract_and_hash;
use codeowl::mcp::CodeOwlServer;
use codeowl::{Graph, build_resolver, extract_imports, resolve_imports};
use rmcp::ServiceExt;

#[derive(Parser)]
#[command(name = "codeowl")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Walk a repo, extract symbols, resolve imports, and print the
    /// resulting symbol list as JSON.
    Extract { path: PathBuf },
    /// Same extraction, then serve the MCP read surface over stdio.
    Serve { path: PathBuf },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Extract { path } => {
            let root = canonical_root(&path)?;
            let graph = build_graph(&root)?;
            let views: Vec<_> = graph
                .symbols()
                .filter_map(|s| graph.find(&s.id))
                .filter_map(|id| codeowl::graph::SymbolView::from_graph(&graph, id))
                .collect();
            println!("{}", serde_json::to_string_pretty(&views)?);
            Ok(())
        }
        Command::Serve { path } => {
            let root = canonical_root(&path)?;
            let graph = build_graph(&root)?;
            let server = CodeOwlServer::new(root, graph);
            let running = server
                .serve(rmcp::transport::io::stdio())
                .await
                .context("starting MCP server")?;
            running.waiting().await.context("MCP server exited")?;
            Ok(())
        }
    }
}

/// Canonicalize `path` to an absolute root before anything else touches
/// it. `resolve.rs` strips this same root off `oxc_resolver`'s (always
/// absolute) resolutions to recover a repo-relative path — passed a
/// relative root like `.`, that `strip_prefix` silently fails for every
/// single import (an absolute path never has a relative path as a
/// component prefix), so every import resolves to `None` with no error at
/// all. `canonicalize` also resolves symlinks, keeping the graph's path
/// scheme consistent regardless of how the caller invoked us.
fn canonical_root(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("resolving {} to an absolute path", path.display()))
}

/// Walk `root`, extract every file's symbols and named imports, resolve
/// those imports against the resulting graph, and persist the result to
/// `.codeowl/graph` (in `root`, not CodeOwl's own repo — see
/// `ARCHITECTURE.md`'s "Storage"). Shared by both subcommands: `serve`
/// needs exactly the same graph `extract` prints, just handed to a running
/// server instead of stdout.
fn build_graph(root: &Path) -> Result<Graph> {
    let mut extractions = Vec::new();
    let mut file_imports = HashMap::new();
    let mut route_literals = Vec::new();
    for entry in ignore::WalkBuilder::new(root).build() {
        let entry = entry.context("walking repo")?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if !is_extractable(path) {
            continue;
        }

        let source =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        file_imports.insert(rel.clone(), extract_imports(&source, &rel));
        route_literals.extend(codeowl::features::extract_route_literals(&source, &rel));
        extractions.push(extract_and_hash(&rel, &source));
    }

    let mut graph = Graph::build(extractions);
    let resolver = build_resolver();
    let resolved = resolve_imports(root, &resolver, &file_imports, &graph);
    let resolved_count = resolved.iter().filter(|r| r.target.is_some()).count();
    let total_count = resolved.len();
    graph.set_resolved_imports(resolved);
    graph.set_route_literals(route_literals);

    graph.save(&root.join(".codeowl").join("graph"))?;
    eprintln!("resolved {resolved_count}/{total_count} named imports");

    Ok(graph)
}

/// `.ts`/`.tsx` only, and never `.d.ts` — ambient declaration files use
/// different grammar shapes (`declare function`, etc.) that M1 doesn't
/// handle; see `ROADMAP.md`'s M1 scope.
fn is_extractable(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if name.ends_with(".d.ts") {
        return false;
    }
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts") | Some("tsx")
    )
}
