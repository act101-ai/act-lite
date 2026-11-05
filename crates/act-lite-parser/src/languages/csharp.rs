//! C# language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register C# in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("csharp"),
        display_name: "C#",
        extensions: &["cs", "csx"],
        category: LanguageCategory::Managed,
    }
}

/// C# language support implementation.
#[derive(Default)]
pub struct CSharpSupport;

impl LanguageSupport for CSharpSupport {
    fn grammar(&self) -> Grammar {
        Grammar::CSharp
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_c_sharp::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Types
            "class_declaration" => NodeKind::Class,
            "record_declaration" => NodeKind::Class,
            "struct_declaration" => NodeKind::Class,
            "interface_declaration" => NodeKind::Interface,
            "enum_declaration" => NodeKind::Enum,
            "delegate_declaration" => NodeKind::TypeAlias,

            // Members
            "method_declaration" => NodeKind::Method,
            "constructor_declaration" => NodeKind::Constructor,
            "property_declaration" => NodeKind::Property,
            "field_declaration" => NodeKind::Property,
            "event_declaration" | "event_field_declaration" => NodeKind::Event,
            "indexer_declaration" => NodeKind::Property,
            "operator_declaration" | "conversion_operator_declaration" => NodeKind::Method,

            // Enum members
            "enum_member_declaration" => NodeKind::Property,

            // Parameters and variables
            "parameter" => NodeKind::Parameter,
            "type_parameter" => NodeKind::TypeParameter,
            "variable_declarator" => NodeKind::Variable,
            "local_function_statement" => NodeKind::Function,

            // Namespaces and using
            "namespace_declaration" | "file_scoped_namespace_declaration" => NodeKind::Module,
            "using_directive" => NodeKind::Import,

            // Blocks
            "block" | "declaration_list" => NodeKind::Block,

            // Expressions and calls
            "invocation_expression" | "object_creation_expression" => NodeKind::Call,
            "lambda_expression" | "anonymous_method_expression" => NodeKind::Function,

            // Identifiers
            "identifier" | "qualified_name" | "generic_name" => NodeKind::Identifier,

            // Comments
            "comment" => NodeKind::Comment,

            // Attributes (decorators)
            "attribute" | "attribute_list" => NodeKind::Decorator,

            // Types
            "nullable_type" | "array_type" | "pointer_type" | "tuple_type" => NodeKind::Type,
            "predefined_type" | "type_argument_list" => NodeKind::Type,

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cs", "csx"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/csharp/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/csharp/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/csharp/references.scm")
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
            "method_declaration",
            "constructor_declaration",
            "local_function_statement",
            "lambda_expression",
            "anonymous_method_expression",
        ]
    }

    fn function_body_kind(&self) -> &'static str {
        "block"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "switch_expression",
            "catch_clause",
            "conditional_expression",
        ]
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "block",
            "expression_statement",
            "local_declaration_statement",
            "return_statement",
            "if_statement",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "switch_expression",
            "try_statement",
            "using_statement",
            "throw_statement",
            "lock_statement",
            "yield_statement",
            "break_statement",
            "continue_statement",
            "goto_statement",
            "labeled_statement",
            "checked_expression",
            "unchecked_expression",
            "fixed_statement",
            "unsafe_statement",
            "invocation_expression",
            "binary_expression",
            "conditional_expression",
            "unary_expression",
            "cast_expression",
            "is_expression",
            "is_pattern_expression",
            "lambda_expression",
            "anonymous_method_expression",
            "member_access_expression",
            "element_access_expression",
            "object_creation_expression",
            "implicit_object_creation_expression",
            "array_creation_expression",
            "parenthesized_expression",
            "assignment_expression",
            "await_expression",
            "query_expression",
            "class_declaration",
            "struct_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "method_declaration",
            "constructor_declaration",
            "field_declaration",
            "property_declaration",
            "namespace_declaration",
        ]
    }

    // --- call_detection ---
    /// C# calls are `invocation_expression`; `new X()` is `object_creation_expression`.
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &[
            "invocation_expression",
            "object_creation_expression",
            "implicit_object_creation_expression",
        ]
    }

    // --- effect_summary ---
    /// `throw e` / `throw;` parses as `throw_statement` (or `throw_expression`).
    fn raise_node_kinds(&self) -> &'static [&'static str] {
        &["throw_statement", "throw_expression"]
    }

    /// `await x` suspends.
    fn await_node_kinds(&self) -> &'static [&'static str] {
        &["await_expression"]
    }

    /// `new` allocations.
    fn allocation_node_kinds(&self) -> &'static [&'static str] {
        &[
            "object_creation_expression",
            "implicit_object_creation_expression",
            "array_creation_expression",
        ]
    }

    /// TPL/threading blocking calls.
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["Wait", "Sleep", "Join", "GetResult", "Result", "WaitOne"]
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
    /// `var x = ...` binds via `variable_declaration` / `variable_declarator`.
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &[
            "local_declaration_statement",
            "variable_declaration",
            "variable_declarator",
        ]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    // --- unsafe_surface ---
    /// `unsafe { ... }` / fixed pointer regions are real unsafe AST nodes.
    fn unsafe_node_kinds(&self) -> &'static [&'static str] {
        &["unsafe_statement", "fixed_statement", "pointer_type"]
    }

    /// Roslyn scripting / dynamic eval.
    fn eval_call_names(&self) -> &'static [&'static str] {
        &["Eval", "EvaluateAsync", "RunAsync", "Compile"]
    }

    /// P/Invoke FFI markers.
    fn ffi_markers(&self) -> &'static [&'static str] {
        &["DllImport", "LoadLibrary", "Marshal", "extern"]
    }

    /// ADO.NET raw SQL sinks (`SqlCommand.ExecuteReader`, etc.).
    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &[
            "ExecuteReader",
            "ExecuteNonQuery",
            "ExecuteScalar",
            "FromSqlRaw",
            "ExecuteSqlRaw",
        ]
    }

    /// Reflective invocation: `Assembly.Load`, `Type.GetMethod`, `MethodInfo.Invoke`.
    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        &["Invoke", "GetMethod", "CreateInstance", "Load", "GetType"]
    }

    /// `BinaryFormatter.Deserialize`, `XmlSerializer`, `LosFormatter`.
    fn unsafe_deserialize_names(&self) -> &'static [&'static str] {
        &["Deserialize", "ReadObject", "BinaryFormatter"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("unsafe_block")
            .with("eval")
            .with("raw_sql")
            .with("ffi")
            .with("reflection_invoke")
            .with("unsafe_deserialize")
    }

    // --- secret_surface ---
    /// `System.Security.Cryptography` APIs.
    fn crypto_api_names(&self) -> &'static [&'static str] {
        &[
            "Create",
            "ComputeHash",
            "HMACSHA256",
            "Aes",
            "RSA",
            "SignData",
            "ProtectedData",
        ]
    }

    /// `Environment.GetEnvironmentVariable`.
    fn env_read_names(&self) -> &'static [&'static str] {
        &["GetEnvironmentVariable", "GetEnvironmentVariables"]
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
    /// ASP.NET request inputs + IO.
    fn taint_source_names(&self) -> &'static [&'static str] {
        &[
            "Request",
            "Query",
            "Form",
            "Headers",
            "ReadLine",
            "GetEnvironmentVariable",
        ]
    }

    /// OS command execution (`Process.Start`).
    fn command_exec_names(&self) -> &'static [&'static str] {
        &["Start", "Process"]
    }

    /// Filesystem sinks.
    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["File", "FileStream", "StreamWriter", "Path", "OpenRead"]
    }

    /// Encoding/sanitizers.
    fn sanitizer_names(&self) -> &'static [&'static str] {
        &["HtmlEncode", "Encode", "Sanitize"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};
    use act_lite_core::registry::LANGUAGE_REGISTRY;

    crate::language_support_tests!(CSharpSupport, Grammar::CSharp, &["cs", "csx"]);

    #[test]
    fn registry_has_csharp() {
        let meta = LANGUAGE_REGISTRY.get(LanguageId("csharp"));
        assert!(meta.is_some(), "C# should be registered");
        let meta = meta.unwrap();
        assert_eq!(meta.display_name, "C#");
        assert!(meta.extensions.contains(&"cs"));
        assert!(meta.extensions.contains(&"csx"));
    }

    #[test]
    fn registry_extension_lookup() {
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("cs"),
            Some(LanguageId("csharp"))
        );
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("csx"),
            Some(LanguageId("csharp"))
        );
    }

    #[test]
    fn node_kind_class_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "public class Foo {}");
        let class_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(support.node_kind(&class_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_record_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "public record Person(string Name);");
        let record_node = find_node_by_kind(tree.root_node(), "record_declaration").unwrap();
        assert_eq!(support.node_kind(&record_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_struct_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(
            &support,
            "public struct Point { public int X; public int Y; }",
        );
        let struct_node = find_node_by_kind(tree.root_node(), "struct_declaration").unwrap();
        assert_eq!(support.node_kind(&struct_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_interface_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "public interface IFoo {}");
        let iface_node = find_node_by_kind(tree.root_node(), "interface_declaration").unwrap();
        assert_eq!(support.node_kind(&iface_node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_enum_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "public enum Status { Active, Inactive }");
        let enum_node = find_node_by_kind(tree.root_node(), "enum_declaration").unwrap();
        assert_eq!(support.node_kind(&enum_node), NodeKind::Enum);
    }

    #[test]
    fn node_kind_method_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { void Bar() {} }");
        let method_node = find_node_by_kind(tree.root_node(), "method_declaration").unwrap();
        assert_eq!(support.node_kind(&method_node), NodeKind::Method);
    }

    #[test]
    fn node_kind_constructor_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { Foo() {} }");
        let ctor_node = find_node_by_kind(tree.root_node(), "constructor_declaration").unwrap();
        assert_eq!(support.node_kind(&ctor_node), NodeKind::Constructor);
    }

    #[test]
    fn node_kind_property_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { public int Bar { get; set; } }");
        let prop_node = find_node_by_kind(tree.root_node(), "property_declaration").unwrap();
        assert_eq!(support.node_kind(&prop_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_namespace_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "namespace Foo.Bar {}");
        let ns_node = find_node_by_kind(tree.root_node(), "namespace_declaration").unwrap();
        assert_eq!(support.node_kind(&ns_node), NodeKind::Module);
    }

    #[test]
    fn node_kind_file_scoped_namespace() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "namespace Foo.Bar;");
        let ns_node =
            find_node_by_kind(tree.root_node(), "file_scoped_namespace_declaration").unwrap();
        assert_eq!(support.node_kind(&ns_node), NodeKind::Module);
    }

    #[test]
    fn node_kind_using_directive() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "using System;");
        let using_node = find_node_by_kind(tree.root_node(), "using_directive").unwrap();
        assert_eq!(support.node_kind(&using_node), NodeKind::Import);
    }

    #[test]
    fn node_kind_invocation_expression() {
        let support = CSharpSupport;
        let tree = parse_with_support(
            &support,
            "class Foo { void Bar() { Console.WriteLine(); } }",
        );
        let call_node = find_node_by_kind(tree.root_node(), "invocation_expression").unwrap();
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_lambda_expression() {
        let support = CSharpSupport;
        let tree = parse_with_support(
            &support,
            "class Foo { System.Func<int, int> f = x => x + 1; }",
        );
        let lambda_node = find_node_by_kind(tree.root_node(), "lambda_expression").unwrap();
        assert_eq!(support.node_kind(&lambda_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_identifier() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo {}");
        let ident_node = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        assert_eq!(support.node_kind(&ident_node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_comment() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "// comment\nclass Foo {}");
        let comment_node = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_attribute() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "[Serializable] class Foo {}");
        let attr_node = find_node_by_kind(tree.root_node(), "attribute").unwrap();
        assert_eq!(support.node_kind(&attr_node), NodeKind::Decorator);
    }

    #[test]
    fn node_kind_parameter() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { void Bar(int x) {} }");
        let param_node = find_node_by_kind(tree.root_node(), "parameter").unwrap();
        assert_eq!(support.node_kind(&param_node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_delegate_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "public delegate void Handler(object sender);");
        let delegate_node = find_node_by_kind(tree.root_node(), "delegate_declaration").unwrap();
        assert_eq!(support.node_kind(&delegate_node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_field_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { private int _value; }");
        let field_node = find_node_by_kind(tree.root_node(), "field_declaration").unwrap();
        assert_eq!(support.node_kind(&field_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_event_field_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { public event EventHandler Changed; }");
        let event_node = find_node_by_kind(tree.root_node(), "event_field_declaration").unwrap();
        assert_eq!(support.node_kind(&event_node), NodeKind::Event);
    }

    #[test]
    fn node_kind_indexer_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(
            &support,
            "class Foo { public int this[int i] { get { return 0; } } }",
        );
        let indexer_node = find_node_by_kind(tree.root_node(), "indexer_declaration").unwrap();
        assert_eq!(support.node_kind(&indexer_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_operator_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(
            &support,
            "class Foo { public static Foo operator +(Foo a, Foo b) { return a; } }",
        );
        let op_node = find_node_by_kind(tree.root_node(), "operator_declaration").unwrap();
        assert_eq!(support.node_kind(&op_node), NodeKind::Method);
    }

    #[test]
    fn node_kind_enum_member_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "enum Status { Active, Inactive }");
        let member_node = find_node_by_kind(tree.root_node(), "enum_member_declaration").unwrap();
        assert_eq!(support.node_kind(&member_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_type_parameter() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo<T> {}");
        let type_param_node = find_node_by_kind(tree.root_node(), "type_parameter").unwrap();
        assert_eq!(support.node_kind(&type_param_node), NodeKind::TypeParameter);
    }

    #[test]
    fn node_kind_variable_declarator() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { void Bar() { int x = 1; } }");
        let var_node = find_node_by_kind(tree.root_node(), "variable_declarator").unwrap();
        assert_eq!(support.node_kind(&var_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_local_function_statement() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { void Bar() { void Local() {} } }");
        let local_fn_node =
            find_node_by_kind(tree.root_node(), "local_function_statement").unwrap();
        assert_eq!(support.node_kind(&local_fn_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_block() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { void Bar() { } }");
        let block_node = find_node_by_kind(tree.root_node(), "block").unwrap();
        assert_eq!(support.node_kind(&block_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_declaration_list() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { }");
        let decl_list_node = find_node_by_kind(tree.root_node(), "declaration_list").unwrap();
        assert_eq!(support.node_kind(&decl_list_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_object_creation_expression() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { object x = new object(); }");
        let new_node = find_node_by_kind(tree.root_node(), "object_creation_expression").unwrap();
        assert_eq!(support.node_kind(&new_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_qualified_name() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "using System.Collections;");
        let qname_node = find_node_by_kind(tree.root_node(), "qualified_name").unwrap();
        assert_eq!(support.node_kind(&qname_node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_generic_name() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { List<int> items; }");
        let generic_node = find_node_by_kind(tree.root_node(), "generic_name").unwrap();
        assert_eq!(support.node_kind(&generic_node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_attribute_list() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "[Obsolete] class Foo {}");
        let attr_list_node = find_node_by_kind(tree.root_node(), "attribute_list").unwrap();
        assert_eq!(support.node_kind(&attr_list_node), NodeKind::Decorator);
    }

    #[test]
    fn node_kind_nullable_type() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { int? value; }");
        let nullable_node = find_node_by_kind(tree.root_node(), "nullable_type").unwrap();
        assert_eq!(support.node_kind(&nullable_node), NodeKind::Type);
    }

    #[test]
    fn node_kind_array_type() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { int[] items; }");
        let array_node = find_node_by_kind(tree.root_node(), "array_type").unwrap();
        assert_eq!(support.node_kind(&array_node), NodeKind::Type);
    }

    #[test]
    fn node_kind_predefined_type() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { int value; }");
        let predef_node = find_node_by_kind(tree.root_node(), "predefined_type").unwrap();
        assert_eq!(support.node_kind(&predef_node), NodeKind::Type);
    }

    #[test]
    fn node_kind_type_argument_list() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { List<int> items; }");
        let type_args_node = find_node_by_kind(tree.root_node(), "type_argument_list").unwrap();
        assert_eq!(support.node_kind(&type_args_node), NodeKind::Type);
    }

    #[test]
    fn node_kind_unknown_fallback() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { }");
        // compilation_unit is the root and should be Unknown
        let root = tree.root_node();
        assert_eq!(support.node_kind(&root), NodeKind::Unknown);
    }

    #[test]
    fn node_kind_event_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(
            &support,
            "class Foo { public event EventHandler Changed { add {} remove {} } }",
        );
        let event_node = find_node_by_kind(tree.root_node(), "event_declaration").unwrap();
        assert_eq!(support.node_kind(&event_node), NodeKind::Event);
    }

    #[test]
    fn node_kind_conversion_operator_declaration() {
        let support = CSharpSupport;
        let tree = parse_with_support(
            &support,
            "class Foo { public static implicit operator int(Foo f) { return 0; } }",
        );
        let conv_op_node =
            find_node_by_kind(tree.root_node(), "conversion_operator_declaration").unwrap();
        assert_eq!(support.node_kind(&conv_op_node), NodeKind::Method);
    }

    #[test]
    fn node_kind_tuple_type() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "class Foo { (int, string) tuple; }");
        let tuple_node = find_node_by_kind(tree.root_node(), "tuple_type").unwrap();
        assert_eq!(support.node_kind(&tuple_node), NodeKind::Type);
    }

    #[test]
    fn node_kind_pointer_type() {
        let support = CSharpSupport;
        let tree = parse_with_support(&support, "unsafe class Foo { int* ptr; }");
        let pointer_node = find_node_by_kind(tree.root_node(), "pointer_type").unwrap();
        assert_eq!(support.node_kind(&pointer_node), NodeKind::Type);
    }

    #[test]
    fn skeleton_query_csharp_indexer_operator_conversion() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_skeleton};

        let source = SourceFile::from_content(
            "test.cs",
            r#"class Foo {
    public int this[int i] { get { return 0; } }
    public static Foo operator +(Foo a, Foo b) { return a; }
    public static implicit operator int(Foo f) { return 0; }
}"#
            .to_string(),
            crate::Grammar::CSharp,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(
                    skel.declarations.iter().any(|d| d.kind == "indexer"),
                    "indexer_declaration should appear in C# skeleton; got: {:?}",
                    skel.declarations
                        .iter()
                        .map(|d| &d.kind)
                        .collect::<Vec<_>>()
                );
                assert!(
                    skel.declarations
                        .iter()
                        .any(|d| d.kind == "method" && d.name == "+"),
                    "operator_declaration should appear as method with name '+'; got: {:?}",
                    skel.declarations
                        .iter()
                        .map(|d| (d.kind.as_str(), d.name.as_str()))
                        .collect::<Vec<_>>()
                );
                assert!(
                    skel.declarations
                        .iter()
                        .any(|d| d.kind == "method" && d.name.contains("int")),
                    "conversion_operator_declaration should appear as method with type name; got: {:?}",
                    skel.declarations
                        .iter()
                        .map(|d| (d.kind.as_str(), d.name.as_str()))
                        .collect::<Vec<_>>()
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn function_parent_kinds_are_csharp_specific() {
        let support = CSharpSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"method_declaration"));
        assert!(kinds.contains(&"constructor_declaration"));
        assert!(kinds.contains(&"local_function_statement"));
        assert!(kinds.contains(&"lambda_expression"));
        // Must NOT contain JS/TS only values
        assert!(!kinds.contains(&"arrow_function"));
        assert!(!kinds.contains(&"function_declaration"));
    }

    #[test]
    fn function_body_kind_is_block() {
        let support = CSharpSupport;
        assert_eq!(support.function_body_kind(), "block");
    }

    #[test]
    fn decision_point_kinds_are_csharp_specific() {
        let support = CSharpSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_statement"));
        assert!(kinds.contains(&"foreach_statement"));
        assert!(kinds.contains(&"switch_expression"));
        assert!(kinds.contains(&"conditional_expression"));
        // Must NOT contain JS/TS only values
        assert!(!kinds.contains(&"for_in_statement"));
        assert!(!kinds.contains(&"switch_case"));
        assert!(!kinds.contains(&"ternary_expression"));
    }

    #[test]
    fn references_query_csharp_includes_declaration_names() {
        // A class declaration like `class Foo {}` yields one reference: the
        // declaring name `Foo`, which `references` reports with the uses.
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_references};

        let source =
            SourceFile::from_content("test.cs", "class Foo { }".to_string(), Grammar::CSharp)
                .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                assert_eq!(
                    references.len(),
                    1,
                    "Declaration name 'Foo' must be captured; got {} references",
                    references.len()
                );
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_csharp_finds_method_invocation() {
        // A method call `Console.WriteLine(x)` should produce references:
        // - `WriteLine` (method name on member access)
        // - `x` (argument)
        // - `Test` and `Method` (the declaring names)
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_references};

        let source = SourceFile::from_content(
            "test.cs",
            "class Test { void Method() { Console.WriteLine(x); } }".to_string(),
            Grammar::CSharp,
        )
        .unwrap();

        let content = source.content();
        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                // Extract identifier text at each captured location using the byte offset.
                let names: Vec<&str> = references
                    .iter()
                    .filter_map(|loc| {
                        let offset = loc.byte_offset()?;
                        let tail = content.get(offset..)?;
                        // Identifiers are sequences of alphanumeric characters or `_`.
                        let end = tail
                            .find(|c: char| !c.is_alphanumeric() && c != '_')
                            .unwrap_or(tail.len());
                        tail.get(..end)
                    })
                    .collect();
                assert_eq!(
                    references.len(),
                    4,
                    "Expected exactly 4 references (Test, Method, WriteLine, x), got {}: {:?}",
                    references.len(),
                    names
                );
                assert!(
                    names.contains(&"WriteLine"),
                    "expected WriteLine in references; got: {:?}",
                    names
                );
                assert!(
                    names.contains(&"x"),
                    "expected x in references; got: {:?}",
                    names
                );
                assert!(
                    names.contains(&"Test"),
                    "declaration name Test must appear; got: {:?}",
                    names
                );
                assert!(
                    names.contains(&"Method"),
                    "declaration name Method must appear; got: {:?}",
                    names
                );
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn print_cs_modifier_tree() {
        let support = CSharpSupport;
        let src = "public class Secret {}";
        let tree = parse_with_support(&support, src);
        println!("{}", tree.root_node().to_sexp());
    }

    #[test]
    fn coverage_hooks_csharp() {
        let s = CSharpSupport;
        let tree = parse_with_support(
            &s,
            "class C { async Task F() { Foo(); throw new Exception(); await Bar(); } }",
        );
        assert!(find_node_by_kind(tree.root_node(), "invocation_expression").is_some());
        assert!(find_node_by_kind(tree.root_node(), "throw_statement").is_some());
        assert!(find_node_by_kind(tree.root_node(), "await_expression").is_some());
        assert!(s.call_node_kinds().contains(&"invocation_expression"));
        assert_ne!(s.call_node_kinds(), ["call_expression", "call"]);
        assert!(s.raise_node_kinds().contains(&"throw_statement"));
        assert!(s.await_node_kinds().contains(&"await_expression"));
        assert!(s.effect_modeled_kinds().contains("await"));
        assert!(s.effect_modeled_kinds().contains("raise"));
        assert!(s.binding_node_kinds().contains(&"variable_declaration"));
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
        assert!(s.unsafe_node_kinds().contains(&"unsafe_statement"));
        assert!(s.raw_sql_sink_names().contains(&"ExecuteReader"));
        assert!(s.unsafe_deserialize_names().contains(&"Deserialize"));
        assert!(s.unsafe_modeled_kinds().contains("unsafe_block"));
        assert!(s.crypto_api_names().contains(&"Aes"));
        assert!(s.env_read_names().contains(&"GetEnvironmentVariable"));
        assert!(s.secret_modeled_kinds().contains("env_secret_read"));
        assert!(s.command_exec_names().contains(&"Start"));
        assert!(s.taint_modeled_kinds().contains("taint"));
    }
}
