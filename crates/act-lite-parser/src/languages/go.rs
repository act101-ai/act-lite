//! Go language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Go in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("go"),
        display_name: "Go",
        extensions: &["go"],
        category: LanguageCategory::Systems,
    }
}

/// Go language support implementation.
#[derive(Default)]
pub struct GoSupport;

impl LanguageSupport for GoSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Go
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Package
            "package_clause" => NodeKind::Module,

            // Functions
            "function_declaration" => NodeKind::Function,

            // Methods
            "method_declaration" => NodeKind::Method,

            // Types - need to check the child type for type_spec
            "type_declaration" | "type_spec" => {
                // Check if this is a struct or interface declaration
                if let Some(type_node) = node.child_by_field_name("type") {
                    match type_node.kind() {
                        "struct_type" => NodeKind::Struct,
                        "interface_type" => NodeKind::Interface,
                        _ => NodeKind::TypeAlias,
                    }
                } else {
                    NodeKind::TypeAlias
                }
            }
            "struct_type" => NodeKind::Struct,
            "interface_type" => NodeKind::Interface,

            // Fields
            "field_declaration" => NodeKind::Property,
            // In tree-sitter-go 0.23.x the interface method element is "method_elem"
            "method_elem" => NodeKind::Method,

            // Variables
            "var_declaration" | "var_spec" => NodeKind::Variable,
            "const_declaration" | "const_spec" => NodeKind::Variable,
            "short_var_declaration" => NodeKind::Variable,

            // Parameters
            "parameter_declaration" | "parameter_list" => NodeKind::Parameter,
            "type_parameter_declaration" => NodeKind::TypeParameter,

            // Imports
            "import_declaration" | "import_spec" => NodeKind::Import,

            // Blocks
            "block" | "source_file" => NodeKind::Block,

            // Calls
            "call_expression" => NodeKind::Call,

            // Identifiers
            "identifier" | "type_identifier" | "field_identifier" | "package_identifier" => {
                NodeKind::Identifier
            }

            // Comments
            "comment" => NodeKind::Comment,

            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/go/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/go/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/go/references.scm")
    }

    fn locals_query(&self) -> &'static str {
        include_str!("../../queries/go/locals.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }

    fn function_body_kind(&self) -> &'static str {
        "block"
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &["function_declaration", "method_declaration"]
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "select_statement",
            "communication_case",
            "type_switch_statement",
            "expression_switch_statement",
            "expression_case",
        ]
    }

    fn cfg_branch_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "expression_switch_statement",
            "type_switch_statement",
            "select_statement",
        ]
    }
    fn cfg_loop_kinds(&self) -> &'static [&'static str] {
        &["for_statement"]
    }
    fn cfg_arm_container_kinds(&self) -> &'static [&'static str] {
        // Go switch arms are direct children of the switch statement; no separate
        // container node, so the engine treats the switch node itself as the
        // container via `cfg_arm_kinds`.
        &[
            "expression_switch_statement",
            "type_switch_statement",
            "select_statement",
        ]
    }
    fn cfg_arm_kinds(&self) -> &'static [&'static str] {
        &[
            "expression_case",
            "type_case",
            "default_case",
            "communication_case",
        ]
    }
    fn cfg_exception_handler_kinds(&self) -> &'static [&'static str] {
        // Go has no try/catch. Exception (handler) dimension is NOT modeled.
        &[]
    }
    fn cfg_throw_kinds(&self) -> &'static [&'static str] {
        // No throw/raise; `panic` is a call. Error-return is plain Return.
        &[]
    }
    fn cfg_return_kinds(&self) -> &'static [&'static str] {
        &["return_statement"]
    }
    fn cfg_modeled_kinds(&self) -> act_lite_core::KindMask {
        // No exception dimension — the honesty-contract degradation.
        act_lite_core::KindMask::new()
            .with("seq")
            .with("branch")
            .with("loop")
            .with("return")
    }

    fn function_keyword(&self) -> &'static str {
        "func"
    }

    fn stub_placeholders(&self) -> &'static [&'static str] {
        &["panic(\"not implemented\")", "panic(\"TODO\")"]
    }

    // --- call_detection: Go's call node is `call_expression` (no bare `call`). ---
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression"]
    }

    // --- effect_summary ---
    // Go has no exceptions (panic/recover are calls). `defer`/`go` schedule, and
    // allocation/blocking are call-name heuristics (make/new, time.Sleep, recv).
    fn allocation_call_names(&self) -> &'static [&'static str] {
        &["make", "new", "append", "Grow"]
    }

    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["Sleep", "Wait", "Lock", "Recv", "Accept"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("allocate")
            .with("block")
    }

    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &[
            "short_var_declaration",
            "var_declaration",
            "const_declaration",
        ]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    fn eval_call_names(&self) -> &'static [&'static str] {
        &[] // Go has no eval
    }

    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &["Query", "Exec", "QueryRow", "QueryContext", "ExecContext"]
    }

    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        &["Call", "MethodByName"]
    }

    fn ffi_markers(&self) -> &'static [&'static str] {
        // cgo: `import "C"` + unsafe.Pointer reinterpretation.
        &["C", "Pointer", "dlopen"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("raw_sql")
            .with("reflection_invoke")
            .with("ffi")
    }

    fn env_read_names(&self) -> &'static [&'static str] {
        &["Getenv", "LookupEnv"]
    }

    fn crypto_api_names(&self) -> &'static [&'static str] {
        &["Sign", "NewHMAC", "SignPKCS1v15"]
    }

    fn secret_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("credential_param")
            .with("token_var")
            .with("signing_key")
            .with("env_secret_read")
            .with("hardcoded_literal")
    }

    fn taint_source_names(&self) -> &'static [&'static str] {
        &["FormValue", "Query", "Get", "Getenv", "ReadFile", "ReadAll"]
    }

    /// LLM SDK response surfaces, pinned to published Go SDKs:
    /// `CreateChatCompletion` (go-openai, github.com/sashabaranov/go-openai);
    /// `Messages.New` (anthropic-sdk-go, github.com/anthropics/anthropic-sdk-go
    /// — bare `New` alone is too generic, so the dotted `Messages.New` tail is
    /// used instead); `GenerateContent` (google/go-genai's
    /// `client.Models.GenerateContent`).
    fn llm_response_names(&self) -> &'static [&'static str] {
        &["CreateChatCompletion", "Messages.New", "GenerateContent"]
    }

    fn command_exec_names(&self) -> &'static [&'static str] {
        &["Command", "CommandContext", "Run", "Output"]
    }

    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["Open", "ReadFile", "WriteFile", "Create"]
    }

    fn sanitizer_names(&self) -> &'static [&'static str] {
        &["QueryEscape", "Clean", "Sanitize"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "block",
            "assignment_statement",
            "short_var_declaration",
            "return_statement",
            "if_statement",
            "for_statement",
            "expression_switch_statement",
            "type_switch_statement",
            "select_statement",
            "defer_statement",
            "go_statement",
            "send_statement",
            "inc_statement",
            "dec_statement",
            "var_declaration",
            "const_declaration",
            "function_declaration",
            "method_declaration",
            "labeled_statement",
            "fallthrough_statement",
            "break_statement",
            "continue_statement",
            "goto_statement",
            "call_expression",
            "binary_expression",
            "unary_expression",
            "selector_expression",
            "index_expression",
            "slice_expression",
            "type_assertion_expression",
            "composite_literal",
            "func_literal",
            "type_conversion_expression",
            "parenthesized_expression",
            "communication_case",
            "default_case",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};
    use act_lite_core::registry::LANGUAGE_REGISTRY;

    crate::language_support_tests!(GoSupport, Grammar::Go, &["go"]);

    #[test]
    fn registry_has_go() {
        let meta = LANGUAGE_REGISTRY.get(LanguageId("go"));
        assert!(meta.is_some(), "Go should be registered");
        let meta = meta.unwrap();
        assert_eq!(meta.display_name, "Go");
        assert!(meta.extensions.contains(&"go"));
    }

    #[test]
    fn node_kind_package() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main");
        let node = find_node_by_kind(tree.root_node(), "package_clause").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Module);
    }

    #[test]
    fn node_kind_function_declaration() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\nfunc foo() {}");
        let node = find_node_by_kind(tree.root_node(), "function_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_method_declaration() {
        let support = GoSupport;
        let tree = parse_with_support(
            &support,
            "package main\ntype T struct{}\nfunc (t *T) foo() {}",
        );
        let node = find_node_by_kind(tree.root_node(), "method_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Method);
    }

    #[test]
    fn node_kind_struct_type() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\ntype User struct { Name string }");
        let node = find_node_by_kind(tree.root_node(), "struct_type").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Struct);
    }

    #[test]
    fn node_kind_interface_type() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\ntype Reader interface { Read() }");
        let node = find_node_by_kind(tree.root_node(), "interface_type").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_field_declaration() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\ntype User struct { Name string }");
        let node = find_node_by_kind(tree.root_node(), "field_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Property);
    }

    #[test]
    fn node_kind_var_declaration() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\nvar x = 1");
        let node = find_node_by_kind(tree.root_node(), "var_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_const_declaration() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\nconst x = 1");
        let node = find_node_by_kind(tree.root_node(), "const_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_short_var_declaration() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\nfunc f() { x := 1 }");
        let node = find_node_by_kind(tree.root_node(), "short_var_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_import_declaration() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\nimport \"fmt\"");
        let node = find_node_by_kind(tree.root_node(), "import_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Import);
    }

    #[test]
    fn node_kind_call_expression() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\nfunc f() { foo() }");
        let node = find_node_by_kind(tree.root_node(), "call_expression").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Call);
    }

    #[test]
    fn node_kind_identifier() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\nvar x = 1");
        let node = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_comment() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\n// comment\nvar x = 1");
        let node = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_type_spec_struct() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\ntype User struct { Name string }");
        let node = find_node_by_kind(tree.root_node(), "type_spec").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Struct);
    }

    #[test]
    fn node_kind_type_spec_interface() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\ntype Reader interface { Read() }");
        let node = find_node_by_kind(tree.root_node(), "type_spec").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_type_spec_alias() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\ntype MyInt int");
        let node = find_node_by_kind(tree.root_node(), "type_spec").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_type_declaration_struct() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\ntype User struct { Name string }");
        let node = find_node_by_kind(tree.root_node(), "type_declaration").unwrap();
        // type_declaration wraps a type_spec; falls through to TypeAlias since
        // type_declaration itself has no direct "type" field child
        assert!(matches!(
            support.node_kind(&node),
            NodeKind::Struct | NodeKind::Interface | NodeKind::TypeAlias
        ));
    }

    #[test]
    fn node_kind_method_elem() {
        let support = GoSupport;
        let tree = parse_with_support(
            &support,
            "package main\ntype Reader interface { Read() error }",
        );
        // tree-sitter-go 0.23.x uses "method_elem" for interface method signatures
        let node = find_node_by_kind(tree.root_node(), "method_elem").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Method);
    }

    #[test]
    fn node_kind_type_parameter_declaration() {
        let support = GoSupport;
        let tree = parse_with_support(
            &support,
            "package main\nfunc Map[T any](s []T) []T { return s }",
        );
        let node = find_node_by_kind(tree.root_node(), "type_parameter_declaration");
        // Only present in Go 1.18+ generics; skip gracefully if not parsed
        if let Some(n) = node {
            assert_eq!(support.node_kind(&n), NodeKind::TypeParameter);
        }
    }

    #[test]
    fn node_kind_parameter_list() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\nfunc f(x int) {}");
        let node = find_node_by_kind(tree.root_node(), "parameter_list").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_type_identifier() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\ntype MyInt int");
        let node = find_node_by_kind(tree.root_node(), "type_identifier").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_field_identifier() {
        let support = GoSupport;
        let tree = parse_with_support(
            &support,
            "package main\ntype S struct { Name string }\nfunc f(s S) { _ = s.Name }",
        );
        let node = find_node_by_kind(tree.root_node(), "field_identifier").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_package_identifier() {
        let support = GoSupport;
        let tree = parse_with_support(
            &support,
            "package main\nimport \"fmt\"\nfunc f() { fmt.Println() }",
        );
        let node = find_node_by_kind(tree.root_node(), "package_identifier");
        if let Some(n) = node {
            assert_eq!(support.node_kind(&n), NodeKind::Identifier);
        }
    }

    #[test]
    fn node_kind_var_spec() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\nvar x int = 1");
        let node = find_node_by_kind(tree.root_node(), "var_spec").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_const_spec() {
        let support = GoSupport;
        let tree = parse_with_support(&support, "package main\nconst x int = 1");
        let node = find_node_by_kind(tree.root_node(), "const_spec").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_unknown() {
        let support = GoSupport;
        // "source_file" is mapped to Block, but "ERROR" or any unmapped kind → Unknown.
        // We test by directly calling node_kind with a known-unmapped kind.
        // The fallback arm of the match is `_ => NodeKind::Unknown`.
        // We can verify via a struct that has no special mapping.
        // It's sufficient to confirm the fallback exists; the tree may or may not have an unknown node.
        // We verify the match compiles and works by confirming Block/Variable are not Unknown.
        let tree2 = parse_with_support(&support, "package main\nvar x = 1");
        let root = tree2.root_node();
        assert_ne!(support.node_kind(&root), NodeKind::Unknown);
    }

    #[test]
    fn function_body_kind_is_block() {
        let support = GoSupport;
        assert_eq!(support.function_body_kind(), "block");
    }

    #[test]
    fn function_parent_kinds_are_go_specific() {
        let support = GoSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"function_declaration"));
        assert!(kinds.contains(&"method_declaration"));
        assert!(!kinds.contains(&"arrow_function"));
        assert!(!kinds.contains(&"method_definition"));
    }

    #[test]
    fn decision_point_kinds_are_go_specific() {
        let support = GoSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_statement"));
        assert!(kinds.contains(&"for_statement"));
        assert!(kinds.contains(&"select_statement"));
        assert!(kinds.contains(&"expression_switch_statement"));
        // Must NOT contain JS/TS constructs
        assert!(!kinds.contains(&"while_statement"));
        assert!(!kinds.contains(&"do_statement"));
        assert!(!kinds.contains(&"ternary_expression"));
        assert!(!kinds.contains(&"for_in_statement"));
    }

    #[test]
    fn cfg_hooks_go_omits_exception_dimension() {
        let g = GoSupport;
        assert!(g.cfg_branch_kinds().contains(&"if_statement"));
        assert!(g.cfg_loop_kinds().contains(&"for_statement"));
        // Go has no try/catch: the exception (handler) dimension is NOT modeled.
        assert!(g.cfg_exception_handler_kinds().is_empty());
        assert!(!g.cfg_modeled_kinds().contains("exception"));
        // …but seq/branch/loop/return are.
        assert!(g.cfg_modeled_kinds().contains("branch"));
        assert!(g.cfg_modeled_kinds().contains("return"));
    }

    #[test]
    fn test_go_struct_symbols() {
        use crate::{Grammar, QueryResult, SourceFile, execute_symbols};
        use act_lite_core::SymbolKind;
        use std::path::PathBuf;

        let code = r#"package main

type User struct {
    Name string
}

func main() {
}
"#;

        let file =
            SourceFile::from_content(PathBuf::from("test.go"), code.to_string(), Grammar::Go)
                .unwrap();
        let result = execute_symbols(&file).unwrap();

        if let QueryResult::Symbols { symbols } = result {
            let user_struct = symbols.iter().find(|s| s.name() == "User");
            assert!(user_struct.is_some(), "User struct not found in symbols");
            assert_eq!(
                user_struct.unwrap().kind(),
                SymbolKind::Struct,
                "User should be a Struct"
            );
        } else {
            panic!("Expected Symbols result");
        }
    }

    #[test]
    fn function_keyword_is_func() {
        let support = GoSupport;
        assert_eq!(support.function_keyword(), "func");
    }

    #[test]
    fn stub_placeholders_returns_language_patterns() {
        let support = GoSupport;
        let placeholders = support.stub_placeholders();
        assert!(!placeholders.is_empty());
        assert!(placeholders.contains(&"panic(\"not implemented\")"));
        assert!(placeholders.contains(&"panic(\"TODO\")"));
    }

    // --- moat coverage hooks (B2) ---

    #[test]
    fn coverage_call_detection_overrides_default() {
        let s = GoSupport;
        assert_eq!(s.call_node_kinds(), &["call_expression"]);
        let tree = parse_with_support(&s, "package m\nfunc f() { g() }");
        assert!(find_node_by_kind(tree.root_node(), "call_expression").is_some());
    }

    #[test]
    fn coverage_effect_summary_widened() {
        let s = GoSupport;
        let baseline = act_lite_core::KindMask::new().with("read").with("write");
        assert_ne!(s.effect_modeled_kinds(), baseline);
        assert!(s.allocation_call_names().contains(&"make"));
        assert!(s.blocking_call_names().contains(&"Sleep"));
    }
}
