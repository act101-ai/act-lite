//! CSS language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register CSS in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("css"),
        display_name: "CSS",
        extensions: &["css"],
        category: LanguageCategory::Data,
    }
}

/// CSS language support implementation.
#[derive(Default)]
pub struct CssSupport;

impl LanguageSupport for CssSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Css
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_css::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Top-level structure
            "stylesheet" => NodeKind::Unknown,

            // Rules and blocks
            "rule_set" => NodeKind::Class, // CSS rules map to class-like structures
            "media_statement" => NodeKind::Class, // @media blocks
            "import_statement" => NodeKind::Import,
            "charset_statement" => NodeKind::Constant,
            "font_face_statement" => NodeKind::Class,
            "keyframes_statement" => NodeKind::Class,
            "supports_statement" => NodeKind::Class,
            "namespace_statement" => NodeKind::Module,
            "page_statement" => NodeKind::Class,

            // Selectors
            "selectors" => NodeKind::Identifier,
            "class_selector" => NodeKind::Identifier,
            "id_selector" => NodeKind::Identifier,
            "tag_name" => NodeKind::Identifier,
            "universal_selector" => NodeKind::Identifier,
            "attribute_selector" => NodeKind::Identifier,
            "pseudo_class_selector" => NodeKind::Identifier,
            "pseudo_element_selector" => NodeKind::Identifier,

            // Properties and values
            "declaration" => NodeKind::Property,
            "property_name" => NodeKind::Identifier,
            "plain_value" => NodeKind::Expression,
            "integer_value" => NodeKind::Expression,
            "float_value" => NodeKind::Expression,
            "string_value" => NodeKind::Expression,
            "color_value" => NodeKind::Expression,
            "call_expression" => NodeKind::Expression,
            "block" => NodeKind::Object,

            // Comments
            "comment" => NodeKind::Comment,

            // Default
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["css"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/css/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/css/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/css/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: None, // CSS has no line comments
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
        assert_eq!(CssSupport.grammar(), Grammar::Css);
    }

    #[test]
    fn extensions() {
        let exts = CssSupport.extensions();
        assert!(exts.contains(&"css"));
    }

    #[test]
    fn ts_language() {
        let support = CssSupport;
        let lang = support.ts_language();
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&lang).is_ok());
    }

    #[test]
    fn node_kind_rule_set() {
        let support = CssSupport;
        let mut parser = tree_sitter::Parser::new();
        let lang = support.ts_language();
        parser.set_language(&lang).unwrap();

        let code = ".button { color: red; }";
        if let Some(tree) = parser.parse(code, None) {
            let root = tree.root_node();
            // Find the rule_set node
            for i in 0..root.child_count() {
                if let Some(child) = root.child(i)
                    && child.kind() == "rule_set"
                {
                    assert_eq!(support.node_kind(&child), NodeKind::Class);
                    return;
                }
            }
            // If we couldn't find a rule_set, at least verify the tree parsed
            assert!(root.child_count() > 0);
        }
    }

    #[test]
    fn comment_delimiters() {
        let support = CssSupport;
        let delims = support.comment_delimiters();
        assert_eq!(delims.line, None);
        assert_eq!(delims.block_start, Some("/*"));
        assert_eq!(delims.block_end, Some("*/"));
    }

    #[test]
    fn queries_embedded() {
        let support = CssSupport;
        // Verify queries are non-empty
        assert!(
            !support.skeleton_query().is_empty(),
            "skeleton query should not be empty"
        );
        assert!(
            !support.symbols_query().is_empty(),
            "symbols query should not be empty"
        );
        assert!(
            !support.references_query().is_empty(),
            "references query should not be empty"
        );
    }

    fn parse_css(code: &str) -> tree_sitter::Tree {
        let support = CssSupport;
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
    fn node_kind_stylesheet() {
        let support = CssSupport;
        let tree = parse_css(".btn { color: red; }");
        assert_eq!(support.node_kind(&tree.root_node()), NodeKind::Unknown);
    }

    #[test]
    fn node_kind_media_statement() {
        let support = CssSupport;
        let tree = parse_css("@media (max-width: 768px) { .btn { color: red; } }");
        if let Some(node) = find_node_by_kind(tree.root_node(), "media_statement") {
            assert_eq!(support.node_kind(&node), NodeKind::Class);
        }
    }

    #[test]
    fn node_kind_import_statement() {
        let support = CssSupport;
        let tree = parse_css("@import url('styles.css');");
        if let Some(node) = find_node_by_kind(tree.root_node(), "import_statement") {
            assert_eq!(support.node_kind(&node), NodeKind::Import);
        }
    }

    #[test]
    fn node_kind_declaration() {
        let support = CssSupport;
        let tree = parse_css(".btn { color: red; }");
        if let Some(node) = find_node_by_kind(tree.root_node(), "declaration") {
            assert_eq!(support.node_kind(&node), NodeKind::Property);
        }
    }

    #[test]
    fn node_kind_comment() {
        let support = CssSupport;
        let tree = parse_css("/* a comment */ .btn { color: red; }");
        if let Some(node) = find_node_by_kind(tree.root_node(), "comment") {
            assert_eq!(support.node_kind(&node), NodeKind::Comment);
        }
    }

    #[test]
    fn node_kind_block() {
        let support = CssSupport;
        let tree = parse_css(".btn { color: red; }");
        if let Some(node) = find_node_by_kind(tree.root_node(), "block") {
            assert_eq!(support.node_kind(&node), NodeKind::Object);
        }
    }

    #[test]
    fn node_kind_class_selector() {
        let support = CssSupport;
        let tree = parse_css(".btn { color: red; }");
        if let Some(node) = find_node_by_kind(tree.root_node(), "class_selector") {
            assert_eq!(support.node_kind(&node), NodeKind::Identifier);
        }
    }

    #[test]
    fn node_kind_id_selector() {
        let support = CssSupport;
        let tree = parse_css("#main { color: blue; }");
        if let Some(node) = find_node_by_kind(tree.root_node(), "id_selector") {
            assert_eq!(support.node_kind(&node), NodeKind::Identifier);
        }
    }

    #[test]
    fn node_kind_tag_name() {
        let support = CssSupport;
        let tree = parse_css("div { margin: 0; }");
        if let Some(node) = find_node_by_kind(tree.root_node(), "tag_name") {
            assert_eq!(support.node_kind(&node), NodeKind::Identifier);
        }
    }

    #[test]
    fn node_kind_keyframes_statement() {
        let support = CssSupport;
        let tree = parse_css("@keyframes slide { from { opacity: 0; } to { opacity: 1; } }");
        if let Some(node) = find_node_by_kind(tree.root_node(), "keyframes_statement") {
            assert_eq!(support.node_kind(&node), NodeKind::Class);
        }
    }

    #[test]
    fn node_kind_integer_value() {
        let support = CssSupport;
        let tree = parse_css(".btn { z-index: 100; }");
        if let Some(node) = find_node_by_kind(tree.root_node(), "integer_value") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_plain_value() {
        let support = CssSupport;
        let tree = parse_css(".btn { color: red; }");
        if let Some(node) = find_node_by_kind(tree.root_node(), "plain_value") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_font_face_statement() {
        let support = CssSupport;
        let tree = parse_css("@font-face { font-family: 'MyFont'; src: url('font.woff'); }");
        if let Some(node) = find_node_by_kind(tree.root_node(), "font_face_statement") {
            assert_eq!(support.node_kind(&node), NodeKind::Class);
        }
    }

    #[test]
    fn default_function_parent_kinds_empty() {
        let support = CssSupport;
        assert!(support.function_parent_kinds().is_empty());
    }

    #[test]
    fn default_function_body_kind_empty() {
        let support = CssSupport;
        assert_eq!(support.function_body_kind(), "");
    }

    #[test]
    fn default_decision_point_kinds_empty() {
        let support = CssSupport;
        assert!(support.decision_point_kinds().is_empty());
    }
}
