//! Hyprland session information (read-only).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Timestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HyprMonitor {
    pub name: String,
    pub description: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: f32,
    pub scale: f32,
    pub focused: bool,
    pub active_workspace: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HyprUnavailableReason {
    /// `HYPRLAND_INSTANCE_SIGNATURE` is not set: no Hyprland session.
    NoSession,
    /// The IPC socket does not exist or is not a socket.
    SocketMissing,
    /// The IPC request timed out.
    Timeout,
    /// The IPC answer could not be parsed.
    InvalidResponse,
}

/// Hyprland information. Only available inside an active Hyprland session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HyprlandInfo {
    pub available: bool,
    pub reason: Option<HyprUnavailableReason>,
    pub version: Option<String>,
    pub monitors: Vec<HyprMonitor>,
    pub active_workspace: Option<String>,
    pub workspace_count: Option<u32>,
    /// Window class / app-id of cachyos-center for Hyprland window rules.
    pub app_class: String,
    #[ts(type = "number")]
    pub collected_at: Timestamp,
}

impl HyprlandInfo {
    pub fn unavailable(reason: HyprUnavailableReason, now: Timestamp) -> Self {
        Self {
            available: false,
            reason: Some(reason),
            version: None,
            monitors: Vec::new(),
            active_workspace: None,
            workspace_count: None,
            app_class: crate::APP_ID.to_string(),
            collected_at: now,
        }
    }
}
