//! Swift language support.

use crate::Grammar;
use crate::language::{CommentDelimiters, LanguageSupport};
use crate::node_kind::NodeKind;
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Swift in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("swift"),
        display_name: "Swift",
        extensions: &["swift"],
        category: LanguageCategory::Systems,
    }
}

/// Swift language support implementation.
pub struct SwiftSupport;

impl LanguageSupport for SwiftSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Swift
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_swift::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Functions
            "function_declaration" => NodeKind::Function,
            "init_declaration" => NodeKind::Constructor,
            "deinit_declaration" => NodeKind::Method,
            "subscript_declaration" => NodeKind::Method,

            // Classes and types
            // Note: struct, actor, enum, and extension are all represented as
            // class_declaration in tree-sitter-swift 0.6. Use node_kind_with_source
            // to inspect the declaration_kind child field for source-aware dispatch.
            "class_declaration" => NodeKind::Class,
            "protocol_declaration" => NodeKind::Interface,
            "typealias_declaration" => NodeKind::TypeAlias,
            "associatedtype_declaration" => NodeKind::TypeAlias,

            // Members
            "property_declaration" => self.classify_property(node),
            "computed_property" => NodeKind::Property,
            "computed_getter" | "computed_setter" => NodeKind::Method,
            "willset_clause" | "didset_clause" => NodeKind::Method,
            "enum_entry" => NodeKind::Property,

            // Protocol requirements
            "protocol_function_declaration" => NodeKind::Method,
            "protocol_property_declaration" => NodeKind::Property,

            // Parameters
            "parameter" | "closure_parameter" => NodeKind::Parameter,
            "type_parameter" => NodeKind::Parameter,

            // Variables
            "pattern" => NodeKind::Variable,

            // Imports
            "import_declaration" => NodeKind::Import,

            // Blocks and control flow
            "code_block" | "statements" => NodeKind::Block,
            "if_statement" | "guard_statement" => NodeKind::Block,
            "switch_statement" | "switch_entry" => NodeKind::Block,
            "for_statement" | "while_statement" | "repeat_while_statement" => NodeKind::Block,
            "do_statement" | "catch_block" => NodeKind::Block,

            // Calls
            "call_expression" => NodeKind::Call,
            "constructor_call" => NodeKind::Call,

            // Identifiers
            "simple_identifier" | "type_identifier" => NodeKind::Identifier,

            // Comments
            "comment" | "multiline_comment" => NodeKind::Comment,

            // Attributes
            "attribute" => NodeKind::Decorator,

            // Types
            "user_type" | "optional_type" | "array_type" | "dictionary_type" => NodeKind::Type,
            "tuple_type" | "function_type" | "protocol_composition_type" => NodeKind::Type,
            "type_arguments" | "type_parameters" => NodeKind::Type,

            // Closures
            "lambda_literal" | "closure_expression" => NodeKind::Function,

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn node_kind_with_source(&self, node: &tree_sitter::Node, source: &[u8]) -> NodeKind {
        if node.kind() == "class_declaration"
            && let Some(kind_node) = node.child_by_field_name("declaration_kind")
        {
            return match &source[kind_node.byte_range()] {
                b"enum" => NodeKind::Enum,
                _ => NodeKind::Class, // struct, class, actor, extension all map to Class
            };
        }
        self.node_kind(node)
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["swift"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/swift/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/swift/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/swift/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }

    fn function_keyword(&self) -> &'static str {
        "func"
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &[
            "function_declaration",
            "init_declaration",
            "protocol_function_declaration",
            "lambda_literal",
        ]
    }

    fn function_body_kind(&self) -> &'static str {
        "function_body"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "guard_statement",
            "switch_statement",
            "for_statement",
            "while_statement",
            "repeat_while_statement",
            "catch_block",
            "switch_entry",
        ]
    }

    fn logical_operator_info(&self) -> Option<(&'static str, &'static [&'static str])> {
        // Swift uses conjunction_expression and disjunction_expression node kinds
        // rather than operators inside a binary_expression.
        // These are not currently counted by the complexity calculator.
        None
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "function_body",
            "code_block",
            "if_statement",
            "guard_statement",
            "switch_statement",
            "for_statement",
            "while_statement",
            "repeat_while_statement",
            "catch_block",
            "do_statement",
            "call_expression",
            "lambda_literal",
        ]
    }

    // --- call_detection: Swift calls are `call_expression` + `constructor_call`. ---
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "constructor_call"]
    }

    // --- effect_summary ---
    // `try_expression` propagates a thrown error (`try`/`try?`/`try!`);
    // `await_expression` suspends an async call. Allocation/blocking are call-name
    // heuristics (`throw` itself is a `control_transfer_statement`, which also
    // covers return/break, so it is intentionally NOT used as a raise signal).
    fn error_propagation_ops(&self) -> &'static [&'static str] {
        &["try_expression"]
    }

    fn await_node_kinds(&self) -> &'static [&'static str] {
        &["await_expression"]
    }

    fn allocation_call_names(&self) -> &'static [&'static str] {
        &[
            "allocate",
            "malloc",
            "UnsafeMutablePointer",
            "ContiguousArray",
        ]
    }

    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["wait", "sleep", "sync", "lock"]
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

    // --- data_flow: `let`/`var` bindings are `property_declaration`. ---
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["property_declaration"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    // --- unsafe_surface: UnsafePointer family + dlopen FFI + KVC reflection. ---
    fn eval_call_names(&self) -> &'static [&'static str] {
        &[
            "withUnsafePointer",
            "withUnsafeMutableBytes",
            "unsafeBitCast",
            "load",
        ]
    }

    fn ffi_markers(&self) -> &'static [&'static str] {
        &["dlopen", "dlsym", "@_cdecl", "@_silgen_name"]
    }

    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        // Objective-C bridging / KVC dynamic dispatch.
        &["perform", "value", "setValue", "Selector"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("eval")
            .with("ffi")
            .with("reflection_invoke")
    }

    // --- secret_surface ---
    fn env_read_names(&self) -> &'static [&'static str] {
        // ProcessInfo.processInfo.environment / getenv.
        &["environment", "getenv"]
    }

    fn crypto_api_names(&self) -> &'static [&'static str] {
        // CryptoKit signing/HMAC/hashing.
        &["signature", "authenticationCode", "HMAC", "SHA256", "sign"]
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
        &["readLine", "contentsOf", "environment", "data"]
    }

    fn command_exec_names(&self) -> &'static [&'static str] {
        // Foundation Process / NSTask.
        &["launch", "run", "Process", "system"]
    }

    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["write", "createFile", "contentsOfFile", "removeItem"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

impl SwiftSupport {
    /// Classify a property based on context.
    fn classify_property(&self, node: &tree_sitter::Node) -> NodeKind {
        // tree-sitter-swift 0.6 uses "class_body" for class/struct/actor/extension
        // and "enum_class_body" for enums. "struct_body", "actor_body", "enum_body" do not exist.
        if let Some(parent) = node.parent() {
            match parent.kind() {
                "class_body" | "enum_class_body" => return NodeKind::Property,
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
    use crate::LanguageSupport;

    fn parse_and_get_tree(source: &str) -> (tree_sitter::Tree, SwiftSupport) {
        let support = SwiftSupport;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&support.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        (tree, support)
    }

    fn find_node_by_kind<'a>(
        node: tree_sitter::Node<'a>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == kind {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_node_by_kind(child, kind) {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn grammar() {
        let support = SwiftSupport;
        assert_eq!(support.grammar(), Grammar::Swift);
    }

    #[test]
    fn extensions() {
        let support = SwiftSupport;
        let exts = support.extensions();
        assert!(exts.contains(&"swift"));
        assert_eq!(exts.len(), 1);
    }

    #[test]
    fn ts_language_returns_valid_language() {
        let support = SwiftSupport;
        let lang = support.ts_language();
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&lang).is_ok());
    }

    #[test]
    fn node_kind_function_declaration() {
        let (tree, support) = parse_and_get_tree("func foo() {}");
        let func_node = find_node_by_kind(tree.root_node(), "function_declaration").unwrap();
        assert_eq!(support.node_kind(&func_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_class_declaration() {
        let (tree, support) = parse_and_get_tree("class Foo {}");
        let class_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(support.node_kind(&class_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_struct_declaration() {
        let source = "struct Point { var x: Int; var y: Int }";
        let (tree, support) = parse_and_get_tree(source);
        // Struct is represented as class_declaration in tree-sitter-swift
        let struct_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(
            support.node_kind_with_source(&struct_node, source.as_bytes()),
            NodeKind::Class
        );
    }

    #[test]
    fn node_kind_enum_declaration() {
        let source = "enum Status { case active, inactive }";
        let (tree, support) = parse_and_get_tree(source);
        // Enum is represented as class_declaration in tree-sitter-swift 0.6
        let enum_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(
            support.node_kind_with_source(&enum_node, source.as_bytes()),
            NodeKind::Enum
        );
    }

    #[test]
    fn node_kind_protocol_declaration() {
        let (tree, support) = parse_and_get_tree("protocol Repository {}");
        let protocol_node = find_node_by_kind(tree.root_node(), "protocol_declaration").unwrap();
        assert_eq!(support.node_kind(&protocol_node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_actor_declaration() {
        let source = "actor DataStore {}";
        let (tree, support) = parse_and_get_tree(source);
        // Actor is represented as class_declaration in tree-sitter-swift
        let actor_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(
            support.node_kind_with_source(&actor_node, source.as_bytes()),
            NodeKind::Class
        );
    }

    #[test]
    fn node_kind_extension_declaration() {
        let source = "extension String {}";
        let (tree, support) = parse_and_get_tree(source);
        // Extension is represented as class_declaration in tree-sitter-swift
        let ext_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(
            support.node_kind_with_source(&ext_node, source.as_bytes()),
            NodeKind::Class
        );
    }

    #[test]
    fn node_kind_typealias_declaration() {
        let (tree, support) = parse_and_get_tree("typealias UserID = Int");
        let type_node = find_node_by_kind(tree.root_node(), "typealias_declaration").unwrap();
        assert_eq!(support.node_kind(&type_node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_init_declaration() {
        let (tree, support) = parse_and_get_tree("class Foo { init() {} }");
        let init_node = find_node_by_kind(tree.root_node(), "init_declaration").unwrap();
        assert_eq!(support.node_kind(&init_node), NodeKind::Constructor);
    }

    #[test]
    fn node_kind_import_declaration() {
        let (tree, support) = parse_and_get_tree("import Foundation");
        let import_node = find_node_by_kind(tree.root_node(), "import_declaration").unwrap();
        assert_eq!(support.node_kind(&import_node), NodeKind::Import);
    }

    #[test]
    fn node_kind_call_expression() {
        let (tree, support) = parse_and_get_tree("func main() { foo() }");
        let call_node = find_node_by_kind(tree.root_node(), "call_expression").unwrap();
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_simple_identifier() {
        let (tree, support) = parse_and_get_tree("let x = y");
        let ident_node = find_node_by_kind(tree.root_node(), "simple_identifier").unwrap();
        assert_eq!(support.node_kind(&ident_node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_comment() {
        let (tree, support) = parse_and_get_tree("// comment\nlet x = 1");
        let comment_node = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_attribute() {
        let (tree, support) = parse_and_get_tree("@available(iOS 15, *) func foo() {}");
        let attr_node = find_node_by_kind(tree.root_node(), "attribute").unwrap();
        assert_eq!(support.node_kind(&attr_node), NodeKind::Decorator);
    }

    #[test]
    fn node_kind_lambda_literal() {
        let (tree, support) = parse_and_get_tree("let f = { x in x + 1 }");
        let lambda_node = find_node_by_kind(tree.root_node(), "lambda_literal");
        if let Some(node) = lambda_node {
            assert_eq!(support.node_kind(&node), NodeKind::Function);
        }
        // Note: May be closure_expression depending on grammar version
    }

    #[test]
    fn comment_delimiters() {
        let support = SwiftSupport;
        let delimiters = support.comment_delimiters();
        assert_eq!(delimiters.line, Some("//"));
        assert_eq!(delimiters.block_start, Some("/*"));
        assert_eq!(delimiters.block_end, Some("*/"));
    }

    #[test]
    fn queries_embedded() {
        let support = SwiftSupport;
        assert!(!support.skeleton_query().is_empty());
        assert!(!support.symbols_query().is_empty());
        assert!(!support.references_query().is_empty());
    }

    #[test]
    fn skeleton_query_is_valid() {
        let support = SwiftSupport;
        let query = tree_sitter::Query::new(&support.ts_language(), support.skeleton_query());
        assert!(
            query.is_ok(),
            "skeleton.scm should parse: {:?}",
            query.err()
        );
    }

    #[test]
    fn symbols_query_is_valid() {
        let support = SwiftSupport;
        let query = tree_sitter::Query::new(&support.ts_language(), support.symbols_query());
        assert!(query.is_ok(), "symbols.scm should parse: {:?}", query.err());
    }

    #[test]
    fn references_query_is_valid() {
        let support = SwiftSupport;
        let query = tree_sitter::Query::new(&support.ts_language(), support.references_query());
        assert!(
            query.is_ok(),
            "references.scm should parse: {:?}",
            query.err()
        );
    }

    #[test]
    fn node_kind_with_source_struct_returns_class() {
        let source = "struct Point { var x: Int }";
        let (tree, support) = parse_and_get_tree(source);
        let class_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(
            support.node_kind_with_source(&class_node, source.as_bytes()),
            NodeKind::Class
        );
    }

    #[test]
    fn node_kind_with_source_enum_returns_enum() {
        let source = "enum Color { case red, green, blue }";
        let (tree, support) = parse_and_get_tree(source);
        let class_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(
            support.node_kind_with_source(&class_node, source.as_bytes()),
            NodeKind::Enum
        );
    }

    #[test]
    fn node_kind_with_source_actor_returns_class() {
        let source = "actor DataStore {}";
        let (tree, support) = parse_and_get_tree(source);
        let class_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(
            support.node_kind_with_source(&class_node, source.as_bytes()),
            NodeKind::Class
        );
    }

    #[test]
    fn node_kind_with_source_extension_returns_class() {
        let source = "extension String {}";
        let (tree, support) = parse_and_get_tree(source);
        let class_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(
            support.node_kind_with_source(&class_node, source.as_bytes()),
            NodeKind::Class
        );
    }

    #[test]
    fn node_kind_with_source_class_returns_class() {
        let source = "class Foo {}";
        let (tree, support) = parse_and_get_tree(source);
        let class_node = find_node_by_kind(tree.root_node(), "class_declaration").unwrap();
        assert_eq!(
            support.node_kind_with_source(&class_node, source.as_bytes()),
            NodeKind::Class
        );
    }

    #[test]
    fn classify_property_in_struct_body_returns_property() {
        let (tree, support) = parse_and_get_tree("struct S { var x: Int = 0 }");
        let prop_node = find_node_by_kind(tree.root_node(), "property_declaration").unwrap();
        assert_eq!(support.node_kind(&prop_node), NodeKind::Property);
    }

    #[test]
    fn classify_property_at_top_level_returns_variable() {
        let (tree, support) = parse_and_get_tree("var x: Int = 0");
        let prop_node = find_node_by_kind(tree.root_node(), "property_declaration").unwrap();
        assert_eq!(support.node_kind(&prop_node), NodeKind::Variable);
    }

    #[test]
    fn classify_property_in_actor_body_returns_property() {
        let (tree, support) = parse_and_get_tree("actor A { var state: Int = 0 }");
        let prop_node = find_node_by_kind(tree.root_node(), "property_declaration").unwrap();
        assert_eq!(support.node_kind(&prop_node), NodeKind::Property);
    }

    #[test]
    fn classify_property_in_enum_body_returns_property() {
        let source = "enum E { case a\nvar tag: Int = 0 }";
        let (tree, support) = parse_and_get_tree(source);
        // Stored properties inside enums use enum_class_body as parent
        let prop_node = find_node_by_kind(tree.root_node(), "property_declaration").unwrap();
        assert_eq!(support.node_kind(&prop_node), NodeKind::Property);
    }

    #[test]
    fn function_keyword_is_func() {
        let support = SwiftSupport;
        assert_eq!(support.function_keyword(), "func");
    }

    #[test]
    fn function_parent_kinds_includes_function_declaration() {
        let support = SwiftSupport;
        let kinds = support.function_parent_kinds();
        assert!(
            kinds.contains(&"function_declaration"),
            "function_declaration missing"
        );
        assert!(
            kinds.contains(&"init_declaration"),
            "init_declaration missing"
        );
        assert!(kinds.contains(&"lambda_literal"), "lambda_literal missing");
        assert!(
            kinds.contains(&"protocol_function_declaration"),
            "protocol_function_declaration missing"
        );
    }

    #[test]
    fn function_body_kind_is_function_body() {
        let support = SwiftSupport;
        assert_eq!(support.function_body_kind(), "function_body");
    }

    #[test]
    fn decision_point_kinds_includes_swift_control_flow() {
        let support = SwiftSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_statement"), "if_statement missing");
        assert!(
            kinds.contains(&"guard_statement"),
            "guard_statement missing"
        );
        assert!(
            kinds.contains(&"switch_statement"),
            "switch_statement missing"
        );
        assert!(kinds.contains(&"for_statement"), "for_statement missing");
        assert!(
            kinds.contains(&"while_statement"),
            "while_statement missing"
        );
        assert!(
            kinds.contains(&"repeat_while_statement"),
            "repeat_while_statement missing"
        );
        assert!(kinds.contains(&"catch_block"), "catch_block missing");
        assert!(kinds.contains(&"switch_entry"), "switch_entry missing");
    }

    #[test]
    fn logical_operator_info_returns_none() {
        let support = SwiftSupport;
        // Swift uses conjunction_expression/disjunction_expression node kinds
        assert!(support.logical_operator_info().is_none());
    }

    #[test]
    fn extractable_node_kinds_includes_guard_statement() {
        let support = SwiftSupport;
        let kinds = support.extractable_node_kinds();
        assert!(
            !kinds.is_empty(),
            "extractable_node_kinds must be non-empty"
        );
        assert!(
            kinds.contains(&"guard_statement"),
            "guard_statement missing"
        );
        assert!(
            kinds.contains(&"repeat_while_statement"),
            "repeat_while_statement missing"
        );
        assert!(kinds.contains(&"catch_block"), "catch_block missing");
        assert!(kinds.contains(&"code_block"), "code_block missing");
        assert!(kinds.contains(&"function_body"), "function_body missing");
        assert!(kinds.contains(&"if_statement"), "if_statement missing");
        assert!(kinds.contains(&"for_statement"), "for_statement missing");
        assert!(
            kinds.contains(&"while_statement"),
            "while_statement missing"
        );
        assert!(
            kinds.contains(&"switch_statement"),
            "switch_statement missing"
        );
        assert!(
            kinds.contains(&"call_expression"),
            "call_expression missing"
        );
        assert!(kinds.contains(&"lambda_literal"), "lambda_literal missing");
    }

    // --- moat coverage hooks (B2) ---

    #[test]
    fn coverage_call_detection_overrides_default() {
        let s = SwiftSupport;
        assert!(s.call_node_kinds().contains(&"call_expression"));
        assert!(s.call_node_kinds().contains(&"constructor_call"));
        let (tree, _) = parse_and_get_tree("func f() { g() }");
        assert!(find_node_by_kind(tree.root_node(), "call_expression").is_some());
    }

    #[test]
    fn coverage_effect_summary_widened() {
        let s = SwiftSupport;
        let baseline = act_lite_core::KindMask::new().with("read").with("write");
        assert_ne!(s.effect_modeled_kinds(), baseline);
        assert!(s.error_propagation_ops().contains(&"try_expression"));
        assert!(s.await_node_kinds().contains(&"await_expression"));
        let (tree, _) = parse_and_get_tree("func f() throws { try g() }");
        assert!(find_node_by_kind(tree.root_node(), "try_expression").is_some());
    }

    #[test]
    fn coverage_data_flow_models_bindings() {
        let s = SwiftSupport;
        assert!(!s.data_flow_modeled_kinds().is_empty());
        assert!(s.binding_node_kinds().contains(&"property_declaration"));
        let (tree, _) = parse_and_get_tree("func f() { let x = 5 }");
        assert!(find_node_by_kind(tree.root_node(), "property_declaration").is_some());
    }

    #[test]
    fn coverage_unsafe_surface_modeled() {
        let s = SwiftSupport;
        assert!(!s.unsafe_modeled_kinds().is_empty());
        assert!(s.eval_call_names().contains(&"unsafeBitCast"));
        assert!(s.ffi_markers().contains(&"dlopen"));
    }

    #[test]
    fn coverage_secret_surface_modeled() {
        let s = SwiftSupport;
        assert!(!s.secret_modeled_kinds().is_empty());
        assert!(s.env_read_names().contains(&"environment"));
        assert!(s.crypto_api_names().contains(&"HMAC"));
    }

    #[test]
    fn coverage_taint_flow_modeled() {
        let s = SwiftSupport;
        assert!(!s.taint_modeled_kinds().is_empty());
        assert!(s.command_exec_names().contains(&"Process"));
        assert!(s.fs_sink_names().contains(&"write"));
    }
}
