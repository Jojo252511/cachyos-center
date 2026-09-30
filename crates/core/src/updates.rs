//! Update candidates and the result of an update check.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Timestamp;
use crate::error::AppError;
use crate::package::PackageId;
use crate::plan::TransactionPlan;

/// Classification of an update. Used for badges and restart hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateFlag {
    /// Linux kernel image.
    Kernel,
    /// Kernel module, GPU driver or Mesa/Vulkan stack.
    Driver,
    /// Firmware package.
    Firmware,
    /// CPU microcode.
    Microcode,
    /// Core system library or init (glibc, systemd, dbus).
    CoreSystem,
    /// Package manager or keyring.
    PackageManager,
    /// Component of the running desktop session (e.g. Hyprland).
    DesktopSession,
    /// Held back by `IgnorePkg`/`IgnoreGroup`.
    HeldBack,
    /// A reboot is recommended after installing this update.
    RebootRecommended,
}

/// One available update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateCandidate {
    pub package_id: PackageId,
    pub old_version: String,
    pub new_version: String,
    /// Download size if the package is not cached yet. `None` when unknown.
    #[ts(type = "number | null")]
    pub download_size: Option<u64>,
    pub flags: Vec<UpdateFlag>,
}

/// Status of the most recent update check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CheckStatus {
    /// No successful check yet. The UI must not show "0 updates".
    NeverChecked,
    /// Result of a recent successful check.
    Fresh,
    /// The last successful check is older than the staleness threshold.
    Stale,
    /// The most recent check failed; `updates` reflects the last successful check (if any).
    Failed,
    /// `checkupdates` (pacman-contrib) or `fakeroot` is missing.
    PrerequisiteMissing,
    /// Package functions are disabled (libalpm bridge unavailable or incompatible).
    Unsupported,
}

/// Result of an update check including uncertainty information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateCheckResult {
    pub status: CheckStatus,
    /// Time of the last *successful* synchronization of the isolated database.
    #[ts(type = "number | null")]
    pub checked_at: Option<Timestamp>,
    /// Time of the last attempt (successful or not).
    #[ts(type = "number | null")]
    pub attempted_at: Option<Timestamp>,
    /// Updates that a full `pacman -Syu` would install (derived from `plan`).
    pub updates: Vec<UpdateCandidate>,
    /// Complete upgrade plan computed from the isolated database. Its digest is
    /// the preview the user confirms; the helper recomputes it before commit.
    pub plan: Option<TransactionPlan>,
    /// Newer versions that pacman holds back (`IgnorePkg`, e.g. via `offline.conf`).
    pub held_back: Vec<UpdateCandidate>,
    #[ts(type = "number | null")]
    pub total_download_size: Option<u64>,
    pub reboot_recommended: bool,
    /// Error of the most recent failed attempt.
    pub error: Option<AppError>,
    /// Missing programs for [`CheckStatus::PrerequisiteMissing`] (package names).
    pub missing_prerequisites: Vec<String>,
}

impl UpdateCheckResult {
    /// Empty result for [`CheckStatus::NeverChecked`] and similar states.
    pub fn empty(status: CheckStatus) -> Self {
        Self {
            status,
            checked_at: None,
            attempted_at: None,
            updates: Vec::new(),
            plan: None,
            held_back: Vec::new(),
            total_download_size: None,
            reboot_recommended: false,
            error: None,
            missing_prerequisites: Vec::new(),
        }
    }

    /// Whether the update list may be presented as the current state.
    pub fn is_current(&self) -> bool {
        self.status == CheckStatus::Fresh
    }
}

/// Age after which a check result is reported as stale (6 hours).
pub const STALE_AFTER_SECS: i64 = 6 * 60 * 60;

/// Maximum age of the repository data for starting an installation (1 hour).
pub const INSTALL_MAX_DATA_AGE_SECS: i64 = 60 * 60;

/// Decides the status of a successful check result based on its age.
pub fn freshness(checked_at: Timestamp, now: Timestamp) -> CheckStatus {
    if now.saturating_sub(checked_at) > STALE_AFTER_SECS || checked_at > now + 300 {
        CheckStatus::Stale
    } else {
        CheckStatus::Fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_thresholds() {
        assert_eq!(freshness(1000, 1000), CheckStatus::Fresh);
        assert_eq!(freshness(1000, 1000 + STALE_AFTER_SECS), CheckStatus::Fresh);
        assert_eq!(freshness(1000, 1001 + STALE_AFTER_SECS), CheckStatus::Stale);
        // Timestamps from the future (clock jumps) are not trusted.
        assert_eq!(freshness(10_000, 1000), CheckStatus::Stale);
    }

    #[test]
    fn empty_result_is_not_current() {
        let r = UpdateCheckResult::empty(CheckStatus::NeverChecked);
        assert!(!r.is_current());
        assert!(r.updates.is_empty());
    }
}
