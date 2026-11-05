use act_lite_parser::{Grammar, QueryResult, SourceFile, execute_symbols};

/// `(name, kind)` for every symbol the grammar's symbols query emits.
fn symbols(ext: &str, grammar: Grammar, src: &str) -> Vec<(String, String)> {
    let file = SourceFile::from_content(format!("probe.{ext}"), src.to_string(), grammar)
        .expect("fixture must parse");
    match execute_symbols(&file).expect("symbols query must run") {
        QueryResult::Symbols { symbols } => symbols
            .iter()
            .map(|s| (s.name().to_string(), format!("{:?}", s.kind())))
            .collect(),
        other => panic!("expected Symbols, got {other:?}"),
    }
}

/// The invariant this row exists to establish, asserted per fixture.
fn assert_no_nameless(rows: &[(String, String)]) {
    let nameless: Vec<_> = rows.iter().filter(|(n, _)| n.is_empty()).collect();
    assert!(
        nameless.is_empty(),
        "nameless symbol(s) emitted: {nameless:?} (all rows: {rows:?})"
    );
}

/// The DROP dispositions: constructs with no name node anywhere, which could
/// therefore only ever be reported nameless. Each fixture also asserts the
/// grammar's *named* declarations survive the drop — the risk of a DROP is
/// removing coverage, not removing noise.
#[test]
fn dropped_anonymous_constructs_leave_named_declarations_intact() {
    let cases: &[(&str, Grammar, &str, &[&str])] = &[
        (
            "dart",
            Grammar::Dart,
            "import 'dart:async';\nclass A {\n  void m() {}\n}\n",
            &["A", "m"],
        ),
        (
            "ex",
            Grammar::Elixir,
            "defmodule M do\n  defstruct [:a]\n  def f(x) do\n    fn y -> y end\n  end\nend\n",
            &["M", "f"],
        ),
        (
            "rb",
            Grammar::Ruby,
            "f = ->(x) { x }\nclass C\n  def m; end\nend\n",
            &["C", "m"],
        ),
    ];

    for (ext, grammar, src, expected) in cases {
        let rows = symbols(ext, *grammar, src);
        assert_no_nameless(&rows);
        let names: Vec<&str> = rows.iter().map(|(n, _)| n.as_str()).collect();
        for want in *expected {
            assert!(
                names.contains(want),
                "{ext}: dropping the anonymous rule must not lose `{want}`; got {rows:?}"
            );
        }
    }
}

#[test]
fn yaml_anchors_are_named() {
    let rows = symbols(
        "yaml",
        Grammar::Yaml,
        "base: &anchor_name\n  a: 1\nuse: *anchor_name\n",
    );
    assert_no_nameless(&rows);
    assert!(
        rows.iter().any(|(n, _)| n == "anchor_name"),
        "yaml must name the anchor; got {rows:?}"
    );
}

/// Swift carries both halves of the defect: three rules that bind no `@name`
/// (`init`/`deinit`/`subscript`) and three that bind it to an optional node.
#[test]
fn swift_members_and_extensions_are_named() {
    let rows = symbols(
        "swift",
        Grammar::Swift,
        "class Box {\n\
         \x20   init(v: Int) {}\n\
         \x20   deinit {}\n\
         \x20   subscript(i: Int) -> Int { return i }\n\
         \x20   func run() {}\n\
         }\n\
         extension Array {\n\
         \x20   func first2() {}\n\
         }\n",
    );
    assert_no_nameless(&rows);

    for (name, kind) in [
        ("init", "Constructor"),
        ("deinit", "Method"),
        ("subscript", "Method"),
        ("Box", "Class"),
        ("run", "Function"),
        ("v", "Parameter"),
    ] {
        assert!(
            rows.iter().any(|(n, k)| n == name && k == kind),
            "swift must report `{name}` as {kind}; got {rows:?}"
        );
    }
    // `extension Array` is a class_declaration whose name is a `user_type`, which
    // is why the old rule made `@name` optional and emitted {"name": ""} at 1:1.
    assert!(
        rows.iter().any(|(n, k)| n == "Array" && k == "Class"),
        "swift must name the extended type; got {rows:?}"
    );
}

/// Kotlin's anonymous `companion object` reached the optional `@name` and was
/// emitted as `{"name": "", "kind": "class"}`.
#[test]
fn kotlin_companion_objects_are_named_or_absent() {
    let rows = symbols(
        "kt",
        Grammar::Kotlin,
        "class Outer {\n\
         \x20   companion object Named {\n\
         \x20       val a = 1\n\
         \x20   }\n\
         }\n\
         class Other {\n\
         \x20   companion object {\n\
         \x20       val b = 2\n\
         \x20   }\n\
         }\n",
    );
    assert_no_nameless(&rows);
    assert!(
        rows.iter().any(|(n, _)| n == "Named"),
        "kotlin must report a named companion object; got {rows:?}"
    );
}

/// PHP carried all three faults the row records, and one rule fixes all three:
/// the nameless top-level const, the initializer reported as a definition, and
/// `kind: "class"` for a constant.
#[test]
fn php_constants_are_named_kinded_and_exclude_initializers() {
    let rows = symbols(
        "php",
        Grammar::Php,
        "<?php\n\
         const TOP = 1;\n\
         const A = 1, B = 2;\n\
         class Config {\n\
         \x20   const FLAGS = JSON_UNESCAPED_SLASHES;\n\
         }\n",
    );
    assert_no_nameless(&rows);

    for name in ["TOP", "A", "B", "FLAGS"] {
        assert!(
            rows.iter().any(|(n, k)| n == name && k == "Constant"),
            "php must report `{name}` as Constant; got {rows:?}"
        );
    }
    assert!(
        !rows.iter().any(|(n, _)| n == "JSON_UNESCAPED_SLASHES"),
        "php must not report a const initializer as a definition; got {rows:?}"
    );
}

/// A guarded `def` is a symbol in both forms: with parentheses
/// (`def f(x) when x > 0`) and without (`def f when true, do: 1`).
#[test]
fn elixir_guarded_def_is_a_symbol() {
    let src = "defmodule M do\n  def f(x) when x > 0, do: x\n  def g when true, do: 1\n  \
               defp h(x) when is_integer(x), do: x\n  defp i when true, do: 1\n  \
               defmacro j(x) when is_atom(x), do: x\n  defmacro k when true, do: 1\nend\n";
    let file = SourceFile::from_content("probe.ex", src.to_string(), Grammar::Elixir)
        .expect("fixture must parse");
    let found = match execute_symbols(&file).expect("symbols query must run") {
        QueryResult::Symbols { symbols } => symbols
            .iter()
            .map(|s| (s.name().to_string(), s.range().start().line()))
            .collect::<Vec<_>>(),
        other => panic!("expected Symbols, got {other:?}"),
    };
    for (name, line) in [("f", 2), ("g", 3), ("h", 4), ("i", 5), ("j", 6), ("k", 7)] {
        assert!(
            found.contains(&(name.to_string(), line)),
            "`{name}` must be a symbol on line {line}; got {found:?}"
        );
    }
    assert!(
        !found.iter().any(|(n, _)| n == "x" || n == "true"),
        "the guard expression must not be captured; got {found:?}"
    );
}
