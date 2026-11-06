//! Julia: skeleton, symbols, name-level references and the language-server
//! limitation, through the CLI and MCP.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const GEOMETRY_JL: &str = "module Geometry
using LinearAlgebra
import Base: show
export area
abstract type Shape end
struct Circle <: Shape
    r::Float64
end
mutable struct Box
    w
end
const UNIT = 1.0
function area(c::Circle)
    if c.r > 0
        return pi * c.r^2
    elseif c.r == 0
        return 0.0
    end
    for i in 1:2; end
    while false; end
end
area(b::Box) = b.w^2
macro twice(ex) end
end
";

const NAME_LEVEL_HINT: &str =
    "Name-level baseline — a same-name symbol in an unrelated scope can appear.";

const SHADOWING: &str = "f(x) = x\ng(x) = x\n";

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
    let ws = workspace("t.jl", source);
    let (set, _) = symbol_triples(ws.path(), "t.jl");
    set.into_iter().map(|(k, n, _)| (k, n)).collect()
}

/// `(kind, name)` of every declaration that `query skeleton` reports for `source`.
fn skeleton_pairs(source: &str) -> BTreeSet<(String, String)> {
    let ws = workspace("t.jl", source);
    pairs(&Value::Array(skeleton_declarations(ws.path(), "t.jl")))
}

#[test]
fn fixture_holds_the_design_bytes() {
    assert_eq!(
        std::fs::read_to_string(fixture("julia/geometry.jl")).unwrap(),
        GEOMETRY_JL
    );
}

fn range_of(d: &Value, key: &str) -> String {
    span(&d[key])
}

#[test]
fn skeleton_is_the_table_set() {
    let list = skeleton_declarations(&fixture("julia"), "geometry.jl");
    let actual: BTreeSet<Quad> = list
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
                range_of(d, "range"),
                range_of(d, "name_range"),
            )
        })
        .collect();
    let expected = quads(&[
        ("module", "Geometry", "1:1-24:4", "1:8-1:16"),
        ("import", "LinearAlgebra", "2:1-2:20", "2:7-2:20"),
        ("import", "Base", "3:1-3:18", "3:8-3:12"),
        ("interface", "Shape", "5:1-5:24", "5:15-5:20"),
        ("struct", "Circle", "6:1-8:4", "6:8-6:14"),
        ("struct", "Box", "9:1-11:4", "9:16-9:19"),
        ("constant", "UNIT", "12:1-12:17", "12:7-12:11"),
        ("function", "area", "13:1-21:4", "13:10-13:14"),
        ("function", "area", "22:1-22:21", "22:1-22:5"),
        ("macro", "twice", "23:1-23:20", "23:7-23:12"),
    ]);
    assert_eq!(actual, expected);
    assert!(list.iter().all(|d| d["kind"] != "export"));
}

#[test]
fn every_declaration_lies_inside_the_module() {
    let list = skeleton_declarations(&fixture("julia"), "geometry.jl");
    let lines = |d: &Value| -> (u64, u64) {
        (
            d["range"]["start"]["line"].as_u64().unwrap(),
            d["range"]["end"]["line"].as_u64().unwrap(),
        )
    };
    let module = list.iter().find(|d| d["name"] == "Geometry").unwrap();
    let (first, last) = lines(module);
    for d in list.iter().filter(|d| d["name"] != "Geometry") {
        let (start, end) = lines(d);
        assert!(first <= start && end <= last, "{d}");
    }
}

#[test]
fn mcp_skeleton_gives_the_table_names_and_kinds_without_import_entries() {
    let skeleton = mcp_payload(
        &Mcp::start(&fixture("julia"))
            .call("skeleton", json!({ "file": "geometry.jl" }))
            .unwrap(),
    );
    assert_eq!(
        pairs(&skeleton["declarations"]),
        pair_set(&[
            ("module", "Geometry"),
            ("interface", "Shape"),
            ("struct", "Circle"),
            ("struct", "Box"),
            ("constant", "UNIT"),
            ("function", "area"),
            ("macro", "twice"),
        ])
    );
}

#[test]
fn symbols_are_the_table_set() {
    let (actual, symbols) = symbol_triples(&fixture("julia"), "geometry.jl");
    let expected = triples(&[
        ("module", "Geometry", "1:8-1:16"),
        ("interface", "Shape", "5:15-5:20"),
        ("struct", "Circle", "6:8-6:14"),
        ("field", "r", "7:5-7:6"),
        ("struct", "Box", "9:16-9:19"),
        ("field", "w", "10:5-10:6"),
        ("constant", "UNIT", "12:7-12:11"),
        ("function", "area", "13:10-13:14"),
        ("function", "area", "22:1-22:5"),
        ("function", "twice", "23:7-23:12"),
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
        ("Geometry", "module"),
        ("LinearAlgebra", "import"),
        ("Base", "import"),
        ("Shape", "interface"),
        ("Circle", "struct"),
        ("Box", "struct"),
        ("UNIT", "constant"),
        ("area", "function"),
        ("twice", "macro"),
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
    let ws = fixture("julia");
    let batch = cli_json(&ws, &["query", "symbols-batch", "--files", "geometry.jl"]);
    assert_eq!(name_kind(&batch), expected, "{batch}");
    let result = Mcp::start(&ws)
        .call("symbols_batch", json!({ "files": ["geometry.jl"] }))
        .unwrap();
    assert_eq!(name_kind(&mcp_payload(&result)), expected, "{result}");
}

#[test]
fn mcp_symbols_give_the_table_names_with_mcp_kinds() {
    let symbols = mcp_payload(
        &Mcp::start(&fixture("julia"))
            .call("symbols", json!({ "file": "geometry.jl" }))
            .unwrap(),
    );
    assert_eq!(
        pairs(&symbols["symbols"]),
        pair_set(&[
            ("Module", "Geometry"),
            ("Interface", "Shape"),
            ("Struct", "Circle"),
            ("Field", "r"),
            ("Struct", "Box"),
            ("Field", "w"),
            ("Constant", "UNIT"),
            ("Function", "area"),
            ("Function", "twice"),
        ])
    );
}

#[test]
fn query_symbol_returns_the_name_range() {
    let symbol = cli_json(
        &fixture("julia"),
        &["query", "symbol", "area", "--file", "geometry.jl"],
    );
    assert_eq!(span(&symbol["symbol"]["range"]), "13:10-13:14", "{symbol}");
}

#[test]
fn callees_as_written_name_the_methods() {
    let source = "Base.show(io, x) = 1\n+(a, b) = 1\nfunction Base.show(io, x) end\n";
    let expected = pair_set(&[("function", "Base.show"), ("function", "+")]);
    assert_eq!(skeleton_pairs(source), expected);
    assert_eq!(symbol_pairs(source), expected);
}

#[test]
fn long_form_signatures_with_a_type_or_where_are_named_by_the_callee() {
    let source = "function f(x)::Int\n  1\nend\nfunction g(x) where T\n  1\nend\n";
    let expected = pair_set(&[("function", "f"), ("function", "g")]);
    assert_eq!(skeleton_pairs(source), expected);
    assert_eq!(symbol_pairs(source), expected);
}

#[test]
fn short_form_with_a_type_or_where_is_named_by_the_callee() {
    let source = "f(x)::Int = x\ng(x::T) where {T} = x\n";
    let expected = pair_set(&[("function", "f"), ("function", "g")]);
    assert_eq!(skeleton_pairs(source), expected);
    assert_eq!(symbol_pairs(source), expected);
}

#[test]
fn nested_signature_wrappers_are_named_by_the_callee() {
    let source = "f(x::T)::T where {T} = x\ng(x::T) where {T} where {S} = x\nfunction h(x::T)::T where {T}\n    x\nend\nfunction i(x::T) where {T} where {S}\n    x\nend\n";
    let expected = pair_set(&[
        ("function", "f"),
        ("function", "g"),
        ("function", "h"),
        ("function", "i"),
    ]);
    assert_eq!(skeleton_pairs(source), expected);
    assert_eq!(symbol_pairs(source), expected);
}

#[test]
fn callable_objects_and_parametric_constructors_are_named_by_the_callee() {
    let source = "function (f::Foo)(x)\n    x\nend\n(g::Bar)(x) = x\nBaz{T}(x::T) where {T} = x\nfunction Qux{T}(x::T) where {T}\n    x\nend\n";
    let expected = pair_set(&[
        ("function", "(f::Foo)"),
        ("function", "(g::Bar)"),
        ("function", "Baz{T}"),
        ("function", "Qux{T}"),
    ]);
    assert_eq!(skeleton_pairs(source), expected);
    assert_eq!(symbol_pairs(source), expected);
}

#[test]
fn a_function_without_methods_is_a_function() {
    let source = "function f end\n";
    assert_eq!(skeleton_pairs(source), pair_set(&[("function", "f")]));
    assert_eq!(symbol_pairs(source), pair_set(&[("function", "f")]));
}

#[test]
fn primitive_types_are_type_aliases_and_baremodules_are_modules() {
    let source = "primitive type Byte8 8 end\nbaremodule B\nend\n";
    let expected = pair_set(&[("type_alias", "Byte8"), ("module", "B")]);
    assert_eq!(skeleton_pairs(source), expected);
    assert_eq!(symbol_pairs(source), expected);
}

#[test]
fn a_parametric_struct_is_named_without_its_parameters() {
    let source = "struct P{T}\n  x::T\nend\nabstract type Q{T} <: P{T} end\n";
    assert_eq!(
        skeleton_pairs(source),
        pair_set(&[("struct", "P"), ("interface", "Q")])
    );
    assert_eq!(
        symbol_pairs(source),
        pair_set(&[("struct", "P"), ("field", "x"), ("interface", "Q")])
    );
}

#[test]
fn a_typed_const_is_a_constant() {
    let source = "const X::Int = 1\n";
    assert_eq!(skeleton_pairs(source), pair_set(&[("constant", "X")]));
    assert_eq!(symbol_pairs(source), pair_set(&[("constant", "X")]));
}

#[test]
fn using_and_import_give_one_entry_per_module() {
    let source = "using A, B\nimport M: x, y\nimport C\nusing D.E\n";
    assert_eq!(
        skeleton_pairs(source),
        pair_set(&[
            ("import", "A"),
            ("import", "B"),
            ("import", "M"),
            ("import", "C"),
            ("import", "D.E"),
        ])
    );
    assert!(symbol_pairs(source).is_empty());
}

#[test]
fn include_is_a_skeleton_entry_named_by_its_string() {
    let source = "include(\"f.jl\")\n";
    assert_eq!(skeleton_pairs(source), pair_set(&[("include", "\"f.jl\"")]));
    assert!(symbol_pairs(source).is_empty());
}

#[test]
fn anonymous_functions_do_blocks_and_exports_are_not_reported() {
    let source = "export a\ng = x -> x\nmap(1:2) do i\n  i\nend\nh = 1\n";
    assert!(skeleton_pairs(source).is_empty());
    assert!(symbol_pairs(source).is_empty());
}

fn reference_sites(ws: &std::path::Path, name: &str, line: &str, column: &str) -> Value {
    cli_json(
        ws,
        &[
            "query",
            "references",
            name,
            "--file",
            "t.jl",
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
fn same_name_parameters_are_a_name_match_on_cli() {
    let ws = workspace("t.jl", SHADOWING);
    let answer = reference_sites(ws.path(), "x", "1", "3");
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    assert_eq!(
        sites(&answer),
        [(1, 3), (1, 8), (2, 3), (2, 8)].into_iter().collect()
    );
}

fn assert_mcp_name_match(mcp: &mut Mcp, reason: &str) {
    let result = mcp
        .call("references", json!({ "symbol": "x", "file": "t.jl" }))
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
    let ws = workspace("t.jl", SHADOWING);
    assert_mcp_name_match(&mut Mcp::start(ws.path()), "no language server");
}

#[test]
fn mcp_reason_reads_disabled_when_language_servers_are_disabled() {
    let ws = workspace("t.jl", SHADOWING);
    let mut mcp = Mcp::start_with_env(ws.path(), &[("ACT_LITE_DISABLE_LSP", "1")]);
    assert_mcp_name_match(&mut mcp, "language servers are disabled");
}

#[test]
fn a_dotted_method_name_matches_the_whole_field_expression() {
    let ws = workspace("t.jl", "Base.show(io, x) = 1\nBase.show(stdout, 2)\n");
    let answer = reference_sites(ws.path(), "Base.show", "1", "1");
    assert_eq!(sites(&answer), [(1, 1), (2, 1)].into_iter().collect());
}

#[test]
fn callers_needs_a_language_server_on_cli_and_mcp() {
    let ws = fixture("julia");
    let output = cli(&ws, &["query", "callers", "area", "--file", "geometry.jl"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr
            .contains("needs a language server, and act-lite has none configured for Julia files"),
        "{stderr}"
    );
    assert!(!stderr.contains("hint:"), "{stderr}");
    let message = Mcp::start(&ws)
        .call(
            "callers",
            json!({ "symbol": "area", "file": "geometry.jl" }),
        )
        .unwrap_err();
    assert!(
        message.contains(
            "query type 'callers' needs a language server, and act-lite has none configured for Julia files"
        ),
        "{message}"
    );
}
