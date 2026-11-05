use act_lite_parser::{Grammar, QueryResult, SourceFile, execute_skeleton};

/// `(name, kind)` for every declaration the grammar's skeleton query emits.
fn skeleton(ext: &str, grammar: Grammar, src: &str) -> Vec<(String, String)> {
    let file = SourceFile::from_content(format!("probe.{ext}"), src.to_string(), grammar)
        .expect("fixture must parse");
    match execute_skeleton(&file).expect("skeleton query must run") {
        QueryResult::Skeleton(result) => result
            .declarations
            .iter()
            .map(|d| (d.name.clone(), d.kind.clone()))
            .collect(),
        other => panic!("expected Skeleton, got {other:?}"),
    }
}

fn assert_no_nameless(rows: &[(String, String)]) {
    let nameless: Vec<_> = rows.iter().filter(|(n, _)| n.is_empty()).collect();
    assert!(
        nameless.is_empty(),
        "nameless declaration(s) emitted: {nameless:?} (all rows: {rows:?})"
    );
}

fn assert_names_present(rows: &[(String, String)], expected: &[&str]) {
    let names: Vec<&str> = rows.iter().map(|(n, _)| n.as_str()).collect();
    for want in expected {
        assert!(
            names.contains(want),
            "expected declaration {want:?} missing from {names:?}"
        );
    }
}

const SQL_DDL: &str = concat!(
    "CREATE TABLE t_create (\n  id_col INT,\n  CONSTRAINT pk_create PRIMARY KEY (id_col)\n);\n",
    "CREATE VIEW v_create AS SELECT id_col FROM t_create;\n",
    "ALTER TABLE t_alter ADD COLUMN age INT;\n",
    "CREATE INDEX idx_create ON t_create (id_col);\n",
    "CREATE FUNCTION f_create() RETURNS INT AS $$ SELECT 1 $$ LANGUAGE sql;\n",
    "CREATE TRIGGER trg_create BEFORE INSERT ON t_create FOR EACH ROW EXECUTE PROCEDURE proc_create();\n",
    "DROP TABLE t_drop;\n",
    "DROP VIEW v_drop;\n",
    "DROP INDEX idx_drop;\n",
);

/// The DML half. These four statements have no name inside the captured node —
/// `select`'s `from` clause is a *sibling* under `statement` — so they are
/// relabelled with the statement keyword the grammar exposes.
const SQL_DML: &str = "SELECT id FROM users;\nINSERT INTO users (id) VALUES (1);\nUPDATE users SET id = 2;\nDELETE FROM users;\nWITH recent AS (SELECT id FROM users) SELECT id FROM recent;\n";

#[test]
fn sql_declarations_are_named() {
    let ddl = skeleton("sql", Grammar::Sql, SQL_DDL);
    assert_no_nameless(&ddl);
    assert_names_present(
        &ddl,
        &[
            "t_create",
            "v_create",
            "t_alter",
            "idx_create",
            "f_create",
            "trg_create",
            "id_col",
            "pk_create",
            "t_drop",
            "v_drop",
            "idx_drop",
        ],
    );

    let dml = skeleton("sql", Grammar::Sql, SQL_DML);
    assert_no_nameless(&dml);
    assert_names_present(&dml, &["SELECT", "INSERT", "UPDATE", "DELETE", "recent"]);
    assert_eq!(ddl.len(), 14, "{ddl:?}");
    assert_eq!(dml.len(), 7, "{dml:?}");
}

const HTML_SRC: &str = "<div id=\"a\"><span>x</span></div><img src=\"b\"/>";

#[test]
fn html_declarations_are_named() {
    let rows = skeleton("html", Grammar::Html, HTML_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["div", "a", "span", "img"]);
    assert_eq!(rows.len(), 4, "{rows:?}");
}

/// The two carrier rules are json/jsonc/json_schema's *entire* skeleton
/// query. Every key is listed, as `symbols` lists it: a key bound to an
/// object or array and a key bound to a scalar. The bare document root and
/// unkeyed array elements are not reported, so this fixture pins the count:
/// the keys are `a`, `x`, `b`, `c`.
const JSON_SRC: &str = r#"{"a": {"x": 1}, "b": [1, 2], "c": 1}"#;

#[test]
fn json_declarations_are_named() {
    let rows = skeleton("json", Grammar::Json, JSON_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["a", "x", "b", "c"]);
    assert_eq!(
        rows.len(),
        4,
        "every key is reported, not the root or unkeyed array elements: {rows:?}"
    );
}

const JSON_BLANK_KEY_SRC: &str = r#"{" ": {"z": 1}, "a": {"x": 1}}"#;

#[test]
fn json_blank_key_is_skipped() {
    let rows = skeleton("json", Grammar::Json, JSON_BLANK_KEY_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["z", "a", "x"]);
    assert_eq!(
        rows.len(),
        3,
        "the blank key must be skipped, not emitted nameless: {rows:?}"
    );
}

const PYTHON_SRC: &str = concat!(
    "\"\"\"Module summary.\n",
    "\n",
    "More detail.\n",
    "\"\"\"\n",
    "import os\n",
    "\n",
    "x = 1\n",
);

#[test]
fn python_declarations_are_named() {
    let rows = skeleton("py", Grammar::Python, PYTHON_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["Module summary.", "x"]);
    assert_eq!(rows.len(), 3, "{rows:?}");
}

/// A single-line docstring has no further lines to skip.
const PYTHON_SINGLE_LINE_DOCSTRING_SRC: &str = "\"\"\"Just one line.\"\"\"\nx = 1\n";

#[test]
fn python_single_line_docstring_is_named() {
    let rows = skeleton("py", Grammar::Python, PYTHON_SINGLE_LINE_DOCSTRING_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["Just one line."]);
    assert_eq!(rows.len(), 2, "{rows:?}");
}

/// The opening `"""` on its own line leaves a blank first line — the
/// "first NON-EMPTY line" rule must skip it rather than naming the
/// docstring the empty string.
const PYTHON_BLANK_FIRST_LINE_DOCSTRING_SRC: &str = "\"\"\"\nSummary here.\n\"\"\"\nx = 1\n";

#[test]
fn python_blank_first_line_docstring_skips_to_next_line() {
    let rows = skeleton("py", Grammar::Python, PYTHON_BLANK_FIRST_LINE_DOCSTRING_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["Summary here."]);
    assert_eq!(rows.len(), 2, "{rows:?}");
}

const PYTHON_BLANK_DOCSTRING_SRC: &str = "\"\"\"   \"\"\"\nx = 1\n";
const PYTHON_MULTILINE_BLANK_DOCSTRING_SRC: &str = "\"\"\"\n\"\"\"\nx = 1\n";

#[test]
fn python_blank_docstring_is_skipped() {
    let rows = skeleton("py", Grammar::Python, PYTHON_BLANK_DOCSTRING_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["x"]);
    assert_eq!(
        rows.len(),
        1,
        "the blank docstring must be skipped, not emitted nameless: {rows:?}"
    );
}

#[test]
fn python_multiline_blank_docstring_is_skipped() {
    let rows = skeleton("py", Grammar::Python, PYTHON_MULTILINE_BLANK_DOCSTRING_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["x"]);
    assert_eq!(rows.len(), 1, "{rows:?}");
}

/// A `using_declaration` covers two unrelated shapes with no common name
/// field: `using std::vector;` wraps a `qualified_identifier`, and
/// `using namespace std;` wraps a bare `identifier` directly. The
/// alternation must bind `@name` in both branches (`@using` is not one of
/// `execute_skeleton`'s five self-naming captures, so this genuinely needs
/// its own `@name`, unlike `@import`/`@use`).
const CPP_USING_SRC: &str = "using std::vector;\nusing namespace std;\n";

#[test]
fn cpp_declarations_are_named() {
    let rows = skeleton("cpp", Grammar::Cpp, CPP_USING_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["std::vector", "std"]);
    assert_eq!(rows.len(), 2, "{rows:?}");
}

const CSHARP_INDEXER_SRC: &str = "class C { public int this[int i] { get { return i; } set { } } }";

#[test]
fn csharp_declarations_are_named() {
    let rows = skeleton("cs", Grammar::CSharp, CSHARP_INDEXER_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["this"]);
    let indexer: Vec<_> = rows.iter().filter(|(_, k)| k == "indexer").collect();
    assert_eq!(
        indexer,
        vec![&("this".to_string(), "indexer".to_string())],
        "indexer must be named after its own `this` keyword: {rows:?}"
    );
}

/// `package_declaration` and `module_declaration` both expose their name
/// via `_name`, a hidden rule choosing between a single `identifier` (an
/// undotted name, e.g. `package foo;`) and a compound `scoped_identifier`
/// (e.g. `package com.example.app;`) — confirmed via `f285_node_dump`, since
/// the module rule's own prior comment ("compound names aren't captured")
/// was wrong only for the compound case and silent on the undotted one.
/// Both branches must bind `@name` or the undotted form silently drops out
/// of the skeleton entirely.
const JAVA_PACKAGE_SRC: &str = "package com.example.app;\nclass T {\n  static { int x = 1; }\n}\n";
const JAVA_PACKAGE_UNDOTTED_SRC: &str = "package foo;\nclass U {}\n";
const JAVA_MODULE_SRC: &str = "module com.example.app {\n  requires java.base;\n}\n";
const JAVA_MODULE_UNDOTTED_SRC: &str = "module app {\n}\n";

#[test]
fn java_declarations_are_named() {
    let rows = skeleton("java", Grammar::Java, JAVA_PACKAGE_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["com.example.app", "static"]);

    let undotted = skeleton("java", Grammar::Java, JAVA_PACKAGE_UNDOTTED_SRC);
    assert_no_nameless(&undotted);
    assert_names_present(&undotted, &["foo"]);

    let module = skeleton("java", Grammar::Java, JAVA_MODULE_SRC);
    assert_no_nameless(&module);
    assert_names_present(&module, &["com.example.app"]);

    let module_undotted = skeleton("java", Grammar::Java, JAVA_MODULE_UNDOTTED_SRC);
    assert_no_nameless(&module_undotted);
    assert_names_present(&module_undotted, &["app"]);
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert_eq!(undotted.len(), 2, "{undotted:?}");
    assert_eq!(module.len(), 1, "{module:?}");
    assert_eq!(module_undotted.len(), 1, "{module_undotted:?}");
}

fn skeleton_with_lines(ext: &str, grammar: Grammar, src: &str) -> Vec<(String, String, u32)> {
    let file = SourceFile::from_content(format!("probe.{ext}"), src.to_string(), grammar)
        .expect("fixture must parse");
    match execute_skeleton(&file).expect("skeleton query must run") {
        QueryResult::Skeleton(result) => result
            .declarations
            .iter()
            .map(|d| (d.name.clone(), d.kind.clone(), d.range.start().line()))
            .collect(),
        other => panic!("expected Skeleton, got {other:?}"),
    }
}

const PHP_CONST_SRC: &str = concat!(
    "<?php\n",
    "const A = 1, B = OTHER;\n",
    "class C {\n",
    "  const X = 1, Y = 2;\n",
    "}\n",
);

#[test]
fn php_declarations_are_named() {
    let rows = skeleton("php", Grammar::Php, PHP_CONST_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["A", "B", "X", "Y"]);
    let constants: Vec<_> = rows.iter().filter(|(_, k)| k == "constant").collect();
    assert_eq!(
        constants.len(),
        4,
        "expected one declaration per const_element, and no separate row for \
         B's initializer reference `OTHER`: {rows:?}"
    );
}

const SWIFT_INIT_SRC: &str = concat!("class C {\n", "  init() {}\n", "  init(x: Int) {}\n", "}\n",);

#[test]
fn swift_declarations_are_named() {
    let rows = skeleton("swift", Grammar::Swift, SWIFT_INIT_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["init"]);
    let inits: Vec<_> = rows.iter().filter(|(_, k)| k == "function").collect();
    assert_eq!(
        inits.len(),
        2,
        "expected one declaration per init_declaration: {rows:?}"
    );
    assert!(
        inits.iter().all(|(n, _)| n == "init"),
        "every initializer must be named after its own `init` keyword: {rows:?}"
    );
}

/// The row's single optional-binding carrier: `(type_identifier)? @name` is
/// de-optionalised to `(type_identifier) @name`, so a **named** companion
/// object is reported and an **anonymous** one is unreported rather than
/// nameless.
const KOTLIN_COMPANION_SRC: &str = concat!(
    "class C {\n",
    "  companion object Named {\n",
    "    val x = 1\n",
    "  }\n",
    "}\n",
    "class D {\n",
    "  companion object {\n",
    "    val y = 1\n",
    "  }\n",
    "}\n",
);

#[test]
fn kotlin_declarations_are_named() {
    let rows = skeleton("kt", Grammar::Kotlin, KOTLIN_COMPANION_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["Named"]);
    // C, D (classes) + Named (companion) = 3 "class"-kind rows; the
    // anonymous companion in D produces no row at all.
    let classes: Vec<_> = rows.iter().filter(|(_, k)| k == "class").collect();
    assert_eq!(
        classes.len(),
        3,
        "expected C, D and the named companion, and no row for the anonymous \
         companion: {rows:?}"
    );
}

const RUBY_ACCESSOR_SRC: &str = concat!(
    "class C\n",
    "  attr_reader :a, :b, :c\n",
    "  module_function :foo\n",
    "end\n",
);

#[test]
fn ruby_declarations_are_named() {
    let rows = skeleton("rb", Grammar::Ruby, RUBY_ACCESSOR_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["a", "b", "c", "module_function"]);
    assert!(
        rows.iter().all(|(n, _)| !n.starts_with(':')),
        "attr_reader symbol names must have their `:` sigil stripped: {rows:?}"
    );
    let properties: Vec<_> = rows.iter().filter(|(_, k)| k == "property").collect();
    assert_eq!(
        properties.len(),
        3,
        "expected one declaration per attr_reader symbol: {rows:?}"
    );
}

/// `defstruct` has no identifier — RELABEL, the `target: (identifier)`
/// capture (the call's own "defstruct" text) is renamed from `@defstruct` to
/// `@name`. A module attribute's own name (`@my_attr "value"`) is already
/// captured by the query and discarded — BIND renames `@attr_name` to
/// `@name` so the identifier it already matches becomes the declaration's
/// name (the `@` operator sigil is a sibling token, not part of the
/// identifier node's span, so no stripping is needed).
const ELIXIR_MODULE_SRC: &str = concat!(
    "defmodule M do\n",
    "  defstruct [:a, :b]\n",
    "  @my_attr \"value\"\n",
    "end\n",
);

#[test]
fn elixir_declarations_are_named() {
    let rows = skeleton("ex", Grammar::Elixir, ELIXIR_MODULE_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["defstruct", "my_attr"]);
    let constant: Vec<_> = rows
        .iter()
        .filter(|(n, k)| k == "constant" && n == "my_attr")
        .collect();
    assert_eq!(
        constant,
        vec![&("my_attr".to_string(), "constant".to_string())],
        "the module attribute must be named after its own identifier: {rows:?}"
    );
}

const MARKDOWN_CLOSING_SEQUENCE_ONLY_SRC: &str = "# ###\n## Real Heading\n";

#[test]
fn markdown_closing_sequence_only_heading_is_skipped() {
    let rows = skeleton("md", Grammar::Markdown, MARKDOWN_CLOSING_SEQUENCE_ONLY_SRC);
    assert_no_nameless(&rows);
    assert_names_present(&rows, &["Real Heading"]);
    assert_eq!(
        rows.len(),
        1,
        "the blank ATX heading must be skipped, not emitted nameless: {rows:?}"
    );
}

/// The full emitted list as `(kind, name)`, in emission order.
fn kind_name_rows(rows: &[(String, String)]) -> Vec<(&str, &str)> {
    rows.iter()
        .map(|(name, kind)| (kind.as_str(), name.as_str()))
        .collect()
}

/// CSS: all eight rules of `queries/css/skeleton.scm`.
///
/// Seven bound `@definition` and nothing else; the eighth, `(declaration)`,
/// emits each property (`color`, `opacity`, ...) as a `field` row after the
/// block that holds it, as `symbols.scm` lists it. Each is bound to a name:
/// `(selectors)` for a rule set (present on EVERY `rule_set`, where
/// `symbols.scm`'s finer `class_selector`/`id_selector`/`tag_name`
/// alternation leaves attribute, pseudo-class, universal and descendant
/// selectors unmatched), the first named child for `@media`/`@supports` (the
/// query itself), `(keyframes_name)`, the `(at_keyword)` for a generic
/// at-rule — whose identity *is* its keyword — and the string's
/// `(string_content)` for `@import`/`@charset`.
///
/// **Selector names keep their sigils** (`.btn`, `#main`): the sigil is what
/// distinguishes a class from an id of the same name. `@import`/`@charset`
/// names are unquoted because tree-sitter-css exposes a `string_content`
/// child; scss's `@use` below has none and therefore keeps its quotes.
///
/// The `.m` / `.s` rows are the nested rule sets inside `@media` / `@supports`:
/// `(rule_set)` is unanchored, so a nested block is a declaration in its own
/// right, which is what an outline of a stylesheet wants.
const CSS_SRC: &str = concat!(
    "@charset \"utf-8\";\n",
    "@import \"other.css\";\n",
    ".btn { color: red; }\n",
    "#main { color: blue; }\n",
    "div { color: green; }\n",
    ".btn, .x > y { color: teal; }\n",
    "@media screen and (min-width: 100px) { .m { color: red; } }\n",
    "@keyframes spin { from { opacity: 0; } }\n",
    "@supports (display: grid) { .s { display: grid; } }\n",
    "@font-face { font-family: \"X\"; }\n",
);

#[test]
fn css_skeleton_names_every_declaration() {
    let rows = skeleton("css", Grammar::Css, CSS_SRC);
    assert_no_nameless(&rows);
    assert_eq!(
        kind_name_rows(&rows),
        vec![
            ("constant", "utf-8"),
            ("import", "other.css"),
            ("class", ".btn"),
            ("field", "color"),
            ("class", "#main"),
            ("field", "color"),
            ("class", "div"),
            ("field", "color"),
            ("class", ".btn, .x > y"),
            ("field", "color"),
            ("class", "screen and (min-width: 100px)"),
            ("class", ".m"),
            ("field", "color"),
            ("class", "spin"),
            ("field", "opacity"),
            ("class", "(display: grid)"),
            ("class", ".s"),
            ("field", "display"),
            ("block", "@font-face"),
            ("field", "font-family"),
        ]
    );
}

/// SCSS: all six surviving rules of `queries/scss/skeleton.scm`.
///
/// Same BINDs as css for `rule_set` and `keyframes_statement`; `(declaration)`
/// takes `(property_name)`, which is `queries/scss/symbols.scm`'s rule
/// verbatim and covers both a SassScript variable (`$primary`) and an ordinary
/// property (`color`); `@mixin`/`@function` take their `name:` field; `@use`
/// takes the whole `(string_value)` — tree-sitter-scss exposes no
/// `string_content` child, so the name keeps its quotes.
///
/// The seventh rule, `(block) @definition`, is DROPped under R3: `block` is
/// the `{ … }` body of every rule set, mixin, function and at-rule; it has no
/// name and duplicates its owner's range, and every owner is already reported
/// by the six rules above. Measured on this exact fixture, it matched four
/// times — `{ color: $primary; }`, `{ color: red; }`, `{ @return $x; }`,
/// `{ opacity: 0; }` — and each owner appears below.
const SCSS_SRC: &str = concat!(
    "@use \"sass:math\";\n",
    "$primary: blue;\n",
    ".btn { color: $primary; }\n",
    "@mixin m { color: red; }\n",
    "@function f($x) { @return $x; }\n",
    "@keyframes k { from { opacity: 0; } }\n",
);

#[test]
fn scss_skeleton_names_every_declaration() {
    let rows = skeleton("scss", Grammar::Scss, SCSS_SRC);
    assert_no_nameless(&rows);
    assert_eq!(
        kind_name_rows(&rows),
        vec![
            ("import", "\"sass:math\""),
            ("field", "$primary"),
            ("class", ".btn"),
            ("field", "color"),
            ("function", "m"),
            ("field", "color"),
            ("function", "f"),
            ("class", "k"),
            ("field", "opacity"),
        ]
    );
}

/// HTML: the one surviving rule of `queries/html/skeleton.scm`.
///
/// `(document) @document` is DROPped (R3). **This drop has no behavioural
/// delta and cannot be RED here**: `@document` is in neither of
/// `execute_skeleton`'s vocabularies, so the rule emitted nothing before the
/// drop and nothing after. Its regression guard is
/// `skeleton_capture_vocabulary_audit::KNOWN_INERT`, whose `("Html", 0,
/// "document")` entry the drop removes. What this fixture pins is the other
/// half — that the file keeps a working rule, which is why html's `(document)`
/// could be dropped at all.
const HTML_T8_SRC: &str = "<div id=\"a\"><span>x</span></div><img src=\"b\"/>";

#[test]
fn html_skeleton_names_every_declaration() {
    let rows = skeleton("html", Grammar::Html, HTML_T8_SRC);
    assert_no_nameless(&rows);
    assert_eq!(
        kind_name_rows(&rows),
        vec![
            ("element", "div"),
            ("field", "a"),
            ("element", "span"),
            ("element", "img"),
        ]
    );
}

#[test]
fn dropped_comment_rules_leave_their_grammars_intact() {
    // (ext, grammar, source, names that must survive)
    let cases: &[(&str, Grammar, &str, &[&str])] = &[(
        "sql",
        Grammar::Sql,
        "-- a leading comment\nCREATE TABLE t_sql (c INT);\n",
        &["t_sql", "c"],
    )];

    for (ext, grammar, src, survivors) in cases {
        let rows = skeleton(ext, *grammar, src);
        assert_no_nameless(&rows);
        assert_names_present(&rows, survivors);
        assert!(
            rows.iter().all(|(n, _)| !n.contains("a leading comment")
                && !n.contains("a block comment")
                && !n.contains("A prose description line.")),
            "{grammar:?}: a comment must not be reported as a declaration: {rows:?}"
        );
    }
}

#[test]
fn unfielded_captures_do_not_report_referents() {
    // (ext, grammar, source, names that must be emitted, names that must not)
    type ReferentCase<'a> = (&'a str, Grammar, &'a str, &'a [&'a str], &'a [&'a str]);
    let cases: &[ReferentCase<'_>] = &[(
        "sql",
        Grammar::Sql,
        "WITH t_cte (col_a, col_b) AS (SELECT 1, 2) SELECT * FROM t_cte;\n",
        &["t_cte"],
        &["col_a", "col_b"],
    )];

    for (ext, grammar, src, wanted, forbidden) in cases {
        let rows = skeleton(ext, *grammar, src);
        assert_no_nameless(&rows);
        assert_names_present(&rows, wanted);
        for bad in *forbidden {
            assert!(
                rows.iter().all(|(n, _)| n != bad),
                "{grammar:?}: {bad:?} is a referent — declared elsewhere and merely \
                 mentioned here — and must not be emitted as a declaration: {rows:?}"
            );
        }
    }
}

#[test]
fn go_grouped_declarations_have_per_spec_ranges() {
    let rows = skeleton_with_lines(
        "go",
        Grammar::Go,
        concat!(
            "type (\n",
            "  A int\n",
            "  B string\n",
            ")\n",
            "const (\n",
            "  C = 1\n",
            "  D = 2\n",
            ")\n",
            "var x, y int\n",
        ),
    );
    assert_eq!(
        rows,
        vec![
            ("A".into(), "type_alias".into(), 2),
            ("B".into(), "type_alias".into(), 3),
            ("C".into(), "constant".into(), 6),
            ("D".into(), "constant".into(), 7),
            ("x".into(), "variable".into(), 9),
            ("y".into(), "variable".into(), 9),
        ]
    );
}

#[test]
fn elixir_plain_and_parameterized_type_aliases() {
    let mut rows = skeleton(
        "ex",
        Grammar::Elixir,
        "@type t :: integer()\n@opaque u(a) :: a\n@typep p :: atom()\n",
    );
    assert_no_nameless(&rows);
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ("p".into(), "type_alias".into()),
            ("t".into(), "type_alias".into()),
            ("u".into(), "type_alias".into()),
        ]
    );
}

#[test]
fn go_grouped_types_emit_one_kind_per_spec() {
    let rows = skeleton_with_lines(
        "go",
        Grammar::Go,
        concat!(
            "package p\n",
            "type (\n",
            "  Scalar int\n",
            "  S struct{}\n",
            "  I interface{ M() }\n",
            ")\n",
        ),
    );
    assert_eq!(
        rows,
        vec![
            ("p".into(), "package".into(), 1),
            ("Scalar".into(), "type_alias".into(), 3),
            ("S".into(), "struct".into(), 4),
            ("I".into(), "interface".into(), 5),
        ],
        "each grouped type spec must emit once, with its specialized kind and own range"
    );
}
