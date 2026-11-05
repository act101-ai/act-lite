//! This crate provides SCSS language support for the [tree-sitter][] parsing library.
//!
//! Typically, you will use the [`LANGUAGE`] constant to add this language to a
//! tree-sitter [Parser][], and then use the parser to parse some code:
//!
//! ```ignore
//! let code = r#"
//! $blue: #056ef0;
//! "#;
//! let mut parser = tree_sitter::Parser::new();
//! parser.set_language(&tree_sitter_scss::LANGUAGE.into()).expect("Error loading SCSS grammar");
//! let tree = parser.parse(code, None).unwrap();
//! assert!(!tree.root_node().has_error());
//! ```
//!
//! The example is `ignore`d: this vendored crate deliberately depends on
//! `tree-sitter-language` only, so `tree_sitter` is not available to its
//! doctests.
//!
//! [Language]: https://docs.rs/tree-sitter/*/tree_sitter/struct.Language.html
//! [Parser]: https://docs.rs/tree-sitter/*/tree_sitter/struct.Parser.html
//! [tree-sitter]: https://tree-sitter.github.io/

// Vendored and patched to use tree-sitter-language instead of tree-sitter
// to avoid duplicate C runtime symbols at link time.

use tree_sitter_language::LanguageFn;

extern "C" {
    fn tree_sitter_scss() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for the SCSS grammar.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_scss) };

/// The content of the `node-types.json` file for this grammar.
pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");
