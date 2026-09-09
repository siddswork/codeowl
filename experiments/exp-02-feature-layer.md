# exp-02 — The feature layer across stacks (M13-pre spike)

**Status:** spike, on paper. Written before M13 freezes `trait StackPack` / `trait FeatureModel`.
**Feeds:** M13 (trait shape), M14 (`RustStack::feature_model()`), M16/M17 (`JavaStack`).
**Not a decision doc** — its conclusions fold into `ARCHITECTURE.md` open question 4 and each milestone's plan as it starts.

---

## Why this spike exists

Everything in CodeOwl except the feature layer models **code** — symbols, imports, containment, hashes. That's universal; a bug is caught by a failing test. The feature layer models a **product**: *"a feature is an entry point plus everything it reaches, narrated for a BA."* Entry points for the pilot are Next.js pages (`app/**/page.tsx`) and orphan API routes — a **framework routing convention**.

Two questions the trait design can't answer from the Phase 1 code alone:

1. **Does a stack with no routing convention have a feature layer at all?** (drives M14, and whether `feature_model()` is `Option`.)
2. **What does an entry point look like when a stack has *several* kinds of them?** (forward sketch for M16/M17, so the trait isn't shaped by Next.js + Rust alone.)

---

## Part 1 — CodeOwl's own feature specs (drives M14)

CodeOwl is a CLI with two subcommands (`extract`, `serve`); `serve` starts an MCP server exposing 8 tools plus an in-session file watcher. `src/` is flat — ~15 single-file modules, essentially one directory. The product is: *extract a structural graph, serve LLM-authored specs over MCP.*

### Candidate A — entry points are the CLI subcommands + `#[tool]` handlers

Enumerate `extract`, `serve`, and `get_symbol` / `get_callers` / `get_callees` / `get_spec` / `get_next_spec_task` / `submit_spec` / `search_code` / `get_spec_coverage`.

- **For:** each tool's real behaviour spans modules — `get_next_spec_task` reaches `mcp.rs` → `spec.rs` (coverage, task selection, `assemble_participants`) → `graph.rs` → `features.rs`. No single file spec tells that story.
- **Against:** ~10 entry points, most of them one function in `mcp.rs` that delegates to one function in `spec.rs`. A per-tool feature spec would be a longer paraphrase of two file-spec sections. The narrative value concentrates in ~3 flows, not 10 tools.

### Candidate B — entry points are the crate's `pub` API (`lib.rs` re-exports)

`extract_file`, `Graph`, `resolve_imports`, `build_resolver`, …

- **Against, decisively:** nobody consumes `codeowl` as a library. The `pub` surface exists for `main.rs` and tests. These specs would document an API with no callers.

### Candidate C — no feature layer; the system spec composes over module rollups

`src/` is flat, so there's exactly one directory rollup plus ~15 file specs, then the system spec on top.

- **For:** the file specs + rollup + system spec already cover a flat, framework-free codebase. Deterministic, no curation.
- **Against:** the single most valuable thing to know about CodeOwl — *how a spec actually gets generated end to end, from `get_next_spec_task` through `submit_spec`* — spans `mcp.rs` + `spec.rs` + `graph.rs` + `features.rs` and would be buried in a system spec that also has to cover extraction and querying.

### Recommendation for M14: `feature_model() -> None`, but require the three flow narratives in the system spec

CodeOwl has **workflows** (extraction; spec generation; structural query; incremental reindex) but **no mechanical way to enumerate them** — no routing table, no `#[feature]` attribute. Enumerating them means a curated manifest, which is a mechanism M13 would have to invent solely for the self-case. Defer that: let a stack with real conventions (M17/Quarkus) shape it.

So M14 implements `feature_model() -> None`, and its self-corpus's **system spec** carries named sections for the cross-cutting flows:

| Flow | Modules it crosses |
|---|---|
| Extraction (`codeowl extract` / graph build on `serve`) | `lang` → `extract` / `schema` → `imports` → `resolve` → `graph` → `index` |
| Spec generation (`get_spec_coverage` → `get_next_spec_task` → `submit_spec`) | `mcp` → `spec` → `graph` → `features` |
| Structural query (`get_symbol` / `get_callers` / `get_callees` / `search_code`) | `mcp` → `graph` / `search` |
| Incremental reindex (watcher + fresh-spawn catch-up) | `watch` → `index` → `graph` |

This validates the trait's **zero-feature path** on a real corpus early (M16 confirms it again on commons-lang), keeps M14 cheap, and puts the highest-value narrative where a reader will actually find it.

**M14 must also verify** (M13 design decision 3): `spec.rs`'s system-spec composition tolerates an empty feature set without a panic or an empty `## Features` section.

---

## Part 2 — Java entry points (forward sketch for M16/M17)

### commons-lang (M16) — `None`, cleanly

`StringUtils`, `ObjectUtils`, `ArrayUtils` — classes of static methods. No `main`, no framework, no runtime entry surface. Its "surface" is its public API, already covered by class/file/package specs. `feature_model() -> None`.

This is a **cleaner `None` than CodeOwl-Rust**: commons-lang genuinely has zero entry points, whereas CodeOwl has workflows that merely aren't framework-enumerable. M16 is the milestone that proves the zero-feature path holds on a large real library (627 files).

### A Quarkus service (M17) — `Some`, with heterogeneous entry-point *kinds*

From `quarkus-super-heroes` (verified against the checkout): `@Path`×9, `@GET`×6, `@POST`×2, `@Channel`×6, `@Incoming`×1, `@Outgoing`×1, `@RegisterRestClient`×2. Persistence is Panache `@Entity` classes (`Villain extends PanacheEntity`), not `.sql` files.

Entry-point kinds coexisting in one repo:

| Kind | Marker | Human title |
|---|---|---|
| HTTP resource method | `@Path` class + `@GET`/`@POST`/… method | `GET /api/heroes/{id}` |
| Message consumer | `@Incoming("channel")` / `@Channel` injection | `consumes fights` |
| Message producer | `@Outgoing("channel")` | `produces hero-fights` |
| Scheduled job | `@Scheduled(every=…)` | `every 10s` |
| gRPC method | `@GrpcService` impl / proto-generated base | the RPC name |
| `main` | Quarkus bootstrap | usually excluded (generated/trivial) |

**Implications the trait must absorb:**

1. **`EntryPoint` carries a `kind`.** The Next.js model has two kinds (page, orphan API route) but never modelled them as such — `features.rs` has `is_page` / `is_api_route` inline. M13 should make kind a first-class field: `EntryPoint { kind, id, title, source_symbol }`, `kind` being a pack-owned open value (string or open enum), not a closed `Page | ApiRoute` enum.

2. **Title derivation is per-kind and per-pack.** Route path, channel name, cron expression — there is no universal "slug from route". `FeatureModel` owns titling.

3. **`admits_to_core` is CDI-graph-shaped, not co-location-shaped.** The Next.js rule (`is_colocated || does_data_work`) doesn't translate. Importantly this is **type-reference classification, not dataflow analysis** — it fits the import graph CodeOwl already has, and needs no call analysis:

   > an `@Inject`ed field or constructor param has a *declared type* → that type resolves through the ordinary import graph to a file → the annotations on that file's class decide admission.

   Admit an `@ApplicationScoped`/`@Singleton` service or a Panache repository/entity; leave a framework primitive (an injected `Config`, `ObjectMapper`) as a one-hop stub dependency. "Does data work" ⇒ "touches an `@Entity` / `PanacheRepository`, or calls a `@RegisterRestClient`." Expect iterations against the one repo, exactly as M11 warns.

4. **Schema is symbol-level for Java, not file-level.** M10's `SourceKind { Code | Schema }` dispatches on file extension (`.sql` → `schema.rs`). Quarkus persistence is a `@Entity` annotation on a Java class extracted by the ordinary `tree-sitter-java` pass — there is no separate schema *file*. M18 finding, flagged now: schema detection needs to be a symbol-level predicate (`is_schema_symbol(symbol) -> bool`, a `classify`-like hook the pack owns) alongside or instead of the file-level `SourceKind::Schema`. (quarkus-super-heroes also has Hibernate `import.sql` seed data and one Liquibase `changeLog.xml` — seed/migration artefacts, not the DDL source of truth here.)

5. **`@RegisterRestClient` is a flow edge, not an entry point.** A `@RegisterRestClient` interface + its `@Path` is a string-carried edge from one service to another service's endpoint — same shape as M10's `.from("table")` and the pilot's `fetch("/api/…")`. `extract_flow_edges` / `resolve_flow_edge` handle it; it does not get its own feature spec.

---

## Consolidated implications for M13's trait design

Today's shape is `EntryPoint { file: String, slug: String }` (`features.rs`), with kind implied by the `is_page` / `is_api_route` predicates inline. Proposed:

```
trait FeatureModel {
    fn enumerate_entry_points(&self, graph: &Graph) -> Vec<EntryPoint>;
    fn admits_to_core(&self, graph: &Graph, entry: &EntryPoint, candidate_file: &str) -> bool;
}

struct EntryPoint {
    kind:  String,   // pack-owned: "page" | "api-route" | "http-resource" | "kafka-consumer" | ...
    id:    String,   // stable slug for the spec filename + coverage id — unique ACROSS kinds
    title: String,   // human-facing, derived by the pack per kind
    file:  String,   // repo-relative, same scheme as today's EntryPoint.file
}
```

`file: String`, not `SymbolId` — a `SymbolId` is only valid for the graph that produced it and must never reach a spec file or a coverage id (see `graph.rs`'s `SymbolId` doc comment). If M17 finds it needs the specific *method* rather than the file, that's a `String` symbol id, still not a `SymbolId`.

- `feature_model() -> Option<&dyn FeatureModel>` is **confirmed necessary** — commons-lang (M16) returns `None` on a real 627-file corpus, and CodeOwl (M14) returns `None` by choice.
- `EntryPoint.kind` is a first-class, pack-owned field. No closed enum.
- **`id` must be unique across kinds.** The pilot never hit this because pages and API routes came from disjoint path spaces; a Quarkus `GET /fights` and a Kafka consumer on channel `fights` both slug to `fights` and collide on `docs/specs/_features/fights.md`. Kind-prefix, or otherwise disambiguate.
- Titling and `admits_to_core` are entirely pack-internal — no generic fallback, no "generic BFS".
- **Keep `FeatureModel` minimal.** M14 and M16 both return `None`, so this trait has exactly **one** implementation until M17 — three milestones with no second opinion. An interface with one impl silently accretes that impl's assumptions. Two methods, nothing speculative; let M17 ask for what it actually needs.
- **`ExtractedSymbol` needs a marker field.** Every Java conclusion above keys on an annotation (`@Path`, `@Entity`, `@ApplicationScoped`, `@RegisterRestClient`), and today's `ExtractedSymbol` has nowhere to record one — `{id, kind, file, lines, signature, docstring, is_exported, source_hash, interface_hash, parent, children}`. Substring-matching `signature` is not a substitute. Add a pack-owned `markers: Vec<String>` in M13 while the symbol shape is open; Rust decorates too (`#[tool]`, `#[derive]`), so M14 exercises it rather than leaving it dead.
- **Do not** build an entry-point manifest mechanism in M13. If M17 finds Quarkus needs merge/rename/exclude, design it then (ARCHITECTURE open question 4), informed by a stack that actually has framework-enumerable entry points.
- **Schema layer** (M10) is file-dispatch-only today and M17 breaks that. M13 need not fix it, but should not deepen the `SourceKind::Schema`-is-a-file assumption; M18 generalizes to a symbol-level hook (which the `markers` field above makes cheap).

## What each milestone executes against

- **M13:** `EntryPoint { kind, id, title, file }`; `feature_model() -> Option`; `ExtractedSymbol.markers`; keep `FeatureModel` to two methods; don't touch the schema layer's file-dispatch beyond what M12 left.
- **M14:** `RustStack::feature_model() -> None`; the self-corpus system spec carries the four flow sections above; verify `spec.rs` tolerates zero features; populate `markers` from Rust attributes so the field isn't dead on arrival.
- **M16 (done):** `JavaStack::feature_model() -> None` — the zero-feature path confirmed on commons-lang (14,725 nodes; `get_spec_coverage` reports 0 features, pending queue is files → rollups → system, no crash). `classify()` → `Test` for `src/test/java`; resolution keys on the `src/main/java` path-suffix, not `pom.xml`, and lands **100 % of internal imports** plus 1,244 accurate same-package edges. Findings for M18: `get_next_spec_task` bundles a folded Container's whole-file source — 60–420 KB for a Java God-class, over the MCP token limit (the headline one); `get_spec_coverage`'s `pending` also over-limit at 626 files; same-package scan over-includes an ambiguous simple name already covered by an explicit import.
- **M17:** `feature_model() -> Some`; implement `enumerate_entry_points` for `@Path` + `@Incoming`/`@Outgoing` first (the kinds actually present in quarkus-super-heroes), kind-unique slugs, type-reference `admits_to_core`, symbol-level `@Entity` schema detection, `@RegisterRestClient` flow edges.

## Open, deliberately

- ~~Whether a `record` is a `Container` or a `Value`~~ — **resolved in M16 (Container, always).** commons-lang has zero records (Java 8), so it was decided on principle: a record is a restricted `final class`, and `Value` would drop a DTO-per-file layout out of the corpus (`file_is_spec_bearing` needs an exported `Container`/`Callable`). M17's Quarkus corpus is the real test.
- Whether M17's `EntryPoint` needs the specific method or just the file. Left as `file` until a Quarkus resource class with several `@GET` methods proves otherwise — likely it does, and that's an M17 ask, not an M13 guess.
- The merge/rename/exclude manifest (ARCHITECTURE open question 4). Still deferred.
