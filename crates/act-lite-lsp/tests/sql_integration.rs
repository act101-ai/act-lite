//! SQL LSP integration tests.

use act_lite_lsp::LspPool;
use act_lite_parser::Grammar;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn sql_server_config_is_valid() {
    let config = act_lite_lsp::config::ServerConfig::sql(PathBuf::from("/workspace"));
    assert_eq!(config.command, "sqls");
    assert_eq!(config.args, Vec::<String>::new()); // sqls uses stdio by default
    assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn sql_pool_detection() {
    let dir = TempDir::new().unwrap();

    // Create minimal SQL project with sqls config
    std::fs::write(
        dir.path().join(".sqls.yaml"),
        r#"
# sqls configuration file
# See https://github.com/sqls-server/sqls

# Connection configuration (optional for basic operation)
# sqls requires a database connection for full functionality
# These tests verify basic LSP pool operations without a live database
connections:
  - driver: postgres
    dataSourceName: host=localhost port=5432 user=test dbname=test
"#,
    )
    .unwrap();

    std::fs::write(
        dir.path().join("schema.sql"),
        r#"
-- Users table
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    username VARCHAR(255) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL,
    full_name VARCHAR(255),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Sample query
SELECT * FROM users WHERE is_active = true;
"#,
    )
    .unwrap();

    let _pool = LspPool::new(dir.path().to_path_buf());

    // Pool should be able to create config for SQL
    let config = act_lite_lsp::config::ServerConfig::sql(dir.path().to_path_buf());
    assert!(config.validate().is_ok());
}

// SQL-30: Replace trivial pool test — assert meaningful behavior with message
#[tokio::test]
async fn sql_pool_has_client_returns_false_for_new_pool() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("query.sql"), "SELECT * FROM users;").unwrap();
    let pool = LspPool::new(dir.path().to_path_buf());
    // A newly created pool has no active clients — sqls has not been started
    assert!(
        !pool.has_client(Grammar::Sql).await,
        "newly created pool should have no SQL client before sqls starts"
    );
}

// SQL-30: Replace trivial pool test — assert meaningful behavior with message
#[tokio::test]
async fn sql_pool_client_count_is_zero_before_initialization() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    assert_eq!(
        pool.client_count().await,
        0,
        "client count should be 0 before any client is initialized"
    );
}

// SQL-30: Replace trivial disabled pool test — assert both get_client and has_client are consistent
#[tokio::test]
async fn sql_disabled_pool_returns_none_for_all_grammars() {
    let pool = LspPool::disabled();
    // Disabled pool must return None for SQL (and all grammars)
    let client = pool.get_client(Grammar::Sql).await;
    assert!(
        client.is_none(),
        "disabled pool must return None, not an uninitialized client"
    );
    // Also verify pool.has_client() is consistent
    assert!(
        !pool.has_client(Grammar::Sql).await,
        "disabled pool must report no clients"
    );
}

#[tokio::test]
async fn sql_pool_shutdown() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let result = pool.shutdown_all().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn sql_pool_remove_client() {
    let pool = LspPool::new(PathBuf::from("/workspace"));
    let removed = pool.remove_client(Grammar::Sql).await;
    assert!(removed.is_none());
}

#[tokio::test]
async fn sql_workspace_detection() {
    let dir = TempDir::new().unwrap();

    std::fs::write(dir.path().join("schema.sql"), "").unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());
    assert_eq!(pool.root(), &dir.path());
}

// SQL-31: Test actual behavior — assert config validity for multi-statement SQL workspaces
#[tokio::test]
async fn sql_multiple_statement_handling() {
    let dir = TempDir::new().unwrap();

    let content = "CREATE TABLE users (id SERIAL PRIMARY KEY);\nCREATE TABLE orders (id SERIAL PRIMARY KEY, user_id INTEGER);";
    std::fs::write(dir.path().join("schema.sql"), content).unwrap();

    // Verify the pool can be created for a workspace with multi-statement SQL files
    let pool = LspPool::new(dir.path().to_path_buf());
    assert_eq!(
        pool.root(),
        &dir.path(),
        "pool root should match workspace directory"
    );

    // Verify sqls config is valid for this workspace
    let config = act_lite_lsp::config::ServerConfig::sql(dir.path().to_path_buf());
    assert!(
        config.validate().is_ok(),
        "server config should be valid for workspace with multi-statement SQL: {:?}",
        config.validate().err()
    );
}

#[tokio::test]
async fn sql_config_with_custom_environment() {
    let dir = TempDir::new().unwrap();

    let mut config = act_lite_lsp::config::ServerConfig::sql(dir.path().to_path_buf());

    // sqls can use environment variables for database connections
    config
        .env
        .insert("SQLS_DRIVER".to_string(), "postgres".to_string());
    config.env.insert(
        "SQLS_DATASOURCE".to_string(),
        "host=localhost port=5432".to_string(),
    );

    assert_eq!(config.env.get("SQLS_DRIVER"), Some(&"postgres".to_string()));
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn sql_timeout_configuration() {
    let config = act_lite_lsp::config::ServerConfig::sql(PathBuf::from("/workspace"));

    // Default timeout should be 30 seconds (SQL queries can be slow)
    assert_eq!(config.timeout_ms, 30000);
}

// SQL-29: Assert initialization_options contains top-level `connections` key for sqls compatibility
#[tokio::test]
async fn sql_initialization_options() {
    let config = act_lite_lsp::config::ServerConfig::sql(PathBuf::from("/workspace"));

    // sqls initialization options must include the `connections` key
    let init_opts = config.initialization_options;
    assert!(
        init_opts.is_object(),
        "initialization_options must be a JSON object, got: {:?}",
        init_opts
    );
    let obj = init_opts.as_object().unwrap();
    assert!(
        obj.contains_key("connections"),
        "initialization_options must contain 'connections' key for sqls compatibility, keys: {:?}",
        obj.keys().collect::<Vec<_>>()
    );
}

// SQL-32: Verify graceful degradation when sqls binary is absent
#[tokio::test]
async fn sql_pool_returns_none_when_sqls_binary_absent() {
    // SQL-32 requirement: when sqls is not on PATH, get_client() must return
    // None rather than panicking.
    //
    // LspPool.get_client() lazily spawns the LSP server on first call. We
    // exercise that spawn path by restricting PATH to a directory that contains
    // no executables, so LspClient::spawn() cannot find `sqls`. The pool must
    // degrade gracefully and return None instead of panicking.
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("query.sql"), "SELECT 1").unwrap();

    let pool = LspPool::new(dir.path().to_path_buf());

    // Restrict PATH so sqls cannot be found anywhere. Save the original PATH
    // and restore it after the call so we do not pollute other tests.
    let old_path = std::env::var("PATH").unwrap_or_default();

    // SAFETY: This test runs in a dedicated tokio task that does not share
    // process state with other tests in ways that would race on the PATH
    // environment variable. The original PATH is captured before modification
    // and unconditionally restored before the function returns, bounding the
    // window of mutation. No other act-lite-lsp integration test modifies PATH.
    unsafe {
        // dir.path() is a real directory but contains no executables, so PATH
        // resolution for "sqls" will fail.
        std::env::set_var("PATH", dir.path());
    }

    // get_client() must return None rather than panicking when sqls is absent.
    // This exercises the Err(...) => None branch inside LspPool::get_client().
    let client = pool.get_client(Grammar::Sql).await;

    // Restore PATH before asserting so a test failure never leaves PATH broken.
    // SAFETY: Same rationale as the set_var call above.
    unsafe {
        std::env::set_var("PATH", &old_path);
    }

    assert!(
        client.is_none(),
        "get_client() should return None when sqls binary is absent, not panic"
    );
}
