//! `definition` for the languages added beside the `tests/fixtures/langs/`
//! matrix: the candidates are the declarations `symbols` reports, at their
//! symbols range, and a dotted use resolves to a dotted declaration name.
//! Every case runs on the command line and over MCP.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::path::Path;

fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    for (file, content) in files {
        std::fs::write(ws.path().join(file), content).unwrap();
    }
    ws
}

/// `symbol line:column-end_line:end_column` of a definition payload.
fn span(payload: &Value) -> String {
    let d = &payload["definition"];
    format!(
        "{} {}:{}-{}:{}",
        payload["symbol"].as_str().unwrap_or_default(),
        d["line"],
        d["column"],
        d["end_line"],
        d["end_column"]
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

/// Both surfaces return `expected`, `symbol line:column-end_line:end_column`,
/// for `name` at `line:column` of `file`.
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
        "Node 7:11-7:15",
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
        "Line 7:11-7:15",
    );
}

#[test]
fn make_prerequisite_resolves_to_the_target() {
    assert_found(&fixture("make"), "Makefile", "app", 8, 6, "app 9:1-9:4");
}

#[test]
fn cmake_command_resolves_to_the_function_name() {
    assert_found(
        &fixture("cmake"),
        "CMakeLists.txt",
        "add_tool",
        16,
        1,
        "add_tool 5:10-5:18",
    );
}

#[test]
fn zig_use_resolves_to_the_variable_and_a_parameter_is_not_a_declaration() {
    let ws = fixture("zig");
    assert_found(&ws, "point.zig", "counter", 11, 9, "counter 9:5-9:12");
    assert_not_found(&ws, "point.zig", "self", 5, 16);
}

#[test]
fn r_parameter_dots_and_assigned_function() {
    assert_found(&fixture("r"), "scale.R", "k", 3, 7, "k 2:25-2:26");
    let dots = workspace(&[("a.R", "h <- function(...) list(...)\n")]);
    assert_found(dots.path(), "a.R", "...", 1, 25, "... 1:15-1:18");
    let call = workspace(&[("a.R", "f <- function() 1\nf()\n")]);
    assert_found(call.path(), "a.R", "f", 2, 1, "f 1:1-1:2");
}

#[test]
fn julia_type_export_macro_and_short_form_function() {
    let ws = fixture("julia");
    assert_found(&ws, "geometry.jl", "Circle", 13, 18, "Circle 6:8-6:14");
    assert_found(&ws, "geometry.jl", "area", 4, 8, "area 13:10-13:14");
    let mac = workspace(&[("a.jl", "macro twice(ex) end\n@twice 1\n")]);
    assert_found(mac.path(), "a.jl", "twice", 2, 2, "twice 1:7-1:12");
    let short = workspace(&[("a.jl", "g(x) = x\ng(1)\n")]);
    assert_found(short.path(), "a.jl", "g", 2, 1, "g 1:1-1:2");
}

#[test]
fn nix_variable_resolves_to_the_binding() {
    assert_found(
        &fixture("nix"),
        "default.nix",
        "mkTool",
        9,
        22,
        "mkTool 4:3-4:9",
    );
}

#[test]
fn xml_end_tag_resolves_to_the_start_tag_name() {
    assert_found(&fixture("xml"), "pom.xml", "name", 5, 17, "name 5:6-5:10");
}

#[test]
fn imports_and_includes_are_never_candidates() {
    assert_not_found(&fixture("zig"), "point.zig", "std", 17, 9);
    let julia = fixture("julia");
    assert_not_found(&julia, "geometry.jl", "LinearAlgebra", 2, 7);
    assert_not_found(&julia, "geometry.jl", "show", 3, 15);
    assert_not_found(&fixture("r"), "scale.R", "stats", 1, 9);
    let cmake = fixture("cmake");
    assert_not_found(&cmake, "CMakeLists.txt", "include", 15, 3);
    assert_not_found(&cmake, "CMakeLists.txt", "cmake/Deps.cmake", 15, 9);
    assert_not_found(&fixture("nix"), "default.nix", "import", 5, 12);
}

#[test]
fn a_typed_or_commented_zig_import_is_never_a_candidate() {
    let ws = workspace(&[(
        "t.zig",
        "const a: type = @import(\"std\");\nconst b = @import(// c\n\"std\");\nconst d = @import(\"std\",);\nconst e = @import // c\n(\"std\");\nconst m = @import(\"a\\\"b.zig\");\nconst t = @import(\"std\") // c\n;\nconst a2: type = // c\n @import(\"std\");\npub // c\nconst ci: type = @import(\"std\");\nfn f() void {\n    _ = a;\n    _ = b;\n    _ = d;\n    _ = e;\n    _ = m;\n    _ = t;\n    _ = a2;\n    _ = ci;\n}\n",
    )]);
    assert_not_found(ws.path(), "t.zig", "a", 15, 9);
    assert_not_found(ws.path(), "t.zig", "b", 16, 9);
    assert_not_found(ws.path(), "t.zig", "d", 17, 9);
    assert_not_found(ws.path(), "t.zig", "e", 18, 9);
    assert_not_found(ws.path(), "t.zig", "m", 19, 9);
    assert_not_found(ws.path(), "t.zig", "t", 20, 9);
    assert_not_found(ws.path(), "t.zig", "a2", 21, 9);
    assert_not_found(ws.path(), "t.zig", "ci", 22, 9);
}

#[test]
fn julia_nested_signature_wrappers_are_candidates() {
    let ws = workspace(&[(
        "n.jl",
        "f(x::T)::T where {T} = x\nfunction h(x::T)::T where {T}\n    x\nend\nf(1)\nh(2)\n",
    )]);
    assert_found(ws.path(), "n.jl", "f", 5, 1, "f 1:1-1:2");
    assert_found(ws.path(), "n.jl", "h", 6, 1, "h 2:10-2:11");
}

#[test]
fn zig_extern_and_struct_literal_declarations_are_candidates() {
    let ws = workspace(&[(
        "x.zig",
        "extern var errno: c_int;\nconst value = struct { y: u8 }{ .y = 1 };\nextern const lim2: u8 // c = 1\n;\nfn f() void {\n    _ = errno;\n    _ = value;\n    _ = lim2;\n}\n",
    )]);
    assert_found(ws.path(), "x.zig", "errno", 6, 9, "errno 1:12-1:17");
    assert_found(ws.path(), "x.zig", "value", 7, 9, "value 2:7-2:12");
    assert_found(ws.path(), "x.zig", "lim2", 8, 9, "lim2 3:14-3:18");
}

#[test]
fn cmake_variable_use_resolves_to_the_variable_not_the_enclosing_argument() {
    let ws = workspace(&[(
        "CMakeLists.txt",
        "set(FOO 1)\nset(${FOO} 2)\nmessage(${FOO})\n",
    )]);
    assert_found(ws.path(), "CMakeLists.txt", "FOO", 3, 11, "FOO 1:5-1:8");
    assert_not_found(&fixture("cmake"), "CMakeLists.txt", "name", 6, 20);
}

#[test]
fn cmake_command_names_match_case_sensitively() {
    let ws = workspace(&[(
        "CMakeLists.txt",
        "function(add_tool name)\nendfunction()\nADD_TOOL(cli)\n",
    )]);
    assert_not_found(ws.path(), "CMakeLists.txt", "add_tool", 3, 1);
    assert_not_found(ws.path(), "CMakeLists.txt", "ADD_TOOL", 3, 1);
}

/// `query references` at `line:column`: each reference as
/// `line:column-line:end_column`. The output carries only the start, so the
/// end is the start plus `name`, after a check that the file text at the
/// reference's byte offset is the whole `name`.
fn reference_sites(ws: &Path, file: &str, name: &str, line: u32, column: u32) -> Vec<String> {
    let text = std::fs::read_to_string(ws.join(file)).unwrap();
    let (line, column) = (line.to_string(), column.to_string());
    let result = cli_json(
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
    result["references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let start = r["byte_offset"].as_u64().unwrap() as usize;
            assert_eq!(text.get(start..start + name.len()), Some(name), "{r}");
            let column = r["column"].as_u64().unwrap() as usize;
            format!(
                "{}:{}-{}:{}",
                r["line"],
                column,
                r["line"],
                column + name.len()
            )
        })
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
        ["1:3-1:19", "1:34-1:50"]
    );
    assert_found(
        ws.path(),
        "a.nix",
        "default",
        1,
        43,
        "packages.default 1:3-1:19",
    );
}

#[test]
fn julia_dotted_use_matches_the_whole_field_expression() {
    let ws = workspace(&[("a.jl", "Base.show(io, x) = 1\nBase.show(io, 2)\n")]);
    assert_eq!(
        reference_sites(ws.path(), "a.jl", "Base.show", 1, 1),
        ["1:1-1:10", "2:1-2:10"]
    );
    assert_found(ws.path(), "a.jl", "show", 2, 6, "Base.show 1:1-1:10");
}

/// `answer` finished within five seconds, or the test fails.
fn within_five_seconds(answer: impl FnOnce() -> Result<String, String> + Send + 'static) -> String {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(answer());
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("definition did not finish within 5 s")
        .expect("definition failed")
}

#[cfg(unix)]
#[test]
fn symlinks_back_to_the_root_do_not_multiply_the_walk() {
    let ws = workspace(&[("a.py", "foo()\n"), ("b.py", "def foo():\n    pass\n")]);
    std::fs::create_dir(ws.path().join("sub")).unwrap();
    for link in ["up", "up2"] {
        std::os::unix::fs::symlink("..", ws.path().join("sub").join(link)).unwrap();
    }
    let root = ws.path().to_path_buf();
    let cli = within_five_seconds({
        let root = root.clone();
        move || cli_definition(&root, "a.py", "foo", 1, 1)
    });
    let mcp = within_five_seconds(move || mcp_definition(&root, "a.py", "foo", 1, 1));
    assert_eq!(cli, "foo 1:5-1:8");
    assert_eq!(mcp, cli);
}

#[test]
fn a_sibling_over_the_size_cap_is_not_loaded() {
    let ws = workspace(&[("a.py", "foo()\n"), ("big.py", "def foo():\n    pass\n")]);
    let big = std::fs::OpenOptions::new()
        .write(true)
        .open(ws.path().join("big.py"))
        .unwrap();
    big.set_len(act_lite_parser::MAX_REFERENCE_WALK_FILE_BYTES + 1)
        .unwrap();
    assert_not_found(ws.path(), "a.py", "foo", 1, 1);
}

#[cfg(unix)]
#[test]
fn a_file_symlink_leaving_the_root_is_not_followed() {
    let outside = workspace(&[("b.py", "def foo():\n    pass\n")]);
    let ws = workspace(&[("a.py", "foo()\n")]);
    std::os::unix::fs::symlink(outside.path().join("b.py"), ws.path().join("b.py")).unwrap();
    assert_not_found(ws.path(), "a.py", "foo", 1, 1);
}
