//! Kotlin LSP integration tests.

use act_lite_lsp::LspPool;
use act_lite_parser::Grammar;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn kotlin_server_config_is_valid() {
    let config = act_lite_lsp::config::ServerConfig::kotlin(PathBuf::from("/workspace"));
    assert_eq!(config.command, "kotlin-language-server");
    assert_eq!(config.args, Vec::<String>::new());
    assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn kotlin_pool_detection() {
    let dir = TempDir::new().unwrap();

    // Create minimal Kotlin project
    std::fs::write(
        dir.path().join("build.gradle.kts"),
        r#"
plugins {
    kotlin("jvm") version "1.9.0"
}
"#,
    )
    .unwrap();

    std::fs::write(
        dir.path().join("Main.kt"),
        r#"
fun main() {
    println("Hello, World!")
}
"#,
    )
    .unwrap();

    let _pool = LspPool::new(dir.path().to_path_buf());

    // Pool should be able to create config for Kotlin
    let config = act_lite_lsp::config::ServerConfig::kotlin(dir.path().to_path_buf());
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn kotlin_disabled_pool_returns_none() {
    let pool = LspPool::disabled();

    // Should return None for disabled pool
    let client = pool.get_client(Grammar::Kotlin).await;
    assert!(client.is_none());
}

#[tokio::test]
async fn kotlin_pool_has_client() {
    let dir = TempDir::new().unwrap();

    std::fs::write(dir.path().join("Main.kt"), "x = 42").unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    assert!(!pool.has_client(Grammar::Kotlin).await);
}

#[tokio::test]
async fn kotlin_pool_client_count() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    assert_eq!(pool.client_count().await, 0);
}

#[tokio::test]
async fn kotlin_pool_shutdown() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let result = pool.shutdown_all().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn kotlin_pool_remove_client() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let removed = pool.remove_client(Grammar::Kotlin).await;
    assert!(removed.is_none());
}

#[tokio::test]
async fn kotlin_workspace_detection() {
    let dir = TempDir::new().unwrap();

    std::fs::write(dir.path().join("Main.kt"), "").unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    assert_eq!(pool.root(), &dir.path());
}
