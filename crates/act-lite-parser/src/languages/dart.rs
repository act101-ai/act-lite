//! Dart language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Dart in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("dart"),
        display_name: "Dart",
        extensions: &["dart"],
        category: LanguageCategory::Managed,
    }
}

/// Dart language support implementation.
#[derive(Default)]
pub struct DartSupport;

impl LanguageSupport for DartSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Dart
    }

    fn ts_language(&self) -> tree_sitter::Language {
        // tree-sitter-dart 0.0.4 uses the tree-sitter 0.20 ABI where
        // `Language` is a single-field struct wrapping `*const TSLanguage`.
        // tree-sitter 0.24 uses an identical layout. Both structs are
        // `struct Language(*const ffi::TSLanguage)`. The transmute is safe
        // because the underlying data is the same raw pointer; only the type
        // wrapper differs between semver-incompatible crate versions.
        // This mirrors the same fix applied to KotlinSupport.
        //
        // TODO: replace once tree-sitter-dart publishes a version that uses
        // tree-sitter-language and exports a `LANGUAGE: LanguageFn` constant.
        tree_sitter_dart::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Top-level function declarations.
            // NOTE: tree-sitter-dart 0.0.4 uses "lambda_expression" for named top-level
            // function declarations (e.g. `void greet(String name) { ... }`). Despite the
            // misleading name, this is the correct node kind as confirmed by parsing real
            // Dart code and inspecting the resulting tree. The term "lambda" here reflects
            // an internal grammar choice, not a semantic lambda/closure concept.
            "lambda_expression" => NodeKind::Function,

            // Class-like declarations
            "class_definition" | "mixin_declaration" | "extension_declaration" => NodeKind::Class,
            "enum_declaration" => NodeKind::Enum,
            "type_alias" => NodeKind::TypeAlias,

            // Imports and exports
            "import_or_export" => NodeKind::Import,

            // Local variable declarations
            "local_variable_declaration" | "initialized_variable_definition" => NodeKind::Variable,

            // Class members: methods, getters, setters, constructors, and fields.
            // All share the "class_member_definition" outer node; we inspect children
            // to determine the actual kind.
            "class_member_definition" => {
                // Named methods, getters, setters, and named constructors are wrapped
                // in a "method_signature" child node.
                let has_method_sig = (0..node.child_count())
                    .any(|i| matches!(node.child(i).map(|c| c.kind()), Some("method_signature")));
                if has_method_sig {
                    return NodeKind::Method;
                }
                // Primary constructors (e.g. `ClassName(...)`) use a "declaration"
                // child that itself contains a "constructor_signature" grandchild.
                let is_constructor =
                    (0..node.child_count())
                        .filter_map(|i| node.child(i))
                        .any(|decl| {
                            decl.kind() == "declaration"
                                && (0..decl.child_count())
                                    .filter_map(|j| decl.child(j))
                                    .any(|gc| gc.kind() == "constructor_signature")
                        });
                if is_constructor {
                    NodeKind::Method
                } else {
                    NodeKind::Variable
                }
            }

            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["dart"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/dart/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/dart/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/dart/references.scm")
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
            // tree-sitter-dart 0.0.4 uses "lambda_expression" for top-level named
            // function declarations. Despite the name, this is NOT a lambda/closure —
            // it is the grammar's node type for any named top-level function body.
            "lambda_expression",
            "function_signature",
            "getter_signature",
            "setter_signature",
            "constructor_signature",
        ]
    }

    fn function_body_kind(&self) -> &'static str {
        "function_body"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "for_in_parts",
            "while_statement",
            "do_statement",
            "switch_statement",
            "try_statement",
        ]
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "expression_statement",
            "local_variable_declaration",
            "if_statement",
            "for_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "try_statement",
            "return_statement",
            "block",
        ]
    }

    // --- call_detection ---
    /// tree-sitter-dart 0.0.4 has no dedicated call node: a function/method call
    /// is a `member_access` carrying a `selector`/`argument_part`. Constructor
    /// calls use `(explicit_)constructor_invocation`. These are the real
    /// call-bearing node kinds.
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &[
            "member_access",
            "constructor_invocation",
            "explicit_constructor_invocation",
        ]
    }

    // --- effect_summary ---
    /// `throw e` parses as `throw_expression`.
    fn raise_node_kinds(&self) -> &'static [&'static str] {
        &["throw_expression"]
    }

    /// `await x` suspends.
    fn await_node_kinds(&self) -> &'static [&'static str] {
        &["await_expression"]
    }

    /// Constructor invocations allocate.
    fn allocation_node_kinds(&self) -> &'static [&'static str] {
        &["constructor_invocation", "explicit_constructor_invocation"]
    }

    /// Future/isolate blocking helpers.
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["sleep", "wait", "then", "join"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("raise")
            .with("await")
            .with("allocate")
    }

    // --- data_flow ---
    /// `var x = ...` binds via `local_variable_declaration` /
    /// `initialized_variable_definition`.
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &[
            "local_variable_declaration",
            "initialized_variable_definition",
        ]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    // --- unsafe_surface ---
    /// `dart:mirrors` reflection + `Function.apply` dynamic invoke.
    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        &["invoke", "apply", "reflect", "reflectClass"]
    }

    /// `dart:ffi` foreign-function interface.
    fn ffi_markers(&self) -> &'static [&'static str] {
        &["DynamicLibrary", "lookupFunction", "lookup", "Native"]
    }

    /// Raw SQL sinks (`sqflite`/`drift` rawQuery/execute).
    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &["rawQuery", "rawInsert", "rawUpdate", "rawDelete", "execute"]
    }

    /// Untrusted deserialization.
    fn unsafe_deserialize_names(&self) -> &'static [&'static str] {
        &["jsonDecode", "decode"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        // Dart has no `eval`/dynamic code-string execution in the core runtime;
        // `eval` is intentionally omitted.
        act_lite_core::KindMask::new()
            .with("raw_sql")
            .with("ffi")
            .with("reflection_invoke")
            .with("unsafe_deserialize")
    }

    // --- secret_surface ---
    /// `package:crypto` / `dart:io` HMAC/hash/cipher APIs.
    fn crypto_api_names(&self) -> &'static [&'static str] {
        &["Hmac", "sha256", "md5", "convert", "sign", "encrypt"]
    }

    /// `Platform.environment` reads environment secrets.
    fn env_read_names(&self) -> &'static [&'static str] {
        &["environment", "fromEnvironment"]
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
    /// HTTP request inputs + IO sources.
    fn taint_source_names(&self) -> &'static [&'static str] {
        &[
            "queryParameters",
            "headers",
            "readAsString",
            "stdin",
            "environment",
        ]
    }

    /// OS process execution (`Process.run`/`Process.start`).
    fn command_exec_names(&self) -> &'static [&'static str] {
        &["run", "start", "runSync", "Process"]
    }

    /// Filesystem sinks.
    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["File", "openWrite", "writeAsString", "Directory"]
    }

    /// Escaping/sanitizers.
    fn sanitizer_names(&self) -> &'static [&'static str] {
        &["htmlEscape", "escape", "sanitize"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use act_lite_core::registry::{LANGUAGE_REGISTRY, LanguageId};

    #[test]
    fn registry_has_dart() {
        let meta = LANGUAGE_REGISTRY.get(LanguageId("dart"));
        assert!(meta.is_some(), "Dart should be registered");
    }

    #[test]
    fn registry_extension_lookup_dart() {
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("dart"),
            Some(LanguageId("dart"))
        );
    }

    #[test]
    fn grammar() {
        let support = DartSupport;
        assert_eq!(support.grammar(), Grammar::Dart);
    }

    #[test]
    fn extensions() {
        let support = DartSupport;
        assert_eq!(support.extensions(), &["dart"]);
    }

    #[test]
    fn ts_language_returns_valid_language() {
        let support = DartSupport;
        let lang = support.ts_language();
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&lang).is_ok());
    }

    #[test]
    fn comment_delimiters_are_set() {
        let support = DartSupport;
        let delimiters = support.comment_delimiters();
        assert_eq!(delimiters.line, Some("//"));
        assert_eq!(delimiters.block_start, Some("/*"));
        assert_eq!(delimiters.block_end, Some("*/"));
    }

    #[test]
    fn skeleton_query_compiles() {
        let support = DartSupport;
        let lang = support.ts_language();
        let query_source = support.skeleton_query();
        assert!(!query_source.is_empty());
        tree_sitter::Query::new(&lang, query_source).unwrap();
    }

    #[test]
    fn symbols_query_compiles() {
        let support = DartSupport;
        let lang = support.ts_language();
        let query_source = support.symbols_query();
        assert!(!query_source.is_empty());
        tree_sitter::Query::new(&lang, query_source).unwrap();
    }

    #[test]
    fn references_query_compiles() {
        let support = DartSupport;
        let lang = support.ts_language();
        let query_source = support.references_query();
        assert!(!query_source.is_empty());
        tree_sitter::Query::new(&lang, query_source).unwrap();
    }

    #[test]
    fn function_parent_kinds_are_dart_specific() {
        let support = DartSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"lambda_expression"));
        assert!(kinds.contains(&"function_signature"));
        assert!(kinds.contains(&"getter_signature"));
        assert!(kinds.contains(&"setter_signature"));
        assert!(kinds.contains(&"constructor_signature"));
        assert!(!kinds.contains(&"arrow_function"));
        assert!(!kinds.contains(&"method_definition"));
    }

    #[test]
    fn function_body_kind_is_function_body() {
        let support = DartSupport;
        assert_eq!(support.function_body_kind(), "function_body");
    }

    #[test]
    fn decision_point_kinds_are_dart_specific() {
        let support = DartSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_statement"));
        assert!(kinds.contains(&"for_statement"));
        assert!(kinds.contains(&"for_in_parts"));
        assert!(kinds.contains(&"while_statement"));
        assert!(kinds.contains(&"do_statement"));
        assert!(kinds.contains(&"switch_statement"));
        assert!(kinds.contains(&"try_statement"));
        assert!(!kinds.contains(&"ternary_expression"));
    }

    #[test]
    fn node_kind_unknown_for_unrecognized() {
        let support = DartSupport;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&support.ts_language())
            .expect("failed to set Dart language");
        let tree = parser
            .parse("void main() {}", None)
            .expect("failed to parse");
        let root = tree.root_node();
        assert_eq!(support.node_kind(&root), NodeKind::Unknown);
    }

    fn parse(code: &str) -> tree_sitter::Tree {
        let support = DartSupport;
        crate::test_utils::parse_with_support(&support, code)
    }

    #[test]
    fn node_kind_function() {
        let support = DartSupport;
        let tree = parse("void greet(String name) { print(name); }");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_class() {
        let support = DartSupport;
        let tree = parse("class Foo {}");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(node.kind(), "class_definition");
        assert_eq!(support.node_kind(&node), NodeKind::Class);
    }

    #[test]
    fn node_kind_mixin() {
        let support = DartSupport;
        let tree = parse("mixin Flyable on Animal {}");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(node.kind(), "mixin_declaration");
        assert_eq!(support.node_kind(&node), NodeKind::Class);
    }

    #[test]
    fn node_kind_extension() {
        let support = DartSupport;
        let tree = parse("extension StringExt on String {}");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(node.kind(), "extension_declaration");
        assert_eq!(support.node_kind(&node), NodeKind::Class);
    }

    #[test]
    fn node_kind_enum() {
        let support = DartSupport;
        let tree = parse("enum Color { red, green, blue }");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(node.kind(), "enum_declaration");
        assert_eq!(support.node_kind(&node), NodeKind::Enum);
    }

    #[test]
    fn node_kind_import() {
        let support = DartSupport;
        let tree = parse("import 'dart:core';");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(node.kind(), "import_or_export");
        assert_eq!(support.node_kind(&node), NodeKind::Import);
    }

    #[test]
    fn node_kind_type_alias() {
        let support = DartSupport;
        let tree = parse("typedef StringMap = Map<String, String>;");
        let node = tree.root_node().child(0).unwrap();
        assert_eq!(node.kind(), "type_alias");
        assert_eq!(support.node_kind(&node), NodeKind::TypeAlias);
    }

    #[test]
    fn type_alias_general_form_parses_clean() {
        let tree = parse("class User {}\ntypedef OptionalUser = User;");
        let root = tree.root_node();
        let node = root.child(1).unwrap();
        assert_eq!(node.kind(), "type_alias");
        assert!(
            !root.has_error(),
            "general type alias should parse without errors: {root}"
        );
    }

    #[test]
    fn type_alias_general_form_symbols_captures_alias_name_only() {
        use crate::{Grammar, QueryResult, SourceFile, execute_symbols};
        use act_lite_core::SymbolKind;
        use std::path::PathBuf;

        let file = SourceFile::from_content(
            PathBuf::from("test.dart"),
            "class User {}\ntypedef OptionalUser = User;".to_string(),
            Grammar::Dart,
        )
        .unwrap();
        let result = execute_symbols(&file).unwrap();

        if let QueryResult::Symbols { symbols } = result {
            let aliases: Vec<_> = symbols
                .iter()
                .filter(|s| s.kind() == SymbolKind::TypeAlias)
                .collect();
            assert_eq!(
                aliases.len(),
                1,
                "expected exactly one type_alias definition, got {symbols:?}"
            );
            assert_eq!(aliases[0].name(), "OptionalUser");
        } else {
            panic!("Expected Symbols result");
        }
    }

    #[test]
    fn node_kind_method() {
        let support = DartSupport;
        let tree = parse("class X { void doWork() {} }");
        let class_node = tree.root_node().child(0).unwrap();
        // class_body > class_member_definition
        let body = (0..class_node.child_count())
            .find_map(|i| {
                let c = class_node.child(i).unwrap();
                if c.kind() == "class_body" {
                    Some(c)
                } else {
                    None
                }
            })
            .unwrap();
        let member = (0..body.child_count())
            .find_map(|i| {
                let c = body.child(i).unwrap();
                if c.kind() == "class_member_definition" {
                    Some(c)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(support.node_kind(&member), NodeKind::Method);
    }

    #[test]
    fn node_kind_field() {
        let support = DartSupport;
        let tree = parse("class X { final String name; }");
        let class_node = tree.root_node().child(0).unwrap();
        let body = (0..class_node.child_count())
            .find_map(|i| {
                let c = class_node.child(i).unwrap();
                if c.kind() == "class_body" {
                    Some(c)
                } else {
                    None
                }
            })
            .unwrap();
        let member = (0..body.child_count())
            .find_map(|i| {
                let c = body.child(i).unwrap();
                if c.kind() == "class_member_definition" {
                    Some(c)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(support.node_kind(&member), NodeKind::Variable);
    }

    #[test]
    fn node_kind_constructor() {
        let support = DartSupport;
        let tree = parse("class X { X(String url) : _url = url; }");
        let class_node = tree.root_node().child(0).unwrap();
        let body = (0..class_node.child_count())
            .find_map(|i| {
                let c = class_node.child(i).unwrap();
                if c.kind() == "class_body" {
                    Some(c)
                } else {
                    None
                }
            })
            .unwrap();
        let member = (0..body.child_count())
            .find_map(|i| {
                let c = body.child(i).unwrap();
                if c.kind() == "class_member_definition" {
                    Some(c)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(support.node_kind(&member), NodeKind::Method);
    }

    #[test]
    fn node_kind_mapping_table() {
        // Table-driven test verifying the complete node_kind mapping
        let support = DartSupport;

        let cases: &[(&str, NodeKind)] = &[
            ("void greet() {}", NodeKind::Function),
            ("class Foo {}", NodeKind::Class),
            ("mixin M on Object {}", NodeKind::Class),
            ("extension E on int {}", NodeKind::Class),
            ("enum E { a }", NodeKind::Enum),
            ("import 'dart:core';", NodeKind::Import),
            ("typedef F = void Function();", NodeKind::TypeAlias),
        ];

        for (code, expected) in cases {
            let tree = parse(code);
            let node = tree.root_node().child(0).unwrap();
            assert_eq!(
                support.node_kind(&node),
                *expected,
                "node kind mismatch for '{}': node.kind() = {}",
                code,
                node.kind()
            );
        }
    }

    #[test]
    fn extractable_node_kinds_covers_dart_statements() {
        let support = DartSupport;
        let kinds = support.extractable_node_kinds();
        assert!(!kinds.is_empty(), "Dart must define extractable_node_kinds");
        assert!(
            kinds.contains(&"expression_statement"),
            "must include expression_statement"
        );
        assert!(
            kinds.contains(&"local_variable_declaration"),
            "must include local_variable_declaration"
        );
        assert!(kinds.contains(&"if_statement"), "must include if_statement");
        assert!(
            kinds.contains(&"return_statement"),
            "must include return_statement"
        );
    }

    #[test]
    fn coverage_hooks_dart() {
        use crate::test_utils::find_node_by_kind;
        let s = DartSupport;
        let tree =
            parse("Future<int> f() async { foo(); throw Exception(); await bar(); return 1; }");
        // calls appear as member_access carrying a selector/argument_part.
        assert!(find_node_by_kind(tree.root_node(), "member_access").is_some());
        assert!(find_node_by_kind(tree.root_node(), "throw_expression").is_some());
        assert!(find_node_by_kind(tree.root_node(), "await_expression").is_some());
        assert!(s.call_node_kinds().contains(&"member_access"));
        assert_ne!(s.call_node_kinds(), ["call_expression", "call"]);
        assert!(s.raise_node_kinds().contains(&"throw_expression"));
        assert!(s.await_node_kinds().contains(&"await_expression"));
        assert!(s.effect_modeled_kinds().contains("await"));
        assert!(
            s.binding_node_kinds()
                .contains(&"local_variable_declaration")
        );
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
        // unsafe_surface — ffi + reflection, no eval in core runtime.
        assert!(s.ffi_markers().contains(&"DynamicLibrary"));
        assert!(s.unsafe_modeled_kinds().contains("ffi"));
        assert!(!s.unsafe_modeled_kinds().contains("eval"));
        // secret_surface
        assert!(s.env_read_names().contains(&"environment"));
        assert!(s.secret_modeled_kinds().contains("env_secret_read"));
        // taint_flow
        assert!(s.command_exec_names().contains(&"run"));
        assert!(s.taint_modeled_kinds().contains("taint"));
    }

    fn type_alias_symbols(code: &str) -> Vec<act_lite_core::Symbol> {
        use crate::{Grammar, QueryResult, SourceFile, execute_symbols};
        use act_lite_core::SymbolKind;
        use std::path::PathBuf;

        let file =
            SourceFile::from_content(PathBuf::from("test.dart"), code.to_string(), Grammar::Dart)
                .unwrap();
        let result = execute_symbols(&file).unwrap();

        match result {
            QueryResult::Symbols { symbols } => symbols
                .into_iter()
                .filter(|s| s.kind() == SymbolKind::TypeAlias)
                .collect(),
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn type_alias_function_form_symbols_captures_alias_name_only() {
        let aliases = type_alias_symbols("typedef Callback = void Function(int value);");
        assert_eq!(
            aliases.len(),
            1,
            "expected exactly one type_alias definition, got {aliases:?}"
        );
        assert_eq!(aliases[0].name(), "Callback");
    }

    #[test]
    fn type_alias_legacy_form_symbols_captures_alias_name_only() {
        let aliases = type_alias_symbols("typedef void LegacyCallback(int value);");
        assert_eq!(
            aliases.len(),
            1,
            "expected exactly one type_alias definition, got {aliases:?}"
        );
        assert_eq!(aliases[0].name(), "LegacyCallback");
    }

    #[test]
    fn type_alias_legacy_named_return_symbols_captures_alias_name_only() {
        let aliases = type_alias_symbols("typedef SomeType Cb(int x);");
        assert_eq!(
            aliases.len(),
            1,
            "expected exactly one type_alias definition, got {aliases:?}"
        );
        assert_eq!(aliases[0].name(), "Cb");
    }

    #[test]
    fn type_alias_generic_form_symbols_captures_alias_name_only() {
        let aliases = type_alias_symbols("typedef Box<T> = List<T>;");
        assert_eq!(
            aliases.len(),
            1,
            "expected exactly one type_alias definition, got {aliases:?}"
        );
        assert_eq!(aliases[0].name(), "Box");
    }

    #[test]
    fn type_alias_generic_function_form_symbols_captures_alias_name_only() {
        let aliases = type_alias_symbols("typedef Callback<T> = void Function(T);");
        assert_eq!(
            aliases.len(),
            1,
            "expected exactly one type_alias definition, got {aliases:?}"
        );
        assert_eq!(aliases[0].name(), "Callback");
    }

    #[test]
    fn type_alias_generic_legacy_form_symbols_captures_alias_name_only() {
        let aliases = type_alias_symbols("typedef bool Predicate<T>(T value);");
        assert_eq!(
            aliases.len(),
            1,
            "expected exactly one type_alias definition, got {aliases:?}"
        );
        assert_eq!(aliases[0].name(), "Predicate");
    }

    #[test]
    fn type_alias_forms_skeleton_captures_alias_name_only() {
        use crate::{Grammar, QueryResult, SourceFile, execute_skeleton};
        use std::path::PathBuf;

        for (code, expected) in [
            ("typedef OptionalUser = User;", "OptionalUser"),
            ("typedef StringMap = Map<String, String>;", "StringMap"),
            ("typedef Box<T> = List<T>;", "Box"),
            ("typedef Callback = void Function(int value);", "Callback"),
            ("typedef Callback<T> = void Function(T);", "Callback"),
            ("typedef void LegacyCallback(int value);", "LegacyCallback"),
            ("typedef SomeType Cb(int x);", "Cb"),
            ("typedef bool Predicate<T>(T value);", "Predicate"),
            // Edge shapes.
            ("typedef F = void Function(int a, String b);", "F"),
            ("typedef User = User;", "User"),
            ("typedef X = Y?;", "X"),
            // Comment-placement matrix (whole-row-review round 1).
            ("typedef X /* mid comment */ = Y;", "X"),
            ("typedef Box<T> /* c */ = List<T>;", "Box"),
            ("typedef void Cb /* c */ (int x);", "Cb"),
            ("typedef SomeType Cb<T> /* c */ (T x);", "Cb"),
            ("typedef X // trailing\n = Y;", "X"),
            ("typedef /* c */ X = Y;", "X"),
            ("typedef X /* a */ /* b */ = Y;", "X"),
            // documentation_comment is a distinct extras kind (whole-row
            // review round 2).
            ("typedef X /// doc\n = Y;", "X"),
            ("typedef Box<T> /// doc\n = List<T>;", "Box"),
            ("typedef SomeType Cb<T> /// doc\n (T x);", "Cb"),
        ] {
            let file = SourceFile::from_content(
                PathBuf::from("test.dart"),
                code.to_string(),
                Grammar::Dart,
            )
            .unwrap();
            let result = execute_skeleton(&file).unwrap();
            let QueryResult::Skeleton(skeleton) = result else {
                panic!("Expected Skeleton result");
            };
            let aliases: Vec<_> = skeleton
                .declarations
                .iter()
                .filter(|d| d.kind == "type_alias")
                .collect();
            assert_eq!(
                aliases.len(),
                1,
                "expected exactly one type_alias declaration for {code:?}, got {aliases:?}"
            );
            assert_eq!(aliases[0].name, expected, "wrong name for {code:?}");
        }
    }

    #[test]
    fn type_alias_comment_forms_symbols_captures_alias_name_only() {
        for (code, expected) in [
            ("typedef X /* mid comment */ = Y;", "X"),
            ("typedef Box<T> /* c */ = List<T>;", "Box"),
            ("typedef void Cb /* c */ (int x);", "Cb"),
            ("typedef SomeType Cb<T> /* c */ (T x);", "Cb"),
            ("typedef X // trailing\n = Y;", "X"),
            ("typedef /* c */ X = Y;", "X"),
            ("typedef X /* a */ /* b */ = Y;", "X"),
            ("typedef Box /* c1 */ <T> = List<T>;", "Box"),
            // documentation_comment is a DISTINCT extras kind (grammar.js
            // extras: comment, documentation_comment, whitespace) and
            // interposes at the same slots.
            ("typedef X /// doc\n = Y;", "X"),
            ("typedef Box<T> /// doc\n = List<T>;", "Box"),
            ("typedef SomeType Cb<T> /// doc\n (T x);", "Cb"),
            ("typedef X /** doc block */ = Y;", "X"),
            ("typedef X /* c */ /// doc\n = Y;", "X"),
        ] {
            let aliases = type_alias_symbols(code);
            assert_eq!(
                aliases.len(),
                1,
                "expected exactly one type_alias definition for {code:?}, got {aliases:?}"
            );
            assert_eq!(aliases[0].name(), expected, "wrong name for {code:?}");
        }
    }

    #[test]
    fn type_alias_edge_forms_symbols_captures_alias_name_only() {
        for (code, expected) in [
            ("typedef F = void Function(int a, String b);", "F"),
            ("typedef User = User;", "User"),
            ("typedef X = Y?;", "X"),
        ] {
            let aliases = type_alias_symbols(code);
            assert_eq!(
                aliases.len(),
                1,
                "expected exactly one type_alias definition for {code:?}, got {aliases:?}"
            );
            assert_eq!(aliases[0].name(), expected, "wrong name for {code:?}");
        }
    }
}
