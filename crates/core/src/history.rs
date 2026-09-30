//! Activity history (operations of the app, the timer and external pacman runs).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Timestamp;
use crate::error::ErrorCode;
use crate::operation::{OperationKind, OperationOrigin, OperationState};

/// Source of a history entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HistorySource {
    /// Operation started from cachyos-center.
    App,
    /// Operation of the cachyos-center timer.
    Timer,
    /// Transaction found in `/var/log/pacman.log` that was not started by cachyos-center.
    ExternalPacman,
}

/// Outcome of an external pacman transaction reconstructed from the pacman log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LogOutcome {
    Completed,
    Failed,
    Interrupted,
    /// Started but no end marker was found.
    Unknown,
}

/// One entry of the activity list. Contains no raw logs and no file paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryEntry {
    pub id: String,
    pub source: HistorySource,
    pub kind: Option<OperationKind>,
    pub origin: Option<OperationOrigin>,
    pub state: Option<OperationState>,
    pub log_outcome: Option<LogOutcome>,
    #[ts(type = "number")]
    pub started_at: Timestamp,
    #[ts(type = "number | null")]
    pub ended_at: Option<Timestamp>,
    pub summary: String,
    pub error_code: Option<ErrorCode>,
    pub installed: u32,
    pub upgraded: u32,
    pub removed: u32,
    pub downgraded: u32,
    /// Up to 20 affected package names.
    pub packages: Vec<String>,
    pub outcome_unknown: bool,
}
