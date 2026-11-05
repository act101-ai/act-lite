//! SCSS/Sass language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register SCSS in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("scss"),
        display_name: "SCSS",
        extensions: &["scss"],
        category: LanguageCategory::Data,
    }
}

/// SCSS/Sass language support implementation.
#[derive(Default)]
pub struct ScssSupport;

impl LanguageSupport for ScssSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Scss
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_scss::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Top-level structure
            "stylesheet" => NodeKind::Unknown,

            // Variable declarations ($variable) - node type is "variable"
            "variable" => NodeKind::Variable,

            // Mixin declarations (@mixin) - node type is "mixin_statement"
            "mixin_statement" => NodeKind::Function,

            // Function declarations (@function) - node type is "function_statement"
            "function_statement" => NodeKind::Function,

            // Rule sets and selectors
            "rule_set" => NodeKind::Class,
            "selectors" => NodeKind::Identifier,
            "class_selector" => NodeKind::Identifier,
            "id_selector" => NodeKind::Identifier,
            "type_selector" => NodeKind::Identifier,
            "tag_name" => NodeKind::Identifier,
            "universal_selector" => NodeKind::Identifier,
            "attribute_selector" => NodeKind::Identifier,
            "pseudo_class_selector" => NodeKind::Identifier,
            "pseudo_element_selector" => NodeKind::Identifier,
            "parent_selector" => NodeKind::Identifier, // & in SCSS

            // Mixin usage (@include)
            "include_statement" => NodeKind::Call,
            "arguments" => NodeKind::Object,

            // Control flow (map to Object since no Decision type exists)
            "if_statement" => NodeKind::Block,
            "each_statement" => NodeKind::Block,
            "for_statement" => NodeKind::Block,
            "while_statement" => NodeKind::Block,

            // Import/Use/Forward
            "import_statement" => NodeKind::Import,
            "use_statement" => NodeKind::Import,
            "forward_statement" => NodeKind::Import,

            // Media queries, at-rules
            "media_statement" => NodeKind::Class,
            "keyframes_statement" => NodeKind::Class,
            "supports_statement" => NodeKind::Class,
            "font_face_statement" => NodeKind::Class,
            "charset_statement" => NodeKind::Constant,
            "namespace_statement" => NodeKind::Module,

            // Properties and values
            "declaration" => NodeKind::Property,
            "property_name" => NodeKind::Identifier,
            "plain_value" => NodeKind::Expression,
            "integer_value" => NodeKind::Expression,
            "float_value" => NodeKind::Expression,
            "string_value" => NodeKind::Expression,
            "color_value" => NodeKind::Expression,
            "function_value" => NodeKind::Expression,
            "variable_value" => NodeKind::Variable, // $variable in expressions

            // Extend placeholder
            "extend_rule" => NodeKind::Class,

            // Block structure
            "block" => NodeKind::Object,

            // Comments
            "comment" => NodeKind::Comment,

            // Default
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["scss"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/scss/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/scss/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/scss/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("//"), // SCSS supports // line comments
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use act_lite_core::LANGUAGE_REGISTRY;

    #[test]
    fn grammar() {
        assert_eq!(ScssSupport.grammar(), Grammar::Scss);
    }

    #[test]
    fn extensions_exact() {
        assert_eq!(ScssSupport.extensions(), &["scss"]);
    }

    #[test]
    fn ts_language() {
        let support = ScssSupport;
        let lang = support.ts_language();
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&lang).is_ok());
    }

    #[test]
    fn node_kind_variable() {
        let support = ScssSupport;
        let mut parser = tree_sitter::Parser::new();
        let lang = support.ts_language();
        parser.set_language(&lang).unwrap();

        let code = r#"
$primary-color: #3498db;
"#;
        if let Some(tree) = parser.parse(code, None) {
            let root = tree.root_node();

            // Variables in SCSS are parsed as declarations
            // Find the declaration node with a property_name starting with $
            fn find_variable_declaration<'a>(
                node: tree_sitter::Node<'a>,
                source: &'a str,
            ) -> Option<tree_sitter::Node<'a>> {
                if node.kind() == "declaration" {
                    // Check if property_name starts with $
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i)
                            && child.kind() == "property_name"
                        {
                            let text = child.utf8_text(source.as_bytes()).unwrap_or("");
                            if text.starts_with('$') {
                                return Some(node);
                            }
                        }
                    }
                }
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i)
                        && let Some(found) = find_variable_declaration(child, source)
                    {
                        return Some(found);
                    }
                }
                None
            }

            if let Some(decl_node) = find_variable_declaration(root, code) {
                // The declaration node itself should be recognized as Property
                // (which is correct - it's still a CSS declaration)
                assert_eq!(support.node_kind(&decl_node), NodeKind::Property);
            } else {
                panic!("No variable declaration found");
            }
        }
    }

    #[test]
    fn node_kind_mixin() {
        let support = ScssSupport;
        let mut parser = tree_sitter::Parser::new();
        let lang = support.ts_language();
        parser.set_language(&lang).unwrap();

        let code = r#"
@mixin button($bg-color: blue) {
    background-color: $bg-color;
}
"#;
        if let Some(tree) = parser.parse(code, None) {
            let root = tree.root_node();
            // Find the mixin_statement node
            for i in 0..root.child_count() {
                if let Some(child) = root.child(i)
                    && child.kind() == "mixin_statement"
                {
                    assert_eq!(support.node_kind(&child), NodeKind::Function);
                    return;
                }
            }
            panic!("No mixin_statement found");
        }
    }

    #[test]
    fn node_kind_function() {
        let support = ScssSupport;
        let mut parser = tree_sitter::Parser::new();
        let lang = support.ts_language();
        parser.set_language(&lang).unwrap();

        let code = r#"
@function em($pixels, $context: 16px) {
    @return ($pixels / $context) * 1em;
}
"#;
        if let Some(tree) = parser.parse(code, None) {
            let root = tree.root_node();
            // Find the function_statement node
            for i in 0..root.child_count() {
                if let Some(child) = root.child(i)
                    && child.kind() == "function_statement"
                {
                    assert_eq!(support.node_kind(&child), NodeKind::Function);
                    return;
                }
            }
            panic!("No function_statement found");
        }
    }

    #[test]
    fn comment_delimiters() {
        let support = ScssSupport;
        let delims = support.comment_delimiters();
        assert_eq!(delims.line, Some("//"));
        assert_eq!(delims.block_start, Some("/*"));
        assert_eq!(delims.block_end, Some("*/"));
    }

    #[test]
    fn queries_embedded() {
        let support = ScssSupport;
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

    #[test]
    fn registry_has_scss() {
        let meta = LANGUAGE_REGISTRY.get(LanguageId("scss"));
        assert!(meta.is_some(), "SCSS should be registered");
    }

    /// Node discovery test - prints all node types found in sample SCSS
    #[test]
    fn discover_node_types() {
        let support = ScssSupport;
        let mut parser = tree_sitter::Parser::new();
        let lang = support.ts_language();
        parser.set_language(&lang).unwrap();

        let code = r#"
// SCSS variables
$primary-color: #3498db;
$font-size-base: 16px;

// Mixin
@mixin button($bg-color: $primary-color) {
    background-color: $bg-color;
    padding: 10px 20px;
    &:hover {
        background-color: darken($bg-color, 10%);
    }
}

// Function
@function em($pixels, $context: 16px) {
    @return ($pixels / $context) * 1em;
}

// Nested rule
.container {
    width: 100%;
    .header {
        h1 {
            font-size: $font-size-base;
        }
    }
}

// Use modern import
@use 'sass:color';

// Keyframes
@keyframes slideIn {
    from { transform: translateX(-100%); }
    to { transform: translateX(0); }
}
"#;

        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        let mut node_types = std::collections::HashSet::new();
        fn collect_types(
            node: tree_sitter::Node,
            types: &mut std::collections::HashSet<&'static str>,
        ) {
            types.insert(node.kind());
            for i in 0..node.child_count() {
                collect_types(node.child(i).unwrap(), types);
            }
        }
        collect_types(root, &mut node_types);

        // Print discovered types for reference
        println!("Discovered SCSS node types:");
        let mut types: Vec<_> = node_types.iter().copied().collect();
        types.sort();
        for t in types {
            println!("  - {}", t);
        }

        // Verify we found the important ones
        assert!(node_types.contains("stylesheet"));
        assert!(node_types.contains("variable"));
        assert!(node_types.contains("mixin_statement"));
        assert!(node_types.contains("function_statement"));
        assert!(node_types.contains("rule_set"));
        assert!(node_types.contains("block"));
    }

    /// Debug tree structure for simple variable
    #[test]
    fn debug_variable_tree() {
        let support = ScssSupport;
        let mut parser = tree_sitter::Parser::new();
        let lang = support.ts_language();
        parser.set_language(&lang).unwrap();

        let code = r#"
$primary-color: #3498db;
"#;

        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn print_tree(node: tree_sitter::Node, depth: usize) {
            let indent = "  ".repeat(depth);
            println!("{}{:?}", indent, node.kind());
            for i in 0..node.child_count() {
                print_tree(node.child(i).unwrap(), depth + 1);
            }
        }
        print_tree(root, 0);
    }
}
