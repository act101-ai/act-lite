//! TSX language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register TSX in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("tsx"),
        display_name: "TSX",
        extensions: &["tsx"],
        category: LanguageCategory::Dynamic,
    }
}

/// TSX language support implementation.
///
/// TSX uses the `LANGUAGE_TSX` grammar from `tree-sitter-typescript`,
/// which handles JSX syntax that the regular TypeScript grammar cannot parse.
#[derive(Default)]
pub struct TsxSupport;

impl LanguageSupport for TsxSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Tsx
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_typescript::LANGUAGE_TSX.into()
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

            // JSX-specific nodes
            "jsx_element" | "jsx_self_closing_element" => NodeKind::Expression,
            "jsx_attribute" => NodeKind::Property,

            // Other
            "comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tsx"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/tsx/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/tsx/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/tsx/references.scm")
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

    // --- Moat hooks: TSX shares TypeScript runtime semantics -----------------

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "new_expression"]
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
        &[] // single-threaded async; no canonical sync-blocking calls
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("raise")
            .with("await")
            .with("allocate")
    }

    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["lexical_declaration", "variable_declaration"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};

    crate::language_support_tests!(TsxSupport, Grammar::Tsx, &["tsx"]);

    #[test]
    fn node_kind_function() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "function foo() {}");
        let func_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&func_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_class() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "class Foo {}");
        let class_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&class_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_interface() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "interface IFoo { name: string; }");
        let interface_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&interface_node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_type_alias() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "type MyType = string | number;");
        let type_alias_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&type_alias_node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_enum() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "enum Color { Red, Green, Blue }");
        let enum_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&enum_node), NodeKind::Enum);
    }

    #[test]
    fn node_kind_variable_declarator() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "const x = 1;");
        let var_node = find_node_by_kind(tree.root_node(), "variable_declarator").unwrap();
        assert_eq!(support.node_kind(&var_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_method_definition() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "class Foo { bar() {} }");
        let method = find_node_by_kind(tree.root_node(), "method_definition").unwrap();
        assert_eq!(support.node_kind(&method), NodeKind::Method);
    }

    #[test]
    fn node_kind_property_signature() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "interface Foo { name: string; }");
        let prop = find_node_by_kind(tree.root_node(), "property_signature").unwrap();
        assert_eq!(support.node_kind(&prop), NodeKind::Property);
    }

    #[test]
    fn node_kind_identifier() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "foo;");
        let ident = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        assert_eq!(support.node_kind(&ident), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_comment() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "// This is a comment\nconst x = 1;");
        let comment = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&comment), NodeKind::Comment);
    }

    #[test]
    fn node_kind_unknown() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "1 + 2;");
        let binary_expr = find_node_by_kind(tree.root_node(), "binary_expression").unwrap();
        assert_eq!(support.node_kind(&binary_expr), NodeKind::Unknown);
    }

    // JSX-specific node kind tests

    #[test]
    fn node_kind_jsx_element() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "const x = <div>hello</div>;");
        let jsx = find_node_by_kind(tree.root_node(), "jsx_element").unwrap();
        assert_eq!(support.node_kind(&jsx), NodeKind::Expression);
    }

    #[test]
    fn node_kind_jsx_self_closing_element() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "const x = <br />;");
        let jsx = find_node_by_kind(tree.root_node(), "jsx_self_closing_element").unwrap();
        assert_eq!(support.node_kind(&jsx), NodeKind::Expression);
    }

    #[test]
    fn node_kind_jsx_fragment_parsed_as_element() {
        // JSX fragments (<>...</>) are parsed as jsx_element by tree-sitter-typescript
        let support = TsxSupport;
        let tree = parse_with_support(&support, "const x = <>hello</>;");
        let jsx = find_node_by_kind(tree.root_node(), "jsx_element").unwrap();
        assert_eq!(support.node_kind(&jsx), NodeKind::Expression);
    }

    #[test]
    fn node_kind_jsx_attribute() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "const x = <div className=\"foo\">hello</div>;");
        let jsx_attr = find_node_by_kind(tree.root_node(), "jsx_attribute").unwrap();
        assert_eq!(support.node_kind(&jsx_attr), NodeKind::Property);
    }

    // Node kinds ported from TypeScriptSupport

    #[test]
    fn node_kind_abstract_class_declaration() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "abstract class Foo {}");
        let node = find_node_by_kind(tree.root_node(), "abstract_class_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Class);
    }

    #[test]
    fn node_kind_arrow_function() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "const fn = () => {};");
        let node = find_node_by_kind(tree.root_node(), "arrow_function").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_generator_function_declaration() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "function* gen() { yield 1; }");
        let node = find_node_by_kind(tree.root_node(), "generator_function_declaration").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_function_signature() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "declare function foo(x: number): void;");
        let node = find_node_by_kind(tree.root_node(), "function_signature").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_abstract_method_signature() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "abstract class Foo { abstract bar(): void; }");
        let node = find_node_by_kind(tree.root_node(), "abstract_method_signature").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Method);
    }

    #[test]
    fn node_kind_method_signature() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "interface Foo { bar(x: number): void; }");
        let node = find_node_by_kind(tree.root_node(), "method_signature").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Method);
    }

    #[test]
    fn node_kind_index_signature() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "interface Foo { [key: string]: number; }");
        let node = find_node_by_kind(tree.root_node(), "index_signature").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Property);
    }

    #[test]
    fn node_kind_enum_assignment() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "enum Color { Red = 0 }");
        let node = find_node_by_kind(tree.root_node(), "enum_assignment").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Property);
    }

    #[test]
    fn node_kind_accessibility_modifier() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "class Foo { private x: number = 0; }");
        let node = find_node_by_kind(tree.root_node(), "accessibility_modifier").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Property);
    }

    #[test]
    fn node_kind_construct_signature() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "interface Foo { new(x: number): Foo; }");
        let node = find_node_by_kind(tree.root_node(), "construct_signature").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Constructor);
    }

    #[test]
    fn node_kind_decorator() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "@Injectable()\nclass Foo {}");
        let node = find_node_by_kind(tree.root_node(), "decorator").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Decorator);
    }

    #[test]
    fn node_kind_internal_module() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "namespace Foo { export const x = 1; }");
        let node = find_node_by_kind(tree.root_node(), "internal_module").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Module);
    }

    #[test]
    fn node_kind_type_parameters() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "function id<T>(x: T): T { return x; }");
        let node = find_node_by_kind(tree.root_node(), "type_parameters").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::TypeParameter);
    }

    #[test]
    fn node_kind_type_parameter() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "function id<T>(x: T): T { return x; }");
        let node = find_node_by_kind(tree.root_node(), "type_parameter").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::TypeParameter);
    }

    #[test]
    fn function_parent_kinds_are_tsx_specific() {
        let support = TsxSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"function_declaration"));
        assert!(kinds.contains(&"method_definition"));
        assert!(kinds.contains(&"arrow_function"));
    }

    #[test]
    fn function_body_kind_is_statement_block() {
        let support = TsxSupport;
        assert_eq!(support.function_body_kind(), "statement_block");
    }

    #[test]
    fn decision_point_kinds_are_tsx_specific() {
        let support = TsxSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_statement"));
        assert!(kinds.contains(&"ternary_expression"));
        assert!(!kinds.is_empty());
    }

    // --- Moat coverage hook tests --------------------------------------------

    #[test]
    fn call_node_kinds_overridden_for_coverage() {
        let s = TsxSupport;
        assert_ne!(s.call_node_kinds(), &["call_expression", "call"]);
        assert!(s.call_node_kinds().contains(&"call_expression"));
        assert!(s.call_node_kinds().contains(&"new_expression"));
    }

    #[test]
    fn new_expression_parses_in_tsx() {
        let support = TsxSupport;
        let tree = parse_with_support(&support, "const d = new Date();");
        assert!(find_node_by_kind(tree.root_node(), "new_expression").is_some());
    }

    #[test]
    fn effect_summary_detects_throw_await_alloc() {
        let s = TsxSupport;
        let throw_tree = parse_with_support(&s, "function f() { throw new Error('x'); }");
        assert!(find_node_by_kind(throw_tree.root_node(), "throw_statement").is_some());
        let await_tree = parse_with_support(&s, "async function f() { await g(); }");
        assert!(find_node_by_kind(await_tree.root_node(), "await_expression").is_some());
        assert!(s.raise_node_kinds().contains(&"throw_statement"));
        assert!(s.await_node_kinds().contains(&"await_expression"));
        assert!(s.allocation_node_kinds().contains(&"new_expression"));
        let mask = s.effect_modeled_kinds();
        assert!(mask.contains("raise"));
        assert!(mask.contains("await"));
        assert!(mask.contains("allocate"));
    }

    #[test]
    fn data_flow_detects_bindings() {
        let s = TsxSupport;
        let tree = parse_with_support(&s, "const x = 1; var y = 2;");
        assert!(find_node_by_kind(tree.root_node(), "lexical_declaration").is_some());
        assert!(s.binding_node_kinds().contains(&"lexical_declaration"));
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
    }

    #[test]
    fn unsafe_surface_models_eval_and_sql() {
        let s = TsxSupport;
        assert!(s.eval_call_names().contains(&"eval"));
        assert!(s.raw_sql_sink_names().contains(&"query"));
        let mask = s.unsafe_modeled_kinds();
        assert!(mask.contains("eval"));
        assert!(mask.contains("raw_sql"));
    }

    #[test]
    fn secret_surface_models_crypto_and_env() {
        let s = TsxSupport;
        assert!(s.crypto_api_names().contains(&"createHmac"));
        assert!(s.env_read_names().contains(&"process.env"));
        let mask = s.secret_modeled_kinds();
        assert!(mask.contains("signing_key"));
        assert!(mask.contains("env_secret_read"));
    }

    #[test]
    fn taint_flow_models_sources_and_sinks() {
        let s = TsxSupport;
        assert!(s.taint_source_names().contains(&"body"));
        assert!(s.command_exec_names().contains(&"exec"));
        assert!(s.fs_sink_names().contains(&"writeFile"));
        assert!(s.sanitizer_names().contains(&"encodeURIComponent"));
        assert!(s.taint_modeled_kinds().contains("taint"));
    }
}
