//! Nix: skeleton, symbols, binding and parameter forms, the
//! structural-not-evaluated limitation, the name-level reference evidence
//! and the language-server errors.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const DEFAULT_NIX: &str = "{ pkgs ? import <nixpkgs> {}, lib, ... }:
let
  version = \"1.0\";
  mkTool = name: pkgs.writeShellScriptBin name \"echo ${name}\";
  common = import ./common.nix { inherit lib; };
in
{
  inherit version;
  packages.default = mkTool \"shop\";
  services.web = { enable = true; port = 8080; };
  check = if version == \"1.0\" then true else false;
}
";

const NAME_LEVEL_HINT: &str =
    "Name-level baseline — a same-name symbol in an unrelated scope can appear.";

const SAME_NAME: &str = "let x = 1; in { a = let x = 2; in x; b = x; }\n";

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

fn mcp_payload(result: &Value) -> Value {
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    let json = text.split("\nNote:").next().unwrap_or_default();
    serde_json::from_str(json).unwrap_or_else(|e| panic!("not JSON ({e}): {result}"))
}

fn mcp_text(result: &Value) -> String {
    result["content"]
        .as_array()
        .unwrap_or_else(|| panic!("no content in {result}"))
        .iter()
        .filter_map(|block| block["text"].as_str())
        .collect()
}

fn items<'a>(answer: &'a Value, key: &str) -> &'a Vec<Value> {
    answer[key]
        .as_array()
        .unwrap_or_else(|| panic!("no {key} in {answer}"))
}

/// The `(kind, name)` pairs of `answer[key]`.
fn kind_names(answer: &Value, key: &str) -> BTreeSet<(String, String)> {
    items(answer, key)
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    list.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

const SKELETON_NAMES: [(&str, &str); 12] = [
    ("import", "<nixpkgs>"),
    ("variable", "version"),
    ("function", "mkTool"),
    ("variable", "common"),
    ("import", "./common.nix"),
    ("variable", "lib"),
    ("variable", "packages.default"),
    ("module", "services.web"),
    ("variable", "enable"),
    ("variable", "port"),
    ("variable", "check"),
    ("variable", "version"),
];

fn skeleton_names() -> BTreeSet<(String, String)> {
    pairs(&SKELETON_NAMES)
}

#[test]
fn fixture_holds_the_design_bytes() {
    assert_eq!(
        std::fs::read_to_string(fixture("nix/default.nix")).unwrap(),
        DEFAULT_NIX
    );
}

#[test]
fn skeleton_is_the_table_set() {
    let skeleton = cli_json(&fixture("nix"), &["query", "skeleton", "default.nix"]);
    let actual: BTreeSet<(String, String, String, String)> = items(&skeleton, "declarations")
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
        ("import", "<nixpkgs>", "1:10-1:26", "1:17-1:26"),
        ("variable", "version", "3:3-3:19", "3:3-3:10"),
        ("function", "mkTool", "4:3-4:63", "4:3-4:9"),
        ("variable", "common", "5:3-5:49", "5:3-5:9"),
        ("import", "./common.nix", "5:12-5:31", "5:19-5:31"),
        ("variable", "lib", "5:34-5:46", "5:42-5:45"),
        ("variable", "version", "8:3-8:19", "8:11-8:18"),
        ("variable", "packages.default", "9:3-9:36", "9:3-9:19"),
        ("module", "services.web", "10:3-10:50", "10:3-10:15"),
        ("variable", "enable", "10:20-10:34", "10:20-10:26"),
        ("variable", "port", "10:35-10:47", "10:35-10:39"),
        ("variable", "check", "11:3-11:52", "11:3-11:8"),
    ]
    .into_iter()
    .map(|(k, n, r, nr)| (k.into(), n.into(), r.into(), nr.into()))
    .collect();
    assert_eq!(actual, expected);
}

#[test]
fn symbols_are_the_table_set() {
    let symbols = cli_json(&fixture("nix"), &["query", "symbols", "default.nix"]);
    let actual: BTreeSet<(String, String, String)> = items(&symbols, "symbols")
        .iter()
        .map(|s| {
            let name = s["name"].as_str().unwrap();
            assert!(!name.is_empty(), "empty name in {s}");
            assert_ne!(s["range"]["start"], s["range"]["end"], "zero width in {s}");
            (
                s["kind"].as_str().unwrap().to_string(),
                name.to_string(),
                span(&s["range"]),
            )
        })
        .collect();
    let expected: BTreeSet<(String, String, String)> = [
        ("parameter", "pkgs", "1:3-1:7"),
        ("parameter", "lib", "1:31-1:34"),
        ("variable", "version", "3:3-3:10"),
        ("function", "mkTool", "4:3-4:9"),
        ("parameter", "name", "4:12-4:16"),
        ("variable", "common", "5:3-5:9"),
        ("variable", "lib", "5:42-5:45"),
        ("variable", "version", "8:11-8:18"),
        ("variable", "packages.default", "9:3-9:19"),
        ("module", "services.web", "10:3-10:15"),
        ("variable", "enable", "10:20-10:26"),
        ("variable", "port", "10:35-10:39"),
        ("variable", "check", "11:3-11:8"),
    ]
    .into_iter()
    .map(|(k, n, r)| (k.into(), n.into(), r.into()))
    .collect();
    assert_eq!(actual, expected);
    assert!(symbols.get("parse_error").is_none(), "{symbols}");
}

#[test]
fn symbols_batch_lists_the_skeleton_set_on_cli_and_mcp() {
    let expected: BTreeSet<(String, String)> =
        skeleton_names().into_iter().map(|(k, n)| (n, k)).collect();
    let by_name_kind = |batch: &Value| -> BTreeSet<(String, String)> {
        items(&batch["results"][0], "symbols")
            .iter()
            .map(|s| {
                (
                    s["name"].as_str().unwrap().to_string(),
                    s["kind"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    let ws = fixture("nix");
    let batch = cli_json(&ws, &["query", "symbols-batch", "--files", "default.nix"]);
    assert_eq!(by_name_kind(&batch), expected, "{batch}");
    let result = Mcp::start(&ws)
        .call("symbols_batch", json!({ "files": ["default.nix"] }))
        .unwrap();
    assert_eq!(by_name_kind(&mcp_payload(&result)), expected, "{result}");
}

#[test]
fn query_symbol_returns_the_name_range() {
    let symbol = cli_json(
        &fixture("nix"),
        &["query", "symbol", "mkTool", "--file", "default.nix"],
    );
    assert_eq!(span(&symbol["symbol"]["range"]), "4:3-4:9", "{symbol}");
}

#[test]
fn mcp_skeleton_and_symbols_give_the_table_names_and_kinds() {
    let mut mcp = Mcp::start(&fixture("nix"));
    let skeleton = mcp_payload(
        &mcp.call("skeleton", json!({ "file": "default.nix" }))
            .unwrap(),
    );
    let without_imports: BTreeSet<(String, String)> = skeleton_names()
        .into_iter()
        .filter(|(kind, _)| kind != "import")
        .collect();
    assert_eq!(kind_names(&skeleton, "declarations"), without_imports);

    let symbols = mcp_payload(
        &mcp.call("symbols", json!({ "file": "default.nix" }))
            .unwrap(),
    );
    let expected = pairs(&[
        ("Parameter", "pkgs"),
        ("Parameter", "lib"),
        ("Variable", "version"),
        ("Function", "mkTool"),
        ("Parameter", "name"),
        ("Variable", "common"),
        ("Variable", "lib"),
        ("Module", "services.web"),
        ("Variable", "packages.default"),
        ("Variable", "enable"),
        ("Variable", "port"),
        ("Variable", "check"),
    ]);
    assert_eq!(kind_names(&symbols, "symbols"), expected);
}

fn skeleton_of(source: &str) -> Value {
    let ws = workspace(&[("default.nix", source)]);
    cli_json(ws.path(), &["query", "skeleton", "default.nix"])
}

fn symbols_of(source: &str) -> Value {
    let ws = workspace(&[("default.nix", source)]);
    cli_json(ws.path(), &["query", "symbols", "default.nix"])
}

#[test]
fn inherit_from_declares_each_name_with_the_inherit_range() {
    let skeleton = skeleton_of("{ inherit (s) a b; }\n");
    let found: BTreeSet<(String, String, String, String)> = items(&skeleton, "declarations")
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
    let expected = [("a", "1:15-1:16"), ("b", "1:17-1:18")]
        .into_iter()
        .map(|(n, nr)| {
            (
                "variable".to_string(),
                n.to_string(),
                "1:3-1:19".to_string(),
                nr.to_string(),
            )
        })
        .collect();
    assert_eq!(found, expected);
}

#[test]
fn rec_sets_and_nested_bindings_are_declarations_at_any_depth() {
    let symbols = symbols_of("rec { a = { b = { c = 1; }; }; d = a; }\n");
    assert_eq!(
        kind_names(&symbols, "symbols"),
        pairs(&[
            ("module", "a"),
            ("module", "b"),
            ("variable", "c"),
            ("variable", "d"),
        ])
    );
}

#[test]
fn let_bindings_and_parameters_are_symbols() {
    let symbols = symbols_of("{ a, b ? 1 }: c: let d = 1; in d\n");
    assert_eq!(
        kind_names(&symbols, "symbols"),
        pairs(&[
            ("parameter", "a"),
            ("parameter", "b"),
            ("parameter", "c"),
            ("variable", "d"),
        ])
    );
}

#[test]
fn an_interpolated_attribute_name_is_reported_as_written() {
    let symbols = symbols_of("{ ${n} = 1; }\n");
    assert_eq!(
        kind_names(&symbols, "symbols"),
        pairs(&[("variable", "${n}")])
    );
}

#[test]
fn an_imports_list_is_a_binding_and_is_not_followed() {
    let skeleton = skeleton_of("{ imports = [ ./a.nix ]; }\n");
    assert_eq!(
        kind_names(&skeleton, "declarations"),
        pairs(&[("variable", "imports")])
    );
}

#[test]
fn import_reports_the_path_only_and_not_the_applied_arguments() {
    let skeleton = skeleton_of("{ x = import ./c.nix { a = 1; }; }\n");
    let imports: Vec<(String, String)> = items(&skeleton, "declarations")
        .iter()
        .filter(|d| d["kind"] == "import")
        .map(|d| (d["name"].as_str().unwrap().to_string(), span(&d["range"])))
        .collect();
    assert_eq!(
        imports,
        vec![("./c.nix".to_string(), "1:7-1:21".to_string())]
    );
}

#[test]
fn the_kind_is_the_syntax_of_the_bound_expression() {
    let symbols = symbols_of("{ f = g x; h = x: x; k = { }: 1; }\n");
    assert_eq!(
        kind_names(&symbols, "symbols"),
        pairs(&[
            ("variable", "f"),
            ("function", "h"),
            ("parameter", "x"),
            ("function", "k"),
        ])
    );
}

#[test]
fn bindings_under_a_conditional_wrapper_are_reported() {
    let symbols = symbols_of("{ a = lib.mkIf false { b = 1; }; }\n");
    let names = kind_names(&symbols, "symbols");
    assert!(
        names.contains(&("variable".to_string(), "b".to_string())),
        "{symbols}"
    );
}

#[test]
fn both_sides_of_a_merge_are_reported_as_written() {
    let symbols = symbols_of("{ p = x // { y = 1; }; }\n");
    let names = kind_names(&symbols, "symbols");
    assert!(
        names.contains(&("variable".to_string(), "y".to_string())),
        "{symbols}"
    );
    let symbols = symbols_of("x // { y = 1; }\n");
    assert_eq!(kind_names(&symbols, "symbols"), pairs(&[("variable", "y")]));
}

#[test]
fn same_name_bindings_are_a_name_match_on_cli() {
    let ws = workspace(&[("default.nix", SAME_NAME)]);
    let answer = cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "x",
            "--file",
            "default.nix",
            "--line",
            "1",
            "--column",
            "5",
        ],
    );
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    let sites: BTreeSet<(u64, u64)> = items(&answer, "references")
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect();
    assert_eq!(
        sites,
        [(1, 5), (1, 25), (1, 35), (1, 42)].into_iter().collect()
    );
}

#[test]
fn a_dotted_declaration_matches_the_attribute_path_of_a_select() {
    let ws = workspace(&[(
        "default.nix",
        "{ packages.default = 1; x = self.packages.default; }\n",
    )]);
    let answer = cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "packages.default",
            "--file",
            "default.nix",
            "--line",
            "1",
            "--column",
            "3",
        ],
    );
    let sites: BTreeSet<(u64, u64)> = items(&answer, "references")
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect();
    assert_eq!(sites, [(1, 3), (1, 34)].into_iter().collect());
}

#[test]
fn parameters_and_inherit_lists_are_name_matches() {
    let ws = workspace(&[("default.nix", "{ lib }: { inherit lib; y = lib.f; }\n")]);
    let answer = cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "lib",
            "--file",
            "default.nix",
            "--line",
            "1",
            "--column",
            "3",
        ],
    );
    let sites: BTreeSet<(u64, u64)> = items(&answer, "references")
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect();
    assert_eq!(sites, [(1, 3), (1, 20), (1, 29)].into_iter().collect());
}

fn assert_mcp_name_match(mcp: &mut Mcp, reason: &str) {
    let result = mcp
        .call(
            "references",
            json!({ "symbol": "x", "file": "default.nix" }),
        )
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
}

#[test]
fn same_name_bindings_are_a_name_match_on_mcp() {
    let ws = workspace(&[("default.nix", SAME_NAME)]);
    assert_mcp_name_match(&mut Mcp::start(ws.path()), "no language server");
}

#[test]
fn mcp_reason_reads_disabled_when_language_servers_are_disabled() {
    let ws = workspace(&[("default.nix", SAME_NAME)]);
    let mut mcp = Mcp::start_with_env(ws.path(), &[("ACT_LITE_DISABLE_LSP", "1")]);
    assert_mcp_name_match(&mut mcp, "language servers are disabled");
}

#[test]
fn callers_needs_a_language_server_on_cli_and_mcp() {
    let ws = fixture("nix");
    let output = cli(
        &ws,
        &["query", "callers", "mkTool", "--file", "default.nix"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("needs a language server, and act-lite has none configured for Nix files"),
        "{stderr}"
    );
    assert!(!stderr.contains("hint:"), "{stderr}");
    let message = Mcp::start(&ws)
        .call(
            "callers",
            json!({ "symbol": "mkTool", "file": "default.nix" }),
        )
        .unwrap_err();
    assert!(
        message.contains(
            "query type 'callers' needs a language server, and act-lite has none configured for Nix files"
        ),
        "{message}"
    );
}
