//! CMake language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register CMake in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("cmake"),
        display_name: "CMake",
        extensions: &["cmake"],
        category: LanguageCategory::Data,
    }
}

/// CMake language support implementation.
#[derive(Default)]
pub struct CMakeSupport;

impl LanguageSupport for CMakeSupport {
    fn grammar(&self) -> Grammar {
        Grammar::CMake
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_cmake::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "line_comment" | "bracket_comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cmake"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/cmake/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/cmake/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/cmake/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
            block_start: Some("#[["),
            block_end: Some("]]"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar() {
        assert_eq!(CMakeSupport.grammar(), Grammar::CMake);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&CMakeSupport.ts_language()).is_ok());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_skeleton};
        let source = SourceFile::from_content(
            "CMakeLists.txt",
            "function(add_tool name)\nendfunction()\n".to_string(),
            Grammar::CMake,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => assert!(
                s.declarations.iter().any(|d| d.name == "add_tool"),
                "expected 'add_tool', got: {:?}",
                s.declarations
            ),
            _ => panic!("wrong result type"),
        }
    }
}
