//! Java language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Java in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("java"),
        display_name: "Java",
        extensions: &["java"],
        category: LanguageCategory::Managed,
    }
}

/// Returns true when a `local_variable_declaration` node has a `final` modifier.
///
/// In the Java grammar, modifiers on a local variable appear either as a
/// `modifiers` child node (when multiple modifiers exist) or as a bare
/// `"final"` keyword child (anonymous node with text "final").
fn has_final_modifier(node: &tree_sitter::Node, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "modifiers" => {
                // modifiers node contains modifier children; look for "final"
                let mut inner = child.walk();
                for modifier in child.children(&mut inner) {
                    if modifier.utf8_text(source).unwrap_or("") == "final" {
                        return true;
                    }
                }
            }
            _ => {
                // Bare anonymous keyword child: text is "final"
                if child.utf8_text(source).unwrap_or("") == "final" {
                    return true;
                }
            }
        }
    }
    false
}

/// Java language support implementation.
#[derive(Default)]
pub struct JavaSupport;

impl LanguageSupport for JavaSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Java
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_java::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Classes and types
            "class_declaration" => NodeKind::Class,
            "interface_declaration" => NodeKind::Interface,
            "enum_declaration" => NodeKind::Enum,
            "record_declaration" => NodeKind::Class, // Records are special classes
            "annotation_type_declaration" => NodeKind::Interface,

            // Methods and constructors
            "method_declaration" => NodeKind::Method,
            "constructor_declaration" => NodeKind::Constructor,
            "compact_constructor_declaration" => NodeKind::Constructor,

            // Fields and variables
            "field_declaration" => NodeKind::Property,
            "constant_declaration" => NodeKind::Constant,
            "local_variable_declaration" => NodeKind::Variable,
            "enum_constant" => NodeKind::Constant,
            "record_component" => NodeKind::Property,
            "variable_declarator" => NodeKind::Variable,

            // Parameters
            "formal_parameter" | "spread_parameter" | "catch_formal_parameter" => {
                NodeKind::Parameter
            }
            "inferred_parameters" => NodeKind::Parameter,
            "receiver_parameter" => NodeKind::Parameter,

            // Imports and packages
            "import_declaration" => NodeKind::Import,
            "package_declaration" => NodeKind::Module,

            // Type parameters
            "type_parameter" => NodeKind::TypeParameter,

            // Blocks
            "block" | "class_body" | "interface_body" | "enum_body" | "annotation_type_body" => {
                NodeKind::Block
            }
            "static_initializer" => NodeKind::Block,

            // Calls and invocations
            "method_invocation" => NodeKind::Call,
            "object_creation_expression" => NodeKind::Call,

            // Identifiers
            "identifier" | "type_identifier" => NodeKind::Identifier,

            // Comments
            "line_comment" | "block_comment" => NodeKind::Comment,

            // Annotations
            "annotation" | "marker_annotation" => NodeKind::Decorator,

            // Lambda expressions
            "lambda_expression" => NodeKind::Function,

            // Module declarations
            "module_declaration" => NodeKind::Module,

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/java/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/java/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/java/references.scm")
    }

    fn node_kind_with_source(&self, node: &tree_sitter::Node, source: &[u8]) -> NodeKind {
        // A `local_variable_declaration` with a `final` modifier is a local constant.
        // The `final` keyword appears as a direct `modifiers` child of the declaration,
        // or as a bare `final` child when no other modifiers are present.
        if node.kind() == "local_variable_declaration" && has_final_modifier(node, source) {
            return NodeKind::Constant;
        }
        self.node_kind(node)
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &["method_declaration", "constructor_declaration"]
    }

    fn function_body_kind(&self) -> &'static str {
        "block"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
            "switch_expression",
            "switch_statement",
            "catch_clause",
            "ternary_expression",
        ]
    }

    fn stub_placeholders(&self) -> &'static [&'static str] {
        &[
            "throw new UnsupportedOperationException()",
            "throw new RuntimeException(\"not implemented\")",
        ]
    }

    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &[
            "executeQuery",
            "execute",
            "executeUpdate",
            "prepareStatement",
        ]
    }

    fn unsafe_deserialize_names(&self) -> &'static [&'static str] {
        &["readObject", "readUnshared"]
    }

    fn taint_source_names(&self) -> &'static [&'static str] {
        &[
            "getParameter",
            "getHeader",
            "getProperty",
            "getenv",
            "readLine",
        ]
    }

    fn command_exec_names(&self) -> &'static [&'static str] {
        &["exec", "command", "start"]
    }

    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["File", "FileInputStream", "FileReader", "Paths"]
    }

    fn sanitizer_names(&self) -> &'static [&'static str] {
        &["escapeHtml", "encode", "sanitize"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }

    // --- call_detection ---
    /// Java calls are `method_invocation`; `new X()` is `object_creation_expression`.
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "object_creation_expression"]
    }

    // --- effect_summary ---
    /// `throw` raises in Java.
    fn raise_node_kinds(&self) -> &'static [&'static str] {
        &["throw_statement"]
    }

    /// `new X(...)` / `new T[...]` allocate.
    fn allocation_node_kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression", "array_creation_expression"]
    }

    /// `Thread.sleep`, blocking IO/locks, and `Future.get` block synchronously.
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["sleep", "join", "get", "wait", "acquire", "take", "put"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("raise")
            .with("allocate")
    }

    // --- data_flow ---
    /// Local variable / field declarations bind names.
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &[
            "local_variable_declaration",
            "field_declaration",
            "variable_declarator",
        ]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    // --- unsafe_surface ---
    /// `ScriptEngine.eval`, `Runtime.exec`, etc. evaluate code dynamically.
    fn eval_call_names(&self) -> &'static [&'static str] {
        &["eval", "evalScript"]
    }

    /// JNI / native interop markers.
    fn ffi_markers(&self) -> &'static [&'static str] {
        &["loadLibrary", "load", "native"]
    }

    /// Reflective invocation: `Class.forName`, `Method.invoke`, etc.
    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        &[
            "forName",
            "invoke",
            "getMethod",
            "getDeclaredMethod",
            "newInstance",
        ]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("eval")
            .with("raw_sql")
            .with("ffi")
            .with("reflection_invoke")
            .with("unsafe_deserialize")
    }

    // --- secret_surface ---
    /// JCA crypto APIs.
    fn crypto_api_names(&self) -> &'static [&'static str] {
        &[
            "getInstance",
            "MessageDigest",
            "Cipher",
            "Mac",
            "SecretKeySpec",
            "KeyPairGenerator",
            "Signature",
        ]
    }

    /// `System.getenv` reads environment secrets.
    fn env_read_names(&self) -> &'static [&'static str] {
        &["getenv", "getProperty"]
    }

    fn secret_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("credential_param")
            .with("token_var")
            .with("signing_key")
            .with("env_secret_read")
            .with("hardcoded_literal")
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "block",
            "expression_statement",
            "local_variable_declaration",
            "return_statement",
            "if_statement",
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "switch_block_statement_group",
            "synchronized_statement",
            "try_statement",
            "try_with_resources_statement",
            "throw_statement",
            "assert_statement",
            "break_statement",
            "continue_statement",
            "labeled_statement",
            "method_declaration",
            "constructor_declaration",
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "field_declaration",
            "static_initializer",
            "yield_statement",
            "method_invocation",
            "binary_expression",
            "ternary_expression",
            "unary_expression",
            "cast_expression",
            "instanceof_expression",
            "lambda_expression",
            "method_reference",
            "member_access",
            "array_access",
            "new_expression",
            "array_creation_expression",
            "parenthesized_expression",
            "assignment_expression",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};

    crate::language_support_tests!(JavaSupport, Grammar::Java, &["java"]);

    #[test]
    fn node_kind_enum_constant_is_constant() {
        // Enum constants are named constants of the enum type, not instance properties.
        // They must map to NodeKind::Constant, not NodeKind::Property.
        let support = JavaSupport;
        let tree = parse_with_support(&support, "enum Color { RED, GREEN, BLUE }");
        let node = find_node_by_kind(tree.root_node(), "enum_constant").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Constant);
    }

    #[test]
    fn node_kind_class_declaration() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "public class Foo {}");
        let node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Class);
    }

    #[test]
    fn node_kind_interface_declaration() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "public interface Bar {}");
        let node = find_node_by_kind(tree.root_node(), "interface_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_enum_declaration() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "public enum Status { ACTIVE, INACTIVE }");
        let node = find_node_by_kind(tree.root_node(), "enum_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Enum);
    }

    #[test]
    fn node_kind_record_declaration() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "public record Point(int x, int y) {}");
        let node = find_node_by_kind(tree.root_node(), "record_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Class);
    }

    #[test]
    fn node_kind_method_declaration() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "class Foo { void bar() {} }");
        let node = find_node_by_kind(tree.root_node(), "method_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Method);
    }

    #[test]
    fn node_kind_constructor_declaration() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "class Foo { Foo() {} }");
        let node = find_node_by_kind(tree.root_node(), "constructor_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Constructor);
    }

    #[test]
    fn node_kind_field_declaration() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "class Foo { private int x; }");
        let node = find_node_by_kind(tree.root_node(), "field_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Property);
    }

    #[test]
    fn node_kind_local_variable() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "class Foo { void bar() { int x = 1; } }");
        let node = find_node_by_kind(tree.root_node(), "local_variable_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_formal_parameter() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "class Foo { void bar(int x) {} }");
        let node = find_node_by_kind(tree.root_node(), "formal_parameter").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_import_declaration() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "import java.util.List;");
        let node = find_node_by_kind(tree.root_node(), "import_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Import);
    }

    #[test]
    fn node_kind_package_declaration() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "package com.example;");
        let node = find_node_by_kind(tree.root_node(), "package_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Module);
    }

    #[test]
    fn node_kind_method_invocation() {
        let support = JavaSupport;
        let tree = parse_with_support(
            &support,
            "class Foo { void bar() { System.out.println(); } }",
        );
        let node = find_node_by_kind(tree.root_node(), "method_invocation").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Call);
    }

    #[test]
    fn node_kind_annotation() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "@Override class Foo {}");
        let node = find_node_by_kind(tree.root_node(), "marker_annotation").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Decorator);
    }

    #[test]
    fn node_kind_lambda() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "class Foo { Runnable r = () -> {}; }");
        let node = find_node_by_kind(tree.root_node(), "lambda_expression").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_line_comment() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "// comment\nclass Foo {}");
        let node = find_node_by_kind(tree.root_node(), "line_comment").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_block_comment() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "/* comment */ class Foo {}");
        let node = find_node_by_kind(tree.root_node(), "block_comment").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_identifier() {
        let support = JavaSupport;
        let tree = parse_with_support(&support, "class Foo { int x; }");
        let node = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_with_source_final_local_is_constant() {
        // `final int MAX = 10;` is a local constant — node_kind_with_source must
        // return NodeKind::Constant, not NodeKind::Variable.
        let support = JavaSupport;
        let src = "class Foo { void bar() { final int MAX = 10; } }";
        let tree = parse_with_support(&support, src);
        let node = find_node_by_kind(tree.root_node(), "local_variable_declaration").unwrap();
        assert_eq!(
            support.node_kind_with_source(&node, src.as_bytes()),
            NodeKind::Constant,
            "final local variable should be NodeKind::Constant"
        );
    }

    #[test]
    fn node_kind_with_source_non_final_local_is_variable() {
        // Non-final locals must remain NodeKind::Variable.
        let support = JavaSupport;
        let src = "class Foo { void bar() { int x = 1; } }";
        let tree = parse_with_support(&support, src);
        let node = find_node_by_kind(tree.root_node(), "local_variable_declaration").unwrap();
        assert_eq!(
            support.node_kind_with_source(&node, src.as_bytes()),
            NodeKind::Variable,
            "non-final local variable should remain NodeKind::Variable"
        );
    }

    #[test]
    fn function_parent_kinds_are_java_specific() {
        // Default returns JavaScript node types; Java must return method_declaration
        // and constructor_declaration. Any consumer of function_parent_kinds() will
        // silently produce wrong results for Java files if this is not overridden.
        let support = JavaSupport;
        let kinds = support.function_parent_kinds();
        assert!(
            kinds.contains(&"method_declaration"),
            "Expected method_declaration in function_parent_kinds, got: {kinds:?}"
        );
        assert!(
            kinds.contains(&"constructor_declaration"),
            "Expected constructor_declaration in function_parent_kinds, got: {kinds:?}"
        );
        assert!(
            !kinds.contains(&"arrow_function"),
            "function_parent_kinds must not contain JavaScript-only arrow_function"
        );
    }

    #[test]
    fn function_body_kind_is_block() {
        // Default returns "statement_block" (JavaScript). Java uses "block".
        let support = JavaSupport;
        assert_eq!(support.function_body_kind(), "block");
    }

    #[test]
    fn decision_point_kinds_are_java_specific() {
        // Default includes for_in_statement and switch_case (JavaScript node types that
        // do not exist in the Java grammar). Java must use its own control flow nodes.
        let support = JavaSupport;
        let kinds = support.decision_point_kinds();
        assert!(
            kinds.contains(&"enhanced_for_statement"),
            "Expected enhanced_for_statement; got: {kinds:?}"
        );
        assert!(
            kinds.contains(&"switch_statement"),
            "Expected switch_statement; got: {kinds:?}"
        );
        assert!(
            !kinds.contains(&"for_in_statement"),
            "decision_point_kinds must not contain JavaScript-only for_in_statement"
        );
        assert!(
            !kinds.contains(&"switch_case"),
            "decision_point_kinds must not contain JavaScript-only switch_case"
        );
    }

    #[test]
    fn stub_placeholders_returns_language_patterns() {
        let support = JavaSupport;
        let placeholders = support.stub_placeholders();
        assert!(!placeholders.is_empty());
        assert!(placeholders.contains(&"throw new UnsupportedOperationException()"));
    }

    #[test]
    fn coverage_hooks_java() {
        let s = JavaSupport;
        // call_detection: real call node is method_invocation, not the default.
        assert!(s.call_node_kinds().contains(&"method_invocation"));
        assert_ne!(s.call_node_kinds(), ["call_expression", "call"]);
        // effect_summary: widened beyond read/write with raise + allocate.
        assert!(s.raise_node_kinds().contains(&"throw_statement"));
        assert!(
            s.allocation_node_kinds()
                .contains(&"object_creation_expression")
        );
        assert!(s.effect_modeled_kinds().contains("raise"));
        assert!(s.effect_modeled_kinds().contains("allocate"));
        // data_flow
        assert!(
            s.binding_node_kinds()
                .contains(&"local_variable_declaration")
        );
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
        // unsafe_surface: real dangerous APIs.
        assert!(s.reflection_invoke_names().contains(&"forName"));
        assert!(s.unsafe_deserialize_names().contains(&"readObject"));
        assert!(s.raw_sql_sink_names().contains(&"executeQuery"));
        assert!(s.unsafe_modeled_kinds().contains("reflection_invoke"));
        // secret_surface
        assert!(s.crypto_api_names().contains(&"Cipher"));
        assert!(s.env_read_names().contains(&"getenv"));
        assert!(s.secret_modeled_kinds().contains("env_secret_read"));
        // taint_flow
        assert!(s.taint_source_names().contains(&"getParameter"));
        assert!(s.taint_modeled_kinds().contains("taint"));
    }
}
