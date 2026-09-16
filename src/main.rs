use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use codeowl::index::RepoIndex;
use codeowl::mcp::CodeOwlServer;
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
    Serve {
        path: PathBuf,
        /// Override the byte threshold above which a `get_next_spec_task`
        /// Container is reduced to member signatures + docstrings instead
        /// of full bodies (default: see `spec::LARGE_CONTAINER_BYTES_DEFAULT`).
        /// Lower this if your MCP client's inline-context limit is
        /// smaller than the default was tuned for.
        #[arg(long)]
        large_container_bytes: Option<usize>,
        /// Override the hard byte ceiling on any single
        /// `get_next_spec_task` `source` field, applied after whatever
        /// reduction already happened (default: see
        /// `spec::MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT`). This is what
        /// actually guarantees no response exceeds your client's real
        /// limit — set it below whatever size you've observed your
        /// client spill to a temp file instead of injecting inline.
        #[arg(long)]
        max_generation_bytes: Option<usize>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Extract { path } => {
            let root = canonical_root(&path)?;
            let graph = RepoIndex::build(&root)?.rebuild()?;
            let views: Vec<_> = graph
                .symbols()
                .filter_map(|s| graph.find(&s.id))
                .filter_map(|id| codeowl::graph::SymbolView::from_graph(&graph, id))
                .collect();
            println!("{}", serde_json::to_string_pretty(&views)?);
            Ok(())
        }
        Command::Serve {
            path,
            large_container_bytes,
            max_generation_bytes,
        } => {
            let root = canonical_root(&path)?;
            let (index, graph, caught) = RepoIndex::open(&root)?;
            let resolved = graph
                .imports()
                .iter()
                .filter(|r| r.target.is_some())
                .count();
            eprintln!(
                "resolved {resolved}/{} named imports",
                graph.imports().len()
            );
            if !caught.is_empty() {
                eprintln!(
                    "catch-up: reindexed {} file(s) changed since last run",
                    caught.total()
                );
            }
            let server = CodeOwlServer::new(root.clone(), graph)
                .with_generation_limits(large_container_bytes, max_generation_bytes);
            // Keep the watcher alive for the whole session — it stops when
            // this handle drops, which is when `serve` returns.
            let _watcher = codeowl::watch::spawn(root, server.graph_store(), index)
                .context("starting the file watcher")?;
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
