//! Shared test utilities for language support tests.
//!
//! This module is only compiled under `#[cfg(test)]`.

use crate::language::LanguageSupport;

/// Parse source code and return the tree using a given language support.
pub fn parse_with_support<S: LanguageSupport>(support: &S, source: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&support.ts_language()).unwrap();
    parser.parse(source, None).unwrap()
}

/// Recursively find the first node with the given kind in the tree.
pub fn find_node_by_kind<'a>(
    node: tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_node_by_kind(child, kind) {
            return Some(found);
        }
    }
    None
}

/// Macro that generates the standard boilerplate tests every LanguageSupport
/// implementation must pass.
///
/// Usage:
/// ```ignore
/// language_support_tests!(ScalaSupport, Grammar::Scala, &["scala", "sc"]);
/// ```
#[macro_export]
macro_rules! language_support_tests {
    ($support_type:ty, $grammar:expr, $extensions:expr) => {
        #[test]
        fn grammar() {
            let support = <$support_type>::default();
            assert_eq!(
                $crate::language::LanguageSupport::grammar(&support),
                $grammar
            );
        }

        #[test]
        fn extensions() {
            let support = <$support_type>::default();
            assert_eq!(
                $crate::language::LanguageSupport::extensions(&support),
                $extensions
            );
        }

        #[test]
        fn ts_language_returns_valid_language() {
            let support = <$support_type>::default();
            let lang = $crate::language::LanguageSupport::ts_language(&support);
            let mut parser = tree_sitter::Parser::new();
            assert!(parser.set_language(&lang).is_ok());
        }

        #[test]
        fn comment_delimiters_are_set() {
            let support = <$support_type>::default();
            let delims = $crate::language::LanguageSupport::comment_delimiters(&support);
            assert!(
                delims.line.is_some() || delims.block_start.is_some(),
                "Language should define at least line or block comment delimiters"
            );
        }

        #[test]
        fn queries_embedded() {
            let support = <$support_type>::default();
            assert!(
                !$crate::language::LanguageSupport::skeleton_query(&support).is_empty(),
                "skeleton query should not be empty"
            );
            assert!(
                !$crate::language::LanguageSupport::symbols_query(&support).is_empty(),
                "symbols query should not be empty"
            );
            assert!(
                !$crate::language::LanguageSupport::references_query(&support).is_empty(),
                "references query should not be empty"
            );
        }

        #[test]
        fn skeleton_query_is_valid() {
            let support = <$support_type>::default();
            let lang = $crate::language::LanguageSupport::ts_language(&support);
            let result = tree_sitter::Query::new(
                &lang,
                $crate::language::LanguageSupport::skeleton_query(&support),
            );
            assert!(
                result.is_ok(),
                "skeleton.scm should parse: {:?}",
                result.err()
            );
        }

        #[test]
        fn symbols_query_is_valid() {
            let support = <$support_type>::default();
            let lang = $crate::language::LanguageSupport::ts_language(&support);
            let result = tree_sitter::Query::new(
                &lang,
                $crate::language::LanguageSupport::symbols_query(&support),
            );
            assert!(
                result.is_ok(),
                "symbols.scm should parse: {:?}",
                result.err()
            );
        }

        #[test]
        fn references_query_is_valid() {
            let support = <$support_type>::default();
            let lang = $crate::language::LanguageSupport::ts_language(&support);
            let result = tree_sitter::Query::new(
                &lang,
                $crate::language::LanguageSupport::references_query(&support),
            );
            assert!(
                result.is_ok(),
                "references.scm should parse: {:?}",
                result.err()
            );
        }
    };
}
