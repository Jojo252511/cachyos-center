//! Local, read-only MCP server of cachyos-center.
//!
//! An MCP host (Claude Code, Claude Desktop, ...) starts `cachyos-center-mcp`
//! as a child process and talks MCP (JSON-RPC) over stdin/stdout. There is no
//! network listener, no HTTP transport and no automatic registration in host
//! configuration files. Logs go to stderr only.
//!
//! The server offers exactly the six tools of
//! [`cachyos_center_core::mcp::TOOL_NAMES`]. All of them only read through
//! [`cachyos_center_service::ReadApi`]; there are no mutating tools, no
//! resources and no prompts, and the crate has no way to reach the privileged
//! helper (no D-Bus dependency).
//!
//! Access is switched off by default. While `mcpEnabled` is off in the user
//! settings, the server still initializes and lists its tools, but every tool
//! call returns an error result with code `UNAVAILABLE`. The setting is read
//! again on every call.
//!
//! Result format: successful calls return the output as structured content
//! and the same JSON pretty-printed as text content. Failures return
//! `isError: true` with `{ "code", "message" }` (codes `UNAVAILABLE`, `BUSY`,
//! `STALE`, `UNSUPPORTED`, `INTERNAL`, and `INVALID_INPUT` for invalid
//! arguments). Unknown tool names are rejected with the JSON-RPC error
//! `-32602` (invalid params).

pub mod args;
pub mod output;
mod render;
mod server;
pub mod tools;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use anyhow::Context;
use cachyos_center_service::ReadApi;
use rmcp::ServiceExt;
use rmcp::service::ServerInitializeError;

pub use render::{MAX_STRING_CHARS, PATH_PLACEHOLDER};
pub use server::{
    DISABLED_MESSAGE, INSTRUCTIONS, MAX_CONCURRENT_CALLS, MAX_OUTPUT_BYTES, McpServer,
    ServerOptions, TOOL_TIMEOUT,
};
pub use tools::ToolKind;

/// Serves MCP on stdin/stdout until the host closes stdin (EOF).
pub async fn serve_stdio(api: Arc<dyn ReadApi>) -> anyhow::Result<()> {
    let server = McpServer::new(api);
    let running = match server.serve(rmcp::transport::stdio()).await {
        Ok(running) => running,
        // The host went away before initializing: nothing to serve.
        Err(ServerInitializeError::ConnectionClosed(_)) => {
            tracing::debug!("stdin closed before initialization");
            return Ok(());
        }
        Err(e) => return Err(e).context("MCP initialization failed"),
    };
    let reason = running.waiting().await.context("MCP service task failed")?;
    tracing::debug!(?reason, "MCP session ended");
    Ok(())
}
