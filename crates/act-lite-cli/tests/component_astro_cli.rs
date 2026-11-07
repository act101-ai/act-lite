//! Astro component files through the built binary: `query skeleton` and
//! `query symbols` give the structure, the region declarations and their
//! locations in the component file, and `parse_error` for malformed files.

mod langs_added;

use langs_added::{Mcp, cli_json};
use serde_json::{Value, json};
use std::path::Path;

/// A temporary workspace holding each `(path, content)` of `files`.
fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        std::fs::write(dir.path().join(rel), content).unwrap();
    }
    dir
}

fn card() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/components/Card.astro"),
    )
    .unwrap()
}

fn compact(range: &Value) -> String {
    let p = |end: &str| {
        format!(
            "{}:{}",
            range[end]["line"].as_u64().unwrap(),
            range[end]["column"].as_u64().unwrap()
        )
    };
    format!("{}-{}", p("start"), p("end"))
}

type Row = (String, String, String, Option<String>);

fn rows(skeleton: &Value) -> Vec<Row> {
    skeleton["declarations"]
        .as_array()
        .unwrap_or_else(|| panic!("no declarations in {skeleton}"))
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
                compact(&d["range"]),
                d.get("name_range").filter(|r| !r.is_null()).map(compact),
            )
        })
        .collect()
}

fn row(kind: &str, name: &str, range: &str, name_range: Option<&str>) -> Row {
    (
        kind.to_string(),
        name.to_string(),
        range.to_string(),
        name_range.map(str::to_string),
    )
}

fn symbols(result: &Value) -> Vec<(String, String, String)> {
    result["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("no symbols in {result}"))
        .iter()
        .map(|s| {
            (
                s["kind"].as_str().unwrap().to_string(),
                s["name"].as_str().unwrap().to_string(),
                compact(&s["range"]),
            )
        })
        .collect()
}

fn sym(kind: &str, name: &str, range: &str) -> (String, String, String) {
    (kind.to_string(), name.to_string(), range.to_string())
}

fn card_table() -> Vec<Row> {
    vec![
        row("module", "---", "1:1-8:4", Some("1:1-1:4")),
        row(
            "import",
            "import Layout from '../layouts/Layout.astro';",
            "2:1-2:46",
            None,
        ),
        row("interface", "Props", "3:1-3:34", Some("3:11-3:16")),
        row("function", "shout", "5:1-7:2", Some("5:10-5:15")),
        row("element", "Layout", "9:1-11:10", Some("9:2-9:8")),
        row("element", "h1", "10:3-10:45", Some("10:4-10:6")),
        row("field", "top", "10:7-10:15", Some("10:11-10:14")),
        row("field", "t", "10:16-10:25", Some("10:23-10:24")),
        row("module", "<script>", "12:1-14:10", Some("12:1-12:9")),
        row("function", "hello", "13:3-13:49", Some("13:12-13:17")),
        row(
            "module",
            "<script is:inline>",
            "15:1-15:43",
            Some("15:1-15:19"),
        ),
        row("variable", "inline", "15:19-15:34", Some("15:23-15:29")),
        row(
            "module",
            "<script type=\"application/ld+json\">",
            "16:1-16:53",
            Some("16:1-16:36"),
        ),
        row("module", "<style>", "17:1-19:9", Some("17:1-17:8")),
        row("class", "h1", "18:3-18:21", Some("18:3-18:5")),
        row("field", "color", "18:8-18:19", Some("18:8-18:13")),
    ]
}

#[test]
fn skeleton_of_card_is_the_table_with_the_byte_offsets_of_shout() {
    let ws = workspace(&[("Card.astro", &card())]);
    let skeleton = cli_json(ws.path(), &["query", "skeleton", "Card.astro"]);
    assert_eq!(rows(&skeleton), card_table());
    let shout = skeleton["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "shout")
        .unwrap();
    assert_eq!(shout["range"]["start"]["byte_offset"], 115);
    assert_eq!(shout["range"]["end"]["byte_offset"], 197);
}

#[test]
fn symbols_of_card_are_the_blocks_the_structure_and_the_region_symbols() {
    let ws = workspace(&[("Card.astro", &card())]);
    let result = cli_json(ws.path(), &["query", "symbols", "Card.astro"]);
    assert!(result.get("parse_error").is_none(), "{result}");
    let got = symbols(&result);
    for want in [
        sym("module", "---", "1:1-1:4"),
        sym("unknown", "title", "4:9-4:14"),
        sym("function", "shout", "5:10-5:15"),
        sym("class", "Layout", "9:2-9:8"),
        sym("class", "h1", "10:4-10:6"),
        sym("field", "top", "10:11-10:14"),
        sym("field", "t", "10:23-10:24"),
        sym("module", "<script>", "12:1-12:9"),
        sym("function", "hello", "13:12-13:17"),
        sym("module", "<script is:inline>", "15:1-15:19"),
        sym("variable", "inline", "15:23-15:29"),
        sym(
            "module",
            "<script type=\"application/ld+json\">",
            "16:1-16:36",
        ),
        sym("module", "<style>", "17:1-17:8"),
        sym("field", "color", "18:8-18:13"),
    ] {
        assert!(got.contains(&want), "{want:?} not in {got:?}");
    }
    // Nothing of the data block's content, nor of a template expression.
    assert!(
        !got.iter()
            .any(|s| s.2.starts_with("16:") && s.0 != "module")
    );
    assert!(!got.iter().any(|s| s.2.starts_with("9:1") && s.1 == "title"));
}

#[test]
fn mcp_skeleton_and_symbols_of_card_give_the_same_names_and_kinds() {
    let ws = workspace(&[("Card.astro", &card())]);
    let cli_skeleton = rows(&cli_json(ws.path(), &["query", "skeleton", "Card.astro"]));
    let cli_symbols = symbols(&cli_json(ws.path(), &["query", "symbols", "Card.astro"]));
    let mut mcp = Mcp::start(ws.path());
    let first = |result: Value| -> Value {
        let text = result["content"][0]["text"].as_str().unwrap().to_string();
        serde_json::from_str(&text).unwrap()
    };
    let pairs = |items: &Value| -> Vec<(String, String)> {
        items
            .as_array()
            .unwrap_or_else(|| panic!("not a list: {items}"))
            .iter()
            .map(|i| {
                (
                    i["kind"].as_str().unwrap().to_lowercase(),
                    i["name"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    let skeleton = first(
        mcp.call("skeleton", json!({ "file": "Card.astro" }))
            .unwrap(),
    );
    let cli_names: Vec<(String, String)> = cli_skeleton
        .iter()
        .filter(|r| r.0 != "import")
        .map(|r| (r.0.clone(), r.1.clone()))
        .collect();
    assert_eq!(pairs(&skeleton["declarations"]), cli_names);
    // MCP `symbols` spells a kind capitalized.
    let result = first(
        mcp.call("symbols", json!({ "file": "Card.astro" }))
            .unwrap(),
    );
    let cli_kinds: Vec<(String, String)> = cli_symbols
        .iter()
        .map(|s| (s.0.clone(), s.1.clone()))
        .collect();
    assert_eq!(pairs(&result["symbols"]), cli_kinds);
}

#[test]
fn skeleton_of_card_with_crlf_keeps_every_line_and_column() {
    let ws = workspace(&[("Card.astro", &card().replace('\n', "\r\n"))]);
    let skeleton = cli_json(ws.path(), &["query", "skeleton", "Card.astro"]);
    assert_eq!(rows(&skeleton), card_table());
}

/// `(skeleton rows, parse_error)` of `name` holding `text`.
fn navigate(name: &str, text: &str) -> (Vec<Row>, bool) {
    let ws = workspace(&[(name, text)]);
    let skeleton = cli_json(ws.path(), &["query", "skeleton", name]);
    let symbols = cli_json(ws.path(), &["query", "symbols", name]);
    let parse_error = match symbols.get("parse_error") {
        None => false,
        Some(v) => v.as_bool().unwrap(),
    };
    (rows(&skeleton), parse_error)
}

#[test]
fn malformed_astro_files_report_no_region_and_a_parse_error() {
    assert_eq!(
        navigate("Unclosed.astro", "---\nconst a = 1;\n<p>{a}</p>\n"),
        (vec![], true)
    );
    assert_eq!(
        navigate(
            "Regex.astro",
            "---\nfunction e(v) { return v.replace(/\"/g, \"&quot;\"); }\n---\n<p/>\n"
        ),
        (vec![row("element", "p", "4:1-4:5", Some("4:2-4:3"))], true)
    );
}

#[test]
fn empty_and_nested_astro_files_report_their_blocks_and_regions() {
    assert_eq!(
        navigate(
            "Empty.astro",
            "---\n---\n<script src=\"/x.js\"/>\n<style></style>\n"
        ),
        (
            vec![
                row("module", "---", "1:1-2:4", Some("1:1-1:4")),
                row(
                    "module",
                    "<script src=\"/x.js\"/>",
                    "3:1-3:22",
                    Some("3:1-3:22")
                ),
                row("module", "<style>", "4:1-4:16", Some("4:1-4:8")),
            ],
            false
        )
    );
    assert_eq!(
        navigate(
            "Nested.astro",
            "<div><script>function n() {}</script></div>\n"
        ),
        (
            vec![
                row("element", "div", "1:1-1:44", Some("1:2-1:5")),
                row("module", "<script>", "1:6-1:38", Some("1:6-1:14")),
                row("function", "n", "1:14-1:29", Some("1:23-1:24")),
            ],
            false
        )
    );
}
