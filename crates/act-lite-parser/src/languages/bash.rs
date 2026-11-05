//! Bash language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Bash in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("bash"),
        display_name: "Bash",
        extensions: &["sh", "bash"],
        category: LanguageCategory::Scripting,
    }
}

/// Bash language support implementation.
#[derive(Default)]
pub struct BashSupport;

impl LanguageSupport for BashSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Bash
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_bash::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "function_definition" => NodeKind::Function,
            "variable_assignment" | "declaration_command" => NodeKind::Variable,
            "command" | "pipeline" => NodeKind::Call,
            "subshell" | "compound_statement" => NodeKind::Block,
            "comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["sh", "bash"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/bash/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/bash/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/bash/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
            block_start: None,
            block_end: None,
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }

    fn function_body_kind(&self) -> &'static str {
        "compound_statement"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "while_statement",
            "until_statement",
            "for_statement",
            "case_statement",
        ]
    }

    // --- Coverage-closure moat hooks (B6: real shell — all six ops apply) ---

    /// Bash invocations: simple `command` nodes and `pipeline`s. Overrides the
    /// `call_expression`/`call` default so the call frontier sees shell commands.
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["command", "pipeline"]
    }

    /// Synchronous blockers in shell: `sleep`, `wait` (on a job/PID), and `read`
    /// (blocks on stdin).
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["sleep", "wait", "read"]
    }

    /// Beyond read/write: shell commands routinely block (sleep/wait/read).
    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("block")
    }

    /// `VAR=...` and `declare`/`local`/`readonly` bind variables.
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["variable_assignment", "declaration_command"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    /// Dynamic code execution: `eval`, and sourcing another script with
    /// `source` / `.` (runs arbitrary commands in the current shell).
    fn eval_call_names(&self) -> &'static [&'static str] {
        &["eval", "source", "."]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("eval")
    }

    /// Environment-secret reads: `printenv`, `env`, `getenv`-style lookups.
    fn env_read_names(&self) -> &'static [&'static str] {
        &["printenv", "env", "getenv"]
    }

    /// Crypto tooling commonly invoked from shell to handle keys/secrets.
    fn crypto_api_names(&self) -> &'static [&'static str] {
        &["openssl", "gpg", "gpg2", "ssh-keygen", "age"]
    }

    fn secret_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("env_secret_read")
            .with("signing_key")
    }

    /// Taint sources: `read` (stdin), positional/env via `getenv`/`printenv`.
    fn taint_source_names(&self) -> &'static [&'static str] {
        &["read", "getenv", "printenv", "env"]
    }

    /// Command-injection sinks: `eval`, `exec`, sourcing, and re-invoking a shell.
    fn command_exec_names(&self) -> &'static [&'static str] {
        &["eval", "exec", "source", ".", "bash", "sh", "system"]
    }

    /// Path/filesystem sinks where tainted data lands.
    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["cp", "mv", "rm", "dd", "tee", "mkdir", "touch"]
    }

    /// Quoting/escaping that clears taint.
    fn sanitizer_names(&self) -> &'static [&'static str] {
        &["printf", "quote", "shquote"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::parse_with_support;
    use act_lite_core::registry::{LANGUAGE_REGISTRY, LanguageId};
    use streaming_iterator::StreamingIterator;

    crate::language_support_tests!(BashSupport, Grammar::Bash, &["sh", "bash"]);

    #[test]
    fn registry_has_bash() {
        let meta = LANGUAGE_REGISTRY.get(LanguageId("bash"));
        assert!(meta.is_some(), "Bash should be registered");
    }

    #[test]
    fn registry_extension_lookup_bash() {
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("sh"),
            Some(LanguageId("bash"))
        );
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("bash"),
            Some(LanguageId("bash"))
        );
    }

    #[test]
    fn node_kind_unknown_for_unrecognized() {
        let support = BashSupport;
        let tree = parse_with_support(&support, "echo hello");
        let root = tree.root_node();
        assert_eq!(support.node_kind(&root), NodeKind::Unknown);
    }

    #[test]
    fn function_parent_kinds_contains_function_definition() {
        let support = BashSupport;
        assert!(
            support
                .function_parent_kinds()
                .contains(&"function_definition")
        );
    }

    #[test]
    fn function_body_kind_is_compound_statement() {
        let support = BashSupport;
        assert_eq!(support.function_body_kind(), "compound_statement");
    }

    #[test]
    fn decision_point_kinds_contains_bash_constructs() {
        let support = BashSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_statement"));
        assert!(kinds.contains(&"while_statement"));
        assert!(kinds.contains(&"until_statement"));
        assert!(kinds.contains(&"for_statement"));
        assert!(kinds.contains(&"case_statement"));
    }

    fn parse_bash(code: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_bash::LANGUAGE.into())
            .unwrap();
        parser.parse(code, None).unwrap()
    }

    #[test]
    fn node_kind_function() {
        let support = BashSupport;
        let tree = parse_bash("greet() { echo hello; }");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_function_keyword_syntax() {
        let support = BashSupport;
        let tree = parse_bash("function greet { echo hello; }");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_variable_assignment() {
        let support = BashSupport;
        let tree = parse_bash("MY_VAR=hello");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_declaration_command() {
        let support = BashSupport;
        let tree = parse_bash("declare -i VAR=0");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_command() {
        let support = BashSupport;
        let tree = parse_bash("echo hello");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Call);
    }

    #[test]
    fn node_kind_pipeline() {
        let support = BashSupport;
        let tree = parse_bash("ls | grep foo");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Call);
    }

    #[test]
    fn node_kind_subshell() {
        let support = BashSupport;
        let tree = parse_bash("(echo hello)");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_compound_statement() {
        let support = BashSupport;
        let tree = parse_bash("{ echo a; }");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_comment() {
        let support = BashSupport;
        let tree = parse_bash("# this is a comment");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_if_statement_is_unknown() {
        let support = BashSupport;
        let tree = parse_bash("if true; then echo yes; fi");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Unknown);
    }

    /// Returns true if any match contains a capture with the given name.
    fn query_has_capture(
        query: &tree_sitter::Query,
        tree: &tree_sitter::Tree,
        source: &[u8],
        capture_name: &str,
    ) -> bool {
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source);
        while let Some(m) = matches.next() {
            for cap in m.captures {
                if query.capture_names()[cap.index as usize] == capture_name {
                    return true;
                }
            }
        }
        false
    }

    /// Returns true if any match exists at all.
    fn query_has_match(
        query: &tree_sitter::Query,
        tree: &tree_sitter::Tree,
        source: &[u8],
    ) -> bool {
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source);
        matches.next().is_some()
    }

    #[test]
    fn skeleton_query_captures_function() {
        let support = BashSupport;
        let code = "deploy() { echo deploying; }";
        let tree = parse_bash(code);
        let query =
            tree_sitter::Query::new(&support.ts_language(), support.skeleton_query()).unwrap();
        assert!(
            query_has_capture(&query, &tree, code.as_bytes(), "function"),
            "skeleton query should capture function definitions"
        );
    }

    #[test]
    fn skeleton_query_captures_function_with_keyword() {
        let support = BashSupport;
        let code = "function deploy { echo deploying; }";
        let tree = parse_bash(code);
        let query =
            tree_sitter::Query::new(&support.ts_language(), support.skeleton_query()).unwrap();
        assert!(
            query_has_capture(&query, &tree, code.as_bytes(), "function"),
            "skeleton query should capture function definitions using function keyword"
        );
    }

    #[test]
    fn skeleton_query_captures_variable() {
        let support = BashSupport;
        let code = "MY_VAR=hello";
        let tree = parse_bash(code);
        let query =
            tree_sitter::Query::new(&support.ts_language(), support.skeleton_query()).unwrap();
        assert!(
            query_has_capture(&query, &tree, code.as_bytes(), "variable"),
            "skeleton query should capture variable assignments"
        );
    }

    #[test]
    fn skeleton_query_captures_source_import() {
        let support = BashSupport;
        let code = "source ./lib.sh";
        let tree = parse_bash(code);
        let query =
            tree_sitter::Query::new(&support.ts_language(), support.skeleton_query()).unwrap();
        assert!(
            query_has_capture(&query, &tree, code.as_bytes(), "import"),
            "skeleton query should capture source directives as imports"
        );
    }

    #[test]
    fn skeleton_query_captures_dot_import() {
        let support = BashSupport;
        let code = ". ./lib.sh";
        let tree = parse_bash(code);
        let query =
            tree_sitter::Query::new(&support.ts_language(), support.skeleton_query()).unwrap();
        assert!(
            query_has_capture(&query, &tree, code.as_bytes(), "import"),
            "skeleton query should capture dot-source directives as imports"
        );
    }

    #[test]
    fn symbols_query_captures_function_definition() {
        let support = BashSupport;
        let code = "greet() { echo hello; }";
        let tree = parse_bash(code);
        let query =
            tree_sitter::Query::new(&support.ts_language(), support.symbols_query()).unwrap();
        assert!(
            query_has_match(&query, &tree, code.as_bytes()),
            "symbols query should match function definition"
        );
    }

    #[test]
    fn symbols_query_captures_variable_assignment() {
        let support = BashSupport;
        let code = "COUNT=42";
        let tree = parse_bash(code);
        let query =
            tree_sitter::Query::new(&support.ts_language(), support.symbols_query()).unwrap();
        assert!(
            query_has_match(&query, &tree, code.as_bytes()),
            "symbols query should match variable assignment"
        );
    }

    #[test]
    fn symbols_query_captures_for_variable() {
        let support = BashSupport;
        let code = "for i in 1 2 3; do echo $i; done";
        let tree = parse_bash(code);
        let query =
            tree_sitter::Query::new(&support.ts_language(), support.symbols_query()).unwrap();
        assert!(
            query_has_match(&query, &tree, code.as_bytes()),
            "symbols query should match for loop variable"
        );
    }

    #[test]
    fn references_query_captures_simple_expansion() {
        let support = BashSupport;
        let code = "echo $MY_VAR";
        let tree = parse_bash(code);
        let query =
            tree_sitter::Query::new(&support.ts_language(), support.references_query()).unwrap();
        assert!(
            query_has_match(&query, &tree, code.as_bytes()),
            "references query should match simple variable expansion"
        );
    }

    #[test]
    fn references_query_captures_brace_expansion() {
        let support = BashSupport;
        let code = "echo ${MY_VAR:-default}";
        let tree = parse_bash(code);
        let query =
            tree_sitter::Query::new(&support.ts_language(), support.references_query()).unwrap();
        assert!(
            query_has_match(&query, &tree, code.as_bytes()),
            "references query should match brace expansion"
        );
    }

    #[test]
    fn references_query_captures_command_name() {
        let support = BashSupport;
        let code = "deploy_app";
        let tree = parse_bash(code);
        let query =
            tree_sitter::Query::new(&support.ts_language(), support.references_query()).unwrap();
        assert!(
            query_has_match(&query, &tree, code.as_bytes()),
            "references query should match command name as reference"
        );
    }

    // --- Coverage-closure moat hook tests ---

    #[test]
    fn moat_call_node_kinds_are_shell_commands() {
        let s = BashSupport;
        assert!(s.call_node_kinds().contains(&"command"));
        assert!(s.call_node_kinds().contains(&"pipeline"));
        assert_ne!(s.call_node_kinds(), &["call_expression", "call"]);
    }

    #[test]
    fn moat_effect_models_blocking() {
        let s = BashSupport;
        assert!(s.blocking_call_names().contains(&"sleep"));
        assert!(s.effect_modeled_kinds().contains("block"));
    }

    #[test]
    fn moat_data_flow_binds_assignments() {
        let s = BashSupport;
        assert!(s.binding_node_kinds().contains(&"variable_assignment"));
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
    }

    #[test]
    fn moat_unsafe_models_eval_and_source() {
        let s = BashSupport;
        assert!(s.eval_call_names().contains(&"eval"));
        assert!(s.eval_call_names().contains(&"source"));
        assert!(s.unsafe_modeled_kinds().contains("eval"));
    }

    #[test]
    fn moat_secret_models_env_and_crypto() {
        let s = BashSupport;
        assert!(s.env_read_names().contains(&"printenv"));
        assert!(s.crypto_api_names().contains(&"openssl"));
        assert!(s.secret_modeled_kinds().contains("env_secret_read"));
    }

    #[test]
    fn moat_taint_models_sources_and_sinks() {
        let s = BashSupport;
        assert!(s.taint_source_names().contains(&"read"));
        assert!(s.command_exec_names().contains(&"eval"));
        assert!(s.fs_sink_names().contains(&"rm"));
        assert!(s.taint_modeled_kinds().contains("taint"));
    }
}
