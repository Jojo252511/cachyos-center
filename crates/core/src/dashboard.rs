//! Start page model.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Timestamp;
use crate::health::{HealthItem, Severity};
use crate::history::HistoryEntry;
use crate::system::{LockStatus, SystemSummary};
use crate::updates::UpdateCheckResult;

/// Data of the dashboard ("Übersicht").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Dashboard {
    pub system: SystemSummary,
    pub updates: UpdateCheckResult,
    pub lock: LockStatus,
    #[ts(type = "number")]
    pub installed_count: u64,
    #[ts(type = "number")]
    pub foreign_count: u64,
    pub health_items: Vec<HealthItem>,
    pub health_worst: Option<Severity>,
    pub last_activity: Option<HistoryEntry>,
    /// Last completed full system upgrade (pacman log).
    #[ts(type = "number | null")]
    pub last_full_upgrade: Option<Timestamp>,
    /// Next background check of the running app (settings interval).
    #[ts(type = "number | null")]
    pub next_check_at: Option<Timestamp>,
    /// An update is prepared by pacman-offline for the next reboot.
    pub offline_update_prepared: bool,
    pub reboot_recommended: bool,
    #[ts(type = "number")]
    pub collected_at: Timestamp,
}
