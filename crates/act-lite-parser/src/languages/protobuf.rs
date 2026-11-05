//! Protocol Buffers language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Protocol Buffers in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("protobuf"),
        display_name: "Protocol Buffers",
        extensions: &["proto"],
        category: LanguageCategory::Data,
    }
}

/// Protocol Buffers language support implementation.
#[derive(Default)]
pub struct ProtobufSupport;

impl LanguageSupport for ProtobufSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Protobuf
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_proto::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["proto"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/protobuf/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/protobuf/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/protobuf/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar() {
        assert_eq!(ProtobufSupport.grammar(), Grammar::Protobuf);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&ProtobufSupport.ts_language()).is_ok());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_skeleton};
        let source = SourceFile::from_content(
            "test.proto",
            "syntax = \"proto3\";\nmessage Order { int32 id = 1; }\n".to_string(),
            Grammar::Protobuf,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => assert!(
                s.declarations.iter().any(|d| d.name == "Order"),
                "expected 'Order', got: {:?}",
                s.declarations
            ),
            _ => panic!("wrong result type"),
        }
    }
}
