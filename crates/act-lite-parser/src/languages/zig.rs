//! Zig language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Zig in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("zig"),
        display_name: "Zig",
        extensions: &["zig"],
        category: LanguageCategory::Systems,
    }
}

/// Zig language support implementation.
#[derive(Default)]
pub struct ZigSupport;

impl LanguageSupport for ZigSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Zig
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_zig::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "function_declaration" | "test_declaration" => NodeKind::Function,
            "struct_declaration" => NodeKind::Struct,
            "enum_declaration" | "error_set_declaration" => NodeKind::Enum,
            "variable_declaration" => NodeKind::Variable,
            "container_field" => NodeKind::Property,
            "parameter" => NodeKind::Parameter,
            "block" | "source_file" => NodeKind::Block,
            "call_expression" | "builtin_function" => NodeKind::Call,
            "identifier" => NodeKind::Identifier,
            "comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["zig"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/zig/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/zig/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/zig/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("//"),
            block_start: None,
            block_end: None,
        }
    }

    // --- call_detection: plain calls are `call_expression`; `@name(…)` builtins are `builtin_function`. ---
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "builtin_function"]
    }

    // --- effect_summary: allocation goes through an `Allocator`; blocking is by call name. ---
    fn allocation_call_names(&self) -> &'static [&'static str] {
        &["alloc", "create", "dupe", "realloc", "allocPrint"]
    }

    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["sleep", "wait", "join", "accept", "read"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("allocate")
            .with("block")
    }

    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["variable_declaration"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    // --- unsafe_surface: C interop through `@cImport` and `extern`. ---
    fn ffi_markers(&self) -> &'static [&'static str] {
        &["@cImport", "@cInclude", "extern"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("ffi")
    }

    fn env_read_names(&self) -> &'static [&'static str] {
        &["getenv", "getEnvVarOwned", "getEnvMap"]
    }

    fn crypto_api_names(&self) -> &'static [&'static str] {
        &["sign"]
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
            "getenv",
            "getEnvVarOwned",
            "argsAlloc",
            "readFileAlloc",
            "readAllAlloc",
        ]
    }

    fn command_exec_names(&self) -> &'static [&'static str] {
        &["run", "spawn", "spawnAndWait", "execve"]
    }

    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["openFile", "createFile", "readFileAlloc", "deleteFile"]
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
        assert_eq!(ZigSupport.grammar(), Grammar::Zig);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&ZigSupport.ts_language()).is_ok());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_skeleton};
        let source = SourceFile::from_content(
            "point.zig",
            "pub fn main() void {}\n".to_string(),
            Grammar::Zig,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => assert!(
                s.declarations
                    .iter()
                    .any(|d| d.name == "main" && d.kind == "function"),
                "expected function 'main', got: {:?}",
                s.declarations
            ),
            _ => panic!("wrong result type"),
        }
    }
}
