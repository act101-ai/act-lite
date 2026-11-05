//! Lua language support for act-lite-parser.

use crate::grammar::Grammar;
use crate::language::{CommentDelimiters, LanguageSupport};
use crate::node_kind::NodeKind;
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Lua in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("lua"),
        display_name: "Lua",
        extensions: &["lua"],
        category: LanguageCategory::Scripting,
    }
}

#[derive(Default)]
pub struct LuaSupport;

/// Returns true if this `assignment_statement` node looks like a Lua class definition.
///
/// Lua OOP convention: `ClassName = { ... }` where the right-hand side is a
/// `table_constructor`. This matches both `Animal = {}` and `Person = {name=''}`.
fn is_class_like_table(node: &tree_sitter::Node) -> bool {
    // Structure: assignment_statement
    //   variable_list
    //     identifier
    //   =
    //   expression_list          ← find this named child
    //     table_constructor      ← check this exists
    for i in 0..node.named_child_count() {
        let child = node.named_child(i).unwrap();
        if child.kind() == "expression_list" {
            // The expression_list has at least one child that is a table_constructor
            for j in 0..child.named_child_count() {
                if child.named_child(j).unwrap().kind() == "table_constructor" {
                    return true;
                }
            }
        }
    }
    false
}

impl LanguageSupport for LuaSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Lua
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_lua::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Both `function foo() end` and `local function foo() end` parse as
            // function_declaration. Anonymous function values parse as function_definition.
            "function_declaration" | "function_definition" => NodeKind::Function,
            // local x = ... wraps an assignment_statement in a variable_declaration
            "variable_declaration" | "assignment_statement" => NodeKind::Variable,
            "comment" => NodeKind::Comment,
            "block" | "do_statement" => NodeKind::Block,
            "function_call" => NodeKind::Call,
            _ => NodeKind::Unknown,
        }
    }

    fn node_kind_with_source(&self, node: &tree_sitter::Node, _source: &[u8]) -> NodeKind {
        if node.kind() == "assignment_statement" && is_class_like_table(node) {
            return NodeKind::Class;
        }
        self.node_kind(node)
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["lua"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/lua/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/lua/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/lua/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("--"),
            block_start: Some("--[["),
            block_end: Some("]]"),
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &["function_declaration", "function_definition"]
    }

    fn function_body_kind(&self) -> &'static str {
        "block"
    }

    fn function_keyword(&self) -> &'static str {
        "function"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "elseif_statement",
            "while_statement",
            "repeat_statement",
            "for_statement",
        ]
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "assignment_statement",
            "variable_declaration",
            "if_statement",
            "while_statement",
            "repeat_statement",
            "for_statement",
            "function_call",
            "return_statement",
        ]
    }

    // ---- moat coverage hooks (Lua) ----

    /// Lua's only call node is `function_call` (covers `f()`, `t.f()`, `t:m()`).
    /// Overrides the `call_expression`/`call` default.
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["function_call"]
    }

    /// `local x = ...` (variable_declaration) and `x = ...` (assignment_statement)
    /// bind names.
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["variable_declaration", "assignment_statement"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    /// Blocking stdlib calls: `os.execute` waits on the child process; `io.read`
    /// and luasocket `receive` block on input.
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["execute", "read", "receive"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("block")
    }

    /// Dynamic code loading/execution: `load`/`loadstring` compile a string,
    /// `dofile`/`loadfile` run an arbitrary file.
    fn eval_call_names(&self) -> &'static [&'static str] {
        &["load", "loadstring", "dofile", "loadfile"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("eval")
    }

    /// `os.getenv` reads environment variables (bare name after `os.`).
    fn env_read_names(&self) -> &'static [&'static str] {
        &["getenv"]
    }

    fn secret_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("env_secret_read")
    }

    /// Taint sources: env (`getenv`), stdin (`read`), socket input (`receive`).
    fn taint_source_names(&self) -> &'static [&'static str] {
        &["getenv", "read", "receive"]
    }

    /// Command-injection sinks: `os.execute` and `io.popen` run shell commands.
    fn command_exec_names(&self) -> &'static [&'static str] {
        &["execute", "popen"]
    }

    /// Filesystem sinks: `io.open` / `os.rename` / `os.remove`.
    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["open", "rename", "remove"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::Grammar;
    use crate::language::LanguageSupport;
    use crate::node_kind::NodeKind;

    crate::language_support_tests!(LuaSupport, Grammar::Lua, &["lua"]);

    fn parse(support: &LuaSupport, code: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        let lang = support.ts_language();
        parser.set_language(&lang).expect("Failed to set language");
        parser.parse(code, None).expect("Parse returned None")
    }

    #[test]
    fn node_kind_function_declaration() {
        let support = LuaSupport;
        let tree = parse(&support, "function greet() return 'hi' end");
        let node = tree.root_node().named_child(0).unwrap();
        assert_eq!(node.kind(), "function_declaration");
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_local_function() {
        let support = LuaSupport;
        // local function also parses as function_declaration
        let tree = parse(&support, "local function add(a, b) return a + b end");
        let node = tree.root_node().named_child(0).unwrap();
        assert_eq!(node.kind(), "function_declaration");
        assert_eq!(support.node_kind(&node), NodeKind::Function);
    }

    #[test]
    fn node_kind_variable_declaration() {
        let support = LuaSupport;
        let tree = parse(&support, "local x = 42");
        let node = tree.root_node().named_child(0).unwrap();
        assert_eq!(node.kind(), "variable_declaration");
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_assignment_statement() {
        let support = LuaSupport;
        let tree = parse(&support, "x = 42");
        let node = tree.root_node().named_child(0).unwrap();
        assert_eq!(node.kind(), "assignment_statement");
        assert_eq!(support.node_kind(&node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_function_call() {
        let support = LuaSupport;
        let tree = parse(&support, "print('hello')");
        let node = tree.root_node().named_child(0).unwrap();
        assert_eq!(node.kind(), "function_call");
        assert_eq!(support.node_kind(&node), NodeKind::Call);
    }

    #[test]
    fn node_kind_comment() {
        let support = LuaSupport;
        let code = "-- this is a comment\nx = 1";
        let tree = parse(&support, code);
        let root = tree.root_node();
        let comment_node = root
            .children(&mut root.walk())
            .find(|n| n.kind() == "comment")
            .expect("should have a comment node");
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_block() {
        let support = LuaSupport;
        let tree = parse(&support, "function foo() local x = 1 end");
        let func = tree.root_node().named_child(0).unwrap();
        let body = func
            .children(&mut func.walk())
            .find(|c| c.kind() == "block")
            .expect("function should have a block body");
        assert_eq!(support.node_kind(&body), NodeKind::Block);
    }

    #[test]
    fn comment_delimiters_line_is_double_dash() {
        let support = LuaSupport;
        let delims = support.comment_delimiters();
        assert_eq!(delims.line, Some("--"));
    }

    #[test]
    fn comment_delimiters_block() {
        let support = LuaSupport;
        let delims = support.comment_delimiters();
        assert_eq!(delims.block_start, Some("--[["));
        assert_eq!(delims.block_end, Some("]]"));
    }

    #[test]
    fn function_parent_kinds_nonempty() {
        let support = LuaSupport;
        assert!(!support.function_parent_kinds().is_empty());
    }

    #[test]
    fn function_body_kind_is_block() {
        let support = LuaSupport;
        assert_eq!(support.function_body_kind(), "block");
    }

    #[test]
    fn function_keyword_is_function() {
        let support = LuaSupport;
        assert_eq!(support.function_keyword(), "function");
    }

    #[test]
    fn decision_points_include_if() {
        let support = LuaSupport;
        assert!(
            support
                .decision_point_kinds()
                .iter()
                .any(|k| k.contains("if"))
        );
    }

    #[test]
    fn decision_points_include_while() {
        let support = LuaSupport;
        assert!(
            support
                .decision_point_kinds()
                .iter()
                .any(|k| k.contains("while"))
        );
    }

    #[test]
    fn decision_points_include_for() {
        let support = LuaSupport;
        assert!(
            support
                .decision_point_kinds()
                .iter()
                .any(|k| k.contains("for"))
        );
    }

    #[test]
    fn node_kind_unknown_for_root() {
        let support = LuaSupport;
        let tree = parse(&support, "x = 1");
        // root node is "chunk" — should be Unknown
        assert_eq!(tree.root_node().kind(), "chunk");
        assert_eq!(support.node_kind(&tree.root_node()), NodeKind::Unknown);
    }

    #[test]
    fn extractable_node_kinds_nonempty() {
        let support = LuaSupport;
        assert!(!support.extractable_node_kinds().is_empty());
    }

    #[test]
    fn node_kind_class_empty_table() {
        let support = LuaSupport;
        let code = "Animal = {}";
        let tree = parse(&support, code);
        let node = tree.root_node().named_child(0).unwrap();
        assert_eq!(node.kind(), "assignment_statement");
        assert_eq!(
            support.node_kind_with_source(&node, code.as_bytes()),
            NodeKind::Class,
            "Animal = {{}} should be NodeKind::Class"
        );
    }

    #[test]
    fn node_kind_class_table_with_fields() {
        let support = LuaSupport;
        let code = "Person = {name = '', age = 0}";
        let tree = parse(&support, code);
        let node = tree.root_node().named_child(0).unwrap();
        assert_eq!(node.kind(), "assignment_statement");
        assert_eq!(
            support.node_kind_with_source(&node, code.as_bytes()),
            NodeKind::Class,
            "Person = {{name = '', age = 0}} should be NodeKind::Class"
        );
    }

    #[test]
    fn node_kind_regular_assignment_stays_variable() {
        let support = LuaSupport;
        let code = "x = 42";
        let tree = parse(&support, code);
        let node = tree.root_node().named_child(0).unwrap();
        assert_eq!(node.kind(), "assignment_statement");
        assert_eq!(
            support.node_kind_with_source(&node, code.as_bytes()),
            NodeKind::Variable,
            "x = 42 should remain NodeKind::Variable"
        );
    }

    #[test]
    fn node_kind_string_assignment_stays_variable() {
        let support = LuaSupport;
        let code = "greeting = 'hello'";
        let tree = parse(&support, code);
        let node = tree.root_node().named_child(0).unwrap();
        assert_eq!(node.kind(), "assignment_statement");
        assert_eq!(
            support.node_kind_with_source(&node, code.as_bytes()),
            NodeKind::Variable,
            "greeting = 'hello' should remain NodeKind::Variable"
        );
    }

    #[test]
    fn skeleton_query_compiles() {
        let support = LuaSupport;
        let src = support.skeleton_query();
        assert!(!src.is_empty());
        tree_sitter::Query::new(&support.ts_language(), src).unwrap();
    }

    #[test]
    fn symbols_query_compiles() {
        let support = LuaSupport;
        let src = support.symbols_query();
        assert!(!src.is_empty());
        tree_sitter::Query::new(&support.ts_language(), src).unwrap();
    }

    #[test]
    fn references_query_compiles() {
        let support = LuaSupport;
        let src = support.references_query();
        assert!(!src.is_empty());
        tree_sitter::Query::new(&support.ts_language(), src).unwrap();
    }

    // ---- moat coverage hooks ----

    #[test]
    fn moat_call_node_is_function_call() {
        let s = LuaSupport;
        assert_eq!(s.call_node_kinds(), &["function_call"]);
        // verify the kind actually exists in a parsed tree
        let tree = parse(&s, "print('hi')");
        let node = tree.root_node().named_child(0).unwrap();
        assert_eq!(node.kind(), "function_call");
    }

    #[test]
    fn moat_effect_models_blocking() {
        let s = LuaSupport;
        assert!(s.blocking_call_names().contains(&"execute"));
        assert!(s.effect_modeled_kinds().contains("block"));
    }

    #[test]
    fn moat_data_flow_binds_declarations() {
        let s = LuaSupport;
        assert!(s.binding_node_kinds().contains(&"variable_declaration"));
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
    }

    #[test]
    fn moat_unsafe_models_eval() {
        let s = LuaSupport;
        assert!(s.eval_call_names().contains(&"load"));
        assert!(s.eval_call_names().contains(&"loadstring"));
        assert!(s.eval_call_names().contains(&"dofile"));
        assert!(s.unsafe_modeled_kinds().contains("eval"));
    }

    #[test]
    fn moat_secret_models_env_read() {
        let s = LuaSupport;
        assert!(s.env_read_names().contains(&"getenv"));
        assert!(s.secret_modeled_kinds().contains("env_secret_read"));
    }

    #[test]
    fn moat_taint_models_sources_and_command_sink() {
        let s = LuaSupport;
        assert!(s.taint_source_names().contains(&"getenv"));
        assert!(s.command_exec_names().contains(&"execute"));
        assert!(s.command_exec_names().contains(&"popen"));
        assert!(s.taint_modeled_kinds().contains("taint"));
    }
}
