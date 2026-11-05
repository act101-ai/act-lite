//! `definition` for the languages added beside the `tests/fixtures/langs/`
//! matrix: the candidates are the declarations `symbols` reports, at their
//! symbols range, and a dotted use resolves to a dotted declaration name.
//! Every case runs on the command line and over MCP.

mod langs_added;

use langs_added::{Mcp, cli, fixture};
use serde_json::{Value, json};
use std::path::Path;

fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    for (file, content) in files {
        std::fs::write(ws.path().join(file), content).unwrap();
    }
    ws
}

/// `line:column-end_line:end_column` of a definition payload.
fn span(payload: &Value) -> String {
    let d = &payload["definition"];
    format!(
        "{}:{}-{}:{}",
        d["line"], d["column"], d["end_line"], d["end_column"]
    )
}

/// CLI `query definition`: `Ok` the span, `Err` the failure output.
fn cli_definition(
    ws: &Path,
    file: &str,
    name: &str,
    line: u32,
    column: u32,
) -> Result<String, String> {
    let (line, column) = (line.to_string(), column.to_string());
    let output = cli(
        ws,
        &[
            "query",
            "definition",
            name,
            "--file",
            file,
            "--line",
            &line,
            "--column",
            &column,
        ],
    );
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        return Err(format!(
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let payload: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {stdout}"));
    Ok(span(&payload))
}

/// MCP `definition`: `Ok` the span, `Err` the error text.
fn mcp_definition(
    ws: &Path,
    file: &str,
    name: &str,
    line: u32,
    column: u32,
) -> Result<String, String> {
    let result = Mcp::start(ws).call(
        "definition",
        json!({ "symbol": name, "file": file, "line": line, "column": column }),
    )?;
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    match serde_json::from_str::<Value>(text) {
        Ok(payload) if payload.get("definition").is_some() => Ok(span(&payload)),
        _ => Err(text.to_string()),
    }
}

/// Both surfaces return `expected` for `name` at `line:column` of `file`.
fn assert_found(ws: &Path, file: &str, name: &str, line: u32, column: u32, expected: &str) {
    for (surface, answer) in [
        ("cli", cli_definition(ws, file, name, line, column)),
        ("mcp", mcp_definition(ws, file, name, line, column)),
    ] {
        assert_eq!(
            answer.as_deref(),
            Ok(expected),
            "{surface}: definition of {name} at {file}:{line}:{column}"
        );
    }
}

/// Both surfaces give the not-found error for `name` at `line:column`.
fn assert_not_found(ws: &Path, file: &str, name: &str, line: u32, column: u32) {
    let message = format!("No definition found for symbol '{name}'");
    for (surface, answer) in [
        ("cli", cli_definition(ws, file, name, line, column)),
        ("mcp", mcp_definition(ws, file, name, line, column)),
    ] {
        match answer {
            Err(text) => assert!(
                text.contains(&message),
                "{surface}: {name} at {file}:{line}:{column}: {text}"
            ),
            Ok(found) => panic!("{surface}: {name} at {file}:{line}:{column} found {found}"),
        }
    }
}

#[test]
fn graphql_named_type_resolves_to_the_interface_name() {
    assert_found(
        &fixture("graphql"),
        "schema.graphql",
        "Node",
        1,
        22,
        "7:11-7:15",
    );
}

#[test]
fn protobuf_field_type_resolves_to_the_nested_message_name() {
    assert_found(
        &fixture("protobuf"),
        "shop.proto",
        "Line",
        8,
        12,
        "7:11-7:15",
    );
}

#[test]
fn make_prerequisite_resolves_to_the_target() {
    assert_found(&fixture("make"), "Makefile", "app", 8, 6, "9:1-9:4");
}

#[test]
fn cmake_command_resolves_to_the_function_name() {
    assert_found(
        &fixture("cmake"),
        "CMakeLists.txt",
        "add_tool",
        16,
        1,
        "5:10-5:18",
    );
}

#[test]
fn zig_use_resolves_to_the_variable_and_a_parameter_is_not_a_declaration() {
    let ws = fixture("zig");
    assert_found(&ws, "point.zig", "counter", 11, 9, "9:5-9:12");
    assert_not_found(&ws, "point.zig", "self", 5, 16);
}

#[test]
fn r_parameter_dots_and_assigned_function() {
    assert_found(&fixture("r"), "scale.R", "k", 3, 7, "2:25-2:26");
    let dots = workspace(&[("a.R", "h <- function(...) list(...)\n")]);
    assert_found(dots.path(), "a.R", "...", 1, 25, "1:15-1:18");
    let call = workspace(&[("a.R", "f <- function() 1\nf()\n")]);
    assert_found(call.path(), "a.R", "f", 2, 1, "1:1-1:2");
}

#[test]
fn julia_type_export_macro_and_short_form_function() {
    let ws = fixture("julia");
    assert_found(&ws, "geometry.jl", "Circle", 13, 18, "6:8-6:14");
    assert_found(&ws, "geometry.jl", "area", 4, 8, "13:10-13:14");
    let mac = workspace(&[("a.jl", "macro twice(ex) end\n@twice 1\n")]);
    assert_found(mac.path(), "a.jl", "twice", 2, 2, "1:7-1:12");
    let short = workspace(&[("a.jl", "g(x) = x\ng(1)\n")]);
    assert_found(short.path(), "a.jl", "g", 2, 1, "1:1-1:2");
}

#[test]
fn nix_variable_resolves_to_the_binding() {
    assert_found(&fixture("nix"), "default.nix", "mkTool", 9, 22, "4:3-4:9");
}

#[test]
fn xml_end_tag_resolves_to_the_start_tag_name() {
    assert_found(&fixture("xml"), "pom.xml", "name", 5, 17, "5:6-5:10");
}

#[test]
fn imports_and_includes_are_never_candidates() {
    assert_not_found(&fixture("zig"), "point.zig", "std", 17, 9);
    let julia = fixture("julia");
    assert_not_found(&julia, "geometry.jl", "LinearAlgebra", 2, 7);
    assert_not_found(&julia, "geometry.jl", "show", 3, 15);
    assert_not_found(&fixture("r"), "scale.R", "stats", 1, 9);
}

/// `query references` at `line:column`: each reference as `line:column`.
fn reference_sites(ws: &Path, file: &str, name: &str, line: u32, column: u32) -> Vec<String> {
    let (line, column) = (line.to_string(), column.to_string());
    let output = cli(
        ws,
        &[
            "query",
            "references",
            name,
            "--file",
            file,
            "--line",
            &line,
            "--column",
            &column,
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    result["references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| format!("{}:{}", r["line"], r["column"]))
        .collect()
}

#[test]
fn nix_dotted_use_matches_the_whole_attribute_path() {
    let ws = workspace(&[(
        "a.nix",
        "{ packages.default = 1; x = self.packages.default; }\n",
    )]);
    assert_eq!(
        reference_sites(ws.path(), "a.nix", "packages.default", 1, 3),
        ["1:3", "1:34"]
    );
    assert_found(ws.path(), "a.nix", "default", 1, 43, "1:3-1:19");
}

#[test]
fn julia_dotted_use_matches_the_whole_field_expression() {
    let ws = workspace(&[("a.jl", "Base.show(io, x) = 1\nBase.show(io, 2)\n")]);
    assert_eq!(
        reference_sites(ws.path(), "a.jl", "Base.show", 1, 1),
        ["1:1", "2:1"]
    );
    assert_found(ws.path(), "a.jl", "show", 2, 6, "1:1-1:10");
}
