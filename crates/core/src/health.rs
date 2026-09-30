//! Health center model: hints that need attention, update blockers, snapshots.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Timestamp;
use crate::policy::{ExternalUpdater, OfflineUpdateStatus};
use crate::system::LockStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// Stable identifiers of health items. The UI renders localized texts for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HealthItemKind {
    PacnewFiles,
    PacsaveFiles,
    RebootRecommended,
    PackageManagerLocked,
    LastOperationFailed,
    InterruptedTransaction,
    PackageCacheLarge,
    PrerequisiteMissing,
    PackageBackendUnavailable,
    UpdateCheckFailed,
    UpdateCheckStale,
    OfflineUpdatePrepared,
    OfflineConfHoldsPackages,
    ExternalUpdaterActive,
    NewsUnread,
    NewsUnavailable,
    LowDiskSpace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HealthItem {
    pub kind: HealthItemKind,
    pub severity: Severity,
    /// Short technical detail (English). Contains no personal paths.
    pub detail: String,
    /// Optional count for the localized text.
    pub count: Option<u32>,
}

/// A `.pacnew`/`.pacsave` file below `/etc`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConfigFileHint {
    /// Absolute path below `/etc`.
    pub path: String,
    /// `pacnew` or `pacsave`.
    pub kind: String,
    #[ts(type = "number | null")]
    pub modified_at: Option<Timestamp>,
}

/// Snapper/Btrfs snapshot support. Only reports what could be verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SnapshotSupport {
    /// Root filesystem is Btrfs.
    pub btrfs_root: bool,
    /// `snapper` is installed.
    pub snapper_installed: bool,
    /// A snapper configuration for `/` exists (name of the config, usually `root`).
    pub root_config: Option<String>,
    /// `snap-pac` creates pre/post snapshots for every pacman transaction.
    pub snap_pac_active: bool,
    /// `true` when cachyos-center can request a snapshot before an upgrade.
    pub can_request_snapshot: bool,
}

/// Aggregated health report (UI health center, dashboard card, MCP `health_get`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HealthReport {
    pub items: Vec<HealthItem>,
    pub pacnew_count: u32,
    pub pacsave_count: u32,
    pub config_files: Vec<ConfigFileHint>,
    pub lock: LockStatus,
    pub reboot_recommended: bool,
    pub reboot_reasons: Vec<String>,
    #[ts(type = "number | null")]
    pub package_cache_bytes: Option<u64>,
    pub snapshot: SnapshotSupport,
    pub offline_update: OfflineUpdateStatus,
    pub external_updaters: Vec<ExternalUpdater>,
    /// Reasons that currently block an unattended update preparation.
    pub update_blockers: Vec<String>,
    #[ts(type = "number")]
    pub collected_at: Timestamp,
}

impl HealthReport {
    /// Highest severity over all items.
    pub fn worst(&self) -> Option<Severity> {
        self.items.iter().map(|i| i.severity).max()
    }
}
