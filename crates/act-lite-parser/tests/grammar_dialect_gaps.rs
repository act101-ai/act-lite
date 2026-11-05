mod css {
    use crate::harness;

    #[test]
    fn anb_forms_parse_in_nth_pseudo_classes() {
        // The finding's shape: minus-B in every nth pseudo-class.
        harness::assert_no_parse_errors("css", "a:nth-child(2n-1) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-last-child(2n-1) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-of-type(2n-1) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-last-of-type(2n-1) { color: red }\n");
        // Whitespace on either side of the joining sign (spec-valid),
        // including the n-dash forms.
        harness::assert_no_parse_errors("css", "a:nth-child(3n - 2) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(3n- 2) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(n - 3) { color: red }\n");
        // Plus-signed A and the spec's own examples.
        harness::assert_no_parse_errors("css", "a:nth-child(+2n+1) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(-4n+10) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(-1n+6) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-of-type(4n+1) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-last-of-type(3n - 2) { color: red }\n");
        // Structure pin (the of-type mis-parse): the An+B argument is ONE
        // plain_value, not two integer values — the pristine grammar read
        // `2n-1` as integer `2n` (with unit) plus integer `-1`.
        let src = "a:nth-of-type(2n-1) { color: red }\n";
        assert_eq!(
            harness::count_kind("css", src, "integer_value"),
            0,
            "an An+B argument is one value, not integer + integer"
        );
        assert!(harness::tree_contains_kind("css", src, "plain_value"));
        harness::assert_no_parse_errors("css", "a:nth-child(2N+1) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-of-type(2N+1) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(ODD) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-last-child(Even) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-of-type(2n/**/+1) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(-2N - 4) { color: red }\n");
        // The spec's invalid stacked-sign example (`3n + -6`) stays
        // refused — the joining sign never takes a signed operand.
        assert!(
            !harness::assert_no_parse_errors_quiet("css", "a:nth-child(3n + -6) { color: red }\n"),
            "a stacked sign after the joining sign is not An+B"
        );
        // Controls (already clean — must stay clean): the forms the old
        // regex accepted, the keywords, the of-filter (nth-child only),
        // and a plain integer.
        harness::assert_no_parse_errors("css", "a:nth-child(2n+1) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(2n) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(-n+3) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(n) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(even) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(odd) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(5) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-child(2n of .foo) { color: red }\n");
        harness::assert_no_parse_errors("css", "a:nth-last-child(2n of .foo) { color: red }\n");
        // The spec's INVALID whitespace shapes stay refused (§6.1:
        // "+ 2n", "3 n", "+ 2") — whitespace never splits the An part.
        assert!(
            !harness::assert_no_parse_errors_quiet("css", "a:nth-child(3 n) { color: red }\n"),
            "whitespace between the digits and n is not An+B"
        );
        assert!(
            !harness::assert_no_parse_errors_quiet("css", "a:nth-child(+ 2n) { color: red }\n"),
            "a space after the A sign is not An+B"
        );
        assert!(
            !harness::assert_no_parse_errors_quiet("css", "a:nth-child(+ 2) { color: red }\n"),
            "a space after a bare + is not An+B"
        );
        // `of S` is nth-child/nth-last-child only (selectors-4) —
        // :nth-of-type takes a bare An+B and must refuse the filter.
        assert!(
            !harness::assert_no_parse_errors_quiet(
                "css",
                "a:nth-of-type(2n of .foo) { color: red }\n"
            ),
            "only :nth-child/:nth-last-child take an `of S` filter"
        );
    }
}

mod harness {
    use act_lite_parser::{Grammar, parser::Parser};

    fn parse(ext: &str, source: &str) -> act_lite_parser::syntax_tree::SyntaxTree {
        let grammar = Grammar::from_extension(ext)
            .unwrap_or_else(|| panic!("no grammar routes extension {ext:?}"));
        Parser::new()
            .parse(source, grammar)
            .unwrap_or_else(|e| panic!("parse failed at the driver level: {e}"))
    }

    /// Parses `source` with the grammar routed for `ext` and asserts the
    /// tree carries no ERROR or MISSING nodes.
    pub fn assert_no_parse_errors(ext: &str, source: &str) {
        let tree = parse(ext, source);
        assert!(
            !tree.root_node().has_error(),
            "parse errors in {ext:?} source {source:?}"
        );
    }

    /// Like `assert_no_parse_errors` but returns the verdict instead of
    /// asserting — for pins that assert a shape does NOT parse.
    pub fn assert_no_parse_errors_quiet(ext: &str, source: &str) -> bool {
        let tree = parse(ext, source);
        !tree.root_node().has_error()
    }

    /// Whether the parse tree of `source` contains a node of `kind`.
    /// Counts nodes of `kind` in the parse tree — for structure pins
    /// (an element that error recovery silently swallowed is a count of
    /// zero even when the file parses clean).
    pub fn count_kind(ext: &str, source: &str, kind: &str) -> usize {
        let tree = parse(ext, source);
        assert!(
            !tree.root_node().has_error(),
            "count_kind is a structure pin, not a recovery probe — parse errors \
             in {ext:?} source {source:?} mean the count could include nodes \
             produced by error recovery"
        );
        let mut cursor = tree.root_node().walk();
        let mut count = 0;
        loop {
            if cursor.node().kind() == kind {
                count += 1;
            }
            if cursor.goto_first_child() {
                continue;
            }
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

    pub fn tree_contains_kind(ext: &str, source: &str, kind: &str) -> bool {
        let tree = parse(ext, source);
        let mut cursor = tree.root_node().walk();
        loop {
            if cursor.node().kind() == kind {
                return true;
            }
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return false;
                }
            }
        }
    }
}
