//! A closed stdout pipe ends act-lite quietly: no panic, no lost error, and
//! an exit code that tells the shell the output was cut.

use std::io::Read;
#[cfg(unix)]
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// An empty directory for `PATH`, so no language server can be found or
/// installed.
fn empty_path_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// A stdout handle whose reader end is already dropped: every write into it
/// fails with `BrokenPipe`.
fn closed_stdout() -> Stdio {
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    Stdio::from(writer)
}

/// A PATH-isolated `act-lite` command run from the fixtures directory.
fn act_lite() -> Command {
    let path = empty_path_dir();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_act-lite"));
    cmd.current_dir(fixtures()).env("PATH", path.path());
    cmd
}

/// Poll `try_wait` to a 60 s deadline, then kill: a child that never exits
/// is a failure, not something to block the suite on.
fn wait(child: &mut Child, what: &str) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().expect("try_wait must not fail") {
            return status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{what} did not exit within 60 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Runs `act-lite <args>` with a closed stdout and returns (status, stderr).
fn run_closed(args: &[&str]) -> (std::process::ExitStatus, String) {
    let mut child = act_lite()
        .args(args)
        .stdout(closed_stdout())
        .stderr(Stdio::piped())
        .spawn()
        .expect("act-lite must spawn");
    // Read stderr while waiting: a pipe buffer can be smaller than the
    // usage text (the kernel shrinks new pipes under memory pressure), and a
    // child that blocks on a full stderr never exits.
    let mut stderr = child.stderr.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut err = String::new();
        stderr.read_to_string(&mut err).unwrap();
        err
    });
    let status = wait(&mut child, &format!("act-lite {args:?}"));
    (status, reader.join().unwrap())
}

#[test]
fn query_closed_exits_141_quietly() {
    let (status, stderr) = run_closed(&["query", "skeleton", "sample.ts"]);
    assert_eq!(
        status.code(),
        Some(141),
        "a cut pipe must exit 141 (128 + SIGPIPE), stderr: {stderr}"
    );
    assert!(
        stderr.is_empty(),
        "a closed pipe must not panic or report anything: {stderr}"
    );
}

#[test]
fn query_error_keeps_exit_and_stderr() {
    let (status, stderr) = run_closed(&["query", "skeleton", "missing.ts"]);
    assert_eq!(status.code(), Some(1), "the command's own error code wins");
    assert!(
        stderr.contains("\"error\""),
        "the JSON error stays on stderr: {stderr}"
    );
    assert!(!stderr.contains("panicked"), "must not panic: {stderr}");
}

#[test]
fn help_closed_exits_141() {
    let (status, stderr) = run_closed(&["--help"]);
    assert_eq!(
        status.code(),
        Some(141),
        "--help into a closed pipe must exit 141, stderr: {stderr}"
    );
}

#[test]
fn no_args_keeps_clap_exit_2() {
    let (status, stderr) = run_closed(&[]);
    assert_eq!(status.code(), Some(2), "clap's usage error code stays");
    assert!(stderr.contains("Usage"), "usage stays on stderr: {stderr}");
}

#[test]
fn version_open_exits_0() {
    // Guard: with the pipe open, --version exits 0 as before.
    let (mut reader, writer) = std::io::pipe().unwrap();
    let mut child = act_lite()
        .arg("--version")
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .spawn()
        .expect("act-lite must spawn");
    let status = wait(&mut child, "act-lite --version");
    let mut stdout = String::new();
    reader.read_to_string(&mut stdout).unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let mut err = String::new();
    stderr.read_to_string(&mut err).unwrap();
    assert_eq!(status.code(), Some(0), "stderr: {err}");
    assert!(stdout.contains("act-lite"), "stdout: {stdout}");
}

/// A client that closes the MCP server's stdout while keeping stdin open
/// ends the session: the server exits 0 instead of hanging on the dead pipe.
#[cfg(unix)]
#[test]
fn mcp_closed_stdout_ends_serve() {
    let initialize = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
    let initialized = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;

    let mut child = act_lite()
        .args(["--disable-lsp", "mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("act-lite must spawn");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    writeln!(stdin, "{initialize}").unwrap();
    writeln!(stdin, "{initialized}").unwrap();
    stdin.flush().unwrap();

    let mut reader = BufReader::new(stdout);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .expect("the initialize response must arrive");
    assert!(
        response.contains("\"id\":1"),
        "not the initialize response: {response}"
    );

    // The client leaves: drop the stdout reader, then ask for the tool list.
    // stdin stays open, so only the dead stdout can end the session.
    drop(reader);
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#).unwrap();
    stdin.flush().unwrap();

    let mut stderr = child.stderr.take().unwrap();
    let status = wait(&mut child, "act-lite mcp serve with a closed stdout");
    drop(stdin);
    let mut err = String::new();
    stderr.read_to_string(&mut err).unwrap();
    assert_eq!(status.code(), Some(0), "stderr: {err}");
    assert!(!err.contains("panicked"), "must not panic: {err}");
}
