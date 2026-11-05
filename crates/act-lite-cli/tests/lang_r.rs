//! R: skeleton, symbols, name-level references and the language-server
//! limitation, through the CLI and MCP.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const SCALE_R: &str = "library(stats)
scale_by <- function(x, k = 2, ...) {
  if (k > 1) x * k else x
}
center = function(v) v - mean(v)
LIMIT <<- 10
f2 <- \\(a) a + 1
for (i in 1:3) print(i)
while (FALSE) next
setClass(\"Shape\", representation(name = \"character\"))
";

const NAME_LEVEL_HINT: &str =
    "Name-level baseline — a same-name symbol in an unrelated scope can appear.";

const SHADOWING: &str = "f <- function(x) x\ng <- function(x) x\n";

type Triple = (String, String, String);

/// A new temporary workspace holding `content` as `name`.
fn workspace(name: &str, content: &str) -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(ws.path().join(name), content).unwrap();
    ws
}

/// `line:column-line:column` of a CLI range object.
fn span(range: &Value) -> String {
    format!(
        "{}:{}-{}:{}",
        range["start"]["line"],
        range["start"]["column"],
        range["end"]["line"],
        range["end"]["column"]
    )
}

/// The JSON payload of an MCP tool result (its first text block, up to any
/// trailing note line).
fn mcp_payload(result: &Value) -> Value {
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    let json = text.split("\nNote:").next().unwrap_or_default();
    serde_json::from_str(json).unwrap_or_else(|e| panic!("not JSON ({e}): {result}"))
}

/// The text of every block of an MCP tool result.
fn mcp_text(result: &Value) -> String {
    result["content"]
        .as_array()
        .unwrap_or_else(|| panic!("no content in {result}"))
        .iter()
        .filter_map(|block| block["text"].as_str())
        .collect()
}

fn pairs(list: &Value) -> BTreeSet<(String, String)> {
    list.as_array()
        .unwrap_or_else(|| panic!("not a list: {list}"))
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pair_set(items: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    items
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

fn symbol_triples(file_dir: &std::path::Path, file: &str) -> (BTreeSet<Triple>, Value) {
    let symbols = cli_json(file_dir, &["query", "symbols", file]);
    let set = symbols["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("no symbols in {symbols}"))
        .iter()
        .map(|s| {
            (
                s["kind"].as_str().unwrap().to_string(),
                s["name"].as_str().unwrap().to_string(),
                span(&s["range"]),
            )
        })
        .collect();
    (set, symbols)
}

type Quad = (String, String, String, String);

fn skeleton_declarations(ws: &std::path::Path, file: &str) -> Vec<Value> {
    let skeleton = cli_json(ws, &["query", "skeleton", file]);
    skeleton["declarations"]
        .as_array()
        .unwrap_or_else(|| panic!("no declarations in {skeleton}"))
        .clone()
}

fn quads(rows: &[(&str, &str, &str, &str)]) -> BTreeSet<Quad> {
    rows.iter()
        .map(|(k, n, r, nr)| (k.to_string(), n.to_string(), r.to_string(), nr.to_string()))
        .collect()
}

fn triples(rows: &[(&str, &str, &str)]) -> BTreeSet<Triple> {
    rows.iter()
        .map(|(k, n, r)| (k.to_string(), n.to_string(), r.to_string()))
        .collect()
}

/// `(kind, name)` of every symbol that `query symbols` reports for `source`.
fn symbol_pairs(source: &str) -> BTreeSet<(String, String)> {
    let ws = workspace("t.R", source);
    let (set, _) = symbol_triples(ws.path(), "t.R");
    set.into_iter().map(|(k, n, _)| (k, n)).collect()
}

/// `(kind, name)` of every declaration that `query skeleton` reports for `source`.
fn skeleton_pairs(source: &str) -> BTreeSet<(String, String)> {
    let ws = workspace("t.R", source);
    pairs(&Value::Array(skeleton_declarations(ws.path(), "t.R")))
}

#[test]
fn fixture_holds_the_design_bytes() {
    assert_eq!(
        std::fs::read_to_string(fixture("r/scale.R")).unwrap(),
        SCALE_R
    );
}

#[test]
fn skeleton_is_the_table_set() {
    let list = skeleton_declarations(&fixture("r"), "scale.R");
    let actual: BTreeSet<Quad> = list
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
                span(&d["range"]),
                span(&d["name_range"]),
            )
        })
        .collect();
    let expected = quads(&[
        ("import", "stats", "1:1-1:15", "1:9-1:14"),
        ("function", "scale_by", "2:1-4:2", "2:1-2:9"),
        ("function", "center", "5:1-5:33", "5:1-5:7"),
        ("variable", "LIMIT", "6:1-6:13", "6:1-6:6"),
        ("function", "f2", "7:1-7:17", "7:1-7:3"),
    ]);
    assert_eq!(actual, expected);
}

#[test]
fn mcp_skeleton_gives_the_table_names_and_kinds_without_import_entries() {
    let skeleton = mcp_payload(
        &Mcp::start(&fixture("r"))
            .call("skeleton", json!({ "file": "scale.R" }))
            .unwrap(),
    );
    assert_eq!(
        pairs(&skeleton["declarations"]),
        pair_set(&[
            ("function", "scale_by"),
            ("function", "center"),
            ("variable", "LIMIT"),
            ("function", "f2"),
        ])
    );
}

#[test]
fn symbols_are_the_table_set() {
    let (actual, symbols) = symbol_triples(&fixture("r"), "scale.R");
    let expected = triples(&[
        ("function", "scale_by", "2:1-2:9"),
        ("parameter", "x", "2:22-2:23"),
        ("parameter", "k", "2:25-2:26"),
        ("parameter", "...", "2:32-2:35"),
        ("function", "center", "5:1-5:7"),
        ("parameter", "v", "5:19-5:20"),
        ("variable", "LIMIT", "6:1-6:6"),
        ("function", "f2", "7:1-7:3"),
        ("parameter", "a", "7:9-7:10"),
    ]);
    assert_eq!(actual, expected);
    assert!(symbols.get("parse_error").is_none(), "{symbols}");
    for s in symbols["symbols"].as_array().unwrap() {
        let name = s["name"].as_str().unwrap();
        assert!(!name.is_empty(), "{s}");
        assert_ne!(s["range"]["start"], s["range"]["end"], "zero-width {s}");
    }
}

#[test]
fn symbols_batch_lists_the_skeleton_set_on_cli_and_mcp() {
    let expected = pair_set(&[
        ("stats", "import"),
        ("scale_by", "function"),
        ("center", "function"),
        ("LIMIT", "variable"),
        ("f2", "function"),
    ]);
    let name_kind = |batch: &Value| -> BTreeSet<(String, String)> {
        batch["results"][0]["symbols"]
            .as_array()
            .unwrap_or_else(|| panic!("no symbols in {batch}"))
            .iter()
            .map(|s| {
                (
                    s["name"].as_str().unwrap().to_string(),
                    s["kind"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    let ws = fixture("r");
    let batch = cli_json(&ws, &["query", "symbols-batch", "--files", "scale.R"]);
    assert_eq!(name_kind(&batch), expected, "{batch}");
    let result = Mcp::start(&ws)
        .call("symbols_batch", json!({ "files": ["scale.R"] }))
        .unwrap();
    assert_eq!(name_kind(&mcp_payload(&result)), expected, "{result}");
}

#[test]
fn mcp_symbols_give_the_table_names_with_mcp_kinds() {
    let symbols = mcp_payload(
        &Mcp::start(&fixture("r"))
            .call("symbols", json!({ "file": "scale.R" }))
            .unwrap(),
    );
    assert_eq!(
        pairs(&symbols["symbols"]),
        pair_set(&[
            ("Function", "scale_by"),
            ("Parameter", "x"),
            ("Parameter", "k"),
            ("Parameter", "..."),
            ("Function", "center"),
            ("Parameter", "v"),
            ("Variable", "LIMIT"),
            ("Function", "f2"),
            ("Parameter", "a"),
        ])
    );
}

#[test]
fn query_symbol_returns_the_name_range() {
    let symbol = cli_json(
        &fixture("r"),
        &["query", "symbol", "scale_by", "--file", "scale.R"],
    );
    assert_eq!(span(&symbol["symbol"]["range"]), "2:1-2:9", "{symbol}");
}

#[test]
fn a_string_left_side_is_named_by_its_content() {
    let source = "\"f\" <- function() 1\n";
    assert_eq!(skeleton_pairs(source), pair_set(&[("function", "f")]));
    assert_eq!(symbol_pairs(source), pair_set(&[("function", "f")]));
}

#[test]
fn a_backtick_name_keeps_its_backticks() {
    let source = "`my fn` <- function() 1\n`v 1` <- 2\n";
    let expected = pair_set(&[("function", "`my fn`"), ("variable", "`v 1`")]);
    assert_eq!(skeleton_pairs(source), expected);
    assert_eq!(symbol_pairs(source), expected);
}

#[test]
fn a_superassignment_of_a_function_is_a_function() {
    let source = "g <<- function() 1\nh = \\() 2\n";
    let expected = pair_set(&[("function", "g"), ("function", "h")]);
    assert_eq!(skeleton_pairs(source), expected);
    assert_eq!(symbol_pairs(source), expected);
}

#[test]
fn right_assignments_name_the_right_side() {
    let source = "1 -> a\n2 ->> b\n";
    let expected = pair_set(&[("variable", "a"), ("variable", "b")]);
    assert_eq!(skeleton_pairs(source), expected);
    assert_eq!(symbol_pairs(source), expected);
}

#[test]
fn a_function_before_a_right_arrow_and_assign_are_not_reported() {
    let source = "function(x) x -> h\nassign(\"k\", 1)\n";
    assert!(skeleton_pairs(source).is_empty());
    assert!(symbol_pairs(source).is_empty());
}

#[test]
fn library_require_and_source_are_skeleton_entries_only() {
    let source = "library(a)\nrequire(b)\nsource(\"f.R\")\n";
    assert_eq!(
        skeleton_pairs(source),
        pair_set(&[("import", "a"), ("import", "b"), ("include", "\"f.R\"")])
    );
    assert!(symbol_pairs(source).is_empty());
}

#[test]
fn a_nested_function_is_reported_inside_its_parent() {
    let source = "f <- function() {\n  inner <- function() 1\n  local <- 2\n}\n";
    let ws = workspace("t.R", source);
    let list = skeleton_declarations(ws.path(), "t.R");
    assert_eq!(
        pairs(&Value::Array(list.clone())),
        pair_set(&[("function", "f"), ("function", "inner")])
    );
    let lines = |name: &str| -> (u64, u64) {
        let r = &list.iter().find(|d| d["name"] == name).unwrap()["range"];
        (
            r["start"]["line"].as_u64().unwrap(),
            r["end"]["line"].as_u64().unwrap(),
        )
    };
    let (outer, inner) = (lines("f"), lines("inner"));
    assert!(
        outer.0 <= inner.0 && inner.1 <= outer.1,
        "{outer:?} {inner:?}"
    );
    assert_eq!(
        symbol_pairs(source),
        pair_set(&[("function", "f"), ("function", "inner")])
    );
}

#[test]
fn other_assignments_and_classes_made_by_calls_are_not_reported() {
    let source = "x <- c(1, 2)\nsetRefClass(\"A\")\nR6Class(\"B\")\n";
    assert_eq!(skeleton_pairs(source), pair_set(&[("variable", "x")]));
}

fn reference_sites(ws: &std::path::Path, name: &str, line: &str, column: &str) -> Value {
    cli_json(
        ws,
        &[
            "query",
            "references",
            name,
            "--file",
            "t.R",
            "--line",
            line,
            "--column",
            column,
        ],
    )
}

fn sites(answer: &Value) -> BTreeSet<(u64, u64)> {
    answer["references"]
        .as_array()
        .unwrap_or_else(|| panic!("no references in {answer}"))
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect()
}

#[test]
fn references_at_a_dots_parameter_list_its_uses() {
    let ws = workspace("t.R", "f <- function(...) list(...)\n");
    let answer = reference_sites(ws.path(), "...", "1", "15");
    assert_eq!(sites(&answer), [(1, 15), (1, 25)].into_iter().collect());
}

#[test]
fn same_name_parameters_are_a_name_match_on_cli() {
    let ws = workspace("t.R", SHADOWING);
    let answer = reference_sites(ws.path(), "x", "1", "15");
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    assert_eq!(
        sites(&answer),
        [(1, 15), (1, 18), (2, 15), (2, 18)].into_iter().collect()
    );
}

fn assert_mcp_name_match(mcp: &mut Mcp, reason: &str) {
    let result = mcp
        .call("references", json!({ "symbol": "x", "file": "t.R" }))
        .unwrap();
    let answer = mcp_payload(&result);
    assert_eq!(
        answer["resolved_by"],
        format!("tree-sitter workspace baseline (name-level; {reason})"),
        "{answer}"
    );
    assert_eq!(
        answer["references"].as_array().map(Vec::len),
        Some(4),
        "{answer}"
    );
    let text = mcp_text(&result);
    assert!(text.contains(NAME_LEVEL_HINT), "{text}");
    assert!(!text.contains("Install"), "{text}");
}

#[test]
fn same_name_parameters_are_a_name_match_on_mcp() {
    let ws = workspace("t.R", SHADOWING);
    assert_mcp_name_match(&mut Mcp::start(ws.path()), "no language server");
}

#[test]
fn mcp_reason_reads_disabled_when_language_servers_are_disabled() {
    let ws = workspace("t.R", SHADOWING);
    let mut mcp = Mcp::start_with_env(ws.path(), &[("ACT_LITE_DISABLE_LSP", "1")]);
    assert_mcp_name_match(&mut mcp, "language servers are disabled");
}

#[test]
fn callers_needs_a_language_server_on_cli_and_mcp() {
    let ws = fixture("r");
    let output = cli(&ws, &["query", "callers", "scale_by", "--file", "scale.R"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("needs a language server, and act-lite has none configured for R files"),
        "{stderr}"
    );
    assert!(!stderr.contains("hint:"), "{stderr}");
    let message = Mcp::start(&ws)
        .call(
            "callers",
            json!({ "symbol": "scale_by", "file": "scale.R" }),
        )
        .unwrap_err();
    assert!(
        message.contains(
            "query type 'callers' needs a language server, and act-lite has none configured for R files"
        ),
        "{message}"
    );
}
