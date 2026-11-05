//! HCL/Terraform language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register HCL/Terraform in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("hcl_terraform"),
        display_name: "HCL/Terraform",
        extensions: &["hcl", "tf"],
        category: LanguageCategory::Data,
    }
}

/// HCL/Terraform language support implementation.
#[derive(Default)]
pub struct HclTerraformSupport;

impl LanguageSupport for HclTerraformSupport {
    fn grammar(&self) -> Grammar {
        Grammar::HclTerraform
    }

    fn ts_language(&self) -> tree_sitter::Language {
        // tree-sitter-hcl uses LanguageFn which can be converted to tree_sitter::Language
        tree_sitter_hcl::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Top-level blocks - these define structure
            // Source-dependent classification is handled in node_kind_with_source.
            "block" => NodeKind::Block,

            // Expressions and literals
            "string_lit" => NodeKind::Expression,
            "number_lit" => NodeKind::Expression,
            "bool_lit" => NodeKind::Expression,
            "null_lit" => NodeKind::Expression,
            "list_lit" => NodeKind::Expression,
            "map_lit" => NodeKind::Expression,
            "tuple_cons" => NodeKind::Expression,
            "object_cons" => NodeKind::Expression,

            // Identifiers and expressions
            "identifier" => NodeKind::Identifier,
            "type_expr" => NodeKind::Type,
            "attribute_splat" => NodeKind::Call,
            "getattr" => NodeKind::Call,
            "index" => NodeKind::Call,
            "binary_op" => NodeKind::Call,
            "conditional" => NodeKind::Call,
            "unary_op" => NodeKind::Call,
            "function_call" => NodeKind::Call,

            // Comments
            "comment" => NodeKind::Comment,

            // Other nodes
            _ => NodeKind::Unknown,
        }
    }

    fn node_kind_with_source(&self, node: &tree_sitter::Node, source: &[u8]) -> NodeKind {
        if node.kind() == "block" {
            if let Some(child) = node.child(0)
                && let Ok(text) = child.utf8_text(source)
            {
                return match text.trim() {
                    "terraform" | "provider" => NodeKind::Block,
                    "resource" | "data" => NodeKind::Class,
                    "variable" | "output" | "locals" => NodeKind::Variable,
                    "module" => NodeKind::Call,
                    _ => NodeKind::Block,
                };
            }
            return NodeKind::Block;
        }
        self.node_kind(node)
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["hcl", "tf"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/hcl_terraform/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/hcl_terraform/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/hcl_terraform/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        // HCL doesn't have traditional functions, but blocks are the functional units
        &["block"]
    }

    fn function_body_kind(&self) -> &'static str {
        // Block contents
        "block"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        // HCL doesn't have control flow in the traditional sense
        // but conditional expressions are decision points
        &["conditional"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::parse_with_support;
    use act_lite_core::registry::{LANGUAGE_REGISTRY, LanguageId};

    crate::language_support_tests!(HclTerraformSupport, Grammar::HclTerraform, &["hcl", "tf"]);

    #[test]
    fn registry_has_hcl_terraform() {
        let meta = LANGUAGE_REGISTRY.get(LanguageId("hcl_terraform"));
        assert!(meta.is_some(), "HCL/Terraform should be registered");
    }

    #[test]
    fn registry_extension_lookup_hcl() {
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("hcl"),
            Some(LanguageId("hcl_terraform")),
            "hcl extension should resolve to hcl_terraform language"
        );
    }

    #[test]
    fn registry_extension_lookup_tf() {
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("tf"),
            Some(LanguageId("hcl_terraform")),
            "tf extension should resolve to hcl_terraform language"
        );
    }

    #[test]
    fn comment_delimiters_hcl() {
        let support = HclTerraformSupport;
        let delims = support.comment_delimiters();
        assert_eq!(delims.line, Some("#"));
        assert_eq!(delims.block_start, Some("/*"));
        assert_eq!(delims.block_end, Some("*/"));
    }

    #[test]
    fn parser_can_set_hcl_language() {
        let support = HclTerraformSupport;
        let lang = support.ts_language();
        let mut parser = tree_sitter::Parser::new();
        assert!(
            parser.set_language(&lang).is_ok(),
            "should be able to set HCL language"
        );
    }

    fn parse_hcl(code: &str) -> tree_sitter::Tree {
        let support = HclTerraformSupport;
        parse_with_support(&support, code)
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
    fn node_kind_identifier() {
        let support = HclTerraformSupport;
        let tree = parse_hcl("resource \"aws_instance\" \"example\" {\n  ami = \"ami-12345\"\n}\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "identifier") {
            assert_eq!(support.node_kind(&node), NodeKind::Identifier);
        }
    }

    #[test]
    fn node_kind_comment() {
        let support = HclTerraformSupport;
        let tree = parse_hcl("# this is a comment\nresource \"aws_instance\" \"example\" {}\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "comment") {
            assert_eq!(support.node_kind(&node), NodeKind::Comment);
        }
    }

    #[test]
    fn node_kind_string_lit() {
        let support = HclTerraformSupport;
        let tree = parse_hcl("variable \"name\" {\n  default = \"value\"\n}\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "string_lit") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_function_call() {
        let support = HclTerraformSupport;
        // Verify function_call maps to NodeKind::Call
        // Use an attribute with a function call expression
        let tree = parse_hcl("locals {\n  x = max(1, 2)\n}\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "function_call") {
            assert_eq!(support.node_kind(&node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_number_lit() {
        let support = HclTerraformSupport;
        let tree = parse_hcl("locals {\n  x = 42\n}\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "number_lit") {
            assert_eq!(support.node_kind(&node), NodeKind::Expression);
        }
    }

    #[test]
    fn node_kind_unknown() {
        let support = HclTerraformSupport;
        let tree = parse_hcl("variable \"name\" {\n  default = \"value\"\n}\n");
        // root node should be unknown (it's "config_file")
        assert_eq!(support.node_kind(&tree.root_node()), NodeKind::Unknown);
    }

    #[test]
    fn function_parent_kinds() {
        let support = HclTerraformSupport;
        assert!(support.function_parent_kinds().contains(&"block"));
    }

    #[test]
    fn function_body_kind() {
        let support = HclTerraformSupport;
        assert_eq!(support.function_body_kind(), "block");
    }

    #[test]
    fn decision_point_kinds() {
        let support = HclTerraformSupport;
        assert!(support.decision_point_kinds().contains(&"conditional"));
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::query::{QueryResult, execute_skeleton};
        use crate::{Grammar, SourceFile};
        use std::path::PathBuf;
        let source = SourceFile::from_content(
            PathBuf::from("main.tf"),
            "resource \"aws_instance\" \"web\" {\n  ami = \"ami-12345\"\n}\n".to_string(),
            Grammar::HclTerraform,
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

    #[test]
    fn symbols_have_names() {
        use crate::query::{QueryResult, execute_symbols};
        use crate::{Grammar, SourceFile};
        use std::path::PathBuf;
        let source = SourceFile::from_content(
            PathBuf::from("main.tf"),
            "variable \"region\" {\n  default = \"us-east-1\"\n}\n".to_string(),
            Grammar::HclTerraform,
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
