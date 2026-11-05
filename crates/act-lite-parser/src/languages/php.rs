//! PHP language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register PHP in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("php"),
        display_name: "PHP",
        extensions: &["php"],
        category: LanguageCategory::Dynamic,
    }
}

/// PHP language support implementation.
#[derive(Default)]
pub struct PhpSupport;

impl LanguageSupport for PhpSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Php
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_php::LANGUAGE_PHP.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Functions
            "function_definition" => NodeKind::Function,
            "method_declaration" => NodeKind::Method,
            "arrow_function" => NodeKind::Function,
            "anonymous_function" => NodeKind::Function,

            // Classes and types
            "class_declaration" => NodeKind::Class,
            "anonymous_class_creation_expression" => NodeKind::Class,
            "interface_declaration" => NodeKind::Interface,
            "trait_declaration" => NodeKind::Trait,
            "enum_declaration" => NodeKind::Enum,
            "enum_case" => NodeKind::Constant,

            // Members
            "property_declaration" => NodeKind::Property,
            "property_element" => NodeKind::Property,
            "property_promotion_parameter" => NodeKind::Property,
            "const_declaration" | "const_element" => NodeKind::Constant,

            // Parameters
            "simple_parameter" => NodeKind::Parameter,
            "variadic_parameter" => NodeKind::Parameter,

            // Variables
            "variable_name" => NodeKind::Variable,
            "static_variable_declaration" => NodeKind::Variable,
            "global_declaration" => NodeKind::Variable,

            // Imports/Namespaces
            "namespace_definition" => NodeKind::Module,
            "namespace_use_declaration" | "namespace_use_clause" => NodeKind::Import,
            "use_declaration" => NodeKind::Import,

            // Blocks and control flow
            "compound_statement" | "statement" => NodeKind::Block,
            "if_statement" | "switch_statement" => NodeKind::Block,
            "for_statement" | "foreach_statement" => NodeKind::Block,
            "while_statement" | "do_statement" => NodeKind::Block,
            "try_statement" | "catch_clause" | "finally_clause" => NodeKind::Block,
            "match_expression" | "match_arm" => NodeKind::Block,

            // Calls
            "function_call_expression" => NodeKind::Call,
            "member_call_expression" => NodeKind::Call,
            "nullsafe_member_call_expression" => NodeKind::Call,
            "scoped_call_expression" => NodeKind::Call,
            "object_creation_expression" => NodeKind::Call,

            // Identifiers
            "name" => NodeKind::Identifier,
            "namespace_name" => NodeKind::Identifier,
            "qualified_name" => NodeKind::Identifier,
            "nullsafe_member_access_expression" => NodeKind::Identifier,

            // Comments
            "comment" => NodeKind::Comment,

            // Attributes
            "attribute_list" | "attribute_group" | "attribute" => NodeKind::Decorator,

            // Types
            "named_type" | "primitive_type" => NodeKind::Type,
            "union_type" | "intersection_type" | "nullable_type" => NodeKind::Type,
            "optional_type" | "type_list" => NodeKind::Type,

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["php", "phtml", "php3", "php4", "php5", "php7", "php8"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/php/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/php/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/php/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        // NOTE: PHP also supports `#` as a single-line comment prefix (legacy syntax,
        // still valid in PHP 8.x). CommentDelimiters supports only one line prefix.
        // Files using `#`-style comments will not be handled correctly by operations
        // that manipulate comments (add-docblock, comment stripping).
        CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &[
            "function_definition",
            "method_declaration",
            "arrow_function",
            "anonymous_function",
        ]
    }

    fn function_body_kind(&self) -> &'static str {
        "compound_statement"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "elseif_clause",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "catch_clause",
            "match_expression",
            "ternary_expression",
            "null_coalescing_expression",
            "null_coalescing_assignment_expression",
        ]
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "compound_statement",
            "expression_statement",
            "assignment_expression",
            "augmented_assignment_expression",
            "return_statement",
            "if_statement",
            "elseif_clause",
            "else_clause",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "case_statement",
            "default_statement",
            "try_statement",
            "catch_clause",
            "finally_clause",
            "throw_statement",
            "break_statement",
            "continue_statement",
            "goto_statement",
            "echo_statement",
            "global_declaration",
            "static_declaration",
            "function_definition",
            "method_declaration",
            "class_declaration",
            "interface_declaration",
            "trait_declaration",
            "enum_declaration",
            "namespace_definition",
            "use_declaration",
            "function_call_expression",
            "member_access_expression",
            "scoped_access_expression",
            "binary_expression",
            "ternary_expression",
            "unary_expression",
            "cast_expression",
            "instanceof_expression",
            "match_expression",
            "arrow_function",
            "closure",
            "new_expression",
            "parenthesized_expression",
            "array_creation_expression",
            "string",
            "encapsed_string",
            "include_expression",
            "require_expression",
            "print_intrinsic",
            "clone_expression",
            "yield_expression",
            "spread_element",
        ]
    }

    // ---- moat coverage hooks (PHP) ----

    /// PHP call nodes: free functions, instance/static/nullsafe methods, and
    /// `new` object creation. Overrides the `call_expression`/`call` default.
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &[
            "function_call_expression",
            "member_call_expression",
            "nullsafe_member_call_expression",
            "scoped_call_expression",
            "object_creation_expression",
        ]
    }

    /// `$x = ...` and compound assignments bind names.
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["assignment_expression", "augmented_assignment_expression"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    /// `throw new Exception(...)` is a `throw_expression`.
    fn raise_node_kinds(&self) -> &'static [&'static str] {
        &["throw_expression"]
    }

    /// `new Foo()` allocates an object.
    fn allocation_node_kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression"]
    }

    /// Blocking calls: `sleep`/`usleep`, `fread`/`fgets` (stdin), `flock`.
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["sleep", "usleep", "fread", "fgets", "flock"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("raise")
            .with("allocate")
            .with("block")
    }

    /// Dynamic evaluation: `eval`, `assert` (string form), `create_function`,
    /// `call_user_func`, plus `include`/`require` which execute arbitrary files.
    fn eval_call_names(&self) -> &'static [&'static str] {
        &[
            "eval",
            "assert",
            "create_function",
            "include",
            "include_once",
            "require",
            "require_once",
        ]
    }

    /// Raw SQL sinks: `mysqli_query`, PDO `query`/`exec`, `mysql_query`.
    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &["query", "exec", "mysqli_query", "mysql_query"]
    }

    /// Reflective invocation: `call_user_func`, `call_user_func_array`,
    /// `ReflectionMethod::invoke`, variable functions.
    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        &[
            "call_user_func",
            "call_user_func_array",
            "invoke",
            "invokeArgs",
        ]
    }

    /// Unsafe deserialization: `unserialize` (object injection).
    fn unsafe_deserialize_names(&self) -> &'static [&'static str] {
        &["unserialize"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("eval")
            .with("raw_sql")
            .with("reflection_invoke")
            .with("unsafe_deserialize")
    }

    /// Crypto APIs: `hash_hmac`, `openssl_sign`, `password_hash`, `crypt`.
    fn crypto_api_names(&self) -> &'static [&'static str] {
        &[
            "hash_hmac",
            "openssl_sign",
            "password_hash",
            "crypt",
            "hash",
        ]
    }

    /// Environment-secret reads: `getenv`, `$_ENV`, `$_SERVER`, `apache_getenv`.
    fn env_read_names(&self) -> &'static [&'static str] {
        &["getenv", "_ENV", "_SERVER", "apache_getenv"]
    }

    fn secret_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("env_secret_read")
            .with("signing_key")
    }

    /// Taint sources: superglobals `$_GET`/`$_POST`/`$_REQUEST`/`$_COOKIE`,
    /// env reads, and raw input via `file_get_contents("php://input")`.
    fn taint_source_names(&self) -> &'static [&'static str] {
        &[
            "_GET",
            "_POST",
            "_REQUEST",
            "_COOKIE",
            "getenv",
            "file_get_contents",
        ]
    }

    /// Command-injection sinks: `system`, `exec`, `shell_exec`, `passthru`,
    /// `proc_open`, `popen`.
    fn command_exec_names(&self) -> &'static [&'static str] {
        &[
            "system",
            "exec",
            "shell_exec",
            "passthru",
            "proc_open",
            "popen",
        ]
    }

    /// Filesystem sinks: `fopen`, `file_put_contents`, `file_get_contents`,
    /// `unlink`, `readfile`.
    fn fs_sink_names(&self) -> &'static [&'static str] {
        &[
            "fopen",
            "file_put_contents",
            "file_get_contents",
            "unlink",
            "readfile",
        ]
    }

    /// Sanitizers: `htmlspecialchars`, `escapeshellarg`, `addslashes`,
    /// `filter_var`, `mysqli_real_escape_string`.
    fn sanitizer_names(&self) -> &'static [&'static str] {
        &[
            "htmlspecialchars",
            "escapeshellarg",
            "escapeshellcmd",
            "addslashes",
            "filter_var",
            "mysqli_real_escape_string",
        ]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};

    crate::language_support_tests!(
        PhpSupport,
        Grammar::Php,
        &["php", "phtml", "php3", "php4", "php5", "php7", "php8"]
    );

    #[test]
    fn node_kind_function_definition() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php function foo() {}");
        let func_node = find_node_by_kind(tree.root_node(), "function_definition").unwrap();
        assert_eq!(support.node_kind(&func_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_class_declaration() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php class Foo {}");
        let class_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(support.node_kind(&class_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_interface_declaration() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php interface Repository {}");
        let iface_node = find_node_by_kind(tree.root_node(), "interface_declaration").unwrap();
        assert_eq!(support.node_kind(&iface_node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_trait_declaration() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php trait Loggable {}");
        let trait_node = find_node_by_kind(tree.root_node(), "trait_declaration").unwrap();
        assert_eq!(support.node_kind(&trait_node), NodeKind::Trait);
    }

    #[test]
    fn node_kind_enum_declaration() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php enum Status { case Active; }");
        let enum_node = find_node_by_kind(tree.root_node(), "enum_declaration").unwrap();
        assert_eq!(support.node_kind(&enum_node), NodeKind::Enum);
    }

    #[test]
    fn node_kind_method_declaration() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php class Foo { public function bar() {} }");
        let method_node = find_node_by_kind(tree.root_node(), "method_declaration").unwrap();
        assert_eq!(support.node_kind(&method_node), NodeKind::Method);
    }

    #[test]
    fn node_kind_arrow_function() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php $f = fn($x) => $x * 2;");
        let arrow_node = find_node_by_kind(tree.root_node(), "arrow_function").unwrap();
        assert_eq!(support.node_kind(&arrow_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_anonymous_function() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php $f = function($x) { return $x; };");
        let anon_node = find_node_by_kind(tree.root_node(), "anonymous_function").unwrap();
        assert_eq!(support.node_kind(&anon_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_namespace_definition() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php namespace App\\Services;");
        let ns_node = find_node_by_kind(tree.root_node(), "namespace_definition").unwrap();
        assert_eq!(support.node_kind(&ns_node), NodeKind::Module);
    }

    #[test]
    fn node_kind_use_declaration() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php use App\\Models\\User;");
        let use_node = find_node_by_kind(tree.root_node(), "namespace_use_declaration").unwrap();
        assert_eq!(support.node_kind(&use_node), NodeKind::Import);
    }

    #[test]
    fn node_kind_function_call() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php foo();");
        let call_node = find_node_by_kind(tree.root_node(), "function_call_expression").unwrap();
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_method_call() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php $obj->method();");
        let call_node = find_node_by_kind(tree.root_node(), "member_call_expression").unwrap();
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_static_call() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php Foo::bar();");
        let call_node = find_node_by_kind(tree.root_node(), "scoped_call_expression").unwrap();
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_comment() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php // comment\n$x = 1;");
        let comment_node = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_attribute() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php #[Attribute] class Foo {}");
        let attr_node = find_node_by_kind(tree.root_node(), "attribute").unwrap();
        assert_eq!(support.node_kind(&attr_node), NodeKind::Decorator);
    }

    #[test]
    fn function_parent_kinds_are_php_specific() {
        let support = PhpSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"function_definition"));
        assert!(kinds.contains(&"method_declaration"));
        assert!(kinds.contains(&"arrow_function"));
        assert!(kinds.contains(&"anonymous_function"));
        // Must NOT contain JS/TS only values
        assert!(!kinds.contains(&"function_declaration"));
        assert!(!kinds.contains(&"method_definition"));
    }

    #[test]
    fn function_body_kind_is_compound_statement() {
        let support = PhpSupport;
        assert_eq!(support.function_body_kind(), "compound_statement");
    }

    #[test]
    fn decision_point_kinds_are_php_specific() {
        let support = PhpSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_statement"));
        assert!(kinds.contains(&"foreach_statement"));
        assert!(kinds.contains(&"match_expression"));
        assert!(kinds.contains(&"elseif_clause"));
        assert!(kinds.contains(&"ternary_expression"));
        assert!(kinds.contains(&"null_coalescing_expression"));
        assert!(kinds.contains(&"null_coalescing_assignment_expression"));
        // Must NOT contain JS/TS only values
        assert!(!kinds.contains(&"for_in_statement"));
        assert!(!kinds.contains(&"switch_case"));
    }

    #[test]
    fn node_kind_nullsafe_member_call() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php $obj?->method();");
        let node = find_node_by_kind(tree.root_node(), "nullsafe_member_call_expression").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Call);
    }

    #[test]
    fn node_kind_nullsafe_member_access() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php $obj?->prop;");
        let node =
            find_node_by_kind(tree.root_node(), "nullsafe_member_access_expression").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_enum_case_is_constant() {
        let support = PhpSupport;
        let tree = parse_with_support(&support, "<?php enum Status { case Active; }");
        let case_node = find_node_by_kind(tree.root_node(), "enum_case").unwrap();
        assert_eq!(support.node_kind(&case_node), NodeKind::Constant);
    }

    // ---- moat coverage hooks ----

    #[test]
    fn moat_call_node_overrides_default() {
        let s = PhpSupport;
        assert!(s.call_node_kinds().contains(&"function_call_expression"));
        assert!(s.call_node_kinds().contains(&"member_call_expression"));
        let tree = parse_with_support(&s, "<?php foo();");
        assert!(find_node_by_kind(tree.root_node(), "function_call_expression").is_some());
    }

    #[test]
    fn moat_effect_models_raise_alloc_blocking() {
        let s = PhpSupport;
        assert!(s.raise_node_kinds().contains(&"throw_expression"));
        assert!(
            s.allocation_node_kinds()
                .contains(&"object_creation_expression")
        );
        assert!(s.effect_modeled_kinds().contains("raise"));
        assert!(s.effect_modeled_kinds().contains("allocate"));
        // throw_expression actually parses.
        let tree = parse_with_support(&s, "<?php function f(){ throw new E(); }");
        assert!(find_node_by_kind(tree.root_node(), "throw_expression").is_some());
    }

    #[test]
    fn moat_data_flow_binds_assignments() {
        let s = PhpSupport;
        assert!(s.binding_node_kinds().contains(&"assignment_expression"));
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
        let tree = parse_with_support(&s, "<?php $x = 1;");
        assert!(find_node_by_kind(tree.root_node(), "assignment_expression").is_some());
    }

    #[test]
    fn moat_unsafe_models_eval_sql_reflection_deserialize() {
        let s = PhpSupport;
        assert!(s.eval_call_names().contains(&"eval"));
        assert!(s.raw_sql_sink_names().contains(&"query"));
        assert!(s.reflection_invoke_names().contains(&"call_user_func"));
        assert!(s.unsafe_deserialize_names().contains(&"unserialize"));
        assert!(s.unsafe_modeled_kinds().contains("eval"));
        assert!(s.unsafe_modeled_kinds().contains("unsafe_deserialize"));
    }

    #[test]
    fn moat_secret_models_env_and_crypto() {
        let s = PhpSupport;
        assert!(s.env_read_names().contains(&"getenv"));
        assert!(s.crypto_api_names().contains(&"hash_hmac"));
        assert!(s.secret_modeled_kinds().contains("env_secret_read"));
        assert!(s.secret_modeled_kinds().contains("signing_key"));
    }

    #[test]
    fn moat_taint_models_superglobals_and_command_sink() {
        let s = PhpSupport;
        assert!(s.taint_source_names().contains(&"_GET"));
        assert!(s.taint_source_names().contains(&"_POST"));
        assert!(s.command_exec_names().contains(&"system"));
        assert!(s.command_exec_names().contains(&"shell_exec"));
        assert!(s.sanitizer_names().contains(&"htmlspecialchars"));
        assert!(s.taint_modeled_kinds().contains("taint"));
    }
}
