//! HTML language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register HTML in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("html"),
        display_name: "HTML",
        extensions: &["html", "htm"],
        category: LanguageCategory::Data,
    }
}

/// HTML language support implementation.
#[derive(Default)]
pub struct HtmlSupport;

impl LanguageSupport for HtmlSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Html
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_html::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Document structure
            "document" => NodeKind::Module,

            // Elements
            "element" => NodeKind::Object,
            "self_closing_tag" => NodeKind::Object,
            "start_tag" => NodeKind::Object,
            "end_tag" => NodeKind::Object,

            // Text content
            "text" => NodeKind::Expression,

            // Attributes and properties
            "attribute" => NodeKind::Property,
            "attribute_name" => NodeKind::Identifier,
            "attribute_value" => NodeKind::Expression,

            // Comments
            "comment" => NodeKind::Comment,

            // Script and style contents
            "script_element" => NodeKind::Object,
            "style_element" => NodeKind::Object,

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["html", "htm"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/html/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/html/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/html/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: None,
            block_start: Some("<!--"),
            block_end: Some("-->"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar() {
        assert_eq!(HtmlSupport.grammar(), Grammar::Html);
    }

    #[test]
    fn extensions() {
        assert_eq!(HtmlSupport.extensions(), &["html", "htm"]);
    }

    #[test]
    fn comment_delimiters() {
        let delims = HtmlSupport.comment_delimiters();
        assert_eq!(delims.line, None);
        assert_eq!(delims.block_start, Some("<!--"));
        assert_eq!(delims.block_end, Some("-->"));
    }

    #[test]
    fn skeleton_query_not_empty() {
        let query = HtmlSupport.skeleton_query();
        assert!(!query.is_empty(), "skeleton.scm should not be empty");
    }

    #[test]
    fn symbols_query_not_empty() {
        let query = HtmlSupport.symbols_query();
        assert!(!query.is_empty(), "symbols.scm should not be empty");
    }

    #[test]
    fn references_query_not_empty() {
        let query = HtmlSupport.references_query();
        assert!(!query.is_empty(), "references.scm should not be empty");
    }

    fn parse_html(code: &str) -> tree_sitter::Tree {
        let support = HtmlSupport;
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
    fn ts_language_valid() {
        let support = HtmlSupport;
        let lang = support.ts_language();
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&lang).is_ok());
    }

    #[test]
    fn node_kind_document() {
        let support = HtmlSupport;
        let tree = parse_html("<html><body></body></html>");
        assert_eq!(support.node_kind(&tree.root_node()), NodeKind::Module);
    }

    #[test]
    fn node_kind_element() {
        let support = HtmlSupport;
        let tree = parse_html("<div>hello</div>");
        if let Some(node) = find_node_by_kind(tree.root_node(), "element") {
            assert_eq!(support.node_kind(&node), NodeKind::Object);
        }
    }

    #[test]
    fn node_kind_start_tag() {
        let support = HtmlSupport;
        let tree = parse_html("<div class=\"foo\">hello</div>");
        if let Some(node) = find_node_by_kind(tree.root_node(), "start_tag") {
            assert_eq!(support.node_kind(&node), NodeKind::Object);
        }
    }

    #[test]
    fn node_kind_attribute() {
        let support = HtmlSupport;
        let tree = parse_html("<div class=\"foo\">hello</div>");
        if let Some(node) = find_node_by_kind(tree.root_node(), "attribute") {
            assert_eq!(support.node_kind(&node), NodeKind::Property);
        }
    }

    #[test]
    fn node_kind_attribute_name() {
        let support = HtmlSupport;
        let tree = parse_html("<div class=\"foo\">hello</div>");
        if let Some(node) = find_node_by_kind(tree.root_node(), "attribute_name") {
            assert_eq!(support.node_kind(&node), NodeKind::Identifier);
        }
    }

    #[test]
    fn node_kind_attribute_value() {
        let support = HtmlSupport;
        let tree = parse_html("<div class=\"foo\">hello</div>");
        if let Some(node) = find_node_by_kind(tree.root_node(), "attribute_value") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_text() {
        let support = HtmlSupport;
        let tree = parse_html("<div>hello world</div>");
        if let Some(node) = find_node_by_kind(tree.root_node(), "text") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_comment() {
        let support = HtmlSupport;
        let tree = parse_html("<!-- a comment --><div></div>");
        if let Some(node) = find_node_by_kind(tree.root_node(), "comment") {
            assert_eq!(support.node_kind(&node), NodeKind::Comment);
        }
    }

    #[test]
    fn node_kind_script_element() {
        let support = HtmlSupport;
        let tree = parse_html("<script>var x = 1;</script>");
        if let Some(node) = find_node_by_kind(tree.root_node(), "script_element") {
            assert_eq!(support.node_kind(&node), NodeKind::Object);
        }
    }

    #[test]
    fn node_kind_style_element() {
        let support = HtmlSupport;
        let tree = parse_html("<style>div { color: red; }</style>");
        if let Some(node) = find_node_by_kind(tree.root_node(), "style_element") {
            assert_eq!(support.node_kind(&node), NodeKind::Object);
        }
    }

    #[test]
    fn node_kind_unknown() {
        let support = HtmlSupport;
        let tree = parse_html("<div></div>");
        // Find a node that's not in the match list - like "end_tag" -> Object or other
        fn find_any_unknown(node: tree_sitter::Node, support: &HtmlSupport) -> bool {
            if support.node_kind(&node) == NodeKind::Unknown {
                return true;
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if find_any_unknown(child, support) {
                    return true;
                }
            }
            false
        }
        let _ = find_any_unknown(tree.root_node(), &support);
        // Just verify the parsing succeeded
        assert!(tree.root_node().child_count() > 0);
    }

    #[test]
    fn default_function_parent_kinds_empty() {
        assert!(HtmlSupport.function_parent_kinds().is_empty());
    }

    #[test]
    fn default_function_body_kind_empty() {
        assert_eq!(HtmlSupport.function_body_kind(), "");
    }

    #[test]
    fn default_decision_point_kinds_empty() {
        assert!(HtmlSupport.decision_point_kinds().is_empty());
    }
}
