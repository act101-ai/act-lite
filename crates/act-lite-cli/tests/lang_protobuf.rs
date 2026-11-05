//! Protocol Buffers: skeleton, symbols, name-level references and the
//! language-server limitation, through the CLI and MCP.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const SHOP: &str = "syntax = \"proto3\";\npackage shop.v1;\nimport \"google/protobuf/empty.proto\";\noption go_package = \"x\";\nmessage Order {\n  string id = 1;\n  message Line { int32 qty = 1; }\n  repeated Line lines = 2;\n  oneof pay { string card = 3; }\n  map<string, int32> tags = 4;\n  enum State { NEW = 0; DONE = 1; }\n}\nenum Kind { KIND_UNSPECIFIED = 0; }\nservice Shop {\n  rpc Get(Order) returns (Order);\n  rpc Watch(stream Order) returns (stream Order) {}\n}\n";

const NAME_LEVEL_HINT: &str =
    "Name-level baseline — a same-name symbol in an unrelated scope can appear.";

const TWO_MESSAGES: &str =
    "message A { message Line {} Line l = 1; }\nmessage B { message Line {} Line m = 1; }\n";

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

#[test]
fn fixture_holds_the_design_bytes() {
    assert_eq!(
        std::fs::read_to_string(fixture("protobuf/shop.proto")).unwrap(),
        SHOP
    );
}

#[test]
fn skeleton_is_the_table_set() {
    let skeleton = cli_json(&fixture("protobuf"), &["query", "skeleton", "shop.proto"]);
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
        ("package", "shop.v1", "2:1-2:17", "2:9-2:16"),
        (
            "import",
            "\"google/protobuf/empty.proto\"",
            "3:1-3:38",
            "3:8-3:37",
        ),
        ("struct", "Order", "5:1-12:2", "5:9-5:14"),
        ("struct", "Line", "7:3-7:34", "7:11-7:15"),
        ("enum", "State", "11:3-11:36", "11:8-11:13"),
        ("enum", "Kind", "13:1-13:36", "13:6-13:10"),
        ("interface", "Shop", "14:1-17:2", "14:9-14:13"),
        ("method", "Get", "15:3-15:34", "15:7-15:10"),
        ("method", "Watch", "16:3-16:52", "16:7-16:12"),
    ]
    .into_iter()
    .map(|(k, n, r, nr)| (k.into(), n.into(), r.into(), nr.into()))
    .collect();
    assert_eq!(actual, expected);
}

#[test]
fn nested_message_lies_inside_its_container() {
    let skeleton = cli_json(&fixture("protobuf"), &["query", "skeleton", "shop.proto"]);
    let range = |name: &str| -> ((u64, u64), (u64, u64)) {
        let d = skeleton["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["name"] == name)
            .unwrap_or_else(|| panic!("no {name} in {skeleton}"));
        let r = &d["range"];
        (
            (
                r["start"]["line"].as_u64().unwrap(),
                r["start"]["column"].as_u64().unwrap(),
            ),
            (
                r["end"]["line"].as_u64().unwrap(),
                r["end"]["column"].as_u64().unwrap(),
            ),
        )
    };
    let (order, line) = (range("Order"), range("Line"));
    assert!(order.0 <= line.0 && line.1 <= order.1, "{order:?} {line:?}");
}

#[test]
fn mcp_skeleton_gives_the_table_names_and_kinds_without_imports() {
    let skeleton = mcp_payload(
        &Mcp::start(&fixture("protobuf"))
            .call("skeleton", json!({ "file": "shop.proto" }))
            .unwrap(),
    );
    assert_eq!(
        pairs(&skeleton["declarations"]),
        pair_set(&[
            ("package", "shop.v1"),
            ("struct", "Order"),
            ("struct", "Line"),
            ("enum", "State"),
            ("enum", "Kind"),
            ("interface", "Shop"),
            ("method", "Get"),
            ("method", "Watch"),
        ])
    );
}

#[test]
fn symbols_are_the_table_set() {
    let (actual, symbols) = symbol_triples(&fixture("protobuf"), "shop.proto");
    let expected: BTreeSet<Triple> = [
        ("module", "shop.v1", "2:9-2:16"),
        ("struct", "Order", "5:9-5:14"),
        ("field", "id", "6:10-6:12"),
        ("struct", "Line", "7:11-7:15"),
        ("field", "qty", "7:24-7:27"),
        ("field", "lines", "8:17-8:22"),
        ("field", "pay", "9:9-9:12"),
        ("field", "card", "9:22-9:26"),
        ("field", "tags", "10:22-10:26"),
        ("enum", "State", "11:8-11:13"),
        ("enum_member", "NEW", "11:16-11:19"),
        ("enum_member", "DONE", "11:25-11:29"),
        ("enum", "Kind", "13:6-13:10"),
        ("enum_member", "KIND_UNSPECIFIED", "13:13-13:29"),
        ("interface", "Shop", "14:9-14:13"),
        ("method", "Get", "15:7-15:10"),
        ("method", "Watch", "16:7-16:12"),
    ]
    .into_iter()
    .map(|(k, n, r)| (k.into(), n.into(), r.into()))
    .collect();
    assert_eq!(actual, expected);
    assert!(symbols.get("parse_error").is_none(), "{symbols}");
    for s in symbols["symbols"].as_array().unwrap() {
        let name = s["name"].as_str().unwrap();
        assert!(!name.is_empty(), "{s}");
        assert_ne!(
            (s["range"]["start"].clone(), s["range"]["end"].clone()),
            (s["range"]["end"].clone(), s["range"]["end"].clone()),
            "zero-width {s}"
        );
    }
}

#[test]
fn symbols_batch_lists_the_skeleton_set_on_cli_and_mcp() {
    let expected = pair_set(&[
        ("shop.v1", "package"),
        ("\"google/protobuf/empty.proto\"", "import"),
        ("Order", "struct"),
        ("Line", "struct"),
        ("State", "enum"),
        ("Kind", "enum"),
        ("Shop", "interface"),
        ("Get", "method"),
        ("Watch", "method"),
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
    let ws = fixture("protobuf");
    let batch = cli_json(&ws, &["query", "symbols-batch", "--files", "shop.proto"]);
    assert_eq!(name_kind(&batch), expected, "{batch}");
    let result = Mcp::start(&ws)
        .call("symbols_batch", json!({ "files": ["shop.proto"] }))
        .unwrap();
    assert_eq!(name_kind(&mcp_payload(&result)), expected, "{result}");
}

#[test]
fn mcp_symbols_give_the_table_names_with_mcp_kinds() {
    let symbols = mcp_payload(
        &Mcp::start(&fixture("protobuf"))
            .call("symbols", json!({ "file": "shop.proto" }))
            .unwrap(),
    );
    assert_eq!(
        pairs(&symbols["symbols"]),
        pair_set(&[
            ("Module", "shop.v1"),
            ("Struct", "Order"),
            ("Field", "id"),
            ("Struct", "Line"),
            ("Field", "qty"),
            ("Field", "lines"),
            ("Field", "pay"),
            ("Field", "card"),
            ("Field", "tags"),
            ("Enum", "State"),
            ("EnumMember", "NEW"),
            ("EnumMember", "DONE"),
            ("Enum", "Kind"),
            ("EnumMember", "KIND_UNSPECIFIED"),
            ("Interface", "Shop"),
            ("Method", "Get"),
            ("Method", "Watch"),
        ])
    );
}

#[test]
fn query_symbol_returns_the_name_range() {
    let symbol = cli_json(
        &fixture("protobuf"),
        &["query", "symbol", "Order", "--file", "shop.proto"],
    );
    assert_eq!(span(&symbol["symbol"]["range"]), "5:9-5:14", "{symbol}");
}

#[test]
fn proto2_group_is_a_struct_and_a_field() {
    let ws = workspace(
        "g.proto",
        "message G {\n  optional group Result = 1 {}\n}\n",
    );
    let (actual, _) = symbol_triples(ws.path(), "g.proto");
    let kinds_of = |name: &str| -> Vec<String> {
        actual
            .iter()
            .filter(|(_, n, _)| n == name)
            .map(|(k, _, _)| k.clone())
            .collect()
    };
    let mut result = kinds_of("Result");
    result.sort();
    assert_eq!(result, vec!["field".to_string(), "struct".to_string()]);
}

#[test]
fn extend_syntax_option_and_reserved_are_not_reported() {
    let ws = workspace(
        "e.proto",
        "syntax = \"proto2\";\noption java_package = \"p\";\nmessage M {\n  extensions 100 to 199;\n  reserved 2, 15;\n  reserved \"old\";\n  option deprecated = true;\n}\nextend M {\n  optional int32 ext = 100;\n}\n",
    );
    let (actual, _) = symbol_triples(ws.path(), "e.proto");
    let names: BTreeSet<&str> = actual.iter().map(|(_, n, _)| n.as_str()).collect();
    assert_eq!(names, ["M"].into_iter().collect());
    let skeleton = cli_json(ws.path(), &["query", "skeleton", "e.proto"]);
    assert_eq!(
        pairs(&skeleton["declarations"]),
        pair_set(&[("struct", "M")])
    );
}

#[test]
fn a_qualified_type_is_a_use_of_its_last_name() {
    let ws = workspace(
        "shop.proto",
        &format!("{SHOP}message X {{ shop.v1.Order o = 1; }}\n"),
    );
    let answer = cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "Order",
            "--file",
            "shop.proto",
            "--line",
            "5",
            "--column",
            "9",
        ],
    );
    let sites: BTreeSet<(u64, u64)> = answer["references"]
        .as_array()
        .unwrap_or_else(|| panic!("no references in {answer}"))
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect();
    assert!(sites.contains(&(18, 21)), "{answer}");
}

#[test]
fn same_name_declarations_are_a_name_match_on_cli() {
    let ws = workspace("a.proto", TWO_MESSAGES);
    let answer = cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "Line",
            "--file",
            "a.proto",
            "--line",
            "1",
            "--column",
            "21",
        ],
    );
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    let sites: BTreeSet<(u64, u64)> = answer["references"]
        .as_array()
        .unwrap_or_else(|| panic!("no references in {answer}"))
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect();
    assert_eq!(
        sites,
        [(1, 21), (1, 29), (2, 21), (2, 29)].into_iter().collect()
    );
}

fn assert_mcp_name_match(mcp: &mut Mcp, reason: &str) {
    let result = mcp
        .call("references", json!({ "symbol": "Line", "file": "a.proto" }))
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
fn same_name_declarations_are_a_name_match_on_mcp() {
    let ws = workspace("a.proto", TWO_MESSAGES);
    assert_mcp_name_match(&mut Mcp::start(ws.path()), "no language server");
}

#[test]
fn mcp_reason_reads_disabled_when_language_servers_are_disabled() {
    let ws = workspace("a.proto", TWO_MESSAGES);
    let mut mcp = Mcp::start_with_env(ws.path(), &[("ACT_LITE_DISABLE_LSP", "1")]);
    assert_mcp_name_match(&mut mcp, "language servers are disabled");
}

#[test]
fn callers_needs_a_language_server_on_cli_and_mcp() {
    let ws = fixture("protobuf");
    let output = cli(&ws, &["query", "callers", "Order", "--file", "shop.proto"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains(
            "needs a language server, and act-lite has none configured for Protocol Buffers files"
        ),
        "{stderr}"
    );
    assert!(!stderr.contains("hint:"), "{stderr}");
    let message = Mcp::start(&ws)
        .call(
            "callers",
            json!({ "symbol": "Order", "file": "shop.proto" }),
        )
        .unwrap_err();
    assert!(
        message.contains(
            "query type 'callers' needs a language server, and act-lite has none configured for Protocol Buffers files"
        ),
        "{message}"
    );
}
