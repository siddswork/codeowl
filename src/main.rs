use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use codeowl::{Graph, build_resolver, extract_file, extract_imports, resolve_imports};

fn main() -> Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .context("usage: codeowl <path-to-repo>")?;

    let mut symbols = Vec::new();
    let mut file_imports = HashMap::new();
    for entry in ignore::WalkBuilder::new(&root).build() {
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
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        symbols.extend(extract_file(&source, &rel));
        file_imports.insert(rel.clone(), extract_imports(&source, &rel));
    }

    let mut graph = Graph::from_symbols(symbols);
    let resolver = build_resolver();
    let resolved = resolve_imports(&root, &resolver, &file_imports, &graph);
    let resolved_count = resolved.iter().filter(|r| r.target.is_some()).count();
    let total_count = resolved.len();
    graph.set_resolved_imports(resolved);

    // `.codeowl/` lives in the *target* repo, not CodeOwl's own — it's the
    // gitignored local cache described in ARCHITECTURE.md's "Storage".
    graph.save(&root.join(".codeowl").join("graph"))?;
    eprintln!("resolved {resolved_count}/{total_count} named imports");

    println!("{}", serde_json::to_string_pretty(graph.symbols())?);
    Ok(())
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
