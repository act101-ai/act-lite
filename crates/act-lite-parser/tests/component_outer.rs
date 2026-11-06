//! The outer structure of component files that is not reported: content
//! inside an ERROR node of the outer tree, the content of a Vue template
//! whose `lang` is not `html`, and the content of a Vue custom block. A
//! block whose start tag spans lines is named by its first line in both
//! `skeleton` and `symbols`.

use act_lite_parser::{Grammar, QueryResult, SourceFile, execute_skeleton, execute_symbols};

fn load(name: &str, text: &str, grammar: Grammar) -> SourceFile {
    SourceFile::from_content(name, text.to_string(), grammar).unwrap()
}

/// `(kind, name)` of each skeleton declaration.
fn skeleton(source: &SourceFile) -> Vec<(String, String)> {
    let QueryResult::Skeleton(skeleton) = execute_skeleton(source).unwrap() else {
        panic!("not a skeleton");
    };
    skeleton
        .declarations
        .into_iter()
        .map(|d| (d.kind, d.name))
        .collect()
}

/// `(kind, name)` of each symbol.
fn symbols(source: &SourceFile) -> Vec<(String, String)> {
    let QueryResult::Symbols { symbols } = execute_symbols(source).unwrap() else {
        panic!("not symbols");
    };
    symbols
        .iter()
        .map(|s| {
            (
                format!("{:?}", s.kind()).to_lowercase(),
                s.name().to_string(),
            )
        })
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(k, n)| (k.to_string(), n.to_string()))
        .collect()
}

#[test]
fn a_vue_custom_block_reports_only_its_block() {
    let source = load(
        "Docs.vue",
        "<docs>\n<p id=\"d\" class=\"c\">x</p>\n</docs>\n",
        Grammar::Vue,
    );
    assert_eq!(skeleton(&source), pairs(&[("module", "<docs>")]));
    assert_eq!(symbols(&source), pairs(&[("module", "<docs>")]));
}

#[test]
fn a_vue_template_in_another_language_reports_only_its_block() {
    let source = load(
        "Pug.vue",
        "<template lang=\"pug\">\n<p id=\"q\"></p>\n</template>\n",
        Grammar::Vue,
    );
    assert_eq!(
        skeleton(&source),
        pairs(&[("module", "<template lang=\"pug\">")])
    );
    assert_eq!(
        symbols(&source),
        pairs(&[("module", "<template lang=\"pug\">")])
    );
}

#[test]
fn a_vue_html_template_reports_its_structure() {
    for open in ["<template>", "<template lang=\"html\">"] {
        let source = load(
            "Html.vue",
            &format!("{open}\n<p id=\"q\"></p>\n</template>\n"),
            Grammar::Vue,
        );
        assert_eq!(
            skeleton(&source),
            pairs(&[("module", open), ("element", "p"), ("field", "q")]),
            "{open}"
        );
    }
}

#[test]
fn a_multi_line_start_tag_names_its_block_by_the_first_line() {
    let source = load(
        "Multi.vue",
        "<template\n  id=\"t\">\n<b class=\"k\"></b>\n</template>\n",
        Grammar::Vue,
    );
    assert_eq!(skeleton(&source)[0], ("module".into(), "<template".into()));
    assert_eq!(symbols(&source)[0], ("module".into(), "<template".into()));
}

#[test]
fn svelte_markup_inside_an_error_is_not_reported() {
    // The outer tree is one ERROR holding the `<span>` element.
    let source = load(
        "Err.svelte",
        "<div>\n<p id=\"a\">\n{#if x}\n<span id=\"s\"></span>\n</div>\n",
        Grammar::Svelte,
    );
    assert!(source.has_any_error());
    assert_eq!(skeleton(&source), Vec::<(String, String)>::new());
    assert_eq!(symbols(&source), Vec::<(String, String)>::new());
}

#[test]
fn markup_beside_an_error_is_still_reported() {
    let source = load(
        "Beside.svelte",
        "<div>\n<p id=\"a\">\n<span id=\"s\"></span>\n</div>\n</p>\n<</span>\n",
        Grammar::Svelte,
    );
    assert!(source.has_any_error());
    assert_eq!(
        skeleton(&source),
        pairs(&[
            ("element", "div"),
            ("element", "p"),
            ("field", "a"),
            ("element", "span"),
            ("field", "s"),
        ])
    );
}
