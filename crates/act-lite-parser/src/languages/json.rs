//! JSON language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register JSON in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("json"),
        display_name: "JSON",
        extensions: &["json"],
        category: LanguageCategory::Data,
    }
}

/// JSON language support implementation.
#[derive(Default)]
pub struct JsonSupport;

impl LanguageSupport for JsonSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Json
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_json::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Top-level structures
            "object" => NodeKind::Object,
            "array" => NodeKind::Expression,

            // Object properties
            "pair" => NodeKind::Property,
            "string" => {
                // Strings are either keys or values
                if let Some(parent) = node.parent()
                    && parent.kind() == "pair"
                    && parent.child_count() > 0
                {
                    // Check if this is the key (first child of pair)
                    if let Some(first_child) = parent.child(0)
                        && first_child.id() == node.id()
                    {
                        return NodeKind::Identifier;
                    }
                }
                NodeKind::Expression
            }

            // Primitive values
            "number" => NodeKind::Expression,
            "true" | "false" => NodeKind::Expression,
            "null" => NodeKind::Expression,

            // Comments (JSON with comments)
            "comment" => NodeKind::Comment,

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["json"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/json/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/json/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/json/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        // JSON doesn't support standard comments, but JSONC does (// and /* */)
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
        assert_eq!(JsonSupport.grammar(), Grammar::Json);
    }

    #[test]
    fn extensions() {
        let exts = JsonSupport.extensions();
        assert!(exts.contains(&"json"));
    }

    #[test]
    fn ts_language() {
        let support = JsonSupport;
        let lang = support.ts_language();
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&lang).is_ok());
    }

    #[test]
    fn node_kind_object() {
        let support = JsonSupport;
        let mut parser = tree_sitter::Parser::new();
        let lang = support.ts_language();
        parser.set_language(&lang).unwrap();

        let code = "{}";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        // Find the object node
        for i in 0..root.child_count() {
            let child = root.child(i).unwrap();
            if child.kind() == "object" {
                assert_eq!(support.node_kind(&child), NodeKind::Object);
            }
        }
    }

    #[test]
    fn node_kind_array() {
        let support = JsonSupport;
        let mut parser = tree_sitter::Parser::new();
        let lang = support.ts_language();
        parser.set_language(&lang).unwrap();

        let code = "[]";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        for i in 0..root.child_count() {
            let child = root.child(i).unwrap();
            if child.kind() == "array" {
                assert_eq!(support.node_kind(&child), NodeKind::Expression);
            }
        }
    }

    #[test]
    fn comment_delimiters() {
        let support = JsonSupport;
        let delims = support.comment_delimiters();
        assert_eq!(delims.line, Some("//"));
        assert_eq!(delims.block_start, Some("/*"));
        assert_eq!(delims.block_end, Some("*/"));
    }

    fn parse_json(code: &str) -> tree_sitter::Tree {
        let support = JsonSupport;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&support.ts_language()).unwrap();
        parser.parse(code, None).unwrap()
    }

    fn find_node_by_kind<'a>(
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

    #[test]
    fn node_kind_pair() {
        let support = JsonSupport;
        let tree = parse_json("{\"key\": \"value\"}");
        if let Some(node) = find_node_by_kind(tree.root_node(), "pair") {
            assert_eq!(support.node_kind(&node), NodeKind::Property);
        }
    }

    #[test]
    fn node_kind_string_as_key() {
        let support = JsonSupport;
        let tree = parse_json("{\"key\": \"value\"}");
        // The key string in a pair should be an Identifier
        if let Some(pair) = find_node_by_kind(tree.root_node(), "pair")
            && let Some(first_child) = pair.child(0)
            && first_child.kind() == "string"
        {
            assert_eq!(support.node_kind(&first_child), NodeKind::Identifier);
        }
    }

    #[test]
    fn node_kind_string_as_value() {
        let support = JsonSupport;
        let tree = parse_json("{\"key\": \"value\"}");
        // The value string in a pair should be an Expression
        if let Some(pair) = find_node_by_kind(tree.root_node(), "pair") {
            // The value is at index 2 (after key and colon)
            if let Some(last_child) = pair.child(pair.child_count() - 1)
                && last_child.kind() == "string"
            {
                assert_eq!(support.node_kind(&last_child), NodeKind::Expression);
            }
        }
    }

    #[test]
    fn node_kind_number() {
        let support = JsonSupport;
        let tree = parse_json("42");
        if let Some(node) = find_node_by_kind(tree.root_node(), "number") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_true() {
        let support = JsonSupport;
        let tree = parse_json("true");
        if let Some(node) = find_node_by_kind(tree.root_node(), "true") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_false() {
        let support = JsonSupport;
        let tree = parse_json("false");
        if let Some(node) = find_node_by_kind(tree.root_node(), "false") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_null() {
        let support = JsonSupport;
        let tree = parse_json("null");
        if let Some(node) = find_node_by_kind(tree.root_node(), "null") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_unknown() {
        let support = JsonSupport;
        let tree = parse_json("{\"key\": \"value\"}");
        // root document -> Unknown
        assert_eq!(support.node_kind(&tree.root_node()), NodeKind::Unknown);
    }

    #[test]
    fn default_function_parent_kinds_empty() {
        assert!(JsonSupport.function_parent_kinds().is_empty());
    }

    #[test]
    fn default_function_body_kind_empty() {
        assert_eq!(JsonSupport.function_body_kind(), "");
    }

    #[test]
    fn default_decision_point_kinds_empty() {
        assert!(JsonSupport.decision_point_kinds().is_empty());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::query::{QueryResult, execute_skeleton};
        use crate::{Grammar, SourceFile};
        use std::path::PathBuf;
        let source = SourceFile::from_content(
            PathBuf::from("test.json"),
            r#"{"name": "alice", "age": 30, "address": {"city": "nyc"}}"#.to_string(),
            Grammar::Json,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => assert!(
                s.declarations.iter().any(|d| d.name == "address"),
                "expected 'address' declaration, got: {:?}",
                s.declarations
            ),
            _ => panic!("wrong result type"),
        }
    }
}
