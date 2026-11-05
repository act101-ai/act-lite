//! JavaScript language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register JavaScript in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("javascript"),
        display_name: "JavaScript",
        extensions: &["js", "mjs", "cjs"],
        category: LanguageCategory::Dynamic,
    }
}

/// JavaScript language support implementation.
#[derive(Default)]
pub struct JavaScriptSupport;

impl LanguageSupport for JavaScriptSupport {
    fn grammar(&self) -> Grammar {
        Grammar::JavaScript
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_javascript::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Functions
            "function_declaration" | "generator_function_declaration" => NodeKind::Function,
            "arrow_function" | "function_expression" | "generator_function" => NodeKind::Function,

            // Classes
            "class_declaration" | "class" => NodeKind::Class,

            // Methods
            "method_definition" => NodeKind::Method,

            // Properties
            "field_definition" | "pair" => NodeKind::Property,

            // Variables
            "variable_declarator" => NodeKind::Variable,
            "lexical_declaration" | "variable_declaration" => NodeKind::Variable,

            // Parameters
            "formal_parameters" => NodeKind::Parameter,
            "identifier" if self.is_parameter_context(node) => NodeKind::Parameter,

            // Imports/Exports
            "import_statement" | "import_specifier" => NodeKind::Import,
            "export_statement" | "export_specifier" => NodeKind::Export,

            // Blocks
            "statement_block" | "program" => NodeKind::Block,

            // Calls
            "call_expression" | "new_expression" => NodeKind::Call,

            // Identifiers
            "identifier" | "property_identifier" | "shorthand_property_identifier" => {
                NodeKind::Identifier
            }

            // JSX
            "jsx_element" | "jsx_self_closing_element" => NodeKind::Expression,
            "jsx_attribute" => NodeKind::Property,

            // Comments
            "comment" => NodeKind::Comment,

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["js", "mjs", "cjs"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/javascript/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/javascript/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/javascript/references.scm")
    }

    fn locals_query(&self) -> &'static str {
        include_str!("../../queries/javascript/locals.scm")
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

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "statement_block",
            "expression_statement",
            "variable_declaration",
            "lexical_declaration",
            "return_statement",
            "if_statement",
            "for_statement",
            "for_in_statement",
            "for_of_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "switch_case",
            "switch_default",
            "try_statement",
            "catch_clause",
            "finally_clause",
            "throw_statement",
            "with_statement",
            "break_statement",
            "continue_statement",
            "labeled_statement",
            "debugger_statement",
            "function_declaration",
            "class_declaration",
            "export_statement",
            "import_statement",
            "call_expression",
            "binary_expression",
            "logical_expression",
            "ternary_expression",
            "unary_expression",
            "update_expression",
            "assignment_expression",
            "augmented_assignment_expression",
            "member_expression",
            "computed_member_expression",
            "subscript_expression",
            "new_expression",
            "await_expression",
            "yield_expression",
            "arrow_function",
            "function_expression",
            "generator_function",
            "template_string",
            "parenthesized_expression",
            "array",
            "object",
            "spread_element",
            "rest_parameter",
            "sequence_expression",
        ]
    }

    // --- Moat hooks: call detection ------------------------------------------

    fn call_node_kinds(&self) -> &'static [&'static str] {
        // JS calls are `call_expression`; constructor calls (`new Foo()`) are
        // `new_expression` and are real call sites for the call frontier.
        &["call_expression", "new_expression"]
    }

    fn field_access_node_kinds(&self) -> &'static [&'static str] {
        &["member_expression"]
    }

    // --- Moat hooks: effect summary ------------------------------------------

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
        &[] // JS is single-threaded async; no canonical sync-blocking calls
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("raise")
            .with("await")
            .with("allocate")
    }

    // --- Moat hooks: data flow -----------------------------------------------

    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["lexical_declaration", "variable_declaration"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    // --- Moat hooks: unsafe surface ------------------------------------------

    fn eval_call_names(&self) -> &'static [&'static str] {
        // `eval` and the `Function` constructor execute strings as code.
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

    // --- Moat hooks: secret surface ------------------------------------------

    fn crypto_api_names(&self) -> &'static [&'static str] {
        // Node `crypto` signing/HMAC/cipher APIs.
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

    // --- Moat hooks: taint flow ----------------------------------------------

    fn taint_source_names(&self) -> &'static [&'static str] {
        // Express/Node request members and fs reads as untrusted inputs.
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
        // Node `child_process` execution sinks.
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

impl JavaScriptSupport {
    /// Check if an identifier is in a parameter context.
    fn is_parameter_context(&self, node: &tree_sitter::Node) -> bool {
        if let Some(parent) = node.parent() {
            parent.kind() == "formal_parameters"
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};

    crate::language_support_tests!(
        JavaScriptSupport,
        Grammar::JavaScript,
        &["js", "mjs", "cjs"]
    );

    #[test]
    fn node_kind_function_declaration() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "function foo() {}");
        let func_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&func_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_arrow_function() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "const foo = () => {}");
        let arrow = find_node_by_kind(tree.root_node(), "arrow_function").unwrap();
        assert_eq!(support.node_kind(&arrow), NodeKind::Function);
    }

    #[test]
    fn node_kind_class_declaration() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "class Foo {}");
        let class_node = tree.root_node().child(0).unwrap();
        assert_eq!(support.node_kind(&class_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_method_definition() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "class Foo { bar() {} }");
        let method = find_node_by_kind(tree.root_node(), "method_definition").unwrap();
        assert_eq!(support.node_kind(&method), NodeKind::Method);
    }

    #[test]
    fn node_kind_variable_declarator() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "const x = 1;");
        let var_node = find_node_by_kind(tree.root_node(), "variable_declarator").unwrap();
        assert_eq!(support.node_kind(&var_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_field_definition() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "class Foo { x = 1; }");
        let field = find_node_by_kind(tree.root_node(), "field_definition").unwrap();
        assert_eq!(support.node_kind(&field), NodeKind::Property);
    }

    #[test]
    fn node_kind_import_statement() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "import { foo } from 'bar';");
        let import_node = find_node_by_kind(tree.root_node(), "import_statement").unwrap();
        assert_eq!(support.node_kind(&import_node), NodeKind::Import);
    }

    #[test]
    fn node_kind_export_statement() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "export const x = 1;");
        let export_node = find_node_by_kind(tree.root_node(), "export_statement").unwrap();
        assert_eq!(support.node_kind(&export_node), NodeKind::Export);
    }

    #[test]
    fn node_kind_call_expression() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "foo();");
        let call = find_node_by_kind(tree.root_node(), "call_expression").unwrap();
        assert_eq!(support.node_kind(&call), NodeKind::Call);
    }

    #[test]
    fn node_kind_identifier() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "foo;");
        let ident = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        assert_eq!(support.node_kind(&ident), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_property_identifier() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "obj.prop;");
        let prop = find_node_by_kind(tree.root_node(), "property_identifier").unwrap();
        assert_eq!(support.node_kind(&prop), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_comment() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "// This is a comment\nconst x = 1;");
        let comment = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&comment), NodeKind::Comment);
    }

    #[test]
    fn node_kind_unknown() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "1 + 2;");
        let binary_expr = find_node_by_kind(tree.root_node(), "binary_expression").unwrap();
        assert_eq!(support.node_kind(&binary_expr), NodeKind::Unknown);
    }

    // JSX node kind tests

    #[test]
    fn node_kind_jsx_element() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "const x = <div>hello</div>;");
        let jsx = find_node_by_kind(tree.root_node(), "jsx_element").unwrap();
        assert_eq!(support.node_kind(&jsx), NodeKind::Expression);
    }

    #[test]
    fn node_kind_jsx_self_closing_element() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "const x = <br />;");
        let jsx = find_node_by_kind(tree.root_node(), "jsx_self_closing_element").unwrap();
        assert_eq!(support.node_kind(&jsx), NodeKind::Expression);
    }

    #[test]
    fn node_kind_jsx_attribute() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "const x = <div className=\"foo\">hello</div>;");
        let jsx_attr = find_node_by_kind(tree.root_node(), "jsx_attribute").unwrap();
        assert_eq!(support.node_kind(&jsx_attr), NodeKind::Property);
    }

    #[test]
    fn function_parent_kinds_are_javascript_specific() {
        let support = JavaScriptSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"function_declaration"));
        assert!(kinds.contains(&"method_definition"));
        assert!(kinds.contains(&"arrow_function"));
    }

    #[test]
    fn function_body_kind_is_statement_block() {
        let support = JavaScriptSupport;
        assert_eq!(support.function_body_kind(), "statement_block");
    }

    #[test]
    fn decision_point_kinds_are_javascript_specific() {
        let support = JavaScriptSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_statement"));
        assert!(kinds.contains(&"for_in_statement"));
        assert!(kinds.contains(&"ternary_expression"));
        assert!(!kinds.is_empty());
    }

    // --- Moat coverage hook tests --------------------------------------------

    #[test]
    fn call_node_kinds_overridden_for_coverage() {
        let s = JavaScriptSupport;
        assert_ne!(s.call_node_kinds(), &["call_expression", "call"]);
        assert!(s.call_node_kinds().contains(&"call_expression"));
        assert!(s.call_node_kinds().contains(&"new_expression"));
    }

    #[test]
    fn new_expression_is_a_call_node() {
        let support = JavaScriptSupport;
        let tree = parse_with_support(&support, "const d = new Date();");
        let node = find_node_by_kind(tree.root_node(), "new_expression").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Call);
        assert!(support.call_node_kinds().contains(&node.kind()));
    }

    #[test]
    fn effect_summary_detects_throw_await_alloc() {
        let s = JavaScriptSupport;
        // throw_statement, await_expression, new_expression all parse in JS.
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
        let s = JavaScriptSupport;
        let tree = parse_with_support(&s, "const x = 1; var y = 2;");
        assert!(find_node_by_kind(tree.root_node(), "lexical_declaration").is_some());
        assert!(find_node_by_kind(tree.root_node(), "variable_declaration").is_some());
        assert!(s.binding_node_kinds().contains(&"lexical_declaration"));
        assert!(s.binding_node_kinds().contains(&"variable_declaration"));
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
    }

    #[test]
    fn unsafe_surface_models_eval_and_sql() {
        let s = JavaScriptSupport;
        assert!(s.eval_call_names().contains(&"eval"));
        assert!(s.eval_call_names().contains(&"Function"));
        assert!(s.raw_sql_sink_names().contains(&"query"));
        let mask = s.unsafe_modeled_kinds();
        assert!(mask.contains("eval"));
        assert!(mask.contains("raw_sql"));
        assert!(mask.contains("unsafe_deserialize"));
    }

    #[test]
    fn secret_surface_models_crypto_and_env() {
        let s = JavaScriptSupport;
        assert!(s.crypto_api_names().contains(&"createHmac"));
        assert!(s.env_read_names().contains(&"process.env"));
        let mask = s.secret_modeled_kinds();
        assert!(mask.contains("signing_key"));
        assert!(mask.contains("env_secret_read"));
    }

    #[test]
    fn taint_flow_models_sources_and_sinks() {
        let s = JavaScriptSupport;
        assert!(s.taint_source_names().contains(&"body"));
        assert!(s.command_exec_names().contains(&"exec"));
        assert!(s.fs_sink_names().contains(&"writeFile"));
        assert!(s.sanitizer_names().contains(&"encodeURIComponent"));
        assert!(s.taint_modeled_kinds().contains("taint"));
    }

    #[test]
    fn field_access_node_kinds_is_member_expression() {
        assert_eq!(
            JavaScriptSupport.field_access_node_kinds(),
            &["member_expression"]
        );
    }
}
