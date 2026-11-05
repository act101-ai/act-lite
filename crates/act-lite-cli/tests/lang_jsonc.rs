//! JSONC: JSON's grammar and queries under its own name, the JSON/JSONC
//! cross-file walks, comments, trailing commas and the name-level evidence.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

const SETTINGS: &str = "{\n  // line comment\n  \"compilerOptions\": {\n    /* block */\n    \"strict\": true,\n  },\n  \"files\": [\"a\", \"b\",],\n}\n";

const NAME_LEVEL_HINT: &str =
    "Name-level baseline — a same-name symbol in an unrelated scope can appear.";

/// A new temporary workspace holding each `(file, content)`.
fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    for (file, content) in files {
        std::fs::write(ws.path().join(file), content).unwrap();
    }
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

/// `value` with every `file` string replaced by `F`.
fn without_file_names(mut value: Value) -> Value {
    match &mut value {
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                *item = if key == "file" && item.is_string() {
                    json!("F")
                } else {
                    without_file_names(item.take())
                };
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                *item = without_file_names(item.take());
            }
        }
        _ => {}
    }
    value
}

/// The file names (last path component) a CLI `references` answer lists.
fn referenced_files(workspace: &Path, file: &str, line: &str, column: &str) -> BTreeSet<String> {
    let answer = cli_json(
        workspace,
        &[
            "query",
            "references",
            "name",
            "--file",
            file,
            "--line",
            line,
            "--column",
            column,
        ],
    );
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    answer["references"]
        .as_array()
        .unwrap_or_else(|| panic!("no references in {answer}"))
        .iter()
        .map(|r| {
            Path::new(r["file"].as_str().unwrap())
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .collect()
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn fixture_holds_the_design_bytes() {
    assert_eq!(
        std::fs::read_to_string(fixture("jsonc/settings.jsonc")).unwrap(),
        SETTINGS
    );
}

#[test]
fn skeleton_is_the_table_set() {
    let skeleton = cli_json(&fixture("jsonc"), &["query", "skeleton", "settings.jsonc"]);
    let actual: BTreeSet<(String, String, String, String)> = skeleton["declarations"]
        .as_array()
        .unwrap_or_else(|| panic!("no declarations in {skeleton}"))
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
    let expected: BTreeSet<(String, String, String, String)> = [
        ("module", "compilerOptions", "3:22-6:4", "3:4-3:19"),
        ("variable", "strict", "5:15-5:19", "5:6-5:12"),
        ("module", "files", "7:12-7:23", "7:4-7:9"),
    ]
    .into_iter()
    .map(|(k, n, r, nr)| (k.into(), n.into(), r.into(), nr.into()))
    .collect();
    assert_eq!(actual, expected);
}

#[test]
fn symbols_are_the_table_set_with_parse_error_and_every_key() {
    let symbols = cli_json(&fixture("jsonc"), &["query", "symbols", "settings.jsonc"]);
    let actual: BTreeSet<(String, String, String)> = symbols["symbols"]
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
    let expected: BTreeSet<(String, String, String)> = [
        ("field", "\"compilerOptions\"", "3:3-3:20"),
        ("field", "\"strict\"", "5:5-5:13"),
        ("field", "\"files\"", "7:3-7:10"),
    ]
    .into_iter()
    .map(|(k, n, r)| (k.into(), n.into(), r.into()))
    .collect();
    assert_eq!(actual, expected);
    assert_eq!(symbols["parse_error"], true, "{symbols}");
}

#[test]
fn symbols_batch_lists_the_skeleton_set_on_cli_and_mcp() {
    let expected: BTreeSet<(String, String)> = [
        ("compilerOptions", "module"),
        ("strict", "variable"),
        ("files", "module"),
    ]
    .into_iter()
    .map(|(n, k)| (n.into(), k.into()))
    .collect();
    let pairs = |batch: &Value| -> BTreeSet<(String, String)> {
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
    let ws = fixture("jsonc");
    let batch = cli_json(
        &ws,
        &["query", "symbols-batch", "--files", "settings.jsonc"],
    );
    assert_eq!(pairs(&batch), expected, "{batch}");
    let result = Mcp::start(&ws)
        .call("symbols_batch", json!({ "files": ["settings.jsonc"] }))
        .unwrap();
    assert_eq!(pairs(&mcp_payload(&result)), expected, "{result}");
}

#[test]
fn query_symbol_by_bare_name_returns_the_key_range() {
    let symbol = cli_json(
        &fixture("jsonc"),
        &[
            "query",
            "symbol",
            "compilerOptions",
            "--file",
            "settings.jsonc",
        ],
    );
    assert_eq!(span(&symbol["symbol"]["range"]), "3:3-3:20", "{symbol}");
}

#[test]
fn mcp_skeleton_and_symbols_give_the_table_names_and_kinds() {
    let mut mcp = Mcp::start(&fixture("jsonc"));
    let skeleton = mcp_payload(
        &mcp.call("skeleton", json!({ "file": "settings.jsonc" }))
            .unwrap(),
    );
    let actual: BTreeSet<(String, String)> = skeleton["declarations"]
        .as_array()
        .unwrap_or_else(|| panic!("no declarations in {skeleton}"))
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let expected: BTreeSet<(String, String)> = [
        ("module", "compilerOptions"),
        ("variable", "strict"),
        ("module", "files"),
    ]
    .into_iter()
    .map(|(k, n)| (k.into(), n.into()))
    .collect();
    assert_eq!(actual, expected);

    let symbols = mcp_payload(
        &mcp.call("symbols", json!({ "file": "settings.jsonc" }))
            .unwrap(),
    );
    let actual: BTreeSet<(String, String)> = symbols["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("no symbols in {symbols}"))
        .iter()
        .map(|s| {
            (
                s["kind"].as_str().unwrap().to_string(),
                s["name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let expected: BTreeSet<(String, String)> = [
        ("Field", "\"compilerOptions\""),
        ("Field", "\"strict\""),
        ("Field", "\"files\""),
    ]
    .into_iter()
    .map(|(k, n)| (k.into(), n.into()))
    .collect();
    assert_eq!(actual, expected);
}

/// `skeleton`, `symbols` and `definition` on `SETTINGS` saved as `name`,
/// alone in a workspace, with every file name replaced.
fn answers_for(name: &str) -> Vec<Value> {
    let ws = workspace(&[(name, SETTINGS)]);
    let at = |args: &[&str]| without_file_names(cli_json(ws.path(), args));
    vec![
        at(&["query", "skeleton", name]),
        at(&["query", "symbols", name]),
        at(&[
            "query",
            "definition",
            "strict",
            "--file",
            name,
            "--line",
            "5",
            "--column",
            "6",
        ]),
    ]
}

#[test]
fn jsonc_json_and_tsconfig_answer_identically() {
    let jsonc = answers_for("settings.jsonc");
    assert_eq!(jsonc[2]["definition"]["line"], 5, "{}", jsonc[2]);
    assert_eq!(jsonc[2]["definition"]["column"], 6, "{}", jsonc[2]);
    assert_eq!(jsonc[2]["definition"]["end_column"], 12, "{}", jsonc[2]);
    assert_eq!(answers_for("b.json"), jsonc);
    assert_eq!(answers_for("tsconfig.json"), jsonc);
}

#[test]
fn references_walk_json_and_named_jsonc_as_before_and_everything_from_jsonc() {
    let ws = workspace(&[
        ("package.json", "{\"name\": 1}\n"),
        ("tsconfig.json", "{\"name\": 2}\n"),
        ("x.jsonc", "{\"name\": 3}\n"),
    ]);
    let json_family = names(&["package.json", "tsconfig.json"]);
    assert_eq!(
        referenced_files(ws.path(), "package.json", "1", "3"),
        json_family
    );
    assert_eq!(
        referenced_files(ws.path(), "tsconfig.json", "1", "3"),
        json_family
    );
    assert_eq!(
        referenced_files(ws.path(), "x.jsonc", "1", "3"),
        names(&["package.json", "tsconfig.json", "x.jsonc"])
    );
}

#[test]
fn definition_walk_reads_named_jsonc_from_json_and_everything_from_jsonc() {
    let ws = workspace(&[
        ("package.json", "{\"name\": 1}\n"),
        ("tsconfig.json", "{\"inTsconfig\": 2}\n"),
        ("x.jsonc", "{\"inJsonc\": 3}\n"),
    ]);
    let definition = |file: &str, symbol: &str| {
        cli(
            ws.path(),
            &[
                "query",
                "definition",
                symbol,
                "--file",
                file,
                "--line",
                "1",
                "--column",
                "3",
            ],
        )
    };
    let found_in = |output: std::process::Output| -> String {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
        Path::new(answer["definition"]["file"].as_str().unwrap())
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string()
    };
    assert_eq!(
        found_in(definition("package.json", "inTsconfig")),
        "tsconfig.json"
    );
    let output = definition("package.json", "inJsonc");
    assert!(!output.status.success(), "a .jsonc key found from JSON");
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("No definition found for symbol 'inJsonc'")
    );
    let output = definition("tsconfig.json", "inJsonc");
    assert!(
        !output.status.success(),
        "a .jsonc key found from tsconfig.json"
    );
    assert_eq!(
        found_in(definition("x.jsonc", "inTsconfig")),
        "tsconfig.json"
    );
    assert_eq!(found_in(definition("x.jsonc", "name")), "package.json");
}

#[test]
fn references_from_package_json_skip_the_dot_named_eslintrc() {
    let ws = workspace(&[
        ("package.json", "{\"name\": 1}\n"),
        (".eslintrc.json", "{\"name\": 2}\n"),
    ]);
    assert_eq!(
        referenced_files(ws.path(), "package.json", "1", "3"),
        names(&["package.json"])
    );
}

const SAME_NAME: &str = "{\"a\": {\"id\": 1}, \"b\": {\"id\": 2}}\n";

#[test]
fn same_name_keys_are_a_name_match_on_cli() {
    let ws = workspace(&[("x.jsonc", SAME_NAME)]);
    let answer = cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "id",
            "--file",
            "x.jsonc",
            "--line",
            "1",
            "--column",
            "9",
        ],
    );
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    let sites: BTreeSet<(u64, u64)> = answer["references"]
        .as_array()
        .unwrap_or_else(|| panic!("no references in {answer}"))
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect();
    assert_eq!(sites, [(1, 9), (1, 25)].into_iter().collect());
}

fn assert_mcp_name_match(mcp: &mut Mcp, reason: &str) {
    let result = mcp
        .call("references", json!({ "symbol": "id", "file": "x.jsonc" }))
        .unwrap();
    let answer = mcp_payload(&result);
    assert_eq!(
        answer["resolved_by"],
        format!("tree-sitter workspace baseline (name-level; {reason})"),
        "{answer}"
    );
    assert_eq!(
        answer["references"].as_array().map(Vec::len),
        Some(2),
        "{answer}"
    );
    let text = mcp_text(&result);
    assert!(text.contains(NAME_LEVEL_HINT), "{text}");
    assert!(!text.contains("Install"), "{text}");
}

#[test]
fn same_name_keys_are_a_name_match_on_mcp() {
    let ws = workspace(&[("x.jsonc", SAME_NAME)]);
    assert_mcp_name_match(&mut Mcp::start(ws.path()), "no language server");
}

#[test]
fn mcp_reason_reads_disabled_when_language_servers_are_disabled() {
    let ws = workspace(&[("x.jsonc", SAME_NAME)]);
    let mut mcp = Mcp::start_with_env(ws.path(), &[("ACT_LITE_DISABLE_LSP", "1")]);
    assert_mcp_name_match(&mut mcp, "language servers are disabled");
}

#[test]
fn callers_needs_a_language_server_on_cli_and_mcp() {
    let ws = fixture("jsonc");
    let output = cli(
        &ws,
        &[
            "query",
            "callers",
            "compilerOptions",
            "--file",
            "settings.jsonc",
        ],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr
            .contains("needs a language server, and act-lite has none configured for JSONC files"),
        "{stderr}"
    );
    assert!(!stderr.contains("hint:"), "{stderr}");
    let message = Mcp::start(&ws)
        .call(
            "callers",
            json!({ "symbol": "compilerOptions", "file": "settings.jsonc" }),
        )
        .unwrap_err();
    assert!(
        message.contains(
            "query type 'callers' needs a language server, and act-lite has none configured for JSONC files"
        ),
        "{message}"
    );
}

#[test]
fn a_commented_key_is_not_reported() {
    let ws = workspace(&[("c.jsonc", "{\"a\": 1, // \"hidden\": 2\n \"b\": 2}\n")]);
    let symbols = cli_json(ws.path(), &["query", "symbols", "c.jsonc"]);
    let actual: BTreeSet<String> = symbols["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("no symbols in {symbols}"))
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(actual, names(&["\"a\"", "\"b\""]));
    assert!(symbols.get("parse_error").is_none(), "{symbols}");
}
