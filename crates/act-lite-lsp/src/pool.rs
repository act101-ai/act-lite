//! LSP server pool management.

use crate::client::{LspClient, LspClientTrait};
use crate::config::ServerConfig;
use crate::error::Result;
use crate::servers::LspServer;
use act_lite_parser::Grammar;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

pub struct LspPool {
    /// Active LSP clients, keyed by the server binary backing them.
    clients: Arc<RwLock<HashMap<LspServer, Arc<LspClient>>>>,

    /// Servers whose LSP install or spawn failed; never retried within this process lifetime.
    failed_installs: Arc<RwLock<HashSet<LspServer>>>,

    /// Workspace root directory.
    workspace_root: PathBuf,

    /// Whether LSP is disabled globally.
    disabled: bool,

    shut_down: Arc<std::sync::atomic::AtomicBool>,
}

impl LspPool {
    /// Creates a new LSP pool for the given workspace.
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            failed_installs: Arc::new(RwLock::new(HashSet::new())),
            workspace_root,
            disabled: false,
            shut_down: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Creates a new LSP pool with LSP disabled.
    pub fn disabled() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            failed_installs: Arc::new(RwLock::new(HashSet::new())),
            workspace_root: PathBuf::new(),
            disabled: true,
            shut_down: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Gets or creates an LSP client for the given grammar.
    ///
    /// Returns `None` if LSP is disabled or the server fails to spawn.
    /// Starts servers for `grammars` that are already installed, concurrently,
    /// without ever triggering an install.
    ///
    /// Warming exists so the first real query does not pay the cold start. It
    /// must not become a cost of its own: `get_client` runs the installer, and
    /// warming every detected language serially through it starves the query it
    /// was meant to accelerate — a `rename` sat past the client's 120s patience
    /// and the agent hand-edited fifty sites instead.
    pub fn warm(self: &Arc<Self>, grammars: impl IntoIterator<Item = Grammar>) {
        if self.disabled {
            return;
        }
        let mut seen = HashSet::new();
        for grammar in grammars {
            let Some(server) = LspServer::for_grammar(grammar) else {
                continue;
            };
            if !seen.insert(server) {
                continue;
            }
            let config = server.config(self.workspace_root.clone());
            if which_on_path(&config.command).is_none() {
                debug!(
                    "not warming {:?}: {} is not installed",
                    server, config.command
                );
                continue;
            }
            let pool = Arc::clone(self);
            tokio::spawn(async move {
                let _ = pool.get_client(grammar).await;
            });
        }
    }

    /// Like [`get_client`](Self::get_client), but never installs: answers
    /// `None` when the server binary is not already on PATH.
    ///
    /// This is the accessor for warming, and it applies the same guard
    /// [`warm`](Self::warm) does, for the reason given there — `get_client`
    /// runs `installer::ensure_installed`, which shells out to a package
    /// manager with no timeout. Warming happens while the MCP server is coming
    /// up, so an install reached from here is an unbounded network operation on
    /// a startup path, on every launch, for a server the user never asked to
    /// install.
    pub async fn get_installed_client(&self, grammar: Grammar) -> Option<Arc<LspClient>> {
        if self.disabled {
            return None;
        }
        let server = LspServer::for_grammar(grammar)?;
        {
            let clients = self.clients.read().await;
            if let Some(client) = clients.get(&server) {
                return Some(Arc::clone(client));
            }
        }
        which_on_path(&server.config(self.workspace_root.clone()).command)?;
        self.get_client(grammar).await
    }

    /// The client for a query: [`get_installed_client`](Self::get_installed_client),
    /// or, when `ACT_LITE_INSTALL_LSP=1`, [`get_client`](Self::get_client),
    /// which installs a missing server first.
    pub async fn get_query_client(&self, grammar: Grammar) -> Option<Arc<LspClient>> {
        if crate::installer::install_opted_in() {
            self.get_client(grammar).await
        } else {
            self.get_installed_client(grammar).await
        }
    }

    pub async fn get_client(&self, grammar: Grammar) -> Option<Arc<LspClient>> {
        if self.disabled || self.is_shut_down() {
            return None;
        }

        let server = LspServer::for_grammar(grammar)?;

        // Check if we already have a client
        {
            let clients = self.clients.read().await;
            if let Some(client) = clients.get(&server) {
                return Some(Arc::clone(client));
            }
        }

        // Skip servers whose install already failed in this process lifetime.
        {
            let failed = self.failed_installs.read().await;
            if failed.contains(&server) {
                return None;
            }
        }

        // Create a new client
        debug!("Creating new LSP client for {:?}", server);

        let config = server.config(self.workspace_root.clone());
        self.spawn_and_initialize(server, config).await
    }

    async fn spawn_and_initialize(
        &self,
        server: LspServer,
        config: ServerConfig,
    ) -> Option<Arc<LspClient>> {
        let language = server.binary_name().to_string();

        // Ensure the binary is installed before attempting to spawn
        if let Err(e) = crate::installer::ensure_installed(
            &config,
            &language,
            crate::installer::install_opted_in(),
        )
        .await
        {
            warn!("LSP binary not available for {:?}: {}", server, e);
            self.failed_installs.write().await.insert(server);
            return None;
        }

        match LspClient::spawn(language, config).await {
            Ok(client) => {
                // Initialize the client
                if let Err(e) = client.initialize(self.workspace_root.clone()).await {
                    warn!("Failed to initialize LSP client for {:?}: {}", server, e);
                    // Record the failure: a server that spawns but cannot
                    // initialize would otherwise be respawned (and killed)
                    // on every get_client call.
                    self.failed_installs.write().await.insert(server);
                    return None;
                }

                let client = Arc::new(client);

                // Store the client
                let mut clients = self.clients.write().await;
                clients.insert(server, Arc::clone(&client));

                Some(client)
            }
            Err(e) => {
                warn!("Failed to spawn LSP server for {:?}: {}", server, e);
                self.failed_installs.write().await.insert(server);
                None
            }
        }
    }

    /// Gets the server configuration for a grammar.
    ///
    /// Backed by the positive table in [`crate::servers::LspServer`]: only
    /// server-backed grammars are listed there; every other grammar returns
    /// `None` without being enumerated.
    ///
    /// Kept as the single expression of "config for a grammar" that the parity
    /// tests pin; the hot paths resolve `LspServer` themselves because they
    /// need the server as a pool key, not just its config.
    #[cfg(test)]
    fn get_server_config(&self, grammar: Grammar) -> Option<ServerConfig> {
        LspServer::for_grammar(grammar).map(|server| server.config(self.workspace_root.clone()))
    }

    /// Whether [`shutdown_all`](Self::shutdown_all) has run on this pool.
    ///
    /// A shut-down pool hands out no further clients; see the `shut_down`
    /// field for why this is terminal rather than `Drop`-driven.
    pub fn is_shut_down(&self) -> bool {
        self.shut_down.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Shuts down all LSP clients in the pool.
    ///
    /// Terminal: the pool spawns nothing after this returns.
    pub async fn shutdown_all(&self) -> Result<()> {
        self.shut_down
            .store(true, std::sync::atomic::Ordering::Release);
        let mut clients = self.clients.write().await;

        for (server, client) in clients.drain() {
            debug!("Shutting down LSP client for {:?}", server);
            if let Err(e) = client.shutdown().await {
                warn!("Error shutting down {:?} client: {}", server, e);
            }
        }

        Ok(())
    }

    /// Checks if a client exists for the server backing the given grammar.
    pub async fn has_client(&self, grammar: Grammar) -> bool {
        let Some(server) = LspServer::for_grammar(grammar) else {
            return false;
        };
        self.clients.read().await.contains_key(&server)
    }

    /// Removes a client from the pool (e.g., after a crash).
    ///
    /// Removes the entry for the SERVER backing `grammar`, which is shared with
    /// every sibling grammar of that server — a crashed `clangd` is gone for C
    /// and C++ alike, because it was one process for both.
    pub async fn remove_client(&self, grammar: Grammar) -> Option<Arc<LspClient>> {
        let server = LspServer::for_grammar(grammar)?;
        self.clients.write().await.remove(&server)
    }

    /// Returns the number of active clients.
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Returns the workspace root directory.
    pub fn root(&self) -> &PathBuf {
        &self.workspace_root
    }

    /// Returns whether LSP is globally disabled for this pool.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

impl Drop for LspPool {
    fn drop(&mut self) {
        // Only attempt to shut down if LSP is enabled
        if self.disabled {
            return;
        }

        // Attempt to shut down all clients on a dedicated thread to avoid
        // blocking the async executor or panicking in a destructor context.
        // A short timeout prevents hanging when LSP servers are unresponsive.
        // We wait for the shutdown thread to complete (with timeout) to ensure
        // LSP clients are properly shut down before the process exits.
        let clients = Arc::clone(&self.clients);
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                rt.block_on(async move {
                    let mut clients = clients.write().await;
                    for (_grammar, client) in clients.drain() {
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_secs(2),
                            client.shutdown(),
                        )
                        .await;
                    }
                });
            }
            let _ = tx.send(());
        });

        // Wait for shutdown thread to complete with a 5-second timeout.
        // This ensures LSP clients are shut down before the process exits.
        let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
    }
}

fn which_on_path(command: &str) -> Option<std::path::PathBuf> {
    which::which(command).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::servers::LspServer;

    #[tokio::test]
    async fn test_pool_creation() {
        let pool = LspPool::new(PathBuf::from("/workspace"));
        assert_eq!(pool.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_disabled_pool() {
        let pool = LspPool::disabled();
        assert!(pool.is_disabled());
        assert_eq!(pool.client_count().await, 0);

        // Should return None for disabled pool
        let client = pool.get_client(Grammar::TypeScript).await;
        assert!(client.is_none());
    }

    /// Warming must never shell out to a package manager.
    ///
    /// `get_client` runs `installer::ensure_installed`, which executes the
    /// grammar's install command (`npm install -g typescript-language-server
    /// typescript`, `brew install …`) through `sh` with no timeout. The MCP
    /// server warms every detected language while starting up, so reaching that
    /// path from a warm turns each launch into an unbounded install — the exact
    /// hazard [`LspPool::warm`] guards against with its PATH check.
    ///
    /// With nothing on PATH, `get_installed_client` must answer `None` and
    /// leave `failed_installs` untouched. That set is the marker: only
    /// `spawn_and_initialize` writes to it, and it cannot be reached without
    /// first running the installer.
    #[tokio::test]
    async fn get_installed_client_never_runs_the_installer() {
        let empty = tempfile::tempdir().unwrap();
        let _path = PathGuard::set(empty.path());

        let pool = LspPool::new(PathBuf::from("/workspace"));
        let client = pool.get_installed_client(Grammar::TypeScript).await;

        assert!(
            client.is_none(),
            "no server on PATH, so there is nothing to warm"
        );
        assert!(
            !pool
                .failed_installs
                .read()
                .await
                .contains(&LspServer::TypeScriptLanguageServer),
            "the installer ran: warming must not attempt an install"
        );
    }

    /// Serializes the `PATH` mutation in the test above against the rest of this
    /// binary's tests, and restores the previous value on drop.
    struct PathGuard {
        previous: Option<std::ffi::OsString>,
        /// Held, not read: it is the exclusion itself, released on drop.
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    static PATH_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    impl PathGuard {
        fn set(dir: &std::path::Path) -> Self {
            let lock = PATH_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
            let previous = std::env::var_os("PATH");
            // SAFETY: the only PATH writer in this binary, serialized by the lock.
            unsafe { std::env::set_var("PATH", dir) };
            Self {
                previous,
                _lock: lock,
            }
        }
    }

    impl Drop for PathGuard {
        fn drop(&mut self) {
            // SAFETY: as above — still holding the lock.
            unsafe {
                match self.previous.take() {
                    Some(previous) => std::env::set_var("PATH", previous),
                    None => std::env::remove_var("PATH"),
                }
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn which_on_path_rejects_a_non_executable_path_entry() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let planted = temp.path().join("typescript-language-server");
        std::fs::write(&planted, "not a real server\n").unwrap();
        std::fs::set_permissions(&planted, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _path = PathGuard::set(temp.path());

        assert!(
            which_on_path("typescript-language-server").is_none(),
            "a non-executable PATH entry must not count as installed"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn get_installed_client_does_not_install_for_a_non_executable_path_entry() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let planted = temp.path().join("typescript-language-server");
        std::fs::write(&planted, "not a real server\n").unwrap();
        std::fs::set_permissions(&planted, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _path = PathGuard::set(temp.path());

        let pool = LspPool::new(PathBuf::from("/workspace"));
        let client = pool.get_installed_client(Grammar::TypeScript).await;

        assert!(
            client.is_none(),
            "a non-executable PATH entry is not a usable server"
        );
        assert!(
            !pool
                .failed_installs
                .read()
                .await
                .contains(&LspServer::TypeScriptLanguageServer),
            "the installer ran: warming must not attempt an install"
        );
    }

    #[cfg(unix)]
    #[test]
    fn which_on_path_accepts_an_executable_path_entry() {
        let temp = tempfile::tempdir().unwrap();
        let planted = temp.path().join("typescript-language-server");
        crate::executable_script::write_executable(&planted, "#!/bin/sh\nexit 0\n");
        let _path = PathGuard::set(temp.path());

        assert!(
            which_on_path("typescript-language-server").is_some(),
            "an executable PATH entry must count as installed"
        );
    }

    #[tokio::test]
    async fn test_has_client() {
        let pool = LspPool::new(PathBuf::from("/workspace"));
        assert!(!pool.has_client(Grammar::TypeScript).await);
    }

    #[test]
    fn test_pool_root_accessor() {
        let workspace = PathBuf::from("/my/project");
        let pool = LspPool::new(workspace.clone());
        assert_eq!(pool.root(), &workspace);

        let pool = LspPool::disabled();
        assert_eq!(pool.root(), &PathBuf::new());
    }

    #[tokio::test]
    async fn test_remove_client_from_empty_pool() {
        let pool = LspPool::new(PathBuf::from("/workspace"));
        let removed = pool.remove_client(Grammar::TypeScript).await;
        assert!(removed.is_none());
    }

    #[tokio::test]
    async fn test_shutdown_all_empty_pool() {
        let pool = LspPool::new(PathBuf::from("/workspace"));
        let result = pool.shutdown_all().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_server_config_typescript() {
        let pool = LspPool::new(PathBuf::from("/workspace"));
        let config = pool.get_server_config(Grammar::TypeScript);
        assert!(config.is_some());
        let config = config.unwrap();
        assert!(
            config.command.contains("typescript-language-server") || config.command.contains("npx")
        );
    }

    /// Parity pin for `get_server_config` over EVERY `Grammar` variant.
    ///
    /// The 26 server-backed grammars (17 server groups) must return a config
    /// spawning the expected binary with the pool's workspace root; every
    /// other grammar must return `None`. Iterating `Grammar::all()` means a
    /// newly added grammar is automatically covered — a missing arm for a
    /// serverless grammar cannot recur silently.
    #[test]
    fn test_get_server_config_parity_all_grammars() {
        let root = PathBuf::from("/parity-test");
        let pool = LspPool::new(root.clone());

        // Grammar → expected server binary (ServerConfig::command).
        let server_backed: &[(Grammar, &str)] = &[
            (Grammar::TypeScript, "typescript-language-server"),
            (Grammar::Tsx, "typescript-language-server"),
            (Grammar::Jsx, "typescript-language-server"),
            (Grammar::JavaScript, "typescript-language-server"),
            (Grammar::CSharp, "OmniSharp"),
            (Grammar::Python, "pyright-langserver"),
            (Grammar::Rust, "rust-analyzer"),
            (Grammar::Go, "gopls"),
            (Grammar::Elixir, "elixir-ls"),
            (Grammar::Java, "jdtls"),
            (Grammar::Kotlin, "kotlin-language-server"),
            (Grammar::C, "clangd"),
            (Grammar::Cpp, "clangd"),
            (Grammar::Ruby, "ruby-lsp"),
            (Grammar::Sql, "sqls"),
            (Grammar::Php, "intelephense"),
            (Grammar::Scala, "metals"),
            (Grammar::Swift, "sourcekit-lsp"),
            (Grammar::Dart, "dart"),
        ];

        for grammar in Grammar::all() {
            let config = pool.get_server_config(grammar);
            match server_backed.iter().find(|(g, _)| *g == grammar) {
                Some((_, expected_command)) => {
                    let config = config.unwrap_or_else(|| {
                        panic!("{grammar:?} is server-backed but get_server_config returned None")
                    });
                    assert_eq!(
                        config.command, *expected_command,
                        "{grammar:?} spawns the wrong LSP server binary"
                    );
                    assert_eq!(
                        config.working_directory.as_deref(),
                        Some(root.as_path()),
                        "{grammar:?} config must use the pool's workspace root"
                    );
                }
                None => {
                    assert!(
                        config.is_none(),
                        "{grammar:?} has no configured LSP server; get_server_config must return None"
                    );
                }
            }
        }
    }

    /// Design-property regression: a grammar absent from the positive server
    /// table returns `None` without needing an explicit arm.
    #[test]
    fn test_serverless_grammar_needs_no_table_entry() {
        let pool = LspPool::new(PathBuf::from("/parity-test"));
        assert!(pool.get_server_config(Grammar::Yaml).is_none());
        assert!(pool.get_server_config(Grammar::Markdown).is_none());
    }

    #[tokio::test]
    async fn test_disabled_pool_has_client() {
        let pool = LspPool::disabled();
        assert!(!pool.has_client(Grammar::TypeScript).await);
    }

    #[tokio::test]
    async fn test_disabled_pool_shutdown_all() {
        let pool = LspPool::disabled();
        let result = pool.shutdown_all().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_disabled_pool_remove_client() {
        let pool = LspPool::disabled();
        let removed = pool.remove_client(Grammar::TypeScript).await;
        assert!(removed.is_none());
    }

    /// A grammar recorded in `failed_installs` is never re-attempted:
    /// `get_client` returns `None` before any install/spawn work, so no
    /// client ever appears in the pool.
    #[tokio::test]
    async fn test_recorded_failure_skips_respawn() {
        let pool = LspPool::new(PathBuf::from("/workspace"));
        pool.failed_installs.write().await.insert(LspServer::Metals);

        assert!(pool.get_client(Grammar::Scala).await.is_none());
        assert_eq!(pool.client_count().await, 0);
    }

    /// A grammar with no configured LSP server yields `None` from
    /// `get_client` without recording a failed install — there is nothing
    /// to install for it.
    #[tokio::test]
    async fn test_get_client_serverless_grammar_returns_none() {
        let pool = LspPool::new(PathBuf::from("/workspace"));

        assert!(pool.get_client(Grammar::Markdown).await.is_none());
        // Markdown resolves to no `LspServer` at all, so nothing can be
        // recorded against it — the set must stay empty.
        assert!(
            pool.failed_installs.read().await.is_empty(),
            "a serverless grammar must record no install failure"
        );
        assert_eq!(pool.client_count().await, 0);
    }

    /// A server that spawns successfully but fails to initialize must be
    /// recorded in `failed_installs` so subsequent `get_client` calls return
    /// `None` immediately instead of respawning (and re-killing) the broken
    /// server on every call.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_initialize_failure_recorded_in_failed_installs() {
        // A fake "sqls" binary: spawns fine, then exits without ever speaking
        // LSP, so `initialize` fails. The file name must be in the allowed
        // command list ("sqls") for `LspClient::spawn` validation to pass.
        let temp = tempfile::tempdir().unwrap();
        let fake_server = temp.path().join("sqls");
        crate::executable_script::write_executable(&fake_server, "#!/bin/sh\nexit 0\n");

        let config = ServerConfig::new(fake_server.to_string_lossy().into_owned(), vec![])
            .with_working_directory(temp.path().to_path_buf())
            .with_timeout(250);

        let pool = LspPool::new(temp.path().to_path_buf());
        let client = pool.spawn_and_initialize(LspServer::Sqls, config).await;
        assert!(
            client.is_none(),
            "initialize against a dead server must fail"
        );

        // The initialize failure must be recorded...
        assert!(
            pool.failed_installs.read().await.contains(&LspServer::Sqls),
            "initialize failure must be recorded in failed_installs"
        );

        // ...so the next get_client returns None up front instead of respawning.
        assert!(pool.get_client(Grammar::Sql).await.is_none());
        assert_eq!(
            pool.client_count().await,
            0,
            "no client may be pooled after an initialize failure"
        );
    }

    /// Spawns a fake server binary named `name` that accepts a spawn but never
    /// speaks LSP. Enough to obtain a real `LspClient` for keying tests, which
    /// must not depend on a language server being installed.
    #[cfg(unix)]
    async fn fake_client(temp: &std::path::Path, name: &str) -> Arc<LspClient> {
        let fake_server = temp.join(name);
        crate::executable_script::write_executable(&fake_server, "#!/bin/sh\nsleep 30\n");
        let config = ServerConfig::new(fake_server.to_string_lossy().into_owned(), vec![])
            .with_working_directory(temp.to_path_buf())
            .with_timeout(250);
        Arc::new(
            LspClient::spawn(name.to_string(), config)
                .await
                .expect("fake server must spawn"),
        )
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_grammars_sharing_a_server_share_one_client() {
        let temp = tempfile::tempdir().unwrap();
        let pool = LspPool::new(temp.path().to_path_buf());

        let client = fake_client(temp.path(), "typescript-language-server").await;
        pool.clients
            .write()
            .await
            .insert(LspServer::for_grammar(Grammar::TypeScript).unwrap(), client);

        for peer in [Grammar::Tsx, Grammar::Jsx, Grammar::JavaScript] {
            assert!(
                pool.has_client(peer).await,
                "{peer:?} shares typescript-language-server with TypeScript \
                 and must resolve to the already-pooled client, not spawn its own"
            );
        }
        assert_eq!(
            pool.client_count().await,
            1,
            "four grammars backed by one binary must hold exactly one client"
        );
    }

    #[tokio::test]
    async fn test_failure_suppression_covers_every_grammar_of_that_server() {
        let pool = LspPool::new(PathBuf::from("/workspace"));
        pool.failed_installs
            .write()
            .await
            .insert(LspServer::for_grammar(Grammar::C).unwrap());

        assert!(
            pool.get_client(Grammar::Cpp).await.is_none(),
            "Cpp shares clangd with C; a recorded clangd failure must suppress it too"
        );
        assert_eq!(pool.client_count().await, 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_shutdown_all_is_terminal_despite_outstanding_clones() {
        let temp = tempfile::tempdir().unwrap();
        let pool = Arc::new(LspPool::new(temp.path().to_path_buf()));

        let client = fake_client(temp.path(), "typescript-language-server").await;
        pool.clients
            .write()
            .await
            .insert(LspServer::TypeScriptLanguageServer, client);

        // Exactly what `ServerComponents::graph_builder` does — this clone is
        // why `Drop` never ran on the outgoing pool.
        let held_elsewhere = Arc::clone(&pool);

        pool.shutdown_all().await.unwrap();

        assert_eq!(
            held_elsewhere.client_count().await,
            0,
            "shutdown must drain the pool even while other Arc clones are live"
        );
        assert!(
            pool.is_shut_down(),
            "a pool that has been shut down must report it"
        );
        assert!(
            pool.get_client(Grammar::TypeScript).await.is_none(),
            "a shut-down pool must not spawn a replacement server nobody owns"
        );
        assert_eq!(
            pool.client_count().await,
            0,
            "no client may be pooled after shutdown"
        );
    }

    #[test]
    fn test_is_disabled_accessor() {
        let pool = LspPool::new(std::env::temp_dir());
        assert!(!pool.is_disabled());

        let pool = LspPool::disabled();
        assert!(pool.is_disabled());
    }

    #[tokio::test]
    async fn test_pool_shutdown_all_idempotent() {
        let pool = LspPool::new(PathBuf::from("/workspace"));

        // Calling shutdown_all multiple times should be safe
        assert!(pool.shutdown_all().await.is_ok());
        assert!(pool.shutdown_all().await.is_ok());
    }
}
