//! Deep nesting in a Rust source file must not kill the MCP server. The test
//! region search (`analyze::embedded::collect`) runs for every analyzer when
//! `include_tests` is false; it must give a result or a per-file skip, not a
//! stack overflow. The server runs in a child process so that a crash fails
//! this test and does not abort the test harness.

#![cfg(unix)]

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;

/// A depth far below what a parser or a file-size cap must refuse, and above
/// the depth at which v0.2.0 overflows the stack (about 2,650 in a debug
/// build).
const DEPTH: usize = 10_000;

/// `fn f() -> u8 { ((( ... 1 ... ))) }` with `depth` parentheses.
fn deep_source(depth: usize) -> String {
    format!(
        "fn f() -> u8 {{\n{}1{}\n}}\n",
        "(".repeat(depth),
        ")".repeat(depth)
    )
}

/// Calls `tool` with `arguments` on a fresh `act-lite mcp serve` rooted at
/// `ws`. Returns the reply (None when the server closed stdout first) and the
/// exit status after stdin is closed.
fn call_once(ws: &Path, tool: &str, arguments: Value) -> (Option<Value>, std::process::ExitStatus) {
    let config = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_act-lite"))
        .env("ACT_CONFIG_DIR", config.path())
        .args(["--disable-lsp", "--workspace"])
        .arg(ws)
        .args(["mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the act-lite binary must start");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut send = |m: Value| {
        let _ = writeln!(stdin, "{m}");
        let _ = stdin.flush();
    };
    send(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}),
    );
    send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    send(
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":tool,"arguments":arguments}}),
    );
    let mut reply = None;
    while let Ok(line) = rx.recv_timeout(Duration::from_secs(120)) {
        if let Ok(v) = serde_json::from_str::<Value>(&line)
            && v["id"] == 2
        {
            reply = Some(v);
            break;
        }
    }
    drop(stdin);
    // Closed stdin ends a live server; a dead one has exited already.
    let status = child.wait().unwrap();
    (reply, status)
}

#[test]
fn deep_nesting_does_not_kill_the_server() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::create_dir(ws.path().join("src")).unwrap();
    std::fs::write(ws.path().join("src/deep.rs"), deep_source(DEPTH)).unwrap();

    // Control: with test regions kept, the same file is analyzed.
    let (reply, status) = call_once(
        ws.path(),
        "analyze_patterns",
        json!({"include_tests": true}),
    );
    assert!(
        reply.is_some() && status.success(),
        "control failed: {status:?}"
    );

    for analyzer in ["analyze_patterns", "analyze_hotspots", "analyze_clones"] {
        let (reply, status) = call_once(ws.path(), analyzer, json!({}));
        assert!(
            reply.is_some(),
            "{analyzer}: no reply; the server died with signal {:?} (status {status:?})",
            status.signal()
        );
        assert!(status.success(), "{analyzer}: {status:?}");
    }
}

#[test]
fn deep_nesting_with_a_syntax_error_does_not_kill_the_server() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::create_dir(ws.path().join("src")).unwrap();
    // `1 +` at the innermost level is a syntax error at full depth, so the
    // first-error search descends through every level.
    let source = format!(
        "fn f() -> u8 {{\n{}1 +{}\n}}\n",
        "(".repeat(DEPTH),
        ")".repeat(DEPTH)
    );
    std::fs::write(ws.path().join("src/deep.rs"), source).unwrap();

    for analyzer in ["analyze_patterns", "analyze_hotspots", "analyze_clones"] {
        let (reply, status) = call_once(ws.path(), analyzer, json!({}));
        assert!(
            reply.is_some(),
            "{analyzer}: no reply; the server died with signal {:?} (status {status:?})",
            status.signal()
        );
        assert!(status.success(), "{analyzer}: {status:?}");
    }
}
