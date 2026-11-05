use act_lite_parser::{Grammar, SourceFile};

#[test]
fn markdown_scanner_survives_pathological_table_row() {
    let path = "tests/fixtures/markdown/pathological_table_row.md";

    // Before the fix: this call throws a C++ exception out of
    // `tree_sitter_markdown_external_scanner_deserialize`, which is
    // uncatchable from Rust and aborts the whole process (SIGABRT / exit
    // 134) — so a failing assertion below is never reached.
    let result = SourceFile::load(path);

    assert!(
        result.is_ok(),
        "expected a parse result (possibly degraded) instead of an error, got: {:?}",
        result.err()
    );
}

#[test]
fn markdown_scanner_survives_oversized_inline_state() {
    let content = "[".repeat(600);

    let result = SourceFile::from_content("oversized-inline-state.md", content, Grammar::Markdown);

    assert!(
        result.is_ok(),
        "expected a parse result (possibly degraded) instead of an error, got: {:?}",
        result.err()
    );
}
