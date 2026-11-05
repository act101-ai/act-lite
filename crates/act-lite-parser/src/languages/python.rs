//! Python language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Python in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("python"),
        display_name: "Python",
        extensions: &["py", "pyi", "pyw"],
        category: LanguageCategory::Dynamic,
    }
}

/// Python language support implementation.
#[derive(Default)]
pub struct PythonSupport;

impl LanguageSupport for PythonSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Python
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        // Check for methods first (functions inside class bodies)
        if node.kind() == "function_definition" && self.is_method_context(node) {
            return NodeKind::Method;
        }

        match node.kind() {
            // Functions (async is a modifier on function_definition, not a separate type)
            "function_definition" | "lambda" => NodeKind::Function,

            // Classes
            "class_definition" => NodeKind::Class,

            // Decorators
            "decorator" | "decorated_definition" => NodeKind::Decorator,

            // Properties/Fields (class attributes checked first)
            "assignment" if self.is_class_attribute(node) => NodeKind::Property,

            // Variables
            "assignment" | "augmented_assignment" => NodeKind::Variable,

            // Parameters
            "parameters" => NodeKind::Parameter,
            "identifier" if self.is_parameter_context(node) => NodeKind::Parameter,
            "typed_parameter" | "default_parameter" | "typed_default_parameter" => {
                NodeKind::Parameter
            }
            "list_splat_pattern" | "dictionary_splat_pattern" => NodeKind::Parameter,

            // Imports
            "import_statement" | "import_from_statement" => NodeKind::Import,
            "aliased_import" | "dotted_name" => NodeKind::Import,

            // Blocks and control flow
            "block" | "module" => NodeKind::Block,
            "if_statement" | "elif_clause" | "else_clause" => NodeKind::Block,
            "for_statement" | "while_statement" => NodeKind::Block,
            "try_statement" | "except_clause" | "finally_clause" => NodeKind::Block,
            "with_statement" => NodeKind::Block,

            // Calls
            "call" => NodeKind::Call,

            // Identifiers
            "identifier" => NodeKind::Identifier,
            "attribute" => NodeKind::Identifier,

            // Comments
            "comment" => NodeKind::Comment,

            // Type annotations
            "type" | "generic_type" | "subscript" => NodeKind::Type,

            // Comprehensions
            "list_comprehension"
            | "dictionary_comprehension"
            | "set_comprehension"
            | "generator_expression" => NodeKind::Expression,

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn node_kind_with_source(&self, node: &tree_sitter::Node, source: &[u8]) -> NodeKind {
        // Check constant assignments (needs source text for identifier name check)
        if node.kind() == "assignment"
            && !self.is_class_attribute(node)
            && self.is_constant_assignment(node, source)
        {
            return NodeKind::Constant;
        }
        self.node_kind(node)
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py", "pyi", "pyw"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/python/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/python/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/python/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
            block_start: None, // Python uses triple-quoted strings, not true block comments
            block_end: None,
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }

    fn function_body_kind(&self) -> &'static str {
        "block"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "elif_clause",
            "for_statement",
            "while_statement",
            "except_clause",
            "conditional_expression",
            "with_statement",
        ]
    }

    fn cfg_branch_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "elif_clause",
            "match_statement",
            "conditional_expression",
        ]
    }
    fn cfg_loop_kinds(&self) -> &'static [&'static str] {
        &["for_statement", "while_statement"]
    }
    fn cfg_arm_container_kinds(&self) -> &'static [&'static str] {
        &["block"]
    }
    fn cfg_arm_kinds(&self) -> &'static [&'static str] {
        &["case_clause"]
    }
    fn cfg_exception_handler_kinds(&self) -> &'static [&'static str] {
        &["try_statement", "except_clause", "finally_clause"]
    }
    fn cfg_throw_kinds(&self) -> &'static [&'static str] {
        &["raise_statement"]
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

    fn logical_operator_info(&self) -> Option<(&'static str, &'static [&'static str])> {
        Some(("boolean_operator", &["and", "or"]))
    }

    fn function_keyword(&self) -> &'static str {
        "def"
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "block",
            "expression_statement",
            "assignment",
            "augmented_assignment",
            "return_statement",
            "if_statement",
            "for_statement",
            "while_statement",
            "try_statement",
            "with_statement",
            "raise_statement",
            "assert_statement",
            "pass_statement",
            "break_statement",
            "continue_statement",
            "delete_statement",
            "global_statement",
            "nonlocal_statement",
            "import_statement",
            "import_from_statement",
            "call",
            "binary_operator",
            "comparison_operator",
            "boolean_operator",
            "not_operator",
            "concatenated_string",
            "lambda",
            "list_comprehension",
            "set_comprehension",
            "dictionary_comprehension",
            "generator_expression",
            "ternary_expression",
            "await",
        ]
    }

    fn stub_placeholders(&self) -> &'static [&'static str] {
        &["pass", "raise NotImplementedError", "..."]
    }

    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["assignment", "augmented_assignment"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    /// Python raises via the `raise` statement. Mirrors TypeScript's
    /// `throw_statement` so `execute_effect_summary` emits a `Raise` effect and
    /// the contract's `raises` dimension is populated.
    fn raise_node_kinds(&self) -> &'static [&'static str] {
        &["raise_statement"]
    }

    /// Python models reads/writes (shared mutations engine) plus `raise`. It does
    /// not yet model `await`/`allocate` as distinct effect kinds, so those stay
    /// out of the mask (honest degradation — see `effect_modeled_kinds` default).
    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("raise")
    }

    fn eval_call_names(&self) -> &'static [&'static str] {
        &["eval", "exec", "compile"]
    }

    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &["execute", "executemany", "executescript", "raw"]
    }

    fn ffi_markers(&self) -> &'static [&'static str] {
        &["CDLL", "cdll", "ctypes", "cffi"]
    }

    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        &["getattr", "__import__"]
    }

    fn unsafe_deserialize_names(&self) -> &'static [&'static str] {
        &["loads", "load", "pickle.loads", "yaml.load"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("eval")
            .with("raw_sql")
            .with("ffi")
            .with("reflection_invoke")
            .with("unsafe_deserialize")
    }

    fn crypto_api_names(&self) -> &'static [&'static str] {
        &["new", "sign", "HMAC", "PKCS1_OAEP"]
    }

    fn env_read_names(&self) -> &'static [&'static str] {
        &["getenv", "environ", "os.getenv", "os.environ"]
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
        &["get", "getenv", "environ", "input", "read", "recv", "args"]
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
            "generate_content",
            "models.generate_content",
            "invoke_model",
            "converse",
        ]
    }

    fn command_exec_names(&self) -> &'static [&'static str] {
        &["system", "popen", "run", "call", "check_output", "Popen"]
    }

    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["open"]
    }

    fn sanitizer_names(&self) -> &'static [&'static str] {
        &["escape", "quote", "sanitize"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }

    /// Python's only call node is `call` (there is no `call_expression`).
    /// Overrides the `call_expression`/`call` default so the call frontier is
    /// declared precisely for the language.
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call"]
    }

    fn field_access_node_kinds(&self) -> &'static [&'static str] {
        &["attribute"]
    }
}

impl PythonSupport {
    /// Check if a function definition is inside a class body (making it a method).
    ///
    /// Traverses up the tree, stopping at function_definition/module boundaries.
    /// Note: decorated_definition nodes are traversed through (handled by _ case),
    /// since decorators wrap the actual function_definition node.
    fn is_method_context(&self, node: &tree_sitter::Node) -> bool {
        let mut current = node.parent();
        while let Some(parent) = current {
            match parent.kind() {
                "class_definition" => return true,
                "function_definition" | "module" => return false,
                _ => current = parent.parent(),
            }
        }
        false
    }

    /// Check if an assignment is a class attribute (inside class body, not in method).
    ///
    /// In tree-sitter-python, assignments are wrapped in expression_statement nodes:
    ///   class_definition -> body: block -> expression_statement -> assignment
    fn is_class_attribute(&self, node: &tree_sitter::Node) -> bool {
        // Walk up the tree looking for a block whose parent is a class_definition
        let mut current = node.parent();
        while let Some(parent) = current {
            match parent.kind() {
                "function_definition" => return false, // Stop at function boundaries
                "class_definition" => return true,     // Found a class
                "block" => {
                    // Check if this block's parent is a class_definition
                    if let Some(grandparent) = parent.parent()
                        && grandparent.kind() == "class_definition"
                    {
                        return true;
                    }
                    // Continue looking up
                }
                _ => {}
            }
            current = parent.parent();
        }
        false
    }

    /// Check if an assignment is a module-level constant (PEP 8 convention).
    ///
    /// Returns `true` if the LHS identifier matches `^[A-Z][A-Z0-9_]+$` (at least 2 chars)
    /// and the assignment is at module level (not inside a function or class).
    fn is_constant_assignment(&self, node: &tree_sitter::Node, source: &[u8]) -> bool {
        // Must be at module level — not inside a function or class
        let mut current = node.parent();
        while let Some(parent) = current {
            match parent.kind() {
                "module" => break,
                "function_definition" | "class_definition" => return false,
                _ => current = parent.parent(),
            }
        }

        // Check if LHS identifier matches UPPER_CASE pattern (at least 2 chars)
        if let Some(left) = node.child_by_field_name("left")
            && left.kind() == "identifier"
            && let Ok(text) = left.utf8_text(source)
        {
            return text.len() >= 2
                && text.starts_with(|c: char| c.is_ascii_uppercase())
                && text
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        }
        false
    }

    /// Check if an identifier is in a parameter context.
    fn is_parameter_context(&self, node: &tree_sitter::Node) -> bool {
        if let Some(parent) = node.parent() {
            matches!(
                parent.kind(),
                "parameters" | "typed_parameter" | "default_parameter" | "typed_default_parameter"
            )
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};
    use act_lite_core::registry::{LANGUAGE_REGISTRY, LanguageId};

    crate::language_support_tests!(PythonSupport, Grammar::Python, &["py", "pyi", "pyw"]);

    #[test]
    fn registry_has_python() {
        let py_meta = LANGUAGE_REGISTRY.get(LanguageId("python"));
        assert!(py_meta.is_some(), "Python should be registered");
        let meta = py_meta.unwrap();
        assert_eq!(meta.display_name, "Python");
        assert!(meta.extensions.contains(&"py"));
        assert!(meta.extensions.contains(&"pyi"));
        assert!(meta.extensions.contains(&"pyw"));
    }

    #[test]
    fn registry_extension_lookup_python() {
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("py"),
            Some(LanguageId("python"))
        );
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("pyi"),
            Some(LanguageId("python"))
        );
        assert_eq!(
            LANGUAGE_REGISTRY.from_extension("pyw"),
            Some(LanguageId("python"))
        );
    }

    /// Helper: parse code, find first node of `target_kind`, return its NodeKind.
    fn parse_and_get_kind(code: &str, target_kind: &str) -> Option<NodeKind> {
        let support = PythonSupport;
        let tree = parse_with_support(&support, code);
        find_node_by_kind(tree.root_node(), target_kind).map(|n| support.node_kind(&n))
    }

    /// Helper: parse code, find first node of `target_kind`, return its NodeKind using source.
    fn parse_and_get_kind_with_source(code: &str, target_kind: &str) -> Option<NodeKind> {
        let support = PythonSupport;
        let tree = parse_with_support(&support, code);
        find_node_by_kind(tree.root_node(), target_kind)
            .map(|n| support.node_kind_with_source(&n, code.as_bytes()))
    }

    #[test]
    fn node_kind_function_definition() {
        let kind = parse_and_get_kind("def foo(): pass", "function_definition");
        assert_eq!(kind, Some(NodeKind::Function));
    }

    #[test]
    fn node_kind_async_function() {
        let kind = parse_and_get_kind("async def foo(): pass", "function_definition");
        assert_eq!(kind, Some(NodeKind::Function));
    }

    #[test]
    fn node_kind_lambda() {
        let kind = parse_and_get_kind("f = lambda x: x", "lambda");
        assert_eq!(kind, Some(NodeKind::Function));
    }

    #[test]
    fn node_kind_class_definition() {
        let kind = parse_and_get_kind("class Foo: pass", "class_definition");
        assert_eq!(kind, Some(NodeKind::Class));
    }

    #[test]
    fn node_kind_assignment() {
        let kind = parse_and_get_kind("x = 1", "assignment");
        assert_eq!(kind, Some(NodeKind::Variable));
    }

    #[test]
    fn node_kind_import_statement() {
        let kind = parse_and_get_kind("import os", "import_statement");
        assert_eq!(kind, Some(NodeKind::Import));
    }

    #[test]
    fn node_kind_import_from_statement() {
        let kind = parse_and_get_kind("from os import path", "import_from_statement");
        assert_eq!(kind, Some(NodeKind::Import));
    }

    #[test]
    fn node_kind_call() {
        let kind = parse_and_get_kind("foo()", "call");
        assert_eq!(kind, Some(NodeKind::Call));
    }

    #[test]
    fn node_kind_identifier() {
        let kind = parse_and_get_kind("x", "identifier");
        assert_eq!(kind, Some(NodeKind::Identifier));
    }

    #[test]
    fn node_kind_comment() {
        let kind = parse_and_get_kind("# comment\nx", "comment");
        assert_eq!(kind, Some(NodeKind::Comment));
    }

    #[test]
    fn node_kind_decorator() {
        let kind = parse_and_get_kind("@decorator\ndef foo(): pass", "decorator");
        assert_eq!(kind, Some(NodeKind::Decorator));
    }

    #[test]
    fn node_kind_list_comprehension() {
        let kind = parse_and_get_kind("[x for x in range(10)]", "list_comprehension");
        assert_eq!(kind, Some(NodeKind::Expression));
    }

    #[test]
    fn node_kind_class_attribute_top_level() {
        // Top-level class assignment is detected as Property
        let kind = parse_and_get_kind("class C:\n    x = 1", "assignment");
        assert_eq!(kind, Some(NodeKind::Property));
    }

    #[test]
    fn node_kind_method_vs_function() {
        // Method inside class should be detected as Method
        let method_kind =
            parse_and_get_kind("class C:\n    def m(self): pass", "function_definition");
        assert_eq!(method_kind, Some(NodeKind::Method));

        // Function at module level should be detected as Function
        let func_kind = parse_and_get_kind("def f(): pass", "function_definition");
        assert_eq!(func_kind, Some(NodeKind::Function));
    }

    #[test]
    fn node_kind_decorated_method() {
        // Decorated method should still be detected as Method
        let method_kind = parse_and_get_kind(
            "class C:\n    @decorator\n    def m(self): pass",
            "function_definition",
        );
        assert_eq!(method_kind, Some(NodeKind::Method));
    }

    #[test]
    fn node_kind_decorated_function() {
        // Decorated function should be detected as Function
        let func_kind = parse_and_get_kind("@decorator\ndef f(): pass", "function_definition");
        assert_eq!(func_kind, Some(NodeKind::Function));
    }

    #[test]
    fn node_kind_class_attribute_in_if_block() {
        // Assignment inside if block within class - currently detected as Property
        // since is_class_attribute only checks if parent is class block
        let kind = parse_and_get_kind("class C:\n    if True:\n        y = 2", "assignment");
        // This is the current behavior - assignments in nested blocks within classes
        // are still detected as Property
        assert_eq!(kind, Some(NodeKind::Property));
    }

    #[test]
    fn node_kind_class_attribute_with_type_annotation() {
        // Class attribute with type annotation
        let kind = parse_and_get_kind("class C:\n    x: int = 1", "assignment");
        assert_eq!(kind, Some(NodeKind::Property));
    }

    #[test]
    fn node_kind_parameter_in_function() {
        // Test that parameters are correctly identified
        let support = PythonSupport;
        let tree = parse_with_support(&support, "def f(x: int) -> None: pass");

        // Find the typed_parameter node and check its child identifier
        let typed_param = find_node_by_kind(tree.root_node(), "typed_parameter").unwrap();
        let mut cursor = typed_param.walk();
        let ident = typed_param
            .children(&mut cursor)
            .find(|c| c.kind() == "identifier")
            .unwrap();
        assert_eq!(support.node_kind(&ident), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_type_annotation() {
        let kind = parse_and_get_kind("x: int", "type");
        assert_eq!(kind, Some(NodeKind::Type));
    }

    #[test]
    fn node_kind_generic_type() {
        let kind = parse_and_get_kind("x: List[str]", "generic_type");
        assert_eq!(kind, Some(NodeKind::Type));
    }

    #[test]
    fn node_kind_constant_uppercase_module_level() {
        // PEP 8: UPPER_CASE identifiers at module level are constants
        let kind = parse_and_get_kind_with_source("MAX_RETRIES = 3", "assignment");
        assert_eq!(kind, Some(NodeKind::Constant));
    }

    #[test]
    fn node_kind_constant_single_letter_uppercase() {
        // Single uppercase letter is not a constant (too ambiguous, requires >= 2 chars)
        let kind = parse_and_get_kind_with_source("X = 1", "assignment");
        assert_eq!(kind, Some(NodeKind::Variable));
    }

    #[test]
    fn node_kind_variable_lowercase_module_level() {
        // Lowercase at module level stays Variable
        let kind = parse_and_get_kind_with_source("my_var = 3", "assignment");
        assert_eq!(kind, Some(NodeKind::Variable));
    }

    #[test]
    fn node_kind_class_attribute_uppercase_stays_property() {
        // Class attributes stay Property even if UPPER_CASE
        let kind = parse_and_get_kind_with_source("class C:\n    MY_CONST = 1", "assignment");
        assert_eq!(kind, Some(NodeKind::Property));
    }

    #[test]
    fn function_keyword_is_def() {
        let support = PythonSupport;
        assert_eq!(support.function_keyword(), "def");
    }

    #[test]
    fn stub_placeholders_returns_language_patterns() {
        let support = PythonSupport;
        let placeholders = support.stub_placeholders();
        assert!(!placeholders.is_empty());
        assert!(placeholders.contains(&"pass"));
        assert!(placeholders.contains(&"raise NotImplementedError"));
    }

    #[test]
    fn cfg_hooks_python() {
        let p = PythonSupport;
        assert!(p.cfg_branch_kinds().contains(&"if_statement"));
        assert!(p.cfg_exception_handler_kinds().contains(&"except_clause"));
        assert!(p.cfg_throw_kinds().contains(&"raise_statement"));
        assert!(p.cfg_modeled_kinds().contains("exception"));
    }

    #[test]
    fn moat_call_node_is_call_only() {
        let p = PythonSupport;
        // Python's real call node is `call`; overriding away from the
        // `[call_expression, call]` default declares it precisely.
        assert_eq!(p.call_node_kinds(), &["call"]);
        let tree = parse_with_support(&p, "foo()");
        assert!(find_node_by_kind(tree.root_node(), "call").is_some());
    }

    #[test]
    fn field_access_node_kinds_is_attribute() {
        assert_eq!(PythonSupport.field_access_node_kinds(), &["attribute"]);
    }
}
