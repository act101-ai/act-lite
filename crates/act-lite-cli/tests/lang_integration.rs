//! The added languages through the command line and MCP together: output
//! limits, the effect of newly recognized files on existing repositories, and
//! the absence of any language server for the ten additions.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};

fn workspace(files: &[(&str, String)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    dir
}

/// The parsed JSON of the first content block of an MCP result, cut at the
/// first line: the symbols tool appends a plain-text note after the JSON.
fn first_block(result: &Value) -> Value {
    let text = result["content"][0]["text"].as_str().unwrap();
    serde_json::from_str(text.lines().next().unwrap()).unwrap()
}

fn text_of(result: &Value) -> &str {
    result["content"][0]["text"].as_str().unwrap()
}

/// A Nix attribute set with 60 file-level bindings.
fn sixty_bindings() -> String {
    let mut source = String::from("{\n");
    for i in 0..60 {
        source.push_str(&format!("  b{i} = {i};\n"));
    }
    source.push_str("}\n");
    source
}

#[test]
fn mcp_symbols_returns_fifty_unless_the_limit_says_more() {
    let ws = workspace(&[("big.nix", sixty_bindings())]);
    let mut mcp = Mcp::start(ws.path());
    let result = mcp.call("symbols", json!({"file": "big.nix"})).unwrap();
    assert_eq!(
        first_block(&result)["symbols"].as_array().unwrap().len(),
        50
    );
    assert!(
        text_of(&result).contains("Note: Returning 50 of 60 symbols."),
        "{result}"
    );
    let result = mcp
        .call("symbols", json!({"file": "big.nix", "limit": 60}))
        .unwrap();
    assert_eq!(
        first_block(&result)["symbols"].as_array().unwrap().len(),
        60
    );
    assert!(!text_of(&result).contains("Note:"), "{result}");
}

#[test]
fn repo_outline_max_files_one_lists_one_file_and_says_truncated() {
    let tree = fixture("");
    let cli_out = cli_json(&tree, &["query", "repo-outline", "--max-files", "1"]);
    assert_eq!(cli_out["files"].as_array().unwrap().len(), 1, "{cli_out}");
    assert_eq!(cli_out["truncated"], true, "{cli_out}");
    let mut mcp = Mcp::start(&tree);
    let result = mcp.call("repo_outline", json!({"max_files": 1})).unwrap();
    assert_eq!(first_block(&result), cli_out);
}

#[test]
fn newly_recognized_files_take_part_of_the_repo_outline_budget() {
    let mut files = Vec::new();
    for i in 0..3 {
        files.push((format!("a{i}.svg"), "<svg/>\n".to_string()));
        files.push((format!("b{i}.ts"), "export const a = 1;\n".to_string()));
    }
    let files: Vec<(&str, String)> = files.iter().map(|(p, c)| (p.as_str(), c.clone())).collect();
    let ws = workspace(&files);
    let cli_out = cli_json(ws.path(), &["query", "repo-outline", "--max-files", "4"]);
    assert_eq!(cli_out["files"].as_array().unwrap().len(), 4, "{cli_out}");
    assert_eq!(cli_out["truncated"], true, "{cli_out}");
    let mut mcp = Mcp::start(ws.path());
    let result = mcp.call("repo_outline", json!({"max_files": 4})).unwrap();
    assert_eq!(first_block(&result), cli_out);
}

/// One Rust function with ten `if`s: complexity 11.
fn branchy_rust() -> String {
    let mut source = String::from("pub fn ok(x: i32) -> i32 {\n");
    for _ in 0..10 {
        source.push_str("    if x > 0 { return 1; }\n");
    }
    source.push_str("    x\n}\n");
    source
}

#[test]
fn an_xml_test_file_adds_one_excluded_entry_and_changes_no_finding() {
    let base = [
        ("src/ok.rs", branchy_rust()),
        ("tests/t.rs", "fn t() {}\n".to_string()),
    ];
    let with_xml = [
        ("src/ok.rs", branchy_rust()),
        ("tests/t.rs", "fn t() {}\n".to_string()),
        ("tests/a.xml", "<a/>\n".to_string()),
    ];
    let before = workspace(&base);
    let after = workspace(&with_xml);
    for analyzer in ["patterns", "hotspots", "clones"] {
        let old = cli_json(before.path(), &["analyze", analyzer]);
        let new = cli_json(after.path(), &["analyze", analyzer]);
        assert_eq!(old["excluded"]["tests"], json!([{"path": "tests/t.rs"}]));
        assert_eq!(
            new["excluded"]["tests"],
            json!([{"path": "tests/a.xml"}, {"path": "tests/t.rs"}]),
            "{analyzer}: {new}"
        );
        assert_eq!(new["findings"], old["findings"], "{analyzer}");
        assert_eq!(new["status"], old["status"], "{analyzer}");
    }
}

/// Each addition with a symbol its fixture declares.
const ADDITIONS: [(&str, &str, &str); 10] = [
    ("JSONC", "jsonc/settings.jsonc", "files"),
    ("XML", "xml/pom.xml", "dependency"),
    ("GraphQL", "graphql/schema.graphql", "User"),
    ("Protocol Buffers", "protobuf/shop.proto", "Order"),
    ("Make", "make/Makefile", "app"),
    ("CMake", "cmake/CMakeLists.txt", "add_tool"),
    ("Zig", "zig/point.zig", "main"),
    ("R", "r/scale.R", "scale_by"),
    ("Julia", "julia/geometry.jl", "area"),
    ("Nix", "nix/default.nix", "mkTool"),
];

/// The language-server binaries act-lite knows; none serves an addition.
const SERVERS: [&str; 15] = [
    "typescript-language-server",
    "OmniSharp",
    "pyright-langserver",
    "rust-analyzer",
    "gopls",
    "elixir-ls",
    "jdtls",
    "kotlin-language-server",
    "clangd",
    "ruby-lsp",
    "sqls",
    "intelephense",
    "metals",
    "sourcekit-lsp",
    "dart",
];

#[test]
fn status_lists_every_language_and_no_server_for_the_ten() {
    let ws = tempfile::tempdir().unwrap();
    let mut mcp = Mcp::start(ws.path());
    let status = first_block(&mcp.call("status", json!({})).unwrap());
    let languages: Vec<&str> = status["languages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(languages.len(), 43, "{status}");
    for (name, _, _) in ADDITIONS {
        assert!(languages.contains(&name), "{name} missing: {status}");
    }
    let mut servers: Vec<&str> = status["lsp"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    servers.sort();
    let mut expected = SERVERS.to_vec();
    expected.sort();
    assert_eq!(servers, expected, "{status}");
}

#[test]
fn callers_on_an_addition_names_the_missing_server_on_both_surfaces() {
    for (name, rel, symbol) in ADDITIONS {
        let source = fixture(rel);
        let workspace = source.parent().unwrap();
        let file = source.file_name().unwrap().to_str().unwrap();
        let out = cli(workspace, &["query", "callers", symbol, "--file", file]);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{name}: {stderr}");
        let expected =
            format!("needs a language server, and act-lite has none configured for {name} files");
        assert!(stderr.contains(&expected), "{name}: {stderr}");
        assert!(!stderr.contains("hint:"), "{name}: {stderr}");
        let mut mcp = Mcp::start(workspace);
        let message = mcp
            .call("callers", json!({"symbol": symbol, "file": file}))
            .unwrap_err();
        assert!(message.contains(&expected), "{name}: {message}");
    }
}
