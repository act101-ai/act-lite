//! Vue component language support: the outer document of a `.vue` file.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Vue in the language registry. Not `Data`: the analyzers
// read the code of a component's script regions.
inventory::submit! {
    LanguageMeta {
        id: LanguageId("vue"),
        display_name: "Vue",
        extensions: &["vue"],
        category: LanguageCategory::Dynamic,
    }
}

/// Vue component language support implementation.
#[derive(Default)]
pub struct VueSupport;

impl LanguageSupport for VueSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Vue
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_vue_next::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "document" => NodeKind::Module,
            "element" | "template_element" | "script_element" | "style_element" => NodeKind::Object,
            "interpolation" | "directive_value" => NodeKind::Expression,
            "directive_attribute" => NodeKind::Property,
            "attribute" => NodeKind::Property,
            "attribute_name" => NodeKind::Identifier,
            "attribute_value" | "text" => NodeKind::Expression,
            "comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["vue"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/vue/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/vue/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/vue/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: None,
            block_start: Some("<!--"),
            block_end: Some("-->"),
        }
    }

    /// The outer document holds markup and the byte ranges of its script and
    /// style regions, never executable code of its own, so no code-semantic
    /// operation applies to it.
    fn coverage_opt_outs(&self) -> &'static [&'static str] {
        &[
            "call_detection",
            "unsafe_surface",
            "secret_surface",
            "data_flow",
            "effect_summary",
            "taint_flow",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_and_extension() {
        assert_eq!(VueSupport.grammar(), Grammar::Vue);
        assert_eq!(VueSupport.extensions(), &["vue"]);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&VueSupport.ts_language()).is_ok());
    }

    #[test]
    fn document_is_a_module() {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&VueSupport.ts_language()).unwrap();
        let tree = parser
            .parse(
                "<template><p>{{ a }}</p></template>\n<script setup>\nconst a = 1;\n</script>\n",
                None,
            )
            .unwrap();
        assert!(!tree.root_node().has_error());
        assert_eq!(VueSupport.node_kind(&tree.root_node()), NodeKind::Module);
    }
}
