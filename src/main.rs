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
        /// The byte threshold above which a large class (or struct /
        /// interface / enum with members) is reduced to member signatures
        /// and docstrings instead of full method bodies, in a
        /// `get_next_spec_task` response — the "nicer" fix, tried first.
        /// Lower it if your MCP client still spills responses to a temp
        /// file at this default; raise it if you'd rather keep more body
        /// text and your client tolerates larger responses.
        #[arg(long, default_value_t = codeowl::spec::LARGE_CONTAINER_BYTES_DEFAULT)]
        large_class_bytes: usize,
        /// The hard byte ceiling applied to any single `get_next_spec_task`
        /// response (a class/symbol task *or* a plain file task), after
        /// whatever reduction already happened above. This is the actual
        /// guarantee — nothing returned can exceed it, regardless of which
        /// code path produced it. Set it below whatever size you've
        /// observed your MCP client spill to a `content.json`-style temp
        /// file instead of injecting inline (VS Code Copilot Chat does
        /// this above its own undocumented inline-context limit).
        #[arg(long, default_value_t = codeowl::spec::MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT)]
        max_spec_task_bytes: usize,
        /// The byte ceiling applied to a single `get_source` response.
        /// Deliberately separate from `max_spec_task_bytes` and much
        /// larger by default: that cap keeps a *generation task* small
        /// enough to author against, this one exists so a verification
        /// read gets the real text rather than a prompt-sized fragment.
        #[arg(long, default_value_t = codeowl::spec::MAX_SOURCE_TEXT_BYTES_DEFAULT)]
        max_source_bytes: usize,
        /// How many pages of raw file text (a page is
        /// `--max-spec-task-bytes`) a file summary is written from before
        /// the file task switches to a stand-in for the file. Raise it to
        /// read more raw text; lower it to switch sooner.
        #[arg(long, default_value_t = codeowl::spec::FILE_RAW_MAX_PAGES_DEFAULT, value_parser = at_least_one)]
        max_file_pages: usize,
        /// The fewest top-level functions and classes a too-big file needs
        /// before its summary is written from their written summaries
        /// instead of raw text. Lower it to use summaries sooner.
        #[arg(long, default_value_t = codeowl::spec::FILE_SUMMARIES_MIN_SYMBOLS_DEFAULT, value_parser = at_least_one)]
        summaries_min_symbols: usize,
        /// The share of a too-big file, in percent of its bytes, that one
        /// class must cover for its summary to be written from a member
        /// outline of that class. Raise it to use the outline less often.
        #[arg(long, default_value_t = codeowl::spec::FILE_OUTLINE_CONTAINER_PERCENT_DEFAULT, value_parser = percent)]
        outline_container_percent: usize,
    },
    /// Write the per-target dependency record into stored specs that predate
    /// it, so `explain_stale` can name the dependency that moved. Changes
    /// only the `dep_targets:` block of a spec's frontmatter, and only for
    /// owners whose stored `deps_hash` it can reproduce. Prints what it
    /// would do unless `--write` is given.
    BackfillDepTargets {
        path: PathBuf,
        /// Apply the changes; without it nothing is written.
        #[arg(long)]
        write: bool,
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
        Command::BackfillDepTargets { path, write } => {
            let root = canonical_root(&path)?;
            let graph = RepoIndex::build(&root)?.rebuild()?;
            let r = codeowl::spec::backfill_dep_targets(&graph, &root, write)?;
            let verb = if write { "wrote" } else { "would write" };
            println!(
                "{verb} records for {} owners in {} spec files",
                r.filled, r.files_changed
            );
            println!(
                "skipped: {} whose dependencies moved since the spec was written, \
                 {} that cannot be checked (symbol gone or source unreadable), \
                 {} already recorded, {} with no dependencies",
                r.skipped_moved, r.unverifiable, r.already_recorded, r.without_dependencies
            );
            if r.unreadable_files > 0 {
                println!(
                    "{} spec file(s) could not be read and were left alone",
                    r.unreadable_files
                );
            }
            if !write && r.filled > 0 {
                println!("dry run: nothing written; pass --write to apply");
            }
            Ok(())
        }
        Command::Serve {
            path,
            large_class_bytes,
            max_spec_task_bytes,
            max_source_bytes,
            max_file_pages,
            summaries_min_symbols,
            outline_container_percent,
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
                .with_generation_limits(Some(large_class_bytes), Some(max_spec_task_bytes))
                .with_source_limit(Some(max_source_bytes))
                .with_file_ladder(codeowl::spec::FileLadder {
                    max_raw_pages: max_file_pages,
                    summaries_min_symbols,
                    outline_container_percent,
                });
            // Keep the handle bound for the whole session. Dropping it only
            // detaches the watcher thread, which runs until the process
            // exits, so this is a keep-alive and not a stop switch.
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

/// clap value parser: a whole number of at least 1.
fn at_least_one(raw: &str) -> Result<usize, String> {
    match raw.parse::<usize>() {
        Ok(n) if n >= 1 => Ok(n),
        _ => Err(format!("{raw:?} is not a whole number of at least 1")),
    }
}

/// clap value parser: a whole percent from 1 to 100.
fn percent(raw: &str) -> Result<usize, String> {
    match raw.parse::<usize>() {
        Ok(n) if (1..=100).contains(&n) => Ok(n),
        _ => Err(format!("{raw:?} is not a whole percent from 1 to 100")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_serve(extra: &[&str]) -> Result<Command, clap::Error> {
        let mut args = vec!["codeowl", "serve", "/some/repo"];
        args.extend_from_slice(extra);
        Cli::try_parse_from(args).map(|cli| cli.command)
    }

    #[test]
    fn serve_ladder_flags_default_to_the_named_constants() {
        let Ok(Command::Serve {
            max_file_pages,
            summaries_min_symbols,
            outline_container_percent,
            ..
        }) = parse_serve(&[])
        else {
            panic!("serve with no extra flags must parse");
        };
        assert_eq!(max_file_pages, codeowl::spec::FILE_RAW_MAX_PAGES_DEFAULT);
        assert_eq!(
            summaries_min_symbols,
            codeowl::spec::FILE_SUMMARIES_MIN_SYMBOLS_DEFAULT
        );
        assert_eq!(
            outline_container_percent,
            codeowl::spec::FILE_OUTLINE_CONTAINER_PERCENT_DEFAULT
        );
    }

    #[test]
    fn serve_ladder_flags_can_be_overridden() {
        let Ok(Command::Serve {
            max_file_pages,
            summaries_min_symbols,
            outline_container_percent,
            ..
        }) = parse_serve(&[
            "--max-file-pages",
            "20",
            "--summaries-min-symbols",
            "15",
            "--outline-container-percent",
            "60",
        ])
        else {
            panic!("overrides must parse");
        };
        assert_eq!(
            (
                max_file_pages,
                summaries_min_symbols,
                outline_container_percent
            ),
            (20, 15, 60)
        );
    }

    #[test]
    fn serve_rejects_out_of_range_ladder_values_at_startup() {
        for bad in [
            ["--max-file-pages", "0"],
            ["--summaries-min-symbols", "0"],
            ["--outline-container-percent", "0"],
            ["--outline-container-percent", "101"],
        ] {
            assert!(parse_serve(&bad).is_err(), "{bad:?} must be rejected");
        }
    }
}
