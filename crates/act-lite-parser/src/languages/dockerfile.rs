//! Dockerfile language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Dockerfile in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("dockerfile"),
        display_name: "Dockerfile",
        extensions: &["dockerfile"],
        category: LanguageCategory::Data,
    }
}

/// Dockerfile language support implementation.
#[derive(Default)]
pub struct DockerfileSupport;

impl LanguageSupport for DockerfileSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Dockerfile
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_dockerfile::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Declarations - instructions that define structure
            "from_instruction" => NodeKind::Class, // FROM defines a stage/base
            "arg_instruction" => NodeKind::Variable, // ARG declares build argument
            "env_instruction" => NodeKind::Variable, // ENV declares environment variable
            "workdir_instruction" => NodeKind::Call, // WORKDIR sets working directory
            "copy_instruction" => NodeKind::Call,  // COPY copies files
            "add_instruction" => NodeKind::Call,   // ADD adds files/URLs
            "run_instruction" => NodeKind::Call,   // RUN executes command
            "expose_instruction" => NodeKind::Call, // EXPOSE documents ports
            "volume_instruction" => NodeKind::Call, // VOLUME documents volumes
            "user_instruction" => NodeKind::Call,  // USER sets user context
            "cmd_instruction" => NodeKind::Call,   // CMD sets default command
            "entrypoint_instruction" => NodeKind::Call, // ENTRYPOINT sets entrypoint
            "label_instruction" => NodeKind::Call, // LABEL adds metadata
            "healthcheck_instruction" => NodeKind::Call, // HEALTHCHECK adds health check
            "stopsignal_instruction" => NodeKind::Call, // STOPSIGNAL sets signal
            "onbuild_instruction" => NodeKind::Call, // ONBUILD deferred instruction
            "cross_build_instruction" => NodeKind::Call, // Cross-build directive

            // Comments
            "comment" => NodeKind::Comment,

            // Other nodes
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["dockerfile"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/dockerfile/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/dockerfile/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/dockerfile/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
            block_start: None,
            block_end: None,
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        // Dockerfile doesn't have traditional functions, but we treat
        // multi-stage builds (FROM statements) as "function parents"
        &["from_instruction"]
    }

    fn function_body_kind(&self) -> &'static str {
        // Everything between one FROM and the next FROM is a "body"
        // For single-stage, it's all instructions after FROM
        "instruction" // Generic instruction node
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        // Dockerfile doesn't have traditional control flow,
        // but HEALTHCHECK and RUN with exit codes can be decision points
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::parse_with_support;
    use act_lite_core::registry::{LANGUAGE_REGISTRY, LanguageId};

    crate::language_support_tests!(DockerfileSupport, Grammar::Dockerfile, &["dockerfile"]);

    #[test]
    fn registry_has_dockerfile() {
        let meta = LANGUAGE_REGISTRY.get(LanguageId("dockerfile"));
        assert!(meta.is_some(), "Dockerfile should be registered");
    }

    #[test]
    fn registry_extension_lookup_dockerfile() {
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("dockerfile"),
            Some(LanguageId("dockerfile")),
            "dockerfile extension should resolve to dockerfile language"
        );
    }

    #[test]
    fn comment_delimiters_dockerfile() {
        let support = DockerfileSupport;
        let delims = support.comment_delimiters();
        assert_eq!(delims.line, Some("#"));
        assert_eq!(delims.block_start, None);
    }

    fn parse_dockerfile(code: &str) -> tree_sitter::Tree {
        let support = DockerfileSupport;
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
    fn node_kind_from_instruction() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "from_instruction") {
            assert_eq!(support.node_kind(&node), NodeKind::Class);
        }
    }

    #[test]
    fn node_kind_arg_instruction() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\nARG MY_VAR=default\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "arg_instruction") {
            assert_eq!(support.node_kind(&node), NodeKind::Variable);
        }
    }

    #[test]
    fn node_kind_env_instruction() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\nENV MY_VAR=value\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "env_instruction") {
            assert_eq!(support.node_kind(&node), NodeKind::Variable);
        }
    }

    #[test]
    fn node_kind_run_instruction() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\nRUN echo hello\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "run_instruction") {
            assert_eq!(support.node_kind(&node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_copy_instruction() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\nCOPY src/ /app/\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "copy_instruction") {
            assert_eq!(support.node_kind(&node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_comment() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("# This is a comment\nFROM ubuntu:20.04\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "comment") {
            assert_eq!(support.node_kind(&node), NodeKind::Comment);
        }
    }

    #[test]
    fn node_kind_unknown() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\n");
        // root node is "source_file" -> Unknown
        assert_eq!(support.node_kind(&tree.root_node()), NodeKind::Unknown);
    }

    #[test]
    fn node_kind_expose_instruction() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\nEXPOSE 8080\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "expose_instruction") {
            assert_eq!(support.node_kind(&node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_cmd_instruction() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\nCMD [\"node\", \"app.js\"]\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "cmd_instruction") {
            assert_eq!(support.node_kind(&node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_workdir_instruction() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\nWORKDIR /app\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "workdir_instruction") {
            assert_eq!(support.node_kind(&node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_label_instruction() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\nLABEL version=\"1.0\"\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "label_instruction") {
            assert_eq!(support.node_kind(&node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_user_instruction() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\nUSER nonroot\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "user_instruction") {
            assert_eq!(support.node_kind(&node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_entrypoint_instruction() {
        let support = DockerfileSupport;
        let tree = parse_dockerfile("FROM ubuntu:20.04\nENTRYPOINT [\"node\"]\n");
        if let Some(node) = find_node_by_kind(tree.root_node(), "entrypoint_instruction") {
            assert_eq!(support.node_kind(&node), NodeKind::Call);
        }
    }

    #[test]
    fn function_parent_kinds_contains_from() {
        let support = DockerfileSupport;
        assert!(
            support
                .function_parent_kinds()
                .contains(&"from_instruction")
        );
    }

    #[test]
    fn function_body_kind_is_instruction() {
        let support = DockerfileSupport;
        assert_eq!(support.function_body_kind(), "instruction");
    }

    #[test]
    fn decision_point_kinds_empty() {
        let support = DockerfileSupport;
        assert!(support.decision_point_kinds().is_empty());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::query::{QueryResult, execute_skeleton};
        use crate::{Grammar, SourceFile};
        use std::path::PathBuf;
        let source = SourceFile::from_content(
            PathBuf::from("Dockerfile"),
            "FROM ubuntu:22.04\nARG VERSION=1.0\nENV APP_HOME=/app\n".to_string(),
            Grammar::Dockerfile,
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
            PathBuf::from("Dockerfile"),
            "ARG VERSION=1.0\nENV APP_HOME=/app\n".to_string(),
            Grammar::Dockerfile,
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
