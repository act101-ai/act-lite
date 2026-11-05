//! Go LSP integration tests.

use act_lite_lsp::{LspPool, config::ServerConfig};
use act_lite_parser::Grammar;
use tempfile::TempDir;

#[tokio::test]
async fn gopls_server_config_is_valid() {
    let config = ServerConfig::go(std::path::PathBuf::from("/workspace"));
    assert_eq!(config.command, "gopls");
    assert_eq!(config.args, vec!["serve"]);
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn go_pool_returns_some_or_none() {
    // This test verifies graceful behavior when gopls may or may not be installed
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

    let pool = LspPool::new(dir.path().to_path_buf());
    let client = pool.get_client(Grammar::Go).await;

    // Result depends on whether gopls is installed
    // We just verify it doesn't panic
    match client {
        Some(_) => {
            // gopls is available - this is fine
        }
        None => {
            // gopls not installed - this is expected in some CI environments
        }
    }
}

#[tokio::test]
async fn go_disabled_pool_returns_none() {
    let pool = LspPool::disabled();

    // Should return None for disabled pool
    let client = pool.get_client(Grammar::Go).await;
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

    let pool = LspPool::new(dir.path().to_path_buf());

    // Getting client should succeed (may return Some if gopls is available)
    let _client = pool.get_client(Grammar::Go).await;
    // We just verify it doesn't crash
}
