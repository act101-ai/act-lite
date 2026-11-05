//! Julia language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Julia in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("julia"),
        display_name: "Julia",
        extensions: &["jl"],
        category: LanguageCategory::Dynamic,
    }
}

/// Julia language support implementation.
#[derive(Default)]
pub struct JuliaSupport;

impl LanguageSupport for JuliaSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Julia
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_julia::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "function_definition" | "macro_definition" | "arrow_function_expression" => {
                NodeKind::Function
            }
            "module_definition" => NodeKind::Module,
            "struct_definition" => NodeKind::Struct,
            "abstract_definition" => NodeKind::Interface,
            "const_statement" => NodeKind::Constant,
            "import_statement" | "using_statement" => NodeKind::Import,
            "compound_statement" | "source_file" => NodeKind::Block,
            "call_expression" | "broadcast_call_expression" => NodeKind::Call,
            "identifier" => NodeKind::Identifier,
            "line_comment" | "block_comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["jl"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/julia/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/julia/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/julia/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
            block_start: Some("#="),
            block_end: Some("=#"),
        }
    }

    // --- call_detection: `f(x)` is a `call_expression`, `f.(x)` a `broadcast_call_expression`. ---
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "broadcast_call_expression"]
    }

    // --- effect_summary: blocking is by call name. ---
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["sleep", "wait", "fetch", "readline", "run"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("block")
    }

    // --- data_flow: `x = …` is an `assignment`. ---
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["assignment"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    // --- unsafe_surface: native code through `ccall` and `cglobal`. ---
    fn ffi_markers(&self) -> &'static [&'static str] {
        &["ccall", "@ccall", "cglobal"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("ffi")
    }

    fn eval_call_names(&self) -> &'static [&'static str] {
        &["eval", "include_string"]
    }

    fn secret_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("credential_param")
            .with("token_var")
            .with("hardcoded_literal")
    }

    fn taint_source_names(&self) -> &'static [&'static str] {
        &["readline", "readlines", "readchomp", "read"]
    }

    fn command_exec_names(&self) -> &'static [&'static str] {
        &["run", "pipeline", "success"]
    }

    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["open", "write", "rm", "mkpath", "cp", "mv"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar() {
        assert_eq!(JuliaSupport.grammar(), Grammar::Julia);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&JuliaSupport.ts_language()).is_ok());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_skeleton};
        let source = SourceFile::from_content(
            "geometry.jl",
            "function area(r)\n    r^2\nend\n".to_string(),
            Grammar::Julia,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => assert!(
                s.declarations
                    .iter()
                    .any(|d| d.name == "area" && d.kind == "function"),
                "expected function 'area', got: {:?}",
                s.declarations
            ),
            _ => panic!("wrong result type"),
        }
    }
}
