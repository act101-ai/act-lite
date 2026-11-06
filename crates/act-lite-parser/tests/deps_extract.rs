//! Relationship extraction: the import-like constructs of JavaScript, JSX,
//! TypeScript, TSX, Python, Go and Rust, and the module-level names that
//! resolution checks.
//!
//! Every expected value below was written by hand from the fixture source.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use act_lite_core::CompactRange;
use act_lite_parser::deps::{self, InlineModule, ModuleNames, Relationship, RelationshipKind};
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
                    path_attribute: Some("other/place.rs".to_string()),
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
            path_attribute: Some("p.rs".to_string()),
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
                    },
                }],
            },
        }],
    };
    assert_eq!(deps::module_names(&file), expected);

    let wrapped = parse(Grammar::Rust, "cfg_rt! { struct S; }\n");
    assert_eq!(deps::module_names(&wrapped).names, names(&["S"]));
    let reexport = parse(Grammar::Rust, "pub use a::B;\n");
    assert_eq!(deps::module_names(&reexport).names, names(&["B"]));
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
