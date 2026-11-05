use act_lite_parser::{Grammar, QueryResult, SourceFile, execute_skeleton};

const FIXTURES: &[(&str, Grammar, &str, &str)] = &[
    (
        "python: single-line blank docstring",
        Grammar::Python,
        "\"\"\"   \"\"\"\nx = 1\n",
        "x",
    ),
    (
        "python: multi-line blank docstring",
        Grammar::Python,
        "\"\"\"\n\"\"\"\nx = 1\n",
        "x",
    ),
    (
        "json: blank object key",
        Grammar::Json,
        r#"{" ": {"z": 1}, "a": {"x": 1}}"#,
        "a",
    ),
    (
        "markdown: ATX heading squeezed to one space by its closing sequence",
        Grammar::Markdown,
        "# ###\n## Real Heading\n",
        "Real Heading",
    ),
];

#[test]
fn no_covered_grammar_emits_a_blank_name_from_a_real_parse() {
    for (label, grammar, src, expect_present) in FIXTURES {
        let file = SourceFile::from_content("probe.txt".to_string(), (*src).to_string(), *grammar)
            .unwrap_or_else(|e| panic!("{label}: fixture must parse: {e}"));
        let result = execute_skeleton(&file)
            .unwrap_or_else(|e| panic!("{label}: skeleton query must run: {e}"));
        let declarations = match result {
            QueryResult::Skeleton(r) => r.declarations,
            other => panic!("{label}: expected Skeleton, got {other:?}"),
        };
        let blank: Vec<_> = declarations.iter().filter(|d| d.name.is_empty()).collect();
        assert!(
            blank.is_empty(),
            "{label}: emitted a blank-named declaration: {blank:?} (all: {declarations:?})"
        );
        assert!(
            declarations.iter().any(|d| &d.name == expect_present),
            "{label}: expected the real sibling declaration {expect_present:?} to be present — \
             its absence means the query broke entirely rather than correctly skipping only the \
             blank: {declarations:?}"
        );
    }
}
