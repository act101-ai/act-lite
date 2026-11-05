//! MCP file tools answer only for files inside the workspace root: a path
//! that leaves it through `..`, an absolute path or a symlink is refused, as
//! `repo_outline` refuses it.

#![cfg(unix)]

use act_lite_cli::mcp::ActLiteServer;
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::Value;
use std::path::Path;

const REFUSAL: &str = "path must be inside the workspace root";

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

fn request(tool: &'static str, arguments: Value) -> CallToolRequestParams {
    CallToolRequestParams::new(tool).with_arguments(
        arguments
            .as_object()
            .expect("arguments must be an object")
            .clone(),
    )
}

/// The first text block of a tool result.
fn first_text(result: &rmcp::model::CallToolResult) -> String {
    result.content[0]
        .as_text()
        .expect("first content block must be text")
        .text
        .clone()
}

#[cfg(unix)]
#[tokio::test]
async fn file_tools_refuse_paths_outside_the_workspace() {
    let w = tempfile::tempdir().unwrap();
    let ws = w.path().join("ws");
    let out = w.path().join("out");
    std::fs::create_dir(&ws).unwrap();
    std::fs::create_dir(&out).unwrap();
    std::fs::write(ws.join("a.py"), "def inside_fn():\n    return 1\n").unwrap();
    std::fs::write(out.join("o.py"), "def outside_fn():\n    return 2\n").unwrap();
    std::fs::write(out.join("private-notes.txt"), "outside\n").unwrap();
    std::os::unix::fs::symlink("../out/o.py", ws.join("link.py")).unwrap();
    std::os::unix::fs::symlink("../out", ws.join("outdir")).unwrap();
    let absolute = out.join("o.py").display().to_string();
    let absolute_id = format!("{absolute}::outside_fn#function");

    let client = connect(&ws).await;

    let inside = client
        .call_tool(request("symbols", serde_json::json!({ "file": "a.py" })))
        .await
        .expect("a file inside the workspace must be answered");
    assert!(
        first_text(&inside).contains("inside_fn"),
        "{}",
        first_text(&inside)
    );

    let refused = [
        ("symbols", serde_json::json!({ "file": "../out/o.py" })),
        ("symbols", serde_json::json!({ "file": absolute })),
        ("symbols", serde_json::json!({ "file": "link.py" })),
        ("skeleton", serde_json::json!({ "file": "outdir/o.py" })),
        (
            "references",
            serde_json::json!({ "symbol": "outside_fn", "file": "../out/o.py" }),
        ),
        (
            "callers",
            serde_json::json!({ "symbol": "outside_fn", "file": "../out/o.py" }),
        ),
        (
            "definition",
            serde_json::json!({ "symbol": "outside_fn", "file": "../out/o.py", "line": 1, "column": 5 }),
        ),
        ("symbols", serde_json::json!({ "file": "../out/nope.py" })),
        ("symbols", serde_json::json!({ "file": "outdir/nope.py" })),
        (
            "symbols_batch",
            serde_json::json!({ "ids": ["../out/o.py::outside_fn#function"] }),
        ),
        ("symbols_batch", serde_json::json!({ "ids": [absolute_id] })),
        (
            "symbols_batch",
            serde_json::json!({ "ids": ["link.py::outside_fn#function"] }),
        ),
        (
            "symbols_batch",
            serde_json::json!({ "ids": ["../out/nope.py::x#function"] }),
        ),
    ];
    for (tool, arguments) in refused {
        let label = format!("{tool} {arguments}");
        let err = client
            .call_tool(request(tool, arguments))
            .await
            .expect_err(&format!("{label} must be refused"))
            .to_string();
        assert!(err.contains(REFUSAL), "{label}: {err}");
        assert!(!err.contains("outside_fn"), "{label}: {err}");
        assert!(!err.contains("private-notes"), "{label}: {err}");
    }

    for file in ["../out/o.py", "../out/nope.py"] {
        let batch = client
            .call_tool(request(
                "symbols_batch",
                serde_json::json!({ "files": [file] }),
            ))
            .await
            .expect("symbols_batch reports path errors in its result");
        let text = first_text(&batch);
        assert!(!text.contains("outside_fn"), "{file}: {text}");
        assert!(!text.contains("private-notes"), "{file}: {text}");
        let value: Value = serde_json::from_str(&text).unwrap();
        let error = value["path_errors"][0]["error"]
            .as_str()
            .unwrap_or_else(|| panic!("{file}: no path error in {value}"));
        assert!(error.contains(REFUSAL), "{file}: {value}");
    }

    let outline = client
        .call_tool(request(
            "repo_outline",
            serde_json::json!({ "symbols": true }),
        ))
        .await
        .expect("repo_outline of the workspace must succeed");
    let text = first_text(&outline);
    assert!(text.contains("inside_fn"), "{text}");
    assert!(!text.contains("outside_fn"), "{text}");

    client.cancel().await.expect("client must cancel");
}

#[cfg(unix)]
#[tokio::test]
async fn missing_path_error_lists_no_outside_directory() {
    let w = tempfile::tempdir().unwrap();
    let ws = w.path().join("ws");
    let out = w.path().join("out");
    std::fs::create_dir_all(ws.join("sub")).unwrap();
    std::fs::create_dir(&out).unwrap();
    std::fs::write(ws.join("a.py"), "def inside_fn():\n    return 1\n").unwrap();
    std::fs::write(ws.join("sub/s.py"), "def sub_fn():\n    return 3\n").unwrap();
    std::fs::write(out.join("o.py"), "def outside_fn():\n    return 2\n").unwrap();
    std::fs::write(out.join("private-notes.txt"), "outside\n").unwrap();
    std::fs::write(w.path().join("outside-marker.txt"), "outside\n").unwrap();
    std::os::unix::fs::symlink("../out", ws.join("outdir")).unwrap();
    std::os::unix::fs::symlink("sub", ws.join("linkdir")).unwrap();

    // True when `text` names an entry of a directory outside the workspace:
    // `out/`'s files, the workspace parent's files, or `out` itself.
    let leaks = |text: &str| {
        text.contains("o.py")
            || text.contains("private-notes")
            || text.contains("outside-marker")
            || text.lines().any(|l| l.trim_end().ends_with("out"))
    };

    let client = connect(&ws).await;

    let missing = [
        ("symbols", serde_json::json!({ "file": "../out/nope.py" })),
        ("repo_outline", serde_json::json!({ "path": "../out/nope" })),
        ("repo_outline", serde_json::json!({ "path": "outdir/nope" })),
        ("symbols", serde_json::json!({ "file": "../ws/nope/x.py" })),
        (
            "repo_outline",
            serde_json::json!({ "path": "../ws/nope/x" }),
        ),
        (
            "symbols",
            serde_json::json!({ "file": "sub/../../ws/nope/x.py" }),
        ),
    ];
    for (tool, arguments) in missing {
        let label = format!("{tool} {arguments}");
        let err = client
            .call_tool(request(tool, arguments))
            .await
            .expect_err(&format!("{label} must fail"))
            .to_string();
        assert!(!leaks(&err), "{label}: {err}");
    }

    let batch = client
        .call_tool(request(
            "symbols_batch",
            serde_json::json!({ "files": ["../ws/nope/x.py"] }),
        ))
        .await
        .expect("symbols_batch reports path errors in its result");
    let text = first_text(&batch);
    assert!(!leaks(&text), "symbols_batch: {text}");

    // A missing path inside the workspace still lists the workspace.
    let absolute = ws.join("missing.py").display().to_string();
    let inside = [
        ("missing.py", "a.py"),
        ("sub/missing.py", "s.py"),
        ("linkdir/missing.py", "s.py"),
        (absolute.as_str(), "a.py"),
    ];
    for (file, listed) in inside {
        let err = client
            .call_tool(request("symbols", serde_json::json!({ "file": file })))
            .await
            .expect_err(&format!("symbols {file} must fail"))
            .to_string();
        assert!(err.contains("Nearest root:"), "{file}: {err}");
        assert!(err.contains(listed), "{file}: {err}");
        assert!(!leaks(&err), "{file}: {err}");
    }

    client.cancel().await.expect("client must cancel");
}

#[cfg(unix)]
#[tokio::test]
async fn definition_does_not_follow_a_symlink_out_of_the_workspace() {
    let w = tempfile::tempdir().unwrap();
    let ws = w.path().join("ws");
    let outside = w.path().join("outside");
    std::fs::create_dir_all(ws.join("sub")).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(ws.join("a.ts"), "secretFn();\ninsideFn();\n").unwrap();
    std::fs::write(ws.join("sub/c.ts"), "function insideFn() {}\n").unwrap();
    std::fs::write(outside.join("b.ts"), "function secretFn() {}\n").unwrap();
    std::os::unix::fs::symlink("../outside", ws.join("ext")).unwrap();
    std::os::unix::fs::symlink("sub", ws.join("linked")).unwrap();

    let client = connect(&ws).await;

    let err = client
        .call_tool(request(
            "definition",
            serde_json::json!({ "symbol": "secretFn", "file": "a.ts", "line": 1, "column": 2 }),
        ))
        .await
        .expect_err("a definition outside the workspace must not be found")
        .to_string();
    assert!(!err.contains("ext/b.ts"), "{err}");
    assert!(!err.contains("outside"), "{err}");

    let inside = client
        .call_tool(request(
            "definition",
            serde_json::json!({ "symbol": "insideFn", "file": "a.ts", "line": 2, "column": 2 }),
        ))
        .await
        .expect("a definition inside the workspace must be found");
    let text = first_text(&inside);
    assert!(text.contains("c.ts"), "{text}");

    client.cancel().await.expect("client must cancel");
}
