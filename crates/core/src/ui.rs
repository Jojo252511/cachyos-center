//! Types that only exist at the desktop IPC boundary.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::system::BackendStatus;

/// Part of an operation log (`ReadOperationLog`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OperationLogChunk {
    pub lines: Vec<String>,
    /// Byte offset for the next request.
    #[ts(type = "number")]
    pub next_offset: u64,
    /// The operation has ended and the log is complete.
    pub complete: bool,
}

/// Information for the MCP settings section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct McpSetup {
    pub enabled: bool,
    /// Absolute path of `cachyos-center-mcp` (`None` when not found).
    pub binary_path: Option<String>,
    /// Example host configuration (JSON, `mcpServers` format) with the absolute path.
    pub host_config: String,
    /// Names of the read-only tools offered by the server.
    pub tools: Vec<String>,
}

/// General information about the running application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    pub app_id: String,
    /// Preferred UI language derived from `LANG`/`LC_MESSAGES` (`de` or `en`).
    pub system_language: String,
    /// Desktop color scheme preference (`dark`, `light` or `unknown`).
    pub system_color_scheme: String,
    /// The privileged helper is installed and reachable on the system bus.
    pub helper_available: bool,
    /// Reason when the helper is not reachable.
    pub helper_error: Option<String>,
    pub backend: BackendStatus,
    /// Development build using the session-bus test helper.
    pub development_mode: bool,
}
