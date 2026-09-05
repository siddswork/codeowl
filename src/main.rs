use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use codeowl::extract_file;

fn main() -> Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .context("usage: codeowl <path-to-repo>")?;

    let mut symbols = Vec::new();
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
    }

    println!("{}", serde_json::to_string_pretty(&symbols)?);
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
