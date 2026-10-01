//! Structured tool output (MCP `structuredContent`) and its JSON schemas.
//!
//! These types are the MCP contract and deliberately independent of the
//! internal models: fields are selected explicitly (e.g. `health_get` has no
//! configuration file paths). Conventions: camelCase field names, times in
//! Unix seconds (UTC), sizes in bytes, enumerations as the camelCase strings
//! of the core model. Every output has `truncated`, which is `true` when lists
//! or overlong texts were shortened to respect the output size limit.

use cachyos_center_core::health::HealthReport;
use cachyos_center_core::history::HistoryEntry;
use cachyos_center_core::package::{PackagePage, PackageSummary};
use cachyos_center_core::system::{LockStatus, SystemSummary};
use cachyos_center_core::updates::{
    CheckStatus, STALE_AFTER_SECS, UpdateCandidate, UpdateCheckResult,
};
use cachyos_center_core::{AppError, ErrorCode, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::args::encode_cursor;

/// Error code in the structured content of an error result (`isError: true`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McpErrorCode {
    /// MCP access is disabled in cachyos-center, a required component is missing, or the call timed out.
    Unavailable,
    /// The package manager is busy (e.g. a running pacman transaction).
    Busy,
    /// The data is too old to be used.
    Stale,
    /// The platform or library version is not supported.
    Unsupported,
    /// Unexpected internal error.
    Internal,
    /// The tool arguments are invalid; correct them and call again.
    InvalidInput,
}

impl McpErrorCode {
    /// Reduction of an internal error code to the public MCP codes
    /// ([`ErrorCode::mcp_code`]).
    pub fn from_app(code: ErrorCode) -> Self {
        match code.mcp_code() {
            "UNAVAILABLE" => Self::Unavailable,
            "BUSY" => Self::Busy,
            "STALE" => Self::Stale,
            "UNSUPPORTED" => Self::Unsupported,
            _ => Self::Internal,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "UNAVAILABLE",
            Self::Busy => "BUSY",
            Self::Stale => "STALE",
            Self::Unsupported => "UNSUPPORTED",
            Self::Internal => "INTERNAL",
            Self::InvalidInput => "INVALID_INPUT",
        }
    }
}

/// Structured content of an error result (`isError: true`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolErrorBody {
    /// Stable error code.
    pub code: McpErrorCode,
    /// Technical English description (sanitized).
    pub message: String,
}

impl ToolErrorBody {
    pub fn from_app(error: &AppError) -> Self {
        Self {
            code: McpErrorCode::from_app(error.code),
            message: error.message.clone(),
        }
    }
}

/// Removal of list entries until the serialized output fits the size limit.
pub(crate) trait Shrink {
    /// Removes some list entries and sets `truncated`. Returns `false` when
    /// nothing is left to remove.
    fn shrink(&mut self) -> bool;
}

/// Drops about a quarter (at least one) of the entries from the end.
fn shrink_vec<T>(items: &mut Vec<T>) -> bool {
    if items.is_empty() {
        return false;
    }
    let remove = (items.len() / 4).max(1);
    items.truncate(items.len() - remove);
    true
}

/// Shrinks the longest of the given lists.
fn shrink_longest(lists: &mut [&mut dyn ShrinkList]) -> bool {
    let longest = lists
        .iter_mut()
        .filter(|l| l.len() > 0)
        .max_by_key(|l| l.len());
    match longest {
        Some(list) => list.shrink_list(),
        None => false,
    }
}

trait ShrinkList {
    fn len(&self) -> usize;
    fn shrink_list(&mut self) -> bool;
}

impl<T> ShrinkList for Vec<T> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn shrink_list(&mut self) -> bool {
        shrink_vec(self)
    }
}

/// camelCase wire name of a core enumeration value.
fn wire<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        _ => String::new(),
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

// ---- system_get_summary -------------------------------------------------------

/// Output of `system_get_summary`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SystemSummaryOutput {
    /// Operating system (`PRETTY_NAME` of os-release).
    pub os: String,
    pub is_cachyos: bool,
    /// Release of the running kernel.
    pub kernel: String,
    pub uptime_seconds: u64,
    /// CPU model.
    pub cpu: String,
    /// GPUs as "vendor model".
    pub gpus: Vec<String>,
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    /// Filesystem type of `/`.
    pub root_filesystem: String,
    pub root_total_bytes: u64,
    pub root_available_bytes: u64,
    /// Session kind: `hyprland`, `otherWayland`, `x11`, `tty` or `unknown`.
    pub session: String,
    /// Desktop (`XDG_CURRENT_DESKTOP`), if known.
    pub desktop: Option<String>,
    /// Installed pacman version, if known.
    pub pacman_version: Option<String>,
    /// Package functions (libalpm) are available.
    pub package_backend_ready: bool,
    pub truncated: bool,
}

impl From<SystemSummary> for SystemSummaryOutput {
    fn from(s: SystemSummary) -> Self {
        Self {
            os: s.os,
            is_cachyos: s.is_cachyos,
            kernel: s.kernel,
            uptime_seconds: s.uptime_seconds,
            cpu: s.cpu,
            gpus: s.gpus,
            memory_total_bytes: s.memory_total_bytes,
            memory_available_bytes: s.memory_available_bytes,
            root_filesystem: s.root_filesystem,
            root_total_bytes: s.root_total_bytes,
            root_available_bytes: s.root_available_bytes,
            session: wire(&s.session),
            desktop: s.desktop,
            pacman_version: s.pacman_version,
            package_backend_ready: s.package_backend_ready,
            truncated: false,
        }
    }
}

impl Shrink for SystemSummaryOutput {
    fn shrink(&mut self) -> bool {
        let shrunk = shrink_vec(&mut self.gpus);
        self.truncated |= shrunk;
        shrunk
    }
}

// ---- updates_list ---------------------------------------------------------------

/// One available or held-back update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateItem {
    pub package: String,
    /// Sync repository providing the new version.
    pub repository: String,
    pub old_version: String,
    pub new_version: String,
    /// Classification: `kernel`, `driver`, `firmware`, `microcode`, `coreSystem`, `packageManager`, `desktopSession`, `heldBack`, `rebootRecommended`.
    pub flags: Vec<String>,
    /// Download size in bytes (`null` when unknown or already cached).
    pub download_size: Option<u64>,
}

impl From<UpdateCandidate> for UpdateItem {
    fn from(u: UpdateCandidate) -> Self {
        Self {
            package: u.package_id.name,
            repository: u.package_id.repository,
            old_version: u.old_version,
            new_version: u.new_version,
            flags: u.flags.iter().map(wire).collect(),
            download_size: u.download_size,
        }
    }
}

/// Code and message of an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInfo {
    pub code: McpErrorCode,
    pub message: String,
}

/// Output of `updates_list`: the result of the last update check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesListOutput {
    /// State of the last check: `neverChecked`, `fresh`, `stale`, `failed`, `prerequisiteMissing` or `unsupported`.
    pub status: String,
    /// Time of the last successful check (`null` if there was none).
    pub checked_at: Option<Timestamp>,
    /// Time of the last check attempt, successful or not.
    pub attempted_at: Option<Timestamp>,
    /// Age of the last successful check in seconds (`null` if there was none).
    pub age_seconds: Option<i64>,
    /// `true` unless `status` is `fresh`; stale data must not be presented as the current state.
    pub stale: bool,
    /// Explanation when the data is not fresh.
    pub note: Option<String>,
    /// Number of available updates (before any truncation).
    pub update_count: u32,
    /// Updates a full system upgrade would install.
    pub updates: Vec<UpdateItem>,
    /// Number of held-back updates (before any truncation).
    pub held_back_count: u32,
    /// Newer versions held back by `IgnorePkg`/`IgnoreGroup` (e.g. offline.conf).
    pub held_back: Vec<UpdateItem>,
    /// Total download size in bytes, if known.
    pub total_download_size: Option<u64>,
    /// A reboot is recommended after installing these updates.
    pub reboot_recommended: bool,
    /// Error of the most recent failed check attempt.
    pub last_error: Option<ErrorInfo>,
    /// Missing programs (package names) when `status` is `prerequisiteMissing`.
    pub missing_prerequisites: Vec<String>,
    pub truncated: bool,
}

impl UpdatesListOutput {
    pub fn new(result: UpdateCheckResult, now: Timestamp) -> Self {
        let age_seconds = result.checked_at.map(|t| now.saturating_sub(t).max(0));
        let note = staleness_note(&result, age_seconds);
        Self {
            status: wire(&result.status),
            checked_at: result.checked_at,
            attempted_at: result.attempted_at,
            age_seconds,
            stale: result.status != CheckStatus::Fresh,
            note,
            update_count: count(result.updates.len()),
            updates: result.updates.into_iter().map(UpdateItem::from).collect(),
            held_back_count: count(result.held_back.len()),
            held_back: result.held_back.into_iter().map(UpdateItem::from).collect(),
            total_download_size: result.total_download_size,
            reboot_recommended: result.reboot_recommended,
            last_error: result.error.map(|e| ErrorInfo {
                code: McpErrorCode::from_app(e.code),
                message: e.message,
            }),
            missing_prerequisites: result.missing_prerequisites,
            truncated: false,
        }
    }
}

/// Explanation for every status except `fresh`.
fn staleness_note(result: &UpdateCheckResult, age_seconds: Option<i64>) -> Option<String> {
    let age = |secs: i64| -> String {
        let hours = secs / 3600;
        if hours >= 48 {
            format!("{} days", hours / 24)
        } else if hours >= 1 {
            format!("{hours} hours")
        } else {
            format!("{} minutes", secs / 60)
        }
    };
    let last = match age_seconds {
        Some(secs) => format!("The last successful check was {} ago.", age(secs)),
        None => "There is no successful check yet.".to_string(),
    };
    let text = match result.status {
        CheckStatus::Fresh => return None,
        CheckStatus::NeverChecked => "No update check has been run yet. The empty list does not \
            mean that the system is up to date; the user can run a check in cachyos-center."
            .to_string(),
        CheckStatus::Stale => format!(
            "The data is outdated (older than {} hours): {last} Newer updates may be available; \
             do not present this list as the current state. The user can run a new check in \
             cachyos-center.",
            STALE_AFTER_SECS / 3600
        ),
        CheckStatus::Failed => format!(
            "The most recent update check failed (see lastError). {last} The list reflects the \
             last successful check, if any, and may be outdated."
        ),
        CheckStatus::PrerequisiteMissing => format!(
            "The update check cannot run because required programs are missing ({}). No current \
             update information is available.",
            if result.missing_prerequisites.is_empty() {
                "unknown".to_string()
            } else {
                result.missing_prerequisites.join(", ")
            }
        ),
        CheckStatus::Unsupported => "Package functions are disabled on this system (the libalpm \
            bridge is unavailable or incompatible). No update information is available."
            .to_string(),
    };
    Some(text)
}

impl Shrink for UpdatesListOutput {
    fn shrink(&mut self) -> bool {
        let shrunk = shrink_longest(&mut [&mut self.updates, &mut self.held_back]);
        self.truncated |= shrunk;
        shrunk
    }
}

// ---- packages_search / packages_installed ------------------------------------------

/// One repository search hit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub name: String,
    /// Sync repository (e.g. `core`, `extra`, `cachyos-v3`).
    pub repository: Option<String>,
    /// Origin: `repo` (configured pacman repositories only).
    pub origin: String,
    /// Version in the repository.
    pub version: Option<String>,
    /// Installed version, `null` when not installed.
    pub installed_version: Option<String>,
    /// Installed and a newer version is available.
    pub update_available: bool,
    pub description: String,
}

impl From<PackageSummary> for SearchHit {
    fn from(p: PackageSummary) -> Self {
        Self {
            origin: wire(&p.origin),
            name: p.name,
            repository: p.repository,
            version: p.available_version,
            installed_version: p.installed_version,
            update_available: p.update_available,
            description: p.description,
        }
    }
}

/// Output of `packages_search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PackagesSearchOutput {
    /// Normalized query.
    pub query: String,
    /// Requested maximum number of results.
    pub limit: u32,
    /// Hits, best matches first.
    pub items: Vec<SearchHit>,
    pub truncated: bool,
}

impl PackagesSearchOutput {
    pub fn new(query: String, limit: u32, hits: Vec<PackageSummary>) -> Self {
        Self {
            query,
            limit,
            items: hits
                .into_iter()
                .take(limit as usize)
                .map(SearchHit::from)
                .collect(),
            truncated: false,
        }
    }
}

impl Shrink for PackagesSearchOutput {
    fn shrink(&mut self) -> bool {
        let shrunk = shrink_vec(&mut self.items);
        self.truncated |= shrunk;
        shrunk
    }
}

/// One installed package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPackage {
    pub name: String,
    /// Installed version.
    pub version: Option<String>,
    /// Sync repository providing the package (`null` for foreign packages).
    pub repository: Option<String>,
    /// `repo` or `localOrAur` (in no configured repository; AUR origin cannot be determined reliably).
    pub origin: String,
    /// `explicit` or `dependency`.
    pub install_reason: Option<String>,
    /// A newer version is available in a sync repository.
    pub update_available: bool,
    /// Version in the sync repositories, if any.
    pub available_version: Option<String>,
    /// Held back by `IgnorePkg`/`IgnoreGroup`.
    pub ignored: bool,
    pub installed_size: Option<u64>,
    pub description: String,
}

impl From<PackageSummary> for InstalledPackage {
    fn from(p: PackageSummary) -> Self {
        Self {
            origin: wire(&p.origin),
            install_reason: p.install_reason.as_ref().map(wire),
            name: p.name,
            version: p.installed_version,
            repository: p.repository,
            update_available: p.update_available,
            available_version: p.available_version,
            ignored: p.ignored,
            installed_size: p.installed_size,
            description: p.description,
        }
    }
}

/// Output of `packages_installed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PackagesInstalledOutput {
    /// Number of installed packages matching the filter.
    pub total: u32,
    /// Position of the first item of this page.
    pub offset: u32,
    pub items: Vec<InstalledPackage>,
    /// Cursor for the next page (`null` on the last page); pass it as `cursor` with the same `query`.
    pub next_cursor: Option<String>,
    pub truncated: bool,
    #[serde(skip)]
    #[schemars(skip)]
    filter_key: String,
}

impl PackagesInstalledOutput {
    pub fn new(page: PackagePage, filter_key: &str) -> Self {
        let mut out = Self {
            total: page.total,
            offset: page.offset,
            items: page.items.into_iter().map(InstalledPackage::from).collect(),
            next_cursor: None,
            truncated: false,
            filter_key: filter_key.to_string(),
        };
        out.update_cursor();
        out
    }

    /// The next page starts right after the last returned item, so items
    /// removed by truncation are delivered on the next page.
    fn update_cursor(&mut self) {
        let next = self.offset.saturating_add(count(self.items.len()));
        self.next_cursor = (next < self.total).then(|| encode_cursor(next, &self.filter_key));
    }
}

impl Shrink for PackagesInstalledOutput {
    fn shrink(&mut self) -> bool {
        let shrunk = shrink_vec(&mut self.items);
        if shrunk {
            self.truncated = true;
            self.update_cursor();
        }
        shrunk
    }
}

// ---- operations_recent ----------------------------------------------------------------

/// Sanitized summary of one operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationSummary {
    pub id: String,
    /// `app` (cachyos-center), `timer` (automatic) or `externalPacman` (transaction from the pacman log).
    pub source: String,
    /// `updateCheck`, `systemUpgrade`, `install`, `remove` or `autoUpdatePrepare`.
    pub kind: Option<String>,
    /// `user` or `timer`.
    pub origin: Option<String>,
    /// Final or current state, e.g. `succeeded`, `failed`, `needsAttention`.
    pub state: Option<String>,
    /// Outcome of external transactions: `completed`, `failed`, `interrupted` or `unknown`.
    pub log_outcome: Option<String>,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    /// Short summary (sanitized, no file paths).
    pub summary: String,
    /// Error code of a failed operation.
    pub error_code: Option<String>,
    pub installed: u32,
    pub upgraded: u32,
    pub removed: u32,
    pub downgraded: u32,
    /// Up to 20 affected package names.
    pub packages: Vec<String>,
    /// The result of the operation is not certain.
    pub outcome_unknown: bool,
}

impl From<HistoryEntry> for OperationSummary {
    fn from(e: HistoryEntry) -> Self {
        Self {
            id: e.id,
            source: wire(&e.source),
            kind: e.kind.as_ref().map(wire),
            origin: e.origin.as_ref().map(wire),
            state: e.state.as_ref().map(wire),
            log_outcome: e.log_outcome.as_ref().map(wire),
            started_at: e.started_at,
            ended_at: e.ended_at,
            summary: e.summary,
            error_code: e.error_code.map(|c| c.as_str().to_string()),
            installed: e.installed,
            upgraded: e.upgraded,
            removed: e.removed,
            downgraded: e.downgraded,
            packages: e.packages.into_iter().take(20).collect(),
            outcome_unknown: e.outcome_unknown,
        }
    }
}

/// Output of `operations_recent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationsRecentOutput {
    /// Operations, newest first.
    pub items: Vec<OperationSummary>,
    pub truncated: bool,
}

impl OperationsRecentOutput {
    pub fn new(entries: Vec<HistoryEntry>, limit: u32) -> Self {
        Self {
            items: entries
                .into_iter()
                .take(limit as usize)
                .map(OperationSummary::from)
                .collect(),
            truncated: false,
        }
    }
}

impl Shrink for OperationsRecentOutput {
    fn shrink(&mut self) -> bool {
        let shrunk = shrink_vec(&mut self.items);
        self.truncated |= shrunk;
        shrunk
    }
}

// ---- health_get -----------------------------------------------------------------------

/// One health hint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthItemOutput {
    /// Stable kind, e.g. `pacnewFiles`, `rebootRecommended`, `packageManagerLocked`, `lowDiskSpace`.
    pub kind: String,
    /// `info`, `warning` or `critical`.
    pub severity: String,
    /// Short technical detail (English, no file paths).
    pub detail: String,
    pub count: Option<u32>,
}

/// State of the pacman database lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LockOutput {
    /// `free` or `locked`.
    pub state: String,
    /// Since when the lock exists (only when locked).
    pub since: Option<Timestamp>,
    /// A package manager process is running (`null` when unknown).
    pub holder_running: Option<bool>,
}

impl From<LockStatus> for LockOutput {
    fn from(lock: LockStatus) -> Self {
        match lock {
            LockStatus::Free => Self {
                state: "free".into(),
                since: None,
                holder_running: None,
            },
            LockStatus::Locked {
                since,
                holder_running,
            } => Self {
                state: "locked".into(),
                since,
                holder_running,
            },
        }
    }
}

/// `pacman-offline` state (without file paths).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OfflineUpdateOutput {
    /// `pacman-offline` is installed.
    pub installed: bool,
    /// An update is prepared and will be installed on the next reboot.
    pub prepared: bool,
    pub prepare_timer_active: bool,
    pub reboot_timer_active: bool,
    /// `offline.conf` is included in the pacman configuration.
    pub offline_conf_included: bool,
    /// Packages held back for online updates by `offline.conf`.
    pub offline_conf_held_packages: Vec<String>,
    /// The configuration could be inspected completely.
    pub configuration_verifiable: bool,
}

/// Another update mechanism found on the system (systemd unit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalUpdaterOutput {
    /// Unit name, e.g. `arch-update.timer`.
    pub name: String,
    /// `system` or `user`.
    pub scope: String,
    /// Enabled or running.
    pub active: bool,
    pub description: String,
}

/// Snapshot support (Btrfs/snapper), as far as it could be verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotOutput {
    pub btrfs_root: bool,
    pub snapper_installed: bool,
    /// Name of the snapper configuration for `/`.
    pub root_config: Option<String>,
    pub snap_pac_active: bool,
    pub can_request_snapshot: bool,
}

/// Output of `health_get`. Contains no file paths and no file contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthOutput {
    /// Highest severity over all items (`null` when there are none).
    pub worst_severity: Option<String>,
    /// Hints, most severe first.
    pub items: Vec<HealthItemOutput>,
    /// Number of `.pacnew` files below `/etc`.
    pub pacnew_count: u32,
    /// Number of `.pacsave` files below `/etc`.
    pub pacsave_count: u32,
    pub lock: LockOutput,
    pub reboot_recommended: bool,
    pub reboot_reasons: Vec<String>,
    pub offline_update: OfflineUpdateOutput,
    pub external_updaters: Vec<ExternalUpdaterOutput>,
    /// Reasons that currently block an unattended update preparation.
    pub update_blockers: Vec<String>,
    /// Size of the package cache in bytes, if known.
    pub package_cache_bytes: Option<u64>,
    pub snapshot: SnapshotOutput,
    pub collected_at: Timestamp,
    pub truncated: bool,
}

impl From<HealthReport> for HealthOutput {
    fn from(r: HealthReport) -> Self {
        // `r.config_files` (absolute paths below /etc) is deliberately dropped.
        Self {
            worst_severity: r.worst().as_ref().map(wire),
            items: r
                .items
                .iter()
                .map(|i| HealthItemOutput {
                    kind: wire(&i.kind),
                    severity: wire(&i.severity),
                    detail: i.detail.clone(),
                    count: i.count,
                })
                .collect(),
            pacnew_count: r.pacnew_count,
            pacsave_count: r.pacsave_count,
            lock: r.lock.into(),
            reboot_recommended: r.reboot_recommended,
            reboot_reasons: r.reboot_reasons.iter().map(|x| x.describe()).collect(),
            offline_update: OfflineUpdateOutput {
                installed: r.offline_update.installed,
                prepared: r.offline_update.prepared,
                prepare_timer_active: r.offline_update.prepare_timer_active,
                reboot_timer_active: r.offline_update.reboot_timer_active,
                offline_conf_included: r.offline_update.offline_conf_included,
                offline_conf_held_packages: r.offline_update.offline_conf_ignored,
                configuration_verifiable: r.offline_update.configuration_verifiable,
            },
            external_updaters: r
                .external_updaters
                .into_iter()
                .map(|u| ExternalUpdaterOutput {
                    name: u.name,
                    scope: u.scope,
                    active: u.active,
                    description: u.description,
                })
                .collect(),
            update_blockers: r
                .update_blockers
                .iter()
                .map(|b| b.describe().to_string())
                .collect(),
            package_cache_bytes: r.package_cache_bytes,
            snapshot: SnapshotOutput {
                btrfs_root: r.snapshot.btrfs_root,
                snapper_installed: r.snapshot.snapper_installed,
                root_config: r.snapshot.root_config,
                snap_pac_active: r.snapshot.snap_pac_active,
                can_request_snapshot: r.snapshot.can_request_snapshot,
            },
            collected_at: r.collected_at,
            truncated: false,
        }
    }
}

impl Shrink for HealthOutput {
    fn shrink(&mut self) -> bool {
        let shrunk = shrink_longest(&mut [
            &mut self.items,
            &mut self.reboot_reasons,
            &mut self.update_blockers,
            &mut self.external_updaters,
            &mut self.offline_update.offline_conf_held_packages,
        ]);
        self.truncated |= shrunk;
        shrunk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_follow_the_core_reduction() {
        for (code, expected) in [
            (ErrorCode::Unavailable, McpErrorCode::Unavailable),
            (ErrorCode::NotFound, McpErrorCode::Unavailable),
            (ErrorCode::Offline, McpErrorCode::Unavailable),
            (ErrorCode::Busy, McpErrorCode::Busy),
            (ErrorCode::Conflict, McpErrorCode::Busy),
            (ErrorCode::Stale, McpErrorCode::Stale),
            (ErrorCode::PlanChanged, McpErrorCode::Stale),
            (ErrorCode::Unsupported, McpErrorCode::Unsupported),
            (ErrorCode::Internal, McpErrorCode::Internal),
            (ErrorCode::InvalidInput, McpErrorCode::Internal),
        ] {
            let mapped = McpErrorCode::from_app(code);
            assert_eq!(mapped, expected, "{code}");
            assert_eq!(mapped.as_str(), code.mcp_code());
            assert_eq!(
                serde_json::to_value(mapped).unwrap(),
                serde_json::Value::String(mapped.as_str().into())
            );
        }
        assert_eq!(
            McpErrorCode::InvalidInput.as_str(),
            ErrorCode::InvalidInput.as_str()
        );
    }

    #[test]
    fn shrink_removes_from_the_longest_list() {
        let mut a = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mut b = vec![1, 2];
        assert!(shrink_longest(&mut [&mut a, &mut b]));
        assert_eq!(a.len(), 6);
        assert_eq!(b.len(), 2);
        let mut empty: Vec<u8> = Vec::new();
        assert!(!shrink_longest(&mut [&mut empty]));
    }
}
