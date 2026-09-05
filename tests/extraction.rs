//! Integration tests against realistic fixtures — the "hardest real cases"
//! `ROADMAP.md`'s M1 validation calls out: a hooked-up React component, a
//! generic function, and a barrel file.

use codeowl::{SymbolKind, extract_file};

#[test]
fn react_component_with_hooks_and_generics() {
    let source = include_str!("fixtures/component.tsx");
    let symbols = extract_file(source, "component.tsx");

    // Exactly the three top-level declarations — nothing nested inside
    // UserBadge's body (the useEffect callback, its inner arrow functions)
    // leaks out as a symbol of its own.
    let names: Vec<&str> = symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "component.tsx::UserBadge",
            "component.tsx::findFirst",
            "component.tsx::fetchUser",
        ]
    );
    assert!(symbols.iter().all(|s| s.kind == SymbolKind::Function));

    let badge = &symbols[0];
    assert_eq!(
        badge.docstring.as_deref(),
        Some("Fetches and displays a user's display name.")
    );
    assert!(badge.signature.contains("UserBadge"));

    let find_first = &symbols[1];
    assert!(
        find_first.signature.contains("<T>"),
        "expected generic type parameter in signature, got: {}",
        find_first.signature
    );
}

#[test]
fn barrel_file_has_no_declarations() {
    let source = include_str!("fixtures/barrel.ts");
    let symbols = extract_file(source, "barrel.ts");
    assert!(symbols.is_empty());
}
