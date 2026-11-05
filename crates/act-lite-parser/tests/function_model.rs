//! The function model: which constructs are functions in each language, the
//! four metrics measured on each function, function names, and the comment and
//! identifier-token rules.
//!
//! Every expected value below and in `fixtures/function_model/*/*.expected.json`
//! was written by hand from the source text.

use std::path::{Path, PathBuf};

use act_lite_core::registry::LanguageCategory;
use act_lite_parser::function_model::{self, Function, Metrics};
use act_lite_parser::{Grammar, SourceFile};
use serde::Deserialize;

fn parse(grammar: Grammar, source: &str) -> SourceFile {
    SourceFile::from_content("test", source.to_string(), grammar).unwrap()
}

fn functions(grammar: Grammar, source: &str) -> Vec<Function> {
    let file = parse(grammar, source);
    function_model::functions(&file).collect()
}

fn named<'a>(functions: &'a [Function], name: &str) -> &'a Function {
    functions
        .iter()
        .find(|f| f.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no function named {name:?} in {functions:#?}"))
}

fn metrics(lines: u32, nesting: u32, complexity: u32, parameters: Option<u32>) -> Metrics {
    Metrics {
        lines,
        nesting,
        complexity,
        parameters,
    }
}

fn is_data(grammar: Grammar) -> bool {
    matches!(grammar.category(), LanguageCategory::Data)
}

fn supported() -> Vec<Grammar> {
    Grammar::all()
        .into_iter()
        .filter(|g| !is_data(*g))
        .collect()
}

// The node kinds of each language's row of the function-construct table,
// copied from the table by hand: functions, parameters, if, else-if,
// multi-way and arms, loops, try and handlers, conditional expressions,
// logical operators.
const JAVASCRIPT_KINDS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "generator_function_declaration",
    "generator_function",
    "arrow_function",
    "method_definition",
    "class_static_block",
    "formal_parameters",
    "if_statement",
    "switch_statement",
    "switch_case",
    "switch_default",
    "for_statement",
    "for_in_statement",
    "while_statement",
    "do_statement",
    "try_statement",
    "catch_clause",
    "ternary_expression",
    "binary_expression",
];
const TYPESCRIPT_KINDS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "generator_function_declaration",
    "generator_function",
    "arrow_function",
    "method_definition",
    "class_static_block",
    "formal_parameters",
    "required_parameter",
    "optional_parameter",
    "this",
    "if_statement",
    "switch_statement",
    "switch_case",
    "switch_default",
    "for_statement",
    "for_in_statement",
    "while_statement",
    "do_statement",
    "try_statement",
    "catch_clause",
    "ternary_expression",
    "binary_expression",
];

fn table_kinds(grammar: Grammar) -> &'static [&'static str] {
    match grammar {
        Grammar::Bash => &[
            "function_definition",
            "if_statement",
            "elif_clause",
            "case_statement",
            "case_item",
            "for_statement",
            "c_style_for_statement",
            "while_statement",
            "ternary_expression",
            "list",
            "binary_expression",
        ],
        Grammar::C => &[
            "function_definition",
            "parameter_list",
            "parameter_declaration",
            "variadic_parameter",
            "if_statement",
            "switch_statement",
            "case_statement",
            "for_statement",
            "while_statement",
            "do_statement",
            "conditional_expression",
            "binary_expression",
        ],
        Grammar::Cpp => &[
            "function_definition",
            "lambda_expression",
            "parameter_list",
            "parameter_declaration",
            "optional_parameter_declaration",
            "variadic_parameter_declaration",
            "if_statement",
            "switch_statement",
            "case_statement",
            "for_statement",
            "for_range_loop",
            "while_statement",
            "do_statement",
            "try_statement",
            "catch_clause",
            "conditional_expression",
            "binary_expression",
        ],
        Grammar::CSharp => &[
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
            "arrow_expression_clause",
            "parameter_list",
            "bracketed_parameter_list",
            "parameter",
            "implicit_parameter",
            "if_statement",
            "switch_statement",
            "switch_section",
            "switch_expression",
            "switch_expression_arm",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "try_statement",
            "catch_clause",
            "conditional_expression",
            "binary_expression",
        ],
        Grammar::Dart => &[
            "lambda_expression",
            "class_member_definition",
            "function_body",
            "function_expression",
            "getter_signature",
            "setter_signature",
            "formal_parameter_list",
            "formal_parameter",
            "optional_formal_parameters",
            "if_statement",
            "switch_statement",
            "switch_label",
            "for_statement",
            "while_statement",
            "do_statement",
            "try_statement",
            "catch_clause",
            "conditional_expression",
            "logical_and_expression",
            "logical_or_expression",
            "if_null_expression",
        ],
        Grammar::Elixir => &[
            "call",
            "do_block",
            "anonymous_function",
            "arguments",
            "stab_clause",
            "rescue_block",
            "catch_block",
            "binary_operator",
        ],
        Grammar::Go => &[
            "function_declaration",
            "method_declaration",
            "func_literal",
            "parameter_list",
            "parameter_declaration",
            "variadic_parameter_declaration",
            "if_statement",
            "expression_switch_statement",
            "type_switch_statement",
            "select_statement",
            "expression_case",
            "type_case",
            "communication_case",
            "default_case",
            "for_statement",
            "binary_expression",
        ],
        Grammar::Java => &[
            "method_declaration",
            "constructor_declaration",
            "compact_constructor_declaration",
            "static_initializer",
            "block",
            "class_body",
            "lambda_expression",
            "formal_parameters",
            "formal_parameter",
            "spread_parameter",
            "receiver_parameter",
            "inferred_parameters",
            "if_statement",
            "switch_expression",
            "switch_label",
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
            "try_statement",
            "try_with_resources_statement",
            "catch_clause",
            "ternary_expression",
            "binary_expression",
        ],
        Grammar::JavaScript | Grammar::Jsx => JAVASCRIPT_KINDS,
        Grammar::TypeScript | Grammar::Tsx => TYPESCRIPT_KINDS,
        Grammar::Kotlin => &[
            "function_declaration",
            "secondary_constructor",
            "anonymous_initializer",
            "getter",
            "setter",
            "anonymous_function",
            "lambda_literal",
            "function_value_parameters",
            "parameter",
            "parameter_with_optional_type",
            "lambda_parameters",
            "variable_declaration",
            "multi_variable_declaration",
            "if_expression",
            "when_expression",
            "when_entry",
            "when_condition",
            "for_statement",
            "while_statement",
            "do_while_statement",
            "try_expression",
            "catch_block",
            "conjunction_expression",
            "disjunction_expression",
            "elvis_expression",
        ],
        Grammar::Lua => &[
            "function_declaration",
            "function_definition",
            "parameters",
            "identifier",
            "vararg_expression",
            "if_statement",
            "elseif_statement",
            "for_statement",
            "while_statement",
            "repeat_statement",
            "binary_expression",
        ],
        Grammar::Php => &[
            "function_definition",
            "method_declaration",
            "anonymous_function",
            "arrow_function",
            "formal_parameters",
            "simple_parameter",
            "variadic_parameter",
            "property_promotion_parameter",
            "if_statement",
            "else_if_clause",
            "switch_statement",
            "case_statement",
            "default_statement",
            "match_expression",
            "match_conditional_expression",
            "match_default_expression",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "try_statement",
            "catch_clause",
            "conditional_expression",
            "binary_expression",
        ],
        Grammar::Python => &[
            "function_definition",
            "lambda",
            "parameters",
            "lambda_parameters",
            "keyword_separator",
            "positional_separator",
            "decorated_definition",
            "if_statement",
            "elif_clause",
            "match_statement",
            "case_clause",
            "for_statement",
            "while_statement",
            "try_statement",
            "except_clause",
            "conditional_expression",
            "boolean_operator",
        ],
        Grammar::Ruby => &[
            "method",
            "singleton_method",
            "lambda",
            "block",
            "do_block",
            "method_parameters",
            "lambda_parameters",
            "block_parameters",
            "hash_splat_nil",
            "if",
            "unless",
            "if_modifier",
            "unless_modifier",
            "elsif",
            "case",
            "when",
            "case_match",
            "in_clause",
            "else",
            "while",
            "until",
            "for",
            "while_modifier",
            "until_modifier",
            "begin",
            "rescue",
            "rescue_modifier",
            "conditional",
            "binary",
        ],
        Grammar::Rust => &[
            "function_item",
            "closure_expression",
            "parameters",
            "parameter",
            "variadic_parameter",
            "self_parameter",
            "closure_parameters",
            "if_expression",
            "match_expression",
            "match_arm",
            "for_expression",
            "while_expression",
            "loop_expression",
            "binary_expression",
            "let_chain",
        ],
        Grammar::Scala => &[
            "function_definition",
            "lambda_expression",
            "parameters",
            "parameter",
            "bindings",
            "if_expression",
            "match_expression",
            "case_clause",
            "for_expression",
            "while_expression",
            "do_while_expression",
            "try_expression",
            "catch_clause",
            "infix_expression",
        ],
        Grammar::Swift => &[
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
            "parameter",
            "lambda_parameter",
            "if_statement",
            "guard_statement",
            "switch_statement",
            "switch_entry",
            "default_keyword",
            "for_statement",
            "while_statement",
            "repeat_while_statement",
            "do_statement",
            "catch_block",
            "ternary_expression",
            "conjunction_expression",
            "disjunction_expression",
            "nil_coalescing_expression",
        ],
        Grammar::Zig => &[
            "function_declaration",
            "test_declaration",
            "parameters",
            "parameter",
            "if_statement",
            "if_expression",
            "switch_expression",
            "switch_case",
            "while_statement",
            "while_expression",
            "for_statement",
            "for_expression",
            "catch_expression",
            "binary_expression",
        ],
        Grammar::R => &[
            "function_definition",
            "parameters",
            "parameter",
            "if_statement",
            "for_statement",
            "while_statement",
            "repeat_statement",
            "binary_operator",
        ],
        _ => &[],
    }
}

#[test]
fn node_kinds_are_the_table_kinds_and_exist_in_the_grammar() {
    let mut failures = Vec::new();
    for grammar in supported() {
        let mut expected: Vec<&str> = table_kinds(grammar).to_vec();
        expected.sort_unstable();
        expected.dedup();
        assert!(!expected.is_empty(), "{grammar:?} has no table row");
        let mut actual = function_model::node_kinds(grammar);
        actual.sort_unstable();
        if actual != expected {
            failures.push(format!(
                "{grammar:?}: node_kinds {actual:?}\n  table {expected:?}"
            ));
        }
        let language = grammar.language_support().ts_language();
        for kind in expected {
            if tree_sitter::Query::new(&language, &format!("({kind}) @probe")).is_err() {
                failures.push(format!("{grammar:?}: {kind} is not in the grammar"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn twenty_two_languages_are_supported() {
    let names: Vec<&str> = supported().iter().map(|g| g.name()).collect();
    assert_eq!(names.len(), 22, "{names:?}");
    for grammar in Grammar::all() {
        assert_eq!(
            function_model::supports(grammar),
            !is_data(grammar),
            "{grammar:?}"
        );
    }
}

// The over and under fixtures of every supported language.
#[derive(Debug, Deserialize, PartialEq)]
struct Expected {
    name: Option<String>,
    range: String,
    lines: u32,
    nesting: u32,
    complexity: u32,
    parameters: Option<u32>,
}

impl From<&Function> for Expected {
    fn from(f: &Function) -> Self {
        Expected {
            name: f.name.clone(),
            range: f.range.to_string(),
            lines: f.metrics.lines,
            nesting: f.metrics.nesting,
            complexity: f.metrics.complexity,
            parameters: f.metrics.parameters,
        }
    }
}

const FIXTURE_LANGUAGES: [(&str, &str); 22] = [
    ("bash", "sh"),
    ("c", "c"),
    ("cpp", "cpp"),
    ("csharp", "cs"),
    ("dart", "dart"),
    ("elixir", "ex"),
    ("go", "go"),
    ("java", "java"),
    ("javascript", "js"),
    ("jsx", "jsx"),
    ("kotlin", "kt"),
    ("lua", "lua"),
    ("php", "php"),
    ("python", "py"),
    ("r", "R"),
    ("ruby", "rb"),
    ("rust", "rs"),
    ("scala", "scala"),
    ("swift", "swift"),
    ("tsx", "tsx"),
    ("typescript", "ts"),
    ("zig", "zig"),
];

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/function_model")
}

#[test]
fn over_and_under_fixtures_match_their_expected_values() {
    let mut grammars = Vec::new();
    let mut failures = Vec::new();
    for (dir, ext) in FIXTURE_LANGUAGES {
        for stem in ["under", "over"] {
            let path = fixture_dir().join(dir).join(format!("{stem}.{ext}"));
            let file = SourceFile::load(&path).unwrap();
            assert!(
                !file.tree().root_node().has_error(),
                "{} does not parse cleanly",
                path.display()
            );
            grammars.push(file.grammar());
            let expected: Vec<Expected> = serde_json::from_str(
                &std::fs::read_to_string(path.with_extension("expected.json")).unwrap(),
            )
            .unwrap();
            let actual: Vec<Expected> = function_model::functions(&file)
                .map(|f| Expected::from(&f))
                .collect();
            if actual != expected {
                failures.push(format!(
                    "{}:\n expected {expected:?}\n actual   {actual:?}",
                    path.display()
                ));
            }
        }
    }
    grammars.sort_by_key(|g| g.name());
    grammars.dedup();
    assert_eq!(grammars.len(), 22, "{grammars:?}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn else_if_chain_nests_once_and_counts_each_branch() {
    let fs = functions(
        Grammar::TypeScript,
        "function g(a, b, c, d, e, f) {\n    if (a) {} else if (b) {} else if (c) {} else if (d) {} else if (e) {} else if (f) {}\n}\n",
    );
    assert_eq!(fs.len(), 1);
    assert_eq!(fs[0].metrics.nesting, 1);
    assert_eq!(fs[0].metrics.complexity, 7);
}

#[test]
fn nested_function_metrics_are_not_the_outer_functions() {
    let fs = functions(
        Grammar::JavaScript,
        "function outer() { const f = () => { if (a) { if (b) { if (c) { if (d) { if (e) {} } } } } }; return f; }",
    );
    assert_eq!(fs.len(), 2);
    assert_eq!(fs[0].name.as_deref(), Some("outer"));
    assert_eq!((fs[0].metrics.nesting, fs[0].metrics.complexity), (0, 1));
    assert_eq!(fs[1].name, None);
    assert_eq!(fs[1].metrics.nesting, 5);
}

#[test]
fn python_complexity_counts_each_decision() {
    let source = r#"def classify(n, items):
    if n < 0 and n > -10:
        return "neg"
    elif n == 0:
        return "zero"
    for x in items:
        while x > 0:
            x -= 1
    try:
        pass
    except ValueError:
        pass
    return "a" if n else "b"
"#;
    let fs = functions(Grammar::Python, source);
    assert_eq!(fs.len(), 1);
    assert_eq!(fs[0].name.as_deref(), Some("classify"));
    assert_eq!(fs[0].metrics, metrics(13, 2, 8, Some(2)));
}

#[test]
fn parameters_exclude_receivers_and_count_grouped_names() {
    let go = functions(
        Grammar::Go,
        "package p\nfunc (r *R) f(a, b, c int, d string, e ...int) {}\nfunc g(a, b, c, d, e, f int) {}\n",
    );
    assert_eq!(named(&go, "f").metrics.parameters, Some(5));
    assert_eq!(named(&go, "g").metrics.parameters, Some(6));

    let py = functions(
        Grammar::Python,
        "class C:\n    def m(self, a, b, c, d, e):\n        pass\n\ndef h(self, a, b, c, d, e):\n    pass\n\ndef k(a, /, b, *, c, **kw):\n    pass\n",
    );
    assert_eq!(named(&py, "m").metrics.parameters, Some(5));
    assert_eq!(named(&py, "h").metrics.parameters, Some(6));
    assert_eq!(named(&py, "k").metrics.parameters, Some(4));

    let ts = functions(
        Grammar::TypeScript,
        "function t(this: X, a, b, c, d, e) {}\n",
    );
    assert_eq!(named(&ts, "t").metrics.parameters, Some(5));
}

#[test]
fn dart_top_level_getter_spans_signature_and_body() {
    let fs = functions(Grammar::Dart, "int get top {\n  return 1;\n}\n");
    assert_eq!(fs.len(), 1);
    assert_eq!(fs[0].name.as_deref(), Some("top"));
    assert_eq!(fs[0].range.to_string(), "1:1-3:2");
    assert_eq!(fs[0].metrics, metrics(3, 0, 1, Some(0)));
}

#[test]
fn dart_top_level_accessor_with_a_comment_before_its_body_is_a_function() {
    let fs = functions(
        Grammar::Dart,
        "int get top /* note */ {\n  if (a) return 1;\n  return 2;\n}\nset top(int v) // note\n{\n  if (v > 0) x = v;\n}\n",
    );
    assert_eq!(fs.len(), 2, "{fs:#?}");
    assert_eq!(fs[0].name.as_deref(), Some("top"));
    assert_eq!(fs[0].range.to_string(), "1:1-4:2");
    assert_eq!(fs[0].metrics, metrics(4, 1, 2, Some(0)));
    assert_eq!(fs[1].name.as_deref(), Some("top"));
    assert_eq!(fs[1].range.to_string(), "5:1-8:2");
    assert_eq!(fs[1].metrics, metrics(4, 1, 2, Some(1)));
}

#[test]
fn swift_shorthand_computed_property_is_unnamed() {
    let fs = functions(
        Grammar::Swift,
        "struct S {\n    var v: Int {\n        return 1\n    }\n}\n",
    );
    assert_eq!(fs.len(), 1);
    assert_eq!(fs[0].name, None);
    assert_eq!(fs[0].range.to_string(), "2:16-4:6");
}

#[test]
fn swift_requirements_without_a_body_are_not_functions() {
    let protocol = "protocol P {\n    subscript(i: Int) -> Int { get }\n    subscript(a: Int, b: Int, c: Int, d: Int, e: Int, f: Int) -> Int { get set }\n    var v: Int { get }\n    var w: Int { get set }\n    func f(a: Int) -> Int\n}\n";
    let fs = functions(Grammar::Swift, protocol);
    assert!(fs.is_empty(), "{fs:#?}");
    // An implemented subscript is a function; a `get` block in it is one too.
    for (source, names) in [
        (
            "struct S {\n    subscript(i: Int) -> Int { return i }\n}\n",
            &["subscript"][..],
        ),
        (
            "struct S {\n    subscript(i: Int) -> Int { get { i } }\n}\n",
            &["subscript", "get"][..],
        ),
    ] {
        let fs = functions(Grammar::Swift, source);
        let found: Vec<_> = fs.iter().map(|f| f.name.as_deref().unwrap()).collect();
        assert_eq!(found, names, "{source}");
        assert_eq!(fs[0].metrics.parameters, Some(1), "{source}");
    }
}

#[test]
fn csharp_expression_bodied_property_and_indexer_are_functions() {
    let fs = functions(
        Grammar::CSharp,
        "class C {\n    int P => 1;\n    int this[int i] => i;\n    int Q { get; set; }\n}\n",
    );
    assert_eq!(fs.len(), 2, "{fs:#?}");
    assert_eq!(fs[0].name.as_deref(), Some("P"));
    assert_eq!(fs[0].range.to_string(), "2:5-2:16");
    assert_eq!(fs[1].name.as_deref(), Some("this"));
    assert_eq!(fs[1].metrics.parameters, Some(1));
}

#[test]
fn python_static_method_excludes_a_first_self() {
    let fs = functions(
        Grammar::Python,
        "class C:\n    @staticmethod\n    def s(self, a):\n        pass\n\n    @staticmethod\n    def t(x, a):\n        pass\n",
    );
    assert_eq!(named(&fs, "s").metrics.parameters, Some(1));
    assert_eq!(named(&fs, "s").range.to_string(), "3:5-4:13");
    assert_eq!(named(&fs, "t").metrics.parameters, Some(2));
}

#[test]
fn ruby_stabby_lambda_is_one_function() {
    let fs = functions(Grammar::Ruby, "l = ->(x) { x + 1 }\n");
    assert_eq!(fs.len(), 1);
    assert_eq!(fs[0].name, None);
    assert_eq!(fs[0].range.to_string(), "1:5-1:20");
    assert_eq!(fs[0].metrics.parameters, Some(1));
}

#[test]
fn elixir_keyword_def_is_named_by_its_head() {
    let fs = functions(Grammar::Elixir, "defmodule M do\n  def f(x), do: x\nend\n");
    assert_eq!(fs.len(), 1);
    assert_eq!(fs[0].name.as_deref(), Some("f"));
    assert_eq!(fs[0].range.to_string(), "2:3-2:18");
    assert_eq!(fs[0].metrics.parameters, Some(1));
}

#[test]
fn rust_generic_function_is_named() {
    let fs = functions(Grammar::Rust, "fn foo<T: Clone>(x: T) -> T {\n    x\n}\n");
    assert_eq!(fs.len(), 1);
    assert_eq!(fs[0].name.as_deref(), Some("foo"));
    assert_eq!(fs[0].range.to_string(), "1:1-3:2");
}

#[test]
fn go_generic_function_is_named() {
    let fs = functions(
        Grammar::Go,
        "package p\n\nfunc Map[T any](x T) T {\n\treturn x\n}\n",
    );
    assert_eq!(fs.len(), 1);
    assert_eq!(fs[0].name.as_deref(), Some("Map"));
    assert_eq!(fs[0].range.to_string(), "3:1-5:2");
    assert_eq!(fs[0].metrics.parameters, Some(1));
}

#[test]
fn java_instance_initializer_is_unnamed() {
    let fs = functions(
        Grammar::Java,
        "class A {\n    int x;\n    {\n        x = 1;\n    }\n}\n",
    );
    assert_eq!(fs.len(), 1);
    assert_eq!(fs[0].name, None);
    assert_eq!(fs[0].range.to_string(), "3:5-5:6");
    assert_eq!(fs[0].metrics, metrics(3, 0, 1, Some(0)));
}

#[test]
fn else_if_adds_a_decision_and_no_nesting() {
    let cases: [(Grammar, &str); 31] = [
        (
            Grammar::Bash,
            "f() {\n  if a; then b; elif c; then d; fi\n}\n",
        ),
        (
            Grammar::Bash,
            "f() {\n  if a; then b; else if c; then d; fi; fi\n}\n",
        ),
        (Grammar::C, "void f() { if (a) {} else if (b) {} }"),
        (Grammar::Cpp, "void f() { if (a) {} else if (b) {} }"),
        (
            Grammar::CSharp,
            "class C { void F() { if (a) {} else if (b) {} } }",
        ),
        (Grammar::Dart, "void f() { if (a) {} else if (b) {} }"),
        (
            Grammar::Elixir,
            "def f do\n  if a do\n    1\n  else\n    if b do\n      2\n    end\n  end\nend\n",
        ),
        (Grammar::Go, "package p\nfunc f() { if a {} else if b {} }"),
        (
            Grammar::Java,
            "class C { void f() { if (a) {} else if (b) {} } }",
        ),
        (
            Grammar::JavaScript,
            "function f() { if (a) {} else if (b) {} }",
        ),
        (Grammar::Jsx, "function f() { if (a) {} else if (b) {} }"),
        (Grammar::Kotlin, "fun f() { if (a) { } else if (b) { } }"),
        (
            Grammar::Lua,
            "function f()\n  if a then elseif b then end\nend\n",
        ),
        (
            Grammar::Php,
            "<?php function f() { if ($a) {} elseif ($b) {} }",
        ),
        (
            Grammar::Php,
            "<?php function f() { if ($a) {} else if ($b) {} }",
        ),
        (
            Grammar::Python,
            "def f():\n    if a:\n        pass\n    elif b:\n        pass\n",
        ),
        (Grammar::Ruby, "def f\n  if a\n  elsif b\n  end\nend\n"),
        (
            Grammar::Ruby,
            "def f\n  if a\n  else\n    if b\n    end\n  end\nend\n",
        ),
        (Grammar::Rust, "fn f() { if a {} else if b {} }"),
        (
            Grammar::Scala,
            "object O { def f() = { if (a) 1 else if (b) 2 } }",
        ),
        (Grammar::Swift, "func f() { if a { } else if b { } }"),
        (
            Grammar::TypeScript,
            "function f() { if (a) {} else if (b) {} }",
        ),
        (Grammar::Tsx, "function f() { if (a) {} else if (b) {} }"),
        (
            Grammar::CSharp,
            "class C { void F() { if (a) {} else if (b) { if (c) {} } } }",
        ),
        (
            Grammar::Kotlin,
            "fun f() { if (a) { } else if (b) { if (c) { } } }",
        ),
        (Grammar::Zig, "fn f() void { if (a) {} else if (b) {} }"),
        (
            Grammar::Zig,
            "fn f() void { const y = if (a) 1 else if (b) 2 else 3; }",
        ),
        (
            Grammar::Zig,
            "fn f() void { if (a) {} else if (b) { if (c) {} } }",
        ),
        (Grammar::R, "f <- function() { if (a) {} else if (b) {} }"),
        (
            Grammar::R,
            "f <- function() {\n  if (a) {} else {\n    if (b) {}\n  }\n}\n",
        ),
        (
            Grammar::R,
            "f <- function() { if (a) {} else if (b) { if (c) {} } }",
        ),
    ];
    for (i, (grammar, source)) in cases.iter().enumerate() {
        let fs = functions(*grammar, source);
        assert_eq!(fs.len(), 1, "{grammar:?} case {i}: {fs:#?}");
        let expected = if source.contains("(c)") {
            (2, 4)
        } else {
            (1, 3)
        };
        assert_eq!(
            (fs[0].metrics.nesting, fs[0].metrics.complexity),
            expected,
            "{grammar:?} case {i}"
        );
    }
}

#[test]
fn lines_skip_docstrings_blank_and_comment_lines() {
    let source = "def f():\n    \"\"\"Doc\n    string.\"\"\"\n\n    # comment\n    x = \"\"\"a\nb\nc\"\"\"\n    return x\n";
    let fs = functions(Grammar::Python, source);
    assert_eq!(fs.len(), 1);
    // def line, the three lines of the non-docstring string, the return line.
    assert_eq!(fs[0].metrics.lines, 5);
}

#[test]
fn lines_shared_with_a_nested_function_count_for_both() {
    let source =
        "function outer() {\n    const g = () => {\n        return 1;\n    };\n    return g;\n}\n";
    let fs = functions(Grammar::JavaScript, source);
    assert_eq!(fs.len(), 2);
    // outer: lines 1, 2, 4, 5, 6 (line 3 holds only nested code).
    assert_eq!(fs[0].metrics.lines, 5);
    // the arrow function: lines 2, 3, 4.
    assert_eq!(fs[1].metrics.lines, 3);
}

#[test]
fn lines_skip_comment_only_lines_in_c_like_code() {
    let source = "fn f() {\n    // one\n    /* two\n       three */\n\n    let x = 1;\n}\n";
    let fs = functions(Grammar::Rust, source);
    assert_eq!(fs[0].metrics.lines, 3);
}

#[test]
fn declarations_without_a_body_are_not_functions() {
    assert!(functions(Grammar::C, "int h(int a);\n").is_empty());
    assert!(functions(Grammar::Java, "interface I { void n(); }\n").is_empty());
    assert!(functions(Grammar::Rust, "trait T { fn s(&self); }\n").is_empty());
    assert!(functions(Grammar::Swift, "protocol P { func g() }\n").is_empty());
    assert!(functions(Grammar::Zig, "extern fn g(a: u8) void;\n").is_empty());
    assert!(functions(Grammar::Dockerfile, "FROM alpine\nRUN true\n").is_empty());
    assert!(
        functions(
            Grammar::HclTerraform,
            "resource \"a\" \"b\" {\n  x = 1\n}\n"
        )
        .is_empty()
    );
}

#[test]
fn data_formats_are_not_supported() {
    let samples: [(Grammar, &str); 10] = [
        (Grammar::Css, "a { color: red; }\n"),
        (Grammar::Dockerfile, "FROM alpine\n"),
        (Grammar::HclTerraform, "a = 1\n"),
        (Grammar::Html, "<p>a</p>\n"),
        (Grammar::Json, "{\"a\": 1}\n"),
        (Grammar::Markdown, "# a\n"),
        (Grammar::Scss, "a { b { color: red; } }\n"),
        (Grammar::Sql, "SELECT 1;\n"),
        (Grammar::Toml, "a = 1\n"),
        (Grammar::Yaml, "a: 1\n"),
    ];
    for (grammar, source) in samples {
        assert!(is_data(grammar));
        assert!(!function_model::supports(grammar), "{grammar:?}");
        assert!(functions(grammar, source).is_empty(), "{grammar:?}");
    }
}

#[test]
fn comment_kinds() {
    for kind in [
        "comment",
        "line_comment",
        "block_comment",
        "doc_comment",
        "multiline_comment",
        "html_comment",
        "documentation_comment",
        "line_continuation",
    ] {
        assert!(function_model::is_comment(kind), "{kind}");
    }
    for kind in [
        "identifier",
        "string",
        "heredoc_body",
        "text_interpolation",
        "preproc_if",
        "comment_content",
    ] {
        assert!(!function_model::is_comment(kind), "{kind}");
    }
}

fn find_leaf<'t>(
    node: tree_sitter::Node<'t>,
    source: &str,
    kind: &str,
    text: &str,
) -> Option<tree_sitter::Node<'t>> {
    if node.kind() == kind && &source[node.byte_range()] == text {
        return Some(node);
    }
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();
    children
        .into_iter()
        .find_map(|child| find_leaf(child, source, kind, text))
}

fn identifier_token(grammar: Grammar, source: &str, kind: &str, text: &str) -> bool {
    let file = parse(grammar, source);
    let node = find_leaf(file.tree().root_node(), source, kind, text)
        .unwrap_or_else(|| panic!("{grammar:?}: no {kind} {text:?}"));
    function_model::is_identifier_token(grammar, &node, source)
}

#[test]
fn identifier_tokens() {
    let yes: [(Grammar, &str, &str, &str); 14] = [
        (Grammar::Rust, "fn f(s: S) { s.x; }", "identifier", "f"),
        (
            Grammar::Rust,
            "fn f(s: S) { s.x; }",
            "field_identifier",
            "x",
        ),
        (Grammar::Rust, "fn f(s: S) { s.x; }", "type_identifier", "S"),
        (Grammar::JavaScript, "a.b;", "property_identifier", "b"),
        (Grammar::Kotlin, "fun f() {}", "simple_identifier", "f"),
        (
            Grammar::Go,
            "package p\nvar x int",
            "type_identifier",
            "int",
        ),
        (Grammar::Ruby, "A = @b + @@c + $d", "constant", "A"),
        (
            Grammar::Ruby,
            "A = @b + @@c + $d",
            "instance_variable",
            "@b",
        ),
        (Grammar::Ruby, "A = @b + @@c + $d", "class_variable", "@@c"),
        (Grammar::Ruby, "A = @b + @@c + $d", "global_variable", "$d"),
        (Grammar::Elixir, "Foo.bar(x)", "alias", "Foo"),
        (Grammar::Elixir, "Foo.bar(x)", "identifier", "x"),
        (Grammar::Php, "<?php f($a);", "name", "a"),
        (Grammar::Bash, "echo $v", "variable_name", "v"),
    ];
    for (grammar, source, kind, text) in yes {
        assert!(
            identifier_token(grammar, source, kind, text),
            "{grammar:?} {kind} {text:?}"
        );
    }
    let no: [(Grammar, &str, &str, &str); 9] = [
        (
            Grammar::Go,
            "package p\nimport _ \"fmt\"",
            "blank_identifier",
            "_",
        ),
        (
            Grammar::Scala,
            "object O { val a = b && c }",
            "operator_identifier",
            "&&",
        ),
        (Grammar::Elixir, "def f do\n  1\nend", "identifier", "def"),
        (Grammar::Elixir, "if a do\n  1\nend", "identifier", "if"),
        (
            Grammar::Elixir,
            "defmodule M do\nend",
            "identifier",
            "defmodule",
        ),
        (Grammar::Elixir, "alias Foo.Bar", "identifier", "alias"),
        (Grammar::Rust, "fn f(x: u8) {}", "primitive_type", "u8"),
        (Grammar::Bash, "echo hi", "word", "hi"),
        (
            Grammar::Cpp,
            "int x = a::b;",
            "qualified_identifier",
            "a::b",
        ),
    ];
    for (grammar, source, kind, text) in no {
        assert!(
            !identifier_token(grammar, source, kind, text),
            "{grammar:?} {kind} {text:?}"
        );
    }
    let keyword_targets = [
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
    for word in keyword_targets {
        // `fn` is a keyword token in call position; after a dot it is an identifier.
        let source = if word == "fn" {
            "x.fn(y)".to_string()
        } else {
            format!("{word}(x)")
        };
        assert!(
            !identifier_token(Grammar::Elixir, &source, "identifier", word),
            "Elixir {word}"
        );
    }
}

#[test]
fn functions_iterates_lazily_in_location_order() {
    let file = parse(
        Grammar::Rust,
        "fn a() {}\nfn b() { let c = || 1; }\nfn d() {}\n",
    );
    let mut it = function_model::functions(&file);
    assert_eq!(it.next().unwrap().name.as_deref(), Some("a"));
    let b = it.next().unwrap();
    assert_eq!(b.name.as_deref(), Some("b"));
    let closure = it.next().unwrap();
    assert_eq!(closure.name, None);
    assert_eq!(closure.range.to_string(), "2:18-2:22");
    assert_eq!(it.next().unwrap().name.as_deref(), Some("d"));
    assert!(it.next().is_none());
}

// The Parameters column, form by form: (grammar, source, index of the function
// in location order, expected count).
#[test]
fn parameter_forms() {
    let cases: [(Grammar, &str, usize, u32); 35] = [
        (Grammar::C, "int f(void) { return 0; }", 0, 0),
        (Grammar::C, "int f(int a, ...) { return 0; }", 0, 2),
        (
            Grammar::Cpp,
            "int f(int a, int b = 1, ...) { return 0; }",
            0,
            3,
        ),
        (
            Grammar::Cpp,
            "void g() { auto l = [](int x, int y) { return x; }; }",
            1,
            2,
        ),
        (
            Grammar::CSharp,
            "class C { void M(int a, params int[] b) {} }",
            0,
            2,
        ),
        (
            Grammar::CSharp,
            "class C { void M() { F f = x => x; } }",
            1,
            1,
        ),
        (
            Grammar::Dart,
            "int f(int a, [int b = 1]) { return a; }",
            0,
            2,
        ),
        (Grammar::Elixir, "f = fn a, b -> a end", 0, 2),
        (Grammar::Elixir, "def g do\n  1\nend", 0, 0),
        (
            Grammar::Elixir,
            "f = fn a, b, c, d, e, f when a > 0 -> a end",
            0,
            6,
        ),
        (Grammar::Elixir, "f = fn a, b, c, d, e, f -> a end", 0, 6),
        (
            Grammar::Elixir,
            "defmodule M do\n  def h(a, b, c) when a > 0, do: a\nend",
            0,
            3,
        ),
        (Grammar::Go, "package p\nfunc f(int, string) {}", 0, 2),
        (
            Grammar::Java,
            "class A { void m() { F f = (x, y) -> x; } }",
            1,
            2,
        ),
        (
            Grammar::Java,
            "class A { void m() { F f = x -> x; } }",
            1,
            1,
        ),
        (Grammar::JavaScript, "function g(a, {b}, ...c) {}", 0, 3),
        (
            Grammar::Kotlin,
            "class K {\n  var p: Int = 0\n    set(v) { field = v }\n}",
            0,
            1,
        ),
        (Grammar::Kotlin, "val l = { a: Int, (b, c): P -> a }", 0, 2),
        (Grammar::Lua, "function f(a, ...) end", 0, 2),
        (
            Grammar::Php,
            "<?php class K { function __construct(private $a, $b, ...$c) {} }",
            0,
            3,
        ),
        (Grammar::Python, "f = lambda: 1", 0, 0),
        (Grammar::Python, "f = lambda x, y: x", 0, 2),
        (Grammar::Ruby, "def f(a, **nil); end", 0, 1),
        (Grammar::Ruby, "xs.each { |v; w| v }", 0, 1),
        (Grammar::Rust, "impl S { fn f(&self, a: u8) {} }", 0, 1),
        (Grammar::Rust, "fn g() { let c = |p, q: u8| p; }", 1, 2),
        (
            Grammar::Scala,
            "object O { val g = (x: Int, y: Int) => x }",
            0,
            2,
        ),
        (
            Grammar::Scala,
            "object O { val g = (x: Int) => x; def h(a: Int)(b: Int) = a }",
            1,
            2,
        ),
        (Grammar::Swift, "let c = { (x: Int, y: Int) in x }", 0, 2),
        (Grammar::Swift, "class S { init(x: Int) {} }", 0, 1),
        (
            Grammar::Zig,
            "fn f(comptime T: type, x: anytype, y: T) void {}",
            0,
            3,
        ),
        (Grammar::Zig, "test \"t\" {}", 0, 0),
        (Grammar::R, "f <- function(x, k = 2, ...) x", 0, 3),
        (Grammar::R, "f <- function() 1", 0, 0),
        (Grammar::R, "lapply(v, function(y) y)", 0, 1),
    ];
    for (i, (grammar, source, index, expected)) in cases.iter().enumerate() {
        let fs = functions(*grammar, source);
        assert_eq!(
            fs.get(*index).and_then(|f| f.metrics.parameters),
            Some(*expected),
            "{grammar:?} case {i}: {fs:#?}"
        );
    }
}

// The name rule's exceptions and keyword names.
#[test]
fn names_of_special_forms() {
    let names = |grammar: Grammar, source: &str| -> Vec<Option<String>> {
        functions(grammar, source)
            .into_iter()
            .map(|f| f.name)
            .collect()
    };
    let some = |names: &[&str]| -> Vec<Option<String>> {
        names.iter().map(|n| Some(n.to_string())).collect()
    };
    assert_eq!(
        names(
            Grammar::Cpp,
            "bool Foo::operator==(const Foo& o) const { return true; }\nFoo::~Foo() {}\nFoo::operator int() const { return 1; }\n",
        ),
        some(&["operator==", "~Foo", "operator int() const"])
    );
    assert_eq!(
        names(
            Grammar::CSharp,
            "class C {\n  public static C operator +(C a, C b) => a;\n  public C(int x) {}\n}\n",
        ),
        vec![None, Some("C".to_string())]
    );
    assert_eq!(
        names(
            Grammar::Dart,
            "class C {\n  bool operator ==(Object o) => true;\n  C.other(int x) {}\n  set v(int x) {}\n}\n",
        ),
        vec![None, Some("other".to_string()), Some("v".to_string())]
    );
    assert_eq!(
        names(
            Grammar::Kotlin,
            "class K(x: Int) {\n  constructor(y: String) : this(1) { }\n  init { }\n  val p: Int\n    get() { return 1 }\n}\n",
        ),
        some(&["constructor", "init", "get"])
    );
    assert_eq!(
        names(
            Grammar::Swift,
            "class S {\n  init() {}\n  deinit {}\n  var v: Int = 0 { willSet {} didSet {} }\n  var w: Int { get { 1 } set {} }\n}\n",
        ),
        some(&["init", "deinit", "willSet", "didSet", "get", "set"])
    );
    assert_eq!(
        names(
            Grammar::Lua,
            "local function f() end\nlocal g = function() end\n"
        ),
        vec![Some("f".to_string()), None]
    );
    assert_eq!(names(Grammar::Ruby, "def x=(v); end\n"), some(&["x"]));
    assert_eq!(
        names(Grammar::JavaScript, "class K { #p() {} }\n"),
        some(&["#p"])
    );
    assert_eq!(names(Grammar::Bash, "function g { :; }\n"), some(&["g"]));
    assert_eq!(
        names(Grammar::C, "int *g(void) { return 0; }\n"),
        some(&["g"])
    );
    assert_eq!(
        names(
            Grammar::Zig,
            "const S = struct {\n    x: u8,\n    fn m(self: S) u8 {\n        return self.x;\n    }\n};\ntest \"t\" {}\ntest n {}\ntest {}\n",
        ),
        vec![Some("m".to_string()), None, Some("n".to_string()), None]
    );
    assert_eq!(
        names(
            Grammar::R,
            "f <- function(x) x\ng = function() 1\nh <<- function() 2\n\"s\" <- function() 3\n`b q` <- function() 4\nlapply(1, function(y) y)\nk <- \\(x) x\nobj$m <- function() 5\n",
        ),
        vec![
            Some("f".to_string()),
            Some("g".to_string()),
            Some("h".to_string()),
            Some("s".to_string()),
            Some("`b q`".to_string()),
            None,
            Some("k".to_string()),
            None,
        ]
    );
}

// A multi-line string counts every line it spans, blank lines inside it included.
#[test]
fn multi_line_strings_count_every_line_they_span() {
    let cases: [(Grammar, &str, u32); 8] = [
        (Grammar::Bash, "f() {\n  echo \"a\n\n\n\nb\"\n}\n", 7),
        (
            Grammar::Kotlin,
            "fun f() {\n    val s = \"\"\"\n\n$x\n\n\"\"\"\n}\n",
            7,
        ),
        (
            Grammar::Scala,
            "object O {\n  def f() = {\n    s\"\"\"\n\n$x\n\n\"\"\"\n  }\n}\n",
            7,
        ),
        (
            Grammar::Dart,
            "void f() {\n  var s = \"\"\"\n\n$x\n\n\"\"\";\n}\n",
            7,
        ),
        (
            Grammar::Php,
            "<?php\nfunction f() {\n    $s = <<<EOT\n\n$x\n\nEOT;\n}\n",
            7,
        ),
        (
            Grammar::Python,
            "def f():\n    x = 1\n    s = \"\"\"\n\na\n\n\"\"\"\n    return s\n",
            8,
        ),
        (
            Grammar::JavaScript,
            "function f() {\n    const s = `\n\na\n\n`;\n}\n",
            7,
        ),
        (Grammar::Ruby, "def f\n  s = \"a\n\n\nb\"\n  s\nend\n", 7),
    ];
    let mut failures = Vec::new();
    for (grammar, source, expected) in cases {
        let fs = functions(grammar, source);
        let lines = fs.first().map(|f| f.metrics.lines);
        if lines != Some(expected) {
            failures.push(format!("{grammar:?}: {lines:?}, expected {expected}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

// The lines of a function nested in a template substitution count only for
// the nested function, not for the function that holds the string.
#[test]
fn a_function_inside_a_template_substitution_keeps_its_lines() {
    let source = "function f() {\n    const s = `a${ (a) => {\n        const b = a + 1;\n        const c = b * 2;\n        return c;\n    } }b`;\n    return s;\n}\n";
    for grammar in [Grammar::JavaScript, Grammar::TypeScript] {
        let fs = functions(grammar, source);
        let lines: Vec<u32> = fs.iter().map(|f| f.metrics.lines).collect();
        // f: lines 1, 2, 6, 7 and 8; the arrow function: lines 2 to 6.
        assert_eq!(lines, [5, 5], "{grammar:?}");
    }
}

#[test]
fn php_keyword_operators_are_case_insensitive() {
    let fs = functions(
        Grammar::Php,
        "<?php function f($a, $b) { return $a AND $b Or $a; }",
    );
    assert_eq!(fs[0].metrics.complexity, 3);
}

// Every anonymous-function kind of the table gives an unnamed function. Each
// case is (grammar, text before the function, the function, text after it,
// index of the function in location order, expected metrics); the range is
// derived from where the function text sits in the source.
#[test]
fn anonymous_functions_are_unnamed_functions() {
    let m = |nesting, complexity| metrics(1, nesting, complexity, Some(1));
    let cases: [(Grammar, &str, &str, &str, usize, Metrics); 28] = [
        (
            Grammar::Cpp,
            "void g() { auto l = ",
            "[](int x) { if (x) { return 1; } return 0; }",
            "; }",
            1,
            m(1, 2),
        ),
        (
            Grammar::CSharp,
            "class C { void M() { F f = ",
            "x => x ? 1 : 0",
            "; } }",
            1,
            m(0, 2),
        ),
        (
            Grammar::CSharp,
            "class C { void M() { F f = ",
            "delegate (int x) { if (x > 0) { return 1; } return 0; }",
            "; } }",
            1,
            m(1, 2),
        ),
        (
            Grammar::Dart,
            "void g() { var f = ",
            "(x) => x ? 1 : 0",
            "; }",
            1,
            m(0, 2),
        ),
        (
            Grammar::Elixir,
            "f = ",
            "fn x -> if x, do: 1, else: 0 end",
            "\n",
            0,
            m(1, 2),
        ),
        (
            Grammar::Go,
            "package p\nfunc g() { f := ",
            "func(x int) int { if x > 0 { return 1 }; return 0 }",
            "; _ = f }",
            1,
            m(1, 2),
        ),
        (
            Grammar::Java,
            "class A { void m() { F f = ",
            "x -> x > 0 ? 1 : 0",
            "; } }",
            1,
            m(0, 2),
        ),
        (
            Grammar::JavaScript,
            "const f = ",
            "x => x ? 1 : 0",
            ";",
            0,
            m(0, 2),
        ),
        (
            Grammar::JavaScript,
            "const f = ",
            "function (x) { if (x) { return 1; } return 0; }",
            ";",
            0,
            m(1, 2),
        ),
        (
            Grammar::JavaScript,
            "const f = ",
            "function* (x) { if (x) { yield 1; } }",
            ";",
            0,
            m(1, 2),
        ),
        (
            Grammar::Jsx,
            "const f = ",
            "x => x ? <a /> : null",
            ";",
            0,
            m(0, 2),
        ),
        (
            Grammar::Jsx,
            "const f = ",
            "function (x) { if (x) { return 1; } return 0; }",
            ";",
            0,
            m(1, 2),
        ),
        (
            Grammar::TypeScript,
            "const f = ",
            "(x: number) => x ? 1 : 0",
            ";",
            0,
            m(0, 2),
        ),
        (
            Grammar::TypeScript,
            "const f = ",
            "function (x: number) { if (x) { return 1; } return 0; }",
            ";",
            0,
            m(1, 2),
        ),
        (
            Grammar::Tsx,
            "const f = ",
            "(x: number) => x ? <a /> : null",
            ";",
            0,
            m(0, 2),
        ),
        (
            Grammar::Tsx,
            "const f = ",
            "function* (x: number) { if (x) { yield 1; } }",
            ";",
            0,
            m(1, 2),
        ),
        (
            Grammar::Kotlin,
            "val f = ",
            "{ x: Int -> if (x > 0) 1 else 0 }",
            "\n",
            0,
            m(1, 2),
        ),
        (
            Grammar::Kotlin,
            "val f = ",
            "fun(x: Int): Int { return if (x > 0) 1 else 0 }",
            "\n",
            0,
            m(1, 2),
        ),
        (
            Grammar::Lua,
            "local f = ",
            "function(x) if x then return 1 end return 0 end",
            "\n",
            0,
            m(1, 2),
        ),
        (
            Grammar::Php,
            "<?php $f = ",
            "function ($x) { if ($x) { return 1; } return 0; }",
            ";",
            0,
            m(1, 2),
        ),
        (
            Grammar::Php,
            "<?php $f = ",
            "fn($x) => $x ? 1 : 0",
            ";",
            0,
            m(0, 2),
        ),
        (
            Grammar::Python,
            "f = ",
            "lambda x: 1 if x else 0",
            "\n",
            0,
            m(0, 2),
        ),
        (
            Grammar::Ruby,
            "f = ",
            "->(x) { x ? 1 : 0 }",
            "\n",
            0,
            m(0, 2),
        ),
        (
            Grammar::Ruby,
            "xs.each ",
            "{ |x| puts(x) if x }",
            "\n",
            0,
            m(1, 2),
        ),
        (
            Grammar::Ruby,
            "xs.each ",
            "do |x| puts(x) if x end",
            "\n",
            0,
            m(1, 2),
        ),
        (
            Grammar::Rust,
            "fn g() { let c = ",
            "|x: u8| if x > 0 { 1 } else { 0 }",
            "; }",
            1,
            m(1, 2),
        ),
        (
            Grammar::Scala,
            "object O { val f = ",
            "(x: Int) => if (x > 0) 1 else 0",
            " }",
            0,
            m(1, 2),
        ),
        (
            Grammar::Swift,
            "let f = ",
            "{ (x: Int) in x > 0 ? 1 : 0 }",
            "\n",
            0,
            m(0, 2),
        ),
    ];
    let mut failures = Vec::new();
    for (grammar, before, function, after, index, expected) in cases {
        let source = format!("{before}{function}{after}");
        let line = before.matches('\n').count() + 1;
        let column = before.len() - before.rfind('\n').map_or(0, |i| i + 1) + 1;
        let range = format!("{line}:{column}-{line}:{}", column + function.len());
        let actual = functions(grammar, &source)
            .get(index)
            .map(|f| (f.name.clone(), f.range.to_string(), f.metrics));
        if actual != Some((None, range.clone(), expected)) {
            failures.push(format!(
                "{grammar:?} {function:?}: {actual:?}, expected {range} {expected:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
