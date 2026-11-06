//! Zig: skeleton, symbols, name-level references and the language-server
//! limitation, through the CLI and MCP.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const POINT_ZIG: &str = "const std = @import(\"std\");
pub const Point = struct {
    x: i32,
    pub fn len(self: Point) i32 {
        return self.x;
    }
};
const Color = enum { red, green };
var counter: u32 = 0;
pub fn main() void {
    if (counter > 0) {} else {}
    while (counter < 3) : (counter += 1) {}
    for ([_]u8{1}) |b| { _ = b; }
    switch (counter) { 0 => {}, else => {} }
}
test \"point len\" {
    try std.testing.expect(true);
}
";

const NAME_LEVEL_HINT: &str =
    "Name-level baseline — a same-name symbol in an unrelated scope can appear.";

const SHADOWING: &str = "const P = struct { x: i32 };
fn f() void { const x = 1; _ = x; }
";

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
    let ws = workspace("t.zig", source);
    let (set, _) = symbol_triples(ws.path(), "t.zig");
    set.into_iter().map(|(k, n, _)| (k, n)).collect()
}

/// `(kind, name)` of every declaration that `query skeleton` reports for `source`.
fn skeleton_names(source: &str) -> Vec<String> {
    let ws = workspace("t.zig", source);
    skeleton_declarations(ws.path(), "t.zig")
        .iter()
        .map(|d| d["name"].as_str().unwrap().to_string())
        .collect()
}

fn skeleton_pairs(source: &str) -> BTreeSet<(String, String)> {
    let ws = workspace("t.zig", source);
    pairs(&Value::Array(skeleton_declarations(ws.path(), "t.zig")))
}

#[test]
fn fixture_holds_the_design_bytes() {
    assert_eq!(
        std::fs::read_to_string(fixture("zig/point.zig")).unwrap(),
        POINT_ZIG
    );
}

#[test]
fn skeleton_is_the_table_set() {
    let list = skeleton_declarations(&fixture("zig"), "point.zig");
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
        ("import", "std", "1:1-1:28", "1:7-1:10"),
        ("struct", "Point", "2:1-7:3", "2:11-2:16"),
        ("method", "len", "4:5-6:6", "4:12-4:15"),
        ("enum", "Color", "8:1-8:35", "8:7-8:12"),
        ("variable", "counter", "9:1-9:22", "9:5-9:12"),
        ("function", "main", "10:1-15:2", "10:8-10:12"),
        ("function", "point len", "16:1-18:2", "16:7-16:16"),
    ]);
    assert_eq!(actual, expected);
}

#[test]
fn a_method_lies_inside_its_struct() {
    let list = skeleton_declarations(&fixture("zig"), "point.zig");
    let range = |name: &str| -> ((u64, u64), (u64, u64)) {
        let r = &list.iter().find(|d| d["name"] == name).unwrap()["range"];
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
    let (outer, inner) = (range("Point"), range("len"));
    assert!(
        outer.0 <= inner.0 && inner.1 <= outer.1,
        "{outer:?} {inner:?}"
    );
}

#[test]
fn mcp_skeleton_gives_the_table_names_and_kinds_without_import_entries() {
    let skeleton = mcp_payload(
        &Mcp::start(&fixture("zig"))
            .call("skeleton", json!({ "file": "point.zig" }))
            .unwrap(),
    );
    assert_eq!(
        pairs(&skeleton["declarations"]),
        pair_set(&[
            ("struct", "Point"),
            ("method", "len"),
            ("enum", "Color"),
            ("variable", "counter"),
            ("function", "main"),
            ("function", "point len"),
        ])
    );
}

#[test]
fn symbols_are_the_table_set() {
    let (actual, symbols) = symbol_triples(&fixture("zig"), "point.zig");
    let expected = triples(&[
        ("struct", "Point", "2:11-2:16"),
        ("field", "x", "3:5-3:6"),
        ("method", "len", "4:12-4:15"),
        ("enum", "Color", "8:7-8:12"),
        ("enum_member", "red", "8:22-8:25"),
        ("enum_member", "green", "8:27-8:32"),
        ("variable", "counter", "9:5-9:12"),
        ("function", "main", "10:8-10:12"),
        ("function", "point len", "16:7-16:16"),
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
        ("std", "import"),
        ("Point", "struct"),
        ("len", "method"),
        ("Color", "enum"),
        ("counter", "variable"),
        ("main", "function"),
        ("point len", "function"),
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
    let ws = fixture("zig");
    let batch = cli_json(&ws, &["query", "symbols-batch", "--files", "point.zig"]);
    assert_eq!(name_kind(&batch), expected, "{batch}");
    let result = Mcp::start(&ws)
        .call("symbols_batch", json!({ "files": ["point.zig"] }))
        .unwrap();
    assert_eq!(name_kind(&mcp_payload(&result)), expected, "{result}");
}

#[test]
fn mcp_symbols_give_the_table_names_with_mcp_kinds() {
    let symbols = mcp_payload(
        &Mcp::start(&fixture("zig"))
            .call("symbols", json!({ "file": "point.zig" }))
            .unwrap(),
    );
    assert_eq!(
        pairs(&symbols["symbols"]),
        pair_set(&[
            ("Struct", "Point"),
            ("Field", "x"),
            ("Method", "len"),
            ("Enum", "Color"),
            ("EnumMember", "red"),
            ("EnumMember", "green"),
            ("Variable", "counter"),
            ("Function", "main"),
            ("Function", "point len"),
        ])
    );
}

#[test]
fn query_symbol_returns_the_name_range() {
    let symbol = cli_json(
        &fixture("zig"),
        &["query", "symbol", "main", "--file", "point.zig"],
    );
    assert_eq!(span(&symbol["symbol"]["range"]), "10:8-10:12", "{symbol}");
}

#[test]
fn an_empty_struct_parses_with_an_error_and_reports_no_empty_field() {
    let ws = workspace("t.zig", "const S = struct {};\n");
    let (symbols, answer) = symbol_triples(ws.path(), "t.zig");
    assert_eq!(answer["parse_error"], true, "{answer}");
    assert_eq!(symbols, triples(&[("struct", "S", "1:7-1:8")]));
    assert_eq!(
        skeleton_pairs("const S = struct {};\n"),
        pair_set(&[("struct", "S")])
    );
    for empty in [
        "const E = enum {};\n",
        "const U = union(enum) {};\n",
        "const O = opaque {};\n",
    ] {
        let ws = workspace("t.zig", empty);
        let (symbols, answer) = symbol_triples(ws.path(), "t.zig");
        assert_eq!(answer["parse_error"], true, "{empty}: {answer}");
        assert_eq!(symbols.len(), 1, "{empty}: {symbols:?}");
    }
}

#[test]
fn a_call_that_builds_a_type_is_a_constant() {
    let source = "const T = List(u8);\n";
    assert_eq!(symbol_pairs(source), pair_set(&[("constant", "T")]));
    assert_eq!(skeleton_pairs(source), pair_set(&[("constant", "T")]));
}

#[test]
fn a_member_access_on_an_import_is_a_constant_and_an_exact_import_is_not() {
    let source = "const d = @import(\"std\").debug;\nconst s = @import(\"std\");\n";
    assert_eq!(
        skeleton_pairs(source),
        pair_set(&[("constant", "d"), ("import", "s")])
    );
    assert_eq!(symbol_pairs(source), pair_set(&[("constant", "d")]));
}

#[test]
fn a_typed_or_commented_exact_import_is_an_import_and_not_a_symbol() {
    let source = "const a: type = @import(\"std\");\nconst b = @import(// c\n\"std\");\nconst c = @import(\"std\");\nconst d = @import(\"std\",);\nconst e = @import // c\n(\"std\");\nconst m = @import(\"a\\\"b.zig\");\nconst n = @import(\\\\x\n);\nconst t = @import(\"std\") // c\n;\nconst S: type = struct { x: i32 };\nconst u: type = @import(\"std\").debug;\nextern var errno: c_int;\npub extern const limit: u8;\nconst value = struct { y: u8 }{ .y = 1 };\nconst In = struct { extern var g: u8; const lit = struct { z: u8 }{ .z = 2 }; };\nconst w: u8 = // c\n 1;\nextern var buffer: u8 align(16);\nconst T: type = type;\nvar typed: struct { q: u8 } = undefined;\nconst C = struct { extern var b2: u8 align(4); var t2: struct { r: u8 } = undefined; };\n";
    assert_eq!(
        skeleton_pairs(source),
        pair_set(&[
            ("import", "a"),
            ("import", "b"),
            ("import", "c"),
            ("import", "d"),
            ("import", "e"),
            ("import", "m"),
            ("constant", "n"),
            ("import", "t"),
            ("struct", "S"),
            ("constant", "u"),
            ("variable", "errno"),
            ("constant", "limit"),
            ("constant", "value"),
            ("struct", "In"),
            ("variable", "g"),
            ("constant", "lit"),
            ("constant", "w"),
            ("variable", "buffer"),
            ("constant", "T"),
            ("variable", "typed"),
            ("struct", "C"),
            ("variable", "b2"),
            ("variable", "t2")
        ])
    );
    assert_eq!(
        symbol_pairs(source),
        pair_set(&[
            ("constant", "n"),
            ("struct", "S"),
            ("field", "x"),
            ("constant", "u"),
            ("variable", "errno"),
            ("constant", "limit"),
            ("constant", "value"),
            ("field", "y"),
            ("struct", "In"),
            ("variable", "g"),
            ("constant", "lit"),
            ("field", "z"),
            ("constant", "w"),
            ("variable", "buffer"),
            ("constant", "T"),
            ("variable", "typed"),
            ("field", "q"),
            ("struct", "C"),
            ("variable", "b2"),
            ("variable", "t2"),
            ("field", "r")
        ])
    );
    let once = skeleton_names(source);
    for name in ["typed", "t2", "buffer", "T"] {
        assert_eq!(once.iter().filter(|n| *n == name).count(), 1, "{name} once");
    }
}

#[test]
fn nameless_tests_and_usingnamespace_are_not_reported() {
    let source = "test { }\nusingnamespace @import(\"x.zig\");\n";
    assert!(skeleton_pairs(source).is_empty());
    assert!(symbol_pairs(source).is_empty());
}

#[test]
fn a_named_test_is_a_function_and_a_doctest_is_a_use() {
    let source = "const name = 1;\ntest name { }\ntest \"other\" { }\n";
    let expected = pair_set(&[("constant", "name"), ("function", "other")]);
    assert_eq!(skeleton_pairs(source), expected);
    assert_eq!(symbol_pairs(source), expected);
    let ws = workspace("t.zig", source);
    let answer = cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "name",
            "--file",
            "t.zig",
            "--line",
            "1",
            "--column",
            "7",
        ],
    );
    assert_eq!(sites(&answer), [(1, 7), (2, 6)].into_iter().collect());
}

#[test]
fn error_set_members_are_enum_members() {
    let source = "const E = error { A, B };\n";
    assert_eq!(skeleton_pairs(source), pair_set(&[("enum", "E")]));
    assert_eq!(
        symbol_pairs(source),
        pair_set(&[("enum", "E"), ("enum_member", "A"), ("enum_member", "B")])
    );
}

#[test]
fn a_union_is_a_union_in_the_skeleton_and_an_enum_in_symbols() {
    let source = "const U = union(enum) { a: i32, b: f32 };\n";
    assert_eq!(skeleton_pairs(source), pair_set(&[("union", "U")]));
    assert_eq!(
        symbol_pairs(source),
        pair_set(&[("enum", "U"), ("field", "a"), ("field", "b")])
    );
}

#[test]
fn functions_in_any_container_are_methods_and_container_constants_are_reported() {
    let source = "const E = enum { p, fn h() void {} };
const U = union { a: i32, fn u() void {} };
const O = opaque { fn g() void {} var v: u8 = 0; const k = 1; };
fn top() void { const local = 1; _ = local; }
";
    let expected = pair_set(&[
        ("method", "h"),
        ("method", "u"),
        ("method", "g"),
        ("function", "top"),
        ("variable", "v"),
        ("constant", "k"),
        ("constant", "O"),
    ]);
    let kept = |set: BTreeSet<(String, String)>| -> BTreeSet<(String, String)> {
        set.into_iter()
            .filter(|(k, _)| matches!(k.as_str(), "method" | "function" | "variable" | "constant"))
            .collect()
    };
    assert_eq!(kept(skeleton_pairs(source)), expected);
    assert_eq!(kept(symbol_pairs(source)), expected);
}

fn reference_sites(ws: &std::path::Path, name: &str, line: &str, column: &str) -> Value {
    cli_json(
        ws,
        &[
            "query",
            "references",
            name,
            "--file",
            "t.zig",
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
fn same_name_field_and_locals_are_a_name_match_on_cli() {
    let ws = workspace("t.zig", SHADOWING);
    let answer = reference_sites(ws.path(), "x", "1", "20");
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    assert_eq!(
        sites(&answer),
        [(1, 20), (2, 21), (2, 32)].into_iter().collect()
    );
}

fn assert_mcp_name_match(mcp: &mut Mcp, reason: &str) {
    let result = mcp
        .call("references", json!({ "symbol": "x", "file": "t.zig" }))
        .unwrap();
    let answer = mcp_payload(&result);
    assert_eq!(
        answer["resolved_by"],
        format!("tree-sitter workspace baseline (name-level; {reason})"),
        "{answer}"
    );
    assert_eq!(
        answer["references"].as_array().map(Vec::len),
        Some(3),
        "{answer}"
    );
    let text = mcp_text(&result);
    assert!(text.contains(NAME_LEVEL_HINT), "{text}");
    assert!(!text.contains("Install"), "{text}");
}

#[test]
fn same_name_field_and_locals_are_a_name_match_on_mcp() {
    let ws = workspace("t.zig", SHADOWING);
    assert_mcp_name_match(&mut Mcp::start(ws.path()), "no language server");
}

#[test]
fn mcp_reason_reads_disabled_when_language_servers_are_disabled() {
    let ws = workspace("t.zig", SHADOWING);
    let mut mcp = Mcp::start_with_env(ws.path(), &[("ACT_LITE_DISABLE_LSP", "1")]);
    assert_mcp_name_match(&mut mcp, "language servers are disabled");
}

#[test]
fn callers_needs_a_language_server_on_cli_and_mcp() {
    let ws = fixture("zig");
    let output = cli(&ws, &["query", "callers", "main", "--file", "point.zig"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("needs a language server, and act-lite has none configured for Zig files"),
        "{stderr}"
    );
    assert!(!stderr.contains("hint:"), "{stderr}");
    let message = Mcp::start(&ws)
        .call("callers", json!({ "symbol": "main", "file": "point.zig" }))
        .unwrap_err();
    assert!(
        message.contains(
            "query type 'callers' needs a language server, and act-lite has none configured for Zig files"
        ),
        "{message}"
    );
}

#[test]
fn modified_containers_are_types_and_error_values_are_constants() {
    let source = "const A = packed struct { a: u8 };\nconst e = error.Foo;\n";
    assert_eq!(
        skeleton_pairs(source),
        pair_set(&[("struct", "A"), ("constant", "e")])
    );
}
