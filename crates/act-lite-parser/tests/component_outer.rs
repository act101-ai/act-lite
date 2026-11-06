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

#[test]
fn the_position_index_agrees_with_position_at_at_every_byte() {
    use act_lite_parser::regions::{PositionIndex, position_at};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../act-lite-cli/tests/fixtures/components");
    let mut files: Vec<(String, String, Grammar)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter_map(|path| {
            let grammar = match path.extension()?.to_str()? {
                "vue" => Grammar::Vue,
                "svelte" => Grammar::Svelte,
                "astro" => Grammar::Astro,
                _ => return None,
            };
            let name = path.file_name()?.to_str()?.to_string();
            Some((name, std::fs::read_to_string(&path).ok()?, grammar))
        })
        .collect();
    assert!(files.len() >= 3);
    files.push((
        "Err.svelte".into(),
        "<div>\n<p id=\"a\">\n{#if x}\n<span id=\"s\">{y}</span>\n</div>\n".into(),
        Grammar::Svelte,
    ));
    files.push((
        "Mixed.vue".into(),
        "<template><p :id=\"a\">{{ b }}</p></template>\n<script lang=\"coffee\">x = 1</script>\n<script>let c = 1;</script>\n<style>p{}</style>\n".into(),
        Grammar::Vue,
    ));
    for (name, text, grammar) in files {
        let source = SourceFile::from_content(&name, text.clone(), grammar).unwrap();
        let index = PositionIndex::new(&source);
        for byte in 0..=text.len() {
            assert_eq!(
                index.at(byte),
                position_at(&source, byte),
                "{name} byte {byte}"
            );
        }
    }
}

#[test]
fn a_flat_vue_template_stays_linear_in_its_elements() {
    if std::env::var("ACT_LITE_CHECK_PERF").as_deref() != Ok("1") {
        eprintln!("skipped: set ACT_LITE_CHECK_PERF=1 to time 10,000 sibling elements");
        return;
    }
    let mut text = String::from("<template>\n");
    for i in 0..10_000 {
        text.push_str(&format!("<p id=\"a{i}\" class=\"c\"></p>\n"));
    }
    text.push_str("</template>\n");
    let source = load("Flat.vue", &text, Grammar::Vue);
    let started = std::time::Instant::now();
    let rows = skeleton(&source);
    let symbols = symbols(&source);
    let elapsed = started.elapsed();
    assert_eq!(rows.len(), 1 + 3 * 10_000);
    assert_eq!(symbols.len(), 1 + 3 * 10_000);
    assert!(elapsed.as_secs_f64() < 3.0, "{elapsed:?}");
}
