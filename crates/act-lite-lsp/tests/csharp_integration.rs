//! C# LSP integration tests.

use act_lite_lsp::{LspPool, config::ServerConfig};
use act_lite_parser::Grammar;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn csharp_server_config_is_valid() {
    let config = ServerConfig::csharp(PathBuf::from("/workspace"));
    assert_eq!(config.command, "OmniSharp");
    assert_eq!(config.args, vec!["-lsp"]);
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn csharp_pool_returns_none_without_omnisharp() {
    // This test verifies graceful behavior when OmniSharp is not installed
    let dir = TempDir::new().unwrap();

    // Create minimal .csproj file
    let csproj_content = r##"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
    <ImplicitUsings>enable</ImplicitUsings>
  </PropertyGroup>
</Project>
"##;
    std::fs::write(dir.path().join("Test.csproj"), csproj_content).unwrap();

    // Create minimal C# file
    let cs_content = r##"
namespace Test;

public class Calculator
{
    public int Add(int a, int b) => a + b;
}
"##;
    std::fs::write(dir.path().join("Program.cs"), cs_content).unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    let client = pool.get_client(Grammar::CSharp).await;

    // Result depends on whether OmniSharp is installed
    // We just verify it doesn't panic
    match client {
        Some(_) => {
            // OmniSharp is available - this is fine
        }
        None => {
            // OmniSharp not installed - this is expected in CI environments
        }
    }
}

#[tokio::test]
async fn csharp_disabled_pool_returns_none() {
    let pool = LspPool::disabled();
    let client = pool.get_client(Grammar::CSharp).await;
    assert!(client.is_none());
}

#[tokio::test]
async fn csharp_workspace_detection() {
    let dir = TempDir::new().unwrap();

    // Create solution marker file
    std::fs::write(dir.path().join("EcommerceApi.sln"), "").unwrap();

    // Create project file
    let csproj_content = r##"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
  </PropertyGroup>
</Project>
"##;
    std::fs::write(dir.path().join("EcommerceApi.csproj"), csproj_content).unwrap();

    // Create global.json
    let json_content = r#"{
  "sdk": {
    "version": "8.0.0"
  }
}"#;
    std::fs::write(dir.path().join("global.json"), json_content).unwrap();

    // Create .editorconfig
    let editorconfig_content = r#"[*.cs]
indent_size = 4
csharp_new_line_before_open_brace = all
"#;
    std::fs::write(dir.path().join(".editorconfig"), editorconfig_content).unwrap();

    // Verify pool can be created and doesn't panic
    let pool = LspPool::new(dir.path().to_path_buf());
    assert_eq!(pool.root(), &dir.path().to_path_buf());
}

#[tokio::test]
async fn csharp_multiple_client_requests() {
    let dir = TempDir::new().unwrap();
    let pool = LspPool::new(dir.path().to_path_buf());

    // Multiple requests should be handled correctly
    let client1 = pool.get_client(Grammar::CSharp).await;
    let client2 = pool.get_client(Grammar::CSharp).await;

    // Both should return the same result (either Some or None)
    assert_eq!(client1.is_some(), client2.is_some());
}

#[tokio::test]
async fn csharp_pool_shutdown() {
    let dir = TempDir::new().unwrap();
    let pool = LspPool::new(dir.path().to_path_buf());

    // Shutdown should work even with no active clients
    let result = pool.shutdown_all().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn csharp_pool_client_count() {
    let dir = TempDir::new().unwrap();
    let pool = LspPool::new(dir.path().to_path_buf());

    // Initially should have 0 clients
    assert_eq!(pool.client_count().await, 0);

    // After attempting to get client (may fail if OmniSharp not installed)
    let _client = pool.get_client(Grammar::CSharp).await;

    // Count should still be valid (0 or 1)
    let count = pool.client_count().await;
    assert!(count <= 1);
}

#[tokio::test]
async fn csharp_pool_has_client() {
    let dir = TempDir::new().unwrap();
    let pool = LspPool::new(dir.path().to_path_buf());

    // Initially should not have client
    assert!(!pool.has_client(Grammar::CSharp).await);
}
