//! Grammar × query compile audit — F4.
//!
//! For every Grammar variant, compiles each of its three query sources
//! (skeleton, symbols, references) against the grammar's tree-sitter language.
//! Collects ALL failures; the assertion lists them so failures are immediately
//! actionable.
//!
//! A failing entry means the query references node types or fields that don't
//! exist in the grammar — it will silently produce zero results at runtime.

use act_lite_parser::Grammar;

/// Compile all three query sources for every grammar against their ts_language.
#[test]
fn all_grammar_queries_compile() {
    let mut failures: Vec<String> = Vec::new();

    for grammar in Grammar::all() {
        let support = grammar.language_support();
        let lang = support.ts_language();

        let queries: &[(&str, &str)] = &[
            ("skeleton", support.skeleton_query()),
            ("symbols", support.symbols_query()),
            ("references", support.references_query()),
        ];

        for (name, source) in queries {
            if let Err(e) = tree_sitter::Query::new(&lang, source) {
                failures.push(format!("{grammar:?}/{name}: {e}"));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "Grammar query compile failures ({} total):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
