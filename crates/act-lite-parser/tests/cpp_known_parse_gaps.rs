use act_lite_parser::{Grammar, parser::Parser};

/// Parse `source` with the C++ grammar and return whether the root has errors.
fn cpp_has_error(source: &str) -> bool {
    let tree = Parser::new()
        .parse(source, Grammar::Cpp)
        .expect("C++ parse must not fail at the driver level");
    tree.root_node().has_error()
}

#[test]
fn cpp_known_parse_gaps_characterization() {
    // Gap: C++17 using-declarator-list — tree-sitter-cpp 0.23.4 folds the list
    // into one `qualified_identifier` holding an `ERROR` node over `"B,"`.
    assert!(
        cpp_has_error("using A::B, A::C;\n"),
        "known parse gap regressed: `using A::B, A::C;` must error at \
         tree-sitter-cpp 0.23.4; if this now parses cleanly, flip this \
         assertion"
    );
    // Control: single using-declarator parses cleanly.
    assert!(
        !cpp_has_error("using A::B;\n"),
        "control regressed: `using A::B;` must parse cleanly"
    );
}
