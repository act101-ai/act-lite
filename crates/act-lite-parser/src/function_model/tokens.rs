//! Comments and identifier tokens.

use crate::Grammar;

/// Elixir call targets that read like keywords. They are `identifier` nodes in
/// the grammar but are not identifier tokens.
const ELIXIR_KEYWORD_TARGETS: [&str; 19] = [
    "def",
    "defp",
    "defmacro",
    "defmacrop",
    "defmodule",
    "if",
    "unless",
    "case",
    "cond",
    "with",
    "for",
    "try",
    "receive",
    "fn",
    "quote",
    "import",
    "alias",
    "require",
    "use",
];

/// Whether a node of this kind is a comment: its kind ends in `comment`, or it
/// is a Python `line_continuation`. A comment's whole subtree is a comment.
pub fn is_comment(kind: &str) -> bool {
    kind.ends_with("comment") || kind == "line_continuation"
}

/// Whether `node` is an identifier token: a leaf whose kind ends in
/// `identifier`, or one of the language's identifier-like kinds (Ruby
/// `constant`, `instance_variable`, `class_variable`, `global_variable`;
/// Elixir `alias`; PHP `name`; Bash `variable_name`). Go `blank_identifier`,
/// `operator_identifier` and the Elixir keyword-like call targets are not
/// identifier tokens.
pub fn is_identifier_token(grammar: Grammar, node: &tree_sitter::Node, source: &str) -> bool {
    if node.child_count() != 0 {
        return false;
    }
    let kind = node.kind();
    match kind {
        "blank_identifier" | "operator_identifier" => return false,
        "identifier" if grammar == Grammar::Elixir => {
            let text = &source[node.byte_range()];
            return !ELIXIR_KEYWORD_TARGETS.contains(&text);
        }
        _ => {}
    }
    if kind.ends_with("identifier") {
        return true;
    }
    match grammar {
        Grammar::Ruby => matches!(
            kind,
            "constant" | "instance_variable" | "class_variable" | "global_variable"
        ),
        Grammar::Elixir => kind == "alias",
        Grammar::Php => kind == "name",
        Grammar::Bash => kind == "variable_name",
        _ => false,
    }
}
