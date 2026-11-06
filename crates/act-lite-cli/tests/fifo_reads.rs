//! The file tools refuse a path that is not a regular file. A named pipe
//! (FIFO) inside the workspace must give an error at once: it must not block
//! the tool until some other process writes to the pipe.

#![cfg(unix)]

use act_lite_cli::mcp::ActLiteServer;
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Value, json};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::Duration;

/// The longest time a refusal may take.
const TIMEOUT: Duration = Duration::from_secs(5);

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

fn mkfifo(path: &Path) {
    let c = CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(
        unsafe { libc::mkfifo(c.as_ptr(), 0o600) },
        0,
        "mkfifo failed"
    );
}

/// Opens the write end of `fifo` and closes it, so that a reader that waits
/// on the pipe gets end-of-file. Nothing happens when no reader waits.
fn release_reader(fifo: &Path) {
    let _ = std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(fifo);
}

// The multi-thread runtime keeps the client timer alive while a server task
// blocks a worker thread in a read of the pipe.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn file_tools_refuse_a_named_pipe_without_blocking() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    std::fs::write(root.join("a.rs"), "fn inside() {}\n").unwrap();
    let fifo = root.join("pipe.rs");
    mkfifo(&fifo);

    let client = connect(root).await;
    let calls = [
        ("skeleton", json!({ "file": "pipe.rs" })),
        ("symbols", json!({ "file": "pipe.rs" })),
        ("references", json!({ "symbol": "x", "file": "pipe.rs" })),
        ("callers", json!({ "symbol": "x", "file": "pipe.rs" })),
        (
            "definition",
            json!({ "symbol": "x", "file": "pipe.rs", "line": 1, "column": 1 }),
        ),
        ("symbols_batch", json!({ "files": ["pipe.rs"] })),
        ("symbols_batch", json!({ "ids": ["pipe.rs::x#function"] })),
    ];
    let mut blocked = Vec::new();
    for (tool, arguments) in calls {
        let label = format!("{tool} {arguments}");
        let outcome =
            tokio::time::timeout(TIMEOUT, client.call_tool(request(tool, arguments))).await;
        // Unblock a server thread that waits on the pipe before the next call.
        release_reader(&fifo);
        match outcome {
            Err(_) => blocked.push(label),
            Ok(Ok(result)) => {
                // symbols_batch with `files` reports a path error in its result.
                let text = result.content[0].as_text().unwrap().text.clone();
                assert!(text.contains("path_errors"), "{label}: not refused: {text}");
            }
            Ok(Err(_refused)) => {}
        }
    }
    assert!(
        blocked.is_empty(),
        "these calls blocked for {TIMEOUT:?} on a FIFO: {blocked:#?}"
    );
    let _ = client.cancel().await;
}
