//! `act-lite mcp serve` speaks MCP over stdio.

use assert_cmd::Command;
use std::path::Path;

#[test]
fn mcp_serve_answers_initialize_as_act_lite() {
    let initialize = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
    let output = Command::cargo_bin("act-lite")
        .expect("the act-lite binary must be built")
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"))
        .args(["--disable-lsp", "mcp", "serve"])
        .write_stdin(format!("{initialize}\n"))
        .timeout(std::time::Duration::from_secs(20))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: serde_json::Value = stdout
        .lines()
        .find_map(|line| serde_json::from_str(line).ok())
        .unwrap_or_else(|| {
            panic!(
                "no JSON-RPC response on stdout: {stdout}\nstderr: {}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
    assert_eq!(response["id"], 1, "{response}");
    assert_eq!(
        response["result"]["serverInfo"]["name"], "act-lite",
        "{response}"
    );
}

/// `callers` over real stdio, LSP enabled, with no language server and a
/// `PATH` whose only `npm` records any run: the call must fail naming the
/// server binary, and nothing may be installed.
#[cfg(unix)]
#[test]
fn mcp_callers_without_server_names_binary_and_installs_nothing() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::fs::PermissionsExt;

    let path = tempfile::tempdir().unwrap();
    let npm = path.path().join("npm");
    std::fs::write(
        &npm,
        format!(
            "#!/bin/sh\n: > '{}'\nexit 1\n",
            path.path().join("ran").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&npm, std::fs::Permissions::from_mode(0o755)).unwrap();
    // The installer runs `sh -c <install command>`; give it a shell.
    std::os::unix::fs::symlink("/bin/sh", path.path().join("sh")).unwrap();

    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("act-lite"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"))
        .env("PATH", path.path())
        .arg("--workspace")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"))
        .args(["mcp", "serve"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    for message in [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"callers","arguments":{"symbol":"add","file":"sample.ts"}}}"#,
    ] {
        writeln!(stdin, "{message}").unwrap();
    }
    stdin.flush().unwrap();

    let mut response = serde_json::Value::Null;
    let mut line = String::new();
    while stdout.read_line(&mut line).unwrap() > 0 {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim())
            && v["id"] == 2
        {
            response = v;
            break;
        }
        line.clear();
    }
    child.kill().unwrap();
    child.wait().unwrap();

    let text = response.to_string();
    assert!(
        text.contains("typescript-language-server"),
        "callers must fail naming the server binary: {text}"
    );
    assert!(
        !path.path().join("ran").exists(),
        "a missing server must not be installed: {text}"
    );
}
