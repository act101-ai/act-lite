//! Rust language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Rust in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("rust"),
        display_name: "Rust",
        extensions: &["rs"],
        category: LanguageCategory::Systems,
    }
}

/// Rust language support implementation.
#[derive(Default)]
pub struct RustSupport;

impl LanguageSupport for RustSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Rust
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Functions
            "function_item" | "function_signature_item" => NodeKind::Function,

            // Types
            "struct_item" | "union_item" => NodeKind::Struct,
            "trait_item" => NodeKind::Interface,
            "type_item" => NodeKind::TypeAlias,
            "enum_item" => NodeKind::Enum,

            // Members
            "field_declaration" => NodeKind::Property,
            "enum_variant" => NodeKind::Property,

            // Variables/Constants
            "let_declaration" => NodeKind::Variable,
            "const_item" | "static_item" => NodeKind::Constant,

            // Parameters
            "parameter" | "self_parameter" => NodeKind::Parameter,

            // Imports
            "use_declaration" => NodeKind::Import,
            "mod_item" => NodeKind::Module,

            // Attributes
            "attribute_item" => NodeKind::Decorator,

            // Blocks
            "block" | "declaration_list" => NodeKind::Block,
            "impl_item" => NodeKind::Block, // Impl blocks group methods

            // Calls
            "call_expression" | "macro_invocation" => NodeKind::Call,

            // Identifiers
            "identifier" | "type_identifier" | "field_identifier" => NodeKind::Identifier,

            // Comments
            "line_comment" | "block_comment" => NodeKind::Comment,

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/rust/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/rust/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/rust/references.scm")
    }

    fn locals_query(&self) -> &'static str {
        include_str!("../../queries/rust/locals.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }

    fn function_keyword(&self) -> &'static str {
        "fn"
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &["function_item"]
    }

    fn function_body_kind(&self) -> &'static str {
        "block"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "match_expression",
            "while_expression",
            "for_expression",
            "loop_expression",
        ]
    }

    fn cfg_branch_kinds(&self) -> &'static [&'static str] {
        &["if_expression", "match_expression"]
    }
    fn cfg_loop_kinds(&self) -> &'static [&'static str] {
        &["while_expression", "for_expression", "loop_expression"]
    }
    fn cfg_arm_container_kinds(&self) -> &'static [&'static str] {
        &["match_block"]
    }
    fn cfg_arm_kinds(&self) -> &'static [&'static str] {
        &["match_arm"]
    }
    fn cfg_exception_handler_kinds(&self) -> &'static [&'static str] {
        // Rust has no try/catch construct.
        &[]
    }
    fn cfg_throw_kinds(&self) -> &'static [&'static str] {
        // `?`-propagation; `panic!` is a macro call (heuristic, not modeled here).
        &["try_expression"]
    }
    fn cfg_return_kinds(&self) -> &'static [&'static str] {
        &["return_expression"]
    }
    fn cfg_modeled_kinds(&self) -> act_lite_core::KindMask {
        // Exception is modeled as propagation-only (no handler resolution), but
        // the dimension IS present.
        act_lite_core::KindMask::new()
            .with("seq")
            .with("branch")
            .with("loop")
            .with("exception")
            .with("return")
    }

    fn stub_placeholders(&self) -> &'static [&'static str] {
        &["todo!()", "unimplemented!()", "panic!(\"not implemented\")"]
    }

    // --- call_detection: Rust calls are `call_expression` + `macro_invocation`. ---
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "macro_invocation"]
    }

    fn field_access_node_kinds(&self) -> &'static [&'static str] {
        &["field_expression"]
    }

    // --- effect_summary ---
    // `try_expression` is the `?` propagation; `await_expression` suspends;
    // allocation is via Box/Vec/String constructors (heuristic call names).
    fn error_propagation_ops(&self) -> &'static [&'static str] {
        &["try_expression"]
    }

    fn await_node_kinds(&self) -> &'static [&'static str] {
        &["await_expression"]
    }

    fn allocation_call_names(&self) -> &'static [&'static str] {
        // Owning-heap constructors. `panic!` is a macro, handled via call frontier.
        &[
            "new",
            "with_capacity",
            "from",
            "to_vec",
            "to_string",
            "boxed",
        ]
    }

    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["sleep", "recv", "lock", "join", "block_on"]
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

    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["let_declaration"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    fn unsafe_node_kinds(&self) -> &'static [&'static str] {
        &["unsafe_block"]
    }

    fn ffi_markers(&self) -> &'static [&'static str] {
        &["foreign_mod_item"]
    }

    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &["query", "execute", "query_unchecked"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("unsafe_block")
            .with("ffi")
            .with("raw_sql")
    }

    fn env_read_names(&self) -> &'static [&'static str] {
        &["var", "var_os"]
    }

    fn crypto_api_names(&self) -> &'static [&'static str] {
        &["sign", "sign_prehashed"]
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
        // env reads, argv, stdin, file reads — untrusted-input entry points.
        &[
            "var",
            "var_os",
            "args",
            "read_line",
            "read_to_string",
            "read",
        ]
    }

    fn command_exec_names(&self) -> &'static [&'static str] {
        // std::process::Command builder + execution.
        &["Command", "new", "arg", "args", "spawn", "output", "status"]
    }

    fn fs_sink_names(&self) -> &'static [&'static str] {
        // std::fs / File path sinks. `read_to_string`/`read` appear here as
        // well as in sources: a path-valued tainted argument flowing into them
        // is a path-traversal sink (distinct from their use as a content source).
        &[
            "File",
            "open",
            "create",
            "write",
            "write_all",
            "remove_file",
            "read_to_string",
            "read",
            "read_dir",
            "copy",
            "rename",
        ]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "block",
            "expression_statement",
            "let_declaration",
            "return_expression",
            "if_expression",
            "while_expression",
            "for_expression",
            "loop_expression",
            "match_expression",
            "match_arm",
            "match_block",
            "function_item",
            "impl_item",
            "trait_item",
            "struct_item",
            "enum_item",
            "mod_item",
            "type_item",
            "const_item",
            "static_item",
            "use_declaration",
            "break_expression",
            "continue_expression",
            "call_expression",
            "binary_expression",
            "unary_expression",
            "field_expression",
            "index_expression",
            "range_expression",
            "cast_expression",
            "closure_expression",
            "struct_expression",
            "tuple_expression",
            "array_expression",
            "parenthesized_expression",
            "macro_invocation",
            "reference_expression",
            "await_expression",
            "unsafe_block",
            "async_block",
            "try_expression",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};

    crate::language_support_tests!(RustSupport, Grammar::Rust, &["rs"]);

    #[test]
    fn node_kind_function_item() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "fn foo() {}");
        let func_node = find_node_by_kind(tree.root_node(), "function_item").unwrap();
        assert_eq!(support.node_kind(&func_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_struct_item() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "struct Foo { x: i32 }");
        let struct_node = find_node_by_kind(tree.root_node(), "struct_item").unwrap();
        assert_eq!(support.node_kind(&struct_node), NodeKind::Struct);
    }

    #[test]
    fn node_kind_enum_item() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "enum Status { Active, Inactive }");
        let enum_node = find_node_by_kind(tree.root_node(), "enum_item").unwrap();
        assert_eq!(support.node_kind(&enum_node), NodeKind::Enum);
    }

    #[test]
    fn node_kind_trait_item() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "trait Processor { fn process(&self); }");
        let trait_node = find_node_by_kind(tree.root_node(), "trait_item").unwrap();
        assert_eq!(support.node_kind(&trait_node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_impl_item() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "impl Foo { fn new() -> Self { Self } }");
        let impl_node = find_node_by_kind(tree.root_node(), "impl_item").unwrap();
        assert_eq!(support.node_kind(&impl_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_type_alias() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "type Result<T> = std::result::Result<T, Error>;");
        let type_node = find_node_by_kind(tree.root_node(), "type_item").unwrap();
        assert_eq!(support.node_kind(&type_node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_const_item() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "const MAX: usize = 100;");
        let const_node = find_node_by_kind(tree.root_node(), "const_item").unwrap();
        assert_eq!(support.node_kind(&const_node), NodeKind::Constant);
    }

    #[test]
    fn node_kind_static_item() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "static COUNT: AtomicUsize = AtomicUsize::new(0);");
        let static_node = find_node_by_kind(tree.root_node(), "static_item").unwrap();
        assert_eq!(support.node_kind(&static_node), NodeKind::Constant);
    }

    #[test]
    fn node_kind_let_declaration() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "fn main() { let x = 1; }");
        let let_node = find_node_by_kind(tree.root_node(), "let_declaration").unwrap();
        assert_eq!(support.node_kind(&let_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_use_declaration() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "use std::collections::HashMap;");
        let use_node = find_node_by_kind(tree.root_node(), "use_declaration").unwrap();
        assert_eq!(support.node_kind(&use_node), NodeKind::Import);
    }

    #[test]
    fn node_kind_mod_item() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "mod utils { pub fn helper() {} }");
        let mod_node = find_node_by_kind(tree.root_node(), "mod_item").unwrap();
        assert_eq!(support.node_kind(&mod_node), NodeKind::Module);
    }

    #[test]
    fn node_kind_field_declaration() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "struct Foo { name: String }");
        let field_node = find_node_by_kind(tree.root_node(), "field_declaration").unwrap();
        assert_eq!(support.node_kind(&field_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_call_expression() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "fn main() { foo(); }");
        let call_node = find_node_by_kind(tree.root_node(), "call_expression").unwrap();
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_macro_invocation() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "fn main() { println!(\"hello\"); }");
        let macro_node = find_node_by_kind(tree.root_node(), "macro_invocation").unwrap();
        assert_eq!(support.node_kind(&macro_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_identifier() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "fn main() { x }");
        let ident_node = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        assert_eq!(support.node_kind(&ident_node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_type_identifier() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "struct Foo {}");
        let type_ident_node = find_node_by_kind(tree.root_node(), "type_identifier").unwrap();
        assert_eq!(support.node_kind(&type_ident_node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_line_comment() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "// This is a comment\nfn main() {}");
        let comment_node = find_node_by_kind(tree.root_node(), "line_comment").unwrap();
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_block_comment() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "/* Block comment */\nfn main() {}");
        let comment_node = find_node_by_kind(tree.root_node(), "block_comment").unwrap();
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_attribute_item() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "#[derive(Debug)]\nstruct Foo {}");
        let attr_node = find_node_by_kind(tree.root_node(), "attribute_item").unwrap();
        assert_eq!(support.node_kind(&attr_node), NodeKind::Decorator);
    }

    #[test]
    fn node_kind_unknown() {
        let support = RustSupport;
        let tree = parse_with_support(&support, "fn main() { 1 + 2 }");
        let binary_node = find_node_by_kind(tree.root_node(), "binary_expression").unwrap();
        assert_eq!(support.node_kind(&binary_node), NodeKind::Unknown);
    }

    #[test]
    fn function_keyword_is_fn() {
        let support = RustSupport;
        assert_eq!(support.function_keyword(), "fn");
    }

    #[test]
    fn function_parent_kinds_are_rust_specific() {
        let support = RustSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"function_item"));
        // Must NOT contain JS/TS values
        assert!(!kinds.contains(&"function_declaration"));
        assert!(!kinds.contains(&"arrow_function"));
    }

    #[test]
    fn function_body_kind_is_block() {
        let support = RustSupport;
        assert_eq!(support.function_body_kind(), "block");
    }

    #[test]
    fn decision_point_kinds_are_rust_specific() {
        let support = RustSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_expression"));
        assert!(kinds.contains(&"match_expression"));
        assert!(kinds.contains(&"loop_expression"));
        // Must NOT contain JS/TS values
        assert!(!kinds.contains(&"ternary_expression"));
        assert!(!kinds.contains(&"switch_case"));
    }

    #[test]
    fn stub_placeholders_returns_language_patterns() {
        let support = RustSupport;
        let placeholders = support.stub_placeholders();
        assert!(!placeholders.is_empty());
        assert!(placeholders.contains(&"todo!()"));
        assert!(placeholders.contains(&"unimplemented!()"));
    }

    #[test]
    fn cfg_hooks_rust_arm_container_is_match_block() {
        let r = RustSupport;
        assert!(r.cfg_branch_kinds().contains(&"match_expression"));
        assert!(r.cfg_loop_kinds().contains(&"loop_expression"));
        // The spike gotcha: Rust arms live under `match_block`, not a *body* node.
        assert_eq!(r.cfg_arm_container_kinds(), &["match_block"]);
        assert!(r.cfg_arm_kinds().contains(&"match_arm"));
        assert!(r.cfg_throw_kinds().contains(&"try_expression"));
        // Rust models exceptions as propagation-only; the dimension is still modeled.
        assert!(r.cfg_modeled_kinds().contains("exception"));
    }

    // --- moat coverage hooks (B2) ---

    #[test]
    fn coverage_call_detection_overrides_default() {
        let s = RustSupport;
        assert_eq!(
            s.call_node_kinds(),
            &["call_expression", "macro_invocation"]
        );
        let tree = parse_with_support(&s, "fn f() { g(); }");
        assert!(find_node_by_kind(tree.root_node(), "call_expression").is_some());
    }

    #[test]
    fn coverage_effect_summary_widened() {
        let s = RustSupport;
        let baseline = act_lite_core::KindMask::new().with("read").with("write");
        assert_ne!(s.effect_modeled_kinds(), baseline);
        assert!(s.error_propagation_ops().contains(&"try_expression"));
        assert!(s.await_node_kinds().contains(&"await_expression"));
        let tree = parse_with_support(&s, "fn f() -> Result<(), E> { g()?; Ok(()) }");
        assert!(find_node_by_kind(tree.root_node(), "try_expression").is_some());
    }

    #[test]
    fn coverage_taint_flow_modeled() {
        let s = RustSupport;
        assert!(!s.taint_modeled_kinds().is_empty());
        assert!(s.command_exec_names().contains(&"Command"));
        assert!(s.fs_sink_names().contains(&"File"));
        assert!(s.taint_source_names().contains(&"var"));
    }

    #[test]
    fn field_access_node_kinds_is_field_expression() {
        assert_eq!(RustSupport.field_access_node_kinds(), &["field_expression"]);
    }
}
