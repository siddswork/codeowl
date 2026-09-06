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
    /// The `/codeowl generate <id>` target — a file id (a repo-relative
    /// path, e.g. "lib/utils.ts"), which may also be a feature entry point
    /// (a page or an orphan API route — see `features.rs`), or a directory
    /// path (e.g. "lib") with >=2 spec-bearing files. Stateless: safe to
    /// call repeatedly with the same target until it reports nothing left.
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct CoreSource {
    pub file: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct DependencyContext {
    pub id: String,
    /// The dependency's already-generated summary if it has a current
    /// spec, otherwise a deterministic stub (signature + docstring) — this
    /// task never triggers the dependency's own generation.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct RollupFile {
    pub file: String,
    /// That file's own current `## Summary` prose — never its raw source
    /// (see "Bottom-up composition"). Only ever populated once the file
    /// itself is current; see `next_task_for_directory`.
    pub summary: String,
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
    /// Generated in one shot, unlike the bottom-up symbol-then-file chase
    /// above: a feature spec is a single document with a single
    /// `spec_hash`, so there's exactly one task, not several.
    Feature {
        /// `"feature:<slug>"` — pass this straight back as `submit_spec`'s
        /// `id`; it's never a real file/symbol id (see `submit_spec`'s own
        /// dispatch).
        id: String,
        entry_point: String,
        core_sources: Vec<CoreSource>,
        dependencies: Vec<DependencyContext>,
    },
    /// Generated in one shot, like a feature spec: composed purely from
    /// its files' own already-generated summaries, never their raw source.
    /// Only ever produced once every file in `files` is itself current —
    /// `get_next_spec_task` walks a directory target's own files' bottom-up
    /// ladders first, the same "symbols before their file" order one level
    /// up: files before their directory's rollup.
    Rollup {
        /// `"rollup:<dir_path>"` — pass this straight back as
        /// `submit_spec`'s `id`; it's never a real file/symbol id.
        id: String,
        dir_path: String,
        files: Vec<RollupFile>,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubmitSpecRequest {
    /// The id `get_next_spec_task` returned — a symbol id, a file id, or
    /// (for a feature or rollup task) the `"feature:<slug>"`/
    /// `"rollup:<dir_path>"` id it reported.
    pub id: String,
    /// For a symbol task: markdown containing `### Summary` and
    /// `### Behavior` headings. For a file or rollup task: plain prose,
    /// becomes the `## Summary`. For a feature task: the whole document
    /// body, starting with a `# Title` line — see `ARCHITECTURE.md`'s
    /// "Feature specs" template.
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SubmitSpecResponse {
    pub id: String,
    /// `None` for a feature or rollup submission — neither has a single
    /// source hash: a feature has a participant map, a rollup has a
    /// per-file hash map (see `get_spec`/the persisted frontmatter for
    /// either).
    pub source_hash: Option<String>,
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

    /// `get_next_spec_task`'s fallback once `target`'s own file/symbol
    /// tasks are exhausted: if `target` is a recognized feature entry
    /// point and its feature spec isn't current, return that as the next
    /// task; otherwise there's genuinely nothing left.
    fn next_feature_task_response(
        &self,
        target: &str,
    ) -> Result<Json<Option<SpecTaskResponse>>, String> {
        let route_literals = self.graph.route_literals();
        let entry_points = crate::features::enumerate_entry_points(&self.graph, route_literals);
        let Some(entry) = entry_points.iter().find(|e| e.file == target) else {
            return Ok(Json(None));
        };

        let task = crate::spec::next_feature_task(&self.graph, &self.root, entry, route_literals)
            .map_err(|e| e.to_string())?;
        let Some(task) = task else {
            return Ok(Json(None));
        };

        Ok(Json(Some(SpecTaskResponse::Feature {
            id: format!("feature:{}", task.slug),
            entry_point: task.entry_point,
            core_sources: task
                .core_sources
                .into_iter()
                .map(|(file, source)| CoreSource { file, source })
                .collect(),
            dependencies: task
                .dependencies
                .into_iter()
                .map(|(id, summary)| DependencyContext { id, summary })
                .collect(),
        })))
    }

    /// `get_next_spec_task`'s path when `target` doesn't resolve as a
    /// graph id at all (directories aren't graph nodes — see `ROADMAP.md`'s
    /// M6 scope): if it's a spec-bearing directory, walk its own files'
    /// bottom-up ladders first, then the rollup task itself once every file
    /// is current; otherwise there's genuinely nothing here.
    fn next_directory_task_response(
        &self,
        target: &str,
    ) -> Result<Json<Option<SpecTaskResponse>>, String> {
        if !crate::spec::directory_is_spec_bearing(&self.graph, target) {
            return Ok(Json(None));
        }

        if let Some(task) = crate::spec::next_task_for_directory(&self.graph, &self.root, target)
            .map_err(|e| e.to_string())?
        {
            return Ok(Json(Some(self.spec_task_to_response(task)?)));
        }

        let Some(task) = crate::spec::next_rollup_task(&self.graph, &self.root, target)
            .map_err(|e| e.to_string())?
        else {
            return Ok(Json(None));
        };
        Ok(Json(Some(SpecTaskResponse::Rollup {
            id: format!("rollup:{}", task.dir_path),
            dir_path: task.dir_path,
            files: task
                .files
                .into_iter()
                .map(|(file, summary)| RollupFile { file, summary })
                .collect(),
        })))
    }

    /// Shared by `get_next_spec_task`'s direct-target path and its
    /// directory-target fallback: turns a bottom-up `SpecTask` (a symbol or
    /// a file, never a feature/rollup — those are assembled by their own
    /// callers) into the wire response, reading whatever source it needs
    /// off disk.
    fn spec_task_to_response(
        &self,
        task: crate::spec::SpecTask,
    ) -> Result<SpecTaskResponse, String> {
        Ok(match task {
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
        })
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
        description = "Get the spec for a symbol id, file id, 'feature:<slug>' id, or 'rollup:<dir_path>' id. Always a pure read -- never triggers generation (that's /codeowl generate, via get_next_spec_task/submit_spec). Returns status \"missing\" with the structurally-extracted signature/docstring stub if nothing's been generated yet, or \"current\" with the LLM-written prose if it has."
    )]
    async fn get_spec(
        &self,
        Parameters(req): Parameters<IdRequest>,
    ) -> Result<Json<SpecResponse>, String> {
        if let Some(dir_path) = req.id.strip_prefix("rollup:") {
            let spec =
                crate::spec::read_rollup_spec(&self.root, dir_path).map_err(|e| e.to_string())?;
            let current = crate::spec::current_file_hashes(&self.graph, &self.root, dir_path)
                .map_err(|e| e.to_string())?;
            let is_current = spec.as_ref().is_some_and(|s| {
                s.files.len() == current.len()
                    && s.files
                        .iter()
                        .all(|(id, h)| current.iter().any(|(cid, ch)| cid == id && ch == h))
            });
            return Ok(Json(SpecResponse {
                id: req.id,
                status: if is_current { "current" } else { "missing" }.to_string(),
                signature: String::new(),
                docstring: None,
                content: is_current.then(|| spec.unwrap().body),
            }));
        }
        if let Some(slug) = req.id.strip_prefix("feature:") {
            let spec =
                crate::spec::read_feature_spec(&self.root, slug).map_err(|e| e.to_string())?;
            let route_literals = self.graph.route_literals();
            let entry_points = crate::features::enumerate_entry_points(&self.graph, route_literals);
            let entry = entry_points
                .iter()
                .find(|e| e.slug == slug)
                .ok_or_else(|| Self::not_found(&req.id))?;
            let participants =
                crate::features::assemble_participants(&self.graph, route_literals, &entry.file);
            let is_current = spec.as_ref().is_some_and(|s| {
                let current = crate::spec::current_participant_hashes(&self.graph, &participants);
                current.is_ok_and(|current| {
                    s.participants.len() == current.len()
                        && s.participants
                            .iter()
                            .all(|(id, h)| current.iter().any(|(cid, ch)| cid == id && ch == h))
                })
            });
            return Ok(Json(SpecResponse {
                id: req.id,
                status: if is_current { "current" } else { "missing" }.to_string(),
                signature: String::new(),
                docstring: None,
                content: is_current.then(|| spec.unwrap().body),
            }));
        }

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
        description = "The next unit /codeowl generate <target> still needs a spec for, bottom-up: a file's uncovered top-level symbols, then the file itself, then (if the target is also a feature entry point -- a page or an orphan API route) the feature spec. target may also be a directory path (e.g. \"lib\") with >=2 spec-bearing files -- walks each of its files' own symbol-then-file ladder first, then the directory's rollup spec once every file is current. Returns null when the target isn't spec-bearing at all (e.g. a barrel file with no feature or rollup either) or everything on it is already current -- that's the generate loop's termination signal. Stateless: safe to call repeatedly with the same target."
    )]
    async fn get_next_spec_task(
        &self,
        Parameters(req): Parameters<GenerateTaskRequest>,
    ) -> Result<Json<Option<SpecTaskResponse>>, String> {
        let Some(target_id) = self.graph.find(&req.target) else {
            return self.next_directory_task_response(&req.target);
        };
        let task = crate::spec::next_task(&self.graph, &self.root, target_id)
            .map_err(|e| e.to_string())?;

        let Some(task) = task else {
            return self.next_feature_task_response(&req.target);
        };
        Ok(Json(Some(self.spec_task_to_response(task)?)))
    }

    #[tool(
        description = "Persist LLM-written spec prose for a symbol id, file id, 'feature:<slug>' id, or 'rollup:<dir_path>' id (all from get_next_spec_task). A symbol's content must contain '### Summary' and '### Behavior' headings; a file's or rollup's content is plain prose for its '## Summary'; a feature's content is the whole document starting with a '# Title' line. Never call this except as part of the get_next_spec_task -> write -> submit_spec loop /codeowl generate drives."
    )]
    async fn submit_spec(
        &self,
        Parameters(req): Parameters<SubmitSpecRequest>,
    ) -> Result<Json<SubmitSpecResponse>, String> {
        if let Some(dir_path) = req.id.strip_prefix("rollup:") {
            let spec = crate::spec::submit_rollup(&self.graph, &self.root, dir_path, &req.content)
                .map_err(|e| e.to_string())?;
            return Ok(Json(SubmitSpecResponse {
                id: req.id,
                source_hash: None,
                spec_hash: spec.spec_hash,
            }));
        }
        if let Some(slug) = req.id.strip_prefix("feature:") {
            let route_literals = self.graph.route_literals();
            let entry_points = crate::features::enumerate_entry_points(&self.graph, route_literals);
            let entry = entry_points
                .iter()
                .find(|e| e.slug == slug)
                .ok_or_else(|| format!("no feature entry point with slug {slug:?}"))?;
            let spec = crate::spec::submit_feature(
                &self.graph,
                &self.root,
                route_literals,
                &entry.file,
                &req.content,
            )
            .map_err(|e| e.to_string())?;
            return Ok(Json(SubmitSpecResponse {
                id: req.id,
                source_hash: None,
                spec_hash: spec.spec_hash,
            }));
        }

        let hash = crate::spec::submit(&self.graph, &self.root, &req.id, &req.content)
            .map_err(|e| e.to_string())?;
        Ok(Json(SubmitSpecResponse {
            id: req.id,
            source_hash: Some(hash.source_hash),
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
        let mut route_literals = Vec::new();
        for (rel, content) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            extractions.push(crate::graph::extract_and_hash(rel, content));
            file_imports.insert(rel.to_string(), extract_imports(content, rel));
            route_literals.extend(crate::features::extract_route_literals(content, rel));
        }

        let mut graph = Graph::build(extractions);
        let resolver = build_resolver();
        let resolved = resolve_imports(&dir, &resolver, &file_imports, &graph);
        graph.set_resolved_imports(resolved);
        graph.set_route_literals(route_literals);

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
    async fn generate_loop_produces_a_feature_spec_once_the_target_files_are_covered() {
        let server = test_server(&[
            (
                "app/submit/page.tsx",
                "import { getSupabase } from '../../lib/supabase';\nexport default function Page() {\n  fetch(\"/api/submit-artwork\");\n  getSupabase();\n  return null;\n}\n",
            ),
            (
                "app/api/submit-artwork/route.ts",
                "import { getSupabase } from '../../../lib/supabase';\nexport async function POST(): Promise<void> {\n  getSupabase();\n}\n",
            ),
            (
                "lib/supabase.ts",
                "export function getSupabase(): void {}\n",
            ),
        ]);

        // `export default function Page()` is a *named* default export,
        // so M1 extracts it as a real, exported symbol -- page.tsx is
        // file-spec-bearing under M4's rule same as any other file. Drain
        // that ladder (its one symbol, then the file itself) before the
        // feature task appears, exactly like a real `/codeowl generate`
        // run against this target would.
        let symbol_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/submit/page.tsx".to_string(),
            }))
            .await
            .unwrap()
            .0
            .expect("expected the page's own symbol task");
        assert!(matches!(symbol_task, SpecTaskResponse::Symbol { .. }));
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "app/submit/page.tsx::Page".to_string(),
                content: "### Summary\nRenders the submission form.\n### Behavior\nCalls the submit-artwork API.\n".to_string(),
            }))
            .await
            .unwrap();
        let file_task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/submit/page.tsx".to_string(),
            }))
            .await
            .unwrap()
            .0
            .expect("expected the page's own file task");
        assert!(matches!(file_task, SpecTaskResponse::File { .. }));
        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: "app/submit/page.tsx".to_string(),
                content: "The artwork submission page.".to_string(),
            }))
            .await
            .unwrap();

        // Now that page.tsx's own symbol+file specs are current, the next
        // task on this same target is the feature.
        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/submit/page.tsx".to_string(),
            }))
            .await
            .unwrap()
            .0
            .expect("expected a feature task");
        let SpecTaskResponse::Feature {
            id,
            entry_point,
            core_sources,
            dependencies,
        } = task
        else {
            panic!("expected a Feature task");
        };
        assert_eq!(id, "feature:submit");
        assert_eq!(entry_point, "app/submit/page.tsx");
        assert_eq!(
            core_sources
                .iter()
                .map(|c| c.file.as_str())
                .collect::<Vec<_>>(),
            vec!["app/submit/page.tsx", "app/api/submit-artwork/route.ts"]
        );
        assert_eq!(dependencies.len(), 1);
        assert_eq!(dependencies[0].id, "lib/supabase.ts::getSupabase");

        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: id.clone(),
                content: "# Artwork submission\n## Summary\nLets an artist submit artwork.\n"
                    .to_string(),
            }))
            .await
            .unwrap();

        let spec = server
            .get_spec(Parameters(IdRequest { id: id.clone() }))
            .await
            .unwrap();
        assert_eq!(spec.0.status, "current");
        assert!(
            spec.0
                .content
                .unwrap()
                .contains("Lets an artist submit artwork.")
        );

        let done = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "app/submit/page.tsx".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(done.0, None);
    }

    #[tokio::test]
    async fn generate_loop_produces_a_rollup_once_its_files_are_covered() {
        let server = test_server(&[
            ("lib/one.ts", "export function one(): void {}\n"),
            ("lib/two.ts", "export function two(): void {}\n"),
        ]);

        // Neither file has a spec yet -- targeting the directory should
        // return one of their own symbol tasks first, not the rollup.
        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "lib".to_string(),
            }))
            .await
            .unwrap()
            .0
            .expect("expected lib/one.ts's own symbol task");
        assert!(matches!(task, SpecTaskResponse::Symbol { .. }));

        for (sym_id, sym_content, file_id, file_content) in [
            (
                "lib/one.ts::one",
                "### Summary\nS1.\n### Behavior\nB1.\n",
                "lib/one.ts",
                "Does one thing.",
            ),
            (
                "lib/two.ts::two",
                "### Summary\nS2.\n### Behavior\nB2.\n",
                "lib/two.ts",
                "Does two things.",
            ),
        ] {
            server
                .submit_spec(Parameters(SubmitSpecRequest {
                    id: sym_id.to_string(),
                    content: sym_content.to_string(),
                }))
                .await
                .unwrap();
            server
                .submit_spec(Parameters(SubmitSpecRequest {
                    id: file_id.to_string(),
                    content: file_content.to_string(),
                }))
                .await
                .unwrap();
        }

        let task = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "lib".to_string(),
            }))
            .await
            .unwrap()
            .0
            .expect("both files current, expected the rollup task");
        let SpecTaskResponse::Rollup {
            id,
            dir_path,
            files,
        } = task
        else {
            panic!("expected a Rollup task, got {task:?}");
        };
        assert_eq!(id, "rollup:lib");
        assert_eq!(dir_path, "lib");
        assert_eq!(
            files,
            vec![
                RollupFile {
                    file: "lib/one.ts".to_string(),
                    summary: "Does one thing.".to_string(),
                },
                RollupFile {
                    file: "lib/two.ts".to_string(),
                    summary: "Does two things.".to_string(),
                },
            ]
        );

        server
            .submit_spec(Parameters(SubmitSpecRequest {
                id: id.clone(),
                content: "Small shared helpers.".to_string(),
            }))
            .await
            .unwrap();

        let spec = server
            .get_spec(Parameters(IdRequest { id: id.clone() }))
            .await
            .unwrap();
        assert_eq!(spec.0.status, "current");
        assert_eq!(spec.0.content.unwrap(), "Small shared helpers.");

        let done = server
            .get_next_spec_task(Parameters(GenerateTaskRequest {
                target: "lib".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(done.0, None);
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
