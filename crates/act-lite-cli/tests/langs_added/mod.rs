//! Shared helpers for the tests of the languages added beside the
//! `tests/fixtures/langs/` matrix: the built `act-lite` on the command line
//! and `act-lite mcp serve` over stdio, each with an isolated configuration
//! directory.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Output, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// A path under `tests/fixtures/langs-added/`.
#[allow(dead_code)]
pub fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/langs-added")
        .join(rel)
}

/// Runs the built `act-lite --workspace <workspace> <args>` from `workspace`,
/// with `ACT_CONFIG_DIR` an empty temporary directory.
pub fn cli(workspace: &Path, args: &[&str]) -> Output {
    let config = tempfile::tempdir().unwrap();
    Command::new(assert_cmd::cargo::cargo_bin("act-lite"))
        .current_dir(workspace)
        .env("ACT_CONFIG_DIR", config.path())
        .arg("--workspace")
        .arg(workspace)
        .args(args)
        .output()
        .expect("the act-lite binary must start")
}

/// Runs [`cli`], asserts exit 0 and returns the parsed stdout.
pub fn cli_json(workspace: &Path, args: &[&str]) -> Value {
    let output = cli(workspace, args);
    assert!(
        output.status.success(),
        "act-lite {args:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout of act-lite {args:?} is not JSON ({e}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

/// A running `act-lite mcp serve` over stdio.
pub struct Mcp {
    child: Child,
    stdin: ChildStdin,
    replies: Receiver<String>,
    next_id: u64,
    _config: tempfile::TempDir,
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Mcp {
    /// Starts `act-lite --workspace <workspace> mcp serve` from `workspace`,
    /// with `ACT_CONFIG_DIR` an empty temporary directory, and performs the
    /// `initialize` handshake.
    pub fn start(workspace: &Path) -> Mcp {
        let config = tempfile::tempdir().unwrap();
        let mut child = Command::new(assert_cmd::cargo::cargo_bin("act-lite"))
            .current_dir(workspace)
            .env("ACT_CONFIG_DIR", config.path())
            .arg("--workspace")
            .arg(workspace)
            .args(["mcp", "serve"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the act-lite binary must start");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, replies) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut mcp = Mcp {
            child,
            stdin,
            replies,
            next_id: 1,
            _config: config,
        };
        mcp.request(
            "initialize",
            json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}),
        );
        mcp.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        mcp
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        loop {
            let line = self
                .replies
                .recv_timeout(Duration::from_secs(60))
                .unwrap_or_else(|e| panic!("no reply to {method} (id {id}): {e}"));
            if let Ok(v) = serde_json::from_str::<Value>(&line)
                && v["id"] == id
            {
                return v;
            }
        }
    }

    /// Calls `tool`: `Ok` holds the `tools/call` response's `result` object as
    /// sent, `Err` the JSON-RPC error message.
    pub fn call(&mut self, tool: &str, args: Value) -> Result<Value, String> {
        let reply = self.request("tools/call", json!({"name": tool, "arguments": args}));
        match reply.get("error") {
            Some(error) => Err(error["message"].as_str().unwrap_or_default().to_string()),
            None => Ok(reply["result"].clone()),
        }
    }
}
