//! JSONC (JSON with comments) language support: JSON's grammar and queries,
//! so the same content answers as JSON does.

use crate::languages::JsonSupport;
use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register JSONC in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("jsonc"),
        display_name: "JSONC",
        extensions: &["jsonc", "code-workspace"],
        category: LanguageCategory::Data,
    }
}

/// JSONC language support implementation.
#[derive(Default)]
pub struct JsoncSupport;

impl LanguageSupport for JsoncSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Jsonc
    }

    fn ts_language(&self) -> tree_sitter::Language {
        JsonSupport.ts_language()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        JsonSupport.node_kind(node)
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["jsonc", "code-workspace"]
    }

    fn skeleton_query(&self) -> &'static str {
        JsonSupport.skeleton_query()
    }

    fn symbols_query(&self) -> &'static str {
        JsonSupport.symbols_query()
    }

    fn references_query(&self) -> &'static str {
        JsonSupport.references_query()
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        JsonSupport.comment_delimiters()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar() {
        assert_eq!(JsoncSupport.grammar(), Grammar::Jsonc);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&JsoncSupport.ts_language()).is_ok());
    }

    #[test]
    fn queries_are_json_queries() {
        assert_eq!(JsoncSupport.skeleton_query(), JsonSupport.skeleton_query());
        assert_eq!(JsoncSupport.symbols_query(), JsonSupport.symbols_query());
        assert_eq!(
            JsoncSupport.references_query(),
            JsonSupport.references_query()
        );
    }
}
