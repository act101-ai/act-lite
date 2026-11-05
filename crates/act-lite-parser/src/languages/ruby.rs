//! Ruby language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Ruby in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("ruby"),
        display_name: "Ruby",
        extensions: &["rb", "rake"],
        category: LanguageCategory::Dynamic,
    }
}

/// Ruby language support implementation.
#[derive(Default)]
pub struct RubySupport;

impl LanguageSupport for RubySupport {
    fn grammar(&self) -> Grammar {
        Grammar::Ruby
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_ruby::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Methods
            "method" => NodeKind::Function,
            "singleton_method" => NodeKind::Function,
            "lambda" => NodeKind::Function,

            // Classes and modules
            "class" => NodeKind::Class,
            "singleton_class" => NodeKind::Class,
            "module" => NodeKind::Module,

            // Blocks
            "block" | "do_block" => NodeKind::Block,
            "begin" => NodeKind::Block,

            // Variables and parameters
            "identifier" => NodeKind::Identifier,
            "instance_variable" => NodeKind::Property,
            "class_variable" => NodeKind::Property,
            "global_variable" => NodeKind::Variable,
            "constant" => NodeKind::Constant,

            // Parameters
            "method_parameters" | "block_parameters" => NodeKind::Parameter,
            "optional_parameter" | "keyword_parameter" => NodeKind::Parameter,
            "splat_parameter" | "hash_splat_parameter" => NodeKind::Parameter,
            "block_parameter" => NodeKind::Parameter,

            // Calls
            "call" | "method_call" => NodeKind::Call,
            "element_reference" => NodeKind::Call,
            "yield" => NodeKind::Call,

            // Imports
            "require" => NodeKind::Import,

            // Control flow
            "if" | "unless" | "case" | "when" => NodeKind::Block,
            "for" | "while" | "until" => NodeKind::Block,
            "in_clause" => NodeKind::Block,

            // Error handling
            "rescue" | "ensure" => NodeKind::Block,
            "raise" => NodeKind::Call,

            // Comments
            "comment" => NodeKind::Comment,

            // Symbols and strings
            "symbol" | "string" | "heredoc_body" => NodeKind::Unknown,

            // Assignments
            "assignment" => self.classify_assignment(node),

            // Unknown
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rb", "rake", "gemspec", "ru", "erb"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/ruby/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/ruby/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/ruby/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("#"),
            block_start: Some("=begin"),
            block_end: Some("=end"),
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &["method", "singleton_method", "lambda"]
    }

    fn function_body_kind(&self) -> &'static str {
        "body_statement"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if", "unless", "case", "when", "while", "until", "for", "rescue",
        ]
    }

    fn function_keyword(&self) -> &'static str {
        "def"
    }

    fn logical_operator_info(&self) -> Option<(&'static str, &'static [&'static str])> {
        // Ruby uses 'binary' node kind (not 'binary_expression').
        // Includes both symbolic (&&, ||) and keyword (and, or) forms.
        Some(("binary", &["&&", "||", "and", "or"]))
    }

    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[
            "block",
            "do_block",
            "lambda",
            "method",
            "call",
            // Statement-level nodes for top-level extraction
            "assignment",
            "binary",
            "method_call",
            "if",
            "unless",
            "while",
            "until",
            "for",
            "case",
            "begin",
            "return",
        ]
    }

    // ---- moat coverage hooks (Ruby) ----

    /// Ruby invocations: `call` (`foo`, `recv.meth`), `method_call`, and
    /// `element_reference` (`a[i]`). Overrides the default to add `method_call`.
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call", "method_call", "element_reference"]
    }

    /// `x = ...` binds names (LHS classified by `classify_assignment`).
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &["assignment", "operator_assignment"]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    /// Allocation: Ruby objects are created via `Klass.new`; `Array`/`Hash`/
    /// `String` literals also allocate but `new` is the universal constructor.
    fn allocation_call_names(&self) -> &'static [&'static str] {
        &["new", "allocate"]
    }

    /// Blocking calls: `sleep`, `gets`/`read` (stdin), `Mutex#lock`, `join`.
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["sleep", "gets", "read", "lock", "join"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("allocate")
            .with("block")
    }

    /// Dynamic evaluation: `eval`, `instance_eval`, `class_eval`, `module_eval`.
    fn eval_call_names(&self) -> &'static [&'static str] {
        &["eval", "instance_eval", "class_eval", "module_eval"]
    }

    /// Raw SQL sinks (ActiveRecord): `execute`, `find_by_sql`, `where` with raw
    /// strings, `connection.execute`.
    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &["execute", "find_by_sql"]
    }

    /// Reflective invocation: `send`, `public_send`, `__send__`, `const_get`,
    /// `method`, `define_method`.
    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        &[
            "send",
            "public_send",
            "__send__",
            "const_get",
            "define_method",
        ]
    }

    /// Unsafe deserialization: `Marshal.load`, `YAML.load`, `YAML.unsafe_load`.
    fn unsafe_deserialize_names(&self) -> &'static [&'static str] {
        &["Marshal.load", "YAML.load", "unsafe_load", "load"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("eval")
            .with("raw_sql")
            .with("reflection_invoke")
            .with("unsafe_deserialize")
    }

    /// Crypto APIs: `OpenSSL::HMAC`, `Digest`, `sign`, `encrypt`.
    fn crypto_api_names(&self) -> &'static [&'static str] {
        &["HMAC", "sign", "encrypt", "Digest", "hexdigest"]
    }

    /// Environment-secret reads: `ENV[...]`, `ENV.fetch`.
    fn env_read_names(&self) -> &'static [&'static str] {
        &["ENV", "fetch"]
    }

    fn secret_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("env_secret_read")
            .with("signing_key")
    }

    /// Taint sources: request `params`, `ENV`, stdin `gets`/`read`, `ARGV`.
    fn taint_source_names(&self) -> &'static [&'static str] {
        &["params", "ENV", "gets", "read", "ARGV"]
    }

    /// Command-injection sinks: `system`, `exec`, `spawn`, `` ` `` (backticks),
    /// `%x{}`, `IO.popen`.
    fn command_exec_names(&self) -> &'static [&'static str] {
        &["system", "exec", "spawn", "popen"]
    }

    /// Filesystem sinks: `File.open`, `File.read`, `File.write`, `IO.read`.
    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["open", "read", "write"]
    }

    /// Sanitizers: `CGI.escape`, `Shellwords.escape`, `sanitize`.
    fn sanitizer_names(&self) -> &'static [&'static str] {
        &["escape", "sanitize", "shellescape"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

impl RubySupport {
    /// Classify an assignment based on the left-hand side.
    fn classify_assignment(&self, node: &tree_sitter::Node) -> NodeKind {
        if let Some(left) = node.child_by_field_name("left") {
            match left.kind() {
                "constant" => NodeKind::Constant,
                "instance_variable" => NodeKind::Property,
                "class_variable" => NodeKind::Property,
                "global_variable" => NodeKind::Variable,
                _ => NodeKind::Variable,
            }
        } else {
            NodeKind::Variable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};

    crate::language_support_tests!(
        RubySupport,
        Grammar::Ruby,
        &["rb", "rake", "gemspec", "ru", "erb"]
    );

    #[test]
    fn node_kind_method() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "def foo; end");
        let method_node = find_node_by_kind(tree.root_node(), "method").unwrap();
        assert_eq!(support.node_kind(&method_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_singleton_method() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "def self.bar; end");
        let method_node = find_node_by_kind(tree.root_node(), "singleton_method").unwrap();
        assert_eq!(support.node_kind(&method_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_class() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "class Foo; end");
        let class_node = find_node_by_kind(tree.root_node(), "class").unwrap();
        assert_eq!(support.node_kind(&class_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_module() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "module Bar; end");
        let module_node = find_node_by_kind(tree.root_node(), "module").unwrap();
        assert_eq!(support.node_kind(&module_node), NodeKind::Module);
    }

    #[test]
    fn node_kind_block() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "[1,2,3].each { |x| puts x }");
        let block_node = find_node_by_kind(tree.root_node(), "block").unwrap();
        assert_eq!(support.node_kind(&block_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_do_block() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "[1,2,3].each do |x|\n  puts x\nend");
        let block_node = find_node_by_kind(tree.root_node(), "do_block").unwrap();
        assert_eq!(support.node_kind(&block_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_lambda() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "f = ->(x) { x * 2 }");
        let lambda_node = find_node_by_kind(tree.root_node(), "lambda").unwrap();
        assert_eq!(support.node_kind(&lambda_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_constant() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "MAX_SIZE = 100");
        let constant_node = find_node_by_kind(tree.root_node(), "constant").unwrap();
        assert_eq!(support.node_kind(&constant_node), NodeKind::Constant);
    }

    #[test]
    fn node_kind_instance_variable() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "def init; @foo = 1; end");
        let ivar_node = find_node_by_kind(tree.root_node(), "instance_variable").unwrap();
        assert_eq!(support.node_kind(&ivar_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_call() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "foo.bar(1, 2)");
        let call_node = find_node_by_kind(tree.root_node(), "call").unwrap();
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_comment() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "# This is a comment\nx = 1");
        let comment_node = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_require() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "require 'json'");
        if let Some(require_node) = find_node_by_kind(tree.root_node(), "require") {
            assert_eq!(support.node_kind(&require_node), NodeKind::Import);
        }
    }

    #[test]
    fn node_kind_global_variable() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "$foo = 1");
        if let Some(gvar_node) = find_node_by_kind(tree.root_node(), "global_variable") {
            assert_eq!(support.node_kind(&gvar_node), NodeKind::Variable);
        }
    }

    #[test]
    fn node_kind_class_variable() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "@@foo = 1");
        if let Some(cvar_node) = find_node_by_kind(tree.root_node(), "class_variable") {
            assert_eq!(support.node_kind(&cvar_node), NodeKind::Property);
        }
    }

    #[test]
    fn node_kind_identifier() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "x = 1");
        if let Some(id_node) = find_node_by_kind(tree.root_node(), "identifier") {
            assert_eq!(support.node_kind(&id_node), NodeKind::Identifier);
        }
    }

    #[test]
    fn node_kind_symbol() {
        let support = RubySupport;
        let tree = parse_with_support(&support, ":foo");
        if let Some(sym_node) = find_node_by_kind(tree.root_node(), "symbol") {
            assert_eq!(support.node_kind(&sym_node), NodeKind::Unknown);
        }
    }

    #[test]
    fn node_kind_string() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "\"hello\"");
        let str_node = find_node_by_kind(tree.root_node(), "string").unwrap();
        assert_eq!(support.node_kind(&str_node), NodeKind::Unknown);
    }

    #[test]
    fn node_kind_if() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "if true\n  x = 1\nend");
        let if_node = find_node_by_kind(tree.root_node(), "if").unwrap();
        assert_eq!(support.node_kind(&if_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_unless() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "x = 1 unless false");
        let unless_node = find_node_by_kind(tree.root_node(), "unless").unwrap();
        assert_eq!(support.node_kind(&unless_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_case() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "case x\nwhen 1\n  puts 1\nend");
        let case_node = find_node_by_kind(tree.root_node(), "case").unwrap();
        assert_eq!(support.node_kind(&case_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_when() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "case x\nwhen 1\n  puts 1\nend");
        let when_node = find_node_by_kind(tree.root_node(), "when").unwrap();
        assert_eq!(support.node_kind(&when_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_for() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "for x in [1,2,3]\n  puts x\nend");
        let for_node = find_node_by_kind(tree.root_node(), "for").unwrap();
        assert_eq!(support.node_kind(&for_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_while() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "while true\n  break\nend");
        let while_node = find_node_by_kind(tree.root_node(), "while").unwrap();
        assert_eq!(support.node_kind(&while_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_begin() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "begin\n  foo\nrescue\n  bar\nend");
        let begin_node = find_node_by_kind(tree.root_node(), "begin").unwrap();
        assert_eq!(support.node_kind(&begin_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_rescue() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "begin\n  foo\nrescue\n  bar\nend");
        let rescue_node = find_node_by_kind(tree.root_node(), "rescue").unwrap();
        assert_eq!(support.node_kind(&rescue_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_ensure() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "begin\n  foo\nensure\n  bar\nend");
        let ensure_node = find_node_by_kind(tree.root_node(), "ensure").unwrap();
        assert_eq!(support.node_kind(&ensure_node), NodeKind::Block);
    }

    #[test]
    fn node_kind_raise() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "raise StandardError, 'error'");
        if let Some(raise_node) = find_node_by_kind(tree.root_node(), "raise") {
            assert_eq!(support.node_kind(&raise_node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_method_call() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "foo.bar");
        if let Some(call_node) = find_node_by_kind(tree.root_node(), "method_call") {
            assert_eq!(support.node_kind(&call_node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_element_reference() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "arr[0]");
        if let Some(elem_node) = find_node_by_kind(tree.root_node(), "element_reference") {
            assert_eq!(support.node_kind(&elem_node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_yield() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "def foo\n  yield 1\nend");
        if let Some(yield_node) = find_node_by_kind(tree.root_node(), "yield") {
            assert_eq!(support.node_kind(&yield_node), NodeKind::Call);
        }
    }

    #[test]
    fn node_kind_assignment_constant() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "FOO = 1");
        let assign_node = find_node_by_kind(tree.root_node(), "assignment").unwrap();
        assert_eq!(support.node_kind(&assign_node), NodeKind::Constant);
    }

    #[test]
    fn node_kind_assignment_instance_variable() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "@foo = 1");
        let assign_node = find_node_by_kind(tree.root_node(), "assignment").unwrap();
        assert_eq!(support.node_kind(&assign_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_assignment_class_variable() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "@@foo = 1");
        let assign_node = find_node_by_kind(tree.root_node(), "assignment").unwrap();
        assert_eq!(support.node_kind(&assign_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_assignment_global_variable() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "$foo = 1");
        let assign_node = find_node_by_kind(tree.root_node(), "assignment").unwrap();
        assert_eq!(support.node_kind(&assign_node), NodeKind::Variable);
    }

    #[test]
    fn node_kind_singleton_class() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "class << self\n  def foo\n  end\nend");
        let sclass_node = find_node_by_kind(tree.root_node(), "singleton_class").unwrap();
        assert_eq!(support.node_kind(&sclass_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_method_parameters() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "def foo(x, y)\nend");
        let params_node = find_node_by_kind(tree.root_node(), "method_parameters").unwrap();
        assert_eq!(support.node_kind(&params_node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_block_parameters() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "[1,2,3].each { |x| puts x }");
        let params_node = find_node_by_kind(tree.root_node(), "block_parameters").unwrap();
        assert_eq!(support.node_kind(&params_node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_optional_parameter() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "def foo(x = 1)\nend");
        let opt_param_node = find_node_by_kind(tree.root_node(), "optional_parameter").unwrap();
        assert_eq!(support.node_kind(&opt_param_node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_keyword_parameter() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "def foo(x: 1)\nend");
        let kw_param_node = find_node_by_kind(tree.root_node(), "keyword_parameter").unwrap();
        assert_eq!(support.node_kind(&kw_param_node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_splat_parameter() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "def foo(*args)\nend");
        let splat_node = find_node_by_kind(tree.root_node(), "splat_parameter").unwrap();
        assert_eq!(support.node_kind(&splat_node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_hash_splat_parameter() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "def foo(**opts)\nend");
        let hsplat_node = find_node_by_kind(tree.root_node(), "hash_splat_parameter").unwrap();
        assert_eq!(support.node_kind(&hsplat_node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_block_parameter() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "def foo(&block)\nend");
        let block_param_node = find_node_by_kind(tree.root_node(), "block_parameter").unwrap();
        assert_eq!(support.node_kind(&block_param_node), NodeKind::Parameter);
    }

    #[test]
    fn node_kind_heredoc_body() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "x = <<DOC\nhello\nDOC");
        let heredoc_node = find_node_by_kind(tree.root_node(), "heredoc_body").unwrap();
        assert_eq!(support.node_kind(&heredoc_node), NodeKind::Unknown);
    }

    #[test]
    fn node_kind_in_clause() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "for x in [1,2,3]\n  puts x\nend");
        if let Some(in_node) = find_node_by_kind(tree.root_node(), "in_clause") {
            assert_eq!(support.node_kind(&in_node), NodeKind::Block);
        }
    }

    #[test]
    fn node_kind_until() {
        let support = RubySupport;
        let tree = parse_with_support(&support, "until false\n  break\nend");
        let until_node = find_node_by_kind(tree.root_node(), "until").unwrap();
        assert_eq!(support.node_kind(&until_node), NodeKind::Block);
    }

    #[test]
    fn function_parent_kinds_are_ruby_specific() {
        let support = RubySupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"method"));
        assert!(kinds.contains(&"singleton_method"));
        assert!(kinds.contains(&"lambda"));
        // Must NOT contain JS/TS only values
        assert!(!kinds.contains(&"function_declaration"));
        assert!(!kinds.contains(&"arrow_function"));
        assert!(!kinds.contains(&"method_definition"));
    }

    #[test]
    fn function_body_kind_is_body_statement() {
        let support = RubySupport;
        assert_eq!(support.function_body_kind(), "body_statement");
    }

    #[test]
    fn decision_point_kinds_are_ruby_specific() {
        let support = RubySupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if"));
        assert!(kinds.contains(&"unless"));
        assert!(kinds.contains(&"case"));
        assert!(kinds.contains(&"rescue"));
        // Must NOT contain JS/TS only values
        assert!(!kinds.contains(&"ternary_expression"));
        assert!(!kinds.contains(&"switch_case"));
        assert!(!kinds.contains(&"for_in_statement"));
    }

    #[test]
    fn function_keyword_is_def() {
        let support = RubySupport;
        assert_eq!(support.function_keyword(), "def");
    }

    #[test]
    fn logical_operator_info_covers_ruby_operators() {
        let support = RubySupport;
        let info = support.logical_operator_info();
        assert!(
            info.is_some(),
            "logical_operator_info must return Some for Ruby"
        );
        let (node_kind, operators) = info.unwrap();
        assert_eq!(
            node_kind, "binary",
            "Ruby binary expressions use 'binary' node kind, not 'binary_expression'"
        );
        assert!(operators.contains(&"&&"), "must cover && operator");
        assert!(operators.contains(&"||"), "must cover || operator");
        assert!(
            operators.contains(&"and"),
            "must cover 'and' keyword operator"
        );
        assert!(
            operators.contains(&"or"),
            "must cover 'or' keyword operator"
        );
    }

    #[test]
    fn extractable_node_kinds_covers_ruby_blocks() {
        let support = RubySupport;
        let kinds = support.extractable_node_kinds();
        assert!(
            !kinds.is_empty(),
            "extractable_node_kinds must not be empty for Ruby"
        );
        assert!(kinds.contains(&"block"), "must include 'block'");
        assert!(kinds.contains(&"do_block"), "must include 'do_block'");
        assert!(kinds.contains(&"lambda"), "must include 'lambda'");
        assert!(kinds.contains(&"method"), "must include 'method'");
    }

    // ---- moat coverage hooks ----

    #[test]
    fn moat_call_node_overrides_default_with_method_call() {
        let s = RubySupport;
        assert!(s.call_node_kinds().contains(&"call"));
        assert!(s.call_node_kinds().contains(&"method_call"));
        // `method_call` actually appears in a parsed tree.
        let tree = parse_with_support(&s, "foo.bar(1)");
        assert!(
            find_node_by_kind(tree.root_node(), "call").is_some()
                || find_node_by_kind(tree.root_node(), "method_call").is_some()
        );
    }

    #[test]
    fn moat_effect_models_allocate_and_blocking() {
        let s = RubySupport;
        assert!(s.allocation_call_names().contains(&"new"));
        assert!(s.blocking_call_names().contains(&"sleep"));
        assert!(s.effect_modeled_kinds().contains("allocate"));
        assert!(s.effect_modeled_kinds().contains("block"));
    }

    #[test]
    fn moat_data_flow_binds_assignments() {
        let s = RubySupport;
        assert!(s.binding_node_kinds().contains(&"assignment"));
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
        let tree = parse_with_support(&s, "x = 1");
        assert!(find_node_by_kind(tree.root_node(), "assignment").is_some());
    }

    #[test]
    fn moat_unsafe_models_eval_reflection_and_deserialize() {
        let s = RubySupport;
        assert!(s.eval_call_names().contains(&"instance_eval"));
        assert!(s.reflection_invoke_names().contains(&"send"));
        assert!(s.reflection_invoke_names().contains(&"const_get"));
        assert!(s.unsafe_deserialize_names().contains(&"Marshal.load"));
        assert!(s.unsafe_modeled_kinds().contains("eval"));
        assert!(s.unsafe_modeled_kinds().contains("reflection_invoke"));
        assert!(s.unsafe_modeled_kinds().contains("unsafe_deserialize"));
    }

    #[test]
    fn moat_secret_models_env_and_crypto() {
        let s = RubySupport;
        assert!(s.env_read_names().contains(&"ENV"));
        assert!(s.crypto_api_names().contains(&"HMAC"));
        assert!(s.secret_modeled_kinds().contains("env_secret_read"));
        assert!(s.secret_modeled_kinds().contains("signing_key"));
    }

    #[test]
    fn moat_taint_models_sources_and_command_sink() {
        let s = RubySupport;
        assert!(s.taint_source_names().contains(&"params"));
        assert!(s.taint_source_names().contains(&"ENV"));
        assert!(s.command_exec_names().contains(&"system"));
        assert!(s.command_exec_names().contains(&"exec"));
        assert!(s.taint_modeled_kinds().contains("taint"));
    }
}
