//! Svelte component files through the built binary: `query skeleton`,
//! `query symbols` and `query symbol` report the markup structure and the
//! declarations of each script and style region at their locations in the
//! component file, and `parse_error` follows the outer tree and the region
//! trees.

mod langs_added;

use langs_added::{Mcp, cli_json};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

/// A temporary workspace holding each `(path, content)` of `files`.
fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        std::fs::write(dir.path().join(rel), content).unwrap();
    }
    dir
}

fn counter() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/components/Counter.svelte");
    std::fs::read_to_string(path).unwrap()
}

fn point(location: &Value) -> String {
    format!("{}:{}", location["line"], location["column"])
}

fn compact(range: &Value) -> String {
    format!("{}-{}", point(&range["start"]), point(&range["end"]))
}

fn offsets(range: &Value) -> (u64, u64) {
    (
        range["start"]["byte_offset"].as_u64().unwrap(),
        range["end"]["byte_offset"].as_u64().unwrap(),
    )
}

/// `(kind, name, range, name_range)` of a declaration.
type Row = (String, String, String, String);

/// `(kind, name, range)` of a symbol.
type SymbolRow = (String, String, String);

/// `(kind, name, range, name_range)` of each declaration, in output order.
fn skeleton_rows(skeleton: &Value) -> Vec<Row> {
    skeleton["declarations"]
        .as_array()
        .unwrap_or_else(|| panic!("no declarations: {skeleton}"))
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
                compact(&d["range"]),
                if d["name_range"].is_null() {
                    "-".to_string()
                } else {
                    compact(&d["name_range"])
                },
            )
        })
        .collect()
}

/// `(kind, name, range)` of each symbol, in output order.
fn symbol_rows(symbols: &Value) -> Vec<SymbolRow> {
    symbols["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("no symbols: {symbols}"))
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

fn row(kind: &str, name: &str, range: &str, name_range: &str) -> Row {
    (
        kind.to_string(),
        name.to_string(),
        range.to_string(),
        name_range.to_string(),
    )
}

fn sym(kind: &str, name: &str, range: &str) -> SymbolRow {
    (kind.to_string(), name.to_string(), range.to_string())
}

#[test]
fn skeleton_gives_the_counter_table_and_its_byte_offsets() {
    let ws = workspace(&[("Counter.svelte", &counter())]);
    let skeleton = cli_json(ws.path(), &["query", "skeleton", "Counter.svelte"]);
    assert!(skeleton.get("parse_error").is_none(), "{skeleton}");
    let got = skeleton_rows(&skeleton);
    let want = vec![
        row(
            "module",
            "<script context=\"module\" lang=\"ts\">",
            "1:1-3:10",
            "1:1-1:36",
        ),
        row("export", "export const prerender = true;", "2:3-2:33", "-"),
        row("variable", "prerender", "2:10-2:33", "2:16-2:25"),
        row("element", "p", "5:1-5:15", "5:2-5:3"),
        row("module", "<script lang=\"ts\">", "5:15-13:10", "5:15-5:33"),
        row("export", "export let start: number = 0;", "5:33-5:62", "-"),
        row("variable", "start", "5:40-5:62", "5:44-5:49"),
        row("variable", "count", "6:3-6:21", "6:7-6:12"),
        row("function", "increment", "8:3-12:4", "8:12-8:21"),
        row("element", "div", "15:1-21:7", "15:2-15:5"),
        row("field", "counter", "15:6-15:21", "15:13-15:20"),
        row("element", "button", "16:3-16:51", "16:4-16:10"),
        row("element", "span", "18:5-18:35", "18:6-18:10"),
        row("function", "row", "20:3-20:48", "20:13-20:16"),
        row("element", "i", "20:28-20:38", "20:29-20:30"),
        row("module", "<style>", "23:1-25:9", "23:1-23:8"),
        row("class", ".counter", "24:3-24:27", "24:3-24:11"),
        row("field", "color", "24:14-24:25", "24:14-24:19"),
    ];
    let got_set: BTreeSet<_> = got.iter().cloned().collect();
    let want_set: BTreeSet<_> = want.iter().cloned().collect();
    assert_eq!(got_set, want_set, "{got:#?}");
    let blocks = |rows: &[Row]| -> Vec<String> {
        rows.iter()
            .filter(|r| r.0 == "module")
            .map(|r| r.1.clone())
            .collect()
    };
    assert_eq!(blocks(&got), blocks(&want));

    let declarations = skeleton["declarations"].as_array().unwrap();
    let find = |kind: &str, name: &str| {
        declarations
            .iter()
            .find(|d| d["kind"] == kind && d["name"] == name)
            .unwrap_or_else(|| panic!("no {kind} {name}"))
    };
    assert_eq!(
        offsets(&find("variable", "prerender")["name_range"]),
        (51, 60)
    );
    assert_eq!(
        offsets(&find("module", "<script lang=\"ts\">")["range"]),
        (94, 300)
    );
    assert_eq!(
        offsets(&find("export", "export let start: number = 0;")["range"]),
        (112, 141)
    );
    assert_eq!(
        offsets(&find("variable", "start")["name_range"]),
        (123, 128)
    );
}

#[test]
fn symbols_give_blocks_elements_fields_the_snippet_and_region_symbols() {
    let ws = workspace(&[("Counter.svelte", &counter())]);
    let symbols = cli_json(ws.path(), &["query", "symbols", "Counter.svelte"]);
    assert!(symbols.get("parse_error").is_none(), "{symbols}");
    let got: BTreeSet<_> = symbol_rows(&symbols).into_iter().collect();
    for want in [
        sym(
            "module",
            "<script context=\"module\" lang=\"ts\">",
            "1:1-1:36",
        ),
        sym("variable", "prerender", "2:16-2:25"),
        sym("class", "p", "5:2-5:3"),
        sym("module", "<script lang=\"ts\">", "5:15-5:33"),
        sym("variable", "start", "5:44-5:49"),
        sym("variable", "count", "6:7-6:12"),
        sym("function", "increment", "8:12-8:21"),
        sym("class", "div", "15:2-15:5"),
        sym("field", "counter", "15:13-15:20"),
        sym("class", "button", "16:4-16:10"),
        sym("class", "span", "18:6-18:10"),
        sym("function", "row", "20:13-20:16"),
        sym("class", "i", "20:29-20:30"),
        sym("module", "<style>", "23:1-23:8"),
        sym("field", "color", "24:14-24:19"),
    ] {
        assert!(got.contains(&want), "missing {want:?} in {got:#?}");
    }
    assert!(!got.iter().any(|s| s.1 == "doubled"), "{got:#?}");
}

#[test]
fn symbol_prerender_reads_source_and_context_from_the_component_file() {
    let ws = workspace(&[("Counter.svelte", &counter())]);
    let found = cli_json(
        ws.path(),
        &["query", "symbol", "prerender", "--file", "Counter.svelte"],
    );
    assert_eq!(compact(&found["symbol"]["range"]), "2:16-2:25", "{found}");
    assert_eq!(found["source"], "prerender");
    assert_eq!(
        found["context_before"],
        "<script context=\"module\" lang=\"ts\">"
    );
    assert_eq!(
        found["context_after"],
        "</script>\n\n<p>Zähler</p><script lang=\"ts\">export let start: number = 0;"
    );
}

#[test]
fn symbol_with_one_name_in_two_regions_is_the_first_by_range_start() {
    let text = "<script context=\"module\">\n  function f() {}\n</script>\n<script>\n  function f() {}\n</script>\n";
    let ws = workspace(&[("Two.svelte", text)]);
    let found = cli_json(ws.path(), &["query", "symbol", "f", "--file", "Two.svelte"]);
    assert_eq!(compact(&found["symbol"]["range"]), "2:12-2:13", "{found}");
}

/// The skeleton rows and the `parse_error` value of `query symbols` for one
/// file written into its own workspace.
fn navigate(name: &str, text: &str) -> (Vec<Row>, Value, Value) {
    let ws = workspace(&[(name, text)]);
    let skeleton = cli_json(ws.path(), &["query", "skeleton", name]);
    let symbols = cli_json(ws.path(), &["query", "symbols", name]);
    let parse_error = symbols.get("parse_error").cloned().unwrap_or(Value::Null);
    (skeleton_rows(&skeleton), symbols, parse_error)
}

#[test]
fn the_svelte_fixtures_give_their_stated_results() {
    // An unsupported script language: the block only.
    let (rows, _, parse_error) = navigate(
        "Ts.svelte",
        "<script lang=\"typescript\">\nlet a: number = 1\n</script>\n",
    );
    assert_eq!(
        rows,
        vec![row(
            "module",
            "<script lang=\"typescript\">",
            "1:1-3:10",
            "1:1-1:27"
        )]
    );
    assert_eq!(parse_error, Value::Null);

    // A Svelte 5 module script and an instance script: two TypeScript regions.
    let (rows, _, parse_error) = navigate(
        "Module.svelte",
        "<script module lang=\"ts\">\n  export const x = 1;\n</script>\n<script lang=\"ts\" generics=\"T\">\n  let { items }: { items: T[] } = $props();\n</script>\n",
    );
    assert!(
        rows.contains(&row("variable", "x", "2:10-2:22", "2:16-2:17")),
        "{rows:#?}"
    );
    assert_eq!(
        rows.iter().filter(|r| r.0 == "module").count(),
        2,
        "{rows:#?}"
    );
    assert_eq!(parse_error, Value::Null);

    // A script inside markup is no region.
    let (rows, symbols, parse_error) = navigate(
        "Nested.svelte",
        "<svelte:head><script>var g = 1;</script></svelte:head>\n",
    );
    assert_eq!(
        rows,
        vec![row("element", "svelte:head", "1:1-1:55", "1:2-1:13")]
    );
    assert!(!symbol_rows(&symbols).iter().any(|s| s.1 == "g"));
    assert_eq!(parse_error, Value::Null);

    // A class value holding an expression is not reported.
    let (rows, _, parse_error) = navigate("Class.svelte", "<p class=\"a {x}\" id=\"i\">t</p>\n");
    assert_eq!(
        rows,
        vec![
            row("element", "p", "1:1-1:30", "1:2-1:3"),
            row("field", "i", "1:18-1:24", "1:22-1:23"),
        ]
    );
    assert_eq!(parse_error, Value::Null);

    // A template expression is not a symbol.
    let (rows, symbols, parse_error) = navigate("Expr.svelte", "<p>{onlyhere}</p>\n");
    assert_eq!(rows, vec![row("element", "p", "1:1-1:18", "1:2-1:3")]);
    assert!(!symbol_rows(&symbols).iter().any(|s| s.1 == "onlyhere"));
    assert_eq!(parse_error, Value::Null);

    // `&` in text is an error of the outer tree; the script outside it is
    // still a region.
    let (rows, symbols, parse_error) = navigate(
        "Amp.svelte",
        "<p>Set & go</p>\n<script>function f() {}</script>\n",
    );
    assert!(
        rows.contains(&row("module", "<script>", "2:1-2:33", "2:1-2:9")),
        "{rows:#?}"
    );
    assert!(symbol_rows(&symbols).contains(&sym("function", "f", "2:18-2:19")));
    assert_eq!(parse_error, Value::Bool(true));
}

#[test]
fn crlf_keeps_lines_and_columns_and_shifts_offsets_by_the_line() {
    let lf = counter();
    let crlf = lf.replace('\n', "\r\n");
    let ws = workspace(&[("Counter.svelte", &lf), ("Crlf.svelte", &crlf)]);
    let a = cli_json(ws.path(), &["query", "skeleton", "Counter.svelte"]);
    let b = cli_json(ws.path(), &["query", "skeleton", "Crlf.svelte"]);
    assert!(b.get("parse_error").is_none(), "{b}");
    assert_eq!(skeleton_rows(&a), skeleton_rows(&b));
    let shift = |range: &Value| {
        let (s, e) = offsets(range);
        (
            s + range["start"]["line"].as_u64().unwrap() - 1,
            e + range["end"]["line"].as_u64().unwrap() - 1,
        )
    };
    let (da, db) = (
        a["declarations"].as_array().unwrap(),
        b["declarations"].as_array().unwrap(),
    );
    assert_eq!(da.len(), 18);
    for (x, y) in da.iter().zip(db) {
        assert_eq!(offsets(&y["range"]), shift(&x["range"]), "{x}");
        if !x["name_range"].is_null() {
            assert_eq!(offsets(&y["name_range"]), shift(&x["name_range"]), "{x}");
        }
    }
}

#[test]
fn mcp_symbols_give_the_names_and_kinds_of_the_cli() {
    let ws = workspace(&[("Counter.svelte", &counter())]);
    let cli: BTreeSet<(String, String)> = symbol_rows(&cli_json(
        ws.path(),
        &["query", "symbols", "Counter.svelte"],
    ))
    .into_iter()
    .map(|(kind, name, _)| (kind, name))
    .collect();
    let result = Mcp::start(ws.path())
        .call("symbols", json!({ "file": "Counter.svelte" }))
        .unwrap();
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    let payload: Value = serde_json::from_str(text.split("\nNote:").next().unwrap_or_default())
        .unwrap_or_else(|e| panic!("not JSON ({e}): {result}"));
    let mcp: BTreeSet<(String, String)> = payload["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("no symbols: {payload}"))
        .iter()
        .map(|s| {
            (
                s["kind"].as_str().unwrap().to_lowercase(),
                s["name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(mcp, cli);
    assert!(payload.get("parse_error").is_none(), "{payload}");
}
