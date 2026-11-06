//! Relationship extraction: the import-like constructs of JavaScript, JSX,
//! TypeScript, TSX, Python, Go and Rust, and the module-level names that
//! resolution checks.
//!
//! Every expected value below was written by hand from the fixture source.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use act_lite_core::CompactRange;
use act_lite_parser::deps::{
    self, BlockModule, InlineModule, InvalidLiteral, LiteralValue, ModuleNames, Relationship,
    RelationshipKind, UseBinding,
};
use act_lite_parser::{Grammar, SourceFile};

use RelationshipKind::*;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/deps")
}

fn fixture(path: &str) -> SourceFile {
    let file = SourceFile::load(fixture_dir().join(path)).unwrap();
    assert!(
        !file.tree().root_node().has_error(),
        "{path} does not parse cleanly"
    );
    file
}

fn parse(grammar: Grammar, source: &str) -> SourceFile {
    SourceFile::from_content("test", source.to_string(), grammar).unwrap()
}

fn range(text: &str) -> CompactRange {
    let n: Vec<u32> = text.split(['-', ':']).map(|p| p.parse().unwrap()).collect();
    CompactRange::new(n[0], n[1], n[2], n[3])
}

fn rel(kind: RelationshipKind, specifier: &str, at: &str) -> Relationship {
    Relationship {
        kind,
        specifier: specifier.to_string(),
        value: Ok(specifier.to_string()),
        literal: true,
        range: range(at),
        imported: None,
        path_attribute: None,
        in_macro: false,
    }
}

fn from_import(specifier: &str, imported: &str, at: &str) -> Relationship {
    Relationship {
        imported: Some(imported.to_string()),
        ..rel(FromImport, specifier, at)
    }
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|n| n.to_string()).collect()
}

// The node kinds of the extraction table, per family, copied by hand.
const JS_KINDS: &[&str] = &[
    "import_statement",
    "export_statement",
    "call_expression",
    "import",
    "string",
    "template_string",
    "template_substitution",
];
const TS_KINDS: &[&str] = &["import_require_clause"];
const PYTHON_KINDS: &[&str] = &[
    "import_statement",
    "import_from_statement",
    "dotted_name",
    "aliased_import",
    "relative_import",
    "import_prefix",
    "wildcard_import",
    "future_import_statement",
];
const GO_KINDS: &[&str] = &[
    "import_declaration",
    "import_spec",
    "interpreted_string_literal",
    "raw_string_literal",
];
const RUST_KINDS: &[&str] = &[
    "mod_item",
    "attribute_item",
    "use_declaration",
    "scoped_use_list",
    "use_list",
    "use_wildcard",
    "use_as_clause",
    "extern_crate_declaration",
    "macro_invocation",
    "token_tree",
];

#[test]
fn every_table_node_kind_is_in_its_grammar() {
    let rows: [(Grammar, Vec<&str>); 7] = [
        (Grammar::JavaScript, JS_KINDS.to_vec()),
        (Grammar::Jsx, JS_KINDS.to_vec()),
        (Grammar::TypeScript, [JS_KINDS, TS_KINDS].concat()),
        (Grammar::Tsx, [JS_KINDS, TS_KINDS].concat()),
        (Grammar::Python, PYTHON_KINDS.to_vec()),
        (Grammar::Go, GO_KINDS.to_vec()),
        (Grammar::Rust, RUST_KINDS.to_vec()),
    ];
    let mut failures = Vec::new();
    for (grammar, kinds) in rows {
        let language = grammar.language_support().ts_language();
        for kind in kinds {
            if tree_sitter::Query::new(&language, &format!("({kind}) @probe")).is_err() {
                failures.push(format!("{grammar:?}: {kind} is not in the grammar"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn kinds_have_their_spellings() {
    let kinds = [
        (DynamicImport, "dynamic_import"),
        (ExternCrate, "extern_crate"),
        (FromImport, "from_import"),
        (Import, "import"),
        (Include, "include"),
        (Module, "module"),
        (Reexport, "reexport"),
        (Require, "require"),
        (TypeImport, "type_import"),
        (Use, "use"),
    ];
    for (kind, spelling) in kinds {
        assert_eq!(kind.as_str(), spelling);
    }
}

// The `ts`, `py`, `go` and `rs` fixtures: kind, specifier, start line, literal.
type Row = (&'static str, &'static str, u32, bool);

const FIXTURES: &[(&str, &[Row])] = &[
    (
        "ts/src/main.ts",
        &[
            ("import", "./a", 1, true),
            ("import", "@lib/b", 2, true),
            ("import", "node:fs", 3, true),
            ("import", "react", 4, true),
            ("require", "./c.js", 5, true),
            ("reexport", "./d", 6, true),
            ("type_import", "./types", 7, true),
            ("dynamic_import", "./missing", 8, true),
            ("require", "name", 9, false),
        ],
    ),
    ("ts/src/a.ts", &[("import", "./main", 1, true)]),
    ("ts/src/lib/b.ts", &[]),
    ("ts/src/c.ts", &[]),
    ("ts/src/d/index.ts", &[]),
    ("ts/src/types.d.ts", &[]),
    (
        "ts/src/view.tsx",
        &[
            ("import", "./style.css", 1, true),
            ("import", "./a.js", 2, true),
        ],
    ),
    ("ts/src/a.test.ts", &[("import", "./a", 1, true)]),
    ("py/app/__init__.py", &[]),
    (
        "py/app/main.py",
        &[
            ("import", "os", 1, true),
            ("from_import", "app.util", 2, true),
            ("from_import", ".models.User", 3, true),
            ("from_import", ".helpers", 4, true),
            ("import", "app.missing", 5, true),
            ("import", "requests", 6, true),
        ],
    ),
    ("py/app/util.py", &[]),
    ("py/app/models/__init__.py", &[]),
    ("py/app/helpers.py", &[("from_import", ".util.x", 1, true)]),
    ("py/app/uses_lib.py", &[("import", "lib", 1, true)]),
    ("py/scripts/run.py", &[("import", "tool", 1, true)]),
    ("py/scripts/tool.py", &[]),
    ("py/src/lib/__init__.py", &[]),
    (
        "go/main.go",
        &[
            ("import", "fmt", 4, true),
            ("import", "example.com/shop/store", 5, true),
            ("import", "example.com/shop/gone", 6, true),
        ],
    ),
    (
        "go/store/store.go",
        &[("import", "example.com/shop/store/db", 3, true)],
    ),
    ("go/store/cart.go", &[]),
    ("go/store/store_test.go", &[]),
    ("go/libs/db/db.go", &[]),
    (
        "go/tools/gen.go",
        &[("import", "example.com/shop/store", 3, true)],
    ),
    (
        "rs/src/lib.rs",
        &[
            ("module", "net", 1, true),
            ("module", "util", 2, true),
            ("module", "gone", 3, true),
            ("use", "crate::net::http::Client", 4, true),
            ("use", "serde::Serialize", 5, true),
            ("use", "std::fmt", 6, true),
            ("use", "mystery::Thing", 7, true),
        ],
    ),
    ("rs/src/net/mod.rs", &[("module", "http", 1, true)]),
    (
        "rs/src/net/http.rs",
        &[("use", "super::super::util::helper", 1, true)],
    ),
    ("rs/src/util.rs", &[]),
    (
        "rs/src/main.rs",
        &[("use", "rs_fixture::util::helper", 1, true)],
    ),
    (
        "rs/src/extra.rs",
        &[("use", "crate::util::helper", 1, true)],
    ),
];

#[test]
fn design_fixtures_give_their_relationships() {
    let mut failures = Vec::new();
    for (path, expected) in FIXTURES {
        let expected: Vec<(&str, String, u32, bool)> = expected
            .iter()
            .map(|&(kind, specifier, line, literal)| (kind, specifier.to_string(), line, literal))
            .collect();
        let actual: Vec<(&str, String, u32, bool)> = deps::extract(&fixture(path))
            .into_iter()
            .map(|r| {
                (
                    r.kind.as_str(),
                    r.specifier,
                    r.range.start_line(),
                    r.literal,
                )
            })
            .collect();
        if actual != expected {
            failures.push(format!(
                "{path}:\n expected {expected:?}\n actual   {actual:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn ranges_are_the_construct_ranges() {
    let main = deps::extract(&fixture("ts/src/main.ts"));
    let require = main.iter().find(|r| r.specifier == "./c.js").unwrap();
    assert_eq!(require.range.to_string(), "5:11-5:28");
    let import = main.iter().find(|r| r.specifier == "./a").unwrap();
    assert_eq!(import.range.to_string(), "1:1-1:25");
    let go = deps::extract(&fixture("go/main.go"));
    assert_eq!(go[1].range.to_string(), "5:2-5:26");
}

// The language-specific forms, one workspace per family.
fn forms() -> Vec<(&'static str, Vec<Relationship>)> {
    vec![
        ("forms-js/src/x.js", vec![rel(Require, "./y", "1:11-1:25")]),
        ("forms-js/src/x.jsx", vec![rel(Reexport, "./y", "1:1-1:21")]),
        ("forms-js/src/t.ts", vec![rel(Require, "./y", "1:1-1:27")]),
        (
            "forms-js/src/u.ts",
            vec![rel(DynamicImport, "./y", "1:11-1:24")],
        ),
        (
            "forms-js/src/v.ts",
            vec![rel(Import, "../../outside", "1:1-1:24")],
        ),
        (
            "forms-js/src/w.ts",
            vec![rel(Import, "#internal", "1:1-1:20")],
        ),
        (
            "forms-js/src/r.ts",
            vec![rel(Import, "/src/y.js", "1:1-1:20")],
        ),
        ("forms-js/src/q.ts", vec![rel(Import, "p", "1:1-1:12")]),
        ("forms-js/src/q2.ts", vec![rel(Import, "e", "1:1-1:12")]),
        ("forms-js/cfg/m.ts", vec![rel(Import, "@t/a", "1:1-1:15")]),
        ("forms-js/ext/m.ts", vec![rel(Import, "@b/k", "1:1-1:15")]),
        ("forms-py/pkg/__init__.py", vec![]),
        (
            "forms-py/pkg/a.py",
            vec![from_import("..x", "x", "1:1-1:17")],
        ),
        ("forms-py/top.py", vec![from_import(".y", "y", "1:1-1:16")]),
        (
            "forms-py/pkg/b.py",
            vec![
                rel(Import, "os", "1:1-1:17"),
                rel(Import, "pkg.a", "1:1-1:17"),
            ],
        ),
        (
            "forms-py/pkg/c.py",
            vec![from_import("pkg.*", "*", "1:1-1:18")],
        ),
        ("forms-py/pkg/d.py", vec![rel(Import, "ns.mod", "1:1-1:14")]),
        (
            "forms-py/pkg/e.py",
            vec![from_import("pkg.gone", "gone", "1:1-1:21")],
        ),
        ("forms-py/pkg/f.py", vec![rel(Import, "yaml", "1:1-1:12")]),
        ("forms-py/pkg/g.py", vec![rel(Import, "lib.x", "1:1-1:13")]),
        (
            "forms-go/main.go",
            vec![
                rel(Import, "C", "4:2-4:5"),
                rel(Import, "example.com/f/p", "5:2-5:19"),
                rel(Import, "example.com/f/q", "6:2-6:21"),
                rel(Import, "example.com/f/r", "7:2-7:21"),
                rel(Import, "example.com/f/s", "8:2-8:21"),
                rel(Import, "example.com/ext", "9:2-9:19"),
            ],
        ),
        (
            "forms-rs/src/lib.rs",
            vec![
                rel(Module, "net", "1:1-1:9"),
                Relationship {
                    path_attribute: Some(Ok("other/place.rs".to_string())),
                    ..rel(Module, "moved", "3:1-3:11")
                },
                Relationship {
                    in_macro: true,
                    ..rel(Module, "gated", "4:16-4:30")
                },
                rel(Use, "net", "5:1-5:32"),
                rel(Use, "net::Conn", "5:1-5:32"),
                rel(ExternCrate, "alloc", "6:1-6:20"),
                rel(Include, "data.txt", "7:18-7:42"),
            ],
        ),
        ("forms-rs/src/net.rs", vec![rel(Module, "http", "1:1-1:14")]),
        (
            "forms-rs/src/other/place.rs",
            vec![rel(Module, "inner", "1:1-1:11")],
        ),
        (
            "forms-rs/src/main.rs",
            vec![rel(Use, "frs::net::Conn", "1:1-1:20")],
        ),
    ]
}

#[test]
fn language_specific_forms_give_their_relationships() {
    let mut failures = Vec::new();
    for (path, expected) in forms() {
        let actual = deps::extract(&fixture(path));
        if actual != expected {
            failures.push(format!(
                "{path}:\n expected {expected:#?}\n actual   {actual:#?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn use_trees_give_one_relationship_per_leaf() {
    let file = parse(
        Grammar::Rust,
        "use a::{b, c::d};\nuse e::*;\nuse f::g as h;\nuse ::i::j;\nuse k::{self as l, m::{*, n}};\n",
    );
    let specifiers: Vec<(String, u32)> = deps::extract(&file)
        .into_iter()
        .map(|r| (r.specifier, r.range.start_line()))
        .collect();
    let expected = [
        ("a::b", 1),
        ("a::c::d", 1),
        ("e", 2),
        ("f::g", 3),
        ("::i::j", 4),
        ("k", 5),
        ("k::m", 5),
        ("k::m::n", 5),
    ];
    let expected: Vec<(String, u32)> = expected.iter().map(|(s, l)| (s.to_string(), *l)).collect();
    assert_eq!(specifiers, expected);
}

#[test]
fn macro_token_forms_are_relationships() {
    let file = parse(
        Grammar::Rust,
        "cfg_rt! {\n    pub(crate) use a::{B, C as D};\n    outer! { mod n; }\n    struct S;\n}\n",
    );
    let expected = vec![
        Relationship {
            in_macro: true,
            ..rel(Use, "a::B", "2:5-2:35")
        },
        Relationship {
            in_macro: true,
            ..rel(Use, "a::C", "2:5-2:35")
        },
        Relationship {
            in_macro: true,
            ..rel(Module, "n", "3:14-3:20")
        },
    ];
    assert_eq!(deps::extract(&file), expected);
}

#[test]
fn a_path_attribute_inside_a_macro_applies_to_the_next_mod() {
    let file = parse(
        Grammar::Rust,
        "cfg_x! {\n    #[path = \"other.rs\"]\n    mod x;\n    #[cfg(unix)]\n    #[path = r\"sys/unix.rs\"]\n    pub(crate) mod sys;\n    #[path = \"lost.rs\"]\n    struct S;\n    mod y;\n}\n",
    );
    let expected = vec![
        Relationship {
            path_attribute: Some(Ok("other.rs".to_string())),
            in_macro: true,
            ..rel(Module, "x", "3:5-3:11")
        },
        Relationship {
            path_attribute: Some(Ok("sys/unix.rs".to_string())),
            in_macro: true,
            ..rel(Module, "sys", "6:5-6:24")
        },
        Relationship {
            in_macro: true,
            ..rel(Module, "y", "9:5-9:11")
        },
    ];
    assert_eq!(deps::extract(&file), expected);
}

#[test]
fn comments_inside_a_macro_keep_a_path_attribute() {
    let file = parse(
        Grammar::Rust,
        r#"wrapper! {
    #[path = "other.rs"] /* note */ mod x;
    #[path = "y.rs"] // c
    mod /* a */ y // b
    ;
    #[path = "z.rs"]
    /// doc
    mod z;
}
"#,
    );
    let expected = vec![
        Relationship {
            path_attribute: Some(Ok("other.rs".to_string())),
            in_macro: true,
            ..rel(Module, "x", "2:37-2:43")
        },
        Relationship {
            path_attribute: Some(Ok("y.rs".to_string())),
            in_macro: true,
            ..rel(Module, "y", "4:5-5:6")
        },
        Relationship {
            path_attribute: Some(Ok("z.rs".to_string())),
            in_macro: true,
            ..rel(Module, "z", "8:5-8:11")
        },
    ];
    assert_eq!(deps::extract(&file), expected);
}

#[test]
fn computed_specifiers_are_kept_with_their_text() {
    let rust = parse(
        Grammar::Rust,
        "include!(concat!(env!(\"OUT_DIR\"), \"/x.rs\"));\n",
    );
    let expected = Relationship {
        literal: false,
        ..rel(Include, "concat!(env!(\"OUT_DIR\"), \"/x.rs\")", "1:1-1:44")
    };
    assert_eq!(deps::extract(&rust), vec![expected]);

    let js = parse(Grammar::JavaScript, "import(`./a${b}`);\n");
    let expected = Relationship {
        literal: false,
        ..rel(DynamicImport, "`./a${b}`", "1:1-1:18")
    };
    assert_eq!(deps::extract(&js), vec![expected]);
}

#[test]
fn type_keyword_on_a_specifier_is_a_plain_import() {
    let file = parse(Grammar::TypeScript, "import { type U } from './u';\n");
    assert_eq!(deps::extract(&file), vec![rel(Import, "./u", "1:1-1:30")]);
}

#[test]
fn a_file_with_a_parse_error_still_gives_its_relationships() {
    let file = parse(Grammar::TypeScript, "import { x } from './q';\nconst = ;\n");
    assert!(file.tree().root_node().has_error());
    assert_eq!(deps::extract(&file), vec![rel(Import, "./q", "1:1-1:25")]);
}

#[test]
fn forms_outside_the_table_are_not_relationships() {
    let python = parse(
        Grammar::Python,
        "from __future__ import annotations\nm = importlib.import_module(\"a\")\nn = __import__(\"b\")\n",
    );
    assert_eq!(deps::extract(&python), vec![]);
    let js = parse(
        Grammar::JavaScript,
        "const u = new URL('./x', import.meta.url);\n",
    );
    assert_eq!(deps::extract(&js), vec![]);
}

#[test]
fn other_grammars_give_no_relationship() {
    let cases = [
        (Grammar::Java, "import a.b.C;\n"),
        (Grammar::C, "#include \"x.h\"\n"),
        (Grammar::Ruby, "require 'x'\n"),
        (Grammar::CSharp, "using A.B;\n"),
    ];
    for (grammar, source) in cases {
        assert_eq!(
            deps::extract(&parse(grammar, source)),
            vec![],
            "{grammar:?}"
        );
    }
}

#[test]
fn python_names_bound_at_top_level() {
    let file = parse(
        Grammar::Python,
        "a, b = 1, 2\n\
         global n\n\
         try:\n    import t.u\nexcept ImportError:\n    t = None\n\
         def f():\n    inner = 1\n\
         class K:\n    attr = 1\n\
         if True:\n    c: int = 3\n\
         import p.q as r\n\
         from x import y as z, w\n",
    );
    let expected = ModuleNames {
        names: names(&["K", "a", "b", "c", "f", "r", "t", "w", "z"]),
        glob: false,
        inline: vec![],
        blocks: vec![],
        uses: vec![],
        files: vec![],
    };
    assert_eq!(deps::module_names(&file), expected);

    let star = parse(Grammar::Python, "from x import *\n");
    assert!(deps::module_names(&star).glob);
}

#[test]
fn python_annotation_without_a_value_binds_nothing() {
    let file = parse(
        Grammar::Python,
        "n: int\n\
         m: int = 1\n\
         if True:\n    o: str\n    p: str = ''\n",
    );
    assert_eq!(deps::module_names(&file).names, names(&["m", "p"]));
}

#[test]
fn python_loop_targets_and_with_aliases_bound_at_top_level() {
    let file = parse(
        Grammar::Python,
        "for i in x:\n    pass\n\
         for (j, [k, *l]) in y:\n    pass\n\
         for m, o in z:\n    pass\n\
         with c() as p, d() as (q, r), e():\n    pass\n\
         with (f() as s, g() as [t, u]):\n    pass\n\
         def h():\n    for inner in x:\n        pass\n",
    );
    assert_eq!(
        deps::module_names(&file).names,
        names(&[
            "h", "i", "j", "k", "l", "m", "o", "p", "q", "r", "s", "t", "u"
        ])
    );
}

#[test]
fn python_match_captures_and_case_bodies_bound_at_top_level() {
    let file = parse(
        Grammar::Python,
        r#"match x:
    case _:
        n = 1
    case [a, (b, *c)] if g:
        pass
    case P(k=d, j={'key': e, **f}):
        pass
    case Color.RED | Q(h):
        pass
    case _ as i:
        pass
    case [*_]:
        pass
    case o:
        def p():
            inner = 1
        class C:
            attr = 2
if True:
    match z:
        case s:
            pass
def q():
    match y:
        case r:
            pass
"#,
    );
    assert_eq!(
        deps::module_names(&file).names,
        names(&[
            "C", "a", "b", "c", "d", "e", "f", "h", "i", "n", "o", "p", "q", "s"
        ])
    );
}

#[test]
fn python_assignment_expressions_bound_at_module_scope() {
    // A `:=` binds in the module wherever the module runs it, also in a
    // list, set or dict comprehension, a default value, a decorator or a
    // base class; not in a function, lambda or class body, and not in a
    // generator expression, whose body runs only when it is iterated.
    let file = parse(
        Grammar::Python,
        r#"if (a := 1): pass
print((b := 2))
[(cv := x) for x in [1] if (d := x)]
{(sc := x) for x in [1]}
{(dk := x): 1 for x in [1]}
gen = ((gb := x) for x in [1] if (gf := x))
print(sum((ga := x) for x in [1]))
f = lambda p=(g := 1): (h := 2)
@(i := deco)
def j(q=(k := 1)):
    m = (o := 1)
class C(r := object):
    s = (t := 1)
match z:
    case _ if (u := 1):
        v = (w := 1)
while (aa := 0): pass
with (bb := c()): pass
for cc in (dd := [1]): pass
ee = [1]
ee[(ff := 0)] = 2
assert (gg := 1)
def fn():
    (inner := 1)
"#,
    );
    assert_eq!(
        deps::module_names(&file).names,
        names(&[
            "C", "a", "aa", "b", "bb", "cc", "cv", "d", "dd", "dk", "ee", "f", "ff", "fn", "g",
            "gen", "gg", "i", "j", "k", "r", "sc", "u", "v", "w"
        ])
    );
}

#[test]
fn python_names_still_bound_after_the_module_runs() {
    // An augmented assignment and a `type` alias bind; an `except … as`
    // name is deleted at the end of its handler; a module-level `del`
    // unbinds, a `del` inside a block may not run.
    let file = parse(
        Grammar::Python,
        r#"total = 0
total += 1
int |= str
type Alias = int
type Gen[T] = list[T]
try:
    raise ValueError
except ValueError as err:
    err = 1
    kept = 1
try:
    raise ExceptionGroup("g", [ValueError()])
except* ValueError as group:
    group = 1
try:
    held = 1
except ValueError as held:
    pass
gone = 1
del gone
x1 = x2 = x3 = [1]
del (x1), [x2], x3[0]
back = 1
del back
back = 2
maybe = 1
if False:
    del maybe
"#,
    );
    assert_eq!(
        deps::module_names(&file).names,
        names(&[
            "Alias", "Gen", "back", "held", "int", "kept", "maybe", "total", "x3"
        ])
    );
}

#[test]
fn rust_names_declared_at_module_level() {
    let file = parse(
        Grammar::Rust,
        r#"use a::B;
pub use a::C as D;
pub use e::*;
fn f() { let x = 1; }
struct S { field: u8 }
impl S { fn method(&self) {} }
enum E { V }
trait T { fn tm(); }
type A = u8;
const K: u8 = 1;
static G: u8 = 1;
macro_rules! mac { () => {} }
extern crate alloc as al;
cfg_rt! { pub struct R; pub(crate) mod gm; pub use x::{Y, Z as W}; impl R { fn m2() {} } }
#[path = "p.rs"]
mod inl {
    pub fn g() {}
    mod deeper {
        fn h() {}
    }
}
mod outl;
"#,
    );
    let expected = ModuleNames {
        names: names(&[
            "A", "B", "D", "E", "G", "K", "R", "S", "T", "W", "Y", "al", "f", "gm", "inl", "mac",
        ]),
        glob: true,
        inline: vec![InlineModule {
            name: "inl".to_string(),
            range: range("16:1-21:2"),
            path_attribute: Some(Ok("p.rs".to_string())),
            names: ModuleNames {
                names: names(&["deeper", "g"]),
                glob: false,
                inline: vec![InlineModule {
                    name: "deeper".to_string(),
                    range: range("18:5-20:6"),
                    path_attribute: None,
                    names: ModuleNames {
                        names: names(&["h"]),
                        glob: false,
                        inline: vec![],
                        blocks: vec![],
                        uses: vec![],
                        files: vec![],
                    },
                }],
                blocks: vec![],
                uses: vec![],
                files: vec![],
            },
        }],
        blocks: vec![],
        uses: vec![],
        files: vec![],
    };
    assert_eq!(deps::module_names(&file), expected);

    let wrapped = parse(Grammar::Rust, "cfg_rt! { struct S; }\n");
    assert_eq!(deps::module_names(&wrapped).names, names(&["S"]));
    let reexport = parse(Grammar::Rust, "pub use a::B;\n");
    assert_eq!(deps::module_names(&reexport).names, names(&["B"]));
}

#[test]
fn the_blocks_of_a_body_are_scopes_outside_the_module() {
    let file = parse(
        Grammar::Rust,
        r#"fn f() {
    use a::B;
    struct L;
    mod inner {
        pub struct I;
        fn g() {
            const C: u8 = 0;
        }
    }
    {
        {
            fn h() {}
        }
    }
}
impl S {
    fn m() {
        let x = 1;
    }
}
"#,
    );
    let names_of = |list: &[&str], inline, blocks| ModuleNames {
        names: names(list),
        glob: false,
        inline,
        blocks,
        uses: vec![],
        files: vec![],
    };
    let block = |at: &str, names| BlockModule {
        range: range(at),
        names,
    };
    let expected = names_of(
        &["f"],
        vec![],
        vec![block(
            "1:8-15:2",
            ModuleNames {
                uses: vec![UseBinding {
                    name: Some("B".to_string()),
                    path: "a::B".to_string(),
                    range: range("2:5-2:14"),
                }],
                ..names_of(
                    &["L", "inner"],
                    vec![InlineModule {
                        name: "inner".to_string(),
                        range: range("4:5-9:6"),
                        path_attribute: None,
                        names: names_of(
                            &["I", "g"],
                            vec![],
                            vec![block("6:16-8:10", names_of(&["C"], vec![], vec![]))],
                        ),
                    }],
                    vec![block("11:9-13:10", names_of(&["h"], vec![], vec![]))],
                )
            },
        )],
    );
    assert_eq!(
        deps::module_names(&file),
        expected,
        "a `use` in a block declares no name but a binding; a block without items gives its inner blocks"
    );
}

#[test]
fn the_use_leaves_of_a_body_are_bindings_of_their_scope() {
    let file = parse(
        Grammar::Rust,
        "use top::T;\nfn f() {\n    use a::{b::C, d as E, f as _};\n    use g::*;\n    mod inner {\n        use crate::h as r#I;\n    }\n}\n",
    );
    let binding = |name: Option<&str>, path: &str, at: &str| UseBinding {
        name: name.map(str::to_string),
        path: path.to_string(),
        range: range(at),
    };
    let module = deps::module_names(&file);
    assert_eq!(module.uses, [], "a module outside a body keeps no bindings");
    let [block] = module.blocks.as_slice() else {
        panic!("one block: {module:?}");
    };
    assert_eq!(block.names.names, names(&["inner"]));
    assert_eq!(
        block.names.uses,
        [
            binding(Some("C"), "a::b::C", "3:5-3:35"),
            binding(Some("E"), "a::d", "3:5-3:35"),
            binding(None, "g", "4:5-4:14"),
        ]
    );
    let inner = &block.names.inline[0].names;
    assert_eq!(
        inner.names,
        names(&["I"]),
        "a module declares its `use` leaves"
    );
    assert_eq!(
        inner.uses,
        [binding(Some("I"), "crate::h", "6:9-6:29")],
        "an inline module in a body keeps its bindings"
    );
}

#[test]
fn other_grammars_declare_no_names() {
    let file = parse(Grammar::Go, "package p\nfunc F() {}\n");
    assert_eq!(deps::module_names(&file), ModuleNames::default());
}

#[test]
fn mods_in_a_removed_region() {
    assert_eq!(deps::mods_in_text("#[cfg(test)] mod tests;"), vec!["tests"]);
}

#[test]
fn python_assignment_expressions_in_del_targets_bind_in_order() {
    // A `del` runs its targets from left to right: a `:=` in a subscript or
    // an attribute target binds, and a name target unbinds what is bound
    // at that point.
    let cases = [
        ("items = [1]\ndel items[(n := 0)]\n", true),
        ("n = 1\nitems = [1]\ndel n, items[(n := 0)]\n", true),
        ("items = [1]\ndel items[(n := 0)], n\n", false),
        ("items = [1]\ndel (items[(n := 0)], n)\n", false),
        ("n = 1\nitems = [1]\ndel [n, items[(n := 0)]]\n", true),
        ("objs = {}\ndel objs[(n := 0)].x\n", true),
        ("items = [1, 2]\ndel items[(n := 0):(m := 1)]\n", true),
    ];
    for (source, bound) in cases {
        let file = parse(Grammar::Python, source);
        assert_eq!(
            deps::module_names(&file).names.contains("n"),
            bound,
            "{source}"
        );
    }
    let slice = parse(
        Grammar::Python,
        "items = [1, 2]\ndel items[(n := 0):(m := 1)]\n",
    );
    assert_eq!(
        deps::module_names(&slice).names,
        names(&["items", "m", "n"])
    );
}

#[test]
fn python_assignment_expressions_in_every_module_level_statement() {
    // Each `:=` below runs at module scope (CPython 3.13; 3.14 refuses one
    // in an annotation), so `n` is bound after the module runs. An `except`
    // expression's `:=` stays bound when its handler does not match.
    let cases = [
        "x: (n := int) = 1\n",
        "x: (n := int)\n",
        "def f(p: (n := int)): pass\n",
        "def f() -> (n := int): pass\n",
        "f = lambda p=(n := 1): p\n",
        "class C(metaclass=(n := type)): pass\n",
        "assert (n := 1)\n",
        "try:\n    raise (n := ValueError)\nexcept ValueError:\n    pass\n",
        "try:\n    raise KeyError\nexcept (n := KeyError):\n    pass\n",
        "try:\n    raise KeyError\nexcept (n := ValueError) as n:\n    pass\nexcept KeyError:\n    pass\n",
        "try:\n    raise ExceptionGroup('g', [KeyError()])\nexcept* (n := KeyError):\n    pass\n",
        "match (n := 1):\n    case _: pass\n",
        "if 0: pass\nelif (n := 1): pass\n",
        "global n\n(n := 1)\n",
        "items = [1]\nif True:\n    del items[(n := 0)]\n",
        "d = {0: 0}\nd[(n := 0)] += 1\n",
        "d = {}\nfor d[(n := 0)] in [1]: pass\n",
        "with c() as d[(n := 0)]: pass\n",
    ];
    for source in cases {
        let file = parse(Grammar::Python, source);
        assert!(deps::module_names(&file).names.contains("n"), "{source}");
    }
    // The handler deletes its alias after its body binds it.
    let handler = parse(
        Grammar::Python,
        "try:\n    raise KeyError\nexcept KeyError as n:\n    (n := 1)\n",
    );
    assert!(!deps::module_names(&handler).names.contains("n"));
}

#[test]
fn rust_raw_identifiers_are_evidence_as_written() {
    let file = parse(
        Grammar::Rust,
        "mod r#type;\nuse crate::r#type::r#Foo;\nextern crate r#core as r#cc;\nw! { mod r#m; use r#type::{r#A}; }\n",
    );
    let relationships = deps::extract(&file);
    let specifiers: Vec<&str> = relationships.iter().map(|r| r.specifier.as_str()).collect();
    assert_eq!(
        specifiers,
        [
            "r#type",
            "crate::r#type::r#Foo",
            "r#core",
            "r#m",
            "r#type::r#A"
        ]
    );
}

#[test]
fn rust_raw_identifiers_declare_their_names() {
    let file = parse(
        Grammar::Rust,
        r#"fn r#match() {}
struct r#Foo;
macro_rules! r#mm { () => {} }
extern crate core as r#cc;
use a::r#B;
use a::C as r#D;
mod r#inl {
    fn r#f() {}
}
w! { struct r#S; mod r#gm; use x::r#Y; macro_rules! r#wm { () => {} } }
"#,
    );
    let declared = deps::module_names(&file);
    assert_eq!(
        declared.names,
        names(&[
            "B", "D", "Foo", "S", "Y", "cc", "gm", "inl", "match", "mm", "wm"
        ])
    );
    assert_eq!(declared.inline[0].name, "inl");
    assert_eq!(declared.inline[0].names.names, names(&["f"]));
    assert_eq!(deps::mods_in_text("mod r#type;"), vec!["type"]);
}

#[test]
fn rust_raw_path_attributes_apply() {
    let file = parse(
        Grammar::Rust,
        "#[r#path = \"z.rs\"]\nmod z;\nw! { #[r#path = \"o.rs\"] mod x; }\n",
    );
    let relationships = deps::extract(&file);
    let attributes: Vec<Option<&str>> = relationships
        .iter()
        .map(|r| r.path_attribute.as_ref().and_then(|p| p.as_deref().ok()))
        .collect();
    assert_eq!(attributes, [Some("z.rs"), Some("o.rs")]);
}

#[test]
fn comments_inside_a_path_attribute_keep_it() {
    let file = parse(
        Grammar::Rust,
        r#"#[/* a */ path /* c */ = /* d */ "y.rs" /* e */]
mod y;
#[/* a */ path = "inl"]
mod inl {}
w! {
    #[path /* c */ = "o.rs"] mod x;
    #[/* a */ path = // b
      "p.rs" /* e */] mod q;
}
"#,
    );
    let relationships = deps::extract(&file);
    let attributes: Vec<(&str, Option<&str>)> = relationships
        .iter()
        .map(|r| {
            (
                r.specifier.as_str(),
                r.path_attribute.as_ref().and_then(|p| p.as_deref().ok()),
            )
        })
        .collect();
    assert_eq!(
        attributes,
        [
            ("y", Some("y.rs")),
            ("x", Some("o.rs")),
            ("q", Some("p.rs"))
        ]
    );
    assert_eq!(
        deps::module_names(&file).inline[0]
            .path_attribute
            .as_ref()
            .and_then(|p| p.as_deref().ok()),
        Some("inl")
    );
}

#[test]
fn comments_inside_include_and_use_tokens_are_skipped() {
    let file = parse(
        Grammar::Rust,
        "const X: &str = include_str!(/* c */ \"d.txt\" // e\n);\nuse a::/* c */b;\nuse a /* c */ ::{/* d */ e};\nuse f::/* c */ *;\nr#include!(\"g.txt\");\nw! { use h /* c */ ::i; }\n",
    );
    let relationships = deps::extract(&file);
    let found: Vec<(RelationshipKind, &str, bool)> = relationships
        .iter()
        .map(|r| (r.kind, r.specifier.as_str(), r.literal))
        .collect();
    assert_eq!(
        found,
        [
            (Include, "d.txt", true),
            (Use, "a::b", true),
            (Use, "a::e", true),
            (Use, "f", true),
            (Include, "g.txt", true),
            (Use, "h::i", true),
        ]
    );
}

/// The written specifier and the decoded value of each relationship.
fn values(grammar: Grammar, source: &str) -> Vec<(String, LiteralValue)> {
    deps::extract(&parse(grammar, source))
        .into_iter()
        .map(|r| (r.specifier, r.value))
        .collect()
}

fn ok(written: &str, value: &str) -> (String, LiteralValue) {
    (written.to_string(), Ok(value.to_string()))
}

fn invalid(written: &str) -> (String, LiteralValue) {
    (written.to_string(), Err(InvalidLiteral))
}

#[test]
fn a_js_string_is_decoded_and_keeps_its_written_text() {
    let source = concat!(
        "import './a.js';\n",
        "import './\\x61.js';\n",
        "import \"./\\u{61}.js\";\n",
        "export * from './\\u0061.js';\n",
        "require('./\\a\\'\\\\.js');\n",
        "import(`./\\x61.js`);\n",
        "import './\\ud83d\\ude00.js';\n",
        "import './\\\na.js';\n",
        "import './\\0.js';\n",
        "import './\\x6.js';\n",
        "import './\\u{110000}.js';\n",
        "import './\\ud800.js';\n",
        "import './\\01.js';\n",
    );
    assert_eq!(
        values(Grammar::JavaScript, source),
        [
            ok("./a.js", "./a.js"),
            ok("./\\x61.js", "./a.js"),
            ok("./\\u{61}.js", "./a.js"),
            ok("./\\u0061.js", "./a.js"),
            ok("./\\a\\'\\\\.js", "./a'\\.js"),
            ok("./\\x61.js", "./a.js"),
            ok("./\\ud83d\\ude00.js", "./\u{1f600}.js"),
            ok("./\\\na.js", "./a.js"),
            ok("./\\0.js", "./\0.js"),
            invalid("./\\x6.js"),
            invalid("./\\u{110000}.js"),
            invalid("./\\ud800.js"),
            invalid("./\\01.js"),
        ]
    );
    assert_eq!(
        values(Grammar::TypeScript, "import a = require('./\\x61');\n"),
        [ok("./\\x61", "./a")]
    );
}

#[test]
fn a_go_interpreted_string_is_decoded_and_a_raw_string_is_not() {
    let source = concat!(
        "package main\n\nimport (\n",
        "\t\"example.com/lib\"\n",
        "\t\"example.com/\\x6cib\"\n",
        "\t\"example.com/\\154ib\"\n",
        "\t\"example.com/\\u006cib\"\n",
        "\t\"example.com/\\U0000006cib\"\n",
        "\t\"example.com/\\u00e9\\xc3\\xa9\"\n",
        "\t`example.com/\\x6cib`\n",
        "\t\"example.com/\\q\"\n",
        "\t\"example.com/\\xff\"\n",
        "\t\"example.com/\\400\"\n",
        ")\n",
    );
    assert_eq!(
        values(Grammar::Go, source),
        [
            ok("example.com/lib", "example.com/lib"),
            ok("example.com/\\x6cib", "example.com/lib"),
            ok("example.com/\\154ib", "example.com/lib"),
            ok("example.com/\\u006cib", "example.com/lib"),
            ok("example.com/\\U0000006cib", "example.com/lib"),
            ok("example.com/\\u00e9\\xc3\\xa9", "example.com/\u{e9}\u{e9}"),
            ok("example.com/\\x6cib", "example.com/\\x6cib"),
            invalid("example.com/\\q"),
            invalid("example.com/\\xff"),
            invalid("example.com/\\400"),
        ]
    );
}

#[test]
fn a_rust_string_is_decoded_and_a_raw_string_is_not() {
    let source = concat!(
        "const _: &[u8] = include!(\"d\\u{61}ta.txt\").as_ref();\n",
        "const _: &[u8] = include_str!(\"d\\x61\\u{000061}ta.txt\").as_ref();\n",
        "const _: &[u8] = include_bytes!(\"da\\\n    ta.txt\").as_ref();\n",
        "const _: &[u8] = include!(r\"d\\u{61}ta.txt\").as_ref();\n",
        "const _: &[u8] = include!(r#\"d\\x61ta.txt\"#).as_ref();\n",
        "const _: &[u8] = include!(\"d\\qta.txt\").as_ref();\n",
        "const _: &[u8] = include!(\"d\\x80ta.txt\").as_ref();\n",
        "const _: &[u8] = include!(\"d\\u{d800}ta.txt\").as_ref();\n",
    );
    assert_eq!(
        values(Grammar::Rust, source),
        [
            ok("d\\u{61}ta.txt", "data.txt"),
            ok("d\\x61\\u{000061}ta.txt", "daata.txt"),
            ok("da\\\n    ta.txt", "data.txt"),
            ok("d\\u{61}ta.txt", "d\\u{61}ta.txt"),
            ok("d\\x61ta.txt", "d\\x61ta.txt"),
            invalid("d\\qta.txt"),
            invalid("d\\x80ta.txt"),
            invalid("d\\u{d800}ta.txt"),
        ]
    );
}

#[test]
fn a_python_module_name_is_its_identifiers_joined_by_dots() {
    let source = concat!(
        "import pkg . sub\n",
        "import pkg .\\\n  sub as s\n",
        "from pkg . sub import v\n",
        "from pkg .\\\n sub import v\n",
        "from . sub import x\n",
        "from . . import x\n",
        "from .\\\n. r import x\n",
        "from ...a import x\n",
        "from a import (b, # c\n d)\n",
    );
    assert_eq!(
        values(Grammar::Python, source),
        [
            ok("pkg . sub", "pkg.sub"),
            ok("pkg .\\\n  sub", "pkg.sub"),
            ok("pkg . sub.v", "pkg.sub.v"),
            ok("pkg .\\\n sub.v", "pkg.sub.v"),
            ok(". sub.x", ".sub.x"),
            ok(". .x", "..x"),
            ok(".\\\n. r.x", "..r.x"),
            ok("...a.x", "...a.x"),
            ok("a.b", "a.b"),
            ok("a.d", "a.d"),
        ]
    );
}

#[test]
fn a_rust_path_attribute_is_decoded() {
    let file = parse(
        Grammar::Rust,
        concat!(
            "#[path = \"\\x6f.rs\"]\nmod x;\n",
            "#[path = r\"\\x6f.rs\"]\nmod r;\n",
            "#[path = \"\\q.rs\"]\nmod bad;\n",
            "w! { #[path = \"\\u{70}.rs\"] mod y; #[path = \"\\q\"] mod z; }\n",
            "#[path = \"\\x69n\"]\nmod inl {}\n",
            "#[path = \"\\q\"]\nmod inl2 {}\n",
        ),
    );
    let attributes: Vec<(String, Option<LiteralValue>)> = deps::extract(&file)
        .into_iter()
        .map(|r| (r.specifier, r.path_attribute))
        .collect();
    let some = |v: &str| Some(Ok(v.to_string()));
    assert_eq!(
        attributes,
        [
            ("x".to_string(), some("o.rs")),
            ("r".to_string(), some("\\x6f.rs")),
            ("bad".to_string(), Some(Err(InvalidLiteral))),
            ("y".to_string(), some("p.rs")),
            ("z".to_string(), Some(Err(InvalidLiteral))),
        ]
    );
    let inline: Vec<Option<LiteralValue>> = deps::module_names(&file)
        .inline
        .into_iter()
        .map(|m| m.path_attribute)
        .collect();
    assert_eq!(inline, [some("in"), Some(Err(InvalidLiteral))]);
}

#[test]
fn a_mod_declaration_in_a_body_is_a_file_module_of_its_scope() {
    let file = parse(
        Grammar::Rust,
        "mod top;\nfn f() {\n    #[path = \"b.rs\"]\n    mod a;\n    mod inner {\n        mod c;\n    }\n}\n",
    );
    let module = deps::module_names(&file);
    assert_eq!(
        module.files,
        [],
        "a `mod x;` outside a body is no file module of a scope"
    );
    let [block] = module.blocks.as_slice() else {
        panic!("one block: {module:?}");
    };
    assert_eq!(block.names.names, names(&["a", "inner"]));
    assert_eq!(block.names.files, [("a".to_string(), range("4:5-4:11"))]);
    let [inner] = block.names.inline.as_slice() else {
        panic!("one inline module: {block:?}");
    };
    assert_eq!(inner.names.names, names(&["c"]));
    assert_eq!(inner.names.files, [("c".to_string(), range("6:9-6:15"))]);
}

#[test]
fn macro_token_forms_in_a_body_are_relationships_of_its_scope() {
    let file = parse(
        Grammar::Rust,
        "fn f() {\n    m! { #[path = \"b.rs\"] pub mod a; use c::D; struct S; }\n    mod inner {\n        m! { mod e; }\n    }\n    let v = n![1];\n}\n",
    );
    let found: Vec<(String, String)> = deps::extract(&file)
        .into_iter()
        .map(|r| (r.kind.as_str().to_string(), r.specifier))
        .collect();
    assert_eq!(
        found,
        [
            ("module".to_string(), "a".to_string()),
            ("use".to_string(), "c::D".to_string()),
            ("module".to_string(), "e".to_string()),
        ],
        "a macro statement of a block is in item position; `n![1]` is not"
    );
    let module = deps::module_names(&file);
    assert_eq!(
        module.names,
        names(&["f"]),
        "the body's items are not the module's"
    );
    let [block] = module.blocks.as_slice() else {
        panic!("one block: {module:?}");
    };
    assert_eq!(block.names.names, names(&["S", "a", "inner"]));
    assert_eq!(block.names.files, [("a".to_string(), range("2:27-2:37"))]);
    assert_eq!(
        block.names.uses,
        [UseBinding {
            name: Some("D".to_string()),
            path: "c::D".to_string(),
            range: range("2:38-2:47"),
        }]
    );
    let [inner] = block.names.inline.as_slice() else {
        panic!("one inline module: {block:?}");
    };
    assert_eq!(inner.names.files, [("e".to_string(), range("4:14-4:20"))]);
}
