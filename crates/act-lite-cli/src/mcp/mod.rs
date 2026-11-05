//! act-lite MCP server: the navigation tools over stdio.

mod catalog;
mod params;
pub(crate) mod path_error;
mod server;

pub use server::ActLiteServer;

use anyhow::Context;
use rmcp::ServiceExt;
use std::path::Path;

/// Serves the MCP tools over stdio until the client disconnects, then stops
/// every language server the session started. Without `workspace`, the root
/// is the nearest enclosing git repository, else the current directory.
pub async fn serve(workspace: Option<&Path>, enable_lsp: bool) -> anyhow::Result<()> {
    let server = match workspace {
        Some(root) => ActLiteServer::with_root(root, enable_lsp),
        None => ActLiteServer::new(enable_lsp),
    };
    let owner = server.clone();
    // The stdout half is watched: a client that closes the pipe has left, so
    // the service is cancelled and the session ends like a stdin EOF — no
    // command output was cut, so the process exits 0.
    let (watched_stdout, stdout_closed) = crate::stdout::watched(tokio::io::stdout());
    let running = server
        .serve((tokio::io::stdin(), watched_stdout))
        .await
        .with_context(|| {
            format!(
                "failed to start the MCP server for {}",
                owner.root().display()
            )
        })?;
    let cancel_token = running.cancellation_token();
    let stdout_watcher = tokio::spawn(async move {
        stdout_closed.wait().await;
        cancel_token.cancel();
    });
    let quit = running
        .waiting()
        .await
        .context("the MCP server stopped with an error");
    owner.shutdown().await?;
    stdout_watcher.abort();
    quit.map(|_| ())
}
