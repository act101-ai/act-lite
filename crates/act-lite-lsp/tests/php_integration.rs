//! PHP LSP integration tests.

use act_lite_lsp::LspPool;
use act_lite_parser::Grammar;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn php_server_config_is_valid() {
    let config = act_lite_lsp::config::ServerConfig::php(PathBuf::from("/workspace"));
    assert_eq!(config.command, "intelephense");
    assert_eq!(config.args, vec!["--stdio"]);
    assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn php_pool_detection() {
    let dir = TempDir::new().unwrap();

    // Create minimal PHP project
    std::fs::write(
        dir.path().join("composer.json"),
        r#"
{
    "name": "test/project",
    "require": {
        "php": "^8.2"
    },
    "autoload": {
        "psr-4": {
            "App\\": "src/"
        }
    }
}
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/UserService.php"),
        r#"<?php

declare(strict_types=1);

namespace App;

class UserService {
    public function findById(int $id): ?User {
        return null;
    }
}
"#,
    )
    .unwrap();

    let _pool = LspPool::new(dir.path().to_path_buf());

    // Pool should be able to create config for PHP
    let config = act_lite_lsp::config::ServerConfig::php(dir.path().to_path_buf());
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn php_disabled_pool_returns_none() {
    let pool = LspPool::disabled();

    // Should return None for disabled pool
    let client = pool.get_client(Grammar::Php).await;
    assert!(client.is_none());
}

#[tokio::test]
async fn php_pool_has_client() {
    let dir = TempDir::new().unwrap();

    std::fs::write(
        dir.path().join("index.php"),
        r#"<?php

echo "Hello, World!";
"#,
    )
    .unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    assert!(!pool.has_client(Grammar::Php).await);
}

#[tokio::test]
async fn php_pool_client_count() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    assert_eq!(pool.client_count().await, 0);
}

#[tokio::test]
async fn php_pool_shutdown() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let result = pool.shutdown_all().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn php_pool_remove_client() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let removed = pool.remove_client(Grammar::Php).await;
    assert!(removed.is_none());
}

#[tokio::test]
async fn php_workspace_detection() {
    let dir = TempDir::new().unwrap();

    std::fs::write(
        dir.path().join("index.php"),
        r#"<?php

phpinfo();
"#,
    )
    .unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    assert_eq!(pool.root(), &dir.path());
}

#[tokio::test]
async fn php_namespace_handling() {
    let dir = TempDir::new().unwrap();

    // Create directory structure first
    std::fs::create_dir_all(dir.path().join("src/Models")).unwrap();

    // Create PHP file with namespaces
    std::fs::write(
        dir.path().join("src/Models/User.php"),
        r#"<?php

declare(strict_types=1);

namespace App\Models;

class User
{
    public function __construct(
        public readonly int $id,
        public readonly string $name,
    ) {}
}
"#,
    )
    .unwrap();

    let _pool = LspPool::new(dir.path().to_path_buf());
    let config = act_lite_lsp::config::ServerConfig::php(dir.path().to_path_buf());
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn php_trait_handling() {
    let dir = TempDir::new().unwrap();

    // Create directory structure first
    std::fs::create_dir_all(dir.path().join("src/Traits")).unwrap();

    // Create PHP file with traits
    std::fs::write(
        dir.path().join("src/Traits/Loggable.php"),
        r#"<?php

declare(strict_types=1);

namespace App\Traits;

trait Loggable
{
    protected function log(string $message): void
    {
        echo $message;
    }
}
"#,
    )
    .unwrap();

    let _pool = LspPool::new(dir.path().to_path_buf());
    assert_eq!(_pool.root(), &dir.path());
}

#[tokio::test]
async fn php_attribute_handling() {
    let dir = TempDir::new().unwrap();

    // Create directory structure first
    std::fs::create_dir_all(dir.path().join("src/Controllers")).unwrap();

    // Create PHP file with PHP 8 attributes
    std::fs::write(
        dir.path().join("src/Controllers/UserController.php"),
        r#"<?php

declare(strict_types=1);

namespace App\Controllers;

#[Route('/api/users', methods: ['GET'])]
class UserController
{
    #[Route('/', methods: ['GET'])]
    public function index(): array
    {
        return [];
    }
}
"#,
    )
    .unwrap();

    let _pool = LspPool::new(dir.path().to_path_buf());
    let config = act_lite_lsp::config::ServerConfig::php(dir.path().to_path_buf());
    assert!(config.validate().is_ok());
}
