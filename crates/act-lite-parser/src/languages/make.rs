//! Make language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Make in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("make"),
        display_name: "Make",
        extensions: &["mk", "mak"],
        category: LanguageCategory::Data,
    }
}

/// Make language support implementation.
#[derive(Default)]
pub struct MakeSupport;

impl LanguageSupport for MakeSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Make
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_make::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["mk", "mak"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/make/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/make/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/make/references.scm")
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
        assert_eq!(MakeSupport.grammar(), Grammar::Make);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&MakeSupport.ts_language()).is_ok());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_skeleton};
        let source =
            SourceFile::from_content("Makefile", "CC = cc\n".to_string(), Grammar::Make).unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => assert!(
                s.declarations.iter().any(|d| d.name == "CC"),
                "expected 'CC', got: {:?}",
                s.declarations
            ),
            _ => panic!("wrong result type"),
        }
    }
}
