//! Positive grammar→LSP-server table.
//!
//! Single source of truth for which grammars are backed by a configured LSP
//! server. [`LspServer::for_grammar`] lists ONLY the server-backed grammars;
//! every other grammar falls through to `None` without being enumerated.
//! Adding a language that needs a server is one match arm here; adding a
//! language without a server requires no edit at all, so a missed arm for a
//! serverless grammar cannot recur.

use crate::config::ServerConfig;
use act_lite_parser::Grammar;
use std::path::PathBuf;

/// An LSP server act knows how to spawn, named after the binary it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LspServer {
    /// `typescript-language-server` — TypeScript, TSX, JSX, JavaScript.
    TypeScriptLanguageServer,
    /// `OmniSharp` — C#.
    OmniSharp,
    /// `pyright-langserver` — Python.
    Pyright,
    /// `rust-analyzer` — Rust.
    RustAnalyzer,
    /// `gopls` — Go.
    Gopls,
    /// `elixir-ls` — Elixir.
    ElixirLs,
    /// `jdtls` — Java.
    Jdtls,
    /// `kotlin-language-server` — Kotlin.
    KotlinLanguageServer,
    /// `clangd` — C, C++.
    Clangd,
    /// `ruby-lsp` — Ruby.
    RubyLsp,
    /// `sqls` — SQL.
    Sqls,
    /// `intelephense` — PHP.
    Intelephense,
    /// `metals` — Scala.
    Metals,
    /// `sourcekit-lsp` — Swift.
    SourceKitLsp,
    /// `dart language-server` — Dart.
    Dart,
}

impl LspServer {
    /// Every configured server, for table-driven tests.
    pub const ALL: [LspServer; 15] = [
        LspServer::TypeScriptLanguageServer,
        LspServer::OmniSharp,
        LspServer::Pyright,
        LspServer::RustAnalyzer,
        LspServer::Gopls,
        LspServer::ElixirLs,
        LspServer::Jdtls,
        LspServer::KotlinLanguageServer,
        LspServer::Clangd,
        LspServer::RubyLsp,
        LspServer::Sqls,
        LspServer::Intelephense,
        LspServer::Metals,
        LspServer::SourceKitLsp,
        LspServer::Dart,
    ];

    /// The server backing a grammar, or `None` for the (majority of)
    /// grammars that have no configured LSP server.
    ///
    /// This is the positive table: list a grammar here ONLY when it gains a
    /// server. Serverless grammars must NOT be enumerated.
    pub fn for_grammar(grammar: Grammar) -> Option<Self> {
        Some(match grammar {
            Grammar::TypeScript | Grammar::Tsx | Grammar::Jsx | Grammar::JavaScript => {
                Self::TypeScriptLanguageServer
            }
            Grammar::CSharp => Self::OmniSharp,
            Grammar::Python => Self::Pyright,
            Grammar::Rust => Self::RustAnalyzer,
            Grammar::Go => Self::Gopls,
            Grammar::Elixir => Self::ElixirLs,
            Grammar::Java => Self::Jdtls,
            Grammar::Kotlin => Self::KotlinLanguageServer,
            Grammar::C | Grammar::Cpp => Self::Clangd,
            Grammar::Ruby => Self::RubyLsp,
            Grammar::Sql => Self::Sqls,
            Grammar::Php => Self::Intelephense,
            Grammar::Scala => Self::Metals,
            Grammar::Swift => Self::SourceKitLsp,
            Grammar::Dart => Self::Dart,
            _ => return None,
        })
    }

    /// Full spawn configuration for this server in the given workspace.
    pub fn config(self, workspace_root: PathBuf) -> ServerConfig {
        match self {
            Self::TypeScriptLanguageServer => ServerConfig::typescript(workspace_root),
            Self::OmniSharp => ServerConfig::csharp(workspace_root),
            Self::Pyright => ServerConfig::python(workspace_root),
            Self::RustAnalyzer => ServerConfig::rust(workspace_root),
            Self::Gopls => ServerConfig::go(workspace_root),
            Self::ElixirLs => ServerConfig::elixir(workspace_root),
            Self::Jdtls => ServerConfig::java(workspace_root),
            Self::KotlinLanguageServer => ServerConfig::kotlin(workspace_root),
            Self::Clangd => ServerConfig::clangd(workspace_root),
            Self::RubyLsp => ServerConfig::ruby(workspace_root),
            Self::Sqls => ServerConfig::sql(workspace_root),
            Self::Intelephense => ServerConfig::php(workspace_root),
            Self::Metals => ServerConfig::scala(workspace_root),
            Self::SourceKitLsp => ServerConfig::swift(workspace_root),
            Self::Dart => ServerConfig::dart(workspace_root),
        }
    }

    /// The notification that ends this server's first build, for a server
    /// that reports the build by no work-done progress: (method,
    /// `params.name`). Until it arrives the server answers from the open
    /// files only: elixir-ls 0.30 builds the project 3 s after `initialized`
    /// and ends the build with `telemetry/event` `{"name": "build"}`.
    pub fn build_end_notification(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::ElixirLs => Some(("telemetry/event", "build")),
            _ => None,
        }
    }

    /// The binary this server runs — always identical to the `command` of
    /// [`Self::config`] (enforced by test). Use for install hints.
    pub fn binary_name(self) -> &'static str {
        match self {
            Self::TypeScriptLanguageServer => "typescript-language-server",
            Self::OmniSharp => "OmniSharp",
            Self::Pyright => "pyright-langserver",
            Self::RustAnalyzer => "rust-analyzer",
            Self::Gopls => "gopls",
            Self::ElixirLs => "elixir-ls",
            Self::Jdtls => "jdtls",
            Self::KotlinLanguageServer => "kotlin-language-server",
            Self::Clangd => "clangd",
            Self::RubyLsp => "ruby-lsp",
            Self::Sqls => "sqls",
            Self::Intelephense => "intelephense",
            Self::Metals => "metals",
            Self::SourceKitLsp => "sourcekit-lsp",
            Self::Dart => "dart",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `binary_name` and `config().command` encode the same fact; they must
    /// never drift apart.
    #[test]
    fn binary_name_matches_config_command_for_every_server() {
        let root = PathBuf::from("/drift-test");
        for server in LspServer::ALL {
            assert_eq!(
                server.config(root.clone()).command,
                server.binary_name(),
                "{server:?}: binary_name drifted from the spawned command"
            );
        }
    }

    /// Design property: a grammar absent from the positive table maps to no
    /// server without needing an explicit arm. `Grammar::Markdown` and
    /// `Grammar::Yaml` are plain data formats.
    #[test]
    fn serverless_grammars_are_not_enumerated() {
        assert_eq!(LspServer::for_grammar(Grammar::Markdown), None);
        assert_eq!(LspServer::for_grammar(Grammar::Yaml), None);
    }

    /// Every server in `ALL` is reachable from at least one grammar — no
    /// orphaned table rows.
    #[test]
    fn every_server_is_reachable_from_some_grammar() {
        for server in LspServer::ALL {
            assert!(
                Grammar::all()
                    .into_iter()
                    .any(|g| LspServer::for_grammar(g) == Some(server)),
                "{server:?} is in ALL but no grammar maps to it"
            );
        }
    }
}
