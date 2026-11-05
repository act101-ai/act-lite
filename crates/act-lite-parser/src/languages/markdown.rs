//! Markdown language support - markup format for documentation and content.
//!
//! Markdown is a lightweight markup language with no executable semantics.
//! Operations focus on structural transformations (headings, links, sections).
//! Since Markdown is data-oriented, tree-sitter analysis is minimal.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Markdown in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("markdown"),
        display_name: "Markdown",
        extensions: &["md", "markdown", "mdown", "mkd", "mkdn"],
        category: LanguageCategory::Data,
    }
}

fn markdown_language() -> tree_sitter::Language {
    tree_sitter_markdown::LANGUAGE.into()
}

/// Markdown language support implementation.
///
/// Markdown is a data format without executable semantics.
/// The parser module is minimal; most work happens at the refactor layer.
#[derive(Default)]
pub struct MarkdownSupport;

impl LanguageSupport for MarkdownSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Markdown
    }

    fn ts_language(&self) -> tree_sitter::Language {
        markdown_language()
    }

    fn node_kind(&self, _node: &tree_sitter::Node) -> NodeKind {
        NodeKind::Block
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["md", "markdown", "mdown", "mkd", "mkdn"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/markdown/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/markdown/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/markdown/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: None,
            block_start: Some("<!--"),
            block_end: Some("-->"),
        }
    }

    fn function_keyword(&self) -> &'static str {
        ""
    }

    fn function_body_kind(&self) -> &'static str {
        ""
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Query;

    #[test]
    fn grammar() {
        assert_eq!(MarkdownSupport.grammar(), Grammar::Markdown);
    }

    #[test]
    fn extensions() {
        let exts = MarkdownSupport.extensions();
        assert!(exts.contains(&"md"));
        assert!(exts.contains(&"markdown"));
    }

    #[test]
    fn comment_delimiters() {
        let delims = MarkdownSupport.comment_delimiters();
        assert!(delims.line.is_none());
        assert_eq!(delims.block_start, Some("<!--"));
        assert_eq!(delims.block_end, Some("-->"));
    }

    #[test]
    fn ts_language_valid() {
        let lang = MarkdownSupport.ts_language();
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&lang).is_ok());
    }

    #[test]
    fn node_kind_always_block() {
        let support = MarkdownSupport;
        let lang = support.ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse("# Hello\n\nSome paragraph.", None).unwrap();
        let root = tree.root_node();
        assert_eq!(support.node_kind(&root), NodeKind::Block);
    }

    #[test]
    fn function_keyword_empty() {
        assert_eq!(MarkdownSupport.function_keyword(), "");
    }

    #[test]
    fn function_body_kind_empty() {
        assert_eq!(MarkdownSupport.function_body_kind(), "");
    }

    #[test]
    fn function_parent_kinds_empty() {
        assert!(MarkdownSupport.function_parent_kinds().is_empty());
    }

    #[test]
    fn decision_point_kinds_empty() {
        assert!(MarkdownSupport.decision_point_kinds().is_empty());
    }

    #[test]
    fn skeleton_query_not_empty() {
        assert!(!MarkdownSupport.skeleton_query().is_empty());
    }

    #[test]
    fn symbols_query_not_empty() {
        assert!(!MarkdownSupport.symbols_query().is_empty());
    }

    #[test]
    fn references_query_not_empty() {
        assert!(!MarkdownSupport.references_query().is_empty());
    }

    #[test]
    fn comment_delimiters_line_is_none() {
        assert!(MarkdownSupport.comment_delimiters().line.is_none());
    }

    fn parse(src: &str) -> tree_sitter::Tree {
        let lang = MarkdownSupport.ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        parser.parse(src, None).unwrap()
    }

    fn run_query(query_src: &str, src: &str) -> Vec<String> {
        use streaming_iterator::StreamingIterator;
        let lang = MarkdownSupport.ts_language();
        let query = Query::new(&lang, query_src).unwrap();
        let tree = parse(src);
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), src.as_bytes());
        let mut results = Vec::new();
        while let Some(m) = matches.next() {
            for c in m.captures {
                results.push(c.node.utf8_text(src.as_bytes()).unwrap().to_string());
            }
        }
        results
    }

    #[test]
    fn skeleton_captures_atx_h1() {
        let results = run_query(MarkdownSupport.skeleton_query(), "# Hello World\n");
        assert!(
            results.iter().any(|s| s.contains("Hello World")),
            "expected heading text in captures, got: {:?}",
            results
        );
    }

    #[test]
    fn skeleton_captures_atx_h2() {
        let results = run_query(MarkdownSupport.skeleton_query(), "## Section Two\n");
        assert!(
            results.iter().any(|s| s.contains("Section Two")),
            "expected heading text in captures, got: {:?}",
            results
        );
    }

    #[test]
    fn skeleton_captures_atx_h3() {
        let results = run_query(MarkdownSupport.skeleton_query(), "### Deep Section\n");
        assert!(
            results.iter().any(|s| s.contains("Deep Section")),
            "expected heading text in captures, got: {:?}",
            results
        );
    }

    #[test]
    fn skeleton_captures_multiple_headings() {
        let src = "# Top\n\n## Middle\n\n### Bottom\n";
        let results = run_query(MarkdownSupport.skeleton_query(), src);
        assert!(
            results.iter().any(|s| s.contains("Top")),
            "missing Top: {:?}",
            results
        );
        assert!(
            results.iter().any(|s| s.contains("Middle")),
            "missing Middle: {:?}",
            results
        );
        assert!(
            results.iter().any(|s| s.contains("Bottom")),
            "missing Bottom: {:?}",
            results
        );
    }

    #[test]
    fn skeleton_query_compiles() {
        let lang = MarkdownSupport.ts_language();
        Query::new(&lang, MarkdownSupport.skeleton_query()).unwrap();
    }

    #[test]
    fn symbols_query_compiles() {
        let lang = MarkdownSupport.ts_language();
        Query::new(&lang, MarkdownSupport.symbols_query()).unwrap();
    }

    #[test]
    fn references_query_compiles() {
        let lang = MarkdownSupport.ts_language();
        Query::new(&lang, MarkdownSupport.references_query()).unwrap();
    }

    #[test]
    fn skeleton_returns_named_headings() {
        use crate::query::{QueryResult, execute_skeleton};
        use crate::{Grammar, SourceFile};
        use std::path::PathBuf;
        let source = SourceFile::from_content(
            PathBuf::from("test.md"),
            "# Introduction\n\nsome text\n\n## Details\n\nmore text\n".to_string(),
            Grammar::Markdown,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => {
                assert!(
                    !s.declarations.is_empty(),
                    "skeleton returned no declarations"
                );
                assert!(
                    s.declarations
                        .iter()
                        .any(|d| d.name.contains("Introduction")),
                    "expected heading text as name, got: {:?}",
                    s.declarations
                );
            }
            _ => panic!("wrong result type"),
        }
    }

    #[test]
    fn symbols_have_names() {
        use crate::query::{QueryResult, execute_symbols};
        use crate::{Grammar, SourceFile};
        use std::path::PathBuf;
        let source = SourceFile::from_content(
            PathBuf::from("test.md"),
            "# My Heading\n\n[link]: https://example.com\n".to_string(),
            Grammar::Markdown,
        )
        .unwrap();
        match execute_symbols(&source).unwrap() {
            QueryResult::Symbols { symbols } => {
                assert!(!symbols.is_empty(), "symbols returned nothing");
                assert!(
                    symbols.iter().all(|s| !s.name().is_empty()),
                    "some symbols have empty names: {:?}",
                    symbols
                );
            }
            _ => panic!("wrong result type"),
        }
    }
}
