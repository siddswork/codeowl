//! A permanent, cross-stack matrix test for the spec lifecycle: cascade vs.
//! local staleness (the M21.b interface_hash fold this exercises directly),
//! orphaned specs, a spec manually deleted from disk, and a source file
//! rename -- run identically against one small, real fixture per supported
//! stack (Rust, TypeScript, Python, Java), not just one.
//!
//! Each stack's fixture is the same shape: an "owner" file defining a
//! `Widget` container with one public field (`count`), one private field
//! (`secret`, used only for the private-field-stays-local scenario), and
//! one method (`touch`, deliberately not referencing either field's type,
//! so a field edit and a method-body edit are independent variables, never
//! both moving together) and a "consumer" file that imports `Widget` and
//! calls `touch`.
//! One shared baseline of specs is submitted once per fixture; every
//! scenario below re-derives a fresh `Graph` from a differently-edited copy
//! of the *same* on-disk files and checks staleness against that one
//! baseline -- the same "one baseline, several independent edits checked
//! against it" pattern `mcp.rs`'s own
//! `dependency_signature_change_stales_importer_but_implementation_change_does_not`
//! test already uses, at the lower `spec.rs` API level instead of through
//! the async MCP tool wrappers (no tokio runtime needed, and the hash-level
//! mechanics this file cares about are `spec.rs`'s directly, not `mcp.rs`'s).

use std::path::Path;

use codeowl::graph::{Graph, SymbolId};
use codeowl::index::RepoIndex;
use codeowl::spec::{self, SpecTask};

struct Fixture {
    stack: &'static str,
    /// Extra files needed before `owner_path`/`consumer_path` are even
    /// resolvable -- Rust's `lib.rs` (`pub mod` declarations), Python's
    /// `pyproject.toml` (belt-and-braces stack detection). Empty for
    /// stacks that need nothing extra.
    scaffold: &'static [(&'static str, &'static str)],
    owner_path: &'static str,
    consumer_path: &'static str,
    owner_symbol_id: &'static str,
    consumer_symbol_id: &'static str,
    owner_v1: &'static str,
    /// Same public surface as `owner_v1` -- only `touch`'s own message
    /// changes. The local-effect case: must stale the owner alone.
    owner_body_edit: &'static str,
    /// `count`'s type changes, `touch`'s body doesn't. The cascade case
    /// this whole matrix exists to exercise: must stale the owner *and*
    /// the consumer, one hop.
    owner_field_type_change: &'static str,
    /// `secret` (private/non-exported)'s type changes, `count` and
    /// `touch` don't. The interface_hash fold only ever walks *public*
    /// fields -- this must stale the owner alone, same as a body edit,
    /// never cross to the consumer.
    owner_private_field_change: &'static str,
    consumer_src: &'static str,
    /// A file path elsewhere in the same repo that this stack's `Widget`
    /// gets renamed to for the rename scenario -- same content as
    /// `owner_v1`, just relocated (and, for Java only, the class
    /// renamed to match -- a real Java rename needs the public class
    /// name to match its file name; the other three stacks can keep the
    /// exported name unchanged across a rename).
    owner_renamed_path: &'static str,
    owner_renamed_content: &'static str,
}

const RUST: Fixture = Fixture {
    stack: "rust",
    scaffold: &[("src/lib.rs", "pub mod owner;\npub mod consumer;\n")],
    owner_path: "src/owner.rs",
    consumer_path: "src/consumer.rs",
    owner_symbol_id: "src/owner.rs::Widget",
    consumer_symbol_id: "src/consumer.rs::make",
    owner_v1: "pub struct Widget {\n    pub count: i32,\n    secret: i32,\n}\n\nimpl Widget {\n    pub fn touch(&self) {\n        println!(\"touched\");\n    }\n}\n",
    owner_body_edit: "pub struct Widget {\n    pub count: i32,\n    secret: i32,\n}\n\nimpl Widget {\n    pub fn touch(&self) {\n        println!(\"touched again\");\n    }\n}\n",
    owner_field_type_change: "pub struct Widget {\n    pub count: i64,\n    secret: i32,\n}\n\nimpl Widget {\n    pub fn touch(&self) {\n        println!(\"touched\");\n    }\n}\n",
    owner_private_field_change: "pub struct Widget {\n    pub count: i32,\n    secret: i64,\n}\n\nimpl Widget {\n    pub fn touch(&self) {\n        println!(\"touched\");\n    }\n}\n",
    consumer_src: "use crate::owner::Widget;\n\npub fn make() -> Widget {\n    let w = Widget { count: 0 };\n    w.touch();\n    w\n}\n",
    owner_renamed_path: "src/gadget.rs",
    owner_renamed_content: "pub struct Widget {\n    pub count: i32,\n    secret: i32,\n}\n\nimpl Widget {\n    pub fn touch(&self) {\n        println!(\"touched\");\n    }\n}\n",
};

const TYPESCRIPT: Fixture = Fixture {
    stack: "typescript",
    scaffold: &[],
    owner_path: "owner.ts",
    consumer_path: "consumer.ts",
    owner_symbol_id: "owner.ts::Widget",
    consumer_symbol_id: "consumer.ts::make",
    owner_v1: "export class Widget {\n    count: number;\n    private secret: number;\n\n    touch(): void {\n        console.log('touched');\n    }\n}\n",
    owner_body_edit: "export class Widget {\n    count: number;\n    private secret: number;\n\n    touch(): void {\n        console.log('touched again');\n    }\n}\n",
    owner_field_type_change: "export class Widget {\n    count: string;\n    private secret: number;\n\n    touch(): void {\n        console.log('touched');\n    }\n}\n",
    owner_private_field_change: "export class Widget {\n    count: number;\n    private secret: string;\n\n    touch(): void {\n        console.log('touched');\n    }\n}\n",
    consumer_src: "import { Widget } from './owner';\n\nexport function make(): Widget {\n    const w = new Widget();\n    w.touch();\n    return w;\n}\n",
    owner_renamed_path: "gadget.ts",
    owner_renamed_content: "export class Widget {\n    count: number;\n    private secret: number;\n\n    touch(): void {\n        console.log('touched');\n    }\n}\n",
};

const PYTHON: Fixture = Fixture {
    stack: "python",
    scaffold: &[("pyproject.toml", "[project]\nname = \"x\"\n")],
    owner_path: "owner.py",
    consumer_path: "consumer.py",
    owner_symbol_id: "owner.py::Widget",
    consumer_symbol_id: "consumer.py::make",
    owner_v1: "class Widget:\n    count: int\n    _secret: int\n\n    def touch(self):\n        print(\"touched\")\n",
    owner_body_edit: "class Widget:\n    count: int\n    _secret: int\n\n    def touch(self):\n        print(\"touched again\")\n",
    owner_field_type_change: "class Widget:\n    count: str\n    _secret: int\n\n    def touch(self):\n        print(\"touched\")\n",
    owner_private_field_change: "class Widget:\n    count: int\n    _secret: str\n\n    def touch(self):\n        print(\"touched\")\n",
    consumer_src: "from owner import Widget\n\n\ndef make():\n    w = Widget()\n    w.touch()\n    return w\n",
    owner_renamed_path: "gadget.py",
    owner_renamed_content: "class Widget:\n    count: int\n    _secret: int\n\n    def touch(self):\n        print(\"touched\")\n",
};

const JAVA: Fixture = Fixture {
    stack: "java",
    scaffold: &[],
    owner_path: "src/main/java/com/example/Widget.java",
    consumer_path: "src/main/java/com/example/Consumer.java",
    owner_symbol_id: "src/main/java/com/example/Widget.java::Widget",
    // `make` is a *method* (nested in Consumer -- Java requires it), so
    // it's never independently spec-bearing (is_exported is always false
    // for a nested member, the same rule that already applies to
    // fields); Consumer itself, the containing class, is what actually
    // carries the task. Unlike Rust/TS/Python's `make`, a bare top-level
    // function there and genuinely independently spec-bearing.
    consumer_symbol_id: "src/main/java/com/example/Consumer.java::Consumer",
    owner_v1: "package com.example;\n\npublic class Widget {\n    public int count;\n    private int secret;\n\n    public void touch() {\n        System.out.println(\"touched\");\n    }\n}\n",
    owner_body_edit: "package com.example;\n\npublic class Widget {\n    public int count;\n    private int secret;\n\n    public void touch() {\n        System.out.println(\"touched again\");\n    }\n}\n",
    owner_field_type_change: "package com.example;\n\npublic class Widget {\n    public String count;\n    private int secret;\n\n    public void touch() {\n        System.out.println(\"touched\");\n    }\n}\n",
    owner_private_field_change: "package com.example;\n\npublic class Widget {\n    public int count;\n    private String secret;\n\n    public void touch() {\n        System.out.println(\"touched\");\n    }\n}\n",
    consumer_src: "package com.example;\n\npublic class Consumer {\n    public Widget make() {\n        Widget w = new Widget();\n        w.touch();\n        return w;\n    }\n}\n",
    // A real Java rename needs the public class name to match the new
    // file name, unlike the other 3 stacks -- Gadget.java's own class is
    // renamed to Gadget, not left as Widget.
    owner_renamed_path: "src/main/java/com/example/Gadget.java",
    owner_renamed_content: "package com.example;\n\npublic class Gadget {\n    public int count;\n    private int secret;\n\n    public void touch() {\n        System.out.println(\"touched\");\n    }\n}\n",
};

fn tempdir(tag: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "codeowl-lifecycle-{tag}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn write_all(dir: &Path, files: &[(&str, &str)]) {
    for (rel, content) in files {
        write_file(dir, rel, content);
    }
}

fn reindex(dir: &Path) -> Graph {
    RepoIndex::build(dir).unwrap().rebuild().unwrap()
}

/// Drives `next_task` -> `submit` to completion for one file, the same
/// bottom-up loop `/codeowl generate` itself runs -- real prose long
/// enough to clear `prose_smells`' `suspiciously_short` check, not a
/// placeholder short enough to trigger it.
fn drain(graph: &Graph, dir: &Path, file_id: SymbolId) {
    loop {
        match spec::next_task(graph, dir, file_id).unwrap() {
            None => break,
            Some(SpecTask::Symbol { id, .. }) => {
                spec::submit(
                    graph,
                    dir,
                    &id,
                    "### Summary\nA short but genuinely descriptive summary sentence.\n### Behavior\nA short but genuinely descriptive behavior sentence.\n",
                )
                .unwrap();
            }
            Some(SpecTask::File { id, .. }) => {
                spec::submit(
                    graph,
                    dir,
                    &id,
                    "A short but genuinely descriptive file-level summary sentence.",
                )
                .unwrap();
            }
        }
    }
}

/// `true` once `next_task` on this file's own ladder has nothing left --
/// i.e. every symbol and the file's own `## Summary` are current.
fn is_fully_current(graph: &Graph, dir: &Path, file_id: SymbolId) -> bool {
    spec::next_task(graph, dir, file_id).unwrap().is_none()
}

/// The id `next_task` offers next for this file, if it's a symbol-level
/// task -- `None` for either "fully current" or "only the file's own
/// `## Summary` is left" (this matrix never needs to tell those apart).
fn next_symbol_task_id(graph: &Graph, dir: &Path, file_id: SymbolId) -> Option<String> {
    match spec::next_task(graph, dir, file_id).unwrap() {
        Some(SpecTask::Symbol { id, .. }) => Some(id),
        _ => None,
    }
}

/// Every symbol-level id `next_task` offers for this file, draining each
/// with a placeholder as it goes, until only the file's own `## Summary`
/// (or nothing) is left. Order-independent by design: a Java consumer
/// wraps its function in a class, so the *containing* class symbol can
/// legitimately be offered before the method itself (it depends on
/// `Widget` too, transitively, through that method's body) -- Rust/TS/
/// Python's bare top-level function has no such wrapper. This checks
/// "did the cascade reach this exact symbol, anywhere in the ladder,"
/// not "was it the very next thing offered."
fn drain_symbol_task_ids(graph: &Graph, dir: &Path, file_id: SymbolId) -> Vec<String> {
    let mut ids = Vec::new();
    while let Some(id) = next_symbol_task_id(graph, dir, file_id) {
        spec::submit(
            graph,
            dir,
            &id,
            "### Summary\nA short but genuinely descriptive summary sentence.\n### Behavior\nA short but genuinely descriptive behavior sentence.\n",
        )
        .unwrap();
        ids.push(id);
    }
    ids
}

fn owner_file_id(graph: &Graph, fx: &Fixture) -> SymbolId {
    graph
        .find(fx.owner_path)
        .unwrap_or_else(|| panic!("{}: owner file not indexed", fx.stack))
}

fn consumer_file_id(graph: &Graph, fx: &Fixture) -> SymbolId {
    graph
        .find(fx.consumer_path)
        .unwrap_or_else(|| panic!("{}: consumer file not indexed", fx.stack))
}

fn run_lifecycle(fx: &Fixture) {
    let dir = tempdir(fx.stack);
    write_all(&dir, fx.scaffold);
    write_all(
        &dir,
        &[
            (fx.owner_path, fx.owner_v1),
            (fx.consumer_path, fx.consumer_src),
        ],
    );

    // --- Baseline: a full, real generate run over both files. ---
    let graph = reindex(&dir);
    let owner_id = owner_file_id(&graph, fx);
    let consumer_id = consumer_file_id(&graph, fx);
    drain(&graph, &dir, owner_id);
    drain(&graph, &dir, consumer_id);
    assert!(
        is_fully_current(&graph, &dir, owner_id),
        "{}: owner should be fully current right after its own baseline drain",
        fx.stack
    );
    assert!(
        is_fully_current(&graph, &dir, consumer_id),
        "{}: consumer should be fully current right after its own baseline drain",
        fx.stack
    );

    // --- 1. Local effect: touch()'s message changes, count's type doesn't. ---
    write_file(&dir, fx.owner_path, fx.owner_body_edit);
    let graph = reindex(&dir);
    let owner_id = owner_file_id(&graph, fx);
    let consumer_id = consumer_file_id(&graph, fx);
    assert_eq!(
        next_symbol_task_id(&graph, &dir, owner_id).as_deref(),
        Some(fx.owner_symbol_id),
        "{}: a method body edit must re-offer exactly the owner's own Widget symbol",
        fx.stack
    );
    assert!(
        is_fully_current(&graph, &dir, consumer_id),
        "{}: a method body edit must NOT cascade to the consumer -- local effect only",
        fx.stack
    );

    // --- 2. Cascade effect: count's type changes, touch()'s body doesn't. ---
    write_file(&dir, fx.owner_path, fx.owner_field_type_change);
    let graph = reindex(&dir);
    let owner_id = owner_file_id(&graph, fx);
    let consumer_id = consumer_file_id(&graph, fx);
    assert_eq!(
        next_symbol_task_id(&graph, &dir, owner_id).as_deref(),
        Some(fx.owner_symbol_id),
        "{}: a public field's type change must re-offer the owner's own Widget symbol",
        fx.stack
    );
    let consumer_offered = drain_symbol_task_ids(&graph, &dir, consumer_id);
    assert!(
        consumer_offered.contains(&fx.consumer_symbol_id.to_string()),
        "{}: a public field's type change must cascade to the consumer's own \
         make symbol somewhere in its ladder -- this is the M21.b interface_hash \
         fold this whole matrix exists to prove. Offered: {consumer_offered:?}",
        fx.stack
    );

    // --- 2b. Private field's own change: never crosses the fold. ---
    // Restore the owner to its baseline shape and re-establish a fully
    // current state for both files first -- the cascade case above left
    // the consumer's spec matching the *field-type-changed* owner, so
    // reverting the owner without redraining would itself stale the
    // consumer, confounding the assertion below with scenario 2's
    // leftover effect rather than the private field edit under test.
    write_file(&dir, fx.owner_path, fx.owner_v1);
    let graph = reindex(&dir);
    let owner_id = owner_file_id(&graph, fx);
    let consumer_id = consumer_file_id(&graph, fx);
    drain(&graph, &dir, owner_id);
    drain(&graph, &dir, consumer_id);
    assert!(
        is_fully_current(&graph, &dir, owner_id) && is_fully_current(&graph, &dir, consumer_id),
        "{}: both files must be fully current again before the private-field scenario",
        fx.stack
    );

    write_file(&dir, fx.owner_path, fx.owner_private_field_change);
    let graph = reindex(&dir);
    let owner_id = owner_file_id(&graph, fx);
    let consumer_id = consumer_file_id(&graph, fx);
    assert_eq!(
        next_symbol_task_id(&graph, &dir, owner_id).as_deref(),
        Some(fx.owner_symbol_id),
        "{}: a private field's change must still re-offer the owner's own Widget \
         symbol -- its source_hash moved even though nothing public did",
        fx.stack
    );
    assert!(
        is_fully_current(&graph, &dir, consumer_id),
        "{}: a private field's change must NOT cross the interface_hash fold -- \
         local effect only, exactly like scenario 1's method-body edit, and the \
         reason M21.b's fold only ever walks *public* fields in the first place",
        fx.stack
    );

    // Restore the owner to its baseline shape before the structural
    // scenarios below, which don't care about field/body edits at all.
    write_file(&dir, fx.owner_path, fx.owner_v1);

    // --- 3. Someone deletes a spec by hand: treated as missing again. ---
    let spec_path = dir.join("docs/specs").join(format!("{}.md", fx.owner_path));
    assert!(
        spec_path.exists(),
        "{}: the owner's spec file should exist before deleting it",
        fx.stack
    );
    std::fs::remove_file(&spec_path).unwrap();
    let graph = reindex(&dir);
    let owner_id = owner_file_id(&graph, fx);
    assert!(
        !is_fully_current(&graph, &dir, owner_id),
        "{}: a manually deleted spec must be re-offered, same as if never generated",
        fx.stack
    );
    let stored = spec::read_file_spec(&dir, fx.owner_path).unwrap();
    assert!(
        stored.is_none(),
        "{}: no spec should be on disk right after deleting it",
        fx.stack
    );

    // Re-drain to restore a clean baseline for the remaining scenarios.
    drain(&graph, &dir, owner_id);

    // --- 4. Orphan: the owner's source file is deleted, its spec survives. ---
    std::fs::remove_file(dir.join(fx.owner_path)).unwrap();
    let graph = reindex(&dir);
    let orphans = spec::find_orphaned_specs(&graph, &dir, None).unwrap();
    assert!(
        orphans
            .iter()
            .any(|o| o.kind == "file" && o.id == fx.owner_path),
        "{}: the owner's now-sourceless spec must show up as an orphan: {orphans:?}",
        fx.stack
    );

    // --- 5. Rename: the old path orphans, the new path starts missing. ---
    // (continues from the same deleted-owner state above -- a rename is
    // exactly this, plus the content reappearing at a new path)
    write_file(&dir, fx.owner_renamed_path, fx.owner_renamed_content);
    let graph = reindex(&dir);
    let orphans = spec::find_orphaned_specs(&graph, &dir, None).unwrap();
    assert!(
        orphans
            .iter()
            .any(|o| o.kind == "file" && o.id == fx.owner_path),
        "{}: the pre-rename path's spec must still show up as orphaned: {orphans:?}",
        fx.stack
    );
    let new_owner_id = graph
        .find(fx.owner_renamed_path)
        .unwrap_or_else(|| panic!("{}: renamed owner file not indexed", fx.stack));
    assert!(
        !is_fully_current(&graph, &dir, new_owner_id),
        "{}: the new path must start out needing generation, same as a brand-new file",
        fx.stack
    );
    let new_stored = spec::read_file_spec(&dir, fx.owner_renamed_path).unwrap();
    assert!(
        new_stored.is_none(),
        "{}: the new path must have no spec at all until it's generated fresh",
        fx.stack
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rust_lifecycle() {
    run_lifecycle(&RUST);
}

#[test]
fn typescript_lifecycle() {
    run_lifecycle(&TYPESCRIPT);
}

#[test]
fn python_lifecycle() {
    run_lifecycle(&PYTHON);
}

#[test]
fn java_lifecycle() {
    run_lifecycle(&JAVA);
}
