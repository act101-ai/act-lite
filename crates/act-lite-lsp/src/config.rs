//! LSP server configuration.

use crate::error::{LspError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Allowed LSP server commands.
///
/// Only commands in this list can be spawned to prevent command injection.
/// This is a security measure to ensure that only known, safe LSP servers
/// can be executed.
const ALLOWED_COMMANDS: &[&str] = &[
    "typescript-language-server",
    "rust-analyzer",
    "pyright-langserver",
    "gopls",
    "clangd",
    "lua-language-server",
    "java", // Eclipse JDT.LS
    "jdtls",
    "kotlin-language-server",
    "ruby-lsp", // Shopify Ruby LSP
    "vscode-json-language-server",
    "vscode-css-language-server",
    "vscode-html-language-server",
    "yaml-language-server",
    "bash-language-server",
    "dockerfile-language-server-nodejs",
    "eslint-language-server",
    "taplo",         // TOML
    "OmniSharp",     // C# (cross-platform)
    "omnisharp",     // C# (lowercase variant)
    "elixir-ls",     // Elixir
    "intelephense",  // PHP (premium license available)
    "phpactor",      // PHP (open source)
    "metals",        // Scala
    "sqls",          // SQL
    "sourcekit-lsp", // Swift
    "dart",          // Dart (`dart language-server`)
];

/// Configuration for an LSP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// The command to run the LSP server (e.g., "typescript-language-server").
    pub command: String,

    /// Arguments to pass to the server command.
    pub args: Vec<String>,

    /// Working directory for the server process.
    pub working_directory: Option<PathBuf>,

    /// Environment variables to set for the server process.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Initialization options to send to the server.
    #[serde(default)]
    pub initialization_options: serde_json::Value,

    /// Request timeout in milliseconds.
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    30000 // 30 second default timeout (allows for LSP cold start)
}

impl ServerConfig {
    /// Creates a TypeScript language server configuration.
    pub fn typescript(workspace_root: PathBuf) -> Self {
        Self {
            command: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "preferences": {
                    "includeInlayParameterNameHints": "all",
                    "includeInlayParameterNameHintsWhenArgumentMatchesName": true,
                    "includeInlayFunctionParameterTypeHints": true,
                    "includeInlayVariableTypeHints": true,
                    "includeInlayPropertyDeclarationTypeHints": true,
                    "includeInlayFunctionLikeReturnTypeHints": true,
                    "includeInlayEnumMemberValueHints": true
                }
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a C# (OmniSharp) language server configuration.
    pub fn csharp(workspace_root: PathBuf) -> Self {
        Self {
            command: "OmniSharp".to_string(),
            args: vec!["-lsp".to_string()],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "RoslynExtensionsOptions": {
                    "EnableAnalyzersSupport": true,
                    "EnableImportCompletion": true,
                    "EnableDecompilationSupport": true
                },
                "FormattingOptions": {
                    "EnableEditorConfigSupport": true
                }
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a Python (Pyright) language server configuration.
    pub fn python(workspace_root: PathBuf) -> Self {
        Self {
            command: "pyright-langserver".to_string(),
            args: vec!["--stdio".to_string()],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "python": {
                    "analysis": {
                        "autoSearchPaths": true,
                        "useLibraryCodeForTypes": true,
                        "diagnosticMode": "workspace"
                    }
                }
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a Rust (rust-analyzer) language server configuration.
    pub fn rust(workspace_root: PathBuf) -> Self {
        Self {
            command: "rust-analyzer".to_string(),
            args: vec![],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "checkOnSave": {
                    "command": "clippy"
                },
                "cargo": {
                    "allFeatures": true
                }
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a Go (gopls) language server configuration.
    pub fn go(workspace_root: PathBuf) -> Self {
        Self {
            command: "gopls".to_string(),
            args: vec!["serve".to_string()],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "usePlaceholders": true,
                "completeUnimported": true
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates an Elixir (elixir-ls) language server configuration.
    pub fn elixir(workspace_root: PathBuf) -> Self {
        Self {
            command: "elixir-ls".to_string(),
            args: vec![],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "elixirLS": {
                    "dialyzerEnabled": true,
                    "fetchDeps": false,
                    "mixEnv": "test",
                    "suggestSpecs": true
                }
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a Java (jdtls) language server configuration.
    pub fn java(workspace_root: PathBuf) -> Self {
        Self {
            command: "jdtls".to_string(),
            args: vec![],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "settings": {
                    "java": {
                        "format": {
                            "enabled": true
                        },
                        "completion": {
                            "enabled": true
                        }
                    }
                }
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a Kotlin language server configuration.
    pub fn kotlin(workspace_root: PathBuf) -> Self {
        Self {
            command: "kotlin-language-server".to_string(),
            args: vec![],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "kotlin": {
                    "compiler": {
                        "jvm": {
                            "target": "17"
                        }
                    },
                    "linting": {
                        "debounceTime": 250
                    },
                    "completion": {
                        "snippets": {
                            "enabled": true
                        }
                    }
                }
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a Ruby (ruby-lsp) language server configuration.
    ///
    /// ruby-lsp is developed by Shopify and provides modern Ruby language support.
    /// Falls back to tree-sitter-only analysis if not installed.
    pub fn ruby(workspace_root: PathBuf) -> Self {
        Self {
            command: "ruby-lsp".to_string(),
            args: vec![],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "enabledFeatures": {
                    "codeActions": true,
                    "diagnostics": true,
                    "documentHighlights": true,
                    "documentLink": true,
                    "documentSymbols": true,
                    "foldingRanges": true,
                    "formatting": true,
                    "hover": true,
                    "inlayHint": true,
                    "onTypeFormatting": true,
                    "selectionRanges": true,
                    "semanticHighlighting": true,
                    "completion": true,
                    "codeLens": true,
                    "definition": true,
                    "workspaceSymbol": true,
                    "signatureHelp": true
                },
                "formatter": "auto",
                "linters": ["rubocop"]
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a PHP (intelephense) language server configuration.
    ///
    /// intelephense is a popular PHP language server with premium features available.
    /// Falls back to phpactor or tree-sitter-only analysis if not installed.
    /// See: https://intelephense.com/
    pub fn php(workspace_root: PathBuf) -> Self {
        Self {
            command: "intelephense".to_string(),
            args: vec!["--stdio".to_string()],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "storagePath": "./.intelephense",
                "clearCache": false,
                "globalVariables": true,
                "diagnostics": {
                    "enable": true
                },
                "completion": {
                    "insertUseDeclaration": true,
                    "fullyQualifyGlobalConstantsAndFunctions": true,
                    "triggerParameterHints": true,
                    "maxItems": 100
                },
                "format": {
                    "enable": true
                },
                "files": {
                    "associations": ["*.php", "*.phtml"],
                    "exclude": [
                        "**/.git/**",
                        "**/.svn/**",
                        "**/.hg/**",
                        "**/CVS/**",
                        "**/.DS_Store/**",
                        "**/Thumbs.db/**",
                        "**/node_modules/**",
                        "**/bower_components/**",
                        "**/vendor/**/{Test,test,Tests,tests}/**"
                    ],
                    "maxSize": 1000000
                },
                "licenceKey": null
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a Metals (Scala) language server configuration.
    ///
    /// Metals requires a build tool (sbt, Mill, Maven, or Gradle) in the workspace.
    /// Install via: `cs install metals` or `brew install metals`
    /// Requires JDK 11+.
    pub fn scala(workspace_root: PathBuf) -> Self {
        Self {
            command: "metals".to_string(),
            args: vec![],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "metals": {
                    "showImplicitArguments": true,
                    "showInferredType": true
                }
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a C/C++ (clangd) language server configuration.
    pub fn clangd(workspace_root: PathBuf) -> Self {
        Self {
            command: "clangd".to_string(),
            args: vec![
                "--background-index".to_string(),
                "--clang-tidy".to_string(),
                "--completion-style=detailed".to_string(),
                "--header-insertion=iwyu".to_string(),
            ],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "clangd": {
                    "fallbackFlags": ["-std=c++17"]
                }
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a Swift (sourcekit-lsp) language server configuration.
    ///
    /// sourcekit-lsp is bundled with Xcode on macOS.
    /// On Linux, install via Swift toolchain: https://swift.org/download/
    pub fn swift(workspace_root: PathBuf) -> Self {
        Self {
            command: "sourcekit-lsp".to_string(),
            args: vec![],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({}),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a SQL (sqls) language server configuration.
    ///
    /// sqls is a Go-based SQL language server supporting multiple dialects.
    /// Install via: `go install github.com/sqls-server/sqls@latest`
    /// Requires a .sqls.yaml config file with database connection details.
    /// See: https://github.com/sqls-server/sqls
    pub fn sql(workspace_root: PathBuf) -> Self {
        Self {
            command: "sqls".to_string(),
            args: vec![],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({
                "connections": [] // Configured per-project in .sqls.yaml
            }),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a Dart (dart language-server) language server configuration.
    ///
    /// The Dart SDK ships with a built-in language server.
    /// Install the Dart SDK: https://dart.dev/get-dart
    /// Or via Flutter: https://flutter.dev/docs/get-started/install
    pub fn dart(workspace_root: PathBuf) -> Self {
        Self {
            command: "dart".to_string(),
            args: vec!["language-server".to_string(), "--protocol=lsp".to_string()],
            working_directory: Some(workspace_root),
            env: HashMap::new(),
            initialization_options: serde_json::json!({}),
            timeout_ms: default_timeout(),
        }
    }

    /// Creates a configuration for a generic LSP server.
    pub fn new(command: String, args: Vec<String>) -> Self {
        Self {
            command,
            args,
            working_directory: None,
            env: HashMap::new(),
            initialization_options: serde_json::Value::Null,
            timeout_ms: default_timeout(),
        }
    }

    /// Sets the working directory for the server.
    pub fn with_working_directory(mut self, dir: PathBuf) -> Self {
        self.working_directory = Some(dir);
        self
    }

    /// Sets an environment variable.
    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.env.insert(key, value);
        self
    }

    /// Sets the initialization options.
    pub fn with_initialization_options(mut self, options: serde_json::Value) -> Self {
        self.initialization_options = options;
        self
    }

    /// Sets the request timeout.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Validates the configuration.
    ///
    /// Returns an error if the command is not in the allowed list.
    /// This prevents command injection attacks.
    pub fn validate(&self) -> Result<()> {
        // Extract the command name (handle both bare commands and paths)
        let command_name = std::path::Path::new(&self.command)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.command);

        if !ALLOWED_COMMANDS.contains(&command_name) {
            return Err(LspError::InvalidCommand {
                command: self.command.clone(),
                allowed: ALLOWED_COMMANDS.iter().map(|s| s.to_string()).collect(),
            });
        }

        // Validate that args don't contain shell metacharacters
        for arg in &self.args {
            if arg.contains('|')
                || arg.contains(';')
                || arg.contains('&')
                || arg.contains('`')
                || arg.contains('$')
                || arg.contains('>')
                || arg.contains('<')
            {
                return Err(LspError::InvalidArgument {
                    argument: arg.clone(),
                    reason: "contains shell metacharacter".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Returns the shell command to install this LSP server, if known.
    ///
    /// Returns `None` for servers with no known installation method.
    pub fn install_command(&self) -> Option<String> {
        // Extract bare command name from possible full path
        let command_name = std::path::Path::new(&self.command)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.command);

        match command_name {
            "typescript-language-server" => {
                Some("npm install -g typescript-language-server typescript".to_string())
            }
            "pyright-langserver" => Some("pip install pyright".to_string()),
            "rust-analyzer" => Some("rustup component add rust-analyzer".to_string()),
            "gopls" => Some("go install golang.org/x/tools/gopls@latest".to_string()),
            "clangd" => Some("brew install llvm".to_string()),
            "ruby-lsp" => Some("gem install ruby-lsp".to_string()),
            "intelephense" => Some("npm install -g intelephense".to_string()),
            "phpactor" => Some("composer global require phpactor/phpactor".to_string()),
            "kotlin-language-server" => Some("brew install kotlin-language-server".to_string()),
            "OmniSharp" | "omnisharp" => Some("brew install omnisharp-roslyn".to_string()),
            "metals" => Some("cs install metals".to_string()),
            "jdtls" => Some("brew install jdtls".to_string()),
            "elixir-ls" => Some("brew install elixir-ls".to_string()),
            "sourcekit-lsp" => Some("xcode-select --install".to_string()),
            "dart" => Some("brew install dart".to_string()),
            "sqls" => Some("go install github.com/sqls-server/sqls@latest".to_string()),
            "lua-language-server" => Some("brew install lua-language-server".to_string()),
            "bash-language-server" => Some("npm install -g bash-language-server".to_string()),
            "yaml-language-server" => Some("npm install -g yaml-language-server".to_string()),
            "taplo" => Some("cargo install taplo-cli".to_string()),
            _ => None,
        }
    }

    /// Returns the project page of this LSP server.
    ///
    /// For a server with no known page, returns the list of language server
    /// implementations.
    pub fn homepage(&self) -> &'static str {
        let command_name = std::path::Path::new(&self.command)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.command);

        match command_name {
            "typescript-language-server" => {
                "https://github.com/typescript-language-server/typescript-language-server"
            }
            "pyright-langserver" => "https://github.com/microsoft/pyright",
            "rust-analyzer" => "https://rust-analyzer.github.io",
            "gopls" => "https://pkg.go.dev/golang.org/x/tools/gopls",
            "clangd" => "https://clangd.llvm.org",
            "ruby-lsp" => "https://github.com/Shopify/ruby-lsp",
            "intelephense" => "https://intelephense.com",
            "phpactor" => "https://github.com/phpactor/phpactor",
            "kotlin-language-server" => "https://github.com/fwcd/kotlin-language-server",
            "OmniSharp" | "omnisharp" => "https://github.com/OmniSharp/omnisharp-roslyn",
            "metals" => "https://scalameta.org/metals",
            "jdtls" => "https://github.com/eclipse-jdtls/eclipse.jdt.ls",
            "elixir-ls" => "https://github.com/elixir-lsp/elixir-ls",
            "sourcekit-lsp" => "https://github.com/swiftlang/sourcekit-lsp",
            "dart" => "https://dart.dev/get-dart",
            "sqls" => "https://github.com/sqls-server/sqls",
            "lua-language-server" => "https://github.com/LuaLS/lua-language-server",
            "bash-language-server" => "https://github.com/bash-lsp/bash-language-server",
            "yaml-language-server" => "https://github.com/redhat-developer/yaml-language-server",
            "taplo" => "https://taplo.tamasfe.dev",
            _ => "https://microsoft.github.io/language-server-protocol/implementors/servers/",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::servers::LspServer;

    /// Every server `servers.rs` names must pass the spawn allow-list.
    #[test]
    fn allowed_commands_include_every_configured_server() {
        let root = PathBuf::from("/allow-list-test");
        for server in LspServer::ALL {
            let config = server.config(root.clone());
            assert!(
                config.validate().is_ok(),
                "{server:?}: command '{}' is refused by validate",
                config.command
            );
        }
    }

    #[test]
    fn test_typescript_config() {
        let config = ServerConfig::typescript(PathBuf::from("/workspace"));
        assert_eq!(config.command, "typescript-language-server");
        assert_eq!(config.args, vec!["--stdio"]);
        assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
    }

    #[test]
    fn test_custom_config() {
        let config = ServerConfig::new("rust-analyzer".to_string(), vec![])
            .with_working_directory(PathBuf::from("/project"))
            .with_env("RUST_BACKTRACE".to_string(), "1".to_string())
            .with_timeout(2000);

        assert_eq!(config.command, "rust-analyzer");
        assert_eq!(config.working_directory, Some(PathBuf::from("/project")));
        assert_eq!(config.env.get("RUST_BACKTRACE"), Some(&"1".to_string()));
        assert_eq!(config.timeout_ms, 2000);
    }

    #[test]
    fn test_validate_allowed_command() {
        let config = ServerConfig::typescript(PathBuf::from("/workspace"));
        assert!(config.validate().is_ok());

        let config = ServerConfig::new("rust-analyzer".to_string(), vec![]);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_disallowed_command() {
        let config = ServerConfig::new("rm".to_string(), vec!["-rf".to_string(), "/".to_string()]);
        assert!(config.validate().is_err());

        let config = ServerConfig::new(
            "bash".to_string(),
            vec!["-c".to_string(), "echo pwned".to_string()],
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_command_with_path() {
        // Commands specified with full path should also be validated
        let config = ServerConfig::new(
            "/usr/bin/typescript-language-server".to_string(),
            vec!["--stdio".to_string()],
        );
        assert!(config.validate().is_ok());

        let config = ServerConfig::new("/usr/bin/rm".to_string(), vec![]);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_shell_metacharacters_in_args() {
        let config = ServerConfig::new(
            "typescript-language-server".to_string(),
            vec!["--stdio; rm -rf /".to_string()],
        );
        assert!(config.validate().is_err());

        let config = ServerConfig::new(
            "typescript-language-server".to_string(),
            vec!["$(whoami)".to_string()],
        );
        assert!(config.validate().is_err());

        let config = ServerConfig::new(
            "typescript-language-server".to_string(),
            vec!["`whoami`".to_string()],
        );
        assert!(config.validate().is_err());

        let config = ServerConfig::new(
            "typescript-language-server".to_string(),
            vec!["arg | cat /etc/passwd".to_string()],
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_redirect_metacharacters() {
        // Test > and < redirects
        let config = ServerConfig::new(
            "typescript-language-server".to_string(),
            vec!["--output>/etc/passwd".to_string()],
        );
        assert!(config.validate().is_err());

        let config = ServerConfig::new(
            "typescript-language-server".to_string(),
            vec!["--input</etc/shadow".to_string()],
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_ampersand_metacharacter() {
        let config = ServerConfig::new(
            "typescript-language-server".to_string(),
            vec!["--opt && rm -rf /".to_string()],
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_config_with_initialization_options() {
        let config = ServerConfig::new("rust-analyzer".to_string(), vec![])
            .with_initialization_options(serde_json::json!({
                "cargo": {
                    "allFeatures": true
                }
            }));

        assert!(config.initialization_options.is_object());
        assert!(
            config
                .initialization_options
                .get("cargo")
                .unwrap()
                .get("allFeatures")
                .unwrap()
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn test_server_config_default_timeout() {
        let config = ServerConfig::new("rust-analyzer".to_string(), vec![]);
        // Default timeout should be 30000ms (allows for LSP cold start)
        assert_eq!(config.timeout_ms, 30000);
    }

    #[test]
    fn test_server_config_multiple_env_vars() {
        let config = ServerConfig::new("rust-analyzer".to_string(), vec![])
            .with_env("VAR1".to_string(), "value1".to_string())
            .with_env("VAR2".to_string(), "value2".to_string())
            .with_env("VAR3".to_string(), "value3".to_string());

        assert_eq!(config.env.len(), 3);
        assert_eq!(config.env.get("VAR1"), Some(&"value1".to_string()));
        assert_eq!(config.env.get("VAR2"), Some(&"value2".to_string()));
        assert_eq!(config.env.get("VAR3"), Some(&"value3".to_string()));
    }

    #[test]
    fn test_server_config_override_env() {
        let config = ServerConfig::new("rust-analyzer".to_string(), vec![])
            .with_env("KEY".to_string(), "original".to_string())
            .with_env("KEY".to_string(), "overridden".to_string());

        assert_eq!(config.env.get("KEY"), Some(&"overridden".to_string()));
    }

    #[test]
    fn test_validate_all_allowed_commands() {
        // Test all allowed commands
        let allowed = vec![
            "bash-language-server",
            "clangd",
            "dockerfile-language-server-nodejs",
            "elixir-ls",
            "eslint-language-server",
            "intelephense",
            "java",
            "jdtls",
            "kotlin-language-server",
            "metals",
            "omnisharp",
            "OmniSharp",
            "phpactor",
            "pyright-langserver",
            "ruby-lsp",
            "rust-analyzer",
            "sqls",
            "taplo",
            "typescript-language-server",
            "vscode-css-language-server",
            "vscode-html-language-server",
            "vscode-json-language-server",
            "yaml-language-server",
        ];

        for cmd in allowed {
            let config = ServerConfig::new(cmd.to_string(), vec![]);
            assert!(config.validate().is_ok(), "Expected {} to be allowed", cmd);
        }
    }

    #[test]
    fn test_csharp_config() {
        let config = ServerConfig::csharp(PathBuf::from("/workspace"));
        assert_eq!(config.command, "OmniSharp");
        assert_eq!(config.args, vec!["-lsp"]);
        assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_python_config() {
        let config = ServerConfig::python(PathBuf::from("/workspace"));
        assert_eq!(config.command, "pyright-langserver");
        assert_eq!(config.args, vec!["--stdio"]);
        assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_rust_config() {
        let config = ServerConfig::rust(PathBuf::from("/workspace"));
        assert_eq!(config.command, "rust-analyzer");
        assert_eq!(config.args, vec![] as Vec<String>);
        assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_go_config() {
        let config = ServerConfig::go(PathBuf::from("/workspace"));
        assert_eq!(config.command, "gopls");
        assert_eq!(config.args, vec!["serve"]);
        assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_elixir_config() {
        let config = ServerConfig::elixir(PathBuf::from("/workspace"));
        assert_eq!(config.command, "elixir-ls");
        assert_eq!(config.args, vec![] as Vec<String>);
        assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_java_config() {
        let config = ServerConfig::java(PathBuf::from("/workspace"));
        assert_eq!(config.command, "jdtls");
        assert_eq!(config.args, vec![] as Vec<String>);
        assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_kotlin_config() {
        let config = ServerConfig::kotlin(PathBuf::from("/workspace"));
        assert_eq!(config.command, "kotlin-language-server");
        assert_eq!(config.args, vec![] as Vec<String>);
        assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_server_config_serialization() {
        let config = ServerConfig::new("rust-analyzer".to_string(), vec!["--stdio".to_string()])
            .with_working_directory(PathBuf::from("/project"))
            .with_timeout(3000);

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("rust-analyzer"));
        assert!(json.contains("--stdio"));
        assert!(json.contains("3000"));

        // Deserialize back
        let deserialized: ServerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.command, "rust-analyzer");
        assert_eq!(deserialized.timeout_ms, 3000);
    }

    #[test]
    fn test_ruby_config() {
        let config = ServerConfig::ruby(PathBuf::from("/workspace"));
        assert_eq!(config.command, "ruby-lsp");
        assert_eq!(config.args, vec![] as Vec<String>);
        assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_sql_config() {
        let config = ServerConfig::sql(PathBuf::from("/workspace"));
        assert_eq!(config.command, "sqls");
        assert_eq!(config.args, vec![] as Vec<String>);
        assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_clangd_config() {
        let config = ServerConfig::clangd(PathBuf::from("/workspace"));
        assert_eq!(config.command, "clangd");
        assert_eq!(
            config.args,
            vec![
                "--background-index",
                "--clang-tidy",
                "--completion-style=detailed",
                "--header-insertion=iwyu"
            ]
        );
        assert_eq!(config.working_directory, Some(PathBuf::from("/workspace")));
        assert!(config.validate().is_ok());

        // Verify initialization options
        let init_opts = config.initialization_options;
        assert!(init_opts.is_object());
        assert!(init_opts.get("clangd").is_some());
        assert_eq!(
            init_opts
                .get("clangd")
                .unwrap()
                .get("fallbackFlags")
                .unwrap()
                .as_array()
                .unwrap()[0],
            "-std=c++17"
        );
    }

    #[test]
    fn test_install_command_typescript() {
        let config = ServerConfig::typescript(PathBuf::from("/ws"));
        assert_eq!(
            config.install_command(),
            Some("npm install -g typescript-language-server typescript".to_string()),
        );
    }

    #[test]
    fn test_install_command_rust() {
        let config = ServerConfig::rust(PathBuf::from("/ws"));
        assert_eq!(
            config.install_command(),
            Some("rustup component add rust-analyzer".to_string()),
        );
    }

    #[test]
    fn test_install_command_python() {
        let config = ServerConfig::python(PathBuf::from("/ws"));
        assert_eq!(
            config.install_command(),
            Some("pip install pyright".to_string()),
        );
    }

    #[test]
    fn test_install_command_go() {
        let config = ServerConfig::go(PathBuf::from("/ws"));
        assert_eq!(
            config.install_command(),
            Some("go install golang.org/x/tools/gopls@latest".to_string()),
        );
    }

    #[test]
    fn test_install_command_generic() {
        let config = ServerConfig::new("rust-analyzer".to_string(), vec![]);
        assert_eq!(
            config.install_command(),
            Some("rustup component add rust-analyzer".to_string())
        );
    }

    #[test]
    fn test_install_command_unknown() {
        // A command not in the known install map returns None
        let config = ServerConfig::new("some-unknown-lsp".to_string(), vec![]);
        // validate() would reject this, but install_command() doesn't care about validation
        assert_eq!(config.install_command(), None);
    }

    /// Every server has its own project page, not the generic fallback.
    #[test]
    fn every_server_has_a_homepage() {
        let fallback = ServerConfig::new("some-unknown-lsp".to_string(), vec![]).homepage();
        for server in LspServer::ALL {
            let homepage = server.config(PathBuf::from("/ws")).homepage();
            assert!(homepage.starts_with("https://"), "{server:?}: {homepage}");
            assert_ne!(homepage, fallback, "{server:?} has no homepage of its own");
        }
    }
}
