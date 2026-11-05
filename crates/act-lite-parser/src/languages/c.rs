//! C language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register C in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("c"),
        display_name: "C",
        extensions: &["c", "h"],
        category: LanguageCategory::Systems,
    }
}

/// C language support implementation.
#[derive(Default)]
pub struct CSupport;

impl LanguageSupport for CSupport {
    fn grammar(&self) -> Grammar {
        Grammar::C
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_c::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Functions
            "function_definition" => NodeKind::Function,
            "function_declarator" => NodeKind::Function,

            // Types
            "struct_specifier" => NodeKind::Struct,
            "union_specifier" => NodeKind::Struct, // Treat union as struct-like
            "enum_specifier" => NodeKind::Enum,
            "type_definition" => NodeKind::TypeAlias,

            // Variables and fields
            "declaration" => self.classify_declaration(node),
            "field_declaration" => NodeKind::Property,
            "init_declarator" => NodeKind::Variable,
            "enumerator" => NodeKind::Property,

            // Parameters
            "parameter_declaration" => NodeKind::Parameter,
            "parameter_list" => NodeKind::Parameter,

            // Preprocessor
            "preproc_include" => NodeKind::Import,
            "preproc_def" | "preproc_function_def" => NodeKind::Variable,
            "preproc_ifdef" | "preproc_if" | "preproc_else" => NodeKind::Block,

            // Statements and blocks
            "compound_statement" => NodeKind::Block,
            "if_statement" | "switch_statement" => NodeKind::Block,
            "for_statement" | "while_statement" | "do_statement" => NodeKind::Block,
            "case_statement" => NodeKind::Block,

            // Expressions
            "call_expression" => NodeKind::Call,
            "binary_expression" | "unary_expression" => NodeKind::Unknown,
            "assignment_expression" => NodeKind::Unknown,
            "conditional_expression" => NodeKind::Unknown,

            // Identifiers
            "identifier" => NodeKind::Identifier,
            "type_identifier" => NodeKind::Type,
            "field_identifier" => NodeKind::Identifier,

            // Comments
            "comment" => NodeKind::Comment,

            // Pointers and arrays
            "pointer_declarator" => NodeKind::Variable,
            "array_declarator" => NodeKind::Variable,

            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["c", "h"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/c/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/c/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/c/references.scm")
    }

    fn locals_query(&self) -> &'static str {
        include_str!("../../queries/c/locals.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
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
            "switch_statement",
            "for_statement",
            "while_statement",
            "do_statement",
            "case_statement",
        ]
    }

    fn function_keyword(&self) -> &'static str {
        "" // C has no function keyword; functions begin with their return type
    }

    fn logical_operator_info(&self) -> Option<(&'static str, &'static [&'static str])> {
        // Explicitly override: tree-sitter C uses binary_expression with && / ||
        // Same as the default, but explicit so future default changes don't silently break C.
        Some(("binary_expression", &["&&", "||"]))
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "compound_statement",
            "if_statement",
            "for_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "case_statement",
        ]
    }

    // --- call_detection: C's only call node is `call_expression` (no bare `call`). ---
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression"]
    }

    // --- effect_summary ---
    // C has no exceptions; effects are allocation (malloc/calloc/realloc, fopen,
    // open) and synchronous blocking syscalls (sleep, recv, read, accept).
    fn allocation_call_names(&self) -> &'static [&'static str] {
        &[
            "malloc",
            "calloc",
            "realloc",
            "aligned_alloc",
            "strdup",
            "fopen",
            "open",
        ]
    }

    fn blocking_call_names(&self) -> &'static [&'static str] {
        &[
            "sleep",
            "usleep",
            "nanosleep",
            "recv",
            "recvfrom",
            "read",
            "accept",
        ]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("allocate")
            .with("block")
    }

    // --- data_flow: locals are `declaration`/`init_declarator`. ---
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["declaration", "init_declarator"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    fn eval_call_names(&self) -> &'static [&'static str] {
        &["system", "popen"]
    }

    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &["mysql_query", "sqlite3_exec", "PQexec"]
    }

    // FFI: dynamic loading + symbol resolution are the C foreign-function surface.
    fn ffi_markers(&self) -> &'static [&'static str] {
        &["dlopen", "dlsym", "LoadLibrary", "GetProcAddress"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("eval")
            .with("raw_sql")
            .with("ffi")
    }

    fn env_read_names(&self) -> &'static [&'static str] {
        &["getenv", "secure_getenv"]
    }

    fn crypto_api_names(&self) -> &'static [&'static str] {
        // OpenSSL EVP/HMAC/SHA signing + digest entry points.
        &[
            "EVP_DigestSign",
            "EVP_DigestSignInit",
            "HMAC",
            "SHA256",
            "SHA512",
        ]
    }

    fn secret_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("credential_param")
            .with("token_var")
            .with("signing_key")
            .with("env_secret_read")
            .with("hardcoded_literal")
    }

    // --- taint_flow ---
    fn taint_source_names(&self) -> &'static [&'static str] {
        &["getenv", "scanf", "fscanf", "gets", "fgets", "read", "recv"]
    }

    fn command_exec_names(&self) -> &'static [&'static str] {
        &[
            "system", "popen", "execl", "execlp", "execle", "execv", "execvp", "execve",
        ]
    }

    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["fopen", "open", "openat", "freopen", "fwrite", "write"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

impl CSupport {
    /// Classify a declaration based on context.
    fn classify_declaration(&self, node: &tree_sitter::Node) -> NodeKind {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_declarator" => return NodeKind::Function,
                "init_declarator" => return NodeKind::Variable,
                _ => {}
            }
        }
        NodeKind::Variable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};

    crate::language_support_tests!(CSupport, Grammar::C, &["c", "h"]);

    #[test]
    fn node_kind_function_definition() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int foo() { return 0; }");
        let func_node = find_node_by_kind(tree.root_node(), "function_definition").unwrap();
        assert_eq!(support.node_kind(&func_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_struct_specifier() {
        let support = CSupport;
        let tree = parse_with_support(&support, "struct Point { int x; int y; };");
        let struct_node = find_node_by_kind(tree.root_node(), "struct_specifier").unwrap();
        assert_eq!(support.node_kind(&struct_node), NodeKind::Struct);
    }

    #[test]
    fn node_kind_enum_specifier() {
        let support = CSupport;
        let tree = parse_with_support(&support, "enum Status { OK, ERROR };");
        let enum_node = find_node_by_kind(tree.root_node(), "enum_specifier").unwrap();
        assert_eq!(support.node_kind(&enum_node), NodeKind::Enum);
    }

    #[test]
    fn node_kind_preproc_include() {
        let support = CSupport;
        let tree = parse_with_support(&support, "#include <stdio.h>");
        let include_node = find_node_by_kind(tree.root_node(), "preproc_include").unwrap();
        assert_eq!(support.node_kind(&include_node), NodeKind::Import);
    }

    #[test]
    fn node_kind_call_expression() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int main() { foo(); return 0; }");
        let call_node = find_node_by_kind(tree.root_node(), "call_expression").unwrap();
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_comment() {
        let support = CSupport;
        let tree = parse_with_support(&support, "/* comment */ int x;");
        let comment_node = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_union_specifier() {
        let support = CSupport;
        let tree = parse_with_support(&support, "union Value { int i; float f; };");
        let union_node = find_node_by_kind(tree.root_node(), "union_specifier").unwrap();
        assert_eq!(support.node_kind(&union_node), NodeKind::Struct);
    }

    #[test]
    fn node_kind_type_definition() {
        let support = CSupport;
        let tree = parse_with_support(&support, "typedef int MyInt;");
        let typedef_node = find_node_by_kind(tree.root_node(), "type_definition").unwrap();
        assert_eq!(support.node_kind(&typedef_node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_field_declaration() {
        let support = CSupport;
        let tree = parse_with_support(&support, "struct Point { int x; int y; };");
        let field_node = find_node_by_kind(tree.root_node(), "field_declaration").unwrap();
        assert_eq!(support.node_kind(&field_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_init_declarator() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int x = 5;");
        let init_node = find_node_by_kind(tree.root_node(), "init_declarator").unwrap();
        assert_eq!(support.node_kind(&init_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_parameter_declaration() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int foo(int x, int y) { return x + y; }");
        let param_node = find_node_by_kind(tree.root_node(), "parameter_declaration").unwrap();
        assert_eq!(support.node_kind(&param_node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_enumerator() {
        let support = CSupport;
        let tree = parse_with_support(&support, "enum Color { RED, GREEN, BLUE };");
        let enum_node = find_node_by_kind(tree.root_node(), "enumerator").unwrap();
        assert_eq!(support.node_kind(&enum_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_identifier() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int x = 5;");
        let id_node = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        assert_eq!(support.node_kind(&id_node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_compound_statement() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int main() { return 0; }");
        let block_node = find_node_by_kind(tree.root_node(), "compound_statement").unwrap();
        assert_eq!(support.node_kind(&block_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_if_statement() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int main() { if (1) return 0; return 1; }");
        let if_node = find_node_by_kind(tree.root_node(), "if_statement").unwrap();
        assert_eq!(support.node_kind(&if_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_for_statement() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int main() { for(;;) {} return 0; }");
        let for_node = find_node_by_kind(tree.root_node(), "for_statement").unwrap();
        assert_eq!(support.node_kind(&for_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_binary_expression() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int x = 1 + 2;");
        let binop_node = find_node_by_kind(tree.root_node(), "binary_expression").unwrap();
        assert_eq!(support.node_kind(&binop_node), NodeKind::Unknown);
    }

    #[test]
    fn node_kind_pointer_declarator() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int* x = NULL;");
        let ptr_node = find_node_by_kind(tree.root_node(), "pointer_declarator").unwrap();
        assert_eq!(support.node_kind(&ptr_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_array_declarator() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int arr[10];");
        let arr_node = find_node_by_kind(tree.root_node(), "array_declarator").unwrap();
        assert_eq!(support.node_kind(&arr_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_preproc_def() {
        let support = CSupport;
        let tree = parse_with_support(&support, "#define FOO 42");
        let def_node = find_node_by_kind(tree.root_node(), "preproc_def").unwrap();
        assert_eq!(support.node_kind(&def_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_function_declarator() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int foo(int x);");
        let func_decl_node = find_node_by_kind(tree.root_node(), "function_declarator").unwrap();
        assert_eq!(support.node_kind(&func_decl_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_declaration_classifies_function() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int foo() { return 0; }");
        let decl_node = find_node_by_kind(tree.root_node(), "declaration");
        if let Some(node) = decl_node {
            assert_eq!(support.node_kind(&node), NodeKind::Function);
        }
    }

    #[test]
    fn node_kind_declaration_classifies_variable() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int x;");
        let decl_node = find_node_by_kind(tree.root_node(), "declaration").unwrap();
        assert_eq!(support.node_kind(&decl_node), NodeKind::Variable);
    }

    #[test]
    fn function_parent_kinds_are_c_specific() {
        let support = CSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"function_definition"));
        // Must NOT contain JS/TS values
        assert!(!kinds.contains(&"arrow_function"));
        assert!(!kinds.contains(&"method_definition"));
    }

    #[test]
    fn function_body_kind_is_compound_statement() {
        let support = CSupport;
        assert_eq!(support.function_body_kind(), "compound_statement");
    }

    #[test]
    fn decision_point_kinds_are_c_specific() {
        let support = CSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_statement"));
        assert!(kinds.contains(&"for_statement"));
        assert!(kinds.contains(&"case_statement"));
        assert!(kinds.contains(&"switch_statement"));
        // Must NOT contain JS/TS only values
        assert!(!kinds.contains(&"ternary_expression"));
        assert!(!kinds.contains(&"switch_case"));
    }

    // C-1
    #[test]
    fn function_keyword_is_empty_for_c() {
        let support = CSupport;
        assert_eq!(
            support.function_keyword(),
            "",
            "C has no function keyword; must return empty string"
        );
    }

    // C-5
    #[test]
    fn logical_operator_info_is_explicit_for_c() {
        let support = CSupport;
        let info = support.logical_operator_info();
        assert!(info.is_some());
        let (kind, ops) = info.unwrap();
        assert_eq!(kind, "binary_expression");
        assert!(ops.contains(&"&&"));
        assert!(ops.contains(&"||"));
    }

    // C-4
    #[test]
    fn extractable_node_kinds_are_defined_for_c() {
        let support = CSupport;
        let kinds = support.extractable_node_kinds();
        assert!(!kinds.is_empty(), "C must define extractable node kinds");
        assert!(kinds.contains(&"compound_statement"));
        assert!(kinds.contains(&"if_statement"));
        assert!(kinds.contains(&"switch_statement"));
    }

    // C-2
    #[test]
    fn classify_declaration_has_no_dead_type_definition_arm() {
        let support = CSupport;
        // typedef int MyInt; — produces type_definition at top level, NOT inside a declaration
        let tree = parse_with_support(&support, "typedef int MyInt;");
        let root = tree.root_node();
        let mut cursor = root.walk();
        let mut found_type_definition = false;
        for child in root.children(&mut cursor) {
            if child.kind() == "type_definition" {
                found_type_definition = true;
            }
            // type_definition is a top-level node, never a child of declaration
            assert_ne!(
                child.kind(),
                "declaration",
                "typedef should produce type_definition node at top level, not a declaration"
            );
        }
        assert!(
            found_type_definition,
            "typedef should produce a top-level type_definition node"
        );
    }

    // C-3
    #[test]
    fn decision_point_kinds_includes_switch_statement() {
        let support = CSupport;
        let kinds = support.decision_point_kinds();
        assert!(
            kinds.contains(&"switch_statement"),
            "switch_statement must be a decision point for cyclomatic complexity"
        );
    }

    #[test]
    fn node_kind_while_statement() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int main() { while (1) {} return 0; }");
        let node = find_node_by_kind(tree.root_node(), "while_statement")
            .expect("while_statement node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_do_statement() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int main() { do {} while (0); return 0; }");
        let node = find_node_by_kind(tree.root_node(), "do_statement")
            .expect("do_statement node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_switch_statement() {
        let support = CSupport;
        let tree = parse_with_support(
            &support,
            "int main() { int x = 1; switch (x) { case 1: break; } return 0; }",
        );
        let node = find_node_by_kind(tree.root_node(), "switch_statement")
            .expect("switch_statement node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_case_statement() {
        let support = CSupport;
        let tree = parse_with_support(
            &support,
            "int main() { int x = 1; switch (x) { case 1: break; } return 0; }",
        );
        let node = find_node_by_kind(tree.root_node(), "case_statement")
            .expect("case_statement node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_preproc_ifdef() {
        let support = CSupport;
        let tree = parse_with_support(&support, "#ifdef FOO\nint x;\n#endif");
        let node = find_node_by_kind(tree.root_node(), "preproc_ifdef")
            .expect("preproc_ifdef node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_preproc_if() {
        let support = CSupport;
        let tree = parse_with_support(&support, "#if 1\nint x;\n#endif");
        let node =
            find_node_by_kind(tree.root_node(), "preproc_if").expect("preproc_if node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_preproc_function_def() {
        let support = CSupport;
        let tree = parse_with_support(&support, "#define MAX(a, b) ((a) > (b) ? (a) : (b))");
        let node = find_node_by_kind(tree.root_node(), "preproc_function_def")
            .expect("preproc_function_def node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_conditional_expression() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int main() { int x = 1 ? 2 : 3; return 0; }");
        let node = find_node_by_kind(tree.root_node(), "conditional_expression")
            .expect("conditional_expression node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Unknown);
    }

    #[test]
    fn node_kind_assignment_expression() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int main() { int x; x = 5; return 0; }");
        let node = find_node_by_kind(tree.root_node(), "assignment_expression")
            .expect("assignment_expression node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Unknown);
    }

    #[test]
    fn node_kind_parameter_list() {
        let support = CSupport;
        let tree = parse_with_support(&support, "int foo(int x, int y) { return x + y; }");
        let node = find_node_by_kind(tree.root_node(), "parameter_list")
            .expect("parameter_list node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_type_identifier() {
        let support = CSupport;
        let tree = parse_with_support(&support, "typedef struct Point Point;");
        let node = find_node_by_kind(tree.root_node(), "type_identifier")
            .expect("type_identifier node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Type);
    }

    #[test]
    fn node_kind_field_identifier() {
        let support = CSupport;
        let tree = parse_with_support(
            &support,
            "int main() { struct { int x; } s; s.x = 1; return 0; }",
        );
        let node = find_node_by_kind(tree.root_node(), "field_identifier")
            .expect("field_identifier node must exist");
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    // --- moat coverage hooks (B2) ---

    #[test]
    fn coverage_call_detection_overrides_default() {
        let s = CSupport;
        // call_expression is C's real call node; `call` (JS/TS default) is absent.
        assert_eq!(s.call_node_kinds(), &["call_expression"]);
        let tree = parse_with_support(&s, "int main() { foo(); return 0; }");
        let call = find_node_by_kind(tree.root_node(), "call_expression").unwrap();
        assert!(s.call_node_kinds().contains(&call.kind()));
    }

    #[test]
    fn coverage_effect_summary_widened() {
        let s = CSupport;
        let baseline = act_lite_core::KindMask::new().with("read").with("write");
        assert_ne!(s.effect_modeled_kinds(), baseline);
        assert!(s.allocation_call_names().contains(&"malloc"));
        assert!(s.blocking_call_names().contains(&"recv"));
    }

    #[test]
    fn coverage_data_flow_models_bindings() {
        let s = CSupport;
        assert!(!s.data_flow_modeled_kinds().is_empty());
        assert!(s.binding_node_kinds().contains(&"declaration"));
        let tree = parse_with_support(&s, "int x = 5;");
        assert!(find_node_by_kind(tree.root_node(), "declaration").is_some());
    }

    #[test]
    fn coverage_unsafe_surface_modeled() {
        let s = CSupport;
        assert!(!s.unsafe_modeled_kinds().is_empty());
        assert!(s.eval_call_names().contains(&"system"));
        assert!(s.ffi_markers().contains(&"dlopen"));
    }

    #[test]
    fn coverage_secret_surface_modeled() {
        let s = CSupport;
        assert!(!s.secret_modeled_kinds().is_empty());
        assert!(s.env_read_names().contains(&"getenv"));
        assert!(s.crypto_api_names().contains(&"HMAC"));
    }

    #[test]
    fn coverage_taint_flow_modeled() {
        let s = CSupport;
        assert!(!s.taint_modeled_kinds().is_empty());
        assert!(s.command_exec_names().contains(&"execvp"));
        assert!(s.fs_sink_names().contains(&"fopen"));
        assert!(s.taint_source_names().contains(&"getenv"));
    }
}
