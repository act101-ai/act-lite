//! Scala language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register Scala in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("scala"),
        display_name: "Scala",
        extensions: &["scala", "sc"],
        category: LanguageCategory::Managed,
    }
}

/// Scala language support implementation.
#[derive(Default)]
pub struct ScalaSupport;

impl LanguageSupport for ScalaSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Scala
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_scala::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // Functions
            "function_definition" => NodeKind::Function,
            "lambda_expression" => NodeKind::Function,
            // Abstract method declarations in traits (no body)
            "function_declaration" => NodeKind::Function,

            // Classes and types
            "class_definition" => NodeKind::Class,
            "object_definition" => NodeKind::Object,
            "trait_definition" => NodeKind::Interface,
            "type_definition" => NodeKind::TypeAlias,
            // Scala 3 enums
            "enum_definition" => NodeKind::Enum,
            "simple_enum_case" | "full_enum_case" => NodeKind::Property,

            // Members
            "val_definition" => self.classify_val(node),
            "var_definition" => self.classify_var(node),

            // Parameters
            "parameter" | "class_parameter" => NodeKind::Parameter,
            "type_parameter" => NodeKind::Parameter,

            // Imports
            "import_declaration" => NodeKind::Import,

            // Blocks and control flow
            "block" | "template_body" => NodeKind::Block,
            "if_expression" | "match_expression" => NodeKind::Block,
            "for_expression" | "while_expression" => NodeKind::Block,
            "try_expression" | "catch_clause" => NodeKind::Block,
            "case_clause" => NodeKind::Block,

            // Calls
            "call_expression" => NodeKind::Call,

            // Identifiers
            "identifier" | "type_identifier" => NodeKind::Identifier,

            // Comments
            "comment" => NodeKind::Comment,

            // Annotations
            "annotation" => NodeKind::Decorator,

            // Types
            "generic_type" => NodeKind::Type,
            "function_type" => NodeKind::Type,

            // Package
            "package_clause" => NodeKind::Module,

            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["scala", "sc"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/scala/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/scala/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/scala/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &["function_definition", "lambda_expression"]
    }

    fn function_body_kind(&self) -> &'static str {
        "block"
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[
            "if_expression",
            "match_expression",
            "case_clause",
            "for_expression",
            "while_expression",
            "catch_clause",
        ]
    }

    // --- call_detection ---
    /// Scala's only call node is `call_expression`; `new T()` is `instance_expression`.
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "instance_expression"]
    }

    // --- effect_summary ---
    /// `throw e` parses as `throw_expression`.
    fn raise_node_kinds(&self) -> &'static [&'static str] {
        &["throw_expression"]
    }

    /// `new T(...)` allocates via `instance_expression`.
    fn allocation_node_kinds(&self) -> &'static [&'static str] {
        &["instance_expression"]
    }

    /// Future/blocking interop.
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &["Await", "result", "sleep", "join", "get"]
    }

    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("read")
            .with("write")
            .with("raise")
            .with("allocate")
    }

    // --- data_flow ---
    /// `val`/`var` bind via `val_definition` / `var_definition`.
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &[
            "val_definition",
            "var_definition",
            "val_declaration",
            "var_declaration",
        ]
    }

    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("def_use")
    }

    // --- unsafe_surface ---
    /// JSR-223 scripting on the JVM.
    fn eval_call_names(&self) -> &'static [&'static str] {
        &["eval", "evalScript"]
    }

    /// JNI / native interop.
    fn ffi_markers(&self) -> &'static [&'static str] {
        &["loadLibrary", "load", "@native"]
    }

    /// JDBC / Slick / Anorm raw SQL sinks.
    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &["execute", "executeQuery", "executeUpdate", "sql", "run"]
    }

    /// Scala/Java reflection.
    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        &["invoke", "forName", "newInstance", "getMethod"]
    }

    /// JVM deserialization.
    fn unsafe_deserialize_names(&self) -> &'static [&'static str] {
        &["readObject", "readUnshared"]
    }

    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
            .with("eval")
            .with("raw_sql")
            .with("ffi")
            .with("reflection_invoke")
            .with("unsafe_deserialize")
    }

    // --- secret_surface ---
    /// JCA crypto APIs.
    fn crypto_api_names(&self) -> &'static [&'static str] {
        &[
            "getInstance",
            "MessageDigest",
            "Cipher",
            "Mac",
            "SecretKeySpec",
        ]
    }

    /// `sys.env` / `System.getenv`.
    fn env_read_names(&self) -> &'static [&'static str] {
        &["getenv", "env", "getProperty"]
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
    /// Play/Akka HTTP + IO sources.
    fn taint_source_names(&self) -> &'static [&'static str] {
        &[
            "getParameter",
            "getHeader",
            "getQueryString",
            "readLine",
            "getenv",
        ]
    }

    /// OS command execution (`scala.sys.process`).
    fn command_exec_names(&self) -> &'static [&'static str] {
        &["exec", "run", "ProcessBuilder", "lineStream"]
    }

    /// Filesystem sinks.
    fn fs_sink_names(&self) -> &'static [&'static str] {
        &["File", "Source", "Paths", "fromFile"]
    }

    /// Escaping/sanitizers.
    fn sanitizer_names(&self) -> &'static [&'static str] {
        &["escapeHtml", "encode", "sanitize"]
    }

    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("taint")
    }
}

impl ScalaSupport {
    fn classify_val(&self, node: &tree_sitter::Node) -> NodeKind {
        if let Some(parent) = node.parent() {
            match parent.kind() {
                "template_body" | "class_definition" | "object_definition" | "trait_definition" => {
                    return NodeKind::Property;
                }
                "block" | "function_definition" | "for_expression" => return NodeKind::Variable,
                _ => {}
            }
        }
        NodeKind::Variable
    }

    fn classify_var(&self, node: &tree_sitter::Node) -> NodeKind {
        if let Some(parent) = node.parent() {
            match parent.kind() {
                "template_body" | "class_definition" | "object_definition" | "trait_definition" => {
                    return NodeKind::Property;
                }
                _ => {}
            }
        }
        NodeKind::Variable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};

    crate::language_support_tests!(ScalaSupport, Grammar::Scala, &["scala", "sc"]);

    #[test]
    fn node_kind_function_definition() {
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "def foo(): Unit = {}");
        let func_node = find_node_by_kind(tree.root_node(), "function_definition").unwrap();
        assert_eq!(support.node_kind(&func_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_class_definition() {
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "class Foo {}");
        let class_node = find_node_by_kind(tree.root_node(), "class_definition").unwrap();
        assert_eq!(support.node_kind(&class_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_trait_definition() {
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "trait Repository[T] {}");
        let trait_node = find_node_by_kind(tree.root_node(), "trait_definition").unwrap();
        assert_eq!(support.node_kind(&trait_node), NodeKind::Interface);
    }

    #[test]
    fn node_kind_object_definition() {
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "object Foo {}");
        let object_node = find_node_by_kind(tree.root_node(), "object_definition").unwrap();
        assert_eq!(support.node_kind(&object_node), NodeKind::Object);
    }

    #[test]
    fn node_kind_type_definition() {
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "type UserId = Long");
        let type_node = find_node_by_kind(tree.root_node(), "type_definition").unwrap();
        assert_eq!(support.node_kind(&type_node), NodeKind::TypeAlias);
    }

    #[test]
    fn node_kind_import_declaration() {
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "import scala.concurrent.Future");
        let import_node = find_node_by_kind(tree.root_node(), "import_declaration").unwrap();
        assert_eq!(support.node_kind(&import_node), NodeKind::Import);
    }

    #[test]
    fn node_kind_call_expression() {
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "def main(): Unit = foo()");
        let call_node = find_node_by_kind(tree.root_node(), "call_expression").unwrap();
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_comment() {
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "// comment\nval x = 1");
        let comment_node = find_node_by_kind(tree.root_node(), "comment").unwrap();
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn function_parent_kinds_are_scala_specific() {
        let support = ScalaSupport;
        let kinds = support.function_parent_kinds();
        assert!(kinds.contains(&"function_definition"));
        assert!(kinds.contains(&"lambda_expression"));
        // Must NOT contain JS/TS only values
        assert!(!kinds.contains(&"function_declaration"));
        assert!(!kinds.contains(&"arrow_function"));
        assert!(!kinds.contains(&"method_definition"));
    }

    #[test]
    fn function_body_kind_is_block() {
        let support = ScalaSupport;
        assert_eq!(support.function_body_kind(), "block");
    }

    #[test]
    fn decision_point_kinds_are_scala_specific() {
        let support = ScalaSupport;
        let kinds = support.decision_point_kinds();
        assert!(kinds.contains(&"if_expression"));
        assert!(kinds.contains(&"match_expression"));
        assert!(kinds.contains(&"case_clause"));
        // Must NOT contain JS/TS only values
        assert!(!kinds.contains(&"ternary_expression"));
        assert!(!kinds.contains(&"switch_case"));
        assert!(!kinds.contains(&"for_in_statement"));
    }

    #[test]
    fn node_kind_scala3_enum_definition() {
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "enum Color:\n  case Red\n  case Green");
        let enum_node = find_node_by_kind(tree.root_node(), "enum_definition").unwrap();
        assert_eq!(support.node_kind(&enum_node), NodeKind::Enum);
    }

    #[test]
    fn node_kind_simple_enum_case() {
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "enum Color:\n  case Red\n  case Green");
        let case_node = find_node_by_kind(tree.root_node(), "simple_enum_case").unwrap();
        assert_eq!(support.node_kind(&case_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_lazy_val_definition() {
        // tree-sitter-scala represents `lazy val` as a `val_definition` with a `modifiers`
        // child containing `lazy` — there is no separate `lazy_val_definition` node kind.
        // The existing `val_definition` → `classify_val` mapping handles this correctly.
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "class Foo {\n  lazy val x: Int = compute()\n}");
        // The lazy val is parsed as val_definition inside a template_body → Property
        let val_node = find_node_by_kind(tree.root_node(), "val_definition").unwrap();
        assert_eq!(support.node_kind(&val_node), NodeKind::Property);
    }

    #[test]
    fn node_kind_function_declaration_in_trait() {
        let support = ScalaSupport;
        let tree = parse_with_support(&support, "trait Foo {\n  def bar(): Int\n}");
        let decl_node = find_node_by_kind(tree.root_node(), "function_declaration").unwrap();
        assert_eq!(support.node_kind(&decl_node), NodeKind::Function);
    }

    #[test]
    fn coverage_hooks_scala() {
        let s = ScalaSupport;
        let tree = parse_with_support(
            &s,
            "def f(): Int = { foo(1); throw new RuntimeException(\"e\") }",
        );
        assert!(find_node_by_kind(tree.root_node(), "call_expression").is_some());
        assert!(find_node_by_kind(tree.root_node(), "throw_expression").is_some());
        assert_ne!(s.call_node_kinds(), ["call_expression", "call"]);
        assert!(s.raise_node_kinds().contains(&"throw_expression"));
        assert!(s.allocation_node_kinds().contains(&"instance_expression"));
        assert!(s.effect_modeled_kinds().contains("raise"));
        assert!(s.binding_node_kinds().contains(&"val_definition"));
        assert!(s.data_flow_modeled_kinds().contains("def_use"));
        assert!(s.reflection_invoke_names().contains(&"forName"));
        assert!(s.unsafe_modeled_kinds().contains("raw_sql"));
        assert!(s.crypto_api_names().contains(&"Cipher"));
        assert!(s.secret_modeled_kinds().contains("env_secret_read"));
        assert!(s.command_exec_names().contains(&"exec"));
        assert!(s.taint_modeled_kinds().contains("taint"));
    }
}
