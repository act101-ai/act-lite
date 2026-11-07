//! The analysis input of component files: the script and frontmatter region
//! trees as code units of one file, the parse failures over them, the
//! regions that are not analyzed and the files that hold no analyzed code.

use act_lite_cli::analyze::discovery::Candidate;
use act_lite_cli::analyze::envelope::AnalyzeError;
use act_lite_cli::analyze::read::{
    CodeUnit, FileSource, Parsed, ReadOutcome, ReadResult, Reader, read_file, read_file_with,
    read_files,
};
use act_lite_parser::Grammar;
use act_lite_parser::function_model::supports;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/components");

const BAD_TS: &str = "<template><p/></template>\n<script lang=\"ts\">\nfunction f(a: number {\n  return a\n}\nfunction g(b: number) { return b }\n</script>\n";
const UNCLOSED: &str =
    "<script setup lang=\"ts\">\nconst a = 1\n\n<template>\n  <p>{{ a }}</p>\n</template>\n";
const LANGS: &str = "<template lang=\"pug\">\ndiv\n  p {{ a }}\n</template>\n<script lang=\"coffee\">\nx = 1\n</script>\n<i18n lang=\"json\">{\"a\":1}</i18n>\n";
const LEGACY: &str = "<script lang=\"coffee\">\nx = 1\n</script>\n";
const EMPTY_ASTRO: &str = "---\n---\n<script src=\"/x.js\"/>\n<style></style>\n";
const LESS_STYLE: &str = "<script setup lang=\"ts\">\nconst a = 1\n</script>\n<style lang=\"less\">\n.a { b: c; }\n</style>\n";
const TWO: &str = "<script setup lang=\"ts\">\nconst a = 1\n</script>\n<script setup lang=\"ts\">\nconst b = 2\n</script>\n";

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn candidate(root: &Path, rel: &str) -> Candidate {
    Candidate {
        path: root.join(rel),
        rel: rel.to_string(),
        grammar: Grammar::from_path(Path::new(rel)).unwrap(),
    }
}

/// A workspace holding `files`, read as the runner reads them.
fn read_all(files: &[(&str, &str)]) -> ReadOutcome {
    let dir = tempfile::tempdir().unwrap();
    let candidates: Vec<Candidate> = files
        .iter()
        .map(|(rel, content)| {
            write(dir.path(), rel, content);
            candidate(dir.path(), rel)
        })
        .collect();
    read_files(&candidates, &supports, false, &CancellationToken::new()).unwrap()
}

fn parsed(result: Result<ReadResult, AnalyzeError>) -> Parsed {
    match result {
        Ok(ReadResult::Parsed { parsed, .. }) => parsed,
        _ => panic!("expected a parsed file"),
    }
}

/// The ranges of the nodes of `kind` in a unit's tree.
fn ranges_of(unit: &CodeUnit<'_>, kind: &str) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    let mut stack = vec![unit.tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.kind() == kind {
            let (s, e) = (node.start_position(), node.end_position());
            out.push((
                format!(
                    "{}:{}-{}:{}",
                    s.row + 1,
                    s.column + 1,
                    e.row + 1,
                    e.column + 1
                ),
                node.start_byte(),
                node.end_byte(),
            ));
        }
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }
    out.sort();
    out
}

fn failures(out: &ReadOutcome) -> Vec<(String, String)> {
    out.parse_failures
        .iter()
        .map(|f| (f.file.clone(), f.range.to_string()))
        .collect()
}

fn not_analyzed(out: &ReadOutcome) -> Vec<(String, String, String)> {
    out.regions_not_analyzed
        .iter()
        .map(|r| (r.file.clone(), r.range.to_string(), r.reason.clone()))
        .collect()
}

fn row(file: &str, range: &str, reason: &str) -> (String, String, String) {
    (file.to_string(), range.to_string(), reason.to_string())
}

#[test]
fn a_vue_file_gives_one_unit_per_script_region_at_component_positions() {
    let file = Candidate {
        path: Path::new(FIXTURES).join("Counter.vue"),
        rel: "Counter.vue".to_string(),
        grammar: Grammar::Vue,
    };
    let p = parsed(read_file(&file, false));
    let units = p.code();
    assert_eq!(units.len(), 2, "two script regions; the style is not code");
    for unit in &units {
        assert_eq!(unit.grammar, Grammar::TypeScript);
        assert!(std::ptr::eq(unit.source, p.source.content()));
        assert!(!unit.tree.root_node().has_error());
    }
    assert_eq!(
        ranges_of(&units[0], "export_statement"),
        [("3:1-3:35".to_string(), 49, 83)]
    );
    assert!(ranges_of(&units[0], "function_declaration").is_empty());
    assert_eq!(
        ranges_of(&units[1], "function_declaration"),
        [("10:1-14:2".to_string(), 224, 332)]
    );
    assert_eq!(
        ranges_of(&units[1], "import_statement")[0].0,
        "7:1-7:26".to_string()
    );
    let declarators: Vec<String> = ranges_of(&units[1], "variable_declarator")
        .into_iter()
        .map(|r| r.0)
        .collect();
    assert_eq!(declarators, ["8:7-8:47", "9:7-9:31"]);
    assert!(ranges_of(&units[1], "export_statement").is_empty());
}

#[test]
fn a_ts_file_gives_one_unit_equal_to_its_tree() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.ts", "function a(): void {}\n");
    let p = parsed(read_file(&candidate(dir.path(), "a.ts"), false));
    let units = p.code();
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].grammar, Grammar::TypeScript);
    assert_eq!(units[0].tree.root_node(), p.source.tree().root_node());
    assert!(std::ptr::eq(units[0].source, p.source.content()));
}

#[test]
fn parse_failures_cover_the_outer_tree_and_the_script_region_trees() {
    let out = read_all(&[("BadTs.vue", BAD_TS), ("Unclosed.vue", UNCLOSED)]);
    let found = failures(&out);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0], ("BadTs.vue".to_string(), "3:21-3:21".to_string()));
    assert_eq!(found[1].0, "Unclosed.vue");
    assert!(found[1].1.starts_with("1:1-"), "{found:?}");
}

#[test]
fn an_error_in_a_style_region_is_not_a_parse_failure() {
    let broken = "<script setup lang=\"ts\">\nconst a = 1\n</script>\n<style>\n.a { color: red; } } {\n</style>\n";
    let out = read_all(&[("Style.vue", broken)]);
    assert!(out.parse_failures.is_empty(), "{:?}", failures(&out));
}

#[test]
fn a_region_parse_timeout_is_a_parse_failure_and_a_region_not_analyzed() {
    let mut body = String::new();
    for i in 0..20_000 {
        body.push_str(&format!("const v{i}: number = {i} + {i};\n"));
    }
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "Slow.vue",
        &format!("<template><p/></template>\n<script lang=\"ts\">\n{body}</script>\n"),
    );
    let file = candidate(dir.path(), "Slow.vue");
    let result = read_file_with(
        &file,
        false,
        &CancellationToken::new(),
        Some(Duration::from_nanos(1)),
    );
    let Ok(ReadResult::Parsed {
        parsed,
        parse_failures,
        regions_not_analyzed,
        without_analyzed_code,
        ..
    }) = result
    else {
        panic!("expected a parsed file");
    };
    assert!(parsed.code().is_empty());
    let failures: Vec<_> = parse_failures
        .iter()
        .map(|f| (f.file.as_str(), f.range.to_string()))
        .collect();
    assert_eq!(failures, [("Slow.vue", "2:1-2:19".to_string())]);
    let listed: Vec<_> = regions_not_analyzed
        .iter()
        .map(|r| (r.file.as_str(), r.range.to_string(), r.reason.as_str()))
        .collect();
    assert_eq!(
        listed,
        [("Slow.vue", "2:1-2:19".to_string(), "parse timeout")]
    );
    assert!(!without_analyzed_code);
}

#[test]
fn unsupported_and_malformed_script_regions_are_listed_and_style_is_not() {
    let card = fs::read_to_string(Path::new(FIXTURES).join("Card.astro")).unwrap();
    let counter = fs::read_to_string(Path::new(FIXTURES).join("Counter.vue")).unwrap();
    let out = read_all(&[
        ("Langs.vue", LANGS),
        ("Unclosed.vue", UNCLOSED),
        ("Less.vue", LESS_STYLE),
        ("Card.astro", &card),
        ("Counter.vue", &counter),
        ("Empty.astro", EMPTY_ASTRO),
    ]);
    assert_eq!(
        not_analyzed(&out),
        [
            row(
                "Langs.vue",
                "5:1-5:23",
                "unsupported script language: coffee"
            ),
            row("Unclosed.vue", "1:1-1:25", "malformed region"),
        ]
    );
    // Langs.vue: its one script is of a language that is not parsed.
    assert_eq!(out.files_without_analyzed_code, 1);
}

#[test]
fn a_file_whose_script_code_is_all_unsupported_holds_no_analyzed_code() {
    let out = read_all(&[("Legacy.vue", LEGACY), ("Empty.astro", EMPTY_ASTRO)]);
    assert_eq!(out.files_without_analyzed_code, 1);
    assert_eq!(
        not_analyzed(&out),
        [row(
            "Legacy.vue",
            "1:1-1:23",
            "unsupported script language: coffee"
        )]
    );
    assert!(out.parse_failures.is_empty());
    let alone = read_all(&[("Empty.astro", EMPTY_ASTRO)]);
    assert_eq!(alone.files_without_analyzed_code, 0);
    assert!(alone.regions_not_analyzed.is_empty());
}

#[test]
fn files_of_the_existing_entries_have_no_region_evidence() {
    let out = read_all(&[("a.ts", "function a() {\n"), ("b.js", "let b = 1;\n")]);
    assert!(out.regions_not_analyzed.is_empty());
    assert_eq!(out.files_without_analyzed_code, 0);
    assert_eq!(failures(&out).len(), 1);
}

#[test]
fn a_cancelled_token_stops_a_component_read_before_its_region_parses() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Two.vue", TWO);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let file = candidate(dir.path(), "Two.vue");
    assert!(matches!(
        read_file_with(&file, false, &cancel, None),
        Err(AnalyzeError::Cancelled)
    ));
    let fresh = CancellationToken::new();
    let p = parsed(read_file_with(&file, false, &fresh, None));
    assert_eq!(p.code().len(), 2);
}

/// What identifies a unit: its grammar and its tree.
fn unit_keys(p: &Parsed) -> Vec<(Grammar, String, usize, usize)> {
    p.code()
        .iter()
        .map(|u| {
            let root = u.tree.root_node();
            (
                u.grammar,
                root.to_sexp(),
                root.start_byte(),
                root.end_byte(),
            )
        })
        .collect()
}

#[test]
fn reread_of_a_component_file_gives_the_same_units() {
    let dir = tempfile::tempdir().unwrap();
    let counter = fs::read_to_string(Path::new(FIXTURES).join("Counter.vue")).unwrap();
    write(dir.path(), "Counter.vue", &counter);
    write(dir.path(), "Langs.vue", LANGS);
    let files = [
        candidate(dir.path(), "Counter.vue"),
        candidate(dir.path(), "Langs.vue"),
    ];
    let mut reader = Reader::new(&files, &supports, false, &CancellationToken::new());
    let first = reader.next_file().unwrap().unwrap();
    assert_eq!(first.rel, "Counter.vue");
    let keys = unit_keys(&first);
    assert_eq!(keys.len(), 2);
    drop(first);
    reader.next_file().unwrap().unwrap();
    let again = reader.reread("Counter.vue").unwrap();
    assert_eq!(unit_keys(&again), keys);
    reader.reread("Langs.vue").unwrap();
    let out = reader.finish();
    assert_eq!(out.regions_not_analyzed.len(), 1, "a reread adds no copy");
}
