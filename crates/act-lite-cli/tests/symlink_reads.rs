//! A symlink inside the workspace that points to a file outside the root is
//! refused by every navigation tool, and the tool does not show the content
//! of the outside file.

#![cfg(unix)]

use act_lite_cli::mcp::ActLiteServer;
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Value, json};
use std::path::Path;

const REFUSAL: &str = "path must be inside the workspace root";

async fn connect(root: &Path) -> RunningService<RoleClient, ()> {
    let (server_io, client_io) = tokio::io::duplex(1 << 16);
    let server = ActLiteServer::with_root(root, false);
    tokio::spawn(async move {
        if let Ok(running) = server.serve(server_io).await {
            let _ = running.waiting().await;
        }
    });
    ().serve(client_io)
        .await
        .expect("client initialization must succeed")
}

fn request(tool: &'static str, arguments: Value) -> CallToolRequestParams {
    CallToolRequestParams::new(tool).with_arguments(arguments.as_object().unwrap().clone())
}

#[tokio::test]
async fn navigation_tools_refuse_a_symlink_to_an_outside_file() {
    let w = tempfile::tempdir().unwrap();
    let ws = w.path().join("ws");
    let out = w.path().join("out");
    std::fs::create_dir_all(ws.join("sub")).unwrap();
    std::fs::create_dir(&out).unwrap();
    std::fs::write(out.join("secret.rs"), "fn outside_secret_fn() {}\n").unwrap();
    // A relative and an absolute symlink, at the top and one level down.
    std::os::unix::fs::symlink("../out/secret.rs", ws.join("rel.rs")).unwrap();
    std::os::unix::fs::symlink(out.join("secret.rs"), ws.join("sub/abs.rs")).unwrap();

    let client = connect(&ws).await;
    for file in ["rel.rs", "sub/abs.rs"] {
        let calls = [
            ("skeleton", json!({ "file": file })),
            ("symbols", json!({ "file": file })),
            (
                "references",
                json!({ "symbol": "outside_secret_fn", "file": file }),
            ),
            (
                "callers",
                json!({ "symbol": "outside_secret_fn", "file": file }),
            ),
            (
                "definition",
                json!({ "symbol": "outside_secret_fn", "file": file, "line": 1, "column": 4 }),
            ),
            (
                "symbols_batch",
                json!({ "ids": [format!("{file}::outside_secret_fn#function")] }),
            ),
        ];
        for (tool, arguments) in calls {
            let label = format!("{tool} {arguments}");
            let err = client
                .call_tool(request(tool, arguments))
                .await
                .expect_err(&format!("{label} must be refused"))
                .to_string();
            assert!(err.contains(REFUSAL), "{label}: {err}");
            assert!(!err.contains("outside_secret_fn()"), "{label}: {err}");
        }
        let batch = client
            .call_tool(request("symbols_batch", json!({ "files": [file] })))
            .await
            .expect("symbols_batch reports path errors in its result");
        let text = batch.content[0].as_text().unwrap().text.clone();
        assert!(text.contains(REFUSAL), "symbols_batch {file}: {text}");
        assert!(
            !text.contains("\"outside_secret_fn\""),
            "symbols_batch {file}: {text}"
        );
    }
    let _ = client.cancel().await;
}
