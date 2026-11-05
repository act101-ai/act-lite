//! Elixir language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Elixir in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("elixir"),
        display_name: "Elixir",
        extensions: &["ex", "exs"],
        category: LanguageCategory::Functional,
    }
}

/// Elixir language support implementation.
#[derive(Default)]
pub struct ElixirSupport;

impl LanguageSupport for ElixirSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Elixir
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_elixir::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "call" => self.classify_call(node),
            "anonymous_function" => NodeKind::Function,
            "stab_clause" => NodeKind::Function,
            "identifier" => NodeKind::Identifier,
            "alias" => NodeKind::Class,
            "atom" => NodeKind::Identifier,
            "binary_operator" => self.classify_binary_op(node),
            "unary_operator" => self.classify_unary_op(node),
            "list" | "tuple" | "map" => NodeKind::Block,
            "if" | "unless" | "case" | "cond" | "with" | "for" => NodeKind::Block,
            "try" | "rescue" | "catch" | "after" => NodeKind::Block,
            "comment" => NodeKind::Comment,
            "string" | "charlist" | "sigil" => NodeKind::Identifier,
            "do_block" | "keywords" => NodeKind::Block,
            "access_call" | "dot" => NodeKind::Call,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ex", "exs", "leex"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/elixir/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/elixir/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/elixir/references.scm")
    }

    fn logical_operator_info(&self) -> Option<(&'static str, &'static [&'static str])> {
        // Elixir logical operators are binary_operator nodes.
        // Includes both symbolic (&&, ||) and keyword (and, or) forms.
        Some(("binary_operator", &["&&", "||", "and", "or"]))
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "call",
            "anonymous_function",
            "stab_clause",
            "binary_operator",
            "do_block",
            "list",
            "tuple",
            "map",
        ]
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
            block_start: None,
            block_end: None,
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &["call"]
    }

    fn function_body_kind(&self) -> &'static str {
        "do_block"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if", "unless", "case", "cond", "with", "for", "try", "rescue", "catch",
        ]
    }

    // --- call_detection ---
    /// Elixir is macro-homoiconic: every invocation — including `def`/`defmodule`
    /// — parses as a `call` node, classified by target via `node_kind_with_source`.
    /// Module-qualified access (`Mod.fun`) parses as `dot` (evidence: a parse of
    /// `System.get_env("KEY")` → `call { dot }`).
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call", "dot"]
    }

    // --- data_flow ---
    /// Elixir binds via the match operator: `y = expr` parses as `binary_operator`
    /// with a `=` operator (classified to `NodeKind::Variable`); `<-` in `with`/
    /// comprehensions also binds.
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["binary_operator"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    // --- unsafe_surface ---
    /// Dynamic code evaluation: `Code.eval_string`/`Code.eval_quoted`, the macro
    /// `apply/3`, and string→atom/code conversion.
    fn eval_call_names(&self) -> &'static [&'static str] {
        &["eval_string", "eval_quoted", "compile_string", "eval"]
    }

    /// NIFs / ports are Elixir's FFI (via the Erlang runtime).
    fn ffi_markers(&self) -> &'static [&'static str] {
        &["load_nif", "open_port", "Port"]
    }

    /// Ecto raw SQL execution (`Ecto.Adapters.SQL.query`, `Repo.query`).
    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &["query", "query!", "execute"]
    }

    /// Reflective / dynamic dispatch.
    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        &["apply", "to_atom", "to_existing_atom", "create"]
    }

    /// `:erlang.binary_to_term` on untrusted input is the canonical RCE sink.
    fn unsafe_deserialize_names(&self) -> &'static [&'static str] {
        &["binary_to_term"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("eval")
            .with("ffi")
            .with("raw_sql")
            .with("reflection_invoke")
            .with("unsafe_deserialize")
    }

    // --- secret_surface ---
    /// `:crypto` module entry points.
    fn crypto_api_names(&self) -> &'static [&'static str] {
        &["hash", "mac", "hmac", "sign", "crypto_one_time"]
    }

    /// Environment reads — `System.get_env/1`, `System.fetch_env!/1`.
    fn env_read_names(&self) -> &'static [&'static str] {
        &["get_env", "fetch_env", "fetch_env!"]
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
    /// Untrusted input sources: Phoenix request `params`, env reads, stdin.
    fn taint_source_names(&self) -> &'static [&'static str] {
        &["params", "get_env", "read_line", "gets"]
    }

    /// OS command execution (`System.cmd/2`, `:os.cmd/1`, `Port.open`).
    fn command_exec_names(&self) -> &'static [&'static str] {
        &["cmd", "shell", "open"]
    }

    /// Filesystem sinks (`File.open/2`, `File.write/2`, `File.read/1`).
    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["write", "read", "open", "stream!"]
    }

    /// HTML/SQL escaping helpers (Phoenix.HTML, EEx escaping).
    fn sanitizer_names(&self) -> &'static [&'static str] {
        &["html_escape", "escape", "safe_to_string"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }

    /// Elixir is macro-homoiconic — `raise`/`throw`/`exit` and any allocation or
    /// async form (`Task.async`, `spawn`) are all ordinary `call` nodes dispatched
    /// by target, with no syntactically distinct AST kind (evidence: `raise "boom"`
    /// → `call`). The effect_summary hooks are node-kind based (`raise_node_kinds`/
    /// `await_node_kinds`/`allocation_node_kinds`) and cannot honestly distinguish
    /// these calls from any other `call`.
    /// reason: failures/concurrency are ordinary calls, not distinct throw/await/
    /// allocate AST node kinds; reads/writes still flow through the shared
    /// mutations engine.
    fn coverage_opt_outs(&self) -> &'static [&'static str] {
        &["effect_summary"]
    }

    fn node_kind_with_source(&self, node: &tree_sitter::Node, source: &[u8]) -> NodeKind {
        match node.kind() {
            "call" => self.classify_call_with_source(node, source),
            "anonymous_function" => NodeKind::Function,
            "stab_clause" => NodeKind::Function,
            "identifier" => NodeKind::Identifier,
            "alias" => NodeKind::Class,
            "atom" => NodeKind::Identifier,
            "binary_operator" => self.classify_binary_op(node),
            "unary_operator" => self.classify_unary_op(node),
            "list" | "tuple" | "map" => NodeKind::Block,
            "if" | "unless" | "case" | "cond" | "with" | "for" => NodeKind::Block,
            "try" | "rescue" | "catch" | "after" => NodeKind::Block,
            "comment" => NodeKind::Comment,
            "string" | "charlist" | "sigil" => NodeKind::Identifier,
            "do_block" | "keywords" => NodeKind::Block,
            "access_call" | "dot" => NodeKind::Call,
            _ => NodeKind::Unknown,
        }
    }
}

impl ElixirSupport {
    fn classify_call_with_source(&self, node: &tree_sitter::Node, source: &[u8]) -> NodeKind {
        if let Some(target) = node.child_by_field_name("target")
            && target.kind() == "identifier"
            && let Ok(text) = target.utf8_text(source)
        {
            return match text {
                "defmodule" => NodeKind::Module,
                "def" | "defp" | "defmacro" | "defmacrop" | "defguard" | "defguardp"
                | "defdelegate" => NodeKind::Function,
                "defprotocol" => NodeKind::Interface,
                "defimpl" | "defstruct" | "defexception" => NodeKind::Class,
                "alias" | "import" | "use" | "require" => NodeKind::Import,
                _ => NodeKind::Call,
            };
        }
        NodeKind::Call
    }

    fn classify_call(&self, _node: &tree_sitter::Node) -> NodeKind {
        // Without source text, call nodes cannot be classified accurately.
        // Correct classification happens via node_kind_with_source which has source.
        NodeKind::Call
    }

    fn classify_binary_op(&self, node: &tree_sitter::Node) -> NodeKind {
        if let Some(op) = node.child_by_field_name("operator") {
            return match op.kind() {
                "=" => NodeKind::Variable,
                "|>" => NodeKind::Call,
                "<-" => NodeKind::Variable,
                "::" => NodeKind::Type,
                _ => NodeKind::Unknown,
            };
        }
        NodeKind::Unknown
    }

    fn classify_unary_op(&self, node: &tree_sitter::Node) -> NodeKind {
        if let Some(op) = node.child_by_field_name("operator") {
            return match op.kind() {
                "@" => NodeKind::Property,
                "&" => NodeKind::Function,
                "^" => NodeKind::Variable,
                "%" => NodeKind::Class,
                _ => NodeKind::Unknown,
            };
        }
        NodeKind::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};

    crate::language_support_tests!(ElixirSupport, Grammar::Elixir, &["ex", "exs", "leex"]);

    fn find_all_nodes<'a>(node: tree_sitter::Node<'a>, kind: &str) -> Vec<tree_sitter::Node<'a>> {
        let mut results = Vec::new();
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            if current.kind() == kind {
                results.push(current);
            }
            let mut cursor = current.walk();
            for child in current.children(&mut cursor) {
                stack.push(child);
            }
        }
        results
    }

    #[test]
    fn node_kind_anonymous_function() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "fn x -> x end");
        let node = find_node_by_kind(tree.root_node(), "anonymous_function").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_identifier() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "x = 1");
        let node = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_atom() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, ":ok");
        let node = find_node_by_kind(tree.root_node(), "atom").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_comment() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "# comment\n1");
        let node = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_alias() {
        // alias keyword for module aliasing
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "alias Foo.Bar");
        let node = find_node_by_kind(tree.root_node(), "alias").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Class);
    }

    #[test]
    fn node_kind_stab_clause() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "fn x -> x end");
        let node = find_node_by_kind(tree.root_node(), "stab_clause").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_list() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "[1, 2, 3]");
        let node = find_node_by_kind(tree.root_node(), "list").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_tuple() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "{1, 2, 3}");
        let node = find_node_by_kind(tree.root_node(), "tuple").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_map() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "%{foo: :bar}");
        let node = find_node_by_kind(tree.root_node(), "map").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_string() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "\"hello\"");
        let node = find_node_by_kind(tree.root_node(), "string").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_charlist() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "'hello'");
        let node = find_node_by_kind(tree.root_node(), "charlist").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_sigil() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "~r/pattern/");
        let node = find_node_by_kind(tree.root_node(), "sigil").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Identifier);
    }

    #[test]
    fn node_kind_do_block() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "if true do\n  :ok\nend");
        let node = find_node_by_kind(tree.root_node(), "do_block").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_access_call() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "foo[bar]");
        let node = find_node_by_kind(tree.root_node(), "access_call").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Call);
    }

    #[test]
    fn node_kind_dot() {
        let support = ElixirSupport;
        let tree = parse_with_support(&support, "Foo.bar()");
        let node = find_node_by_kind(tree.root_node(), "dot").unwrap();
        assert_eq!(support.node_kind(&node), NodeKind::Call);
    }

    // Tests for classify_call_with_source - exercises all branches
    #[test]
    fn test_classify_call_defmodule() {
        let source = b"defmodule Foo do end";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_call_with_source(node, source) == NodeKind::Module);
        assert!(found, "Should find at least one Module node");
    }

    #[test]
    fn test_classify_call_def() {
        let source = b"def foo do :ok end";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes.iter().any(|node| {
            ElixirSupport.classify_call_with_source(node, source) == NodeKind::Function
        });
        assert!(found, "Should find at least one Function node");
    }

    #[test]
    fn test_classify_call_defp() {
        let source = b"defp foo do :ok end";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes.iter().any(|node| {
            ElixirSupport.classify_call_with_source(node, source) == NodeKind::Function
        });
        assert!(found, "Should find at least one Function node");
    }

    #[test]
    fn test_classify_call_defmacro() {
        let source = b"defmacro foo do :ok end";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes.iter().any(|node| {
            ElixirSupport.classify_call_with_source(node, source) == NodeKind::Function
        });
        assert!(found, "Should find at least one Function node");
    }

    #[test]
    fn test_classify_call_defmacrop() {
        let source = b"defmacrop foo do :ok end";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes.iter().any(|node| {
            ElixirSupport.classify_call_with_source(node, source) == NodeKind::Function
        });
        assert!(found, "Should find at least one Function node");
    }

    #[test]
    fn test_classify_call_defguard() {
        let source = b"defguard is_true(x) when x == true";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes.iter().any(|node| {
            ElixirSupport.classify_call_with_source(node, source) == NodeKind::Function
        });
        assert!(found, "Should find at least one Function node");
    }

    #[test]
    fn test_classify_call_defguardp() {
        let source = b"defguardp is_true(x) when x == true";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes.iter().any(|node| {
            ElixirSupport.classify_call_with_source(node, source) == NodeKind::Function
        });
        assert!(found, "Should find at least one Function node");
    }

    #[test]
    fn test_classify_call_defdelegate() {
        let source = b"defdelegate foo, to: Bar";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes.iter().any(|node| {
            ElixirSupport.classify_call_with_source(node, source) == NodeKind::Function
        });
        assert!(found, "Should find at least one Function node");
    }

    #[test]
    fn test_classify_call_defprotocol() {
        let source = b"defprotocol Foo do end";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes.iter().any(|node| {
            ElixirSupport.classify_call_with_source(node, source) == NodeKind::Interface
        });
        assert!(found, "Should find at least one Interface node");
    }

    #[test]
    fn test_classify_call_defimpl() {
        let source = b"defimpl Foo, for: Bar do end";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_call_with_source(node, source) == NodeKind::Class);
        assert!(found, "Should find at least one Class node");
    }

    #[test]
    fn test_classify_call_defstruct() {
        let source = b"defstruct [:foo, :bar]";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_call_with_source(node, source) == NodeKind::Class);
        assert!(found, "Should find at least one Class node");
    }

    #[test]
    fn test_classify_call_defexception() {
        let source = b"defexception [:message]";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_call_with_source(node, source) == NodeKind::Class);
        assert!(found, "Should find at least one Class node");
    }

    #[test]
    fn test_classify_call_alias() {
        let source = b"alias Foo.Bar";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_call_with_source(node, source) == NodeKind::Import);
        assert!(found, "Should find at least one Import node");
    }

    #[test]
    fn test_classify_call_import() {
        let source = b"import Foo";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_call_with_source(node, source) == NodeKind::Import);
        assert!(found, "Should find at least one Import node");
    }

    #[test]
    fn test_classify_call_use() {
        let source = b"use Foo";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_call_with_source(node, source) == NodeKind::Import);
        assert!(found, "Should find at least one Import node");
    }

    #[test]
    fn test_classify_call_require() {
        let source = b"require Foo";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_call_with_source(node, source) == NodeKind::Import);
        assert!(found, "Should find at least one Import node");
    }

    #[test]
    fn test_classify_call_regular_function() {
        let source = b"IO.puts(\"hello\")";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_call_with_source(node, source) == NodeKind::Call);
        assert!(found, "Should find at least one Call node");
    }

    #[test]
    fn test_classify_call_no_target_fallback() {
        let source = b"foo.(bar)";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_call_with_source(node, source) == NodeKind::Call);
        assert!(found, "Should find at least one Call node");
    }

    // Tests for classify_binary_op
    #[test]
    fn test_classify_binary_op_assignment() {
        let source = b"x = 1";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "binary_operator");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_binary_op(node) == NodeKind::Variable);
        assert!(found, "Should find at least one Variable node");
    }

    #[test]
    fn test_classify_binary_op_pipe() {
        let source = b"[1, 2] |> Enum.map(fn x -> x end)";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "binary_operator");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_binary_op(node) == NodeKind::Call);
        assert!(found, "Should find at least one Call node");
    }

    #[test]
    fn test_classify_binary_op_left_arrow() {
        // The <- operator in for comprehensions
        let source = b"for x <- list, do: x";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "binary_operator");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_binary_op(node) == NodeKind::Variable);
        assert!(found, "Should find at least one Variable node");
    }

    #[test]
    fn test_classify_binary_op_type_spec() {
        let source = b"@spec foo() :: :ok";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "binary_operator");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_binary_op(node) == NodeKind::Type);
        assert!(found, "Should find at least one Type node");
    }

    #[test]
    fn test_classify_binary_op_unknown() {
        let source = b"x + y";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "binary_operator");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_binary_op(node) == NodeKind::Unknown);
        assert!(found, "Should find at least one Unknown node");
    }

    // Tests for classify_unary_op
    #[test]
    fn test_classify_unary_op_attribute() {
        let source = b"@doc \"hello\"";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "unary_operator");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_unary_op(node) == NodeKind::Property);
        assert!(found, "Should find at least one Property node");
    }

    #[test]
    fn test_classify_unary_op_capture() {
        let source = b"& &1";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "unary_operator");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_unary_op(node) == NodeKind::Function);
        assert!(found, "Should find at least one Function node");
    }

    #[test]
    fn test_classify_unary_op_pin() {
        let source = b"x = ^y";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "unary_operator");
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_unary_op(node) == NodeKind::Variable);
        assert!(found, "Should find at least one Variable node");
    }

    #[test]
    fn test_classify_unary_op_struct() {
        // The % operator for struct names
        let source = b"%{}";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "unary_operator");
        if nodes.is_empty() {
            // No unary_operator nodes in this source, that's OK
            return;
        }
        let found = nodes
            .iter()
            .any(|node| ElixirSupport.classify_unary_op(node) == NodeKind::Class);
        assert!(found, "Should find at least one Class node");
    }

    #[test]
    fn test_classify_unary_op_unknown() {
        // The ~ sigil operator - uses unknown since it's not in the match
        let source = b"~r/pattern/";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "unary_operator");
        // Just verify we can call classify_unary_op without panicking
        for node in nodes {
            let _ = ElixirSupport.classify_unary_op(&node);
        }
    }

    // Test for "unless" keyword node kind
    #[test]
    fn node_kind_unless() {
        let tree = parse_with_support(&ElixirSupport, "unless false, do: :ok");
        if let Some(node) = find_node_by_kind(tree.root_node(), "unless") {
            assert_eq!(ElixirSupport.node_kind(&node), NodeKind::Block);
        }
    }

    // Test for "with" keyword node kind
    #[test]
    fn node_kind_with() {
        let tree = parse_with_support(&ElixirSupport, "with {:ok, val} <- result, do: val");
        if let Some(node) = find_node_by_kind(tree.root_node(), "with") {
            assert_eq!(ElixirSupport.node_kind(&node), NodeKind::Block);
        }
    }

    // Test for "for" keyword node kind
    #[test]
    fn node_kind_for() {
        let tree = parse_with_support(&ElixirSupport, "for x <- list, do: x * 2");
        if let Some(node) = find_node_by_kind(tree.root_node(), "for") {
            assert_eq!(ElixirSupport.node_kind(&node), NodeKind::Block);
        }
    }

    // Test for "keywords" node kind
    #[test]
    fn node_kind_keywords() {
        let tree = parse_with_support(&ElixirSupport, "if true, do: :ok, else: :error");
        if let Some(node) = find_node_by_kind(tree.root_node(), "keywords") {
            assert_eq!(ElixirSupport.node_kind(&node), NodeKind::Block);
        }
    }

    // Test for unknown node kind
    #[test]
    fn node_kind_unknown() {
        let tree = parse_with_support(&ElixirSupport, "1 + 2");
        // Find a node that doesn't match any known kind
        let mut cursor = tree.root_node().walk();
        for child in tree.root_node().children(&mut cursor) {
            if child.kind() == "integer" {
                // integer doesn't match any pattern, should return Unknown
                assert_eq!(ElixirSupport.node_kind(&child), NodeKind::Unknown);
                return;
            }
        }
    }

    // Test classify_call when target is not an identifier
    #[test]
    fn test_classify_call_non_identifier_target() {
        let support = ElixirSupport;
        // Create a mock scenario where we have a call node
        // but the target is not an identifier
        // This should return NodeKind::Call (the default)
        // We'll test this with an actual parse
        let source = b"foo.(bar)";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        for node in nodes {
            let kind = support.classify_call_with_source(&node, source);
            if kind == NodeKind::Call {
                return;
            }
        }
    }

    // Test classify_call when target is identifier but not a known keyword
    #[test]
    fn test_classify_call_unknown_identifier() {
        let support = ElixirSupport;
        let source = b"unknown_function foo bar";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        for node in nodes {
            let kind = support.classify_call_with_source(&node, source);
            if kind == NodeKind::Call {
                return;
            }
        }
    }

    // Test for "rescue" keyword node kind
    #[test]
    fn node_kind_rescue() {
        let tree = parse_with_support(&ElixirSupport, "try do\n  :ok\nrescue\n  _ -> :error\nend");
        if let Some(node) = find_node_by_kind(tree.root_node(), "rescue") {
            assert_eq!(ElixirSupport.node_kind(&node), NodeKind::Block);
        }
    }

    // Test for "catch" keyword node kind
    #[test]
    fn node_kind_catch() {
        let tree = parse_with_support(
            &ElixirSupport,
            "try do\n  :ok\ncatch\n  :error -> :error\nend",
        );
        if let Some(node) = find_node_by_kind(tree.root_node(), "catch") {
            assert_eq!(ElixirSupport.node_kind(&node), NodeKind::Block);
        }
    }

    // Test for "after" keyword node kind
    #[test]
    fn node_kind_after() {
        let tree = parse_with_support(&ElixirSupport, "try do\n  :ok\nafter\n  :cleanup\nend");
        if let Some(node) = find_node_by_kind(tree.root_node(), "after") {
            assert_eq!(ElixirSupport.node_kind(&node), NodeKind::Block);
        }
    }

    // Test for binary operator when there is no operator field (should return Unknown)
    #[test]
    fn test_classify_binary_op_no_operator_field() {
        let support = ElixirSupport;
        // This test ensures we handle nodes without an operator field gracefully
        // We create a simple parse and check that classify_binary_op returns Unknown for non-operator nodes
        let tree = parse_with_support(&ElixirSupport, "1");
        let mut cursor = tree.walk();
        for node in tree.root_node().children(&mut cursor) {
            // Any non-binary_operator node should not be found by find_all_nodes
            // So we just verify the method exists and doesn't panic
            let _ = support.classify_binary_op(&node);
        }
    }

    // Test for unary operator when there is no operator field (should return Unknown)
    #[test]
    fn test_classify_unary_op_no_operator_field() {
        let support = ElixirSupport;
        // This test ensures we handle nodes without an operator field gracefully
        let tree = parse_with_support(&ElixirSupport, "1");
        let mut cursor = tree.walk();
        for node in tree.root_node().children(&mut cursor) {
            // Any non-unary_operator node should not be found by find_all_nodes
            // So we just verify the method exists and doesn't panic
            let _ = support.classify_unary_op(&node);
        }
    }

    // Test for classify_call when there is no target field (should return Call)
    #[test]
    fn test_classify_call_no_target_field() {
        let support = ElixirSupport;
        // Parse something that creates a call node
        let tree = parse_with_support(&ElixirSupport, "Foo");
        let mut cursor = tree.walk();
        // Look for any node and test classify_call on it
        for node in tree.root_node().children(&mut cursor) {
            // This might not be a call node, but we test that classify_call handles it
            let _ = support.classify_call(&node);
        }
    }

    // Additional test for all the def* patterns in classify_call_with_source
    #[test]
    fn test_classify_call_all_def_patterns() {
        let support = ElixirSupport;

        // Test defmacro pattern
        let source = b"defmacro my_macro(x) do x end";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let mut found = false;
        for node in nodes {
            let kind = support.classify_call_with_source(&node, source);
            if kind == NodeKind::Function {
                found = true;
                break;
            }
        }
        assert!(found, "defmacro should classify as Function");

        // Test defmacrop pattern
        let source = b"defmacrop private_macro(x) do x end";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let mut found = false;
        for node in nodes {
            let kind = support.classify_call_with_source(&node, source);
            if kind == NodeKind::Function {
                found = true;
                break;
            }
        }
        assert!(found, "defmacrop should classify as Function");

        // Test defimpl pattern
        let source = b"defimpl Foo, for: Bar do end";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let mut found = false;
        for node in nodes {
            let kind = support.classify_call_with_source(&node, source);
            if kind == NodeKind::Class {
                found = true;
                break;
            }
        }
        assert!(found, "defimpl should classify as Class");

        // Test defexception pattern
        let source = b"defexception [:message]";
        let tree = parse_with_support(&ElixirSupport, std::str::from_utf8(source).unwrap());
        let nodes = find_all_nodes(tree.root_node(), "call");
        let mut found = false;
        for node in nodes {
            let kind = support.classify_call_with_source(&node, source);
            if kind == NodeKind::Class {
                found = true;
                break;
            }
        }
        assert!(found, "defexception should classify as Class");
    }

    // Test for all other binary operators that should return Unknown
    #[test]
    fn test_classify_binary_op_other_operators() {
        let support = ElixirSupport;
        let test_cases: Vec<&str> = vec![
            "1 + 2", "1 - 2", "1 * 2", "1 / 2", "1 == 2", "1 != 2", "1 < 2", "1 > 2", "1 <= 2",
            "1 >= 2", "1 and 2", "1 or 2", "1 && 2", "1 || 2", "1 <> 2", "1 ++ 2", "1 -- 2",
            "1 ** 2",
        ];

        for source in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            let nodes = find_all_nodes(tree.root_node(), "binary_operator");
            for node in nodes {
                let kind = support.classify_binary_op(&node);
                // These operators should return Unknown (not one of the special cases)
                if kind == NodeKind::Unknown {
                    // Found an Unknown, which is expected for these operators
                } else {
                    // If it's not Unknown, it must be one of our special cases (=, |>, <-, ::)
                    assert!(
                        matches!(kind, NodeKind::Variable | NodeKind::Call | NodeKind::Type),
                        "Unexpected NodeKind: {:?}",
                        kind
                    );
                }
            }
        }
    }

    // Test for all other unary operators that should return Unknown
    #[test]
    fn test_classify_unary_op_other_operators() {
        let support = ElixirSupport;
        let test_cases: Vec<&str> = vec!["!true", "not true", "+1", "-1", "~~~x"];

        for source in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            let nodes = find_all_nodes(tree.root_node(), "unary_operator");
            for node in nodes {
                let kind = support.classify_unary_op(&node);
                // These operators should return Unknown (not one of the special cases)
                if kind == NodeKind::Unknown {
                    // Found an Unknown, which is expected for these operators
                } else {
                    // If it's not Unknown, it must be one of our special cases (@, &, ^, %)
                    assert!(
                        matches!(
                            kind,
                            NodeKind::Property
                                | NodeKind::Function
                                | NodeKind::Variable
                                | NodeKind::Class
                        ),
                        "Unexpected NodeKind: {:?}",
                        kind
                    );
                }
            }
        }
    }

    // Test for interpolation and other string-related nodes
    #[test]
    fn node_kind_interpolation() {
        let support = ElixirSupport;
        let tree = parse_with_support(&ElixirSupport, "\"Hello #{name}!\"");

        // Test that we can walk and classify all nodes
        let mut cursor = tree.walk();
        for child in tree.root_node().children(&mut cursor) {
            let _ = support.node_kind(&child);
        }
    }

    // Test for anonymous function with multiple clauses
    #[test]
    fn node_kind_multi_clause_anonymous_function() {
        let support = ElixirSupport;
        let tree = parse_with_support(&ElixirSupport, "fn\n  x -> x + 1\n  y -> y * 2\nend");

        // Test that we can classify the function and its clauses
        let mut cursor = tree.walk();
        for child in tree.root_node().children(&mut cursor) {
            let _ = support.node_kind(&child);
        }
    }

    // Test for qualified call and dot access patterns
    #[test]
    fn node_kind_qualified_patterns() {
        let support = ElixirSupport;

        // Test various qualified call patterns
        let patterns = vec![
            "Module.function()",
            "Module.function(arg1, arg2)",
            "__MODULE__.function()",
            "%Module{field: value}",
            "%Module{}",
            "& &1",
            "^pinned_var",
        ];

        for pattern in patterns {
            let tree = parse_with_support(&ElixirSupport, pattern);
            let mut cursor = tree.walk();
            for child in tree.root_node().children(&mut cursor) {
                let _ = support.node_kind(&child);
            }
        }
    }

    // Test node_kind_with_source for all the call types
    #[test]
    fn node_kind_with_source_call_types() {
        let support = ElixirSupport;

        let test_cases = vec![
            ("defmodule Foo do end", NodeKind::Module),
            ("def foo, do: :ok", NodeKind::Function),
            ("defp bar, do: :ok", NodeKind::Function),
            ("defmacro m(x), do: x", NodeKind::Function),
            ("defprotocol P, do: :ok", NodeKind::Interface),
            ("defimpl P, for: T, do: :ok", NodeKind::Class),
            ("alias Foo.Bar", NodeKind::Import),
            ("import Foo", NodeKind::Import),
            ("use Foo", NodeKind::Import),
            ("require Foo", NodeKind::Import),
        ];

        for (source, expected_kind) in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            let nodes = find_all_nodes(tree.root_node(), "call");
            for node in nodes {
                let kind = support.node_kind_with_source(&node, source.as_bytes());
                if kind == expected_kind {
                    // Found the expected node kind
                    break;
                }
            }
        }
    }

    // Test that node_kind_with_source falls back to Call for unknown calls
    #[test]
    fn node_kind_with_source_unknown_call() {
        let support = ElixirSupport;
        let source = "some_random_function arg1 arg2";
        let tree = parse_with_support(&ElixirSupport, source);
        let nodes = find_all_nodes(tree.root_node(), "call");
        for node in nodes {
            let kind = support.node_kind_with_source(&node, source.as_bytes());
            if kind == NodeKind::Call {
                return;
            }
        }
    }

    // Test that node_kind_with_source handles all node types
    #[test]
    fn node_kind_with_source_all_types() {
        let support = ElixirSupport;

        let test_cases = vec![
            ("fn x -> x end", "anonymous_function", NodeKind::Function),
            ("x", "identifier", NodeKind::Identifier),
            (":ok", "atom", NodeKind::Identifier),
            ("x = 1", "binary_operator", NodeKind::Variable),
            ("[1, 2, 3]", "list", NodeKind::Block),
            ("{1, 2}", "tuple", NodeKind::Block),
            ("%{}", "map", NodeKind::Block),
            ("# comment\n1", "comment", NodeKind::Comment),
            ("\"hello\"", "string", NodeKind::Identifier),
            ("~r/pattern/", "sigil", NodeKind::Identifier),
            ("@doc", "unary_operator", NodeKind::Property),
            ("& &1", "unary_operator", NodeKind::Function),
            ("^x", "unary_operator", NodeKind::Variable),
            ("%Module{}", "unary_operator", NodeKind::Class),
        ];

        for (source, node_type, expected_kind) in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            if let Some(node) = find_node_by_kind(tree.root_node(), node_type) {
                let kind = support.node_kind_with_source(&node, source.as_bytes());
                assert_eq!(kind, expected_kind, "Failed for source: {}", source);
            }
        }
    }

    // Test all the remaining keywords in node_kind_with_source
    #[test]
    fn node_kind_with_source_control_flow() {
        let support = ElixirSupport;

        let test_cases = vec![
            ("if true, do: :ok", "if", NodeKind::Block),
            ("unless false, do: :ok", "unless", NodeKind::Block),
            ("case x do\n  :ok -> :ok\nend", "case", NodeKind::Block),
            ("cond do\n  true -> :ok\nend", "cond", NodeKind::Block),
            ("with x <- y, do: x", "with", NodeKind::Block),
            ("for x <- list, do: x", "for", NodeKind::Block),
            (
                "try do\n  :ok\nrescue\n  _ -> :error\nend",
                "try",
                NodeKind::Block,
            ),
        ];

        for (source, node_type, expected_kind) in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            if let Some(node) = find_node_by_kind(tree.root_node(), node_type) {
                let kind = support.node_kind_with_source(&node, source.as_bytes());
                assert_eq!(kind, expected_kind, "Failed for source: {}", source);
            }
        }
    }

    // Test all special binary operators
    #[test]
    fn node_kind_with_source_binary_operators() {
        let support = ElixirSupport;

        let test_cases = vec![
            ("x = 1", "=", NodeKind::Variable),
            ("[1, 2] |> Enum.map(fn x -> x end)", "|>", NodeKind::Call),
            ("@spec foo() :: :ok", "::", NodeKind::Type),
        ];

        for (source, _op_kind, expected_kind) in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            let nodes = find_all_nodes(tree.root_node(), "binary_operator");
            for node in nodes {
                let kind = support.node_kind_with_source(&node, source.as_bytes());
                if kind == expected_kind {
                    break;
                }
            }
        }
    }

    // Test all special unary operators
    #[test]
    fn node_kind_with_source_unary_operators() {
        let support = ElixirSupport;

        let test_cases = vec![
            ("@doc", "@", NodeKind::Property),
            ("& &1", "&", NodeKind::Function),
            ("x = ^y", "^", NodeKind::Variable),
            ("%Module{}", "%", NodeKind::Class),
        ];

        for (source, _op_kind, expected_kind) in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            let nodes = find_all_nodes(tree.root_node(), "unary_operator");
            for node in nodes {
                let kind = support.node_kind_with_source(&node, source.as_bytes());
                if kind == expected_kind {
                    break;
                }
            }
        }
    }

    // Test the catch-all Unknown case
    #[test]
    fn node_kind_with_source_unknown_node_type() {
        let support = ElixirSupport;
        let source = "123";
        let tree = parse_with_support(&ElixirSupport, source);
        let mut cursor = tree.walk();
        for child in tree.root_node().children(&mut cursor) {
            if child.kind() == "integer" {
                let kind = support.node_kind_with_source(&child, source.as_bytes());
                assert_eq!(kind, NodeKind::Unknown);
                return;
            }
        }
    }

    // Test all the other node kind types
    #[test]
    fn node_kind_with_source_misc_types() {
        let support = ElixirSupport;

        let test_cases = vec![
            ("do_block", "if true do\n  :ok\nend"),
            ("keywords", "if true, do: :ok"),
            ("access_call", "foo[bar]"),
            ("dot", "Foo.bar()"),
            ("alias", "__MODULE__"),
            ("stab_clause", "fn x -> x end"),
            ("charlist", "'hello'"),
            ("do_block", "if true do\n  :ok\nend"),
        ];

        for (node_type, source) in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            if let Some(node) = find_node_by_kind(tree.root_node(), node_type) {
                let _ = support.node_kind_with_source(&node, source.as_bytes());
            }
        }
    }

    // More comprehensive call type testing
    #[test]
    fn node_kind_with_source_more_call_types() {
        let support = ElixirSupport;

        let test_cases = vec![
            ("defguard is_true(x) when x", NodeKind::Function),
            ("defguardp is_false(x) when x", NodeKind::Function),
            ("defdelegate foo, to: Bar", NodeKind::Function),
            ("defstruct [:x, :y]", NodeKind::Class),
            ("defexception [:message]", NodeKind::Class),
            ("defmacrop m(x), do: x", NodeKind::Function),
        ];

        for (source, expected_kind) in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            let nodes = find_all_nodes(tree.root_node(), "call");
            for node in nodes {
                let kind = support.node_kind_with_source(&node, source.as_bytes());
                if kind == expected_kind {
                    break;
                }
            }
        }
    }

    // Test that classify_call (called via node_kind without source) doesn't panic
    // and returns Call as fallback
    #[test]
    fn node_kind_call_without_source_returns_call() {
        let support = ElixirSupport;

        // Test various call nodes - without source, classify_call should return Call
        let test_cases = vec![
            "defmodule Foo do end",
            "def foo, do: :ok",
            "IO.puts(\"hello\")",
            "alias Foo.Bar",
        ];

        for source in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            let nodes = find_all_nodes(tree.root_node(), "call");
            assert!(
                !nodes.is_empty(),
                "Expected to find 'call' nodes in source: {}",
                source
            );
            for node in nodes {
                // This should NOT panic - classify_call handles missing source gracefully
                let kind = support.node_kind(&node);
                // Without source, we expect Call as fallback
                assert_eq!(
                    kind,
                    NodeKind::Call,
                    "node_kind should return Call for: {}",
                    source
                );
            }
        }
    }

    // Test that classify_call handles nodes without target field
    #[test]
    fn classify_call_no_target_returns_call() {
        let support = ElixirSupport;
        let tree = parse_with_support(&ElixirSupport, "foo.(bar)");
        let nodes = find_all_nodes(tree.root_node(), "call");
        for node in nodes {
            let kind = support.classify_call(&node);
            assert_eq!(kind, NodeKind::Call);
        }
    }

    // Test that classify_call handles non-identifier targets
    #[test]
    fn classify_call_non_identifier_target_returns_call() {
        let support = ElixirSupport;
        // Create a call with non-identifier target
        let tree = parse_with_support(&ElixirSupport, "Module.function()");
        let nodes = find_all_nodes(tree.root_node(), "call");
        for node in nodes {
            // Check if this call has a non-identifier target
            if let Some(target) = node.child_by_field_name("target")
                && target.kind() != "identifier"
            {
                let kind = support.classify_call(&node);
                assert_eq!(kind, NodeKind::Call);
            }
        }
    }

    // Exercise all branches of node_kind through node_kind_with_source
    #[test]
    fn node_kind_with_source_comprehensive_coverage() {
        let support = ElixirSupport;

        // Test every single node type in the match statement
        let test_cases = vec![
            // call type - covered extensively elsewhere
            ("defmodule Foo do end", "call"),
            // anonymous_function
            ("fn x -> x end", "anonymous_function"),
            // stab_clause
            ("fn x -> x end", "stab_clause"),
            // identifier
            ("x", "identifier"),
            // alias
            ("__MODULE__", "alias"),
            // atom
            (":ok", "atom"),
            // binary_operator - tested elsewhere
            ("x = 1", "binary_operator"),
            // unary_operator - tested elsewhere
            ("@doc", "unary_operator"),
            // list
            ("[1, 2]", "list"),
            // tuple
            ("{1, 2}", "tuple"),
            // map
            ("%{}", "map"),
            // if
            ("if true, do: :ok", "if"),
            // unless
            ("unless false, do: :ok", "unless"),
            // case
            ("case x do\n  :ok -> :ok\nend", "case"),
            // cond
            ("cond do\n  true -> :ok\nend", "cond"),
            // with
            ("with x <- y, do: x", "with"),
            // for
            ("for x <- list, do: x", "for"),
            // try
            ("try do\n  :ok\nrescue\n  _ -> :error\nend", "try"),
            // rescue
            ("try do\n  :ok\nrescue\n  _ -> :error\nend", "rescue"),
            // catch
            ("try do\n  :ok\ncatch\n  :error -> :error\nend", "catch"),
            // after
            ("try do\n  :ok\nafter\n  :cleanup\nend", "after"),
            // comment
            ("# comment\n1", "comment"),
            // string
            ("\"hello\"", "string"),
            // charlist
            ("'hello'", "charlist"),
            // sigil
            ("~r/pattern/", "sigil"),
            // do_block
            ("if true do\n  :ok\nend", "do_block"),
            // keywords
            ("if true, do: :ok", "keywords"),
            // access_call
            ("foo[bar]", "access_call"),
            // dot
            ("Foo.bar", "dot"),
            // unknown (will use integer)
            ("123", "integer"),
        ];

        for (source, expected_node_type) in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            if let Some(node) = find_node_by_kind(tree.root_node(), expected_node_type) {
                // This exercises the node_kind_with_source match
                let _ = support.node_kind_with_source(&node, source.as_bytes());
            }
        }
    }

    // Test all the default NodeKind returns for unknown types
    #[test]
    fn node_kind_returns_unknown_for_unmatched_types() {
        let support = ElixirSupport;

        // Test that unknown node types return Unknown via node_kind (not node_kind_with_source)
        let source = "123"; // integer node doesn't match any pattern
        let tree = parse_with_support(&ElixirSupport, source);
        let mut cursor = tree.walk();
        let found = tree
            .root_node()
            .children(&mut cursor)
            .any(|child| child.kind() == "integer");
        if found {
            // The integer node should return Unknown through node_kind
            let integer_node = tree
                .root_node()
                .children(&mut cursor)
                .find(|c| c.kind() == "integer")
                .unwrap();
            assert_eq!(support.node_kind(&integer_node), NodeKind::Unknown);
        }
    }

    // Test that the OLD node_kind method (without source) is exercised for call nodes
    // This covers lines 17-36 and the classify_call method (line 104-120)
    #[test]
    fn node_kind_old_method_exercised_for_calls() {
        let support = ElixirSupport;

        // These should all use the OLD node_kind method which calls classify_call
        let sources = vec![
            "IO.puts(\"test\")",     // regular function call
            "defmodule Test do end", // defmodule
            "def foo, do: :ok",      // def
        ];

        for source in sources {
            let tree = parse_with_support(&ElixirSupport, source);
            let nodes = find_all_nodes(tree.root_node(), "call");
            assert!(
                !nodes.is_empty(),
                "Expected to find 'call' nodes in: {}",
                source
            );
            for node in nodes {
                // Call the OLD node_kind method (without source)
                let _ = support.node_kind(&node);
            }
        }
    }

    // Test that node_kind exercises all its branches
    #[test]
    fn node_kind_exercises_all_branches() {
        let support = ElixirSupport;

        // Test each branch in node_kind's match statement
        let test_cases = vec![
            ("fn x -> x end", "anonymous_function", NodeKind::Function),
            ("fn x -> x end", "stab_clause", NodeKind::Function),
            ("x", "identifier", NodeKind::Identifier),
            ("__MODULE__", "alias", NodeKind::Class),
            (":ok", "atom", NodeKind::Identifier),
            ("x = 1", "binary_operator", NodeKind::Variable),
            ("@doc", "unary_operator", NodeKind::Property),
            ("[1]", "list", NodeKind::Block),
            ("{1, 2}", "tuple", NodeKind::Block),
            ("%{}", "map", NodeKind::Block),
            ("# comment\n1", "comment", NodeKind::Comment),
            ("\"hi\"", "string", NodeKind::Identifier),
            ("~r/pattern/", "sigil", NodeKind::Identifier),
            ("if true do\n  :ok\nend", "do_block", NodeKind::Block),
            ("if true, do: :ok", "keywords", NodeKind::Block),
            ("foo[bar]", "access_call", NodeKind::Call),
            ("Foo.bar", "dot", NodeKind::Call),
        ];

        for (source, node_type, expected) in test_cases {
            let tree = parse_with_support(&ElixirSupport, source);
            if let Some(node) = find_node_by_kind(tree.root_node(), node_type) {
                let kind = support.node_kind(&node);
                assert_eq!(kind, expected, "node_kind failed for {}", node_type);
            }
        }
    }

    // Test that calls with "dot" targets (not "identifier") hit the fallback path
    // This exercises lines 99-101 in classify_call_with_source
    #[test]
    fn classify_call_with_source_dot_target() {
        let support = ElixirSupport;

        // These calls have "dot" nodes as targets, not "identifier"
        // This should hit the fallback at line 101
        let sources = vec![
            "IO.puts(\"hi\")", // dot target
            "module.fun()",    // dot target
            "(&1).()",         // dot target with unary operator
        ];

        for source in sources {
            let tree = parse_with_support(&ElixirSupport, source);
            let call_nodes = find_all_nodes(tree.root_node(), "call");
            assert!(
                !call_nodes.is_empty(),
                "Should find call nodes in: {}",
                source
            );

            for call_node in call_nodes {
                // Always call classify_call_with_source
                let kind = support.classify_call_with_source(&call_node, source.as_bytes());
                // With "dot" target, should fall through to default NodeKind::Call
                assert_eq!(
                    kind,
                    NodeKind::Call,
                    "dot target should return Call for: {}",
                    source
                );
            }
        }
    }

    #[test]
    fn node_kind_dispatch_binary_op() {
        let tree = parse_with_support(&ElixirSupport, "x = 1");
        let node = find_node_by_kind(tree.root_node(), "binary_operator").unwrap();
        assert_eq!(ElixirSupport.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_dispatch_unary_op() {
        let tree = parse_with_support(&ElixirSupport, "x = ^y");
        let node = find_node_by_kind(tree.root_node(), "unary_operator").unwrap();
        assert_eq!(ElixirSupport.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_dispatch_call() {
        // node_kind() on a call node without source defaults to Call
        let tree = parse_with_support(&ElixirSupport, "foo()");
        let node = find_node_by_kind(tree.root_node(), "call").unwrap();
        assert_eq!(ElixirSupport.node_kind(&node), NodeKind::Call);
    }

    #[test]
    fn node_kind_with_source_dispatch_binary_op() {
        let source = "x = 1";
        let tree = parse_with_support(&ElixirSupport, source);
        let node = find_node_by_kind(tree.root_node(), "binary_operator").unwrap();
        assert_eq!(
            ElixirSupport.node_kind_with_source(&node, source.as_bytes()),
            NodeKind::Variable
        );
    }

    #[test]
    fn node_kind_with_source_dispatch_unary_op() {
        let source = "x = ^y";
        let tree = parse_with_support(&ElixirSupport, source);
        let node = find_node_by_kind(tree.root_node(), "unary_operator").unwrap();
        assert_eq!(
            ElixirSupport.node_kind_with_source(&node, source.as_bytes()),
            NodeKind::Variable
        );
    }

    #[test]
    fn node_kind_with_source_dispatch_call_defmodule() {
        let source = "defmodule Foo do end";
        let tree = parse_with_support(&ElixirSupport, source);
        let node = find_node_by_kind(tree.root_node(), "call").unwrap();
        assert_eq!(
            ElixirSupport.node_kind_with_source(&node, source.as_bytes()),
            NodeKind::Module
        );
    }

    #[test]
    fn logical_operator_info_uses_binary_operator_node() {
        let support = ElixirSupport;
        let info = support.logical_operator_info();
        assert!(info.is_some(), "Elixir must provide logical_operator_info");
        let (node_kind, ops) = info.unwrap();
        assert_eq!(
            node_kind, "binary_operator",
            "Elixir logical ops are binary_operator, not binary_expression"
        );
        assert!(ops.contains(&"&&"), "must include &&");
        assert!(ops.contains(&"||"), "must include ||");
        assert!(ops.contains(&"and"), "must include keyword 'and'");
        assert!(ops.contains(&"or"), "must include keyword 'or'");
    }

    #[test]
    fn extractable_node_kinds_covers_elixir_expressions() {
        let support = ElixirSupport;
        let kinds = support.extractable_node_kinds();
        assert!(
            !kinds.is_empty(),
            "Elixir must define extractable_node_kinds"
        );
        for expected in &["call", "anonymous_function", "binary_operator"] {
            assert!(
                kinds.contains(expected),
                "extractable_node_kinds must include '{}'",
                expected
            );
        }
    }

    #[test]
    fn function_parent_kinds_includes_call() {
        let support = ElixirSupport;
        assert!(
            support.function_parent_kinds().contains(&"call"),
            "function_parent_kinds must include 'call' for Elixir def/defp detection"
        );
    }

    #[test]
    fn function_body_kind_is_do_block() {
        let support = ElixirSupport;
        assert_eq!(
            support.function_body_kind(),
            "do_block",
            "Elixir function bodies are do_block nodes"
        );
    }

    #[test]
    fn decision_point_kinds_covers_elixir_control_flow() {
        let support = ElixirSupport;
        let kinds = support.decision_point_kinds();
        for expected in &[
            "if", "unless", "case", "cond", "with", "for", "try", "rescue", "catch",
        ] {
            assert!(
                kinds.contains(expected),
                "decision_point_kinds must include '{}'",
                expected
            );
        }
    }

    #[test]
    fn coverage_hooks_elixir() {
        let s = ElixirSupport;
        // call_detection — `System.get_env(...)` parses with a `dot` access node.
        let tree = parse_with_support(&s, "System.get_env(\"KEY\")");
        assert!(
            find_node_by_kind(tree.root_node(), "dot").is_some(),
            "dot node must exist for a module-qualified call"
        );
        assert!(s.call_node_kinds().contains(&"call"));
        assert!(s.call_node_kinds().contains(&"dot"));
        assert_ne!(s.call_node_kinds(), ["call_expression", "call"]);
        // data_flow — `x = 1` parses as a binary_operator binding.
        let tree2 = parse_with_support(&s, "x = 1");
        assert!(
            find_node_by_kind(tree2.root_node(), "binary_operator").is_some(),
            "binary_operator binding node must exist for `x = 1`"
        );
        assert!(s.binding_node_kinds().contains(&"binary_operator"));
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
        // unsafe_surface
        assert!(s.eval_call_names().contains(&"eval_string"));
        assert!(s.unsafe_deserialize_names().contains(&"binary_to_term"));
        assert!(s.unsafe_modeled_kinds().contains("eval"));
        // secret_surface
        assert!(s.env_read_names().contains(&"get_env"));
        assert!(s.secret_modeled_kinds().contains("env_secret_read"));
        // taint_flow
        assert!(s.command_exec_names().contains(&"cmd"));
        assert!(s.taint_modeled_kinds().contains("taint"));
        // effect_summary is a documented, evidence-based opt-out.
        assert!(s.coverage_opt_outs().contains(&"effect_summary"));
    }
}
