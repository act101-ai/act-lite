//! In-process tests for the act-lite MCP server: a real rmcp client talks to
//! the server over a tokio duplex, so every assertion reads what an MCP
//! client would receive.

use act_lite_cli::mcp::ActLiteServer;
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Starts an LSP-disabled server rooted at `root` and connects a client to it.
async fn connect(root: &Path) -> RunningService<RoleClient, ()> {
    let (server_io, client_io) = tokio::io::duplex(1 << 16);
    let server = ActLiteServer::with_root(root, false);
    tokio::spawn(async move {
        server
            .serve(server_io)
            .await
            .expect("server initialization must succeed")
            .waiting()
            .await
            .expect("server must shut down cleanly")
    });
    ().serve(client_io)
        .await
        .expect("client initialization must succeed")
}

fn args(value: Value) -> rmcp::model::JsonObject {
    value
        .as_object()
        .expect("arguments must be an object")
        .clone()
}

/// The first text block of a tool result, parsed as JSON.
fn first_json(result: &rmcp::model::CallToolResult) -> Value {
    let text = &result.content[0]
        .as_text()
        .expect("first content block must be text")
        .text;
    serde_json::from_str(text).unwrap_or_else(|e| panic!("tool output is not JSON ({e}): {text}"))
}

#[tokio::test]
async fn tools_list_is_exactly_the_eleven() {
    let client = connect(&fixtures()).await;
    let mut names: Vec<String> = client
        .list_all_tools()
        .await
        .expect("tools/list must succeed")
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    names.sort();
    // Builds with debug assertions also serve the test analyzer.
    let mut want = vec![
        "analyze_clones",
        "analyze_hotspots",
        "analyze_patterns",
        "callers",
        "definition",
        "references",
        "repo_outline",
        "skeleton",
        "status",
        "symbols",
        "symbols_batch",
    ];
    if cfg!(debug_assertions) {
        want.push("analyze_fixture");
        want.sort();
    }
    assert_eq!(names, want);
    client.cancel().await.expect("client must cancel");
}

#[tokio::test]
async fn skeleton_tool_returns_add() {
    let client = connect(&fixtures()).await;
    let result = client
        .call_tool(
            CallToolRequestParams::new("skeleton")
                .with_arguments(args(serde_json::json!({ "file": "sample.ts" }))),
        )
        .await
        .expect("skeleton must succeed");
    let value = first_json(&result);
    let found = value["declarations"]
        .as_array()
        .unwrap_or_else(|| panic!("no declarations in {value}"))
        .iter()
        .any(|d| d["name"] == "add" && d["kind"] == "function");
    assert!(found, "expected function `add` in {value}");
    client.cancel().await.expect("client must cancel");
}

#[tokio::test]
async fn symbols_on_directory_is_actionable_error() {
    let client = connect(&fixtures()).await;
    let dir = fixtures().display().to_string();
    let err = client
        .call_tool(
            CallToolRequestParams::new("symbols")
                .with_arguments(args(serde_json::json!({ "file": dir }))),
        )
        .await
        .expect_err("a directory must be refused")
        .to_string();
    assert!(
        err.contains("single source file"),
        "the error must say a single file is required: {err}"
    );
    client.cancel().await.expect("client must cancel");
}

#[tokio::test]
async fn skeleton_on_empty_path_is_actionable_error() {
    let client = connect(&fixtures()).await;
    let err = client
        .call_tool(
            CallToolRequestParams::new("skeleton")
                .with_arguments(args(serde_json::json!({ "file": "" }))),
        )
        .await
        .expect_err("an empty path must be refused")
        .to_string();
    assert!(
        err.contains("single source file"),
        "the error must say a single file is required: {err}"
    );
    client.cancel().await.expect("client must cancel");
}

#[tokio::test]
async fn skeleton_on_unsupported_extension_lists_languages() {
    let client = connect(&fixtures()).await;
    let err = client
        .call_tool(
            CallToolRequestParams::new("skeleton")
                .with_arguments(args(serde_json::json!({ "file": "sample.cob" }))),
        )
        .await
        .expect_err("a .cob file must be refused")
        .to_string();
    assert!(err.contains(".cob"), "must name the extension: {err}");
    assert!(err.contains("TypeScript"), "must list the languages: {err}");
    client.cancel().await.expect("client must cancel");
}

#[tokio::test]
async fn status_lists_every_language() {
    let client = connect(&fixtures()).await;
    let result = client
        .call_tool(CallToolRequestParams::new("status"))
        .await
        .expect("status must succeed");
    let value = first_json(&result);
    let languages = value["languages"]
        .as_array()
        .unwrap_or_else(|| panic!("no languages in {value}"));
    assert_eq!(languages.len(), 36, "{value}");
    let lsp = value["lsp"]
        .as_object()
        .unwrap_or_else(|| panic!("no lsp object in {value}"));
    assert!(
        lsp.values().all(|s| s == "ready" || s == "not_installed"),
        "each server is ready or not_installed: {value}"
    );
    client.cancel().await.expect("client must cancel");
}

#[tokio::test]
async fn references_without_server_falls_back_and_says_so() {
    let client = connect(&fixtures()).await;
    let result = client
        .call_tool(
            CallToolRequestParams::new("references").with_arguments(args(
                serde_json::json!({ "symbol": "add", "file": "sample.ts" }),
            )),
        )
        .await
        .expect("references must fall back to tree-sitter");
    let value = first_json(&result);
    assert!(
        value["resolved_by"]
            .as_str()
            .is_some_and(|s| s.starts_with("tree-sitter")),
        "the answer must say it came from tree-sitter: {value}"
    );
    let found = value["references"]
        .as_array()
        .unwrap_or_else(|| panic!("no references in {value}"))
        .iter()
        .any(|r| {
            r["file"]
                .as_str()
                .is_some_and(|f| f.ends_with("use_sample.ts"))
        });
    assert!(
        found,
        "the call site in use_sample.ts must be found: {value}"
    );
    client.cancel().await.expect("client must cancel");
}

#[tokio::test]
async fn repo_outline_holds_max_files_and_depth() {
    let workspace = tempfile::tempdir().unwrap();
    for i in 0..5 {
        std::fs::write(workspace.path().join(format!("f{i}.ts")), "const x = 1;\n").unwrap();
    }
    std::fs::create_dir(workspace.path().join("sub")).unwrap();
    std::fs::write(workspace.path().join("sub/deep.ts"), "const y = 2;\n").unwrap();
    let client = connect(workspace.path()).await;

    let outline = |arguments: Value| {
        let client = &client;
        async move {
            let result = client
                .call_tool(
                    CallToolRequestParams::new("repo_outline").with_arguments(args(arguments)),
                )
                .await
                .expect("repo_outline must succeed");
            first_json(&result)
        }
    };

    let capped = outline(serde_json::json!({ "max_files": 2 })).await;
    assert_eq!(
        capped["files"].as_array().map(Vec::len),
        Some(2),
        "{capped}"
    );

    let shallow = outline(serde_json::json!({ "depth": 1 })).await;
    let paths: Vec<&str> = shallow["files"]
        .as_array()
        .unwrap_or_else(|| panic!("no files in {shallow}"))
        .iter()
        .filter_map(|f| f["path"].as_str())
        .collect();
    assert_eq!(paths.len(), 5, "{shallow}");
    assert!(!paths.iter().any(|p| p.contains("deep")), "{shallow}");
    client.cancel().await.expect("client must cancel");
}

#[tokio::test]
async fn symbols_batch_reports_unsupported_file_with_languages() {
    let client = connect(&fixtures()).await;
    let result = client
        .call_tool(
            CallToolRequestParams::new("symbols_batch").with_arguments(args(
                serde_json::json!({ "files": ["sample.ts", "sample.cob"] }),
            )),
        )
        .await
        .expect("a batch with one bad file still answers for the rest");
    let value = first_json(&result);
    let errors = value["path_errors"]
        .as_array()
        .unwrap_or_else(|| panic!("no path_errors in {value}"));
    let cob = errors
        .iter()
        .find(|e| e["file"] == "sample.cob")
        .unwrap_or_else(|| panic!("no error for sample.cob in {value}"));
    let msg = cob["error"].as_str().unwrap_or_default();
    assert!(msg.contains(".cob") && msg.contains("TypeScript"), "{msg}");
    assert_eq!(
        value["results"].as_array().map(Vec::len),
        Some(1),
        "{value}"
    );
    client.cancel().await.expect("client must cancel");
}

/// A root reached through a symlink: repo_outline `path` results must still
/// be workspace-relative.
#[cfg(unix)]
#[tokio::test]
async fn repo_outline_path_under_symlinked_root_is_workspace_relative() {
    let real = tempfile::tempdir().unwrap();
    std::fs::create_dir(real.path().join("sub")).unwrap();
    std::fs::write(real.path().join("sub/lib.rs"), "pub fn a() {}\n").unwrap();
    let links = tempfile::tempdir().unwrap();
    let root = links.path().join("ws");
    std::os::unix::fs::symlink(real.path(), &root).unwrap();
    let client = connect(&root).await;
    let result = client
        .call_tool(
            CallToolRequestParams::new("repo_outline")
                .with_arguments(args(serde_json::json!({ "path": "sub" }))),
        )
        .await
        .expect("repo_outline must succeed");
    let value = first_json(&result);
    let paths: Vec<&str> = value["files"]
        .as_array()
        .unwrap_or_else(|| panic!("no files in {value}"))
        .iter()
        .filter_map(|f| f["path"].as_str())
        .collect();
    assert_eq!(paths, ["sub/lib.rs"], "{value}");
    client.cancel().await.expect("client must cancel");
}

/// The instructions the server sent in its initialize result.
fn read_instructions(client: &RunningService<RoleClient, ()>) -> String {
    client
        .peer_info()
        .expect("the client must hold the initialize result")
        .instructions
        .clone()
        .expect("the server must send instructions")
}

/// The entries of the language list, each with its `*` mark if it has one.
fn listed_languages(text: &str) -> Vec<String> {
    let line = text
        .lines()
        .skip_while(|l| !l.starts_with("Languages ("))
        .nth(1)
        .unwrap_or_else(|| panic!("no language list in the instructions: {text}"));
    line.split(", ").map(str::to_string).collect()
}

#[tokio::test]
async fn instructions_name_every_language_and_tool() {
    let client = connect(&fixtures()).await;
    let text = read_instructions(&client);
    let grammars = act_lite_parser::Grammar::all();

    let names: Vec<String> = listed_languages(&text)
        .iter()
        .map(|l| l.trim_end_matches('*').to_string())
        .collect();
    let expected: Vec<&str> = grammars.iter().map(|g| g.name()).collect();
    assert_eq!(names, expected, "the list must be Grammar::all() in order");
    assert!(
        text.contains(&format!("across {} languages", grammars.len())),
        "the count must come from Grammar::all(): {text}"
    );

    let tools = client
        .list_all_tools()
        .await
        .expect("tools/list must succeed");
    for tool in &tools {
        assert!(
            text.lines()
                .any(|l| l.starts_with(&format!("- {}: ", tool.name))),
            "no line for tool {}: {text}",
            tool.name
        );
    }
    assert_eq!(
        text.lines().filter(|l| l.starts_with("- ")).count(),
        tools.len(),
        "one line per tool, no others: {text}"
    );
    client.cancel().await.expect("client must cancel");
}

#[tokio::test]
async fn instructions_mark_exactly_the_lsp_languages() {
    let client = connect(&fixtures()).await;
    let text = read_instructions(&client);
    let marked: Vec<String> = listed_languages(&text)
        .into_iter()
        .filter_map(|l| l.strip_suffix('*').map(str::to_string))
        .collect();
    let expected: Vec<&str> = act_lite_parser::Grammar::all()
        .into_iter()
        .filter(|g| act_lite_lsp::LspServer::for_grammar(*g).is_some())
        .map(|g| g.name())
        .collect();
    assert_eq!(
        marked, expected,
        "marked languages must be those with a server"
    );
    client.cancel().await.expect("client must cancel");
}
