//! The function constructs of each language: which nodes are functions, which
//! nest and decide, and how parameters and names are read.

use tree_sitter::Node;

use super::tokens::{is_comment, is_identifier_token};
use crate::Grammar;

/// One language's constructs, by node kind.
pub(super) struct Row {
    pub functions: &'static [&'static str],
    pub ifs: &'static [&'static str],
    pub else_ifs: &'static [&'static str],
    pub multiways: &'static [&'static str],
    pub arms: &'static [&'static str],
    pub loops: &'static [&'static str],
    pub tries: &'static [&'static str],
    pub handlers: &'static [&'static str],
    pub conditionals: &'static [&'static str],
    /// Node kinds that hold logical operators.
    pub logical: &'static [&'static str],
    /// The operator texts counted in `logical` nodes.
    pub operators: &'static [&'static str],
    /// Other node kinds read to find bodies, parameters, names and arms.
    pub other: &'static [&'static str],
}

const NONE: &[&str] = &[];

const BASH: Row = Row {
    functions: &["function_definition"],
    ifs: &["if_statement"],
    else_ifs: &["elif_clause"],
    multiways: &["case_statement"],
    arms: &["case_item"],
    loops: &["for_statement", "c_style_for_statement", "while_statement"],
    tries: NONE,
    handlers: NONE,
    conditionals: &["ternary_expression"],
    logical: &["list", "binary_expression"],
    operators: &["&&", "||"],
    other: &["else_clause", "word"],
};

const C: Row = Row {
    functions: &["function_definition"],
    ifs: &["if_statement"],
    else_ifs: NONE,
    multiways: &["switch_statement"],
    arms: &["case_statement"],
    loops: &["for_statement", "while_statement", "do_statement"],
    tries: NONE,
    handlers: NONE,
    conditionals: &["conditional_expression"],
    logical: &["binary_expression"],
    operators: &["&&", "||"],
    other: &[
        "parameter_list",
        "parameter_declaration",
        "variadic_parameter",
        "compound_statement",
        "else_clause",
    ],
};

const CPP: Row = Row {
    functions: &["function_definition", "lambda_expression"],
    ifs: &["if_statement"],
    else_ifs: NONE,
    multiways: &["switch_statement"],
    arms: &["case_statement"],
    loops: &[
        "for_statement",
        "for_range_loop",
        "while_statement",
        "do_statement",
    ],
    tries: &["try_statement"],
    handlers: &["catch_clause"],
    conditionals: &["conditional_expression"],
    logical: &["binary_expression"],
    operators: &["&&", "||", "and", "or"],
    other: &[
        "parameter_list",
        "parameter_declaration",
        "optional_parameter_declaration",
        "variadic_parameter_declaration",
        "operator_name",
        "destructor_name",
        "operator_cast",
        "template_argument_list",
        "template_parameter_list",
        "compound_statement",
        "else_clause",
    ],
};

const CSHARP: Row = Row {
    functions: &[
        "method_declaration",
        "constructor_declaration",
        "destructor_declaration",
        "operator_declaration",
        "conversion_operator_declaration",
        "accessor_declaration",
        "local_function_statement",
        "lambda_expression",
        "anonymous_method_expression",
        "property_declaration",
        "indexer_declaration",
    ],
    ifs: &["if_statement"],
    else_ifs: NONE,
    multiways: &["switch_statement", "switch_expression"],
    arms: &["switch_section", "switch_expression_arm"],
    loops: &[
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_statement",
    ],
    tries: &["try_statement"],
    handlers: &["catch_clause"],
    conditionals: &["conditional_expression"],
    logical: &["binary_expression"],
    operators: &["&&", "||", "??"],
    other: &[
        "parameter_list",
        "bracketed_parameter_list",
        "parameter",
        "implicit_parameter",
        "arrow_expression_clause",
        "discard",
    ],
};

const DART: Row = Row {
    functions: &[
        "lambda_expression",
        "class_member_definition",
        "function_expression",
        "getter_signature",
        "setter_signature",
    ],
    ifs: &["if_statement"],
    else_ifs: NONE,
    multiways: &["switch_statement"],
    arms: &["switch_label"],
    loops: &["for_statement", "while_statement", "do_statement"],
    tries: &["try_statement"],
    handlers: &["catch_clause"],
    conditionals: &["conditional_expression"],
    logical: &[
        "logical_and_expression",
        "logical_or_expression",
        "if_null_expression",
    ],
    operators: &["&&", "||", "??"],
    other: &[
        "function_body",
        "formal_parameter_list",
        "formal_parameter",
        "optional_formal_parameters",
        "operator_signature",
        "type_parameters",
        "program",
    ],
};

const ELIXIR: Row = Row {
    functions: &["call", "anonymous_function"],
    ifs: &["call"],
    else_ifs: NONE,
    multiways: &["call"],
    arms: &["stab_clause"],
    loops: &["call"],
    tries: &["call"],
    handlers: &["rescue_block", "catch_block"],
    conditionals: NONE,
    logical: &["binary_operator"],
    operators: &["&&", "||", "and", "or"],
    other: &[
        "do_block",
        "else_block",
        "arguments",
        "keywords",
        "pair",
        "keyword",
        "identifier",
        "boolean",
    ],
};

const GO: Row = Row {
    functions: &["function_declaration", "method_declaration", "func_literal"],
    ifs: &["if_statement"],
    else_ifs: NONE,
    multiways: &[
        "expression_switch_statement",
        "type_switch_statement",
        "select_statement",
    ],
    arms: &["expression_case", "type_case", "communication_case"],
    loops: &["for_statement"],
    tries: NONE,
    handlers: NONE,
    conditionals: NONE,
    logical: &["binary_expression"],
    operators: &["&&", "||"],
    other: &[
        "parameter_list",
        "parameter_declaration",
        "variadic_parameter_declaration",
        "default_case",
    ],
};

const JAVA: Row = Row {
    functions: &[
        "method_declaration",
        "constructor_declaration",
        "compact_constructor_declaration",
        "static_initializer",
        "block",
        "lambda_expression",
    ],
    ifs: &["if_statement"],
    else_ifs: NONE,
    multiways: &["switch_expression"],
    arms: &["switch_label"],
    loops: &[
        "for_statement",
        "enhanced_for_statement",
        "while_statement",
        "do_statement",
    ],
    tries: &["try_statement", "try_with_resources_statement"],
    handlers: &["catch_clause"],
    conditionals: &["ternary_expression"],
    logical: &["binary_expression"],
    operators: &["&&", "||"],
    other: &[
        "class_body",
        "formal_parameters",
        "formal_parameter",
        "spread_parameter",
        "receiver_parameter",
        "inferred_parameters",
    ],
};

const JAVASCRIPT: Row = Row {
    functions: &[
        "function_declaration",
        "function_expression",
        "generator_function_declaration",
        "generator_function",
        "arrow_function",
        "method_definition",
        "class_static_block",
    ],
    ifs: &["if_statement"],
    else_ifs: NONE,
    multiways: &["switch_statement"],
    arms: &["switch_case"],
    loops: &[
        "for_statement",
        "for_in_statement",
        "while_statement",
        "do_statement",
    ],
    tries: &["try_statement"],
    handlers: &["catch_clause"],
    conditionals: &["ternary_expression"],
    logical: &["binary_expression"],
    operators: &["&&", "||", "??"],
    other: &["formal_parameters", "switch_default", "else_clause"],
};

const TYPESCRIPT: Row = Row {
    other: &[
        "formal_parameters",
        "required_parameter",
        "optional_parameter",
        "this",
        "switch_default",
        "else_clause",
    ],
    ..JAVASCRIPT
};

const KOTLIN: Row = Row {
    functions: &[
        "function_declaration",
        "secondary_constructor",
        "anonymous_initializer",
        "getter",
        "setter",
        "anonymous_function",
        "lambda_literal",
    ],
    ifs: &["if_expression"],
    else_ifs: NONE,
    multiways: &["when_expression"],
    arms: &["when_entry"],
    loops: &["for_statement", "while_statement", "do_while_statement"],
    tries: &["try_expression"],
    handlers: &["catch_block"],
    conditionals: NONE,
    logical: &[
        "conjunction_expression",
        "disjunction_expression",
        "elvis_expression",
    ],
    operators: &["&&", "||", "?:"],
    other: &[
        "function_body",
        "function_value_parameters",
        "parameter",
        "parameter_with_optional_type",
        "lambda_parameters",
        "variable_declaration",
        "multi_variable_declaration",
        "type_parameters",
        "when_condition",
        "control_structure_body",
    ],
};

const LUA: Row = Row {
    functions: &["function_declaration", "function_definition"],
    ifs: &["if_statement"],
    else_ifs: &["elseif_statement"],
    multiways: NONE,
    arms: NONE,
    loops: &["for_statement", "while_statement", "repeat_statement"],
    tries: NONE,
    handlers: NONE,
    conditionals: NONE,
    logical: &["binary_expression"],
    operators: &["and", "or"],
    other: &[
        "parameters",
        "identifier",
        "vararg_expression",
        "dot_index_expression",
        "method_index_expression",
    ],
};

const PHP: Row = Row {
    functions: &[
        "function_definition",
        "method_declaration",
        "anonymous_function",
        "arrow_function",
    ],
    ifs: &["if_statement"],
    else_ifs: &["else_if_clause"],
    multiways: &["switch_statement", "match_expression"],
    arms: &["case_statement", "match_conditional_expression"],
    loops: &[
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_statement",
    ],
    tries: &["try_statement"],
    handlers: &["catch_clause"],
    conditionals: &["conditional_expression"],
    logical: &["binary_expression"],
    operators: &["&&", "||", "and", "or", "??"],
    other: &[
        "formal_parameters",
        "simple_parameter",
        "variadic_parameter",
        "property_promotion_parameter",
        "default_statement",
        "match_default_expression",
        "else_clause",
    ],
};

const PYTHON: Row = Row {
    functions: &["function_definition", "lambda"],
    ifs: &["if_statement"],
    else_ifs: &["elif_clause"],
    multiways: &["match_statement"],
    arms: &["case_clause"],
    loops: &["for_statement", "while_statement"],
    tries: &["try_statement"],
    handlers: &["except_clause"],
    conditionals: &["conditional_expression"],
    logical: &["boolean_operator"],
    operators: &["and", "or"],
    other: &[
        "parameters",
        "lambda_parameters",
        "keyword_separator",
        "positional_separator",
        "decorated_definition",
        "class_definition",
        "block",
        "expression_statement",
        "string",
        "concatenated_string",
        "case_pattern",
    ],
};

const RUBY: Row = Row {
    functions: &["method", "singleton_method", "lambda", "block", "do_block"],
    ifs: &["if", "unless", "if_modifier", "unless_modifier"],
    else_ifs: &["elsif"],
    multiways: &["case", "case_match"],
    arms: &["when", "in_clause"],
    loops: &["while", "until", "for", "while_modifier", "until_modifier"],
    tries: &["begin"],
    handlers: &["rescue", "rescue_modifier"],
    conditionals: &["conditional"],
    logical: &["binary"],
    operators: &["&&", "||", "and", "or"],
    other: &[
        "method_parameters",
        "lambda_parameters",
        "block_parameters",
        "hash_splat_nil",
        "else",
    ],
};

const RUST: Row = Row {
    functions: &["function_item", "closure_expression"],
    ifs: &["if_expression"],
    else_ifs: NONE,
    multiways: &["match_expression"],
    arms: &["match_arm"],
    loops: &["for_expression", "while_expression", "loop_expression"],
    tries: NONE,
    handlers: NONE,
    conditionals: NONE,
    logical: &["binary_expression", "let_chain"],
    operators: &["&&", "||"],
    other: &[
        "parameters",
        "parameter",
        "variadic_parameter",
        "self_parameter",
        "closure_parameters",
        "match_pattern",
        "else_clause",
    ],
};

const SCALA: Row = Row {
    functions: &["function_definition", "lambda_expression"],
    ifs: &["if_expression"],
    else_ifs: NONE,
    multiways: &["match_expression"],
    arms: &["case_clause"],
    loops: &["for_expression", "while_expression", "do_while_expression"],
    tries: &["try_expression"],
    handlers: &["catch_clause"],
    conditionals: NONE,
    logical: &["infix_expression"],
    operators: &["&&", "||"],
    other: &[
        "parameters",
        "parameter",
        "bindings",
        "binding",
        "case_block",
        "wildcard",
        "guard",
    ],
};

const SWIFT: Row = Row {
    functions: &[
        "function_declaration",
        "init_declaration",
        "deinit_declaration",
        "subscript_declaration",
        "computed_getter",
        "computed_setter",
        "computed_modify",
        "willset_clause",
        "didset_clause",
        "computed_property",
        "lambda_literal",
    ],
    ifs: &["if_statement", "guard_statement"],
    else_ifs: NONE,
    multiways: &["switch_statement"],
    arms: &["switch_entry"],
    loops: &["for_statement", "while_statement", "repeat_while_statement"],
    tries: &["do_statement"],
    handlers: &["catch_block"],
    conditionals: &["ternary_expression"],
    logical: &[
        "conjunction_expression",
        "disjunction_expression",
        "nil_coalescing_expression",
    ],
    operators: &["&&", "||", "??"],
    other: &[
        "parameter",
        "lambda_function_type",
        "lambda_function_type_parameters",
        "lambda_parameter",
        "default_keyword",
        "property_declaration",
        "function_body",
        "else",
    ],
};

/// The row of a supported grammar; `None` for the `Data` formats.
pub(super) fn row(grammar: Grammar) -> Option<&'static Row> {
    Some(match grammar {
        Grammar::Bash => &BASH,
        Grammar::C => &C,
        Grammar::Cpp => &CPP,
        Grammar::CSharp => &CSHARP,
        Grammar::Dart => &DART,
        Grammar::Elixir => &ELIXIR,
        Grammar::Go => &GO,
        Grammar::Java => &JAVA,
        Grammar::JavaScript | Grammar::Jsx => &JAVASCRIPT,
        Grammar::TypeScript | Grammar::Tsx => &TYPESCRIPT,
        Grammar::Kotlin => &KOTLIN,
        Grammar::Lua => &LUA,
        Grammar::Php => &PHP,
        Grammar::Python => &PYTHON,
        Grammar::Ruby => &RUBY,
        Grammar::Rust => &RUST,
        Grammar::Scala => &SCALA,
        Grammar::Swift => &SWIFT,
        _ => return None,
    })
}

impl Row {
    /// Every named node kind of the row.
    pub fn kinds(&self) -> Vec<&'static str> {
        let mut kinds: Vec<&'static str> = [
            self.functions,
            self.ifs,
            self.else_ifs,
            self.multiways,
            self.arms,
            self.loops,
            self.tries,
            self.handlers,
            self.conditionals,
            self.logical,
            self.other,
        ]
        .concat();
        kinds.sort_unstable();
        kinds.dedup();
        kinds
    }
}

/// What a node is in the metrics.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Other,
    If,
    ElseIf,
    MultiWay,
    Arm,
    DefaultArm,
    Loop,
    Try,
    Handler,
    Conditional,
}

/// The node-local view the rules read.
pub(super) struct Ctx<'s> {
    pub grammar: Grammar,
    pub row: &'static Row,
    pub source: &'s str,
}

fn text<'s>(node: Node, source: &'s str) -> &'s str {
    &source[node.byte_range()]
}

fn children(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

fn named_children(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| !is_comment(c.kind()))
        .collect()
}

fn has_child(node: Node, kind: &str) -> bool {
    children(node).iter().any(|c| c.kind() == kind)
}

fn count_kinds(list: Node, kinds: &[&str]) -> u32 {
    named_children(list)
        .iter()
        .filter(|c| kinds.contains(&c.kind()))
        .count() as u32
}

/// The previous sibling that is not a comment.
fn previous(node: Node) -> Option<Node> {
    let mut prev = node.prev_sibling();
    while let Some(p) = prev {
        if !is_comment(p.kind()) {
            return Some(p);
        }
        prev = p.prev_sibling();
    }
    None
}

/// The first node of `kind` in preorder, not entering nodes of `stop` kinds.
fn find_first<'t>(node: Node<'t>, kind: &str, stop: &[&str]) -> Option<Node<'t>> {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if n.kind() == kind {
            return Some(n);
        }
        if n != node && stop.contains(&n.kind()) {
            continue;
        }
        let mut kids = children(n);
        kids.reverse();
        stack.extend(kids);
    }
    None
}

/// An Elixir call's target identifier text.
fn elixir_target<'s>(node: Node, source: &'s str) -> Option<&'s str> {
    if node.kind() != "call" {
        return None;
    }
    let target = node.child_by_field_name("target")?;
    (target.kind() == "identifier").then(|| text(target, source))
}

const ELIXIR_DEFS: [&str; 4] = ["def", "defp", "defmacro", "defmacrop"];

impl Ctx<'_> {
    /// Whether `node` is a named node of one of `kinds` (Ruby, for one, has
    /// keyword tokens with the same kind names as its constructs).
    fn is(&self, node: Node, kinds: &[&str]) -> bool {
        node.is_named() && kinds.contains(&node.kind())
    }

    /// Whether `node` is a function: a function construct with a body.
    pub fn is_function(&self, node: Node) -> bool {
        if !self.is(node, self.row.functions) {
            return false;
        }
        let kind = node.kind();
        let body = || node.child_by_field_name("body").is_some();
        match self.grammar {
            Grammar::Cpp | Grammar::Go if kind != "lambda_expression" && kind != "func_literal" => {
                body()
            }
            Grammar::CSharp => match kind {
                "property_declaration" | "indexer_declaration" => node
                    .child_by_field_name("value")
                    .is_some_and(|v| v.kind() == "arrow_expression_clause"),
                "lambda_expression" | "anonymous_method_expression" => true,
                _ => body(),
            },
            Grammar::Dart => match kind {
                "class_member_definition" => has_child(node, "function_body"),
                "getter_signature" | "setter_signature" => self.dart_accessor_body(node).is_some(),
                _ => true,
            },
            Grammar::Elixir => match kind {
                "call" => self.is_elixir_def(node),
                _ => true,
            },
            Grammar::Java => match kind {
                "block" => node.parent().is_some_and(|p| p.kind() == "class_body"),
                "method_declaration" => body(),
                _ => true,
            },
            Grammar::Kotlin => match kind {
                "function_declaration" | "getter" | "setter" | "anonymous_function" => {
                    has_child(node, "function_body")
                }
                "secondary_constructor" => has_child(node, "{"),
                _ => true,
            },
            Grammar::Php if kind == "method_declaration" => body(),
            Grammar::Ruby => match kind {
                "block" | "do_block" => node.parent().is_none_or(|p| p.kind() != "lambda"),
                _ => true,
            },
            Grammar::Swift => match kind {
                "function_declaration" | "init_declaration" | "deinit_declaration" => body(),
                "subscript_declaration" => has_child(node, "computed_property"),
                "computed_getter" | "computed_setter" | "computed_modify" | "willset_clause"
                | "didset_clause" => has_child(node, "{"),
                "computed_property" => {
                    node.parent()
                        .is_some_and(|p| p.kind() == "property_declaration")
                        && !children(node).iter().any(|c| {
                            matches!(
                                c.kind(),
                                "computed_getter" | "computed_setter" | "computed_modify"
                            )
                        })
                }
                _ => true,
            },
            _ => true,
        }
    }

    /// The `function_body` that follows a Dart top-level getter or setter signature.
    pub fn dart_accessor_body<'t>(&self, node: Node<'t>) -> Option<Node<'t>> {
        if self.grammar != Grammar::Dart
            || !matches!(node.kind(), "getter_signature" | "setter_signature")
            || node.parent()?.kind() != "program"
        {
            return None;
        }
        let next = node.next_named_sibling()?;
        (next.kind() == "function_body").then_some(next)
    }

    fn is_elixir_def(&self, node: Node) -> bool {
        if !elixir_target(node, self.source).is_some_and(|t| ELIXIR_DEFS.contains(&t)) {
            return false;
        }
        if has_child(node, "do_block") {
            return true;
        }
        children(node)
            .iter()
            .filter(|c| c.kind() == "arguments")
            .flat_map(|a| children(*a))
            .filter(|k| k.kind() == "keywords")
            .flat_map(children)
            .filter_map(|pair| pair.child_by_field_name("key"))
            .any(|key| text(key, self.source).trim_end().starts_with("do:"))
    }

    /// The node's role in the metrics.
    pub fn role(&self, node: Node) -> Role {
        if self.grammar == Grammar::Elixir {
            return self.elixir_role(node);
        }
        let row = self.row;
        if self.is(node, row.ifs) {
            Role::If
        } else if self.is(node, row.else_ifs) {
            Role::ElseIf
        } else if self.is(node, row.multiways) {
            Role::MultiWay
        } else if self.is(node, row.arms) {
            self.arm_role(node)
        } else if self.is(node, row.loops) {
            Role::Loop
        } else if self.is(node, row.tries) {
            Role::Try
        } else if self.is(node, row.handlers) {
            Role::Handler
        } else if self.is(node, row.conditionals) {
            Role::Conditional
        } else {
            Role::Other
        }
    }

    fn elixir_role(&self, node: Node) -> Role {
        match node.kind() {
            "call" => match elixir_target(node, self.source) {
                Some("if" | "unless" | "with") => Role::If,
                Some("case" | "cond" | "receive") => Role::MultiWay,
                Some("for") => Role::Loop,
                Some("try") => Role::Try,
                _ => Role::Other,
            },
            "stab_clause" => {
                let multiway = node
                    .parent()
                    .filter(|p| p.kind() == "do_block")
                    .and_then(|p| p.parent())
                    .and_then(|c| elixir_target(c, self.source));
                let default = match multiway {
                    Some("case" | "receive") => "_",
                    Some("cond") => "true",
                    _ => return Role::Other,
                };
                let left = node
                    .child_by_field_name("left")
                    .map(named_children)
                    .unwrap_or_default();
                if left.len() == 1 && text(left[0], self.source) == default {
                    Role::DefaultArm
                } else {
                    Role::Arm
                }
            }
            "rescue_block" | "catch_block" => Role::Handler,
            _ => Role::Other,
        }
    }

    fn arm_role(&self, node: Node) -> Role {
        let source = self.source;
        let default = match self.grammar {
            Grammar::Bash => {
                let mut cursor = node.walk();
                let values: Vec<Node> = node.children_by_field_name("value", &mut cursor).collect();
                values.len() == 1 && text(values[0], source) == "*"
            }
            Grammar::C | Grammar::Cpp => node.child_by_field_name("value").is_none(),
            Grammar::CSharp => match node.kind() {
                "switch_section" => has_child(node, "default") && !has_child(node, "case"),
                _ => named_children(node)
                    .first()
                    .is_some_and(|p| p.kind() == "discard"),
            },
            Grammar::Dart | Grammar::Java => has_child(node, "default"),
            Grammar::Kotlin => !has_child(node, "when_condition"),
            Grammar::Python => {
                let patterns: Vec<Node> = named_children(node)
                    .into_iter()
                    .filter(|c| c.kind() == "case_pattern")
                    .collect();
                node.child_by_field_name("guard").is_none()
                    && patterns.len() == 1
                    && text(patterns[0], source) == "_"
            }
            Grammar::Rust => node
                .child_by_field_name("pattern")
                .is_some_and(|p| text(p, source) == "_"),
            Grammar::Scala => {
                let in_match = node
                    .parent()
                    .filter(|p| p.kind() == "case_block")
                    .and_then(|p| p.parent())
                    .is_some_and(|m| m.kind() == "match_expression");
                if !in_match {
                    return Role::Other;
                }
                node.child_by_field_name("pattern")
                    .is_some_and(|p| p.kind() == "wildcard")
                    && !has_child(node, "guard")
            }
            Grammar::Swift => has_child(node, "default_keyword"),
            _ => false,
        };
        if default { Role::DefaultArm } else { Role::Arm }
    }

    /// Whether an if construct is the else branch of another if construct.
    pub fn is_else_branch(&self, node: Node) -> bool {
        let Some(parent) = node.parent() else {
            return false;
        };
        let is_if = |n: Node| self.role(n) == Role::If;
        let after_else = |n: Node| previous(n).is_some_and(|p| p.kind() == "else");
        if is_if(parent) {
            return after_else(node);
        }
        if named_children(parent) != [node] {
            return false;
        }
        if self.grammar == Grammar::Elixir {
            return parent.kind() == "else_block"
                && parent
                    .parent()
                    .and_then(|block| block.parent())
                    .is_some_and(is_if);
        }
        let opens_with_else = parent.child(0).is_some_and(|c| c.kind() == "else");
        parent.parent().is_some_and(is_if) && (opens_with_else || after_else(parent))
    }

    /// The number of logical-operator occurrences held directly by `node`.
    pub fn logical_operators(&self, node: Node) -> u32 {
        if !self.is(node, self.row.logical) {
            return 0;
        }
        children(node)
            .iter()
            .filter(|c| {
                c.child_count() == 0 && self.row.operators.contains(&text(**c, self.source))
            })
            .count() as u32
    }

    /// The Python docstring of a function: a string that is the first statement of its body.
    pub fn docstring<'t>(&self, function: Node<'t>) -> Option<Node<'t>> {
        if self.grammar != Grammar::Python || function.kind() != "function_definition" {
            return None;
        }
        let first = *named_children(function.child_by_field_name("body")?).first()?;
        let inner = named_children(first);
        (first.kind() == "expression_statement"
            && inner.len() == 1
            && matches!(inner[0].kind(), "string" | "concatenated_string"))
        .then_some(first)
    }

    /// The parameter count; `None` for Bash.
    pub fn parameters(&self, node: Node) -> Option<u32> {
        let field = |name: &str| node.child_by_field_name(name);
        let kind = node.kind();
        let count = match self.grammar {
            Grammar::Bash => return None,
            Grammar::C | Grammar::Cpp => self.c_parameters(node),
            Grammar::CSharp => match field("parameters") {
                Some(p) if p.kind() == "implicit_parameter" => 1,
                Some(list) => {
                    count_kinds(list, &["parameter"])
                        + children(list)
                            .iter()
                            .filter(|c| c.kind() == "params")
                            .count() as u32
                }
                None => 0,
            },
            Grammar::Dart => {
                let header = if kind == "class_member_definition" {
                    node
                } else if kind == "lambda_expression" {
                    field("parameters").unwrap_or(node)
                } else {
                    node
                };
                match find_first(header, "formal_parameter_list", &["function_body"]) {
                    Some(list) => {
                        count_kinds(list, &["formal_parameter"])
                            + named_children(list)
                                .iter()
                                .filter(|c| c.kind() == "optional_formal_parameters")
                                .map(|o| count_kinds(*o, &["formal_parameter"]))
                                .sum::<u32>()
                    }
                    None => 0,
                }
            }
            Grammar::Elixir => self.elixir_parameters(node),
            Grammar::Go => field("parameters").map_or(0, |list| {
                named_children(list)
                    .iter()
                    .map(|p| match p.kind() {
                        "parameter_declaration" => {
                            let mut cursor = p.walk();
                            (p.children_by_field_name("name", &mut cursor).count() as u32).max(1)
                        }
                        "variadic_parameter_declaration" => 1,
                        _ => 0,
                    })
                    .sum()
            }),
            Grammar::Java => match field("parameters") {
                Some(p) if p.kind() == "identifier" => 1,
                Some(p) if p.kind() == "inferred_parameters" => count_kinds(p, &["identifier"]),
                Some(list) => count_kinds(list, &["formal_parameter", "spread_parameter"]),
                None => 0,
            },
            Grammar::JavaScript | Grammar::Jsx => match (field("parameter"), field("parameters")) {
                (Some(_), _) => 1,
                (None, Some(list)) => named_children(list).len() as u32,
                (None, None) => 0,
            },
            Grammar::TypeScript | Grammar::Tsx => match (field("parameter"), field("parameters")) {
                (Some(_), _) => 1,
                (None, Some(list)) => named_children(list)
                    .iter()
                    .filter(|p| matches!(p.kind(), "required_parameter" | "optional_parameter"))
                    .filter(|p| {
                        p.child_by_field_name("pattern")
                            .is_none_or(|pattern| pattern.kind() != "this")
                    })
                    .count() as u32,
                (None, None) => 0,
            },
            Grammar::Kotlin => match kind {
                "setter" => count_kinds(node, &["parameter_with_optional_type", "parameter"]),
                "lambda_literal" => named_children(node)
                    .iter()
                    .filter(|c| c.kind() == "lambda_parameters")
                    .map(|l| {
                        count_kinds(*l, &["variable_declaration", "multi_variable_declaration"])
                    })
                    .sum(),
                _ => named_children(node)
                    .iter()
                    .filter(|c| c.kind() == "function_value_parameters")
                    .map(|l| count_kinds(*l, &["parameter"]))
                    .sum(),
            },
            Grammar::Lua => field("parameters")
                .map_or(0, |l| count_kinds(l, &["identifier", "vararg_expression"])),
            Grammar::Php => field("parameters").map_or(0, |l| {
                count_kinds(
                    l,
                    &[
                        "simple_parameter",
                        "variadic_parameter",
                        "property_promotion_parameter",
                    ],
                )
            }),
            Grammar::Python => field("parameters").map_or(0, |list| {
                let params: Vec<Node> = named_children(list)
                    .into_iter()
                    .filter(|p| !matches!(p.kind(), "keyword_separator" | "positional_separator"))
                    .collect();
                let receiver = self.is_python_method(node)
                    && params.first().is_some_and(|p| {
                        p.kind() == "identifier" && matches!(text(*p, self.source), "self" | "cls")
                    });
                params.len() as u32 - u32::from(receiver)
            }),
            Grammar::Ruby => field("parameters").map_or(0, |list| {
                let mut cursor = list.walk();
                let mut n = 0;
                for (i, child) in list.children(&mut cursor).enumerate() {
                    let local = list.field_name_for_child(i as u32) == Some("locals");
                    if child.is_named()
                        && !local
                        && !is_comment(child.kind())
                        && child.kind() != "hash_splat_nil"
                    {
                        n += 1;
                    }
                }
                n
            }),
            Grammar::Rust => match field("parameters") {
                Some(list) if kind == "closure_expression" => named_children(list).len() as u32,
                Some(list) => count_kinds(list, &["parameter", "variadic_parameter"]),
                None => 0,
            },
            Grammar::Scala => match kind {
                "lambda_expression" => match field("parameters") {
                    Some(b) if b.kind() == "bindings" => count_kinds(b, &["binding"]),
                    Some(_) => 1,
                    None => 0,
                },
                _ => named_children(node)
                    .iter()
                    .filter(|c| c.kind() == "parameters")
                    .map(|l| count_kinds(*l, &["parameter"]))
                    .sum(),
            },
            Grammar::Swift => match kind {
                "function_declaration" | "init_declaration" | "subscript_declaration" => {
                    count_kinds(node, &["parameter"])
                }
                "lambda_literal" => {
                    find_first(node, "lambda_function_type_parameters", &["statements"])
                        .map_or(0, |l| count_kinds(l, &["lambda_parameter"]))
                }
                _ => 0,
            },
            _ => 0,
        };
        Some(count)
    }

    fn c_parameters(&self, node: Node) -> u32 {
        let Some(declarator) = node.child_by_field_name("declarator") else {
            return 0;
        };
        let Some(list) = find_first(declarator, "parameter_list", &[]) else {
            return 0;
        };
        let params = named_children(list);
        if params.len() == 1
            && params[0].kind() == "parameter_declaration"
            && text(params[0], self.source) == "void"
        {
            return 0;
        }
        let kinds: &[&str] = if self.grammar == Grammar::C {
            &["parameter_declaration", "variadic_parameter"]
        } else {
            &[
                "parameter_declaration",
                "optional_parameter_declaration",
                "variadic_parameter_declaration",
            ]
        };
        let bare_ellipsis = if self.grammar == Grammar::Cpp {
            children(list).iter().filter(|c| c.kind() == "...").count() as u32
        } else {
            0
        };
        count_kinds(list, kinds) + bare_ellipsis
    }

    /// The head of an Elixir `def`: the call or identifier left of any `when`.
    fn elixir_head<'t>(&self, node: Node<'t>) -> Option<Node<'t>> {
        let arguments = children(node)
            .into_iter()
            .find(|c| c.kind() == "arguments")?;
        let head = *named_children(arguments).first()?;
        if head.kind() == "binary_operator"
            && head
                .child_by_field_name("operator")
                .is_some_and(|o| text(o, self.source) == "when")
        {
            return head.child_by_field_name("left");
        }
        Some(head)
    }

    fn elixir_parameters(&self, node: Node) -> u32 {
        if node.kind() == "anonymous_function" {
            return named_children(node)
                .into_iter()
                .find(|c| c.kind() == "stab_clause")
                .and_then(|clause| clause.child_by_field_name("left"))
                .map_or(0, |args| named_children(args).len() as u32);
        }
        match self.elixir_head(node) {
            Some(head) if head.kind() == "call" => children(head)
                .into_iter()
                .find(|c| c.kind() == "arguments")
                .map_or(0, |args| named_children(args).len() as u32),
            _ => 0,
        }
    }

    fn is_python_method(&self, node: Node) -> bool {
        let mut parent = node.parent();
        if parent.is_some_and(|p| p.kind() == "decorated_definition") {
            parent = parent.and_then(|p| p.parent());
        }
        parent.is_some_and(|p| p.kind() == "block")
            && parent
                .and_then(|p| p.parent())
                .is_some_and(|c| c.kind() == "class_definition")
    }

    /// The function's name, `None` when it has none.
    pub fn name(&self, node: Node) -> Option<String> {
        let source = self.source;
        let kind = node.kind();
        let keyword = |k: &str| Some(k.to_string());
        match self.grammar {
            Grammar::Bash => node
                .child_by_field_name("name")
                .map(|n| text(n, source).to_string()),
            Grammar::C | Grammar::Cpp => {
                if kind == "lambda_expression" {
                    return None;
                }
                let declarator = node.child_by_field_name("declarator")?;
                for special in ["operator_name", "destructor_name", "operator_cast"] {
                    if let Some(n) = find_first(declarator, special, &["parameter_list"]) {
                        return Some(text(n, source).to_string());
                    }
                }
                self.last_identifier_before(
                    node,
                    &["parameter_list"],
                    &["template_argument_list", "template_parameter_list"],
                )
            }
            Grammar::CSharp => match kind {
                "indexer_declaration" => keyword("this"),
                "operator_declaration"
                | "conversion_operator_declaration"
                | "lambda_expression"
                | "anonymous_method_expression" => None,
                "accessor_declaration" => node
                    .child_by_field_name("name")
                    .map(|n| text(n, source).to_string()),
                _ => self.name_field(node),
            },
            Grammar::Dart => {
                if kind == "function_expression"
                    || find_first(node, "operator_signature", &["function_body"]).is_some()
                {
                    return None;
                }
                self.last_identifier_before(
                    node,
                    &["formal_parameter_list", "function_body"],
                    &["type_parameters"],
                )
            }
            Grammar::Elixir => {
                if kind != "call" {
                    return None;
                }
                let head = self.elixir_head(node)?;
                let name = if head.kind() == "call" {
                    head.child_by_field_name("target")?
                } else {
                    head
                };
                is_identifier_token(self.grammar, &name, source)
                    .then(|| text(name, source).to_string())
            }
            Grammar::Kotlin => match kind {
                "secondary_constructor" => keyword("constructor"),
                "anonymous_initializer" => keyword("init"),
                "getter" => keyword("get"),
                "setter" => keyword("set"),
                "function_declaration" => self.last_identifier_before(
                    node,
                    &["function_value_parameters", "function_body"],
                    &["type_parameters"],
                ),
                _ => None,
            },
            Grammar::Lua => {
                let name = node.child_by_field_name("name")?;
                self.last_identifier_before(name, &[], &[])
            }
            Grammar::Swift => match kind {
                "init_declaration" => keyword("init"),
                "deinit_declaration" => keyword("deinit"),
                "subscript_declaration" => keyword("subscript"),
                "computed_getter" => keyword("get"),
                "computed_setter" => keyword("set"),
                "computed_modify" => keyword("_modify"),
                "willset_clause" => keyword("willSet"),
                "didset_clause" => keyword("didSet"),
                "function_declaration" => self.name_field(node),
                _ => None,
            },
            _ => match kind {
                "block" | "static_initializer" | "lambda_expression" | "class_static_block"
                | "arrow_function" | "func_literal" | "anonymous_function" | "lambda"
                | "do_block" | "closure_expression" => None,
                _ => self.name_field(node),
            },
        }
    }

    /// The text of the first identifier token of the `name` field.
    fn name_field(&self, node: Node) -> Option<String> {
        let name = node.child_by_field_name("name")?;
        let mut stack = vec![name];
        while let Some(n) = stack.pop() {
            if is_identifier_token(self.grammar, &n, self.source) {
                return Some(text(n, self.source).to_string());
            }
            let mut kids = children(n);
            kids.reverse();
            stack.extend(kids);
        }
        None
    }

    /// The last identifier token in preorder before the first node of a `stop`
    /// kind, not entering nodes of `skip` kinds.
    fn last_identifier_before(&self, node: Node, stop: &[&str], skip: &[&str]) -> Option<String> {
        let mut last = None;
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            if stop.contains(&n.kind()) {
                break;
            }
            if skip.contains(&n.kind()) {
                continue;
            }
            if is_identifier_token(self.grammar, &n, self.source) {
                last = Some(n);
            }
            let mut kids = children(n);
            kids.reverse();
            stack.extend(kids);
        }
        last.map(|n| text(n, self.source).to_string())
    }
}
