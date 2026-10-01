//! Application core of cachyos-center.
//!
//! [`AppCore`] is the single read service used by the desktop UI and the MCP
//! server ([`ReadApi`]). It combines system information, the package service,
//! health checks, news, settings and the operation history. It never changes
//! the package set; package changes go through the privileged helper.

pub mod activity;
pub mod autoupdate;
pub mod diagnostic;
pub mod health;
pub mod history;
pub mod settings;

use std::path::Path;

use cachyos_center_core::dashboard::Dashboard;
use cachyos_center_core::health::HealthReport;
use cachyos_center_core::history::HistoryEntry;
use cachyos_center_core::hyprland::HyprlandInfo;
use cachyos_center_core::news::NewsStatus;
use cachyos_center_core::operation::{Operation, OperationKind, OperationOrigin, OperationState};
use cachyos_center_core::package::{
    CatalogQuery, InstalledQuery, PackageOrigin, PackagePage, PackageRecord, PackageRef,
    PackageSummary, RepositoryInfo,
};
use cachyos_center_core::paths::{SYSTEM_OPERATIONS_DIR, UserDirs};
use cachyos_center_core::plan::TransactionPlan;
use cachyos_center_core::policy::AutoUpdateStatus;
use cachyos_center_core::sanitize::SanitizeContext;
use cachyos_center_core::settings::Settings;
use cachyos_center_core::system::{PacmanStatus, SystemInfo, SystemSummary};
use cachyos_center_core::updates::{CheckStatus, UpdateCheckResult};
use cachyos_center_core::{AppError, AppResult, ErrorCode, Timestamp, now};
use cachyos_center_packages::PackageService;
use cachyos_center_system::news::{self, NewsCache};
use serde::{Deserialize, Serialize};

pub use history::HistoryStore;
pub use settings::SettingsStore;

/// Maximum age of the news cache before a refresh is attempted (1 hour).
pub const NEWS_MAX_AGE: i64 = 60 * 60;

/// Read-only interface shared by the GUI backend and the MCP server.
pub trait ReadApi: Send + Sync {
    fn system_summary(&self) -> AppResult<SystemSummary>;
    /// Last update check (never triggers network access).
    fn updates(&self) -> AppResult<UpdateCheckResult>;
    fn search(&self, query: &CatalogQuery) -> AppResult<Vec<PackageSummary>>;
    fn installed(&self, query: &InstalledQuery) -> AppResult<PackagePage>;
    fn recent_activity(&self, limit: u32) -> AppResult<Vec<HistoryEntry>>;
    fn health(&self) -> AppResult<HealthReport>;
    /// Current user settings (e.g. whether the MCP server may answer).
    fn user_settings(&self) -> Settings;
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewsAck {
    acknowledged_until: Option<Timestamp>,
}

/// Application core.
#[derive(Debug, Clone)]
pub struct AppCore {
    dirs: UserDirs,
    packages: PackageService,
    settings: SettingsStore,
    history: HistoryStore,
}

impl AppCore {
    /// Core for the current user (XDG directories from the environment).
    pub fn from_env() -> AppResult<Self> {
        let dirs = UserDirs::from_env().ok_or_else(|| {
            AppError::unavailable(
                "cannot determine the user directories (HOME/XDG variables missing)",
            )
        })?;
        let packages = PackageService::new(dirs.check_db(), dirs.check_state());
        Ok(Self::with_parts(dirs, packages))
    }

    pub fn with_parts(dirs: UserDirs, packages: PackageService) -> Self {
        Self {
            settings: SettingsStore::new(dirs.settings_file()),
            history: HistoryStore::new(dirs.history_db()),
            dirs,
            packages,
        }
    }

    pub fn dirs(&self) -> &UserDirs {
        &self.dirs
    }

    pub fn packages(&self) -> &PackageService {
        &self.packages
    }

    pub fn history(&self) -> &HistoryStore {
        &self.history
    }

    pub fn settings(&self) -> Settings {
        self.settings.load()
    }

    pub fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        self.settings.save(settings)
    }

    pub fn pacman_status(&self) -> PacmanStatus {
        let backend = self.packages.backend_status();
        let installed = self.packages.installed_all().unwrap_or_default();
        let foreign = installed
            .iter()
            .filter(|p| p.origin == PackageOrigin::LocalOrAur)
            .count() as u64;
        PacmanStatus {
            pacman_version: installed
                .iter()
                .find(|p| p.name == "pacman")
                .and_then(|p| p.installed_version.clone()),
            backend,
            lock: self.packages.lock_status(),
            last_full_upgrade: self
                .packages
                .log_summary()
                .ok()
                .and_then(|l| l.last_full_upgrade),
            installed_count: installed.len() as u64,
            foreign_count: foreign,
            repositories: self
                .packages
                .config()
                .map(|c| c.repos.iter().map(|r| r.name.clone()).collect())
                .unwrap_or_default(),
        }
    }

    pub fn system_info(&self) -> SystemInfo {
        cachyos_center_system::collect(self.pacman_status())
    }

    pub fn hyprland(&self) -> HyprlandInfo {
        cachyos_center_system::hyprland::info()
    }

    /// Result of the last update check. A package operation that failed or
    /// needs attention *after* that check makes the result stale: the app must
    /// never show "System aktuell" after a failed transaction without a new check.
    pub fn last_check(&self) -> UpdateCheckResult {
        let mut result = self.packages.last_check();
        if let Some(op) = self.failed_operation_after(result.checked_at) {
            if result.status == CheckStatus::Fresh {
                result.status = CheckStatus::Stale;
            }
            result.error = Some(AppError::new(
                ErrorCode::Stale,
                format!(
                    "a package operation ended with state {:?} after the last check; run a new check",
                    op.state
                ),
            ));
        }
        result
    }

    /// Most recent package operation that ended unsuccessfully after `since`.
    fn failed_operation_after(&self, since: Option<Timestamp>) -> Option<Operation> {
        let since = since?;
        self.recent_operations(20)
            .into_iter()
            .filter(|o| {
                matches!(
                    o.kind,
                    OperationKind::SystemUpgrade | OperationKind::Install | OperationKind::Remove
                )
            })
            .filter(|o| o.ended_at.is_some_and(|e| e >= since))
            .max_by_key(|o| o.ended_at.unwrap_or(0))
            .filter(|o| {
                o.commit_started
                    && matches!(
                        o.state,
                        OperationState::Failed | OperationState::NeedsAttention
                    )
            })
    }

    /// Runs an update check (network) and records it in the history.
    pub fn check_updates(&self) -> UpdateCheckResult {
        let started = now();
        let result = self.packages.check_now();
        let mut op = Operation::new(
            new_id(),
            OperationKind::UpdateCheck,
            OperationOrigin::User,
            started,
        );
        let _ = op.transition(OperationState::Checking, started);
        let end = now();
        let _ = match result.status {
            CheckStatus::Fresh => {
                op.summary = format!("{} updates available", result.updates.len());
                op.transition(OperationState::Succeeded, end)
            }
            _ => {
                let err = result.error.clone().unwrap_or_else(|| {
                    AppError::new(ErrorCode::Unavailable, "update check failed")
                });
                op.fail(err, end)
            }
        };
        if let Err(e) = self.history.record(&op) {
            tracing::warn!("cannot record update check: {e}");
        }
        result
    }

    pub fn repositories(&self) -> AppResult<Vec<RepositoryInfo>> {
        self.packages.repositories()
    }

    pub fn details(&self, reference: &PackageRef) -> AppResult<PackageRecord> {
        self.packages.details(reference)
    }

    pub fn plan_install(&self, repository: &str, name: &str) -> AppResult<TransactionPlan> {
        self.packages.plan_install(repository, name)
    }

    pub fn plan_remove(&self, name: &str, recursive: bool) -> AppResult<TransactionPlan> {
        self.packages.plan_remove(name, recursive)
    }

    // ---- News -------------------------------------------------------------

    fn news_ack_path(&self) -> std::path::PathBuf {
        self.dirs.state.join("news-ack.json")
    }

    fn news_ack(&self) -> Option<Timestamp> {
        std::fs::read_to_string(self.news_ack_path())
            .ok()
            .and_then(|t| serde_json::from_str::<NewsAck>(&t).ok())
            .and_then(|a| a.acknowledged_until)
    }

    /// News status. With `refresh`, the feeds are fetched when the cache is
    /// older than [`NEWS_MAX_AGE`] (or `force`).
    pub fn news(&self, refresh: bool, force: bool) -> NewsStatus {
        let settings = self.settings();
        let path = self.dirs.news_cache();
        let mut cache = NewsCache::load(&path);
        let t = now();
        if settings.news_enabled
            && refresh
            && (force
                || cache
                    .fetched_at
                    .is_none_or(|f| t.saturating_sub(f) > NEWS_MAX_AGE))
        {
            cache = news::fetch_all(&cache, t);
            if let Err(e) = cache.save(&path) {
                tracing::warn!("cannot store news cache: {e}");
            }
        }
        news::status(&cache, self.news_ack(), !settings.news_enabled, t)
    }

    /// Marks all news published up to `until` as read.
    pub fn acknowledge_news(&self, until: Timestamp) -> AppResult<NewsStatus> {
        let until = until.min(now());
        let ack = NewsAck {
            acknowledged_until: Some(until.max(self.news_ack().unwrap_or(0))),
        };
        let text = serde_json::to_vec(&ack)
            .map_err(|e| AppError::internal(format!("cannot encode news state: {e}")))?;
        settings::write_private(&self.news_ack_path(), &text)?;
        Ok(self.news(false, false))
    }

    // ---- Activity, health, dashboard --------------------------------------

    /// App history and helper journal, merged by id.
    pub fn recent_operations(&self, limit: u32) -> Vec<Operation> {
        let mut ops = self.history.recent(limit, 0).unwrap_or_default();
        ops.extend(activity::read_journal(
            Path::new(SYSTEM_OPERATIONS_DIR),
            limit as usize,
        ));
        ops
    }

    pub fn activity(&self, limit: u32) -> Vec<HistoryEntry> {
        let app = self.history.recent(limit.max(20), 0).unwrap_or_default();
        let helper = activity::read_journal(Path::new(SYSTEM_OPERATIONS_DIR), 50);
        let log = self.packages.log_summary().ok();
        activity::merge(app, helper, log.as_ref(), now(), limit as usize)
    }

    pub fn health_report(&self) -> HealthReport {
        let kernel = cachyos_center_system::os::kernel_info();
        let log = self.packages.log_summary().ok();
        let ops = self.recent_operations(20);
        let backend = self.packages.backend_status();
        let check = self.last_check();
        let news = self.news(false, false);
        let cache_dirs = self
            .packages
            .config()
            .map(|c| c.cache_dirs.clone())
            .unwrap_or_else(|_| vec!["/var/cache/pacman/pkg".to_string()]);
        health::build(health::HealthInputs {
            config_files: cachyos_center_system::health::config_files(Path::new("/etc")),
            lock: self.packages.lock_status(),
            kernel: &kernel,
            log: log.as_ref(),
            recent_operations: &ops,
            backend: &backend,
            check: &check,
            package_cache_bytes: cachyos_center_system::health::package_cache_bytes(&cache_dirs),
            snapshot: cachyos_center_system::health::snapshot_support(),
            offline: cachyos_center_system::updaters::offline_status(),
            external_updaters: cachyos_center_system::updaters::external_updaters(),
            news: Some(&news),
            root_available: cachyos_center_system::hardware::free_bytes(Path::new("/")),
            now: now(),
        })
    }

    pub fn auto_update_status(&self) -> AutoUpdateStatus {
        autoupdate::status()
    }

    pub fn dashboard(&self) -> Dashboard {
        let system = self.system_info();
        let health = self.health_report();
        let updates = self.last_check();
        let settings = self.settings();
        let next_check_at = (settings.check_interval_hours > 0).then(|| {
            let base = updates
                .attempted_at
                .or(updates.checked_at)
                .unwrap_or_else(now);
            base + i64::from(settings.check_interval_hours) * 3600
        });
        let last_activity = self.activity(1).into_iter().next();
        Dashboard {
            reboot_recommended: summary_reboot(&health, &updates),
            system: system.summary(),
            lock: system.pacman.lock.clone(),
            installed_count: system.pacman.installed_count,
            foreign_count: system.pacman.foreign_count,
            health_worst: health.worst(),
            health_items: health.items.clone(),
            last_activity,
            last_full_upgrade: system.pacman.last_full_upgrade,
            next_check_at,
            offline_update_prepared: health.offline_update.prepared,
            updates,
            collected_at: now(),
        }
    }

    /// Sanitized diagnostic report for review and copying.
    pub fn diagnostic_report(&self) -> String {
        let system = self.system_info();
        let health = self.health_report();
        let updates = self.last_check();
        let auto = self.auto_update_status();
        let activity = self.activity(10);
        diagnostic::build(
            &diagnostic::ReportInputs {
                system: &system,
                health: &health,
                updates: &updates,
                auto_update: &auto,
                activity: &activity,
                now: now(),
            },
            &SanitizeContext::from_env(),
        )
    }

    /// Applies the log retention setting to the history and operation logs.
    pub fn prune(&self) -> AppResult<usize> {
        let days = i64::from(self.settings().log_retention_days);
        let before = now() - days * 24 * 3600;
        let removed = self.history.prune(before)?;
        if let Ok(entries) = std::fs::read_dir(self.dirs.operation_logs()) {
            for entry in entries.flatten() {
                let old = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                    .is_some_and(|d| i64::try_from(d.as_secs()).unwrap_or(0) < before);
                if old {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Ok(removed)
    }
}

fn summary_reboot(health: &HealthReport, updates: &UpdateCheckResult) -> bool {
    health.reboot_recommended || (updates.reboot_recommended && updates.updates.is_empty())
}

/// New random operation id (UUID v4, lower case).
pub fn new_id() -> String {
    let mut bytes = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut bytes);
    }
    if bytes == [0u8; 16] {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        bytes = n.to_le_bytes();
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

impl ReadApi for AppCore {
    fn system_summary(&self) -> AppResult<SystemSummary> {
        Ok(self.system_info().summary())
    }

    fn updates(&self) -> AppResult<UpdateCheckResult> {
        Ok(self.last_check())
    }

    fn search(&self, query: &CatalogQuery) -> AppResult<Vec<PackageSummary>> {
        self.packages.search(query)
    }

    fn installed(&self, query: &InstalledQuery) -> AppResult<PackagePage> {
        self.packages.installed(query)
    }

    fn recent_activity(&self, limit: u32) -> AppResult<Vec<HistoryEntry>> {
        Ok(self.activity(limit))
    }

    fn health(&self) -> AppResult<HealthReport> {
        Ok(self.health_report())
    }

    fn user_settings(&self) -> Settings {
        self.settings()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_valid_uuids() {
        for _ in 0..20 {
            let id = new_id();
            cachyos_center_core::validate::operation_id(&id).unwrap();
        }
        assert_ne!(new_id(), new_id());
    }
}
