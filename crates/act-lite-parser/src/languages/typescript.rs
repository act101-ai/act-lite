//! TypeScript language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register TypeScript in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("typescript"),
        display_name: "TypeScript",
        extensions: &["ts", "mts", "cts"],
        category: LanguageCategory::Dynamic,
    }
}

/// TypeScript language support implementation.
#[derive(Default)]
pub struct TypeScriptSupport;

impl LanguageSupport for TypeScriptSupport {
    fn grammar(&self) -> Grammar {
        Grammar::TypeScript
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Declarations
            "function_declaration"
            | "function_signature"
            | "generator_function_declaration"
            | "arrow_function" => NodeKind::Function,
            "class_declaration" | "abstract_class_declaration" => NodeKind::Class,
            "interface_declaration" => NodeKind::Interface,
            "type_alias_declaration" => NodeKind::TypeAlias,
            "enum_declaration" => NodeKind::Enum,
            "variable_declarator" => NodeKind::Variable,
            "method_definition" | "abstract_method_signature" | "method_signature" => {
                NodeKind::Method
            }
            "property_signature"
            | "public_field_definition"
            | "index_signature"
            | "enum_assignment"
            | "accessibility_modifier" => NodeKind::Property,
            "formal_parameters" | "required_parameter" | "optional_parameter" => {
                NodeKind::Parameter
            }
            "construct_signature" => NodeKind::Constructor,
            "decorator" => NodeKind::Decorator,
            "internal_module" => NodeKind::Module,
            "type_parameter" | "type_parameters" => NodeKind::TypeParameter,

            // Statements
            "import_statement" => NodeKind::Import,
            "export_statement" => NodeKind::Export,
            "statement_block" => NodeKind::Block,

            // Expressions
            "call_expression" => NodeKind::Call,
            "identifier" | "type_identifier" | "property_identifier" => NodeKind::Identifier,

            // Other
            "comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "mts", "cts"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/typescript/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/typescript/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/typescript/references.scm")
    }

    fn locals_query(&self) -> &'static str {
        include_str!("../../queries/typescript/locals.scm")
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
            "method_definition",
            "arrow_function",
        ]
    }

    fn function_body_kind(&self) -> &'static str {
        "statement_block"
    }

    fn function_keyword(&self) -> &'static str {
        "function"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "switch_case",
            "catch_clause",
            "ternary_expression",
        ]
    }

    fn cfg_branch_kinds(&self) -> &'static [&'static str] {
        &["if_statement", "ternary_expression", "switch_statement"]
    }
    fn cfg_loop_kinds(&self) -> &'static [&'static str] {
        &[
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
        ]
    }
    fn cfg_arm_container_kinds(&self) -> &'static [&'static str] {
        &["switch_body"]
    }
    fn cfg_arm_kinds(&self) -> &'static [&'static str] {
        &["switch_case", "switch_default"]
    }
    fn cfg_exception_handler_kinds(&self) -> &'static [&'static str] {
        &["try_statement", "catch_clause", "finally_clause"]
    }
    fn cfg_throw_kinds(&self) -> &'static [&'static str] {
        &["throw_statement"]
    }
    fn cfg_return_kinds(&self) -> &'static [&'static str] {
        &["return_statement"]
    }
    fn cfg_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("seq")
            .with("branch")
            .with("loop")
            .with("exception")
            .with("return")
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "statement_block",
            "function_body",
            "block",
            "expression_statement",
            "variable_declaration",
            "lexical_declaration",
            "if_statement",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "return_statement",
            "break_statement",
            "continue_statement",
            "throw_statement",
            "try_statement",
            "switch_statement",
            "call_expression",
            "binary_expression",
            "member_expression",
            "arrow_function",
            "function_expression",
            "assignment_expression",
            "await_expression",
            "yield_expression",
        ]
    }

    fn stub_placeholders(&self) -> &'static [&'static str] {
        &[
            "throw new Error(\"not implemented\")",
            "throw new Error(\"Not implemented\")",
            "TODO",
        ]
    }

    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["lexical_declaration", "variable_declaration"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    fn raise_node_kinds(&self) -> &'static [&'static str] {
        &["throw_statement"]
    }

    fn await_node_kinds(&self) -> &'static [&'static str] {
        &["await_expression"]
    }

    fn allocation_node_kinds(&self) -> &'static [&'static str] {
        &["new_expression"]
    }

    fn blocking_call_names(&self) -> &'static [&'static str] {
        &[] // TS is single-threaded async; no canonical sync-blocking calls
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("raise")
            .with("await")
            .with("allocate")
    }

    fn eval_call_names(&self) -> &'static [&'static str] {
        &["eval", "Function"]
    }

    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &["query", "execute", "executeQuery", "raw", "unsafe"]
    }

    fn unsafe_deserialize_names(&self) -> &'static [&'static str] {
        &["deserialize"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("eval")
            .with("raw_sql")
            .with("unsafe_deserialize")
    }

    fn crypto_api_names(&self) -> &'static [&'static str] {
        &[
            "createHmac",
            "createSign",
            "sign",
            "createCipheriv",
            "privateDecrypt",
        ]
    }

    fn env_read_names(&self) -> &'static [&'static str] {
        &["process.env", "env"]
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
        &[
            "query",
            "body",
            "param",
            "params",
            "headers",
            "readFile",
            "readFileSync",
            "getenv",
        ]
    }

    fn llm_response_names(&self) -> &'static [&'static str] {
        &[
            "messages.create",
            "messages.stream",
            "beta.messages.create",
            "chat.completions.create",
            "chat.completions.parse",
            "completions.create",
            "responses.create",
            "generateContent",
            "models.generate_content",
        ]
    }

    fn command_exec_names(&self) -> &'static [&'static str] {
        &["exec", "execSync", "spawn", "spawnSync"]
    }

    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["readFile", "writeFile", "open", "createReadStream"]
    }

    fn html_sink_names(&self) -> &'static [&'static str] {
        &["insertAdjacentHTML", "document.write"]
    }

    fn sanitizer_names(&self) -> &'static [&'static str] {
        &["escape", "sanitize", "encodeURIComponent", "parameterize"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        // TS calls are `call_expression`; constructor calls (`new Foo()`) are
        // `new_expression` and are real call sites for the call frontier.
        &["call_expression", "new_expression"]
    }

    fn field_access_node_kinds(&self) -> &'static [&'static str] {
        &["member_expression"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};
    use act_lite_core::registry::LANGUAGE_REGISTRY;

    crate::language_support_tests!(
        TypeScriptSupport,
        Grammar::TypeScript,
        &["ts", "mts", "cts"]
    );

    #[test]
    fn registry_has_typescript() {
        let ts_meta = LANGUAGE_REGISTRY.get(LanguageId("typescript"));
        assert!(ts_meta.is_some(), "TypeScript should be registered");
        let meta = ts_meta.unwrap();
        assert_eq!(meta.display_name, "TypeScript");
        assert!(meta.extensions.contains(&"ts"));
        assert!(meta.extensions.contains(&"mts"));
        assert!(meta.extensions.contains(&"cts"));
    }

    #[test]
    fn registry_extension_lookup() {
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("ts"),
            Some(LanguageId("typescript"))
        );
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("mts"),
            Some(LanguageId("typescript"))
        );
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("cts"),
            Some(LanguageId("typescript"))
        );
    }

    #[test]
    fn node_kind_function() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "function foo() {}");
        let func_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&func_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_class() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "class Foo {}");
        let class_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&class_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_interface() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "interface IFoo { name: string; }");
        let interface_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&interface_node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_type_alias() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "type MyType = string | number;");
        let type_alias_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&type_alias_node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_enum() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "enum Color { Red, Green, Blue }");
        let enum_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&enum_node), NodeKind::Enum);
    }

    #[test]
    fn node_kind_variable_declarator() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "const x = 1;");
        let var_node = find_node_by_kind(tree.root_node(), "variable_declarator").unwrap();
        assert_eq!(support.node_kind(&var_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_method_definition() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "class Foo { bar() {} }");
        let method = find_node_by_kind(tree.root_node(), "method_definition").unwrap();
        assert_eq!(support.node_kind(&method), NodeKind::Method);
    }

    #[test]
    fn node_kind_property_signature() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "interface Foo { name: string; }");
        let prop = find_node_by_kind(tree.root_node(), "property_signature").unwrap();
        assert_eq!(support.node_kind(&prop), NodeKind::Property);
    }

    #[test]
    fn node_kind_public_field_definition() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "class Foo { name = 'test'; }");
        let field = find_node_by_kind(tree.root_node(), "public_field_definition").unwrap();
        assert_eq!(support.node_kind(&field), NodeKind::Property);
    }

    #[test]
    fn node_kind_required_parameter() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "function foo(x: number) {}");
        let param = find_node_by_kind(tree.root_node(), "required_parameter").unwrap();
        assert_eq!(support.node_kind(&param), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_optional_parameter() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "function foo(x?: number) {}");
        let param = find_node_by_kind(tree.root_node(), "optional_parameter").unwrap();
        assert_eq!(support.node_kind(&param), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_formal_parameters() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "function foo(x: number) {}");
        let params = find_node_by_kind(tree.root_node(), "formal_parameters").unwrap();
        assert_eq!(support.node_kind(&params), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_import_statement() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "import { foo } from 'bar';");
        let import_node = find_node_by_kind(tree.root_node(), "import_statement").unwrap();
        assert_eq!(support.node_kind(&import_node), NodeKind::Import);
    }

    #[test]
    fn node_kind_export_statement() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "export const x = 1;");
        let export_node = find_node_by_kind(tree.root_node(), "export_statement").unwrap();
        assert_eq!(support.node_kind(&export_node), NodeKind::Export);
    }

    #[test]
    fn node_kind_statement_block() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "function foo() { const x = 1; }");
        let block = find_node_by_kind(tree.root_node(), "statement_block").unwrap();
        assert_eq!(support.node_kind(&block), NodeKind::Block);
    }

    #[test]
    fn node_kind_call_expression() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "foo();");
        let call = find_node_by_kind(tree.root_node(), "call_expression").unwrap();
        assert_eq!(support.node_kind(&call), NodeKind::Call);
    }

    #[test]
    fn node_kind_identifier() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "foo;");
        let ident = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        assert_eq!(support.node_kind(&ident), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_type_identifier() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "type X = Foo;");
        let type_ident = find_node_by_kind(tree.root_node(), "type_identifier").unwrap();
        assert_eq!(support.node_kind(&type_ident), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_property_identifier() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "obj.prop;");
        let prop = find_node_by_kind(tree.root_node(), "property_identifier").unwrap();
        assert_eq!(support.node_kind(&prop), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_comment() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "// This is a comment\nconst x = 1;");
        let comment = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&comment), NodeKind::Comment);
    }

    #[test]
    fn node_kind_unknown() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "1 + 2;");
        let binary_expr = find_node_by_kind(tree.root_node(), "binary_expression").unwrap();
        assert_eq!(support.node_kind(&binary_expr), NodeKind::Unknown);
    }

    #[test]
    fn node_kind_abstract_class_declaration() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "abstract class Foo {}");
        let node = find_node_by_kind(tree.root_node(), "abstract_class_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Class);
    }

    #[test]
    fn node_kind_arrow_function() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "const fn = () => {};");
        let node = find_node_by_kind(tree.root_node(), "arrow_function").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_generator_function_declaration() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "function* gen() { yield 1; }");
        let node = find_node_by_kind(tree.root_node(), "generator_function_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_decorator() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "@Injectable()\nclass Foo {}");
        let node = find_node_by_kind(tree.root_node(), "decorator").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Decorator);
    }

    #[test]
    fn node_kind_internal_module() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "namespace Foo { export const x = 1; }");
        let node = find_node_by_kind(tree.root_node(), "internal_module").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Module);
    }

    #[test]
    fn node_kind_abstract_method_signature() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "abstract class Foo { abstract bar(): void; }");
        let node = find_node_by_kind(tree.root_node(), "abstract_method_signature").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Method);
    }

    #[test]
    fn node_kind_function_signature() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "declare function foo(x: number): void;");
        let node = find_node_by_kind(tree.root_node(), "function_signature").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_method_signature() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "interface Foo { bar(x: number): void; }");
        let node = find_node_by_kind(tree.root_node(), "method_signature").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Method);
    }

    #[test]
    fn node_kind_index_signature() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "interface Foo { [key: string]: number; }");
        let node = find_node_by_kind(tree.root_node(), "index_signature").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Property);
    }

    #[test]
    fn node_kind_construct_signature() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "interface Foo { new(x: number): Foo; }");
        let node = find_node_by_kind(tree.root_node(), "construct_signature").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Constructor);
    }

    #[test]
    fn node_kind_enum_assignment() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "enum Color { Red = 0 }");
        let node = find_node_by_kind(tree.root_node(), "enum_assignment").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Property);
    }

    #[test]
    fn node_kind_type_parameters() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "function id<T>(x: T): T { return x; }");
        let node = find_node_by_kind(tree.root_node(), "type_parameters").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::TypeParameter);
    }

    #[test]
    fn node_kind_type_parameter() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "function id<T>(x: T): T { return x; }");
        let node = find_node_by_kind(tree.root_node(), "type_parameter").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::TypeParameter);
    }

    #[test]
    fn node_kind_accessibility_modifier() {
        let support = TypeScriptSupport;
        let tree = parse_with_support(&support, "class Foo { private x: number = 0; }");
        let node = find_node_by_kind(tree.root_node(), "accessibility_modifier").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Property);
    }

    #[test]
    fn function_keyword_is_function() {
        let support = TypeScriptSupport;
        assert_eq!(support.function_keyword(), "function");
    }

    #[test]
    fn function_parent_kinds_are_typescript_specific() {
        let support = TypeScriptSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"function_declaration"));
        assert!(kinds.contains(&"method_definition"));
        assert!(kinds.contains(&"arrow_function"));
    }

    #[test]
    fn function_body_kind_is_statement_block() {
        let support = TypeScriptSupport;
        assert_eq!(support.function_body_kind(), "statement_block");
    }

    #[test]
    fn decision_point_kinds_are_typescript_specific() {
        let support = TypeScriptSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_statement"));
        assert!(kinds.contains(&"for_statement"));
        assert!(kinds.contains(&"ternary_expression"));
        // Must NOT contain empty (confirms the JS default is explicit here)
        assert!(!kinds.is_empty());
    }

    #[test]
    fn stub_placeholders_returns_language_patterns() {
        let support = TypeScriptSupport;
        let placeholders = support.stub_placeholders();
        assert!(!placeholders.is_empty());
        assert!(placeholders.contains(&"throw new Error(\"not implemented\")"));
        assert!(placeholders.contains(&"TODO"));
    }

    #[test]
    fn typescript_effect_hooks_populated() {
        let s = TypeScriptSupport;
        assert!(s.raise_node_kinds().contains(&"throw_statement"));
        assert!(s.await_node_kinds().contains(&"await_expression"));
        assert!(s.allocation_node_kinds().contains(&"new_expression"));
        let mask = s.effect_modeled_kinds();
        assert!(mask.contains("raise"));
        assert!(mask.contains("await"));
        assert!(mask.contains("allocate"));
    }

    #[test]
    fn call_node_kinds_overridden_for_coverage() {
        let s = TypeScriptSupport;
        // Must override the trait default so the call-detection invariant
        // counts TS as covered (real call node declared).
        assert_ne!(s.call_node_kinds(), &["call_expression", "call"]);
        assert!(s.call_node_kinds().contains(&"call_expression"));
        assert!(s.call_node_kinds().contains(&"new_expression"));
    }

    #[test]
    fn cfg_hooks_typescript() {
        let ts = TypeScriptSupport;
        assert!(ts.cfg_branch_kinds().contains(&"if_statement"));
        assert!(ts.cfg_loop_kinds().contains(&"while_statement"));
        assert_eq!(ts.cfg_arm_container_kinds(), &["switch_body"]);
        assert!(ts.cfg_arm_kinds().contains(&"switch_case"));
        assert!(ts.cfg_exception_handler_kinds().contains(&"catch_clause"));
        assert!(ts.cfg_throw_kinds().contains(&"throw_statement"));
        assert!(ts.cfg_return_kinds().contains(&"return_statement"));
        assert!(ts.cfg_modeled_kinds().contains("exception"));
    }

    #[test]
    fn field_access_node_kinds_is_member_expression() {
        assert_eq!(
            TypeScriptSupport.field_access_node_kinds(),
            &["member_expression"]
        );
    }
}
