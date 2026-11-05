//! Python LSP integration tests.

use act_lite_lsp::LspPool;
use act_lite_parser::Grammar;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn python_server_config_is_valid() {
    let config = act_lite_lsp::config::ServerConfig::python(PathBuf::from("/workspace"));
    assert_eq!(config.command, "pyright-langserver");
    assert_eq!(config.args, vec!["--stdio"]);
    assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn python_pool_detection() {
    let dir = TempDir::new().unwrap();

    // Create minimal Python project
    std::fs::write(
        dir.path().join("pyproject.toml"),
        r#"
[project]
name = "test"
version = "0.1.0"
"#,
    )
    .unwrap();

    std::fs::write(
        dir.path().join("main.py"),
        r#"
def greet(name: str) -> str:
    return f"Hello, {name}!"
"#,
    )
    .unwrap();

    let _pool = LspPool::new(dir.path().to_path_buf());

    // Pool should be able to create config for Python
    let config = act_lite_lsp::config::ServerConfig::python(dir.path().to_path_buf());
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn python_disabled_pool_returns_none() {
    let pool = LspPool::disabled();

    // Should return None for disabled pool
    let client = pool.get_client(Grammar::Python).await;
    assert!(client.is_none());
}

#[tokio::test]
async fn python_pool_has_client() {
    let dir = TempDir::new().unwrap();

    std::fs::write(dir.path().join("main.py"), "x = 42").unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    assert!(!pool.has_client(Grammar::Python).await);
}

#[tokio::test]
async fn python_pool_client_count() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    assert_eq!(pool.client_count().await, 0);
}

#[tokio::test]
async fn python_pool_shutdown() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let result = pool.shutdown_all().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn python_pool_remove_client() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let removed = pool.remove_client(Grammar::Python).await;
    assert!(removed.is_none());
}

#[tokio::test]
async fn python_workspace_detection() {
    let dir = TempDir::new().unwrap();

    std::fs::write(dir.path().join("main.py"), "").unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    assert_eq!(pool.root(), &dir.path());
}
