//! Go LSP integration tests.

use act_lite_lsp::{LspPool, config::ServerConfig};
use act_lite_parser::Grammar;
use tempfile::TempDir;

include!("common/helper.rs");

#[tokio::test]
async fn gopls_server_config_is_valid() {
    let config = ServerConfig::go(std::path::PathBuf::from("/workspace"));
    assert_eq!(config.command, "gopls");
    assert_eq!(config.args, vec!["serve"]);
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn go_pool_starts_gopls() {
    let dir = TempDir::new().unwrap();

    std::fs::write(dir.path().join("go.mod"), "module test\n\ngo 1.21\n").unwrap();

    std::fs::write(
        dir.path().join("main.go"),
        r#"
package main

func greet(name string) string {
    return "Hello, " + name
}
"#,
    )
    .unwrap();

    if !require_server("gopls") {
        return;
    }
    let pool = LspPool::new(dir.path().to_path_buf());
    let client = pool.get_installed_client(Grammar::Go).await;
    assert!(client.is_some(), "gopls did not start");
}

#[tokio::test]
async fn go_disabled_pool_returns_none() {
    let pool = LspPool::disabled();

    // Should return None for disabled pool
    let client = pool.get_installed_client(Grammar::Go).await;
    assert!(client.is_none());
}

#[tokio::test]
async fn go_workspace_detection() {
    let dir = TempDir::new().unwrap();

    std::fs::write(dir.path().join("go.mod"), "module example\n\ngo 1.21\n").unwrap();

    // Create a valid Go file
    std::fs::write(
        dir.path().join("lib.go"),
        r#"
package example

func Add(a, b int) int {
    return a + b
}
"#,
    )
    .unwrap();

    if !require_server("gopls") {
        return;
    }
    let pool = LspPool::new(dir.path().to_path_buf());

    let client = pool.get_installed_client(Grammar::Go).await;
    assert!(client.is_some(), "gopls did not start");
}
