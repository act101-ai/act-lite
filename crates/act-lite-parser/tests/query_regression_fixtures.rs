//! Per-grammar regression fixtures — F4 batch 1 + batch 2.
//!
//! Each test verifies that the fixed queries for its grammar actually return
//! at least one expected symbol, proving the queries compile AND match real AST
//! nodes. Tests are intentionally minimal: one representative fixture per
//! grammar, just enough to fail if the node-name fix is reverted.

use act_lite_parser::{Grammar, QueryResult, SourceFile, execute_symbols};

// ── CSS ───────────────────────────────────────────────────────────────────────

/// Regression: CSS queries used `type_selector` (absent) instead of the real
/// `tag_name` node inside `selectors`. After fix, `div` appears as a symbol.
#[test]
fn css_symbols_find_tag_name() {
    let source = SourceFile::from_content(
        "test.css",
        "div { color: red; }\n".to_string(),
        Grammar::Css,
    )
    .unwrap();

    let syms = execute_symbols(&source).expect("CSS symbols must not error");
    match syms {
        QueryResult::Symbols { symbols } => {
            let names: Vec<_> = symbols.iter().map(|s| s.name()).collect();
            assert!(
                names.contains(&"div"),
                "CSS symbols: expected tag 'div', got: {names:?}"
            );
        }
        other => panic!("Expected Symbols for CSS, got: {other:?}"),
    }
}

// ── HTML ──────────────────────────────────────────────────────────────────────

/// Regression: HTML symbols/references used `name:` field on `start_tag` and
/// `attribute` (neither has a `name` field; `tag_name` and `attribute_name`
/// are direct children). After fix, the `html` tag is found as a symbol.
#[test]
fn html_symbols_find_tag() {
    let source = SourceFile::from_content(
        "test.html",
        "<html><body><p>Hello</p></body></html>\n".to_string(),
        Grammar::Html,
    )
    .unwrap();

    let syms = execute_symbols(&source).expect("HTML symbols must not error");
    match syms {
        QueryResult::Symbols { symbols } => {
            let names: Vec<_> = symbols.iter().map(|s| s.name()).collect();
            assert!(
                names.contains(&"html"),
                "HTML symbols: expected tag 'html', got: {names:?}"
            );
        }
        other => panic!("Expected Symbols for HTML, got: {other:?}"),
    }
}
