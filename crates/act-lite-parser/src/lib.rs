//! act-lite-parser: Tree-sitter integration with language-agnostic parsing interface.
//!
//! This crate provides a unified interface for parsing source code across
//! multiple languages using tree-sitter. It defines language-agnostic types
//! and traits that allow adding new languages without modifying core logic.

pub mod compact_declaration;
#[cfg(test)]
mod coverage;
pub mod deps;
pub mod error;
pub mod function_model;
pub mod grammar;
pub mod import_types;
pub mod language;
pub mod languages;
pub mod node_kind;
pub mod parser;
pub mod query;
pub mod regions;
pub mod source_file;
pub mod symbol_id;
pub mod syntax_tree;
pub mod workspace;

#[cfg(test)]
pub mod test_utils;

pub use compact_declaration::CompactDeclaration;
pub use error::{ParserError, Result};
pub use grammar::Grammar;
pub use import_types::{Import, ImportHelper, ImportKind};
pub use language::{CommentDelimiters, LanguageSupport};
pub use languages::{
    CSharpSupport, CSupport, CppSupport, ElixirSupport, GoSupport, JavaScriptSupport, JavaSupport,
    KotlinSupport, PhpSupport, PythonSupport, RubySupport, RustSupport, ScalaSupport, SqlSupport,
    TypeScriptSupport,
};
pub use node_kind::NodeKind;
pub use parser::Parser;
pub use query::{
    Declaration, DependencyInfo, DiagnosticInfo, MAX_REFERENCE_WALK_FILE_BYTES,
    MAX_REFERENCE_WALK_FILES, QueryCapture, QueryResult, QueryType, RawMatch, ReferenceSource,
    ReferencesWalk, SkeletonResult, SkippedFile, SuggestionInfo, SymbolResult, execute_query,
    execute_raw_query, execute_reference_ranges_for_symbol_across, execute_references,
    execute_references_for_symbol, execute_references_for_symbol_across, execute_skeleton,
    execute_symbol, execute_symbols, walk_reference_siblings,
};
pub use source_file::{ParseOptions, SourceFile, StopFn, open_regular_within, read_regular_text};
pub use symbol_id::SymbolId;
pub use syntax_tree::SyntaxTree;
pub use workspace::Workspace;

// Ensure all tree-sitter language libraries are linked
// These are needed for language support to work correctly in all contexts
use tree_sitter_bash as _;
use tree_sitter_elixir as _;
use tree_sitter_javascript as _;
use tree_sitter_typescript as _;
