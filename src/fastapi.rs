//! The FastAPI feature model (M17) — the second `FeatureModel`
//! implementation, and the first that isn't Next.js. A feature is an HTTP
//! route (a `@router.get(…)` / `.post` / … decorated function) plus the
//! CRUD / schema-touching files it reaches. Route enumeration and the
//! `core`-admission rule are the FastAPI-specific judgments; the
//! participant walk in `features.rs` stays generic.
//!
//! Returned by `PythonStack::feature_model()`. A plain Python library has
//! no `@router`-decorated functions, so `enumerate_entry_points` returns
//! an empty vec and the corpus is symbol / file / rollup / system specs
//! alone — the same outcome as `feature_model() -> None`.
//!
//! **Not yet (M19):** the outer `app.include_router(…, prefix=…)` chain —
//! only the entry file's own `APIRouter(prefix=…)` is applied, so a route
//! titles as `GET /items/{id}`, not `GET /api/v1/items/{id}`. The literal
//! segment is what the slug and BA-facing title need; the API-version
//! prefix is cosmetic (`ROADMAP.md` M17 → "router-prefix resolution").

use crate::features::{EntryPoint, FeatureModel};
use crate::graph::Graph;
use crate::symbol::SymbolKind;

const ROUTE_VERBS: &[&str] = &[
    "get",
    "post",
    "put",
    "patch",
    "delete",
    "head",
    "options",
    "websocket",
];

#[derive(Debug, Default, Clone, Copy)]
pub struct FastApiFeatureModel;

impl FeatureModel for FastApiFeatureModel {
    fn enumerate_entry_points(&self, graph: &Graph) -> Vec<EntryPoint> {
        let mut out: Vec<EntryPoint> = graph
            .symbols()
            .filter(|s| s.kind == SymbolKind::Callable)
            .filter_map(|s| {
                let (verb, raw_path) = s.markers.iter().find_map(|m| parse_route_decorator(m))?;
                let prefix = router_prefix(graph, &s.file).unwrap_or_default();
                let full = join_route(&prefix, &raw_path);
                let kind = if verb == "websocket" {
                    "websocket"
                } else {
                    "http"
                };
                Some(EntryPoint {
                    kind: kind.to_string(),
                    id: route_slug(kind, &verb, &full),
                    title: format!("{} {full}", verb.to_uppercase()),
                    file: s.file.clone(),
                })
            })
            .collect();
        out.sort_by(|a, b| (a.id.as_str(), a.file.as_str()).cmp(&(b.id.as_str(), b.file.as_str())));
        disambiguate_colliding_slugs(&mut out);
        out
    }

    fn admits_to_core(&self, graph: &Graph, _entry: &EntryPoint, candidate_file: &str) -> bool {
        is_crud_module(candidate_file) || file_touches_schema(graph, candidate_file)
    }
}

/// `@router.get("/items/{id}", response_model=…)` → `("get",
/// "/items/{id}")`. The receiver name (`router` / `app` / `api_router`)
/// isn't checked — any `.<verb>("…")` decorator is a route.
fn parse_route_decorator(marker: &str) -> Option<(String, String)> {
    let m = marker.trim_start().trim_start_matches('@');
    for verb in ROUTE_VERBS {
        let needle = format!(".{verb}(");
        if let Some(pos) = m.find(&needle) {
            let path = first_string_literal(&m[pos + needle.len()..])?;
            return Some(((*verb).to_string(), path));
        }
    }
    None
}

/// The first `"…"` / `'…'` literal in `s`.
fn first_string_literal(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let start = b.iter().position(|&c| c == b'"' || c == b'\'')?;
    let quote = b[start];
    let rel_end = b[start + 1..].iter().position(|&c| c == quote)?;
    Some(s[start + 1..start + 1 + rel_end].to_string())
}

/// `router = APIRouter(prefix="/items", tags=[…])` declared in `file` →
/// `"/items"`. `prefix=settings.X` (not a literal) or no `APIRouter` → `None`.
fn router_prefix(graph: &Graph, file: &str) -> Option<String> {
    graph
        .symbols()
        .filter(|s| {
            s.file == file && s.kind == SymbolKind::Value && s.signature.contains("APIRouter(")
        })
        .find_map(|s| kwarg_string_literal(&s.signature, "prefix"))
}

/// The string literal assigned to kwarg `key` in `sig`, if it's a literal
/// (`prefix="/x"` → `"/x"`; `prefix=settings.X` → `None`). A byte scan, not
/// an unanchored `sig.find(key=)` (the code-review finding this fixes: that
/// matched the literal text `"prefix="` wherever it first appeared,
/// including inside an earlier kwarg's own string value, e.g.
/// `responses={404: {"description": "...prefix=legacy..."}}, prefix="/x"`)
/// -- so this skips over quoted spans entirely and only matches `key=` at a
/// real token boundary (preceded by the start of `sig` or a non-identifier
/// character).
fn kwarg_string_literal(sig: &str, key: &str) -> Option<String> {
    let bytes = sig.as_bytes();
    let mut i = 0;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(quote) = in_string {
            i += if c == b'\\' { 2 } else { 1 };
            if c == quote {
                in_string = None;
            }
            continue;
        }
        if c == b'"' || c == b'\'' {
            in_string = Some(c);
            i += 1;
            continue;
        }
        let at_boundary = i == 0 || !is_ident_byte(bytes[i - 1]);
        if at_boundary && sig[i..].starts_with(key) && bytes.get(i + key.len()) == Some(&b'=') {
            let rest = sig[i + key.len() + 1..].trim_start();
            return if rest.starts_with('"') || rest.starts_with('\'') {
                first_string_literal(rest)
            } else {
                None
            };
        }
        // A full UTF-8 char, not a blind `i += 1` -- `sig[i..]` above only
        // slices at a valid char boundary if `i` always lands on one; a
        // multi-byte char outside any string (an allowed if unusual
        // Python identifier) advanced one raw byte at a time could
        // otherwise leave `i` mid-character and panic on the next slice.
        i += sig[i..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn join_route(prefix: &str, path: &str) -> String {
    let prefix = prefix.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    match (prefix.is_empty(), path.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{path}"),
        (false, true) => prefix.to_string(),
        (false, false) => format!("{prefix}/{path}"),
    }
}

/// `("http", "get", "/items/{id}")` → `"http-get-items-id"`. Filename-safe,
/// and unique across the verbs/paths of *one router* (`ROADMAP.md` M17 —
/// "slugs must be unique across kinds") -- but two *different* routers can
/// still produce the same slug (most likely today because the outer
/// `app.include_router(prefix=...)` chain isn't threaded in yet, M19); see
/// `disambiguate_colliding_slugs`, which `enumerate_entry_points` always
/// runs afterward, for how that case is handled.
fn route_slug(kind: &str, verb: &str, path: &str) -> String {
    let path_part = path
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|seg| !seg.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-");
    if path_part.is_empty() {
        format!("{kind}-{verb}-root")
    } else {
        format!("{kind}-{verb}-{path_part}")
    }
}

/// `entries` sorted by `(id, file)`: give every id beyond the first in a
/// run a stable, deterministic `-2`, `-3`, … suffix instead of letting
/// `enumerate_entry_points` silently collapse them (the code-review
/// finding this fixes -- a `dedup_by` on `id` alone dropped every route
/// after the first whenever two different files' routes collided). The
/// first entry in each run keeps its clean id unchanged, so the common,
/// non-colliding case never churns existing spec ids.
fn disambiguate_colliding_slugs(entries: &mut [EntryPoint]) {
    let mut i = 0;
    while i < entries.len() {
        let mut n = 2;
        let mut j = i + 1;
        while j < entries.len() && entries[j].id == entries[i].id {
            entries[j].id = format!("{}-{n}", entries[i].id);
            n += 1;
            j += 1;
        }
        i = j;
    }
}

/// A repository / data-access module — joins a feature's `core` even
/// though it isn't co-located (the CDI-shaped intuition the M17 plan
/// borrows for Java too: "does data work" ⇒ `core`).
fn is_crud_module(file: &str) -> bool {
    let stem = file.rsplit('/').next().unwrap_or(file);
    stem == "crud.py"
        || stem == "repository.py"
        || stem == "repositories.py"
        || file.contains("/crud/")
        || file.contains("/repositories/")
}

/// `file` declares an ORM table, or imports one — the "does data work"
/// half of `admits_to_core`. `is_schema_symbol` (M17) already retagged the
/// models, so this is a plain kind check off the graph, no call analysis.
fn file_touches_schema(graph: &Graph, file: &str) -> bool {
    graph
        .symbols()
        .any(|s| s.file == file && s.kind == SymbolKind::Schema)
        || graph
            .imports()
            .iter()
            .filter(|i| i.from_file == file)
            .any(|i| {
                i.target
                    .and_then(|t| graph.get_symbol(t))
                    .is_some_and(|s| s.kind == SymbolKind::Schema)
            })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_route_decorators() {
        assert_eq!(
            parse_route_decorator(r#"@router.get("/", response_model=ItemsPublic)"#),
            Some(("get".to_string(), "/".to_string()))
        );
        assert_eq!(
            parse_route_decorator(r#"@router.get("/{id}", response_model=ItemPublic)"#),
            Some(("get".to_string(), "/{id}".to_string()))
        );
        assert_eq!(
            parse_route_decorator(
                r#"@router.post( "/password-recovery/{email}", dependencies=[Depends(x)], )"#
            ),
            Some(("post".to_string(), "/password-recovery/{email}".to_string()))
        );
        assert_eq!(
            parse_route_decorator("@router.websocket(\"/ws\")"),
            Some(("websocket".to_string(), "/ws".to_string()))
        );
        assert_eq!(parse_route_decorator("@staticmethod"), None);
        assert_eq!(parse_route_decorator("@app.middleware(\"http\")"), None);
    }

    #[test]
    fn prefix_is_joined_and_slugged() {
        assert_eq!(join_route("/items", "/{id}"), "/items/{id}");
        assert_eq!(join_route("", "/login/access-token"), "/login/access-token");
        assert_eq!(join_route("/items", "/"), "/items");
        assert_eq!(
            route_slug("http", "get", "/items/{id}"),
            "http-get-items-id"
        );
        assert_eq!(route_slug("http", "post", "/"), "http-post-root");
        assert_eq!(
            route_slug("http", "post", "/login/access-token"),
            "http-post-login-access-token"
        );
    }

    #[test]
    fn kwarg_literal_vs_reference() {
        assert_eq!(
            kwarg_string_literal(
                r#"router = APIRouter(prefix="/items", tags=["items"])"#,
                "prefix"
            ),
            Some("/items".to_string())
        );
        assert_eq!(
            kwarg_string_literal(
                r#"router = APIRouter(tags=["private"], prefix="/private")"#,
                "prefix"
            ),
            Some("/private".to_string())
        );
        assert_eq!(
            kwarg_string_literal(
                r#"app.include_router(api_router, prefix=settings.API_V1_STR)"#,
                "prefix"
            ),
            None
        );
    }

    #[test]
    fn kwarg_string_literal_does_not_panic_on_multi_byte_utf8_content() {
        // Guards the byte-scan fix above: a non-ASCII character inside a
        // string value must not leave the scan mid-character and panic on
        // the next `sig[i..]` slice.
        assert_eq!(
            kwarg_string_literal(
                r#"router = APIRouter(tags=["café"], prefix="/items")"#,
                "prefix"
            ),
            Some("/items".to_string())
        );
    }

    #[test]
    fn kwarg_string_literal_ignores_the_key_text_inside_another_kwargs_value() {
        // Code-review finding: an unanchored `sig.find("prefix=")`
        // matched the *literal text* "prefix=" wherever it first
        // appeared, including inside an earlier kwarg's own string value
        // -- corrupting the parsed route prefix.
        assert_eq!(
            kwarg_string_literal(
                r#"router = APIRouter(responses={404: {"description": "see prefix=legacy for old routes"}}, prefix="/items")"#,
                "prefix"
            ),
            Some("/items".to_string()),
            "must find the real prefix= kwarg, not the literal text inside another kwarg's string value"
        );
    }

    #[test]
    fn crud_module_detection() {
        assert!(is_crud_module("app/crud.py"));
        assert!(is_crud_module("app/repositories/user.py"));
        assert!(!is_crud_module("app/api/routes/items.py"));
    }
}
