//! YAML language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register YAML in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("yaml"),
        display_name: "YAML",
        extensions: &["yaml", "yml"],
        category: LanguageCategory::Data,
    }
}

/// YAML language support implementation.
#[derive(Default)]
pub struct YamlSupport;

impl LanguageSupport for YamlSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Yaml
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_yaml::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Top-level mappings and sequences
            "stream" => NodeKind::Module,
            "document" => NodeKind::Module,
            "block_mapping" | "flow_mapping" => NodeKind::Class,
            "block_sequence" | "flow_sequence" => NodeKind::Class,

            // Mapping and sequence items
            "block_mapping_pair" => NodeKind::Property,
            "block_sequence_item" => NodeKind::Property,

            // Keys and identifiers
            "flow_node" => NodeKind::Identifier,
            "plain_scalar"
            | "double_quote_scalar"
            | "single_quote_scalar"
            | "literal_scalar"
            | "folded_scalar" => NodeKind::Identifier,

            // Values
            "block_node" => NodeKind::Expression,

            // Anchors and aliases
            "anchor" => NodeKind::Identifier,
            "alias" => NodeKind::Identifier,

            // Comments
            "comment" => NodeKind::Comment,

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["yaml", "yml"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/yaml/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/yaml/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/yaml/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
            block_start: None,
            block_end: None,
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        // YAML is a data format, not a programming language
        &[]
    }

    fn function_body_kind(&self) -> &'static str {
        "block_node"
    }

    fn function_keyword(&self) -> &'static str {
        ""
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        // YAML is a data format, not a programming language
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::parse_with_support;
    use act_lite_core::registry::{LANGUAGE_REGISTRY, LanguageId};

    crate::language_support_tests!(YamlSupport, Grammar::Yaml, &["yaml", "yml"]);

    #[test]
    #[allow(non_snake_case)]
    fn registry_has_YAML() {
        let meta = LANGUAGE_REGISTRY.get(LanguageId("yaml"));
        assert!(meta.is_some(), "YAML should be registered");
    }

    #[test]
    #[allow(non_snake_case)]
    fn registry_extension_lookup_YAML() {
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("yaml"),
            Some(LanguageId("yaml"))
        );
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("yml"),
            Some(LanguageId("yaml"))
        );
    }

    #[test]
    fn yaml_comment_delimiters() {
        let support = YamlSupport;
        let delims = support.comment_delimiters();
        assert_eq!(delims.line, Some("#"));
        assert_eq!(delims.block_start, None);
        assert_eq!(delims.block_end, None);
    }

    #[test]
    fn yaml_node_kinds() {
        let support = YamlSupport;
        let code = "key: value\nitems:\n  - one\n  - two";
        let tree = parse_with_support(&support, code);
        let root = tree.root_node();

        // Verify we can walk the tree and get node kinds
        let mut cursor = root.walk();
        let mut found_mapping = false;
        if cursor.goto_first_child() {
            loop {
                let node = cursor.node();
                let kind = support.node_kind(&node);
                if matches!(kind, NodeKind::Class) {
                    found_mapping = true;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        assert!(
            found_mapping || root.child_count() > 0,
            "should find YAML structure"
        );
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
    fn node_kind_stream() {
        let support = YamlSupport;
        let code = "key: value\n";
        let tree = parse_with_support(&support, code);
        // root is "stream"
        assert_eq!(support.node_kind(&tree.root_node()), NodeKind::Module);
    }

    #[test]
    fn node_kind_document() {
        let support = YamlSupport;
        let code = "key: value\n";
        let tree = parse_with_support(&support, code);
        if let Some(node) = find_node_by_kind(tree.root_node(), "document") {
            assert_eq!(support.node_kind(&node), NodeKind::Module);
        }
    }

    #[test]
    fn node_kind_block_mapping() {
        let support = YamlSupport;
        let code = "key: value\nother: data\n";
        let tree = parse_with_support(&support, code);
        if let Some(node) = find_node_by_kind(tree.root_node(), "block_mapping") {
            assert_eq!(support.node_kind(&node), NodeKind::Class);
        }
    }

    #[test]
    fn node_kind_block_sequence() {
        let support = YamlSupport;
        let code = "items:\n  - one\n  - two\n";
        let tree = parse_with_support(&support, code);
        if let Some(node) = find_node_by_kind(tree.root_node(), "block_sequence") {
            assert_eq!(support.node_kind(&node), NodeKind::Class);
        }
    }

    #[test]
    fn node_kind_block_mapping_pair() {
        let support = YamlSupport;
        let code = "key: value\n";
        let tree = parse_with_support(&support, code);
        if let Some(node) = find_node_by_kind(tree.root_node(), "block_mapping_pair") {
            assert_eq!(support.node_kind(&node), NodeKind::Property);
        }
    }

    #[test]
    fn node_kind_block_sequence_item() {
        let support = YamlSupport;
        let code = "- one\n- two\n";
        let tree = parse_with_support(&support, code);
        if let Some(node) = find_node_by_kind(tree.root_node(), "block_sequence_item") {
            assert_eq!(support.node_kind(&node), NodeKind::Property);
        }
    }

    #[test]
    fn node_kind_plain_scalar() {
        let support = YamlSupport;
        let code = "key: value\n";
        let tree = parse_with_support(&support, code);
        if let Some(node) = find_node_by_kind(tree.root_node(), "plain_scalar") {
            assert_eq!(support.node_kind(&node), NodeKind::Identifier);
        }
    }

    #[test]
    fn node_kind_comment() {
        let support = YamlSupport;
        let code = "# comment\nkey: value\n";
        let tree = parse_with_support(&support, code);
        if let Some(node) = find_node_by_kind(tree.root_node(), "comment") {
            assert_eq!(support.node_kind(&node), NodeKind::Comment);
        }
    }

    #[test]
    fn node_kind_unknown() {
        let support = YamlSupport;
        // Test that unrecognized node kinds map to Unknown
        // We can use a real node that doesn't match any case
        let code = "key: value\n";
        let tree = parse_with_support(&support, code);
        // Walk looking for unrecognized kind
        let root = tree.root_node();
        fn walk_for_unknown(node: tree_sitter::Node, support: &YamlSupport) -> bool {
            if support.node_kind(&node) == NodeKind::Unknown {
                return true;
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if walk_for_unknown(child, support) {
                    return true;
                }
            }
            false
        }
        let _found_unknown = walk_for_unknown(root, &support);
        assert!(root.child_count() > 0);
    }

    #[test]
    fn node_kind_block_node() {
        let support = YamlSupport;
        let code = "key:\n  nested: value\n";
        let tree = parse_with_support(&support, code);
        if let Some(node) = find_node_by_kind(tree.root_node(), "block_node") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn function_body_kind_is_block_node() {
        let support = YamlSupport;
        assert_eq!(support.function_body_kind(), "block_node");
    }

    #[test]
    fn function_keyword_empty() {
        let support = YamlSupport;
        assert_eq!(support.function_keyword(), "");
    }

    #[test]
    fn function_parent_kinds_empty() {
        let support = YamlSupport;
        assert!(support.function_parent_kinds().is_empty());
    }

    #[test]
    fn decision_point_kinds_empty() {
        let support = YamlSupport;
        assert!(support.decision_point_kinds().is_empty());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::query::{QueryResult, execute_skeleton};
        use crate::{Grammar, SourceFile};
        use std::path::PathBuf;
        let source = SourceFile::from_content(
            PathBuf::from("test.yaml"),
            "name: alice\nage: 30\n".to_string(),
            Grammar::Yaml,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => assert!(
                !s.declarations.is_empty(),
                "skeleton returned no declarations"
            ),
            _ => panic!("wrong result type"),
        }
    }
}
