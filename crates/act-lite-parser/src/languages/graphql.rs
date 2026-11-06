//! GraphQL language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register GraphQL in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("graphql"),
        display_name: "GraphQL",
        extensions: &["graphql", "graphqls", "gql"],
        category: LanguageCategory::Data,
    }
}

/// GraphQL language support implementation.
#[derive(Default)]
pub struct GraphQlSupport;

impl LanguageSupport for GraphQlSupport {
    fn grammar(&self) -> Grammar {
        Grammar::GraphQl
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_graphql::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["graphql", "graphqls", "gql"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/graphql/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/graphql/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/graphql/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
            block_start: None,
            block_end: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar() {
        assert_eq!(GraphQlSupport.grammar(), Grammar::GraphQl);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&GraphQlSupport.ts_language()).is_ok());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_skeleton};
        let source = SourceFile::from_content(
            "test.graphql",
            "type User { id: ID }\n".to_string(),
            Grammar::GraphQl,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => assert!(
                s.declarations.iter().any(|d| d.name == "User"),
                "expected 'User', got: {:?}",
                s.declarations
            ),
            _ => panic!("wrong result type"),
        }
    }
}
