//! End-to-end tests for `act-lite query`, run against the built binary.

use assert_cmd::Command;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn act_lite() -> Command {
    Command::cargo_bin("act-lite").expect("the act-lite binary must be built")
}

/// Runs `act-lite query <args>` from the fixtures directory and returns the
/// parsed stdout JSON, asserting success.
fn query_json(args: &[&str]) -> Value {
    let output = act_lite()
        .current_dir(fixtures())
        .arg("query")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "act-lite query {args:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn declarations(result: &Value) -> Vec<(String, String)> {
    result["declarations"]
        .as_array()
        .unwrap_or_else(|| panic!("no declarations array in {result}"))
        .iter()
        .map(|d| {
            (
                d["name"].as_str().unwrap_or_default().to_string(),
                d["kind"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// An empty directory for `PATH`, so no language server can be found or
/// installed.
fn empty_path_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// A three-file TypeScript workspace: `helper` is defined in util.ts and
/// used in main.ts; `format` is defined in both main.ts and other.ts and
/// used in its own file. The uses are bare statements, so the name at the
/// position is the used identifier, not a surrounding declaration.
fn definition_workspace() -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(
        ws.path().join("util.ts"),
        "export function helper(x: number): number { return x + 1; }\n",
    )
    .unwrap();
    std::fs::write(
        ws.path().join("main.ts"),
        "import { helper } from \"./util\";\n\
         export function format(s: string): string { return s.trim(); }\n\
         helper(1);\n\
         format(\"a\");\n",
    )
    .unwrap();
    std::fs::write(
        ws.path().join("other.ts"),
        "export function format(n: number): string { return String(n); }\n\
         format(1);\n",
    )
    .unwrap();
    ws
}

/// Runs `act-lite query definition` against a workspace file with an
/// isolated `PATH` and returns the parsed stdout JSON, asserting success.
fn definition_query(ws: &Path, name: &str, file: &Path, line: u32, column: u32) -> Value {
    let path = empty_path_dir();
    let output = act_lite()
        .env("PATH", path.path())
        .arg("--workspace")
        .arg(ws)
        .args(["query", "definition", name, "--file"])
        .arg(file)
        .args(["--line", &line.to_string(), "--column", &column.to_string()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "act-lite query definition {name} {}:{line}:{column} failed:\nstdout: {}\nstderr: {}",
        file.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

/// The definition payload is `{"symbol", "definition": {file, line, column,
/// end_line, end_column}}`, the same shape the MCP `definition` tool returns.
fn assert_definition_shape(value: &Value) {
    assert!(
        value.get("symbol").and_then(Value::as_str).is_some(),
        "payload must carry symbol: {value}"
    );
    let definition = value["definition"]
        .as_object()
        .unwrap_or_else(|| panic!("no definition object in {value}"));
    for key in ["file", "line", "column", "end_line", "end_column"] {
        assert!(
            definition.contains_key(key),
            "definition.{key} missing in {value}"
        );
    }
}

#[test]
fn definition_use_site_resolves_into_another_file() {
    let ws = definition_workspace();
    let result = definition_query(ws.path(), "helper", &ws.path().join("main.ts"), 3, 1);
    assert_definition_shape(&result);
    assert_eq!(result["symbol"], "helper", "{result}");
    assert!(
        result["definition"]["file"]
            .as_str()
            .unwrap()
            .ends_with("util.ts"),
        "the use in main.ts must resolve into util.ts: {result}"
    );
}

#[test]
fn definition_position_picks_among_same_named() {
    let ws = definition_workspace();
    // Two files define `format`; the use site's own file wins.
    let result = definition_query(ws.path(), "format", &ws.path().join("main.ts"), 4, 1);
    assert_eq!(result["symbol"], "format", "{result}");
    assert!(
        result["definition"]["file"]
            .as_str()
            .unwrap()
            .ends_with("main.ts"),
        "a use in main.ts must resolve to main.ts's format: {result}"
    );
    let result = definition_query(ws.path(), "format", &ws.path().join("other.ts"), 2, 1);
    assert!(
        result["definition"]["file"]
            .as_str()
            .unwrap()
            .ends_with("other.ts"),
        "a use in other.ts must resolve to other.ts's format: {result}"
    );
}

#[test]
fn definition_blank_name_takes_symbol_at_position() {
    let ws = definition_workspace();
    let result = definition_query(ws.path(), "", &ws.path().join("main.ts"), 3, 1);
    assert_eq!(result["symbol"], "helper", "{result}");
    assert!(
        result["definition"]["file"]
            .as_str()
            .unwrap()
            .ends_with("util.ts"),
        "the blank name must fall back to the symbol at the position: {result}"
    );
}

#[test]
fn definition_not_found_is_an_error() {
    let ws = definition_workspace();
    let path = empty_path_dir();
    let output = act_lite()
        .env("PATH", path.path())
        .arg("--workspace")
        .arg(ws.path())
        .args(["query", "definition", "missing", "--file"])
        .arg(ws.path().join("main.ts"))
        .args(["--line", "3", "--column", "1"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a name defined nowhere is an error, not a null answer"
    );
    assert!(
        stderr.contains("No definition found for symbol 'missing'"),
        "stderr must name the symbol: {stderr}"
    );
    assert!(!stderr.contains("panicked"), "must not panic: {stderr}");
}

#[test]
fn definition_relative_file_resolves_from_working_dir() {
    let ws = definition_workspace();
    let path = empty_path_dir();
    let output = act_lite()
        .current_dir(ws.path())
        .env("PATH", path.path())
        .arg("--workspace")
        .arg(ws.path())
        .args([
            "query",
            "definition",
            "helper",
            "--file",
            "main.ts",
            "--line",
            "3",
            "--column",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "a relative --file must resolve from the working directory:\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        result["definition"]["file"]
            .as_str()
            .unwrap()
            .ends_with("util.ts"),
        "{result}"
    );
}

#[test]
fn skeleton_ts_lists_function_signature() {
    let result = query_json(&["skeleton", "sample.ts"]);
    let decls = declarations(&result);
    assert!(
        decls.iter().any(|(n, k)| n == "add" && k == "function"),
        "expected function `add`, got {decls:?}"
    );
}

#[test]
fn symbols_python_lists_class_and_method() {
    let result = query_json(&["symbols", "sample.py"]);
    let symbols: Vec<(String, String)> = result["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("no symbols array in {result}"))
        .iter()
        .map(|s| {
            (
                s["name"].as_str().unwrap_or_default().to_string(),
                s["kind"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert!(
        symbols.iter().any(|(n, k)| n == "Greeter" && k == "class"),
        "expected class `Greeter`, got {symbols:?}"
    );
    assert!(
        symbols.iter().any(|(n, k)| n == "greet" && k == "method"),
        "expected method `greet`, got {symbols:?}"
    );
}

#[test]
fn unsupported_extension_errors_with_supported_list() {
    let output = act_lite()
        .current_dir(fixtures())
        .args(["query", "skeleton", "sample.cob"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "a .cob file must be refused");
    assert!(
        stderr.contains(".cob"),
        "stderr must name the extension: {stderr}"
    );
    assert!(
        stderr.contains("TypeScript"),
        "stderr must list the supported languages: {stderr}"
    );
    assert!(!stderr.contains("panicked"), "must not panic: {stderr}");
}

#[test]
fn partial_parse_still_returns_declarations() {
    let result = query_json(&["skeleton", "broken.rs"]);
    let decls = declarations(&result);
    assert!(
        decls.iter().any(|(n, _)| n == "ok"),
        "the declaration before the syntax error must survive, got {decls:?}"
    );
}

#[test]
fn callers_without_server_names_binary() {
    let path = empty_path_dir();
    let output = act_lite()
        .current_dir(fixtures())
        .env("PATH", path.path())
        .args(["query", "callers", "add", "--file", "sample.ts"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "callers needs a language server");
    assert!(
        stderr.contains("typescript-language-server"),
        "stderr must name the server binary to install: {stderr}"
    );
}

#[test]
fn references_without_server_falls_back() {
    let path = empty_path_dir();
    let output = act_lite()
        .current_dir(fixtures())
        .env("PATH", path.path())
        .args([
            "query",
            "references",
            "add",
            "--file",
            "sample.ts",
            "--line",
            "1",
            "--column",
            "17",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "references must fall back to tree-sitter: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let files: Vec<&str> = result["references"]
        .as_array()
        .unwrap_or_else(|| panic!("no references array in {result}"))
        .iter()
        .filter_map(|r| r["file"].as_str())
        .collect();
    assert!(
        files.iter().any(|f| f.ends_with("use_sample.ts")),
        "the call site in use_sample.ts must be found, got {files:?}"
    );
    assert_eq!(
        result["source"], "tree-sitter",
        "the output must say the answer came from tree-sitter: {result}"
    );
}

/// A `PATH` holding only a fake `npm` that records any run by writing
/// `ran` into its own directory.
#[cfg(unix)]
fn path_with_sentinel_npm() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let npm = dir.path().join("npm");
    std::fs::write(
        &npm,
        format!(
            "#!/bin/sh\n: > '{}'\nexit 1\n",
            dir.path().join("ran").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&npm, std::fs::Permissions::from_mode(0o755)).unwrap();
    // The installer runs `sh -c <install command>`; give it a shell.
    std::os::unix::fs::symlink("/bin/sh", dir.path().join("sh")).unwrap();
    dir
}

#[cfg(unix)]
#[test]
fn callers_without_server_never_runs_an_installer() {
    let path = path_with_sentinel_npm();
    let output = act_lite()
        .current_dir(fixtures())
        .env("PATH", path.path())
        .args(["query", "callers", "add", "--file", "sample.ts"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "callers needs a language server");
    assert!(
        !path.path().join("ran").exists(),
        "a missing server must not be installed: {stderr}"
    );
    assert!(stderr.contains("typescript-language-server"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn references_without_server_never_runs_an_installer() {
    let path = path_with_sentinel_npm();
    let output = act_lite()
        .current_dir(fixtures())
        .env("PATH", path.path())
        .args([
            "query",
            "references",
            "add",
            "--file",
            "sample.ts",
            "--line",
            "1",
            "--column",
            "17",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !path.path().join("ran").exists(),
        "a missing server must not be installed"
    );
}

#[test]
fn symbols_partial_parse_still_returns_declarations() {
    let result = query_json(&["symbols", "broken.rs"]);
    let names: Vec<&str> = result["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("no symbols array in {result}"))
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert!(names.contains(&"ok"), "got {names:?}");
    assert_eq!(result["parse_error"], true, "{result}");
}

/// `skeleton` and `symbol` report a partial parse as `symbols` does: a
/// top-level `"parse_error": true` on a tree with errors, no key on a clean
/// one.
#[test]
fn skeleton_and_symbol_report_a_parse_error() {
    let skeleton = query_json(&["skeleton", "broken.rs"]);
    assert_eq!(skeleton["parse_error"], true, "{skeleton}");
    let symbol = query_json(&["symbol", "ok", "--file", "broken.rs"]);
    assert_eq!(symbol["parse_error"], true, "{symbol}");
    let clean = query_json(&["skeleton", "sample.py"]);
    assert!(clean.get("parse_error").is_none(), "{clean}");
}

/// With `--workspace`, a relative file path resolves against the workspace
/// root, not the current directory, for every per-file query; a file of the
/// same name in the current directory is not read.
#[test]
fn relative_files_resolve_against_the_workspace() {
    let ws = definition_workspace();
    let cwd = tempfile::tempdir().unwrap();
    std::fs::write(
        cwd.path().join("main.ts"),
        "export function decoy(): void {}\n",
    )
    .unwrap();
    let path = empty_path_dir();
    let run = |args: &[&str]| -> Value {
        let output = act_lite()
            .current_dir(cwd.path())
            .env("PATH", path.path())
            .arg("--workspace")
            .arg(ws.path())
            .arg("--disable-lsp")
            .arg("query")
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "act-lite query {args:?} failed:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let skeleton = run(&["skeleton", "main.ts"]);
    let names: Vec<String> = declarations(&skeleton)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert!(names.contains(&"format".to_string()), "{skeleton}");
    assert!(!names.contains(&"decoy".to_string()), "{skeleton}");
    let symbols = run(&["symbols", "main.ts"]);
    assert!(symbols.to_string().contains("\"format\""), "{symbols}");
    run(&["symbol", "format", "--file", "main.ts"]);
    let definition = run(&[
        "definition",
        "helper",
        "--file",
        "main.ts",
        "--line",
        "3",
        "--column",
        "1",
    ]);
    assert!(
        definition["definition"]["file"]
            .as_str()
            .unwrap()
            .ends_with("util.ts"),
        "{definition}"
    );
    let references = run(&[
        "references",
        "helper",
        "--file",
        "main.ts",
        "--line",
        "3",
        "--column",
        "1",
    ]);
    assert!(references.to_string().contains("util.ts"), "{references}");
}

#[test]
fn symbols_batch_unsupported_file_lists_languages() {
    let output = act_lite()
        .current_dir(fixtures())
        .args(["query", "symbols-batch", "--files", "sample.cob"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "a .cob file must be refused");
    assert!(
        stderr.contains(".cob") && stderr.contains("TypeScript"),
        "{stderr}"
    );
}
