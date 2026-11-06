//! Svelte component language support: the outer document of a `.svelte` file.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Svelte in the language registry. Not `Data`: the analyzers
// read the code of a component's script regions.
inventory::submit! {
    LanguageMeta {
        id: LanguageId("svelte"),
        display_name: "Svelte",
        extensions: &["svelte"],
        category: LanguageCategory::Dynamic,
    }
}

/// Svelte component language support implementation.
#[derive(Default)]
pub struct SvelteSupport;

impl LanguageSupport for SvelteSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Svelte
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_svelte_ng::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "document" => NodeKind::Module,
            "element" | "script_element" | "style_element" => NodeKind::Object,
            "if_statement" | "each_statement" | "await_statement" | "key_statement"
            | "snippet_statement" => NodeKind::Block,
            "expression" | "expression_tag" => NodeKind::Expression,
            "attribute" => NodeKind::Property,
            "attribute_name" => NodeKind::Identifier,
            "attribute_value" | "text" => NodeKind::Expression,
            "comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["svelte"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/svelte/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/svelte/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/svelte/references.scm")
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
        assert_eq!(SvelteSupport.grammar(), Grammar::Svelte);
        assert_eq!(SvelteSupport.extensions(), &["svelte"]);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&SvelteSupport.ts_language()).is_ok());
    }

    #[test]
    fn document_is_a_module() {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&SvelteSupport.ts_language()).unwrap();
        let tree = parser
            .parse(
                "<script>\nlet a = 1;\n</script>\n{#if a}<p>{a}</p>{/if}\n",
                None,
            )
            .unwrap();
        assert!(!tree.root_node().has_error());
        assert_eq!(SvelteSupport.node_kind(&tree.root_node()), NodeKind::Module);
    }
}
