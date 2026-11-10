//! act-lite-lsp: Language Server Protocol client integration.
//!
//! This crate provides:
//! - LSP client for communicating with language servers ([`LspClient`])
//! - Server pool management ([`LspPool`]) - lazy spawn, lifecycle
//! - Capability mapping (references, hover, diagnostics, etc.) - see [`capabilities`]
//! - Fallback behavior when LSP unavailable
//!
//! # Architecture
//!
//! The LSP integration follows a layered approach:
//!
//! 1. **Client Layer** ([`client`]): Raw JSON-RPC over stdio communication
//! 2. **Pool Layer** ([`pool`]): One client per language grammar, lazy initialization
//! 3. **Capability Layer** ([`capabilities`]): High-level query operations
//!
//! # Usage
//!
//! ```no_run
//! use act_lite_lsp::{LspPool, capabilities};
//! use act_lite_parser::Grammar;
//! use std::path::PathBuf;
//!
//! # async fn example() -> act_lite_lsp::Result<()> {
//! // Create a pool for the workspace
//! let pool = LspPool::new(PathBuf::from("/workspace"));
//!
//! // Get a client for TypeScript
//! let client = pool.get_client(Grammar::TypeScript).await;
//!
//! // Query references
//! if let Some(client) = client {
//!     let location = act_lite_core::Location::new(
//!         PathBuf::from("src/main.ts"),
//!         10,
//!         5,
//!     );
//!     let file = PathBuf::from("src/main.ts");
//!     let refs = capabilities::query_references(
//!         client.as_ref(),
//!         &file,
//!         &location,
//!         None,
//!     ).await?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Graceful Degradation
//!
//! If an LSP server is not available, the pool will return `None` for that grammar.
//! Callers should fall back to tree-sitter-only operations in this case.

pub mod capabilities;
pub mod client;
pub mod config;
pub mod error;
pub mod installer;
#[cfg(windows)]
mod job;
pub mod mock;
pub mod pool;
pub mod servers;
pub mod types;
pub mod watchdog;

#[cfg(all(test, unix))]
#[path = "../../../tests/support/executable_script.rs"]
mod executable_script;
#[cfg(test)]
mod test_path;

pub use capabilities::{
    CalleesResult, CallersResult, CodeAction, CodeActionsResult, Diagnostic, DiagnosticSeverity,
    DiagnosticsResult, ImplementationsResult, ReferencesResult, RenamePreview, TypeInfo,
    WorkspaceSymbolsResult, lsp_diagnostic_to_diagnostic, lsp_severity_to_diagnostic_severity,
    open_document, query_callees, query_callers, query_code_actions, query_diagnostics,
    query_implementations, query_references, query_type, query_workspace_symbols, sync_document,
    validate_rename,
};
pub use client::{LspClient, LspClientTrait};
pub use config::ServerConfig;
pub use error::{LspError, Result};
pub use mock::MockLspClient;
pub use pool::LspPool;
pub use servers::LspServer;
pub use types::{path_to_uri, uri_to_path};
