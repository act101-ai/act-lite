//! Discovery of the added languages by extension and by conventional file
//! name, on the command line and over MCP. Every case writes its files into
//! its own temporary directory, so names that differ only in letter case hold
//! on case-insensitive file systems.

mod langs_added;

use langs_added::{Mcp, cli, cli_json};
use serde_json::{Value, json};
use std::path::Path;

/// The language `query repo-outline` gives `file`, if it lists it.
fn outline_language(workspace: &Path, file: &str) -> Option<String> {
    let outline = cli_json(workspace, &["query", "repo-outline"]);
    outline["files"]
        .as_array()
        .unwrap_or_else(|| panic!("no files in {outline}"))
        .iter()
        .find(|f| f["path"] == file)
        .map(|f| f["language"].as_str().unwrap_or_default().to_string())
}

/// Writes each `(file, content)` into one new temporary directory and checks
/// that `query skeleton` exits 0, `query repo-outline` lists the file under
/// `language`, and MCP `skeleton` answers it.
fn assert_discovered(files: &[(&str, &str)], language: &str) {
    let ws = tempfile::tempdir().unwrap();
    for (file, content) in files {
        std::fs::write(ws.path().join(file), content).unwrap();
    }
    let mut mcp = Mcp::start(ws.path());
    for (file, _) in files {
        let skeleton = cli_json(ws.path(), &["query", "skeleton", file]);
        assert!(
            skeleton["declarations"].is_array(),
            "{file}: no declarations in {skeleton}"
        );
        assert_eq!(
            outline_language(ws.path(), file).as_deref(),
            Some(language),
            "{file}: repo-outline language"
        );
        let result = mcp
            .call("skeleton", json!({ "file": file }))
            .unwrap_or_else(|e| panic!("{file}: MCP skeleton failed: {e}"));
        assert_ne!(
            result["isError"],
            Value::Bool(true),
            "{file}: MCP skeleton is an error: {result}"
        );
    }
}

/// One-line `a.<ext>` files for each extension, with `line` as content.
fn by_extension(exts: &[&str], line: &str, language: &str) {
    let names: Vec<String> = exts.iter().map(|ext| format!("a.{ext}")).collect();
    let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), line)).collect();
    assert_discovered(&files, language);
}

#[test]
fn xml_extensions() {
    by_extension(
        &[
            "xml", "xsd", "xsl", "xslt", "svg", "plist", "csproj", "fsproj", "vbproj", "props",
            "targets", "resx", "xaml",
        ],
        "<a/>\n",
        "XML",
    );
}

#[test]
fn graphql_extensions() {
    by_extension(
        &["graphql", "graphqls", "gql"],
        "type A { a: Int }\n",
        "GraphQL",
    );
}

#[test]
fn protobuf_extension() {
    by_extension(&["proto"], "syntax = \"proto3\";\n", "Protocol Buffers");
}

#[test]
fn make_extensions() {
    by_extension(&["mk", "mak"], "x = 1\n", "Make");
}

#[test]
fn cmake_extension() {
    by_extension(&["cmake"], "set(x 1)\n", "CMake");
}

#[test]
fn nix_extension() {
    by_extension(&["nix"], "{ a = 1; }\n", "Nix");
}

#[test]
fn zig_extension() {
    by_extension(&["zig"], "const a = 1;\n", "Zig");
}

#[test]
fn julia_extension() {
    by_extension(&["jl"], "a = 1\n", "Julia");
}

#[test]
fn r_upper_case_extension() {
    by_extension(&["R"], "a <- 1\n", "R");
}

#[test]
fn r_lower_case_extension() {
    by_extension(&["r"], "a <- 1\n", "R");
}

#[test]
fn rprofile_name() {
    assert_discovered(&[(".Rprofile", "options(digits = 4)\n")], "R");
}

#[test]
fn jsonc_extensions() {
    by_extension(&["jsonc", "code-workspace"], "{\"a\": 1}\n", "JSONC");
}

#[test]
fn jsonc_names() {
    for name in [
        "tsconfig.json",
        "tsconfig.app.json",
        "jsconfig.json",
        "devcontainer.json",
        ".devcontainer.json",
        ".eslintrc.json",
    ] {
        assert_discovered(&[(name, "{\"a\": 1}\n")], "JSONC");
    }
}

#[test]
fn mytsconfig_json_stays_json() {
    assert_discovered(&[("mytsconfig.json", "{\"a\": 1}\n")], "JSON");
}

#[test]
fn makefile_name() {
    assert_discovered(&[("Makefile", "all:\n\ttrue\n")], "Make");
}

#[test]
fn lowercase_makefile_name() {
    assert_discovered(&[("makefile", "all:\n\ttrue\n")], "Make");
}

#[test]
fn gnumakefile_name() {
    assert_discovered(&[("GNUmakefile", "all:\n\ttrue\n")], "Make");
}

#[test]
fn cmakelists_name() {
    assert_discovered(&[("CMakeLists.txt", "project(a)\n")], "CMake");
}

/// Names that look like a convention but are not one: `query skeleton`
/// refuses each with the unsupported-file-type error, and `repo-outline`
/// does not list it.
#[test]
fn near_miss_names_are_refused() {
    for name in [
        "Makefile.bak",
        "makefile.txt",
        "cmakelists.txt",
        "CMakeLists.txt.orig",
        "notes.txt",
        "tsconfig.json.bak",
    ] {
        let ws = tempfile::tempdir().unwrap();
        std::fs::write(ws.path().join(name), "all:\n").unwrap();
        let output = cli(ws.path(), &["query", "skeleton", name]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{name} must be refused: {stderr}");
        assert!(
            stderr.contains("unsupported file type") && stderr.contains(name),
            "{name}: {stderr}"
        );
        assert_eq!(outline_language(ws.path(), name), None, "{name} listed");
    }
}
