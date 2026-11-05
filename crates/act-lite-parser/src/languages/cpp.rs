//! C++ language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register C++ in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("cpp"),
        display_name: "C++",
        extensions: &["cpp", "cc", "cxx", "hpp", "h"],
        category: LanguageCategory::Systems,
    }
}

/// C++ language support implementation.
#[derive(Default)]
pub struct CppSupport;

impl LanguageSupport for CppSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Cpp
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_cpp::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Functions
            "function_definition" => NodeKind::Function,
            "function_declarator" => NodeKind::Function,
            "lambda_expression" => NodeKind::Function,

            // Classes and structs
            "class_specifier" => NodeKind::Class,
            "struct_specifier" => NodeKind::Struct,
            "union_specifier" => NodeKind::Struct,
            "enum_specifier" => NodeKind::Enum,

            // Templates
            "template_declaration" => self.classify_template(node),
            "template_parameter_list" => NodeKind::Type,
            "type_parameter_declaration" => NodeKind::Type,

            // Namespaces
            "namespace_definition" => NodeKind::Module,
            "namespace_alias_definition" => NodeKind::Module,

            // Type definitions
            "alias_declaration" => NodeKind::TypeAlias,
            "type_definition" => NodeKind::TypeAlias,

            // Members
            "field_declaration" => NodeKind::Property,
            "access_specifier" => NodeKind::Unknown,
            "base_class_clause" => NodeKind::Type,

            // Variables
            "declaration" => self.classify_declaration(node),
            "init_declarator" => NodeKind::Variable,
            "enumerator" => NodeKind::Property,

            // Parameters
            "parameter_declaration" => NodeKind::Parameter,
            "parameter_list" => NodeKind::Parameter,

            // Preprocessor
            "preproc_include" => NodeKind::Import,
            "preproc_def" | "preproc_function_def" => NodeKind::Variable,
            "preproc_ifdef" | "preproc_if" | "preproc_else" => NodeKind::Block,

            // Using declarations
            // Note: tree-sitter-cpp represents both `using std::cout;` and
            // `using namespace std;` as `using_declaration`. There is no
            // `using_directive` node kind in this grammar.
            "using_declaration" => NodeKind::Import,

            // Statements and blocks
            "compound_statement" => NodeKind::Block,
            "if_statement" | "switch_statement" => NodeKind::Block,
            "for_statement" | "while_statement" | "do_statement" => NodeKind::Block,
            "for_range_loop" => NodeKind::Block,
            "try_statement" | "catch_clause" => NodeKind::Block,

            // Expressions
            "call_expression" => NodeKind::Call,
            "new_expression" => NodeKind::Call,
            "delete_expression" => NodeKind::Call,

            // Identifiers
            "identifier" => NodeKind::Identifier,
            "type_identifier" => NodeKind::Type,
            "namespace_identifier" => NodeKind::Identifier,
            "field_identifier" => NodeKind::Identifier,
            "qualified_identifier" => NodeKind::Identifier,

            // Comments
            "comment" => NodeKind::Comment,

            // Modern C++ features
            "concept_definition" => NodeKind::Interface,
            "requires_clause" => NodeKind::Type,
            "structured_binding_declarator" => NodeKind::Variable,
            "lambda_capture_specifier" => NodeKind::Parameter,

            // Operator and destructor names
            "operator_name" => NodeKind::Function, // operator== operator< etc.
            "destructor_name" => NodeKind::Function, // ~Foo

            // C++20 coroutines
            "co_await_expression" => NodeKind::Call,
            "co_yield_statement" => NodeKind::Call,
            "co_return_statement" => NodeKind::Block,

            // Static assertions
            "static_assert_declaration" => NodeKind::Unknown,

            // Attributes — attribute_declaration is [[...]], attribute_specifier is __attribute__((..))
            "attribute_declaration" | "attribute_specifier" | "attribute" => NodeKind::Decorator,

            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cpp", "cxx", "cc", "hpp", "hxx", "hh"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/cpp/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/cpp/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/cpp/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &["function_definition", "lambda_expression"]
    }

    fn function_body_kind(&self) -> &'static str {
        "compound_statement"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "switch_statement",
            "for_statement",
            "for_range_loop",
            "while_statement",
            "do_statement",
            "case_statement",
            "catch_clause",
        ]
    }

    fn logical_operator_info(&self) -> Option<(&'static str, &'static [&'static str])> {
        Some(("binary_expression", &["&&", "||"]))
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "compound_statement",
            "if_statement",
            "for_statement",
            "for_range_loop",
            "while_statement",
            "do_statement",
            "switch_statement",
            "case_statement",
            "lambda_expression",
            "try_statement",
            "binary_expression",
            "conditional_expression",
            "call_expression",
            "assignment_expression",
        ]
    }

    // --- call_detection: real call nodes are call/new/delete + coroutine ops. ---
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &[
            "call_expression",
            "new_expression",
            "delete_expression",
            "co_await_expression",
            "co_yield_statement",
        ]
    }

    // --- effect_summary: C++ throws, allocates with `new`, awaits with co_await. ---
    fn raise_node_kinds(&self) -> &'static [&'static str] {
        &["throw_statement"]
    }

    fn await_node_kinds(&self) -> &'static [&'static str] {
        &["co_await_expression"]
    }

    fn allocation_node_kinds(&self) -> &'static [&'static str] {
        &["new_expression"]
    }

    fn allocation_call_names(&self) -> &'static [&'static str] {
        &[
            "malloc",
            "calloc",
            "realloc",
            "make_unique",
            "make_shared",
            "fopen",
        ]
    }

    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["sleep", "sleep_for", "recv", "read", "accept", "join"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("raise")
            .with("await")
            .with("allocate")
            .with("block")
    }

    // --- data_flow ---
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
        &["mysql_query", "sqlite3_exec", "PQexec", "exec"]
    }

    fn ffi_markers(&self) -> &'static [&'static str] {
        // dynamic loading + `extern "C"` linkage specification.
        &[
            "dlopen",
            "dlsym",
            "LoadLibrary",
            "GetProcAddress",
            "linkage_specification",
        ]
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
        &["getenv", "getline", "scanf", "read", "recv"]
    }

    fn command_exec_names(&self) -> &'static [&'static str] {
        &[
            "system", "popen", "execl", "execlp", "execv", "execvp", "execve",
        ]
    }

    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["fopen", "open", "openat", "freopen", "write"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

impl CppSupport {
    /// Classify a template declaration based on its contents.
    fn classify_template(&self, node: &tree_sitter::Node) -> NodeKind {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "class_specifier" | "struct_specifier" => return NodeKind::Class,
                "function_definition" => return NodeKind::Function,
                "alias_declaration" => return NodeKind::TypeAlias,
                _ => {}
            }
        }
        NodeKind::Type
    }

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

    crate::language_support_tests!(
        CppSupport,
        Grammar::Cpp,
        &["cpp", "cxx", "cc", "hpp", "hxx", "hh"]
    );

    #[test]
    fn node_kind_function_definition() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int foo() { return 0; }");
        let func_node = find_node_by_kind(tree.root_node(), "function_definition").unwrap();
        assert_eq!(support.node_kind(&func_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_class_specifier() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "class Foo { public: int x; };");
        let class_node = find_node_by_kind(tree.root_node(), "class_specifier").unwrap();
        assert_eq!(support.node_kind(&class_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_namespace_definition() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "namespace foo { int x; }");
        let ns_node = find_node_by_kind(tree.root_node(), "namespace_definition").unwrap();
        assert_eq!(support.node_kind(&ns_node), NodeKind::Module);
    }

    #[test]
    fn node_kind_template_class() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "template<typename T> class Container {};");
        let template_node = find_node_by_kind(tree.root_node(), "template_declaration").unwrap();
        assert_eq!(support.node_kind(&template_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_lambda_expression() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "auto f = []() { return 42; };");
        let lambda_node = find_node_by_kind(tree.root_node(), "lambda_expression").unwrap();
        assert_eq!(support.node_kind(&lambda_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_using_declaration() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "using std::cout;");
        let using_node = find_node_by_kind(tree.root_node(), "using_declaration").unwrap();
        assert_eq!(support.node_kind(&using_node), NodeKind::Import);
    }

    #[test]
    fn node_kind_comment() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "// comment\nint x;");
        let comment_node = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_struct_specifier() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "struct Point { int x; int y; };");
        let struct_node = find_node_by_kind(tree.root_node(), "struct_specifier").unwrap();
        assert_eq!(support.node_kind(&struct_node), NodeKind::Struct);
    }

    #[test]
    fn node_kind_enum_specifier() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "enum Color { RED, GREEN, BLUE };");
        let enum_node = find_node_by_kind(tree.root_node(), "enum_specifier").unwrap();
        assert_eq!(support.node_kind(&enum_node), NodeKind::Enum);
    }

    #[test]
    fn node_kind_alias_declaration() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "using MyInt = int;");
        let alias_node = find_node_by_kind(tree.root_node(), "alias_declaration").unwrap();
        assert_eq!(support.node_kind(&alias_node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_field_declaration() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "class Foo { int x; };");
        let field_node = find_node_by_kind(tree.root_node(), "field_declaration").unwrap();
        assert_eq!(support.node_kind(&field_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_access_specifier() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "class Foo { public: int x; };");
        let access_node = find_node_by_kind(tree.root_node(), "access_specifier").unwrap();
        assert_eq!(support.node_kind(&access_node), NodeKind::Unknown);
    }

    #[test]
    fn node_kind_base_class_clause() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "class Foo : public Base {};");
        let base_node = find_node_by_kind(tree.root_node(), "base_class_clause").unwrap();
        assert_eq!(support.node_kind(&base_node), NodeKind::Type);
    }

    #[test]
    fn node_kind_parameter_declaration() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int foo(int x) { return x; }");
        let param_node = find_node_by_kind(tree.root_node(), "parameter_declaration").unwrap();
        assert_eq!(support.node_kind(&param_node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_preproc_include() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "#include <iostream>");
        let include_node = find_node_by_kind(tree.root_node(), "preproc_include").unwrap();
        assert_eq!(support.node_kind(&include_node), NodeKind::Import);
    }

    #[test]
    fn node_kind_using_directive() {
        // tree-sitter-cpp represents `using namespace std;` as `using_declaration`,
        // not a separate `using_directive` node kind. Both forms of `using` produce
        // `using_declaration` in this grammar.
        let support = CppSupport;
        let tree = parse_with_support(&support, "using namespace std;");
        let node = find_node_by_kind(tree.root_node(), "using_declaration")
            .expect("expected tree-sitter to produce a using_declaration node from the test input");
        assert_eq!(support.node_kind(&node), NodeKind::Import);
    }

    #[test]
    fn node_kind_call_expression() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int main() { foo(); return 0; }");
        let call_node = find_node_by_kind(tree.root_node(), "call_expression").unwrap();
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_new_expression() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int* x = new int(42);");
        let new_node = find_node_by_kind(tree.root_node(), "new_expression").unwrap();
        assert_eq!(support.node_kind(&new_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_delete_expression() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "delete x;");
        let delete_node = find_node_by_kind(tree.root_node(), "delete_expression").unwrap();
        assert_eq!(support.node_kind(&delete_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_identifier() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int x = 5;");
        let id_node = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        assert_eq!(support.node_kind(&id_node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_type_identifier() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "MyClass* x = nullptr;");
        let type_id_node = find_node_by_kind(tree.root_node(), "type_identifier").unwrap();
        assert_eq!(support.node_kind(&type_id_node), NodeKind::Type);
    }

    #[test]
    fn node_kind_namespace_identifier() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "std::cout << \"hello\";");
        let ns_id_node = find_node_by_kind(tree.root_node(), "namespace_identifier").unwrap();
        assert_eq!(support.node_kind(&ns_id_node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_qualified_identifier() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "std::vector<int> v;");
        let qual_id_node = find_node_by_kind(tree.root_node(), "qualified_identifier").unwrap();
        assert_eq!(support.node_kind(&qual_id_node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_concept_definition() {
        let support = CppSupport;
        let tree = parse_with_support(
            &support,
            "template<typename T> concept Integral = std::is_integral_v<T>;",
        );
        let concept_node = find_node_by_kind(tree.root_node(), "concept_definition").unwrap();
        assert_eq!(support.node_kind(&concept_node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_requires_clause() {
        let support = CppSupport;
        let tree = parse_with_support(
            &support,
            "template<typename T> requires Integral<T> void foo() {}",
        );
        let requires_node = find_node_by_kind(tree.root_node(), "requires_clause").unwrap();
        assert_eq!(support.node_kind(&requires_node), NodeKind::Type);
    }

    #[test]
    fn node_kind_structured_binding_declarator() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "auto [x, y] = std::pair<int, int>{1, 2};");
        let binding_node =
            find_node_by_kind(tree.root_node(), "structured_binding_declarator").unwrap();
        assert_eq!(support.node_kind(&binding_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_lambda_capture_specifier() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "auto f = [x = 42]() { return x; };");
        let capture_node = find_node_by_kind(tree.root_node(), "lambda_capture_specifier").unwrap();
        assert_eq!(support.node_kind(&capture_node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_enumerator() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "enum Color { RED };");
        let enum_node = find_node_by_kind(tree.root_node(), "enumerator").unwrap();
        assert_eq!(support.node_kind(&enum_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_init_declarator() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int x = 5;");
        let init_node = find_node_by_kind(tree.root_node(), "init_declarator").unwrap();
        assert_eq!(support.node_kind(&init_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_for_range_loop() {
        let support = CppSupport;
        let tree = parse_with_support(
            &support,
            "int main() { int arr[] = {1,2,3}; for (int x : arr) {} }",
        );
        let for_node = find_node_by_kind(tree.root_node(), "for_range_loop").unwrap();
        assert_eq!(support.node_kind(&for_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_try_statement() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int main() { try { foo(); } catch (...) {} }");
        let try_node = find_node_by_kind(tree.root_node(), "try_statement").unwrap();
        assert_eq!(support.node_kind(&try_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_catch_clause() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int main() { try { foo(); } catch (...) {} }");
        let catch_node = find_node_by_kind(tree.root_node(), "catch_clause").unwrap();
        assert_eq!(support.node_kind(&catch_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_compound_statement() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int main() { return 0; }");
        let block_node = find_node_by_kind(tree.root_node(), "compound_statement").unwrap();
        assert_eq!(support.node_kind(&block_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_if_statement() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int main() { if (true) return 0; return 1; }");
        let if_node = find_node_by_kind(tree.root_node(), "if_statement").unwrap();
        assert_eq!(support.node_kind(&if_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_template_function() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "template<typename T> T foo(T x) { return x; }");
        let template_node = find_node_by_kind(tree.root_node(), "template_declaration").unwrap();
        assert_eq!(support.node_kind(&template_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_template_alias() {
        let support = CppSupport;
        let tree = parse_with_support(
            &support,
            "template<typename T> using MyVec = std::vector<T>;",
        );
        let template_node = find_node_by_kind(tree.root_node(), "template_declaration").unwrap();
        assert_eq!(support.node_kind(&template_node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_declaration_classifies_function() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int foo();");
        let node = find_node_by_kind(tree.root_node(), "declaration")
            .expect("expected tree-sitter to produce a declaration node from the test input");
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_declaration_classifies_variable() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int x;");
        let decl_node = find_node_by_kind(tree.root_node(), "declaration").unwrap();
        assert_eq!(support.node_kind(&decl_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_union_specifier() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "union Value { int i; float f; };");
        let union_node = find_node_by_kind(tree.root_node(), "union_specifier").unwrap();
        assert_eq!(support.node_kind(&union_node), NodeKind::Struct);
    }

    #[test]
    fn node_kind_template_parameter_list() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "template<typename T, typename U> class Foo {};");
        let param_list_node =
            find_node_by_kind(tree.root_node(), "template_parameter_list").unwrap();
        assert_eq!(support.node_kind(&param_list_node), NodeKind::Type);
    }

    #[test]
    fn node_kind_type_parameter_declaration() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "template<typename T> void foo() {}");
        let type_param_node =
            find_node_by_kind(tree.root_node(), "type_parameter_declaration").unwrap();
        assert_eq!(support.node_kind(&type_param_node), NodeKind::Type);
    }

    #[test]
    fn node_kind_namespace_alias_definition() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "namespace MyAlias = std;");
        let ns_alias_node =
            find_node_by_kind(tree.root_node(), "namespace_alias_definition").unwrap();
        assert_eq!(support.node_kind(&ns_alias_node), NodeKind::Module);
    }

    #[test]
    fn node_kind_type_definition() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "typedef int MyInt;");
        let typedef_node = find_node_by_kind(tree.root_node(), "type_definition").unwrap();
        assert_eq!(support.node_kind(&typedef_node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_preproc_def() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "#define FOO 42");
        let def_node = find_node_by_kind(tree.root_node(), "preproc_def").unwrap();
        assert_eq!(support.node_kind(&def_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_function_declarator() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int foo(int x);");
        let func_decl_node = find_node_by_kind(tree.root_node(), "function_declarator").unwrap();
        assert_eq!(support.node_kind(&func_decl_node), NodeKind::Function);
    }

    #[test]
    fn function_parent_kinds_are_cpp_specific() {
        let support = CppSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"function_definition"));
        assert!(kinds.contains(&"lambda_expression"));
        // Must NOT contain JS/TS values
        assert!(!kinds.contains(&"arrow_function"));
        assert!(!kinds.contains(&"method_definition"));
    }

    #[test]
    fn function_body_kind_is_compound_statement() {
        let support = CppSupport;
        assert_eq!(support.function_body_kind(), "compound_statement");
    }

    #[test]
    fn decision_point_kinds_are_cpp_specific() {
        let support = CppSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_statement"));
        assert!(kinds.contains(&"for_range_loop"));
        assert!(kinds.contains(&"catch_clause"));
        assert!(kinds.contains(&"switch_statement"));
        // Must NOT contain JS/TS only values
        assert!(!kinds.contains(&"ternary_expression"));
        assert!(!kinds.contains(&"switch_case"));
    }

    // Fix 1: CPP-1
    #[test]
    fn function_keyword_is_empty_for_cpp() {
        let support = CppSupport;
        assert_eq!(
            support.function_keyword(),
            "",
            "C++ has no function keyword"
        );
    }

    // Fix 2: CPP-9
    #[test]
    fn logical_operator_info_is_explicit_for_cpp() {
        let support = CppSupport;
        let info = support.logical_operator_info();
        assert!(info.is_some());
        let (kind, ops) = info.unwrap();
        assert_eq!(kind, "binary_expression");
        assert!(ops.contains(&"&&"));
        assert!(ops.contains(&"||"));
    }

    // Fix 3: CPP-8
    #[test]
    fn extractable_node_kinds_are_defined_for_cpp() {
        let support = CppSupport;
        let kinds = support.extractable_node_kinds();
        assert!(!kinds.is_empty(), "C++ must define extractable node kinds");
        assert!(kinds.contains(&"compound_statement"));
        assert!(kinds.contains(&"lambda_expression"));
        assert!(kinds.contains(&"switch_statement"));
    }

    // Fix 4: CPP-3
    #[test]
    fn decision_point_kinds_includes_switch_statement() {
        let support = CppSupport;
        let kinds = support.decision_point_kinds();
        assert!(
            kinds.contains(&"switch_statement"),
            "switch_statement must be a decision point for cyclomatic complexity"
        );
    }

    // Fix 5: CPP-4 and CPP-5
    #[test]
    fn node_kind_operator_name() {
        let support = CppSupport;
        let tree = parse_with_support(
            &support,
            "bool operator==(const Foo& o) const { return true; }",
        );
        let op_node = find_node_by_kind(tree.root_node(), "operator_name")
            .expect("expected tree-sitter to produce an operator_name node from the test input");
        assert_eq!(
            support.node_kind(&op_node),
            NodeKind::Function,
            "operator_name should map to Function, not Unknown"
        );
    }

    #[test]
    fn node_kind_destructor_name() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "class Foo { ~Foo() {} };");
        let dtor_node = find_node_by_kind(tree.root_node(), "destructor_name")
            .expect("expected tree-sitter to produce a destructor_name node from the test input");
        assert_eq!(
            support.node_kind(&dtor_node),
            NodeKind::Function,
            "destructor_name should map to Function, not Unknown"
        );
    }

    // Fix 6: CPP-6
    #[test]
    fn node_kind_co_return_statement() {
        let support = CppSupport;
        let tree = parse_with_support(
            &support,
            "#include <coroutine>\nauto task() { co_return 42; }",
        );
        let node = find_node_by_kind(tree.root_node(), "co_return_statement")
            .expect("expected tree-sitter to produce a co_return_statement node");
        assert_eq!(
            support.node_kind(&node),
            NodeKind::Block,
            "co_return_statement must map to NodeKind::Block"
        );
    }

    // Fix 7: CPP-7
    #[test]
    fn node_kind_static_assert_declaration() {
        let support = CppSupport;
        let tree = parse_with_support(
            &support,
            "static_assert(sizeof(int) == 4, \"int must be 4 bytes\");",
        );
        let node = find_node_by_kind(tree.root_node(), "static_assert_declaration")
            .expect("expected tree-sitter to produce a static_assert_declaration node");
        assert_eq!(support.node_kind(&node), NodeKind::Unknown);
    }

    #[test]
    fn node_kind_while_statement() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int main() { while (true) {} }");
        let node = find_node_by_kind(tree.root_node(), "while_statement")
            .expect("while_statement must exist in the parsed tree");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_do_statement() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int main() { do {} while (true); }");
        let node = find_node_by_kind(tree.root_node(), "do_statement")
            .expect("do_statement must exist in the parsed tree");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_switch_statement() {
        let support = CppSupport;
        let tree = parse_with_support(
            &support,
            "int main() { int x = 1; switch (x) { case 1: break; } }",
        );
        let node = find_node_by_kind(tree.root_node(), "switch_statement")
            .expect("switch_statement must exist in the parsed tree");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_for_statement() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int main() { for (int i = 0; i < 10; i++) {} }");
        let node = find_node_by_kind(tree.root_node(), "for_statement")
            .expect("for_statement must exist in the parsed tree");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_parameter_list() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "void foo(int x, int y) {}");
        let node = find_node_by_kind(tree.root_node(), "parameter_list")
            .expect("parameter_list must exist in the parsed tree");
        assert_eq!(support.node_kind(&node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_preproc_function_def() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "#define MAX(a, b) ((a) > (b) ? (a) : (b))");
        let node = find_node_by_kind(tree.root_node(), "preproc_function_def")
            .expect("preproc_function_def must exist in the parsed tree");
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_preproc_ifdef() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "#ifdef NDEBUG\nint x;\n#endif");
        let node = find_node_by_kind(tree.root_node(), "preproc_ifdef")
            .expect("preproc_ifdef must exist in the parsed tree");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_preproc_if() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "#if defined(FOO)\nint x;\n#endif");
        let node = find_node_by_kind(tree.root_node(), "preproc_if")
            .expect("preproc_if must exist in the parsed tree");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_preproc_else() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "#if defined(FOO)\nint x;\n#else\nint y;\n#endif");
        let node = find_node_by_kind(tree.root_node(), "preproc_else")
            .expect("preproc_else must exist in the parsed tree");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_field_identifier() {
        let support = CppSupport;
        let tree = parse_with_support(
            &support,
            "struct Foo { int x; }; void bar() { Foo f; f.x = 1; }",
        );
        let node = find_node_by_kind(tree.root_node(), "field_identifier")
            .expect("field_identifier must exist in the parsed tree");
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_co_await_expression() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "auto f() { co_await something(); }");
        let node = find_node_by_kind(tree.root_node(), "co_await_expression")
            .expect("expected tree-sitter to produce a co_await_expression node");
        assert_eq!(
            support.node_kind(&node),
            NodeKind::Call,
            "co_await_expression must map to NodeKind::Call"
        );
    }

    #[test]
    fn node_kind_co_yield_statement() {
        // tree-sitter-cpp produces a `co_yield_statement` node (not `co_yield_expression`)
        // for `co_yield` expressions. The match arm maps this to NodeKind::Call.
        let support = CppSupport;
        let tree = parse_with_support(&support, "auto f() { co_yield 42; }");
        let node = find_node_by_kind(tree.root_node(), "co_yield_statement")
            .expect("expected tree-sitter to produce a co_yield_statement node");
        assert_eq!(
            support.node_kind(&node),
            NodeKind::Call,
            "co_yield_statement must map to NodeKind::Call"
        );
    }

    #[test]
    fn node_kind_case_statement_falls_to_unknown() {
        let support = CppSupport;
        let tree = parse_with_support(
            &support,
            "int main() { int x = 1; switch (x) { case 1: break; } }",
        );
        let node = find_node_by_kind(tree.root_node(), "case_statement")
            .expect("expected tree-sitter to produce a case_statement node");
        assert_eq!(
            support.node_kind(&node),
            NodeKind::Unknown,
            "case_statement is not in the node_kind match arm and must fall through to Unknown"
        );
    }

    #[test]
    fn node_kind_conditional_expression_falls_to_unknown() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int main() { int x = 1 ? 2 : 3; }");
        let node = find_node_by_kind(tree.root_node(), "conditional_expression")
            .expect("expected tree-sitter to produce a conditional_expression node");
        assert_eq!(
            support.node_kind(&node),
            NodeKind::Unknown,
            "conditional_expression is not in the node_kind match arm and must fall through to Unknown"
        );
    }

    #[test]
    fn node_kind_assignment_expression_falls_to_unknown() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "int main() { int x; x = 42; }");
        let node = find_node_by_kind(tree.root_node(), "assignment_expression")
            .expect("expected tree-sitter to produce an assignment_expression node");
        assert_eq!(
            support.node_kind(&node),
            NodeKind::Unknown,
            "assignment_expression is not in the node_kind match arm and must fall through to Unknown"
        );
    }

    // Fix 8: CPP-2 — rewritten attribute tests (non-vacuous, unconditional assertions)
    //
    // The C++ tree-sitter grammar parses [[nodiscard]] int foo(); as:
    //   declaration
    //     attribute_declaration  ← the [[...]] wrapper
    //       attribute            ← the individual item (nodiscard)
    //         identifier
    //     ...
    #[test]
    fn node_kind_attribute_specifier() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "[[nodiscard]] int foo();");
        // attribute_declaration is the [[...]] container node in the C++ grammar
        let attr_node = find_node_by_kind(tree.root_node(), "attribute_declaration")
            .expect("[[nodiscard]] must produce an attribute_declaration node in the C++ AST");
        assert_eq!(
            support.node_kind(&attr_node),
            NodeKind::Decorator,
            "attribute_declaration must map to NodeKind::Decorator"
        );
    }

    #[test]
    fn node_kind_attribute_in_declaration() {
        let support = CppSupport;
        let tree = parse_with_support(&support, "[[nodiscard]] int foo();");
        // attribute is the individual item node inside [[...]] in the C++ grammar
        let attr_node = find_node_by_kind(tree.root_node(), "attribute").expect(
            "[[nodiscard]] must produce an attribute node inside the attribute_declaration",
        );
        assert_eq!(
            support.node_kind(&attr_node),
            NodeKind::Decorator,
            "attribute nodes must map to NodeKind::Decorator"
        );
    }

    #[test]
    fn node_kind_gcc_attribute_specifier() {
        let support = CppSupport;
        // GCC __attribute__((...)) syntax produces an `attribute_specifier` node.
        let tree = parse_with_support(&support, "int foo() __attribute__((noinline));");
        let node = find_node_by_kind(tree.root_node(), "attribute_specifier")
            .expect("expected tree-sitter to produce an attribute_specifier node for __attribute__((noinline))");
        assert_eq!(
            support.node_kind(&node),
            NodeKind::Decorator,
            "attribute_specifier (__attribute__((..))) must map to NodeKind::Decorator"
        );
    }

    #[test]
    fn registry_has_cpp() {
        use act_lite_core::registry::{LANGUAGE_REGISTRY, LanguageId};
        let meta = LANGUAGE_REGISTRY.get(LanguageId("cpp"));
        assert!(meta.is_some(), "C++ should be registered");
        let meta = meta.unwrap();
        assert_eq!(meta.display_name, "C++");
        assert!(meta.extensions.contains(&"cpp"));
        assert!(meta.extensions.contains(&"hpp"));
    }

    #[test]
    #[ignore]
    fn debug_cpp_dlopen_node_types() {
        let code = r#"
void *risky(void *b) {
    void *lib = dlopen("lib.so", 1);
    return lib;
}
"#;
        let support = CppSupport;
        let tree = parse_with_support(&support, code);
        fn print_tree(node: tree_sitter::Node, src: &[u8], indent: usize) {
            let text = node
                .utf8_text(src)
                .unwrap_or("")
                .chars()
                .take(40)
                .collect::<String>();
            println!(
                "{}{} [{},{}] {:?}",
                "  ".repeat(indent),
                node.kind(),
                node.start_position().row + 1,
                node.start_position().column,
                text
            );
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                print_tree(child, src, indent + 1);
            }
        }
        print_tree(tree.root_node(), code.as_bytes(), 0);
    }

    // --- moat coverage hooks (B2) ---

    #[test]
    fn coverage_call_detection_overrides_default() {
        let s = CppSupport;
        assert!(s.call_node_kinds().contains(&"call_expression"));
        assert!(s.call_node_kinds().contains(&"new_expression"));
        let tree = parse_with_support(&s, "void f() { int *p = new int(5); g(); }");
        assert!(find_node_by_kind(tree.root_node(), "new_expression").is_some());
    }

    #[test]
    fn coverage_effect_summary_widened() {
        let s = CppSupport;
        let baseline = act_lite_core::KindMask::new().with("read").with("write");
        assert_ne!(s.effect_modeled_kinds(), baseline);
        assert!(s.raise_node_kinds().contains(&"throw_statement"));
        assert!(s.allocation_node_kinds().contains(&"new_expression"));
        let tree = parse_with_support(&s, "void f() { throw std::runtime_error(\"e\"); }");
        assert!(find_node_by_kind(tree.root_node(), "throw_statement").is_some());
    }

    #[test]
    fn coverage_data_flow_models_bindings() {
        let s = CppSupport;
        assert!(!s.data_flow_modeled_kinds().is_empty());
        assert!(s.binding_node_kinds().contains(&"declaration"));
    }

    #[test]
    fn coverage_unsafe_surface_modeled() {
        let s = CppSupport;
        assert!(!s.unsafe_modeled_kinds().is_empty());
        assert!(s.eval_call_names().contains(&"system"));
        assert!(s.ffi_markers().contains(&"dlopen"));
    }

    #[test]
    fn coverage_secret_surface_modeled() {
        let s = CppSupport;
        assert!(!s.secret_modeled_kinds().is_empty());
        assert!(s.env_read_names().contains(&"getenv"));
        assert!(s.crypto_api_names().contains(&"HMAC"));
    }

    #[test]
    fn coverage_taint_flow_modeled() {
        let s = CppSupport;
        assert!(!s.taint_modeled_kinds().is_empty());
        assert!(s.command_exec_names().contains(&"execvp"));
        assert!(s.fs_sink_names().contains(&"fopen"));
    }
}
