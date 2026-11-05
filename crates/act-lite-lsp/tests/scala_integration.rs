//! Scala (Metals) LSP integration tests.

use act_lite_lsp::LspPool;
use act_lite_parser::Grammar;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn scala_server_config_is_valid() {
    let workspace_root = PathBuf::from("/tmp/test-workspace");
    let config = act_lite_lsp::config::ServerConfig::scala(workspace_root.clone());

    assert_eq!(config.command, "metals");
    assert!(config.args.is_empty());
    assert_eq!(config.working_directory, Some(workspace_root));
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn scala_server_config_has_initialization_options() {
    let config = act_lite_lsp::config::ServerConfig::scala(PathBuf::from("/tmp/test"));

    assert!(config.initialization_options.is_object());
    let metals = &config.initialization_options["metals"];
    assert!(metals.is_object());
    assert_eq!(metals["showImplicitArguments"], true);
    assert_eq!(metals["showInferredType"], true);
}

#[tokio::test]
async fn scala_server_config_has_timeout() {
    let config = act_lite_lsp::config::ServerConfig::scala(PathBuf::from("/tmp/test"));
    assert_eq!(config.timeout_ms, 30000);
}

#[tokio::test]
async fn scala_pool_detection() {
    let dir = TempDir::new().unwrap();

    // Create minimal Scala project structure
    std::fs::write(dir.path().join("build.sbt"), r#"scalaVersion := "3.3.0""#).unwrap();
    std::fs::write(
        dir.path().join("Main.scala"),
        r#"@main def main(): Unit = println("Hello")"#,
    )
    .unwrap();

    let _pool = LspPool::new(dir.path().to_path_buf());

    // Pool should be able to create config for Scala
    let config = act_lite_lsp::config::ServerConfig::scala(dir.path().to_path_buf());
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn scala_disabled_pool_returns_none() {
    let pool = LspPool::disabled();

    // Disabled pool must return None for Scala
    let client = pool.get_installed_client(Grammar::Scala).await;
    assert!(client.is_none());
}

#[tokio::test]
async fn scala_pool_has_client_returns_false_when_not_started() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("Main.scala"), "").unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    assert!(!pool.has_client(Grammar::Scala).await);
}

#[tokio::test]
async fn scala_pool_client_count_starts_at_zero() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    assert_eq!(pool.client_count().await, 0);
}

#[tokio::test]
async fn scala_pool_shutdown_succeeds_when_empty() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let result = pool.shutdown_all().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn scala_pool_remove_client_returns_none_when_not_started() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let removed = pool.remove_client(Grammar::Scala).await;
    assert!(removed.is_none());
}

#[tokio::test]
async fn scala_workspace_detection() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("Main.scala"), "").unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    assert_eq!(pool.root(), &dir.path());
}
