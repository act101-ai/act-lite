//! Kotlin language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Kotlin in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("kotlin"),
        display_name: "Kotlin",
        extensions: &["kt", "kts"],
        category: LanguageCategory::Managed,
    }
}

/// Kotlin language support implementation.
#[derive(Default)]
pub struct KotlinSupport;

impl LanguageSupport for KotlinSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Kotlin
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_kotlin::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Functions
            "function_declaration" => NodeKind::Function,
            "anonymous_function" | "lambda_literal" => NodeKind::Function,

            // Classes and types
            "class_declaration" => self.classify_class(node),
            "object_declaration" => NodeKind::Class,
            "companion_object" => NodeKind::Class,
            "type_alias" => NodeKind::TypeAlias,

            // Members
            "property_declaration" => self.classify_property(node),
            "getter" | "setter" => NodeKind::Property,
            "enum_entry" => NodeKind::Property,

            // Parameters
            "parameter" | "value_parameter" | "class_parameter" => NodeKind::Parameter,
            "lambda_parameters" => NodeKind::Parameter,

            // Variables
            "variable_declaration" => NodeKind::Variable,
            "multi_variable_declaration" => NodeKind::Variable,

            // Imports and packages
            "import_header" | "import_alias" => NodeKind::Import,
            "package_header" => NodeKind::Module,

            // Blocks and control flow
            "class_body" | "function_body" | "control_structure_body" => NodeKind::Block,
            "when_expression" | "when_entry" => NodeKind::Block,
            "if_expression" | "try_expression" => NodeKind::Block,

            // Calls
            "call_expression" | "constructor_invocation" => NodeKind::Call,

            // Identifiers
            "simple_identifier" | "type_identifier" | "identifier" => NodeKind::Identifier,

            // Comments
            "line_comment" | "multiline_comment" => NodeKind::Comment,

            // Annotations
            "annotation" | "annotation_entry" => NodeKind::Decorator,

            // Types
            "user_type" | "nullable_type" | "type_reference" => NodeKind::Type,
            "type_parameters" | "type_parameter" => NodeKind::Type,

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["kt", "kts"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/kotlin/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/kotlin/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/kotlin/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &[
            "function_declaration",
            "function_literal",
            "anonymous_function",
        ]
    }

    fn function_body_kind(&self) -> &'static str {
        "function_body"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "when_expression",
            "when_entry",
            "for_statement",
            "while_statement",
            "do_while_statement",
            "catch_block",
            "ternary",
        ]
    }

    fn function_keyword(&self) -> &'static str {
        "fun"
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "when_expression",
            "when_entry",
            "property_declaration",
            "for_statement",
            "while_statement",
            "do_while_statement",
            "try_expression",
            "function_body",
            "lambda_literal",
            "function_declaration",
        ]
    }

    // --- call_detection ---
    /// Kotlin calls are `call_expression`; `Foo()` constructor calls use
    /// `constructor_invocation` (and superclass init in `delegation_specifier`).
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "constructor_invocation"]
    }

    // --- effect_summary ---
    /// Kotlin has no dedicated throw node; `throw e` parses as a
    /// `jump_expression` (shared with `return`/`break`/`continue`). This is the
    /// only throw-bearing node the grammar exposes.
    fn raise_node_kinds(&self) -> &'static [&'static str] {
        &["jump_expression"]
    }

    /// `Foo()` allocates via `constructor_invocation` / object literal.
    fn allocation_node_kinds(&self) -> &'static [&'static str] {
        &["constructor_invocation", "object_literal"]
    }

    /// `suspend`/coroutine awaits and blocking interop.
    fn await_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression"]
    }

    /// Coroutine + JVM blocking calls.
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["runBlocking", "sleep", "join", "await", "get"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("raise")
            .with("allocate")
    }

    // --- data_flow ---
    /// `val`/`var` bind via `property_declaration` / `variable_declaration`.
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &[
            "property_declaration",
            "variable_declaration",
            "multi_variable_declaration",
        ]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    // --- unsafe_surface ---
    /// JSR-223 scripting + Kotlin scripting host.
    fn eval_call_names(&self) -> &'static [&'static str] {
        &["eval", "evalScript", "evaluationScript"]
    }

    /// JNI / native interop.
    fn ffi_markers(&self) -> &'static [&'static str] {
        &["loadLibrary", "load", "external"]
    }

    /// JDBC raw SQL sinks (Kotlin uses the JVM/JDBC stack).
    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &["execute", "executeQuery", "executeUpdate", "rawQuery"]
    }

    /// Kotlin reflection + Java reflection interop.
    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        &["call", "callBy", "invoke", "forName", "newInstance"]
    }

    /// Java deserialization on the JVM.
    fn unsafe_deserialize_names(&self) -> &'static [&'static str] {
        &["readObject", "readUnshared"]
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
    /// JCA crypto APIs (Kotlin uses the Java crypto stack).
    fn crypto_api_names(&self) -> &'static [&'static str] {
        &[
            "getInstance",
            "MessageDigest",
            "Cipher",
            "Mac",
            "SecretKeySpec",
        ]
    }

    /// `System.getenv` / property reads.
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

    // --- taint_flow ---
    /// Web-input + IO sources (Ktor/servlet/stdin).
    fn taint_source_names(&self) -> &'static [&'static str] {
        &["getParameter", "getHeader", "receive", "readLine", "getenv"]
    }

    /// OS command execution.
    fn command_exec_names(&self) -> &'static [&'static str] {
        &["exec", "command", "start", "ProcessBuilder"]
    }

    /// Filesystem sinks.
    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["File", "readText", "writeText", "Paths"]
    }

    /// Escaping/sanitizers.
    fn sanitizer_names(&self) -> &'static [&'static str] {
        &["escapeHtml", "encode", "sanitize"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

impl KotlinSupport {
    /// Classify a class declaration based on its structure.
    ///
    /// According to tree-sitter-kotlin grammar:
    /// - Interface: `class_declaration` contains "interface" as a direct child token (not in modifiers)
    /// - Enum: `class_declaration` has "enum" followed by "class" as direct children
    /// - Annotation: `class_declaration` has "annotation" in modifiers
    /// - Data class: `class_declaration` has "data" in modifiers
    /// - Sealed class: `class_declaration` has "sealed" in modifiers
    fn classify_class(&self, node: &tree_sitter::Node) -> NodeKind {
        // First check for "enum" pattern (enum class)
        // In tree-sitter-kotlin, enum class is parsed as: [modifiers?, "enum", "class", type_identifier, ...]
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();

        // Look for the pattern: "enum" followed by "class"
        for (i, child) in children.iter().enumerate() {
            if child.kind() == "enum" {
                // Check if next child is "class"
                if i + 1 < children.len() && children[i + 1].kind() == "class" {
                    return NodeKind::Enum;
                }
            }
        }

        // Look for "interface" keyword (not in modifiers)
        // Interface can be just "interface" or "fun interface"
        for child in &children {
            if child.kind() == "interface" || child.kind() == "fun" {
                return NodeKind::Interface;
            }
        }

        // Check modifiers for annotation, sealed, data
        for child in &children {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for modifier in child.children(&mut mod_cursor) {
                    match modifier.kind() {
                        "annotation" => return NodeKind::Decorator,
                        "sealed" | "data" | "inner" | "value" => {
                            // These are special class types but still classify as Class
                            // with specific attributes - for now return Class
                        }
                        _ => {}
                    }
                }
            }
        }

        NodeKind::Class
    }

    /// Classify a property based on context.
    fn classify_property(&self, node: &tree_sitter::Node) -> NodeKind {
        // Check if property is inside a class body (field) or top-level (variable)
        if let Some(parent) = node.parent() {
            match parent.kind() {
                "class_body" => return NodeKind::Property,
                "source_file" => return NodeKind::Variable,
                _ => {}
            }
        }
        NodeKind::Property
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};
    use act_lite_core::registry::LANGUAGE_REGISTRY;

    crate::language_support_tests!(KotlinSupport, Grammar::Kotlin, &["kt", "kts"]);

    #[test]
    fn registry_has_kotlin() {
        let meta = LANGUAGE_REGISTRY.get(LanguageId("kotlin"));
        assert!(meta.is_some(), "Kotlin should be registered");
        let meta = meta.unwrap();
        assert_eq!(meta.display_name, "Kotlin");
        assert!(meta.extensions.contains(&"kt"));
        assert!(meta.extensions.contains(&"kts"));
    }

    #[test]
    fn ts_language_does_not_panic() {
        // Smoke test: calling ts_language() must not panic and the returned
        // Language must be dropped safely. This exercises both the transmute
        // and the Drop path (ts_language_delete on a static TSLanguage pointer).
        let support = KotlinSupport;
        let _lang = support.ts_language();
    }

    #[test]
    fn node_kind_function_declaration() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "fun foo() {}");
        let func_node = find_node_by_kind(tree.root_node(), "function_declaration").unwrap();
        assert_eq!(support.node_kind(&func_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_class_declaration() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "class Foo {}");
        let class_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(support.node_kind(&class_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_data_class() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "data class User(val name: String)");
        let class_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(support.node_kind(&class_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_interface() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "interface Repository {}");
        let iface_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(support.node_kind(&iface_node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_enum_class() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "enum class Status { ACTIVE, INACTIVE }");
        let enum_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(support.node_kind(&enum_node), NodeKind::Enum);
    }

    #[test]
    fn node_kind_object_declaration() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "object Singleton {}");
        let obj_node = find_node_by_kind(tree.root_node(), "object_declaration").unwrap();
        assert_eq!(support.node_kind(&obj_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_type_alias() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "typealias UserMap = Map<String, User>");
        let type_node = find_node_by_kind(tree.root_node(), "type_alias").unwrap();
        assert_eq!(support.node_kind(&type_node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_property_declaration() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "val x = 1");
        let prop_node = find_node_by_kind(tree.root_node(), "property_declaration").unwrap();
        // Top-level property is Variable
        assert_eq!(support.node_kind(&prop_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_import_header() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "import kotlin.collections.List");
        let import_node = find_node_by_kind(tree.root_node(), "import_header").unwrap();
        assert_eq!(support.node_kind(&import_node), NodeKind::Import);
    }

    #[test]
    fn node_kind_call_expression() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "fun main() { foo() }");
        let call_node = find_node_by_kind(tree.root_node(), "call_expression").unwrap();
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_simple_identifier() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "val x = y");
        let ident_node = find_node_by_kind(tree.root_node(), "simple_identifier").unwrap();
        assert_eq!(support.node_kind(&ident_node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_line_comment() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "// comment\nval x = 1");
        let comment_node = find_node_by_kind(tree.root_node(), "line_comment").unwrap();
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_multiline_comment() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "/* comment */\nval x = 1");
        let comment_node = find_node_by_kind(tree.root_node(), "multiline_comment").unwrap();
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_annotation() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "@Deprecated(\"use new\") fun foo() {}");
        let ann_node = find_node_by_kind(tree.root_node(), "annotation").unwrap();
        assert_eq!(support.node_kind(&ann_node), NodeKind::Decorator);
    }

    #[test]
    fn node_kind_lambda_literal() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "val f = { x: Int -> x + 1 }");
        let lambda_node = find_node_by_kind(tree.root_node(), "lambda_literal").unwrap();
        assert_eq!(support.node_kind(&lambda_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_package_header_is_module() {
        let support = KotlinSupport;
        let tree = parse_with_support(&support, "package com.example.app");
        let pkg_node = find_node_by_kind(tree.root_node(), "package_header").unwrap();
        assert_eq!(support.node_kind(&pkg_node), NodeKind::Module);
    }
    #[test]
    fn node_kind_getter_is_property() {
        let support = KotlinSupport;
        let code = "class Foo {\n    val count: Int\n        get() = 0\n}";
        let tree = parse_with_support(&support, code);
        if let Some(getter_node) = find_node_by_kind(tree.root_node(), "getter") {
            assert_eq!(support.node_kind(&getter_node), NodeKind::Property);
        }
    }

    #[test]
    fn node_kind_setter_is_property() {
        let support = KotlinSupport;
        let code = "class Foo {\n    var count: Int = 0\n        set(value) { field = value }\n}";
        let tree = parse_with_support(&support, code);
        if let Some(setter_node) = find_node_by_kind(tree.root_node(), "setter") {
            assert_eq!(support.node_kind(&setter_node), NodeKind::Property);
        }
    }
    #[test]
    fn function_parent_kinds_are_kotlin_specific() {
        let support = KotlinSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"function_declaration"));
        assert!(kinds.contains(&"function_literal"));
        assert!(kinds.contains(&"anonymous_function"));
        // Must NOT contain JS-only values
        assert!(
            !kinds.contains(&"method_definition"),
            "method_definition is a JS node, not Kotlin"
        );
        assert!(
            !kinds.contains(&"arrow_function"),
            "arrow_function is a JS node, not Kotlin"
        );
    }

    #[test]
    fn function_body_kind_is_kotlin_specific() {
        let support = KotlinSupport;
        assert_eq!(
            support.function_body_kind(),
            "function_body",
            "Kotlin uses function_body not statement_block"
        );
    }

    #[test]
    fn decision_point_kinds_are_kotlin_specific() {
        let support = KotlinSupport;
        let kinds = support.decision_point_kinds();
        assert!(
            kinds.contains(&"if_expression"),
            "Kotlin uses if_expression"
        );
        assert!(
            kinds.contains(&"when_expression"),
            "Kotlin uses when_expression"
        );
        assert!(
            kinds.contains(&"for_statement"),
            "Kotlin uses for_statement"
        );
        assert!(
            kinds.contains(&"while_statement"),
            "Kotlin uses while_statement"
        );
        // Must NOT contain JS-only values
        assert!(
            !kinds.contains(&"if_statement"),
            "if_statement is JS, Kotlin uses if_expression"
        );
        assert!(
            !kinds.contains(&"switch_case"),
            "switch_case is JS, Kotlin uses when_expression"
        );
    }

    #[test]
    fn function_keyword_is_fun() {
        let support = KotlinSupport;
        assert_eq!(support.function_keyword(), "fun");
    }

    #[test]
    fn extractable_node_kinds_includes_kotlin_kinds() {
        let support = KotlinSupport;
        let kinds = support.extractable_node_kinds();
        assert!(kinds.contains(&"if_expression"));
        assert!(kinds.contains(&"when_expression"));
        assert!(kinds.contains(&"try_expression"));
        assert!(!kinds.is_empty());
    }

    #[test]
    fn coverage_hooks_kotlin() {
        let s = KotlinSupport;
        // call_detection — real call node present and differs from default.
        let tree = parse_with_support(&s, "fun main() { foo() }");
        assert!(find_node_by_kind(tree.root_node(), "call_expression").is_some());
        assert!(s.call_node_kinds().contains(&"constructor_invocation"));
        assert_ne!(s.call_node_kinds(), ["call_expression", "call"]);
        // effect_summary — throw is jump_expression in this grammar.
        let t2 = parse_with_support(&s, "fun f() { throw RuntimeException(\"e\") }");
        assert!(find_node_by_kind(t2.root_node(), "jump_expression").is_some());
        assert!(s.raise_node_kinds().contains(&"jump_expression"));
        assert!(s.effect_modeled_kinds().contains("raise"));
        assert!(s.effect_modeled_kinds().contains("allocate"));
        // data_flow
        assert!(s.binding_node_kinds().contains(&"property_declaration"));
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
        // unsafe_surface
        assert!(s.reflection_invoke_names().contains(&"forName"));
        assert!(s.unsafe_modeled_kinds().contains("eval"));
        // secret_surface
        assert!(s.crypto_api_names().contains(&"Cipher"));
        assert!(s.secret_modeled_kinds().contains("env_secret_read"));
        // taint_flow
        assert!(s.command_exec_names().contains(&"exec"));
        assert!(s.taint_modeled_kinds().contains("taint"));
    }
}
