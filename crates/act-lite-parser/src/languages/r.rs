//! R language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register R in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("r"),
        display_name: "R",
        extensions: &["R", "r"],
        category: LanguageCategory::Dynamic,
    }
}

/// R language support implementation.
#[derive(Default)]
pub struct RSupport;

impl LanguageSupport for RSupport {
    fn grammar(&self) -> Grammar {
        Grammar::R
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_r::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "function_definition" => NodeKind::Function,
            "parameter" => NodeKind::Parameter,
            "braced_expression" | "program" => NodeKind::Block,
            "call" => NodeKind::Call,
            "identifier" => NodeKind::Identifier,
            "comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["R", "r"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/r/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/r/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/r/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
            block_start: None,
            block_end: None,
        }
    }

    // --- call_detection: every call, `f(x)` and `pkg::f(x)`, is a `call`. ---
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call"]
    }

    // --- effect_summary: blocking is by call name. ---
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["Sys.sleep", "readline", "readLines", "system", "system2"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("block")
    }

    // --- data_flow: assignments (`<-`, `=`, `<<-`, `->`) are binary operators. ---
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["binary_operator"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    // --- unsafe_surface: native code through `.Call` and friends. ---
    fn ffi_markers(&self) -> &'static [&'static str] {
        &[".Call", ".C", ".External", ".Fortran", "dyn.load"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("ffi")
    }

    fn eval_call_names(&self) -> &'static [&'static str] {
        &["eval", "evalq", "parse"]
    }

    fn env_read_names(&self) -> &'static [&'static str] {
        &["Sys.getenv"]
    }

    fn secret_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("credential_param")
            .with("token_var")
            .with("env_secret_read")
            .with("hardcoded_literal")
    }

    fn taint_source_names(&self) -> &'static [&'static str] {
        &[
            "readline",
            "readLines",
            "readRDS",
            "commandArgs",
            "Sys.getenv",
        ]
    }

    fn command_exec_names(&self) -> &'static [&'static str] {
        &["system", "system2", "shell"]
    }

    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["file", "writeLines", "saveRDS", "unlink", "file.remove"]
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
        assert_eq!(RSupport.grammar(), Grammar::R);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&RSupport.ts_language()).is_ok());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_skeleton};
        let source = SourceFile::from_content(
            "scale.R",
            "scale_by <- function(x, k = 2) x * k\n".to_string(),
            Grammar::R,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => assert!(
                s.declarations
                    .iter()
                    .any(|d| d.name == "scale_by" && d.kind == "function"),
                "expected function 'scale_by', got: {:?}",
                s.declarations
            ),
            _ => panic!("wrong result type"),
        }
    }
}
