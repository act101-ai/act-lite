//! TOML language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register TOML in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("toml"),
        display_name: "TOML",
        extensions: &["toml"],
        category: LanguageCategory::Data,
    }
}

/// TOML language support implementation.
#[derive(Default)]
pub struct TomlSupport;

impl LanguageSupport for TomlSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Toml
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_toml_ng::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Document root
            "document" => NodeKind::Object,

            // Table headers: [table] and [[table]]
            "table" => NodeKind::Object,
            "table_array_element" => NodeKind::Object,

            // Key-value pairs
            "pair" => NodeKind::Property,

            // Keys
            "bare_key" | "quoted_key" => NodeKind::Identifier,
            "dotted_key" => NodeKind::Identifier,

            // Values
            "string" => NodeKind::Expression,
            "integer" | "float" => NodeKind::Expression,
            "boolean" => NodeKind::Expression,
            "array" => NodeKind::Expression,
            "inline_table" => NodeKind::Object,

            // Date/time values
            "offset_date_time" | "local_date_time" | "local_date" | "local_time" => {
                NodeKind::Expression
            }

            // Comments
            "comment" => NodeKind::Comment,

            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["toml"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/toml/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/toml/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/toml/references.scm")
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
        assert_eq!(TomlSupport.grammar(), Grammar::Toml);
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::query::{QueryResult, execute_skeleton};
        use crate::{Grammar, SourceFile};
        let source = SourceFile::from_content(
            "test.toml",
            "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n".to_string(),
            Grammar::Toml,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => {
                assert!(
                    !s.declarations.is_empty(),
                    "skeleton returned no declarations"
                );
                assert!(
                    s.declarations.iter().any(|d| d.name == "package"),
                    "expected 'package' table, got: {:?}",
                    s.declarations
                );
            }
            _ => panic!("wrong result type"),
        }
    }

    #[test]
    fn extensions() {
        let exts = TomlSupport.extensions();
        assert!(exts.contains(&"toml"));
    }

    #[test]
    fn ts_language() {
        let support = TomlSupport;
        let lang = support.ts_language();
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&lang).is_ok());
    }

    #[test]
    fn comment_delimiters() {
        let delims = TomlSupport.comment_delimiters();
        assert_eq!(delims.line, Some("#"));
        assert_eq!(delims.block_start, None);
        assert_eq!(delims.block_end, None);
    }

    fn parse_toml(code: &str) -> tree_sitter::Tree {
        let support = TomlSupport;
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
        let support = TomlSupport;
        let tree = parse_toml("key = \"value\"\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "pair") {
            assert_eq!(support.node_kind(&node), NodeKind::Property);
        }
    }

    #[test]
    fn node_kind_bare_key() {
        let support = TomlSupport;
        let tree = parse_toml("key = \"value\"\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "bare_key") {
            assert_eq!(support.node_kind(&node), NodeKind::Identifier);
        }
    }

    #[test]
    fn node_kind_table() {
        let support = TomlSupport;
        let tree = parse_toml("[database]\nhost = \"localhost\"\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "table") {
            assert_eq!(support.node_kind(&node), NodeKind::Object);
        }
    }

    #[test]
    fn node_kind_table_array_element() {
        let support = TomlSupport;
        let tree = parse_toml("[[products]]\nname = \"Hammer\"\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "table_array_element") {
            assert_eq!(support.node_kind(&node), NodeKind::Object);
        }
    }

    #[test]
    fn node_kind_string_value() {
        let support = TomlSupport;
        let tree = parse_toml("key = \"value\"\n");
        // The string value node
        if let Some(node) = find_node_by_kind(tree.root_node(), "string") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_integer() {
        let support = TomlSupport;
        let tree = parse_toml("port = 5432\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "integer") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_boolean() {
        let support = TomlSupport;
        let tree = parse_toml("enabled = true\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "boolean") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_document() {
        let support = TomlSupport;
        let tree = parse_toml("key = \"value\"\n");
        assert_eq!(support.node_kind(&tree.root_node()), NodeKind::Object);
    }

    #[test]
    fn default_function_parent_kinds_empty() {
        assert!(TomlSupport.function_parent_kinds().is_empty());
    }

    #[test]
    fn default_function_body_kind_empty() {
        assert_eq!(TomlSupport.function_body_kind(), "");
    }

    #[test]
    fn default_decision_point_kinds_empty() {
        assert!(TomlSupport.decision_point_kinds().is_empty());
    }
}
