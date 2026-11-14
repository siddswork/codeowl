//! The C++ pack: walks one C++ file with tree-sitter and pulls out its
//! declarations. Planned in `docs/cpp-pack-plan.md`; the decisions are
//! `DECISIONS.md` question 28.
//!
//! What is different from the other packs:
//!
//! - **The text is blanked before it is parsed** ([`blank_macros`]): a macro
//!   standing where the grammar expects nothing would otherwise turn a class
//!   into a function. The tree is built from the blanked copy; every span,
//!   signature and hash is read from the original text at the same offsets.
//! - **Every configuration is kept.** Both branches of an `#if` are walked,
//!   so a symbol defined once per platform appears once per branch, sharing
//!   an id the way an overload does.
//! - **A namespace is not a symbol** and does not appear in an id. An
//!   anonymous namespace only makes what is inside it file-local.
//! - **"Exported" follows inclusion, not linkage.** Everything at the top
//!   level of a header can be used by any file that includes it, `static`
//!   or not. In a source file, only a name with external linkage can be
//!   reached from elsewhere (through a declaration in some header).
//!
//! Kind mapping onto [`SymbolKind`]: `class` / `struct` / `union` / `enum`
//! with a body is a `Container`; a function (definition or prototype), a
//! function-like macro and a gtest case are `Callable`; a variable, a field,
//! an enumerator, a `typedef` / `using` alias, a `concept` and an object-like
//! macro are `Value`. The concrete word lands in `raw`.
//!
//! A method defined outside its class (`Status DB::Open(…) { … }`) is
//! extracted where it is written, as a top-level `Callable` with the
//! qualified name in its id. Folding it into its class is a later step
//! (pairing, `docs/cpp-pack-plan.md` §4.6).

mod macros;
#[cfg(test)]
mod standards;

pub use self::macros::blank_macros;

use std::collections::HashMap;

use tree_sitter::{Node, Parser, Tree};

use crate::hash::hash_text;
use crate::symbol::{ExtractedSymbol, SymbolKind};

/// Parse `source` (the contents of `rel_path`) and extract its declarations.
pub fn extract_file(source: &str, rel_path: &str) -> Vec<ExtractedSymbol> {
    extract_counted(source, rel_path).0
}

/// [`extract_file`] and the number of places tree-sitter could not read,
/// from one parse. Zero means the whole file was read; a high count means
/// symbols may be missing.
pub fn extract_counted(source: &str, rel_path: &str) -> (Vec<ExtractedSymbol>, usize) {
    let (symbols, errors) = extract_with(source, rel_path, true);
    (fold_same_id(symbols), errors)
}

/// [`extract_counted`] before declarations that share an id are folded: one
/// symbol per declaration, each with its own kind and markers. For the
/// accuracy check (`examples/cpp_probe.rs`), which measures what the pack
/// reads against what the compiler reads, declaration by declaration; the
/// graph only ever sees the folded view.
#[doc(hidden)]
pub fn extract_unfolded_counted(source: &str, rel_path: &str) -> (Vec<ExtractedSymbol>, usize) {
    extract_with(source, rel_path, true)
}

/// The number of places tree-sitter could not parse, after blanking.
pub fn parse_error_count(source: &str) -> usize {
    parse(&blank_macros(source)).map_or(0, |tree| count_errors(&tree))
}

/// Error and missing nodes in `tree`. Walks with a cursor and no recursion:
/// a long chain of `+` or a big initialiser list nests hundreds of
/// thousands of levels deep, and a recursive walk of that overflows the
/// stack of a watcher thread. An error node is counted once and not
/// entered, so one broken region counts once however much is inside it.
fn count_errors(tree: &Tree) -> usize {
    let mut cursor = tree.walk();
    let mut count = 0;
    loop {
        let node = cursor.node();
        let broken = node.is_error() || node.is_missing();
        count += usize::from(broken);
        // Descend, unless this node is itself the unreadable region.
        if !broken && node.has_error() && cursor.goto_first_child() {
            continue;
        }
        // Otherwise move to the next sibling, climbing as far as needed.
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return count;
            }
        }
    }
}

fn parse(text: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_cpp::LANGUAGE.into())
        .expect("bundled tree-sitter-cpp grammar should always load");
    parser.parse(text, None)
}

/// [`extract_file`] before the fold, with blanking optional so a test can
/// show what the grammar does without it.
fn extract_with(source: &str, rel_path: &str, blank: bool) -> (Vec<ExtractedSymbol>, usize) {
    // `blanked` has the same length and line breaks as `source`, so a byte
    // range taken from the tree is valid in `source` too.
    let blanked = if blank {
        blank_macros(source)
    } else {
        source.to_string()
    };
    // `tree` owns the arena every `Node<'_>` below borrows from; nothing
    // escapes this function (CLAUDE.md's "don't let Node<'a> escape" rule).
    let Some(tree) = parse(&blanked) else {
        return (Vec::new(), 0);
    };
    let errors = count_errors(&tree);
    let cx = Cx {
        src: source,
        file: rel_path,
        header: is_header(rel_path),
    };
    let mut out = Vec::new();
    let mut scope = Scope {
        parent: None,
        anon: false,
        access: None,
    };
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        cx.visit(child, child, &mut scope, &mut out);
    }
    (out, errors)
}

/// Merge symbols that share an id into one, so the graph (which keeps one
/// symbol per id) loses nothing and an edge to the id sees every declaration.
///
/// C++ gives several declarations one id: overloads, a prototype and its
/// definition, both branches of an `#if`, a template and its specialisation.
/// The strongest kind (`Container`, then `Callable`, then `Value`) supplies
/// the kind, signature and main span, ties going to the first. The others
/// add their line ranges to `extra_spans` and their hashes into both of the
/// survivor's, so an edit to any declaration moves the interface hash that
/// dependants read. Children are merged by id; a child that was declared in
/// both halves is listed once. The same move as `typescript::fold_same_id`,
/// applied to members too, since an overloaded method shares an id as well.
fn fold_same_id(symbols: Vec<ExtractedSymbol>) -> Vec<ExtractedSymbol> {
    fn strength(kind: &SymbolKind) -> u8 {
        match kind {
            SymbolKind::Container => 2,
            SymbolKind::Callable => 1,
            SymbolKind::Value | SymbolKind::Schema => 0,
        }
    }

    let mut folded: Vec<ExtractedSymbol> = Vec::with_capacity(symbols.len());
    let mut index: HashMap<String, usize> = HashMap::new();
    for mut sym in symbols {
        let Some(&at) = index.get(&sym.id) else {
            index.insert(sym.id.clone(), folded.len());
            folded.push(sym);
            continue;
        };
        let first = &mut folded[at];
        // Equal kinds: a declaration with a body outranks a bare prototype,
        // so a prototype written first does not stand for its definition;
        // then one another file can name (a public or protected member, or
        // a non-member) outranks a private one, so the symbol is described
        // by the overload a user can call.
        let rank = |s: &ExtractedSymbol| {
            let private = s.markers.iter().any(|m| m == "private");
            (
                strength(&s.kind),
                !s.markers.iter().any(|m| m == "declaration"),
                !private,
            )
        };
        if rank(&sym) > rank(first) {
            std::mem::swap(first, &mut sym);
        }
        first.docstring = first.docstring.take().or(sym.docstring);
        first.source_hash = hash_text(&format!("{}\n{}", first.source_hash, sym.source_hash));
        first.interface_hash = match (first.interface_hash.take(), sym.interface_hash) {
            (Some(a), Some(b)) => Some(hash_text(&format!("{a}\n{b}"))),
            (a, b) => a.or(b),
        };
        first.is_exported |= sym.is_exported;
        first.extra_spans.push(sym.lines);
        first.extra_spans.extend(sym.extra_spans);
        // "declaration" means the symbol has no body anywhere: it survives
        // only if every folded declaration carries it.
        let declared = first.markers.iter().any(|m| m == "declaration")
            && sym.markers.iter().any(|m| m == "declaration");
        first.markers.retain(|m| m != "declaration");
        for marker in sym.markers {
            if marker != "declaration" && !first.markers.contains(&marker) {
                first.markers.push(marker);
            }
        }
        if declared {
            first.markers.push("declaration".to_string());
        }
        first.children.extend(sym.children);
    }
    // A class that lists a member in both `#if` branches names it twice.
    for sym in &mut folded {
        let mut seen = std::collections::HashSet::new();
        sym.children.retain(|c| seen.insert(c.clone()));
    }
    folded
}

/// Extensions of files other files include (template implementation files
/// and module interface units among them).
const HEADER_EXTENSIONS: &[&str] = &[
    "h", "hpp", "hh", "hxx", "ipp", "inl", "tpp", "tcc", "cppm", "ixx",
];

/// Extensions of files compiled by themselves. Plain C (`.c`) is not read.
const SOURCE_EXTENSIONS: &[&str] = &["cc", "cpp", "cxx"];

/// The extension in lower case: `.H` and `.HPP` exist.
fn extension(rel_path: &str) -> String {
    let name = rel_path.rsplit('/').next().unwrap_or(rel_path);
    name.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Whether the pack reads this file at all. The one list `CppStack` and
/// [`is_header`] share, so a header extension cannot be read as a source file.
pub fn is_cpp_file(rel_path: &str) -> bool {
    let ext = extension(rel_path);
    HEADER_EXTENSIONS.contains(&ext.as_str()) || SOURCE_EXTENSIONS.contains(&ext.as_str())
}

/// A file other files include. Decides what "exported" means (module doc).
fn is_header(rel_path: &str) -> bool {
    HEADER_EXTENSIONS.contains(&extension(rel_path).as_str())
}

/// What every visit needs and none changes.
struct Cx<'a> {
    /// The original text. Offsets from the tree index into this.
    src: &'a str,
    file: &'a str,
    header: bool,
}

/// Where the walk currently is.
#[derive(Clone)]
struct Scope {
    /// The id of the enclosing class, or `None` at namespace scope.
    parent: Option<String>,
    /// Inside an anonymous namespace.
    anon: bool,
    /// The access level in force inside a class body (`public`, `protected`,
    /// `private`); `None` at namespace scope. An `access_specifier` updates
    /// it in place, which is why visits take the scope by `&mut`.
    access: Option<&'static str>,
}

/// How a declarator reads once its name is found.
struct Declared {
    name: String,
    /// It has a parameter list: a function, not a variable.
    is_function: bool,
    /// The node holding the name and, for a function, its parameters: the
    /// end of a variable's signature (what follows is its initialiser).
    end_byte: usize,
}

impl<'a> Cx<'a> {
    fn text(&self, node: Node) -> &'a str {
        self.src.get(node.byte_range()).unwrap_or_default()
    }

    /// Handle one node. `span` is the outermost node of the declaration (the
    /// `template<…>` wrapper when there is one): it gives the lines, the
    /// start of the signature and the doc comment.
    fn visit(&self, node: Node, span: Node, scope: &mut Scope, out: &mut Vec<ExtractedSymbol>) {
        match node.kind() {
            "namespace_definition" => {
                let mut inner = scope.clone();
                inner.anon = scope.anon || node.child_by_field_name("name").is_none();
                if let Some(body) = node.child_by_field_name("body") {
                    self.visit_children(body, &mut inner, out);
                }
            }
            // `extern "C" { … }` or `extern "C" int f();`
            "linkage_specification" => {
                if let Some(body) = node.child_by_field_name("body") {
                    if body.kind() == "declaration_list" {
                        self.visit_children(body, scope, out);
                    } else {
                        self.visit(body, body, scope, out);
                    }
                }
            }
            // Every branch of a conditional is read; an unparsed region is
            // searched for the declarations tree-sitter did recover.
            "declaration_list" | "preproc_if" | "preproc_ifdef" | "preproc_else"
            | "preproc_elif" | "preproc_elifdef" | "ERROR" => {
                self.visit_children(node, scope, out);
            }
            // A C++20 `export { … }` block: with `export` blanked it parses
            // as a bare block between declarations. Any other bare block is
            // the body of something whose head did not parse (a Catch2
            // `TEST_CASE("…") { … }`), and its locals are not declarations.
            "compound_statement" if self.follows_export(node) => {
                self.visit_children(node, scope, out);
            }
            "template_declaration" => {
                let mut cursor = node.walk();
                let inner = node.named_children(&mut cursor).find(|c| {
                    !matches!(
                        c.kind(),
                        "template_parameter_list" | "requires_clause" | "comment"
                    )
                });
                if let Some(inner) = inner {
                    self.visit(inner, span, scope, out);
                }
            }
            "access_specifier" => {
                scope.access = match self.text(node).trim() {
                    "public" => Some("public"),
                    "protected" => Some("protected"),
                    "private" => Some("private"),
                    _ => scope.access,
                };
            }
            "class_specifier" | "struct_specifier" | "union_specifier" => {
                let name = node.child_by_field_name("name").map(|n| self.text(n));
                if let (Some(name), Some(_)) = (name, node.child_by_field_name("body")) {
                    self.container(node, span, name, scope, out);
                }
            }
            "enum_specifier" => self.enumeration(node, span, None, scope, out),
            "function_definition" => self.function(node, span, scope, out),
            "declaration" | "field_declaration" => self.declaration(node, span, scope, out),
            "type_definition" => self.type_definition(node, span, scope, out),
            "alias_declaration" => {
                if let Some(name) = node.child_by_field_name("name") {
                    let end = name.end_byte();
                    self.leaf(
                        span,
                        self.text(name),
                        SymbolKind::Value,
                        "using",
                        end,
                        scope,
                        out,
                    );
                }
            }
            "concept_definition" => {
                if let Some(name) = node.child_by_field_name("name") {
                    let end = name.end_byte();
                    self.leaf(
                        span,
                        self.text(name),
                        SymbolKind::Value,
                        "concept",
                        end,
                        scope,
                        out,
                    );
                }
            }
            "preproc_def" | "preproc_function_def" => self.macro_definition(node, out),
            _ => {}
        }
    }

    /// Whether the original text has the keyword `export` directly in front
    /// of `node`.
    fn follows_export(&self, node: Node) -> bool {
        let before = self
            .src
            .get(..node.start_byte())
            .unwrap_or_default()
            .trim_end();
        before
            .strip_suffix("export")
            .is_some_and(|rest| !rest.ends_with(|c: char| c.is_alphanumeric() || c == '_'))
    }

    fn visit_children(&self, node: Node, scope: &mut Scope, out: &mut Vec<ExtractedSymbol>) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit(child, child, scope, out);
        }
    }

    fn id(&self, scope: &Scope, name: &str) -> String {
        match &scope.parent {
            Some(parent) => format!("{parent}::{name}"),
            None => format!("{}::{name}", self.file),
        }
    }

    /// Reachable from another file? See the module doc.
    fn exported(&self, scope: &Scope, is_static: bool) -> bool {
        if scope.parent.is_some() {
            // A member is never depended on by itself, only through its class.
            return false;
        }
        self.header || !(scope.anon || is_static)
    }

    /// The access level as a marker, for a member.
    fn markers(scope: &Scope) -> Vec<String> {
        scope.access.iter().map(|a| a.to_string()).collect()
    }

    /// `src[start..end]`, trimmed, without a trailing `;` or the `=` that
    /// introduces an initialiser cut off at `end`.
    fn signature(&self, start: usize, end: usize) -> String {
        self.src
            .get(start..end.max(start))
            .unwrap_or_default()
            .trim()
            .trim_end_matches([';', '='])
            .trim_end()
            .to_string()
    }

    /// A symbol with no children. `sig_end` is where its signature stops.
    #[allow(clippy::too_many_arguments)]
    fn leaf(
        &self,
        span: Node,
        name: &str,
        kind: SymbolKind,
        raw: &str,
        sig_end: usize,
        scope: &Scope,
        out: &mut Vec<ExtractedSymbol>,
    ) {
        let sig = self.signature(span.start_byte(), sig_end);
        // In a source file a type alias has no linkage at all, and a `const`
        // variable has internal linkage unless declared `extern`.
        let internal = !self.header
            && scope.parent.is_none()
            && (matches!(raw, "using" | "typedef" | "concept")
                || (raw == "variable"
                    && has_const(span, self.src)
                    && !has_storage_class(span, self.src, "extern")));
        let is_exported =
            !internal && self.exported(scope, has_storage_class(span, self.src, "static"));
        out.push(ExtractedSymbol {
            id: self.id(scope, name),
            kind,
            raw: raw.to_string(),
            file: self.file.to_string(),
            lines: node_lines(span),
            signature: sig.clone(),
            docstring: self.leading_doc(span),
            is_exported,
            source_hash: hash_text(self.text(span)),
            interface_hash: is_exported.then(|| hash_text(&sig)),
            markers: Self::markers(scope),
            extra_spans: Vec::new(),
            parent: scope.parent.clone(),
            children: Vec::new(),
        });
    }

    /// A `class` / `struct` / `union` with a body: its members, then itself,
    /// with the members' hashes folded into its own (as the Java pack does).
    fn container(
        &self,
        node: Node,
        span: Node,
        name: &str,
        scope: &Scope,
        out: &mut Vec<ExtractedSymbol>,
    ) {
        let raw = match node.kind() {
            "struct_specifier" => "struct",
            "union_specifier" => "union",
            _ => "class",
        };
        let id = self.id(scope, &plain_name(name));
        let before = out.len();
        let mut inner = Scope {
            parent: Some(id.clone()),
            anon: scope.anon,
            // What a member gets before the first `public:` / `private:`.
            access: Some(if raw == "class" { "private" } else { "public" }),
        };
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_children(body, &mut inner, out);
        }
        let sig_end = node
            .child_by_field_name("body")
            .map_or(node.end_byte(), |b| b.start_byte());
        self.push_container(span, id, raw, sig_end, before, scope, out);
    }

    /// An `enum` with a body. `name_override` names an anonymous enum that a
    /// `typedef` names. With no name at all the enumerators belong to the
    /// enclosing scope.
    fn enumeration(
        &self,
        node: Node,
        span: Node,
        name_override: Option<&str>,
        scope: &Scope,
        out: &mut Vec<ExtractedSymbol>,
    ) {
        let Some(body) = node.child_by_field_name("body") else {
            return;
        };
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n))
            .or(name_override);
        let id = name.map(|n| self.id(scope, &plain_name(n)));
        let before = out.len();
        let inner = match &id {
            Some(id) => Scope {
                parent: Some(id.clone()),
                anon: scope.anon,
                // An enumerator is as visible as its enum.
                access: Some("public"),
            },
            None => scope.clone(),
        };
        let mut cursor = body.walk();
        for e in body.named_children(&mut cursor) {
            if e.kind() != "enumerator" {
                continue;
            }
            if let Some(n) = e.child_by_field_name("name") {
                self.leaf(
                    e,
                    self.text(n),
                    SymbolKind::Value,
                    "enumerator",
                    e.end_byte(),
                    &inner,
                    out,
                );
            }
        }
        if let Some(id) = id {
            self.push_container(span, id, "enum", body.start_byte(), before, scope, out);
        }
    }

    /// Insert the container in front of the members pushed since `before`.
    #[allow(clippy::too_many_arguments)]
    fn push_container(
        &self,
        span: Node,
        id: String,
        raw: &str,
        sig_end: usize,
        before: usize,
        scope: &Scope,
        out: &mut Vec<ExtractedSymbol>,
    ) {
        // Direct children only: a nested class has already folded its own.
        let direct: Vec<&ExtractedSymbol> = out[before..]
            .iter()
            .filter(|s| s.parent.as_deref() == Some(id.as_str()))
            .collect();
        let sig = self.signature(span.start_byte(), sig_end);
        let mut rollup = sig.clone();
        let mut iface = sig.clone();
        for s in &direct {
            rollup.push('\n');
            rollup.push_str(&s.source_hash);
            // The interface is the declaration plus every member another
            // file can name: `public` and `protected` ones.
            if s.markers.iter().any(|m| m == "public" || m == "protected") {
                iface.push('\n');
                iface.push_str(&s.signature);
            }
        }
        // A nested class is reachable through its outer class when the outer
        // class lets it be named.
        let is_exported = match scope.parent {
            None => self.header || !scope.anon,
            Some(_) => matches!(scope.access, Some("public" | "protected")),
        };
        let children = direct.iter().map(|s| s.id.clone()).collect();
        out.insert(
            before,
            ExtractedSymbol {
                id,
                kind: SymbolKind::Container,
                raw: raw.to_string(),
                file: self.file.to_string(),
                lines: node_lines(span),
                signature: sig,
                docstring: self.leading_doc(span),
                is_exported,
                source_hash: hash_text(&rollup),
                interface_hash: is_exported.then(|| hash_text(&iface)),
                markers: Self::markers(scope),
                extra_spans: Vec::new(),
                parent: scope.parent.clone(),
                children,
            },
        );
    }

    /// A function with a body (or `= default` / `= delete`).
    fn function(&self, node: Node, span: Node, scope: &Scope, out: &mut Vec<ExtractedSymbol>) {
        let Some(declarator) = node.child_by_field_name("declarator") else {
            return;
        };
        let Some(mut d) = self.declared(declarator) else {
            return;
        };
        // `constexpr uint128::operator int() const { … }`: with a specifier
        // in front, the grammar reads `uint128::operator` as the return type
        // and `int` as the function's name. Put the two halves back together.
        if let Some(scope) = node
            .child_by_field_name("type")
            .map(|t| one_line(self.text(t)))
            .as_deref()
            .and_then(|t| t.strip_suffix("::operator"))
        {
            d.name = format!("{}::operator {}", plain_name(scope), d.name);
        }
        // The signature stops at the constructor's initialiser list, or at
        // the body.
        let mut cursor = node.walk();
        let sig_end = node
            .children(&mut cursor)
            .find(|c| {
                matches!(
                    c.kind(),
                    "field_initializer_list" | "compound_statement" | "try_statement"
                )
            })
            .map_or(node.end_byte(), |c| c.start_byte());

        // gtest: `TEST(Suite, Name) { … }` has no return type and reads as a
        // function called `TEST` with two parameters.
        if node.child_by_field_name("type").is_none()
            && matches!(
                d.name.as_str(),
                "TEST" | "TEST_F" | "TEST_P" | "TYPED_TEST" | "TYPED_TEST_P"
            )
            && let Some(name) = self.test_name(declarator)
        {
            self.leaf(
                span,
                &name,
                SymbolKind::Callable,
                "test",
                sig_end,
                scope,
                out,
            );
            return;
        }
        let raw = self.function_raw(&d.name, scope, "function");
        self.leaf(
            span,
            &d.name,
            SymbolKind::Callable,
            raw,
            sig_end,
            scope,
            out,
        );
    }

    /// `Suite.Name` from the two "parameters" of a gtest macro.
    fn test_name(&self, declarator: Node) -> Option<String> {
        let params = declarator.child_by_field_name("parameters")?;
        let mut cursor = params.walk();
        let parts: Vec<&str> = params
            .named_children(&mut cursor)
            .map(|p| self.text(p).trim())
            .collect();
        (parts.len() == 2).then(|| format!("{}.{}", parts[0], parts[1]))
    }

    /// The `raw` word for a function. `plain` is what it is called when it
    /// is neither a member nor qualified.
    fn function_raw(&self, name: &str, scope: &Scope, plain: &'static str) -> &'static str {
        match &scope.parent {
            Some(parent) => {
                let class = parent.rsplit("::").next().unwrap_or(parent);
                if name == class {
                    "constructor"
                } else if name.starts_with('~') {
                    "destructor"
                } else {
                    "method"
                }
            }
            // `A::b` at namespace scope: a member of class `A` defined
            // outside it, or a function of namespace `A`. Pairing decides.
            None if name.contains("::") && !name.starts_with("operator") => "qualified_function",
            None => plain,
        }
    }

    /// `int a, *b = 0;`, `Status Open();`, `struct S { … } s;`: one symbol
    /// per declarator, plus a class or enum defined in the declared type.
    fn declaration(&self, node: Node, span: Node, scope: &Scope, out: &mut Vec<ExtractedSymbol>) {
        if let Some(ty) = node.child_by_field_name("type") {
            self.type_with_body(ty, None, scope, out);
        }
        // A default member initialiser (`int x = 3;`, `int y{4};`) is not
        // part of the signature.
        let default_start = node
            .child_by_field_name("default_value")
            .map(|v| v.start_byte());
        let lone = node
            .children_by_field_name("declarator", &mut node.walk())
            .count()
            == 1;
        let mut cursor = node.walk();
        for declarator in node.children_by_field_name("declarator", &mut cursor) {
            let Some(d) = self.declared(declarator) else {
                continue;
            };
            if d.is_function {
                let raw = self.function_raw(&d.name, scope, "prototype");
                let end = node.end_byte();
                self.leaf(span, &d.name, SymbolKind::Callable, raw, end, scope, out);
                // Declared here, defined elsewhere (or nowhere). Pairing a
                // declaration with its body needs to know which is which, and
                // for a member `raw` says "method" either way.
                if let Some(pushed) = out.last_mut() {
                    pushed.markers.push("declaration".to_string());
                }
            } else {
                let raw = if scope.parent.is_some() {
                    "field"
                } else {
                    "variable"
                };
                // `int a, b = 5;` has one `default_value` for the last name;
                // each other declarator stops after its own name.
                let end = match default_start {
                    Some(s) if lone => s,
                    _ => d.end_byte,
                };
                self.leaf(span, &d.name, SymbolKind::Value, raw, end, scope, out);
            }
        }
    }

    /// `typedef int Handle;`, `typedef struct { … } Point;`
    fn type_definition(
        &self,
        node: Node,
        span: Node,
        scope: &Scope,
        out: &mut Vec<ExtractedSymbol>,
    ) {
        let mut cursor = node.walk();
        let names: Vec<Declared> = node
            .children_by_field_name("declarator", &mut cursor)
            .filter_map(|d| self.declared(d))
            .collect();
        let mut named_by_typedef = false;
        if let Some(ty) = node.child_by_field_name("type") {
            let first = names.first().map(|d| d.name.as_str());
            named_by_typedef = self.type_with_body(ty, first, scope, out);
        }
        // `typedef struct Foo { … } Foo;`: the typedef repeats the struct's
        // own name, so it is the same thing and gets no symbol of its own.
        let repeats_tag = node
            .child_by_field_name("type")
            // Only a definition: `typedef struct X X;` names a forward
            // declaration and still gets its alias.
            .filter(|ty| ty.child_by_field_name("body").is_some())
            .and_then(|ty| ty.child_by_field_name("name"))
            .map(|n| plain_name(self.text(n)))
            .zip(names.first())
            .is_some_and(|(tag, d)| tag == d.name);
        for (i, d) in names.iter().enumerate() {
            // The name already given to an anonymous struct is that struct.
            if i == 0 && (named_by_typedef || repeats_tag) {
                continue;
            }
            self.leaf(
                span,
                &d.name,
                SymbolKind::Value,
                "typedef",
                node.end_byte(),
                scope,
                out,
            );
        }
    }

    /// If `ty` is a class or enum defined in place, extract it. An anonymous
    /// one takes `fallback_name`; the return value says whether it did.
    fn type_with_body(
        &self,
        ty: Node,
        fallback_name: Option<&str>,
        scope: &Scope,
        out: &mut Vec<ExtractedSymbol>,
    ) -> bool {
        if ty.child_by_field_name("body").is_none() {
            return false;
        }
        let own_name = ty.child_by_field_name("name").map(|n| self.text(n));
        match ty.kind() {
            "class_specifier" | "struct_specifier" | "union_specifier" => {
                if let Some(name) = own_name.or(fallback_name) {
                    self.container(ty, ty, name, scope, out);
                }
            }
            "enum_specifier" => self.enumeration(ty, ty, fallback_name, scope, out),
            _ => return false,
        }
        own_name.is_none() && fallback_name.is_some()
    }

    /// `#define NAME value` / `#define NAME(args) value`. An include guard
    /// (`#ifndef X` / `#define X`) is not a symbol.
    fn macro_definition(&self, node: Node, out: &mut Vec<ExtractedSymbol>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = self.text(name_node);
        let has_value = node.child_by_field_name("value").is_some();
        let guards = node.parent().is_some_and(|p| {
            p.kind() == "preproc_ifdef"
                && p.child_by_field_name("name").map(|n| self.text(n)) == Some(name)
        });
        if guards && !has_value {
            return;
        }
        let (kind, sig_end) = match node.child_by_field_name("parameters") {
            Some(params) => (SymbolKind::Callable, params.end_byte()),
            None => (SymbolKind::Value, name_node.end_byte()),
        };
        // A macro ignores namespaces and classes: it is always file-level,
        // and visible to includers only when defined in a header.
        let sig = self.signature(node.start_byte(), sig_end);
        let is_exported = self.header;
        out.push(ExtractedSymbol {
            id: format!("{}::{name}", self.file),
            kind,
            raw: "macro".to_string(),
            file: self.file.to_string(),
            lines: node_lines(node),
            signature: sig.clone(),
            docstring: self.leading_doc(node),
            is_exported,
            source_hash: hash_text(self.text(node)),
            interface_hash: is_exported.then(|| hash_text(&sig)),
            markers: Vec::new(),
            extra_spans: Vec::new(),
            parent: None,
            children: Vec::new(),
        });
    }

    /// Find the declared name inside a declarator.
    fn declared(&self, declarator: Node) -> Option<Declared> {
        let mut node = declarator;
        let mut is_function = false;
        let mut end_byte = declarator.end_byte();
        loop {
            match node.kind() {
                "function_declarator" => {
                    let inner = node.child_by_field_name("declarator")?;
                    // `A (max)() { … }` with no return type the grammar can
                    // see: it reads `A(max)` as a function taking a `max`
                    // and the real parameter list as a second call. The
                    // name is the lone "parameter type" of the inner one.
                    if inner.kind() == "function_declarator"
                        && let Some(name) = self.lone_parameter_type(inner)
                    {
                        return Some(Declared {
                            name: plain_name(name),
                            is_function: true,
                            end_byte: node.end_byte(),
                        });
                    }
                    // `int (*handler)(int);` declares a variable that holds a
                    // function, not a function. `int (min)(int, int)` is a
                    // function whose name is only wrapped in parentheses.
                    let wraps_a_plain_name = inner.kind() == "parenthesized_declarator"
                        && inner.named_child_count() == 1
                        && inner.named_child(0).is_some_and(|c| {
                            matches!(
                                c.kind(),
                                "identifier" | "field_identifier" | "qualified_identifier"
                            )
                        });
                    if inner.kind() != "parenthesized_declarator" || wraps_a_plain_name {
                        is_function = true;
                    }
                    end_byte = node.end_byte();
                    node = inner;
                }
                "init_declarator" => {
                    node = node.child_by_field_name("declarator")?;
                    end_byte = node.end_byte();
                }
                "pointer_declarator"
                | "reference_declarator"
                | "parenthesized_declarator"
                | "array_declarator"
                | "attributed_declarator" => {
                    node = node.child_by_field_name("declarator").or_else(|| {
                        let mut cursor = node.walk();
                        node.named_children(&mut cursor)
                            .filter(|c| {
                                c.kind() != "type_qualifier" && c.kind() != "ms_pointer_modifier"
                            })
                            .last()
                    })?;
                }
                // `operator double() const`
                "operator_cast" => {
                    return Some(Declared {
                        name: self.conversion_name(node)?,
                        is_function: true,
                        end_byte: node.end_byte(),
                    });
                }
                // `V::operator bool() const`, defined outside the class: the
                // conversion operator sits at the end of the qualified name.
                "qualified_identifier" if self.innermost_name(node).kind() == "operator_cast" => {
                    let cast = self.innermost_name(node);
                    let scope = self
                        .src
                        .get(node.start_byte()..cast.start_byte())
                        .unwrap_or_default();
                    return Some(Declared {
                        name: format!("{}{}", plain_name(scope), self.conversion_name(cast)?),
                        is_function: true,
                        end_byte: node.end_byte(),
                    });
                }
                "identifier"
                | "field_identifier"
                | "type_identifier"
                | "qualified_identifier"
                | "destructor_name"
                | "operator_name"
                | "template_function"
                | "template_method" => {
                    return Some(Declared {
                        name: plain_name(self.text(node)),
                        is_function,
                        end_byte,
                    });
                }
                _ => return None,
            }
        }
    }

    /// The last `name` of a (possibly nested) qualified identifier:
    /// `ns::V::operator bool` gives the `operator bool` node.
    fn innermost_name<'t>(&self, mut node: Node<'t>) -> Node<'t> {
        while node.kind() == "qualified_identifier"
            && let Some(name) = node.child_by_field_name("name")
        {
            node = name;
        }
        node
    }

    /// `operator const char*` for an `operator_cast` node: its text up to
    /// the parameter list, so qualifiers and pointers stay in the name.
    fn conversion_name(&self, cast: Node) -> Option<String> {
        // The declarator chain (`*`, `&`) ends in the `()` of the operator.
        let mut d = cast.child_by_field_name("declarator")?;
        while d.kind() != "abstract_function_declarator" {
            d = d.child_by_field_name("declarator")?;
        }
        let params = d.child_by_field_name("parameters")?;
        let text = self.src.get(cast.start_byte()..params.start_byte())?;
        Some(one_line(text))
    }

    /// If `declarator` (a function declarator) has exactly one parameter and
    /// that parameter is a bare type with no name, the type's text.
    fn lone_parameter_type(&self, declarator: Node) -> Option<&'a str> {
        let params = declarator.child_by_field_name("parameters")?;
        if params.named_child_count() != 1 {
            return None;
        }
        let only = params.named_child(0)?;
        if only.kind() != "parameter_declaration"
            || only.child_by_field_name("declarator").is_some()
        {
            return None;
        }
        only.child_by_field_name("type").map(|t| self.text(t))
    }

    /// The comment block directly above `node`: a run of `//` lines or one
    /// `/* … */`, with no blank line between it and the declaration. A
    /// comment that trails the previous declaration on its own line is not
    /// part of it.
    fn leading_doc(&self, node: Node) -> Option<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut expected_row = node.start_position().row;
        let mut cursor = node.prev_sibling();
        while let Some(n) = cursor {
            if n.kind() != "comment" {
                break;
            }
            let end = n.end_position();
            if end.row + usize::from(end.column > 0) != expected_row {
                break;
            }
            // `int a;  // about a` then, on the next line, this declaration.
            let trails = n
                .prev_sibling()
                .is_some_and(|p| p.end_position().row == n.start_position().row);
            if trails {
                break;
            }
            lines.extend(comment_body(self.text(n)).into_iter().rev());
            expected_row = n.start_position().row;
            cursor = n.prev_sibling();
        }
        if lines.is_empty() {
            return None;
        }
        lines.reverse();
        Some(lines.join("\n"))
    }
}

/// A comment's text without its markers, one entry per non-empty line.
fn comment_body(raw: &str) -> Vec<String> {
    let inner = raw
        .strip_prefix("/*")
        .and_then(|s| s.strip_suffix("*/"))
        .unwrap_or(raw);
    inner
        .lines()
        .map(|l| {
            let l = l.trim();
            let l = if l.starts_with("//") {
                // `//`, `///`, `//!`, `///<`: the marker, once.
                let l = l.trim_start_matches('/');
                l.strip_prefix(['!', '<']).unwrap_or(l)
            } else {
                // A block comment's continuation line: one leading `*`.
                l.strip_prefix('*').unwrap_or(l)
            };
            l.trim().to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

/// Whitespace collapsed to single spaces.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A name as it goes into an id: on one line, no leading `::`, and without
/// template arguments, so a specialisation shares its id with the primary
/// template the way an overload shares one. An operator's name is kept
/// whole (`operator<`, `operator()`).
fn plain_name(text: &str) -> String {
    let name: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let name = name.trim_start_matches("::");
    if let Some(at) = name.find("operator") {
        // `operator+`, `operator()`: no space. `operator new`, `operator
        // delete[]`, `operator bool`: the keyword stays a separate word.
        let (head, rest) = name.split_at(at + "operator".len());
        let rest = rest.trim_start();
        let keyword = rest.starts_with(|c: char| c.is_alphabetic() || c == '_');
        let rest: String = if keyword {
            format!(" {rest}")
        } else {
            rest.split_whitespace().collect()
        };
        return format!("{head}{rest}");
    }
    let name: String = name.split_whitespace().collect();
    let name = name.as_str();
    let mut out = String::new();
    let mut depth = 0usize;
    for c in name.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Whether the declaration is `const` or `constexpr`.
fn has_const(node: Node, src: &str) -> bool {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        let mut cursor = n.walk();
        for c in n.children(&mut cursor) {
            match c.kind() {
                "type_qualifier"
                    if matches!(src.get(c.byte_range()), Some("const" | "constexpr")) =>
                {
                    return true;
                }
                "declaration" if n.kind() == "template_declaration" => stack.push(c),
                _ => {}
            }
        }
    }
    false
}

/// Whether the declaration carries the given storage class (`static`).
fn has_storage_class(node: Node, src: &str, word: &str) -> bool {
    // Under a `template<…>` wrapper the specifier sits on the wrapped node.
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        let mut cursor = n.walk();
        for c in n.children(&mut cursor) {
            match c.kind() {
                "storage_class_specifier" if src.get(c.byte_range()) == Some(word) => return true,
                "declaration" | "function_definition" if n.kind() == "template_declaration" => {
                    stack.push(c)
                }
                _ => {}
            }
        }
    }
    false
}

fn node_lines(node: Node) -> [usize; 2] {
    [node.start_position().row + 1, node.end_position().row + 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(src: &str) -> Vec<ExtractedSymbol> {
        extract_file(src, "db.h")
    }

    fn source(src: &str) -> Vec<ExtractedSymbol> {
        extract_file(src, "db.cc")
    }

    #[track_caller]
    fn get<'a>(syms: &'a [ExtractedSymbol], id: &str) -> &'a ExtractedSymbol {
        syms.iter().find(|s| s.id == id).unwrap_or_else(|| {
            let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
            panic!("no symbol `{id}`; have {ids:?}")
        })
    }

    fn ids(syms: &[ExtractedSymbol]) -> Vec<&str> {
        syms.iter().map(|s| s.id.as_str()).collect()
    }

    const DB: &str = "\
// A persistent ordered map.
// Safe for concurrent access.
class LEVELDB_EXPORT DB {
 public:
  static Status Open(const Options& options, DB** dbptr);
  DB() = default;
  virtual ~DB();
  virtual Status Put(const Slice& key) = 0;
  int count() const { return count_; }

 private:
  int count_ GUARDED_BY(mu_) = 0;
};
";

    #[test]
    fn a_class_behind_an_export_macro_is_a_container_with_its_members() {
        let syms = header(DB);
        let db = get(&syms, "db.h::DB");
        assert_eq!(db.kind, SymbolKind::Container);
        assert_eq!(db.raw, "class");
        assert_eq!(db.signature, "class LEVELDB_EXPORT DB");
        assert_eq!(
            db.docstring.as_deref(),
            Some("A persistent ordered map.\nSafe for concurrent access.")
        );
        assert_eq!(db.lines, [3, 13]);
        assert!(db.is_exported);
        assert_eq!(
            db.children,
            [
                "db.h::DB::Open",
                "db.h::DB::DB",
                "db.h::DB::~DB",
                "db.h::DB::Put",
                "db.h::DB::count",
                "db.h::DB::count_"
            ]
        );
        // Parent first, then members, in declaration order.
        assert_eq!(ids(&syms)[0], "db.h::DB");

        let open = get(&syms, "db.h::DB::Open");
        assert_eq!(open.kind, SymbolKind::Callable);
        assert_eq!(open.raw, "method");
        assert_eq!(
            open.signature,
            "static Status Open(const Options& options, DB** dbptr)"
        );
        assert_eq!(open.markers, ["public", "declaration"]);
        // `count` has a body here, so it is not a bare declaration.
        assert_eq!(get(&syms, "db.h::DB::count").markers, ["public"]);
        assert_eq!(open.parent.as_deref(), Some("db.h::DB"));
        assert!(!open.is_exported, "a member is reached through its class");

        assert_eq!(get(&syms, "db.h::DB::DB").raw, "constructor");
        assert_eq!(get(&syms, "db.h::DB::~DB").raw, "destructor");
        assert_eq!(get(&syms, "db.h::DB::count").signature, "int count() const");

        let field = get(&syms, "db.h::DB::count_");
        assert_eq!(field.kind, SymbolKind::Value);
        assert_eq!(field.raw, "field");
        assert_eq!(field.markers, ["private"]);
        // The original text, annotation included, and no initialiser.
        assert_eq!(field.signature, "int count_ GUARDED_BY(mu_)");
    }

    #[test]
    fn without_blanking_the_grammar_loses_the_class() {
        // The negative control for the test above: the same text, parsed as
        // written, yields no container at all.
        let syms = extract_with(DB, "db.h", false).0;
        assert!(
            !syms.iter().any(|s| s.kind == SymbolKind::Container),
            "{:?}",
            ids(&syms)
        );
    }

    #[test]
    fn interface_hash_follows_public_members_only() {
        let base = "class C {\n public:\n  int a();\n private:\n  int b();\n};\n";
        let iface = |src: &str| get(&header(src), "db.h::C").interface_hash.clone();
        let whole = |src: &str| get(&header(src), "db.h::C").source_hash.clone();

        let public_changed = base.replace("int a();", "long a();");
        assert_ne!(iface(base), iface(&public_changed));

        let private_changed = base.replace("int b();", "long b();");
        assert_eq!(iface(base), iface(&private_changed));
        assert_ne!(whole(base), whole(&private_changed));
    }

    #[test]
    fn a_body_edit_moves_the_source_hash_and_not_the_interface() {
        let a = "struct S {\n  int f() { return 1; }\n};\n";
        let b = "struct S {\n  int f() { return 2; }\n};\n";
        let (sa, sb) = (header(a), header(b));
        let (ca, cb) = (get(&sa, "db.h::S"), get(&sb, "db.h::S"));
        assert_ne!(ca.source_hash, cb.source_hash);
        assert_eq!(ca.interface_hash, cb.interface_hash);
        // A struct's members are public unless it says otherwise.
        assert_eq!(get(&sa, "db.h::S::f").markers, ["public"]);
    }

    #[test]
    fn free_functions_and_what_is_reachable_from_another_file() {
        let src = "\
Status DestroyDB(const std::string& name);
static int Helper(int a) { return a; }
namespace {
int Hidden() { return 1; }
}
int Visible() { return 2; }
";
        let cc = source(src);
        let proto = get(&cc, "db.cc::DestroyDB");
        assert_eq!(
            (proto.kind, proto.raw.as_str()),
            (SymbolKind::Callable, "prototype")
        );
        assert_eq!(proto.signature, "Status DestroyDB(const std::string& name)");
        assert_eq!(proto.markers, ["declaration"]);
        assert!(proto.is_exported);
        assert_eq!(get(&cc, "db.cc::Visible").raw, "function");
        assert!(get(&cc, "db.cc::Visible").is_exported);
        assert!(
            !get(&cc, "db.cc::Helper").is_exported,
            "static in a source file"
        );
        assert!(
            !get(&cc, "db.cc::Hidden").is_exported,
            "anonymous namespace"
        );
        assert!(get(&cc, "db.cc::Hidden").interface_hash.is_none());

        // In a header every top-level name reaches the files that include it.
        let h = header(src);
        assert!(get(&h, "db.h::Helper").is_exported);
        assert!(get(&h, "db.h::Hidden").is_exported);
    }

    #[test]
    fn a_method_defined_outside_its_class_keeps_the_qualified_name() {
        let src = "\
namespace leveldb {
Status DB::Open(const Options& options, DB** dbptr) { return Status(); }
DBImpl::~DBImpl() {}
DBImpl::DBImpl(const Options& o) : options_(o), count_(0) {}
}
";
        let syms = source(src);
        let open = get(&syms, "db.cc::DB::Open");
        assert_eq!(open.raw, "qualified_function");
        assert_eq!(open.parent, None);
        assert_eq!(
            open.signature,
            "Status DB::Open(const Options& options, DB** dbptr)"
        );
        assert_eq!(
            get(&syms, "db.cc::DBImpl::~DBImpl").raw,
            "qualified_function"
        );
        // The initialiser list is body, not signature.
        assert_eq!(
            get(&syms, "db.cc::DBImpl::DBImpl").signature,
            "DBImpl::DBImpl(const Options& o)"
        );
    }

    #[test]
    fn namespaces_are_walked_and_do_not_appear_in_ids() {
        let src = "namespace a { namespace b { int f(); } }\nnamespace c::d { struct S {}; }\n";
        assert_eq!(ids(&header(src)), ["db.h::f", "db.h::S"]);
    }

    #[test]
    fn templates_keep_their_header_in_the_signature() {
        let src = "\
template <typename T>
class Box {
 public:
  T value;
};
template <typename T> T Max(T a, T b) { return a < b ? b : a; }
template <> class Box<bool> {};
";
        let syms = header(src);
        let b = get(&syms, "db.h::Box");
        assert_eq!(b.signature, "template <typename T>\nclass Box");
        assert_eq!(b.lines, [1, 5]);
        assert_eq!(
            get(&syms, "db.h::Max").signature,
            "template <typename T> T Max(T a, T b)"
        );
        // A specialisation shares the primary template's id, so it folds
        // into the primary: one symbol, the specialisation's range kept.
        assert_eq!(syms.iter().filter(|s| s.id == "db.h::Box").count(), 1);
        assert_eq!(b.extra_spans, [[7, 7]]);
    }

    #[test]
    fn enums_hold_their_enumerators() {
        let src = "enum Color { kRed, kGreen = 5 };\nenum class Mode : unsigned char { Read, Write };\nenum { kLoose = 1 };\n";
        let syms = header(src);
        let color = get(&syms, "db.h::Color");
        assert_eq!(
            (color.kind, color.raw.as_str()),
            (SymbolKind::Container, "enum")
        );
        assert_eq!(color.signature, "enum Color");
        assert_eq!(color.children, ["db.h::Color::kRed", "db.h::Color::kGreen"]);
        let green = get(&syms, "db.h::Color::kGreen");
        assert_eq!(
            (green.kind, green.raw.as_str()),
            (SymbolKind::Value, "enumerator")
        );
        assert_eq!(green.signature, "kGreen = 5");
        assert_eq!(
            get(&syms, "db.h::Mode").signature,
            "enum class Mode : unsigned char"
        );
        // An unnamed enum's entries belong to the scope around it.
        assert_eq!(get(&syms, "db.h::kLoose").parent, None);

        // Adding an enumerator changes what users of the enum can name.
        let more = src.replace("kGreen = 5", "kGreen = 5, kBlue");
        assert_ne!(
            color.interface_hash,
            get(&header(&more), "db.h::Color").interface_hash
        );
    }

    #[test]
    fn values_typedefs_aliases_variables_and_concepts() {
        let src = "\
typedef uint64_t SequenceNumber;
typedef int (*Callback)(void*, const char*);
using Ptr = std::unique_ptr<int>;
static const int kNumLevels = 7;
extern int g_count, *g_ptr;
int (*handler)(int);
template <typename T> concept Number = std::integral<T>;
";
        let syms = header(src);
        let raw = |id: &str| {
            let s = get(&syms, id);
            assert_eq!(s.kind, SymbolKind::Value, "{id}");
            s.raw.clone()
        };
        assert_eq!(raw("db.h::SequenceNumber"), "typedef");
        assert_eq!(raw("db.h::Callback"), "typedef");
        assert_eq!(raw("db.h::Ptr"), "using");
        assert_eq!(raw("db.h::kNumLevels"), "variable");
        assert_eq!(raw("db.h::g_count"), "variable");
        assert_eq!(raw("db.h::g_ptr"), "variable");
        assert_eq!(raw("db.h::handler"), "variable");
        assert_eq!(raw("db.h::Number"), "concept");
        // The value is not part of the signature.
        assert_eq!(
            get(&syms, "db.h::kNumLevels").signature,
            "static const int kNumLevels"
        );
    }

    #[test]
    fn macros_are_symbols_and_an_include_guard_is_not() {
        let src = "\
#ifndef STORAGE_DB_H_
#define STORAGE_DB_H_
#define kMaxSize 10
#define SQUARE(x) ((x) * (x))
#endif
";
        let syms = header(src);
        assert_eq!(ids(&syms), ["db.h::kMaxSize", "db.h::SQUARE"]);
        let max = get(&syms, "db.h::kMaxSize");
        assert_eq!((max.kind, max.raw.as_str()), (SymbolKind::Value, "macro"));
        assert_eq!(max.signature, "#define kMaxSize");
        let sq = get(&syms, "db.h::SQUARE");
        assert_eq!(sq.kind, SymbolKind::Callable);
        assert_eq!(sq.signature, "#define SQUARE(x)");
        assert!(sq.is_exported);
        // A macro defined in a source file cannot be used from another file.
        assert!(!get(&source(src), "db.cc::SQUARE").is_exported);
    }

    #[test]
    fn gtest_cases_are_named_by_suite_and_test() {
        let src = "TEST(DBTest, Empty) { ASSERT_TRUE(true); }\nTEST_F(DBTest, ReadWrite) {}\n";
        let syms = extract_file(src, "db_test.cc");
        let t = get(&syms, "db_test.cc::DBTest.Empty");
        assert_eq!((t.kind, t.raw.as_str()), (SymbolKind::Callable, "test"));
        assert_eq!(t.signature, "TEST(DBTest, Empty)");
        get(&syms, "db_test.cc::DBTest.ReadWrite");
    }

    #[test]
    fn every_branch_of_a_conditional_is_read() {
        let src = "\
#if defined(_WIN32)
int Open(const wchar_t* path);
#else
int Open(const char* path);
#endif
#ifdef HAVE_SNAPPY
class Snappy {};
#endif
";
        let syms = header(src);
        // Both branches are read; the two `Open`s are one symbol.
        let open = get(&syms, "db.h::Open");
        assert_eq!(count(&syms, "db.h::Open"), 1);
        assert_eq!(open.extra_spans, [[4, 4]]);
        get(&syms, "db.h::Snappy");
    }

    #[test]
    fn extern_c_blocks_and_single_declarations() {
        let src = "extern \"C\" {\nint c_api(int);\ntypedef struct leveldb_t leveldb_t;\n}\nextern \"C\" int one(void);\n";
        assert_eq!(
            ids(&header(src)),
            ["db.h::c_api", "db.h::leveldb_t", "db.h::one"]
        );
    }

    #[test]
    fn overloads_and_operators() {
        let src = "\
struct V {
  V operator+(const V& o) const;
  bool operator<(const V& o) const;
  int& operator[](int i);
  explicit operator bool() const;
  void set(int);
  void set(double);
};
bool operator==(const V& a, const V& b);
";
        let syms = header(src);
        for id in [
            "db.h::V::operator+",
            "db.h::V::operator<",
            "db.h::V::operator[]",
            "db.h::V::operator bool",
            "db.h::operator==",
        ] {
            assert_eq!(get(&syms, id).kind, SymbolKind::Callable, "{id}");
        }
        // The two `set` overloads are one symbol.
        assert_eq!(syms.iter().filter(|s| s.id == "db.h::V::set").count(), 1);
    }

    #[test]
    fn a_nested_class_is_a_child_container_with_its_own_members() {
        let src = "\
class Outer {
 public:
  struct Rep { int refs; };
 private:
  class Hidden {};
};
";
        let syms = header(src);
        let outer = get(&syms, "db.h::Outer");
        assert_eq!(outer.children, ["db.h::Outer::Rep", "db.h::Outer::Hidden"]);
        let rep = get(&syms, "db.h::Outer::Rep");
        assert_eq!(rep.kind, SymbolKind::Container);
        assert_eq!(rep.children, ["db.h::Outer::Rep::refs"]);
        assert!(rep.is_exported);
        assert!(!get(&syms, "db.h::Outer::Hidden").is_exported);
    }

    #[test]
    fn c_style_struct_typedefs_and_declared_instances() {
        let src =
            "typedef struct { int x; int y; } Point;\nstruct Config { int level; } g_config;\n";
        let syms = header(src);
        let point = get(&syms, "db.h::Point");
        assert_eq!(point.kind, SymbolKind::Container);
        assert_eq!(point.children, ["db.h::Point::x", "db.h::Point::y"]);
        assert_eq!(syms.iter().filter(|s| s.id == "db.h::Point").count(), 1);
        assert_eq!(get(&syms, "db.h::Config").kind, SymbolKind::Container);
        assert_eq!(get(&syms, "db.h::g_config").kind, SymbolKind::Value);
    }

    #[test]
    fn a_trailing_comment_is_not_the_next_declarations_doc() {
        let src = "int a;  // about a\nint b;\n\n/* Block.\n * Two lines. */\nint c;\n\n// Detached.\n\nint d;\n";
        let syms = header(src);
        assert_eq!(get(&syms, "db.h::b").docstring, None);
        assert_eq!(
            get(&syms, "db.h::c").docstring.as_deref(),
            Some("Block.\nTwo lines.")
        );
        assert_eq!(get(&syms, "db.h::d").docstring, None);
    }

    #[test]
    fn review_regressions() {
        // Keyword operators keep their space.
        let ops =
            header("struct S { void* operator new(size_t n); void operator delete[](void*); };\n");
        assert_eq!(
            ids(&ops),
            [
                "db.h::S",
                "db.h::S::operator new",
                "db.h::S::operator delete[]"
            ]
        );

        // `typedef struct Foo { … } Foo;` is one symbol, not two.
        let c = header("typedef struct Foo { int a; } Foo;\n");
        assert_eq!(c.iter().filter(|s| s.id == "db.h::Foo").count(), 1);
        assert_eq!(get(&c, "db.h::Foo").kind, SymbolKind::Container);
        // A typedef of a different name still gets its own symbol.
        let d = header("typedef struct Foo { int a; } Bar;\n");
        get(&d, "db.h::Foo");
        assert_eq!(get(&d, "db.h::Bar").raw, "typedef");

        // Each declarator stops after its own name.
        let m = header("struct S { int a, b = 5; };\n");
        assert_eq!(get(&m, "db.h::S::a").signature, "int a");
        assert!(get(&m, "db.h::S::b").signature.ends_with("b"));

        // Linkage in a source file: constants and aliases are not reachable.
        let cc = source(
            "const int kMax = 5;\nusing U = int;\nextern const int kPub;\nint g_count = 0;\n",
        );
        assert!(!get(&cc, "db.cc::kMax").is_exported);
        assert!(!get(&cc, "db.cc::U").is_exported);
        assert!(get(&cc, "db.cc::kPub").is_exported);
        assert!(get(&cc, "db.cc::g_count").is_exported);

        // Template implementation files are headers; the extension's case is ignored.
        assert!(is_header("a/vec.tpp") && is_header("a/vec.TCC") && is_header("a/X.H"));
        assert!(!is_header("a/x.cc"));
        assert!(is_cpp_file("a/x.cc") && is_cpp_file("a/X.H") && is_cpp_file("m.ixx"));
        assert!(!is_cpp_file("a/x.c") && !is_cpp_file("Makefile") && !is_cpp_file("a.b/README"));

        // A bare block is read only when it is an `export { … }` block. The
        // body of a test macro the grammar could not attach to a function
        // must not leak its locals as declarations.
        let leak = source(
            "TEST_CASE(\"vectors\", \"[vector]\") {\n  std::vector<int> v(5);\n  int local = 1;\n}\nint real;\n",
        );
        assert_eq!(ids(&leak), ["db.cc::real"]);

        // Doc text that starts with a marker character keeps it.
        let doc = header("// <T> must be copyable\n// *Note*: thread safe\nint a;\n");
        assert_eq!(
            get(&doc, "db.h::a").docstring.as_deref(),
            Some("<T> must be copyable\n*Note*: thread safe")
        );
        let doc = header("/**\n * Block *text*.\n */\nint b;\nint c;  ///< trailing\n");
        assert_eq!(
            get(&doc, "db.h::b").docstring.as_deref(),
            Some("Block *text*.")
        );
    }

    #[test]
    fn conversion_operators_defined_outside_their_class() {
        // Three shapes the grammar gives the same thing.
        let syms = source(
            "V::operator bool() const { return true; }\n\
             inline ns::V::operator const char*() const { return p; }\n\
             constexpr uint128::operator int() const { return lo_; }\n",
        );
        assert_eq!(
            ids(&syms),
            [
                "db.cc::V::operator bool",
                "db.cc::ns::V::operator const char*",
                "db.cc::uint128::operator int",
            ]
        );
        for s in &syms {
            assert_eq!(
                (s.kind, s.raw.as_str()),
                (SymbolKind::Callable, "qualified_function")
            );
        }
        assert_eq!(syms[0].signature, "V::operator bool() const");
        assert_eq!(syms[2].signature, "constexpr uint128::operator int() const");

        // Inside the class the whole target type is the name too.
        let inside =
            header("struct V { operator const char*() const; explicit operator bool() const; };\n");
        assert_eq!(
            ids(&inside),
            [
                "db.h::V",
                "db.h::V::operator const char*",
                "db.h::V::operator bool"
            ]
        );
    }

    #[test]
    fn a_function_name_in_parentheses() {
        // `(min)(…)` is written that way so a `min` macro cannot expand it.
        let syms = header(
            "int (min)(int a, int b) { return a; }\n\
             struct A {\n  static constexpr A (max)() { return A(); }\n  int (clamp)(int v);\n};\n\
             constexpr uint128 (std::numeric_limits<uint128>::max)() { return 1; }\n\
             int (*handler)(int);\n",
        );
        let kind = |id: &str| (get(&syms, id).kind, get(&syms, id).raw.clone());
        assert_eq!(
            kind("db.h::min"),
            (SymbolKind::Callable, "function".to_string())
        );
        assert_eq!(
            kind("db.h::A::max"),
            (SymbolKind::Callable, "method".to_string())
        );
        assert_eq!(
            kind("db.h::A::clamp"),
            (SymbolKind::Callable, "method".to_string())
        );
        assert_eq!(
            kind("db.h::std::numeric_limits::max"),
            (SymbolKind::Callable, "qualified_function".to_string())
        );
        // A pointer in the parentheses still means a variable.
        assert_eq!(
            kind("db.h::handler"),
            (SymbolKind::Value, "variable".to_string())
        );
    }

    #[test]
    fn counting_parse_errors_survives_a_very_deep_tree() {
        // 200,000 terms nest 200,000 levels deep in the syntax tree. A
        // recursive walk of it overflows the stack (found in review).
        let terms = vec!["1"; 200_000].join(" + ");
        let src = format!("int x = {terms};\nint after;\n");
        // Run on a thread with a small stack, as the watcher's are.
        let handle = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                let (syms, errors) = extract_counted(&src, "big.h");
                (syms.iter().any(|s| s.id == "big.h::after"), errors)
            })
            .unwrap();
        let (found, errors) = handle.join().expect("no stack overflow");
        assert!(found);
        assert_eq!(errors, 0);
    }

    // ---- one symbol per id (step 2, part 0) ----

    fn count(syms: &[ExtractedSymbol], id: &str) -> usize {
        syms.iter().filter(|s| s.id == id).count()
    }

    #[test]
    fn overloads_fold_into_one_symbol_keeping_every_range() {
        let syms = header("void set(int);\nvoid set(double);\nvoid set(char*);\n");
        assert_eq!(count(&syms, "db.h::set"), 1);
        let set = get(&syms, "db.h::set");
        assert_eq!(set.lines, [1, 1]);
        assert_eq!(set.extra_spans, [[2, 2], [3, 3]]);
    }

    #[test]
    fn an_edit_to_any_overload_moves_both_hashes() {
        let base = "void set(int);\nvoid set(double);\n";
        let edited = "void set(int);\nvoid set(double, int);\n";
        let a = header(base);
        let b = header(edited);
        let (a, b) = (get(&a, "db.h::set"), get(&b, "db.h::set"));
        assert_ne!(a.interface_hash, b.interface_hash);
        assert_ne!(a.source_hash, b.source_hash);
    }

    #[test]
    fn a_prototype_and_its_definition_in_one_file_fold() {
        let syms = source("int f(int);\nint f(int x) { return x; }\n");
        assert_eq!(count(&syms, "db.cc::f"), 1);
        assert_eq!(get(&syms, "db.cc::f").extra_spans.len(), 1);
        // A definition is among them, so the folded symbol is not "only
        // declared"; a prototype folded with a prototype still is.
        assert!(
            !get(&syms, "db.cc::f")
                .markers
                .iter()
                .any(|m| m == "declaration")
        );
        let protos = source("int g(int);\nint g(long);\n");
        assert!(
            get(&protos, "db.cc::g")
                .markers
                .iter()
                .any(|m| m == "declaration")
        );
    }

    #[test]
    fn both_branches_of_an_if_fold() {
        let src = "#if A\nint v() { return 1; }\n#else\nint v() { return 2; }\n#endif\n";
        let syms = header(src);
        assert_eq!(count(&syms, "db.h::v"), 1);
        assert_eq!(get(&syms, "db.h::v").extra_spans.len(), 1);
    }

    #[test]
    fn a_stronger_kind_survives_whichever_comes_first() {
        let syms = header("struct stat_t;\nint stat_t(int);\nstruct stat_t { int a; };\n");
        let s = get(&syms, "db.h::stat_t");
        assert_eq!(s.kind, SymbolKind::Container);
        assert_eq!(count(&syms, "db.h::stat_t"), 1);
        // The member survives with its parent.
        assert_eq!(s.children, ["db.h::stat_t::a"]);
    }

    #[test]
    fn members_declared_in_both_branches_are_one_child() {
        let src = "class C {\n#if A\n  int n() const;\n#else\n  long n() const;\n#endif\n};\n";
        let syms = header(src);
        assert_eq!(count(&syms, "db.h::C::n"), 1);
        assert_eq!(get(&syms, "db.h::C").children, ["db.h::C::n"]);
    }

    #[test]
    fn the_unfolded_view_keeps_each_declaration_with_its_own_kind() {
        let src = "struct stat_t { int a; };\nint stat_t(int);\nint stat_t(int x) { return x; }\n";
        let (syms, _) = extract_unfolded_counted(src, "db.h");
        let of_id: Vec<&ExtractedSymbol> = syms.iter().filter(|s| s.id == "db.h::stat_t").collect();
        assert_eq!(of_id.len(), 3);
        assert_eq!(of_id[0].kind, SymbolKind::Container);
        assert!(of_id[1].markers.iter().any(|m| m == "declaration"));
        assert!(!of_id[2].markers.iter().any(|m| m == "declaration"));
        // The folded view of the same text is one symbol.
        assert_eq!(count(&extract_file(src, "db.h"), "db.h::stat_t"), 1);
    }

    #[test]
    fn the_public_overload_stands_for_a_private_one_written_before_it() {
        let src = "class C {\n private:\n  void f(double);\n public:\n  void f(int);\n};\n";
        let syms = header(src);
        let f = get(&syms, "db.h::C::f");
        // The symbol is described by the overload a user can call.
        assert_eq!(f.signature, "void f(int)");
        assert_eq!(f.lines, [5, 5]);
        // The class's interface (what a dependant reads) is unaffected by
        // an edit to the private overload.
        let edited = header(&src.replace("f(double)", "f(double, int)"));
        assert_eq!(
            get(&edited, "db.h::C").interface_hash,
            get(&syms, "db.h::C").interface_hash
        );
    }

    #[test]
    fn the_definition_stands_for_a_prototype_written_before_it() {
        let syms = source("int f(int);\nint f(int x) { return x; }\nint f(long);\n");
        let f = get(&syms, "db.cc::f");
        assert_eq!(f.lines, [2, 2]);
        assert_eq!(f.extra_spans, [[1, 1], [3, 3]]);
        // Three folds: the marker stays off once a body is among them.
        assert!(!f.markers.iter().any(|m| m == "declaration"));
        assert_ne!(f.raw, "prototype");
    }

    #[test]
    fn a_doc_comment_on_a_later_declaration_is_kept() {
        let syms = header("void g(int);\n// Sets the thing.\nvoid g(double);\n");
        let g = get(&syms, "db.h::g");
        assert!(
            g.docstring
                .as_deref()
                .is_some_and(|d| d.contains("Sets the thing"))
        );
    }

    #[test]
    fn no_two_symbols_of_a_file_share_an_id() {
        let syms = header(DB);
        let mut seen = std::collections::HashSet::new();
        assert!(syms.iter().all(|s| seen.insert(s.id.clone())));
    }

    #[test]
    fn extraction_and_the_error_count_come_from_one_parse() {
        let (syms, errors) = extract_counted("class A {};\nclass { int ; ] ) = ;\n", "a.h");
        assert!(syms.iter().any(|s| s.id == "a.h::A"));
        assert_eq!(
            errors,
            parse_error_count("class A {};\nclass { int ; ] ) = ;\n")
        );
        assert!(errors > 0);
    }

    #[test]
    fn parse_errors_are_counted_after_blanking() {
        assert_eq!(parse_error_count(DB), 0);
        assert!(parse_error_count("class { int ; ] ) = ;\n") > 0);
    }
}
