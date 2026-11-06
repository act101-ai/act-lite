//! Nix language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Nix in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("nix"),
        display_name: "Nix",
        extensions: &["nix"],
        category: LanguageCategory::Data,
    }
}

/// Nix language support implementation.
#[derive(Default)]
pub struct NixSupport;

impl LanguageSupport for NixSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Nix
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_nix::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["nix"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/nix/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/nix/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/nix/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
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
        assert_eq!(NixSupport.grammar(), Grammar::Nix);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&NixSupport.ts_language()).is_ok());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_skeleton};
        let source = SourceFile::from_content(
            "default.nix",
            "{ mkTool = name: name; }\n".to_string(),
            Grammar::Nix,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => assert!(
                s.declarations.iter().any(|d| d.name == "mkTool"),
                "expected 'mkTool', got: {:?}",
                s.declarations
            ),
            _ => panic!("wrong result type"),
        }
    }
}
