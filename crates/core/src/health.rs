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

/// Why a reboot is recommended. The UI renders localized texts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum RebootReason {
    /// The modules of the running kernel were removed by a kernel update.
    KernelReplaced,
    /// Packages that usually need a reboot were updated since boot.
    UpdatedSinceBoot { packages: Vec<String> },
}

impl RebootReason {
    /// English description (MCP output, logs).
    pub fn describe(&self) -> String {
        match self {
            Self::KernelReplaced => "running kernel was replaced by an update".to_string(),
            Self::UpdatedSinceBoot { packages } => {
                format!("updated since boot: {}", packages.join(", "))
            }
        }
    }
}

/// Stable identifiers of reasons that block an unattended update preparation.
/// The UI renders localized texts for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateBlocker {
    PackageManagerBusy,
    LastTransactionIncomplete,
    LastOperationNeedsAttention,
    PackageBackendUnavailable,
    PrerequisitesMissing,
    ExternalPrepareTimer,
    NewsUnread,
    NewsUnavailable,
    NewsDisabled,
    LowDiskSpace,
}

impl UpdateBlocker {
    /// English description (MCP output, logs).
    pub fn describe(self) -> &'static str {
        match self {
            Self::PackageManagerBusy => "package manager is busy (db.lck)",
            Self::LastTransactionIncomplete => "last pacman transaction did not complete",
            Self::LastOperationNeedsAttention => "last operation needs attention",
            Self::PackageBackendUnavailable => "package functions are disabled",
            Self::PrerequisitesMissing => "update check prerequisites are missing",
            Self::ExternalPrepareTimer => {
                "pacman-offline-prepare.timer is active (externally managed)"
            }
            Self::NewsUnread => "unread news",
            Self::NewsUnavailable => "news check not possible",
            Self::NewsDisabled => "news check disabled",
            Self::LowDiskSpace => "not enough free disk space",
        }
    }
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
    pub reboot_reasons: Vec<RebootReason>,
    #[ts(type = "number | null")]
    pub package_cache_bytes: Option<u64>,
    pub snapshot: SnapshotSupport,
    pub offline_update: OfflineUpdateStatus,
    pub external_updaters: Vec<ExternalUpdater>,
    /// Reasons that currently block an unattended update preparation.
    pub update_blockers: Vec<UpdateBlocker>,
    #[ts(type = "number")]
    pub collected_at: Timestamp,
}

impl HealthReport {
    /// Highest severity over all items.
    pub fn worst(&self) -> Option<Severity> {
        self.items.iter().map(|i| i.severity).max()
    }
}
