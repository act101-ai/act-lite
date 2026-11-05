//! GraphQL: skeleton, symbols, the Federation-header limitation, the
//! name-level reference evidence and the language-server errors.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const SCHEMA: &str = "type User implements Node {\n  id: ID!\n  name(format: String): String\n}\nenum Role { ADMIN USER }\ninput NewUser { name: String! }\ninterface Node { id: ID! }\nunion Item = User | Post\nscalar Date\ndirective @auth on FIELD_DEFINITION\nschema { query: Query }\nextend type User { age: Int }\nquery GetUser($id: ID!) {\n  user(id: $id) { ...UserParts }\n}\nfragment UserParts on User { id name }\n{ me { id } }\n";

const NAME_LEVEL_HINT: &str =
    "Name-level baseline — a same-name symbol in an unrelated scope can appear.";

const SAME_NAME: &str = "type A { id: ID }\ntype B { id: ID }\n";

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

const SKELETON_NAMES: [(&str, &str); 10] = [
    ("class", "User"),
    ("enum", "Role"),
    ("struct", "NewUser"),
    ("interface", "Node"),
    ("union", "Item"),
    ("type_alias", "Date"),
    ("annotation", "auth"),
    ("class", "User"),
    ("function", "GetUser"),
    ("constant", "UserParts"),
];

fn skeleton_names() -> BTreeSet<(String, String)> {
    pairs(&SKELETON_NAMES)
}

#[test]
fn fixture_holds_the_design_bytes() {
    assert_eq!(
        std::fs::read_to_string(fixture("graphql/schema.graphql")).unwrap(),
        SCHEMA
    );
}

#[test]
fn skeleton_is_the_table_set() {
    let skeleton = cli_json(
        &fixture("graphql"),
        &["query", "skeleton", "schema.graphql"],
    );
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
        ("class", "User", "1:1-4:2", "1:6-1:10"),
        ("enum", "Role", "5:1-5:25", "5:6-5:10"),
        ("struct", "NewUser", "6:1-6:32", "6:7-6:14"),
        ("interface", "Node", "7:1-7:27", "7:11-7:15"),
        ("union", "Item", "8:1-8:25", "8:7-8:11"),
        ("type_alias", "Date", "9:1-9:12", "9:8-9:12"),
        ("annotation", "auth", "10:1-10:36", "10:12-10:16"),
        ("class", "User", "12:1-12:30", "12:13-12:17"),
        ("function", "GetUser", "13:1-15:2", "13:7-13:14"),
        ("constant", "UserParts", "16:1-16:39", "16:10-16:19"),
    ]
    .into_iter()
    .map(|(k, n, r, nr)| (k.into(), n.into(), r.into(), nr.into()))
    .collect();
    assert_eq!(actual, expected);
}

#[test]
fn symbols_are_the_table_set() {
    let symbols = cli_json(&fixture("graphql"), &["query", "symbols", "schema.graphql"]);
    let actual: BTreeSet<(String, String, String)> = items(&symbols, "symbols")
        .iter()
        .map(|s| {
            let name = s["name"].as_str().unwrap();
            assert!(!name.is_empty(), "empty name in {s}");
            let range = span(&s["range"]);
            assert_ne!(s["range"]["start"], s["range"]["end"], "zero width in {s}");
            (
                s["kind"].as_str().unwrap().to_string(),
                name.to_string(),
                range,
            )
        })
        .collect();
    let expected: BTreeSet<(String, String, String)> = [
        ("class", "User", "1:6-1:10"),
        ("field", "id", "2:3-2:5"),
        ("field", "name", "3:3-3:7"),
        ("enum", "Role", "5:6-5:10"),
        ("enum_member", "ADMIN", "5:13-5:18"),
        ("enum_member", "USER", "5:19-5:23"),
        ("struct", "NewUser", "6:7-6:14"),
        ("field", "name", "6:17-6:21"),
        ("interface", "Node", "7:11-7:15"),
        ("field", "id", "7:18-7:20"),
        ("enum", "Item", "8:7-8:11"),
        ("type_alias", "Date", "9:8-9:12"),
        ("attribute", "auth", "10:12-10:16"),
        ("class", "User", "12:13-12:17"),
        ("field", "age", "12:20-12:23"),
        ("function", "GetUser", "13:7-13:14"),
        ("constant", "UserParts", "16:10-16:19"),
    ]
    .into_iter()
    .map(|(k, n, r)| (k.into(), n.into(), r.into()))
    .collect();
    assert_eq!(actual, expected);
    assert!(symbols.get("parse_error").is_none(), "{symbols}");
}

#[test]
fn fields_lie_inside_their_type() {
    let symbols = cli_json(&fixture("graphql"), &["query", "symbols", "schema.graphql"]);
    let skeleton = cli_json(
        &fixture("graphql"),
        &["query", "skeleton", "schema.graphql"],
    );
    let user = &items(&skeleton, "declarations")[0];
    let line = |v: &Value| v["start"]["line"].as_u64().unwrap();
    let id = items(&symbols, "symbols")
        .iter()
        .find(|s| s["name"] == "id")
        .unwrap();
    assert!(line(&user["range"]) <= line(&id["range"]));
    assert!(line(&id["range"]) <= user["range"]["end"]["line"].as_u64().unwrap());
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
    let ws = fixture("graphql");
    let batch = cli_json(
        &ws,
        &["query", "symbols-batch", "--files", "schema.graphql"],
    );
    assert_eq!(by_name_kind(&batch), expected, "{batch}");
    let result = Mcp::start(&ws)
        .call("symbols_batch", json!({ "files": ["schema.graphql"] }))
        .unwrap();
    assert_eq!(by_name_kind(&mcp_payload(&result)), expected, "{result}");
}

#[test]
fn query_symbol_returns_the_name_range() {
    let symbol = cli_json(
        &fixture("graphql"),
        &["query", "symbol", "User", "--file", "schema.graphql"],
    );
    assert_eq!(span(&symbol["symbol"]["range"]), "1:6-1:10", "{symbol}");
}

#[test]
fn mcp_skeleton_and_symbols_give_the_table_names_and_kinds() {
    let mut mcp = Mcp::start(&fixture("graphql"));
    let skeleton = mcp_payload(
        &mcp.call("skeleton", json!({ "file": "schema.graphql" }))
            .unwrap(),
    );
    assert_eq!(kind_names(&skeleton, "declarations"), skeleton_names());

    let symbols = mcp_payload(
        &mcp.call("symbols", json!({ "file": "schema.graphql" }))
            .unwrap(),
    );
    let expected = pairs(&[
        ("Class", "User"),
        ("Field", "id"),
        ("Field", "name"),
        ("Enum", "Role"),
        ("EnumMember", "ADMIN"),
        ("EnumMember", "USER"),
        ("Struct", "NewUser"),
        ("Interface", "Node"),
        ("Enum", "Item"),
        ("TypeAlias", "Date"),
        ("Attribute", "auth"),
        ("Field", "age"),
        ("Function", "GetUser"),
        ("Constant", "UserParts"),
    ]);
    assert_eq!(kind_names(&symbols, "symbols"), expected);
}

fn symbols_of(source: &str) -> Value {
    let ws = workspace(&[("a.graphql", source)]);
    cli_json(ws.path(), &["query", "symbols", "a.graphql"])
}

#[test]
fn mutation_and_subscription_are_functions() {
    let symbols = symbols_of("mutation M { x }\nsubscription S { y }\nmutation { z }\n");
    assert_eq!(
        kind_names(&symbols, "symbols"),
        pairs(&[("function", "M"), ("function", "S")])
    );
    assert!(symbols.get("parse_error").is_none(), "{symbols}");
}

#[test]
fn a_federation_header_without_a_body_loses_the_next_definition() {
    let symbols =
        symbols_of("extend schema @link(url: \"x\")\ntype A { a: Int }\ntype B { b: Int }\n");
    assert_eq!(
        kind_names(&symbols, "symbols"),
        pairs(&[("class", "B"), ("field", "b")])
    );
    assert_eq!(symbols["parse_error"], true, "{symbols}");
}

#[test]
fn a_schema_extension_with_a_body_parses_cleanly() {
    let symbols = symbols_of(
        "extend schema @link(url: \"x\") { query: Q }\ntype A { a: Int }\ntype B { b: Int }\n",
    );
    assert_eq!(
        kind_names(&symbols, "symbols"),
        pairs(&[
            ("class", "A"),
            ("field", "a"),
            ("class", "B"),
            ("field", "b")
        ])
    );
    assert!(symbols.get("parse_error").is_none(), "{symbols}");
}

#[test]
fn same_name_fields_are_a_name_match_on_cli() {
    let ws = workspace(&[("a.graphql", SAME_NAME)]);
    let answer = cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "id",
            "--file",
            "a.graphql",
            "--line",
            "1",
            "--column",
            "10",
        ],
    );
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    let sites: BTreeSet<(u64, u64)> = items(&answer, "references")
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect();
    assert_eq!(sites, [(1, 10), (2, 10)].into_iter().collect());
}

fn assert_mcp_name_match(mcp: &mut Mcp, reason: &str) {
    let result = mcp
        .call("references", json!({ "symbol": "id", "file": "a.graphql" }))
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
}

#[test]
fn same_name_fields_are_a_name_match_on_mcp() {
    let ws = workspace(&[("a.graphql", SAME_NAME)]);
    assert_mcp_name_match(&mut Mcp::start(ws.path()), "no language server");
}

#[test]
fn mcp_reason_reads_disabled_when_language_servers_are_disabled() {
    let ws = workspace(&[("a.graphql", SAME_NAME)]);
    let mut mcp = Mcp::start_with_env(ws.path(), &[("ACT_LITE_DISABLE_LSP", "1")]);
    assert_mcp_name_match(&mut mcp, "language servers are disabled");
}

#[test]
fn callers_needs_a_language_server_on_cli_and_mcp() {
    let ws = fixture("graphql");
    let output = cli(
        &ws,
        &["query", "callers", "User", "--file", "schema.graphql"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains(
            "needs a language server, and act-lite has none configured for GraphQL files"
        ),
        "{stderr}"
    );
    assert!(!stderr.contains("hint:"), "{stderr}");
    let message = Mcp::start(&ws)
        .call(
            "callers",
            json!({ "symbol": "User", "file": "schema.graphql" }),
        )
        .unwrap_err();
    assert!(
        message.contains(
            "query type 'callers' needs a language server, and act-lite has none configured for GraphQL files"
        ),
        "{message}"
    );
}
