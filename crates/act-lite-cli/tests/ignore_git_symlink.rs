//! A workspace whose `.git` is a symlink to a git directory elsewhere is a
//! work-tree top, as git treats it. The analyzers then apply only the ignore
//! files of the workspace: an ignore file outside the root must not change
//! the result.

#![cfg(unix)]

use act_lite_cli::mcp::ActLiteServer;
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

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

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A function with 6 parameters: `analyze_patterns` reports it.
const REPORTED: &str = "fn a(p1: u8, p2: u8, p3: u8,\n     p4: u8, p5: u8, p6: u8) -> u8 {\n    let s = p1 + p2;\n    let t = p3 + p4;\n    let u = p5 + p6;\n    s + t\n        + u\n}\n";

/// `analyze_patterns` over the whole workspace: the result text, or the error.
async fn patterns(ws: &Path) -> String {
    let client = connect(ws).await;
    let outcome = client
        .call_tool(
            CallToolRequestParams::new("analyze_patterns")
                .with_arguments(json!({}).as_object().unwrap().clone()),
        )
        .await;
    let _ = client.cancel().await;
    match outcome {
        Ok(result) => result.content[0].as_text().unwrap().text.clone(),
        Err(e) => format!("ERROR: {e}"),
    }
}

/// `outer/` is a work tree whose `.gitignore` ignores `src/`. The workspace
/// `outer/ws` has `.git` as a symlink to the git directory of another
/// repository, `gitdir/`, whose `info/exclude` also ignores `src/`.
fn layout(w: &Path) -> std::path::PathBuf {
    let outer = w.join("outer");
    fs::create_dir_all(outer.join(".git")).unwrap();
    write(&outer.join(".gitignore"), "src/\n");
    let gitdir = w.join("gitdir");
    write(&gitdir.join("info/exclude"), "src/\n");
    let ws = outer.join("ws");
    write(&ws.join("src/lib.rs"), REPORTED);
    std::os::unix::fs::symlink(&gitdir, ws.join(".git")).unwrap();
    ws
}

#[tokio::test]
async fn an_outside_gitignore_does_not_change_the_result() {
    let w = tempfile::tempdir().unwrap();
    assert!(
        w.path().ancestors().all(|d| !d.join(".git").exists()),
        "the temporary directory is inside a git work tree"
    );
    let ws = layout(w.path());
    let with_outer = patterns(&ws).await;

    // The same workspace with the outside ignore files removed.
    fs::remove_file(w.path().join("outer/.gitignore")).unwrap();
    fs::remove_file(w.path().join("gitdir/info/exclude")).unwrap();
    let without_outer = patterns(&ws).await;

    assert!(
        without_outer.contains("src/lib.rs"),
        "control: src/lib.rs must be analyzed: {without_outer}"
    );
    assert_eq!(
        with_outer, without_outer,
        "ignore files outside the workspace root changed the analyzer result"
    );
}

/// The root's own `.gitignore` applies when `.git` is a symlink.
#[tokio::test]
async fn the_root_gitignore_applies_when_git_is_a_symlink() {
    let w = tempfile::tempdir().unwrap();
    assert!(w.path().ancestors().all(|d| !d.join(".git").exists()));
    let gitdir = w.path().join("gitdir");
    fs::create_dir_all(&gitdir).unwrap();
    let ws = w.path().join("ws");
    write(&ws.join("src/lib.rs"), REPORTED);
    write(&ws.join("build-out/x.rs"), REPORTED);
    write(&ws.join(".gitignore"), "build-out/\n");
    std::os::unix::fs::symlink(&gitdir, ws.join(".git")).unwrap();
    let text = patterns(&ws).await;
    assert!(text.contains("src/lib.rs"), "{text}");
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        value["excluded"]["counts"]["ignored"], 1,
        "build-out/ must be ignored: {value}"
    );
}

/// The `info/exclude` of the symlink target outside the root is not read.
#[tokio::test]
async fn the_info_exclude_of_an_outside_git_directory_is_not_read() {
    let w = tempfile::tempdir().unwrap();
    assert!(w.path().ancestors().all(|d| !d.join(".git").exists()));
    let gitdir = w.path().join("gitdir");
    write(&gitdir.join("info/exclude"), "src/\n");
    let ws = w.path().join("ws");
    write(&ws.join("src/lib.rs"), REPORTED);
    std::os::unix::fs::symlink(&gitdir, ws.join(".git")).unwrap();
    let text = patterns(&ws).await;
    assert!(text.contains("\"file\":\"src/lib.rs\""), "{text}");
}
