//! CMake: skeleton, symbols, name-level references and the language-server
//! limitation, through the CLI and MCP.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const CMAKE_LISTS: &str = "cmake_minimum_required(VERSION 3.20)
project(shop LANGUAGES C)
set(SRCS a.c b.c)
option(WITH_TESTS \"tests\" ON)
function(add_tool name)
  add_executable(${name} ${SRCS})
endfunction()
macro(log msg)
  message(STATUS ${msg})
endmacro()
add_library(core STATIC ${SRCS})
if(WITH_TESTS)
  add_subdirectory(tests)
endif()
include(cmake/Deps.cmake)
add_tool(cli)
";

const NAME_LEVEL_HINT: &str =
    "Name-level baseline — a same-name symbol in an unrelated scope can appear.";

const TWO_SETS: &str = "function(f) set(v 1) endfunction()
set(v 2)
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

#[test]
fn fixture_holds_the_design_bytes() {
    assert_eq!(
        std::fs::read_to_string(fixture("cmake/CMakeLists.txt")).unwrap(),
        CMAKE_LISTS
    );
}

fn skeleton_declarations(ws: &std::path::Path, file: &str) -> (Value, Vec<Value>) {
    let skeleton = cli_json(ws, &["query", "skeleton", file]);
    let list = skeleton["declarations"]
        .as_array()
        .unwrap_or_else(|| panic!("no declarations in {skeleton}"))
        .clone();
    (skeleton, list)
}

#[test]
fn skeleton_is_the_table_set() {
    let (_, list) = skeleton_declarations(&fixture("cmake"), "CMakeLists.txt");
    let actual: BTreeSet<(String, String, String, String)> = list
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
        ("module", "shop", "2:1-2:26", "2:9-2:13"),
        ("variable", "SRCS", "3:1-3:18", "3:5-3:9"),
        ("variable", "WITH_TESTS", "4:1-4:30", "4:8-4:18"),
        ("function", "add_tool", "5:1-7:14", "5:10-5:18"),
        ("class", "${name}", "6:3-6:34", "6:18-6:25"),
        ("macro", "log", "8:1-10:11", "8:7-8:10"),
        ("class", "core", "11:1-11:33", "11:13-11:17"),
        ("include", "tests", "13:3-13:26", "13:20-13:25"),
        ("include", "cmake/Deps.cmake", "15:1-15:26", "15:9-15:25"),
    ]
    .into_iter()
    .map(|(k, n, r, nr)| (k.into(), n.into(), r.into(), nr.into()))
    .collect();
    assert_eq!(actual, expected);
    let names: BTreeSet<&str> = list.iter().map(|d| d["name"].as_str().unwrap()).collect();
    for absent in ["cmake_minimum_required", "message", "if", "add_tool(cli)"] {
        assert!(!names.contains(absent), "{absent} in {names:?}");
    }
    assert_eq!(
        list.iter().filter(|d| d["name"] == "add_tool").count(),
        1,
        "the call add_tool(cli) is not a declaration"
    );
}

#[test]
fn nested_target_lies_inside_its_function() {
    let (_, list) = skeleton_declarations(&fixture("cmake"), "CMakeLists.txt");
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
    let (function, target) = (range("add_tool"), range("${name}"));
    assert!(
        function.0 <= target.0 && target.1 <= function.1,
        "{function:?} {target:?}"
    );
}

#[test]
fn mcp_skeleton_gives_the_table_names_and_kinds_without_import_entries() {
    let skeleton = mcp_payload(
        &Mcp::start(&fixture("cmake"))
            .call("skeleton", json!({ "file": "CMakeLists.txt" }))
            .unwrap(),
    );
    assert_eq!(
        pairs(&skeleton["declarations"]),
        pair_set(&[
            ("module", "shop"),
            ("variable", "SRCS"),
            ("variable", "WITH_TESTS"),
            ("function", "add_tool"),
            ("class", "${name}"),
            ("macro", "log"),
            ("class", "core"),
            ("include", "tests"),
            ("include", "cmake/Deps.cmake"),
        ])
    );
}

#[test]
fn symbols_are_the_table_set() {
    let (actual, symbols) = symbol_triples(&fixture("cmake"), "CMakeLists.txt");
    let expected: BTreeSet<Triple> = [
        ("module", "shop", "2:9-2:13"),
        ("variable", "SRCS", "3:5-3:9"),
        ("variable", "WITH_TESTS", "4:8-4:18"),
        ("function", "add_tool", "5:10-5:18"),
        ("class", "${name}", "6:18-6:25"),
        ("function", "log", "8:7-8:10"),
        ("class", "core", "11:13-11:17"),
    ]
    .into_iter()
    .map(|(k, n, r)| (k.into(), n.into(), r.into()))
    .collect();
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
        ("shop", "module"),
        ("SRCS", "variable"),
        ("WITH_TESTS", "variable"),
        ("add_tool", "function"),
        ("${name}", "class"),
        ("log", "macro"),
        ("core", "class"),
        ("tests", "include"),
        ("cmake/Deps.cmake", "include"),
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
    let ws = fixture("cmake");
    let batch = cli_json(
        &ws,
        &["query", "symbols-batch", "--files", "CMakeLists.txt"],
    );
    assert_eq!(name_kind(&batch), expected, "{batch}");
    let result = Mcp::start(&ws)
        .call("symbols_batch", json!({ "files": ["CMakeLists.txt"] }))
        .unwrap();
    assert_eq!(name_kind(&mcp_payload(&result)), expected, "{result}");
}

#[test]
fn mcp_symbols_give_the_table_names_with_mcp_kinds() {
    let symbols = mcp_payload(
        &Mcp::start(&fixture("cmake"))
            .call("symbols", json!({ "file": "CMakeLists.txt" }))
            .unwrap(),
    );
    assert_eq!(
        pairs(&symbols["symbols"]),
        pair_set(&[
            ("Module", "shop"),
            ("Variable", "SRCS"),
            ("Variable", "WITH_TESTS"),
            ("Function", "add_tool"),
            ("Class", "${name}"),
            ("Function", "log"),
            ("Class", "core"),
        ])
    );
}

#[test]
fn query_symbol_returns_the_name_range() {
    let symbol = cli_json(
        &fixture("cmake"),
        &["query", "symbol", "add_tool", "--file", "CMakeLists.txt"],
    );
    assert_eq!(span(&symbol["symbol"]["range"]), "5:10-5:18", "{symbol}");
}

#[test]
fn other_build_targets_and_packages_are_reported() {
    let ws = workspace(
        "CMakeLists.txt",
        "add_custom_target(docs ALL)\nfind_package(Threads REQUIRED)\nADD_LIBRARY(core STATIC a.c)\n",
    );
    let (symbols, _) = symbol_triples(ws.path(), "CMakeLists.txt");
    let expected: BTreeSet<Triple> = [
        ("class", "docs", "1:19-1:23"),
        ("class", "core", "3:13-3:17"),
    ]
    .into_iter()
    .map(|(k, n, r)| (k.into(), n.into(), r.into()))
    .collect();
    assert_eq!(symbols, expected);
    let (_, list) = skeleton_declarations(ws.path(), "CMakeLists.txt");
    let kinds: BTreeSet<(String, String)> = pairs(&Value::Array(list));
    assert_eq!(
        kinds,
        pair_set(&[("class", "docs"), ("import", "Threads"), ("class", "core"),])
    );
}

#[test]
fn a_target_named_by_a_variable_and_a_command_under_if_false_are_reported() {
    let ws = workspace(
        "CMakeLists.txt",
        "if(FALSE)\n  add_executable(${name} main.c)\n  set(hidden 1)\nendif()\n",
    );
    let (symbols, _) = symbol_triples(ws.path(), "CMakeLists.txt");
    let names: BTreeSet<(String, String)> = symbols
        .iter()
        .map(|(k, n, _)| (k.clone(), n.clone()))
        .collect();
    assert_eq!(
        names,
        pair_set(&[("class", "${name}"), ("variable", "hidden")])
    );
}

const DELIMITED: &str = "set(\"FOO\" 1)
set([[BAR]] 2)
function(\"qf\" a)
endfunction()
add_library(\"lib\" STATIC a.c)
";

#[test]
fn a_comment_before_the_first_argument_does_not_hide_a_declaration() {
    let source = "set(\n  # c\n  FOO 1\n)\nset(BAR 1 BAZ)\nset(#[[ b ]] BRK 3)\nfunction(\n  # c\n  qf a)\nendfunction()\nadd_library(# c\n  core STATIC a.c)\n";
    let expected = pair_set(&[
        ("variable", "FOO"),
        ("variable", "BAR"),
        ("variable", "BRK"),
        ("function", "qf"),
        ("class", "core"),
    ]);
    let ws = workspace("CMakeLists.txt", source);
    let (_, list) = skeleton_declarations(ws.path(), "CMakeLists.txt");
    assert_eq!(pairs(&Value::Array(list)), expected);
    let (symbols, _) = symbol_triples(ws.path(), "CMakeLists.txt");
    let symbol_pairs: BTreeSet<(String, String)> =
        symbols.into_iter().map(|(k, n, _)| (k, n)).collect();
    assert_eq!(symbol_pairs, expected);
}

#[test]
fn quote_and_bracket_delimiters_are_not_part_of_a_name() {
    let ws = workspace("CMakeLists.txt", DELIMITED);
    let (_, list) = skeleton_declarations(ws.path(), "CMakeLists.txt");
    let actual: BTreeSet<Triple> = list
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
                span(&d["name_range"]),
            )
        })
        .collect();
    let expected: BTreeSet<Triple> = [
        ("variable", "FOO", "1:6-1:9"),
        ("variable", "BAR", "2:7-2:10"),
        ("function", "qf", "3:11-3:13"),
        ("class", "lib", "5:14-5:17"),
    ]
    .into_iter()
    .map(|(k, n, r)| (k.into(), n.into(), r.into()))
    .collect();
    assert_eq!(actual, expected);
    let (symbols, _) = symbol_triples(ws.path(), "CMakeLists.txt");
    assert_eq!(symbols, expected);
}

#[test]
fn a_quoted_argument_is_a_use_of_its_content() {
    let ws = workspace(
        "CMakeLists.txt",
        "add_library(core a.c)\ntarget_link_libraries(app \"core\")\n",
    );
    assert_eq!(
        sites(&reference_sites(ws.path(), "core", "1", "13")),
        [(1, 13), (2, 28)].into_iter().collect()
    );
}

fn reference_sites(ws: &std::path::Path, name: &str, line: &str, column: &str) -> Value {
    cli_json(
        ws,
        &[
            "query",
            "references",
            name,
            "--file",
            "CMakeLists.txt",
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
fn a_call_in_other_letter_case_is_not_a_reference() {
    let ws = workspace(
        "CMakeLists.txt",
        "function(add_tool name)\nendfunction()\nADD_TOOL(cli)\nadd_tool(x)\n",
    );
    let answer = reference_sites(ws.path(), "add_tool", "1", "10");
    assert_eq!(sites(&answer), [(1, 10), (4, 1)].into_iter().collect());
}

#[test]
fn variable_uses_and_unquoted_arguments_are_uses_of_the_name() {
    let ws = workspace(
        "CMakeLists.txt",
        "add_library(core a.c)\ntarget_link_libraries(app core)\nset(x ${SRCS})\nmessage(${x})\n",
    );
    assert_eq!(
        sites(&reference_sites(ws.path(), "core", "1", "13")),
        [(1, 13), (2, 27)].into_iter().collect()
    );
    assert_eq!(
        sites(&reference_sites(ws.path(), "x", "3", "5")),
        [(3, 5), (4, 11)].into_iter().collect()
    );
}

#[test]
fn same_name_sets_are_a_name_match_on_cli() {
    let ws = workspace("CMakeLists.txt", TWO_SETS);
    let answer = reference_sites(ws.path(), "v", "1", "17");
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    assert_eq!(sites(&answer), [(1, 17), (2, 5)].into_iter().collect());
}

fn assert_mcp_name_match(mcp: &mut Mcp, reason: &str) {
    let result = mcp
        .call(
            "references",
            json!({ "symbol": "v", "file": "CMakeLists.txt" }),
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
        Some(2),
        "{answer}"
    );
    let text = mcp_text(&result);
    assert!(text.contains(NAME_LEVEL_HINT), "{text}");
    assert!(!text.contains("Install"), "{text}");
}

#[test]
fn same_name_sets_are_a_name_match_on_mcp() {
    let ws = workspace("CMakeLists.txt", TWO_SETS);
    assert_mcp_name_match(&mut Mcp::start(ws.path()), "no language server");
}

#[test]
fn mcp_reason_reads_disabled_when_language_servers_are_disabled() {
    let ws = workspace("CMakeLists.txt", TWO_SETS);
    let mut mcp = Mcp::start_with_env(ws.path(), &[("ACT_LITE_DISABLE_LSP", "1")]);
    assert_mcp_name_match(&mut mcp, "language servers are disabled");
}

#[test]
fn callers_needs_a_language_server_on_cli_and_mcp() {
    let ws = fixture("cmake");
    let output = cli(
        &ws,
        &["query", "callers", "add_tool", "--file", "CMakeLists.txt"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr
            .contains("needs a language server, and act-lite has none configured for CMake files"),
        "{stderr}"
    );
    assert!(!stderr.contains("hint:"), "{stderr}");
    let message = Mcp::start(&ws)
        .call(
            "callers",
            json!({ "symbol": "add_tool", "file": "CMakeLists.txt" }),
        )
        .unwrap_err();
    assert!(
        message.contains(
            "query type 'callers' needs a language server, and act-lite has none configured for CMake files"
        ),
        "{message}"
    );
}
