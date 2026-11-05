use act_lite_parser::{Grammar, parser::Parser};

fn assert_clean(grammar: Grammar, source: &str) {
    let tree = Parser::new().parse(source, grammar).unwrap();
    assert!(
        !tree.root_node().has_error(),
        "{grammar:?} produced ERROR/missing nodes: {}",
        tree.root_node().to_sexp()
    );
}

#[test]
fn css_non_ascii_identifier_is_a_clean_calibration_input() {
    assert_clean(Grammar::Css, ".フレックス { display: block; }\n");
}

// Negative guard for the non-ASCII ident widening: an ASCII digit is still not
// a valid ident-start (CSS Syntax 3 §4.3.1), so these stay invalid.
#[test]
fn css_ascii_digit_leading_identifier_stays_invalid() {
    for source in [".1a { display: block; }\n", ".-1a { display: block; }\n"] {
        let tree = Parser::new().parse(source, Grammar::Css).unwrap();
        assert!(
            tree.root_node().has_error(),
            "ASCII-digit-leading class unexpectedly parsed cleanly: {source}\n{}",
            tree.root_node().to_sexp()
        );
    }
}

#[test]
fn scss_hyphenated_function_no_args_is_a_clean_calibration_input() {
    assert_clean(
        Grammar::Scss,
        "@function golden-ratio() {\n  @return 1.618;\n}\n",
    );
}

// Negative guard for the optional empty parameter list: a dangling comma is
// not a valid parameter list and still errors, and a non-empty parameter
// list keeps parsing clean.
#[test]
fn scss_function_dangling_comma_stays_invalid() {
    assert_clean(Grammar::Scss, "@function f($x) {\n  @return $x;\n}\n");
    let source = "@function f(,) {\n  @return 1;\n}\n";
    let tree = Parser::new().parse(source, Grammar::Scss).unwrap();
    assert!(
        tree.root_node().has_error(),
        "dangling-comma SCSS parameter list unexpectedly parsed cleanly: {source}\n{}",
        tree.root_node().to_sexp()
    );
}

#[test]
fn sql_having_without_group_by_is_a_clean_calibration_input() {
    assert_clean(
        Grammar::Sql,
        "SELECT status, COUNT(*) FROM users HAVING COUNT(*) > 10;\n",
    );
}

#[test]
fn sql_sqlite_postfix_null_test_is_a_clean_calibration_input() {
    for source in [
        "SELECT * FROM users WHERE name ISNULL;\n",
        "SELECT * FROM users WHERE email NOTNULL;\n",
    ] {
        assert_clean(Grammar::Sql, source);
    }
}

// Negative guard for the sibling HAVING clause and the SQLite postfix null
// tests: a HAVING clause with no SELECT above it still errors, and the
// widened forms are precise — the two-word postfix `x NOT NULL` parses
// clean, and `SELECT isnull FROM t` keeps its identifier reading.
#[test]
fn sql_having_without_select_stays_invalid() {
    assert_clean(Grammar::Sql, "SELECT * FROM t WHERE x NOT NULL;\n");
    assert_clean(Grammar::Sql, "SELECT isnull FROM t;\n");
    let source = "HAVING COUNT(*) > 10;\n";
    let tree = Parser::new().parse(source, Grammar::Sql).unwrap();
    assert!(
        tree.root_node().has_error(),
        "bare HAVING clause unexpectedly parsed cleanly: {source}\n{}",
        tree.root_node().to_sexp()
    );
}
