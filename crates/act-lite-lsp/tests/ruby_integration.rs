//! Ruby LSP integration tests.

use act_lite_lsp::LspPool;
use act_lite_parser::Grammar;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn ruby_server_config_is_valid() {
    let config = act_lite_lsp::config::ServerConfig::ruby(PathBuf::from("/workspace"));
    assert_eq!(config.command, "ruby-lsp");
    assert_eq!(config.args, Vec::<String>::new()); // ruby-lsp uses stdio by default
    assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn ruby_pool_detection() {
    let dir = TempDir::new().unwrap();

    // Create minimal Ruby project
    std::fs::write(
        dir.path().join("Gemfile"),
        r#"
source 'https://rubygems.org'

gem 'ruby-lsp'
"#,
    )
    .unwrap();

    std::fs::write(
        dir.path().join("main.rb"),
        r#"
# frozen_string_literal: true

def greet(name)
  "Hello, #{name}!"
end
"#,
    )
    .unwrap();

    let _pool = LspPool::new(dir.path().to_path_buf());

    // Pool should be able to create config for Ruby
    let config = act_lite_lsp::config::ServerConfig::ruby(dir.path().to_path_buf());
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn ruby_disabled_pool_returns_none() {
    let pool = LspPool::disabled();

    // Should return None for disabled pool
    let client = pool.get_client(Grammar::Ruby).await;
    assert!(client.is_none());
}

#[tokio::test]
async fn ruby_pool_has_client() {
    let dir = TempDir::new().unwrap();

    std::fs::write(dir.path().join("main.rb"), "x = 42").unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    assert!(!pool.has_client(Grammar::Ruby).await);
}

#[tokio::test]
async fn ruby_pool_client_count() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    assert_eq!(pool.client_count().await, 0);
}

#[tokio::test]
async fn ruby_pool_shutdown() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let result = pool.shutdown_all().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn ruby_pool_remove_client() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let removed = pool.remove_client(Grammar::Ruby).await;
    assert!(removed.is_none());
}

#[tokio::test]
async fn ruby_workspace_detection() {
    let dir = TempDir::new().unwrap();

    std::fs::write(dir.path().join("main.rb"), "").unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    assert_eq!(pool.root(), &dir.path());
}
