//! Property-based tests for parser behavior.

use act_lite_parser::{Grammar, Parser, ParserError, SourceFile};
use proptest::prelude::*;
use std::time::Duration;

proptest! {
    #[test]
    fn parse_always_terminates(source in "\\PC{0,1000}") {
        // Arbitrary non-control Unicode is almost entirely unparseable as TS, which
        // drives tree-sitter's super-linear error recovery — some inputs take
        // many seconds. The parser must therefore be *bounded*: with a short
        // deadline the call always returns (never hangs CI), yielding either an
        // error-tolerant tree or a clean timeout — and never any other failure.
        let mut parser = Parser::new();
        parser.set_timeout(Some(Duration::from_secs(2)));
        let result = parser.parse(&source, Grammar::TypeScript);
        prop_assert!(
            result.is_ok() || matches!(&result, Err(ParserError::ParseTimeout { .. })),
            "expected a tree or a timeout, got error: {:?}",
            result.err()
        );
    }

    #[test]
    fn incremental_reparse_consistency(
        initial in "[a-z]{10,50}",
        updated in "[a-z]{10,50}"
    ) {
        // Create initial source
        let initial_content = format!("const {} = 1;", initial);
        let mut source = SourceFile::from_content(
            "test.ts",
            initial_content,
            Grammar::TypeScript,
        ).unwrap();

        // Update content
        let updated_content = format!("const {} = 2;", updated);
        source.update_content(updated_content.clone()).unwrap();

        // Source should match updated content
        prop_assert_eq!(source.content(), updated_content);
    }
}

#[test]
fn parse_is_bounded_by_deadline() {
    // Deterministic regression guard for the unbounded-parse hang: a large input
    // with a near-zero deadline must be cancelled and surface ParseTimeout, not
    // run to completion (and never hang). The input is large enough that the
    // progress callback fires before parsing finishes.
    let mut parser = Parser::new();
    parser.set_timeout(Some(Duration::from_nanos(1)));
    let big = "const x = 1;\n".repeat(50_000);
    let result = parser.parse(&big, Grammar::TypeScript);
    assert!(
        matches!(&result, Err(ParserError::ParseTimeout { .. })),
        "near-zero deadline must yield ParseTimeout, got error: {:?}",
        result.err()
    );
}

#[test]
fn parse_unbounded_when_timeout_disabled() {
    // With the cap disabled, normal input still parses fine (no false timeout).
    let mut parser = Parser::new();
    parser.set_timeout(None);
    let tree = parser
        .parse("const x = 1;", Grammar::TypeScript)
        .expect("normal parse with no deadline must succeed");
    assert_eq!(tree.root_node().kind(), "program");
}

#[test]
fn parse_empty_source() {
    let mut parser = Parser::new();
    let tree = parser.parse("", Grammar::TypeScript).unwrap();
    assert_eq!(tree.root_node().kind(), "program");
}

#[test]
fn parse_unicode() {
    let mut parser = Parser::new();
    let source = "const 你好 = '世界';";
    let tree = parser.parse(source, Grammar::TypeScript).unwrap();
    assert_eq!(tree.root_node().kind(), "program");
}
