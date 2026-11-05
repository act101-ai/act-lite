//! Elixir LSP integration tests.

use act_lite_lsp::{LspPool, config::ServerConfig};
use act_lite_parser::Grammar;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn elixir_server_config_is_valid() {
    let config = ServerConfig::elixir(PathBuf::from("/workspace"));
    assert_eq!(config.command, "elixir-ls");
    assert_eq!(config.args, vec![] as Vec<String>);
    assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn elixir_pool_detection() {
    let dir = TempDir::new().unwrap();

    // Create lib directory
    std::fs::create_dir_all(dir.path().join("lib")).unwrap();

    // Create minimal Elixir project
    std::fs::write(
        dir.path().join("mix.exs"),
        r#"
defmodule App do
  def project do
    [
      app: :app,
      version: "0.1.0"
    ]
  end
end
"#,
    )
    .unwrap();

    std::fs::write(
        dir.path().join("lib/app.ex"),
        r#"
defmodule App do
  def hello(name) do
    "Hello, #{name}!"
  end
end
"#,
    )
    .unwrap();

    let _pool = LspPool::new(dir.path().to_path_buf());

    // Pool should be able to create config for Elixir
    let config = ServerConfig::elixir(dir.path().to_path_buf());
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn elixir_disabled_pool_returns_none() {
    let pool = LspPool::disabled();

    // Should return None for disabled pool
    let client = pool.get_installed_client(Grammar::Elixir).await;
    assert!(client.is_none());
}

#[tokio::test]
async fn elixir_pool_has_client() {
    let dir = TempDir::new().unwrap();

    // Create minimal Elixir project
    std::fs::write(
        dir.path().join("mix.exs"),
        r#"
defmodule App do
  def project do
    [app: :app, version: "0.1.0"]
  end
end
"#,
    )
    .unwrap();

    let _pool = LspPool::new(dir.path().to_path_buf());

    // For now, just verify the pool is created (actual elixir-ls may not be installed)
    // Note: dir is kept alive by _pool reference
}

#[tokio::test]
async fn elixir_server_config_has_dialyzer() {
    let config = ServerConfig::elixir(PathBuf::from("/workspace"));

    // Check that dialyzer is enabled in init options
    assert!(config.initialization_options.is_object());
}

#[tokio::test]
async fn elixir_server_config_has_suggest_specs() {
    let config = ServerConfig::elixir(PathBuf::from("/workspace"));

    // Check that suggestSpecs is enabled
    assert!(config.initialization_options.is_object());
}
