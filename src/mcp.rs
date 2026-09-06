//! The MCP read surface — M3. Every tool here is a pure read: nothing in
//! this module calls an LLM or writes anything, ever. `get_next_spec_task`/
//! `submit_spec` (the write side that drives generation) don't exist until
//! M4 — see `CLAUDE.md`'s hard invariants on `get_spec` staying a pure read.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::graph::Graph;
use crate::search::SearchMatch;
use crate::symbol::Symbol;

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
    /// One of `"missing"` | `"current"` | `"stale"`. Only `"missing"` is
    /// possible until M4 (generation) and M5 (staleness) exist.
    pub status: String,
    pub signature: String,
    pub docstring: Option<String>,
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

#[tool_router]
impl CodeOwlServer {
    #[tool(
        description = "Look up one symbol's full record (signature, docstring, line range, hashes) by its stable id, e.g. \"lib/utils.ts::cn\"."
    )]
    async fn get_symbol(
        &self,
        Parameters(req): Parameters<IdRequest>,
    ) -> Result<Json<Symbol>, String> {
        let id = self
            .graph
            .find(&req.id)
            .ok_or_else(|| Self::not_found(&req.id))?;
        Ok(Json(self.graph.get(id).clone()))
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
        let file = self.graph.get(id).file.clone();
        let callees = self
            .graph
            .imports()
            .iter()
            .filter(|imp| imp.from_file == file)
            .map(|imp| CalleeInfo {
                specifier: imp.specifier.clone(),
                imported_name: imp.imported_name.clone(),
                resolved_id: imp.target.map(|t| self.graph.get(t).id.clone()),
            })
            .collect();
        Ok(Json(callees))
    }

    #[tool(
        description = "Get the spec for a symbol. Always a pure read -- never triggers generation. Until M4/M5 land, nothing has ever been generated, so this always returns status \"missing\" alongside the signature/docstring CodeOwl already extracted structurally."
    )]
    async fn get_spec(
        &self,
        Parameters(req): Parameters<IdRequest>,
    ) -> Result<Json<SpecResponse>, String> {
        let id = self
            .graph
            .find(&req.id)
            .ok_or_else(|| Self::not_found(&req.id))?;
        let symbol = self.graph.get(id);
        Ok(Json(SpecResponse {
            id: symbol.id.clone(),
            status: "missing".to_string(),
            signature: symbol.signature.clone(),
            docstring: symbol.docstring.clone(),
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
    use crate::extract::extract_file;
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

        let mut all_symbols = Vec::new();
        let mut file_imports = HashMap::new();
        for (rel, content) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            all_symbols.extend(extract_file(content, rel));
            file_imports.insert(rel.to_string(), extract_imports(content, rel));
        }

        let mut graph = Graph::from_symbols(all_symbols);
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
    async fn get_spec_always_reports_missing_in_m3() {
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
