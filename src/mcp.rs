//! The MCP surface — read tools from M3, plus M4's write side driving
//! generation. `get_spec`/`get_symbol`/`get_callers`/`get_callees`/
//! `search_code` are pure reads, always: none of them ever writes
//! anything or calls an LLM — see `CLAUDE.md`'s hard invariants.
//! `get_next_spec_task`/`submit_spec` are the two calls the *client's* own
//! LLM drives (never CodeOwl itself — it holds no credentials) via
//! `/codeowl generate`; see `ARCHITECTURE.md`'s "Who actually writes the
//! spec text".

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::graph::{Graph, SymbolView};
use crate::search::SearchMatch;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IdRequest {
    /// A symbol's stable id, e.g. "lib/utils.ts::cn".
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchRequest {
    /// A regex pattern to search for across the repo's source files.
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct CallerInfo {
    pub from_file: String,
    pub imported_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct CalleeInfo {
    pub specifier: String,
    pub imported_name: String,
    /// The imported symbol's stable id, when it resolved to one CodeOwl
    /// tracks. `None` covers both external packages and internal names
    /// M1 doesn't extract as symbols yet (a `type`/`interface`, a
    /// destructured const) — see M2's real-repo validation notes.
    pub resolved_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SpecResponse {
    pub id: String,
    /// One of `"missing"` | `"current"`. `"stale"` doesn't exist until M6
    /// (staleness/invalidation) lands — see `ARCHITECTURE.md`'s "Ordering".
    pub status: String,
    pub signature: String,
    pub docstring: Option<String>,
    /// The LLM-written prose `submit_spec` persisted, when `status` is
    /// `"current"`. `None` while `status` is `"missing"`.
    pub content: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GenerateTaskRequest {
    /// The `/codeowl generate <id>` target — currently must be a file id
    /// (a repo-relative path, e.g. "lib/utils.ts"). Stateless: safe to call
    /// repeatedly with the same target until it reports nothing left.
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpecTaskResponse {
    Symbol {
        id: String,
        signature: String,
        docstring: Option<String>,
        source: String,
        dependencies: Vec<String>,
    },
    File {
        id: String,
        source: String,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubmitSpecRequest {
    /// The id `get_next_spec_task` returned — a symbol or file id.
    pub id: String,
    /// For a symbol task: markdown containing `### Summary` and
    /// `### Behavior` headings. For a file task: plain prose, becomes the
    /// file's `## Summary`.
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SubmitSpecResponse {
    pub id: String,
    pub source_hash: String,
    pub spec_hash: String,
}

#[derive(Clone)]
pub struct CodeOwlServer {
    graph: Arc<Graph>,
    root: Arc<PathBuf>,
    tool_router: ToolRouter<Self>,
}

impl CodeOwlServer {
    pub fn new(root: PathBuf, graph: Graph) -> Self {
        Self {
            graph: Arc::new(graph),
            root: Arc::new(root),
            tool_router: Self::tool_router(),
        }
    }

    fn not_found(id: &str) -> String {
        format!("no symbol with id {id:?}")
    }
}

/// Slice `rel_path`'s raw text down to `lines` (1-indexed, inclusive) —
/// what `get_next_spec_task` hands the agent as a symbol task's own
/// source, per `ARCHITECTURE.md`'s "Bottom-up composition".
fn read_lines(
    root: &std::path::Path,
    rel_path: &str,
    lines: [usize; 2],
) -> std::io::Result<String> {
    let content = std::fs::read_to_string(root.join(rel_path))?;
    let [start, end] = lines;
    Ok(content
        .lines()
        .skip(start.saturating_sub(1))
        .take(end + 1 - start)
        .collect::<Vec<_>>()
        .join("\n"))
}

#[tool_router]
impl CodeOwlServer {
    #[tool(
        description = "Look up one symbol's full record (signature, docstring, line range, hashes) by its stable id, e.g. \"lib/utils.ts::cn\"."
    )]
    async fn get_symbol(
        &self,
        Parameters(req): Parameters<IdRequest>,
    ) -> Result<Json<SymbolView>, String> {
        let id = self
            .graph
            .find(&req.id)
            .ok_or_else(|| Self::not_found(&req.id))?;
        SymbolView::from_graph(&self.graph, id)
            .map(Json)
            .ok_or_else(|| Self::not_found(&req.id))
    }

    #[tool(
        description = "List every file that imports this symbol by name, via a resolved reference edge (M2)."
    )]
    async fn get_callers(
        &self,
        Parameters(req): Parameters<IdRequest>,
    ) -> Result<Json<Vec<CallerInfo>>, String> {
        let id = self
            .graph
            .find(&req.id)
            .ok_or_else(|| Self::not_found(&req.id))?;
        let callers = self
            .graph
            .imports()
            .iter()
            .filter(|imp| imp.target == Some(id))
            .map(|imp| CallerInfo {
                from_file: imp.from_file.clone(),
                imported_name: imp.imported_name.clone(),
            })
            .collect();
        Ok(Json(callers))
    }

    #[tool(
        description = "List what the FILE containing this symbol imports. File-level granularity, not per-symbol: M2 resolves file-to-file reference edges, not call edges (see ROADMAP.md's M2 scope)."
    )]
    async fn get_callees(
        &self,
        Parameters(req): Parameters<IdRequest>,
    ) -> Result<Json<Vec<CalleeInfo>>, String> {
        let id = self
            .graph
            .find(&req.id)
            .ok_or_else(|| Self::not_found(&req.id))?;
        let file = self
            .graph
            .get_symbol(id)
            .ok_or_else(|| Self::not_found(&req.id))?
            .file
            .clone();
        let callees = self
            .graph
            .imports()
            .iter()
            .filter(|imp| imp.from_file == file)
            .map(|imp| CalleeInfo {
                specifier: imp.specifier.clone(),
                imported_name: imp.imported_name.clone(),
                resolved_id: imp.target.map(|t| self.graph.string_id(t).to_string()),
            })
            .collect();
        Ok(Json(callees))
    }

    #[tool(
        description = "Get the spec for a symbol or file id. Always a pure read -- never triggers generation (that's /codeowl generate, via get_next_spec_task/submit_spec). Returns status \"missing\" with the structurally-extracted signature/docstring stub if nothing's been generated yet, or \"current\" with the LLM-written prose if it has."
    )]
    async fn get_spec(
        &self,
        Parameters(req): Parameters<IdRequest>,
    ) -> Result<Json<SpecResponse>, String> {
        let id = self
            .graph
            .find(&req.id)
            .ok_or_else(|| Self::not_found(&req.id))?;

        match self.graph.get(id) {
            crate::graph::Node::Symbol(symbol) => {
                let file_id = symbol.parent.ok_or_else(|| Self::not_found(&req.id))?;
                let file = self
                    .graph
                    .get_file(file_id)
                    .ok_or_else(|| Self::not_found(&req.id))?;
                let existing =
                    crate::spec::read_file_spec(&self.root, &file.id).map_err(|e| e.to_string())?;
                let current = existing.and_then(|spec| {
                    let (_, hash) = spec
                        .symbols
                        .iter()
                        .find(|(sid, _)| sid == &symbol.id)?
                        .clone();
                    (hash.source_hash == symbol.source_hash).then_some(spec)
                });
                match current.and_then(|spec| {
                    spec.sections
                        .iter()
                        .find(|(sid, _)| sid == &symbol.id)
                        .map(|(_, p)| p.clone())
                }) {
                    Some(prose) => Ok(Json(SpecResponse {
                        id: symbol.id.clone(),
                        status: "current".to_string(),
                        signature: symbol.signature.clone(),
                        docstring: symbol.docstring.clone(),
                        content: Some(format!(
                            "### Summary\n{}\n\n### Behavior\n{}",
                            prose.summary, prose.behavior
                        )),
                    })),
                    None => Ok(Json(SpecResponse {
                        id: symbol.id.clone(),
                        status: "missing".to_string(),
                        signature: symbol.signature.clone(),
                        docstring: symbol.docstring.clone(),
                        content: None,
                    })),
                }
            }
            crate::graph::Node::File(file) => {
                let existing =
                    crate::spec::read_file_spec(&self.root, &file.id).map_err(|e| e.to_string())?;
                match existing.filter(|spec| spec.file.source_hash == file.source_hash) {
                    Some(spec) => Ok(Json(SpecResponse {
                        id: file.id.clone(),
                        status: "current".to_string(),
                        signature: String::new(),
                        docstring: None,
                        content: Some(spec.file_summary),
                    })),
                    None => Ok(Json(SpecResponse {
                        id: file.id.clone(),
                        status: "missing".to_string(),
                        signature: String::new(),
                        docstring: None,
                        content: None,
                    })),
                }
            }
        }
    }

    #[tool(
        description = "The next unit /codeowl generate <target> still needs a spec for, bottom-up: a file's uncovered top-level symbols before the file itself. Returns null when the target isn't spec-bearing (e.g. a barrel file) or everything on it is already current -- that's the generate loop's termination signal. Stateless: safe to call repeatedly with the same target."
    )]
    async fn get_next_spec_task(
        &self,
        Parameters(req): Parameters<GenerateTaskRequest>,
    ) -> Result<Json<Option<SpecTaskResponse>>, String> {
        let target_id = self
            .graph
            .find(&req.target)
            .ok_or_else(|| Self::not_found(&req.target))?;
        let task = crate::spec::next_task(&self.graph, &self.root, target_id)
            .map_err(|e| e.to_string())?;

        let Some(task) = task else {
            return Ok(Json(None));
        };
        let response = match task {
            crate::spec::SpecTask::Symbol {
                id,
                signature,
                docstring,
                lines,
            } => {
                let sym_id = self.graph.find(&id).ok_or_else(|| Self::not_found(&id))?;
                let file_id = self
                    .graph
                    .parent_id(sym_id)
                    .ok_or_else(|| Self::not_found(&id))?;
                let file = self
                    .graph
                    .get_file(file_id)
                    .ok_or_else(|| Self::not_found(&id))?;
                let source = read_lines(&self.root, &file.id, lines).map_err(|e| e.to_string())?;
                let dependencies = self
                    .graph
                    .imports()
                    .iter()
                    .filter(|imp| imp.from_file == file.id)
                    .map(|imp| match imp.target {
                        Some(t) => format!("{} ({})", self.graph.string_id(t), imp.specifier),
                        None => format!("{} ({}, unresolved)", imp.imported_name, imp.specifier),
                    })
                    .collect();
                SpecTaskResponse::Symbol {
                    id,
                    signature,
                    docstring,
                    source,
                    dependencies,
                }
            }
            crate::spec::SpecTask::File { id } => {
                let source =
                    std::fs::read_to_string(self.root.join(&id)).map_err(|e| e.to_string())?;
                SpecTaskResponse::File { id, source }
            }
        };
        Ok(Json(Some(response)))
    }

    #[tool(
        description = "Persist LLM-written spec prose for a symbol or file id (from get_next_spec_task). A symbol's content must contain '### Summary' and '### Behavior' headings; a file's content is plain prose for its '## Summary'. Never call this except as part of the get_next_spec_task -> write -> submit_spec loop /codeowl generate drives."
    )]
    async fn submit_spec(
        &self,
        Parameters(req): Parameters<SubmitSpecRequest>,
    ) -> Result<Json<SubmitSpecResponse>, String> {
        let hash = crate::spec::submit(&self.graph, &self.root, &req.id, &req.content)
            .map_err(|e| e.to_string())?;
        Ok(Json(SubmitSpecResponse {
            id: req.id,
            source_hash: hash.source_hash,
            spec_hash: hash.spec_hash,
        }))
    }

    #[tool(
        name = "search_code",
        description = "Regex-search the repo's source files. Phase 1 implementation: embedded ripgrep, no index (see ARCHITECTURE.md's Storage section)."
    )]
    async fn search(
        &self,
        Parameters(req): Parameters<SearchRequest>,
    ) -> Result<Json<Vec<SearchMatch>>, String> {
        crate::search::search_code(&self.root, &req.query)
            .map(Json)
            .map_err(|e| e.to_string())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for CodeOwlServer {
    fn get_info(&self) -> ServerInfo {
        // ServerInfo is #[non_exhaustive] -- built from Default, then its
        // public fields set individually, rather than a struct literal.
        let mut info = ServerInfo::default();
        info.protocol_version = ProtocolVersion::LATEST;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(
            "CodeOwl's read-only structural surface over this repo. get_spec always returns \
             status \"missing\" until spec generation exists -- this server exposes structural \
             facts extracted from source, not LLM-written summaries, yet."
                .to_string(),
        );
        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;
    use crate::imports::extract_imports;
    use crate::resolve::{build_resolver, resolve_imports};
    use std::collections::HashMap;

    /// Build a small in-memory server the same way `main.rs`'s `serve`
    /// subcommand will: extract, resolve, wrap in a `Graph`. Files are
    /// still written to a real temp dir because `oxc_resolver` needs real
    /// paths on disk to resolve against (see `resolve.rs`'s own tests).
    ///
    /// The directory name includes a per-call counter, not just the pid:
    /// `#[tokio::test]`s in this module run concurrently in the same
    /// process, so a pid-only path let two tests race to write different
    /// content to the same "a.ts" and silently corrupt each other's fixture.
    fn test_server(files: &[(&str, &str)]) -> CodeOwlServer {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("codeowl-mcp-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut extractions = Vec::new();
        let mut file_imports = HashMap::new();
        for (rel, content) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            extractions.push(crate::graph::extract_and_hash(rel, content));
            file_imports.insert(rel.to_string(), extract_imports(content, rel));
        }

        let mut graph = Graph::build(extractions);
        let resolver = build_resolver();
        let resolved = resolve_imports(&dir, &resolver, &file_imports, &graph);
        graph.set_resolved_imports(resolved);

        CodeOwlServer::new(dir, graph)
    }

    #[tokio::test]
    async fn get_symbol_returns_the_full_record() {
        let server = test_server(&[("a.ts", "export function double(x: number) {}\n")]);
        let result = server
            .get_symbol(Parameters(IdRequest {
                id: "a.ts::double".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(result.0.id, "a.ts::double");
        assert!(result.0.is_exported);
    }

    #[tokio::test]
    async fn get_symbol_on_unknown_id_is_an_error() {
        let server = test_server(&[("a.ts", "export function double(x: number) {}\n")]);
        let result = server
            .get_symbol(Parameters(IdRequest {
                id: "a.ts::nope".to_string(),
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_callers_lists_the_importing_file() {
        let server = test_server(&[
            ("a.ts", "import { helper } from './b';\n"),
            ("b.ts", "export function helper(): void {}\n"),
        ]);
        let result = server
            .get_callers(Parameters(IdRequest {
                id: "b.ts::helper".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(
            result.0,
            vec![CallerInfo {
                from_file: "a.ts".to_string(),
                imported_name: "helper".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn get_callees_lists_the_files_imports_with_resolution() {
        // Query via "a.ts::marker" (not "helper") to confirm callees is
        // keyed off the containing FILE, not the queried symbol itself.
        let server = test_server(&[
            (
                "a.ts",
                "export const marker = 1;\nimport { helper } from './b';\nimport { z } from 'zod';\n",
            ),
            ("b.ts", "export function helper(): void {}\n"),
        ]);
        let result = server
            .get_callees(Parameters(IdRequest {
                id: "a.ts::marker".to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(result.0.len(), 2);
        let helper = result
            .0
            .iter()
            .find(|c| c.imported_name == "helper")
            .unwrap();
        assert!(
            helper.resolved_id.is_some(),
            "internal import should resolve"
        );
        let external = result.0.iter().find(|c| c.imported_name == "z").unwrap();
        assert_eq!(
            external.resolved_id, None,
            "external package should not resolve"
        );
    }

    #[tokio::test]
    async fn get_spec_reports_missing_when_nothing_generated_yet() {
        let server = test_server(&[(
            "a.ts",
            "/** Doubles a number. */\nexport function double(x: number) {}\n",
        )]);
        let result = server
            .get_spec(Parameters(IdRequest {
                id: "a.ts::double".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(result.0.status, "missing");
        assert_eq!(result.0.docstring.as_deref(), Some("Doubles a number."));
        assert_eq!(result.0.content, None);
    }

    #[tokio::test]
    async fn generate_loop_walks_symbol_then_file_then_reports_done() {
        let server = test_server(&[(
            "a.ts",
            "/** Doubles a number. */\nexport function double(x: number) {}\n",
        )]);

        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "a.ts".to_string(),
            }))
            .await
            .unwrap();
        let Some(SpecTaskResponse::Symbol { id, source, .. }) = task.0 else {
            panic!("expected a symbol task, got {:?}", task.0);
        };
        assert_eq!(id, "a.ts::double");
        assert!(source.contains("function double"));

        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: id.clone(),
                content: "### Summary\nDoubles a number.\n### Behavior\nMultiplies by two."
                    .to_string(),
            }))
            .await
            .unwrap();

        // Re-running get_spec now finds a real, current spec instead of
        // the missing stub -- this is the M4 validation's core claim.
        let spec = server
            .get_spec(Parameters(IdRequest { id: id.clone() }))
            .await
            .unwrap();
        assert_eq!(spec.0.status, "current");
        assert!(spec.0.content.unwrap().contains("Multiplies by two."));

        // Re-running generate with source unchanged must skip the
        // now-current symbol and move on to the file-level task.
        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "a.ts".to_string(),
            }))
            .await
            .unwrap();
        let Some(SpecTaskResponse::File { id: file_id, .. }) = task.0 else {
            panic!("expected a file task, got {:?}", task.0);
        };
        assert_eq!(file_id, "a.ts");

        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: file_id,
                content: "A file with one doubling helper.".to_string(),
            }))
            .await
            .unwrap();

        let done = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "a.ts".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(done.0, None);
    }

    #[tokio::test]
    async fn generate_loop_skips_a_barrel_file() {
        let server = test_server(&[("a.ts", "export { Foo } from './foo';\n")]);
        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "a.ts".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(task.0, None);
    }

    #[tokio::test]
    async fn search_finds_a_known_string() {
        let server = test_server(&[("a.ts", "export function findMe() {}\n")]);
        let result = server
            .search(Parameters(SearchRequest {
                query: "findMe".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(result.0.len(), 1);
        assert_eq!(result.0[0].file, "a.ts");
    }
}
