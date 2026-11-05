//! act-lite: navigation-only code queries and sequestered secrets for AI agents.
//!
//! The `act-lite` binary exposes the `query`, `mcp serve` and `env` commands;
//! this library holds their argument parsing, output formatting, query
//! runners, the MCP server and the `env` dispatch.

#![cfg_attr(not(test), deny(clippy::print_stdout))]

pub mod analyze;
pub mod cli;
pub mod definition;
pub mod env;
pub mod mcp;
pub mod output;
pub mod query;
pub mod stdout;

pub use query::{
    execute_repo_outline, execute_symbols_batch_by_files, lsp_unavailable_error,
    unsupported_language_error,
};
