//! Health report ("Gesundheitszentrum", dashboard card, MCP `health_get`).

use cachyos_center_core::Timestamp;
use cachyos_center_core::classify;
use cachyos_center_core::health::{
    ConfigFileHint, HealthItem, HealthItemKind, HealthReport, Severity, SnapshotSupport,
};
use cachyos_center_core::history::LogOutcome;
use cachyos_center_core::news::NewsStatus;
use cachyos_center_core::operation::{Operation, OperationKind, OperationState};
use cachyos_center_core::policy::{ExternalUpdater, OfflineUpdateStatus};
use cachyos_center_core::system::{BackendStatus, KernelInfo, LockStatus};
use cachyos_center_core::updates::{CheckStatus, UpdateCheckResult};
use cachyos_center_packages::pacman_log::LogSummary;

/// Package cache size from which a hint is shown (5 GiB).
pub const LARGE_CACHE: u64 = 5 * 1024 * 1024 * 1024;
/// Free space below which a warning is shown (5 GiB) or which is critical (1 GiB).
pub const LOW_SPACE_WARNING: u64 = 5 * 1024 * 1024 * 1024;
pub const LOW_SPACE_CRITICAL: u64 = 1024 * 1024 * 1024;

/// All inputs of the health report (collected by the caller).
#[derive(Debug, Clone)]
pub struct HealthInputs<'a> {
    pub config_files: Vec<ConfigFileHint>,
    pub lock: LockStatus,
    pub kernel: &'a KernelInfo,
    pub log: Option<&'a LogSummary>,
    pub recent_operations: &'a [Operation],
    pub backend: &'a BackendStatus,
    pub check: &'a UpdateCheckResult,
    pub package_cache_bytes: Option<u64>,
    pub snapshot: SnapshotSupport,
    pub offline: OfflineUpdateStatus,
    pub external_updaters: Vec<ExternalUpdater>,
    pub news: Option<&'a NewsStatus>,
    pub root_available: Option<u64>,
    pub now: Timestamp,
}

fn item(
    kind: HealthItemKind,
    severity: Severity,
    detail: impl Into<String>,
    count: Option<u32>,
) -> HealthItem {
    HealthItem {
        kind,
        severity,
        detail: detail.into(),
        count,
    }
}

/// Reasons for a reboot recommendation.
pub fn reboot_reasons(
    kernel: &KernelInfo,
    log: Option<&LogSummary>,
    now: Timestamp,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if kernel.modules_missing {
        reasons.push("running kernel was replaced by an update".to_string());
    }
    let boot = now - i64::try_from(kernel.uptime_seconds).unwrap_or(0);
    if let Some(log) = log {
        let mut pkgs: Vec<String> = log
            .transactions
            .iter()
            .filter(|t| t.started_at > boot && t.outcome == LogOutcome::Completed)
            .flat_map(|t| t.packages.iter())
            .filter(|p| {
                classify::update_flags(p, kernel.package.as_deref(), false, false)
                    .contains(&cachyos_center_core::updates::UpdateFlag::RebootRecommended)
            })
            .cloned()
            .collect();
        pkgs.sort();
        pkgs.dedup();
        if !pkgs.is_empty() {
            reasons.push(format!("updated since boot: {}", pkgs.join(", ")));
        }
    }
    reasons
}

pub fn build(input: HealthInputs<'_>) -> HealthReport {
    let mut items = Vec::new();
    let pacnew = input
        .config_files
        .iter()
        .filter(|f| f.kind == "pacnew")
        .count() as u32;
    let pacsave = input
        .config_files
        .iter()
        .filter(|f| f.kind == "pacsave")
        .count() as u32;
    let mut blockers = Vec::new();

    if pacnew > 0 {
        items.push(item(
            HealthItemKind::PacnewFiles,
            Severity::Warning,
            "configuration files need a manual merge",
            Some(pacnew),
        ));
    }
    if pacsave > 0 {
        items.push(item(
            HealthItemKind::PacsaveFiles,
            Severity::Info,
            "saved configuration files of removed packages",
            Some(pacsave),
        ));
    }
    let reasons = reboot_reasons(input.kernel, input.log, input.now);
    if !reasons.is_empty() {
        let severity = if input.kernel.modules_missing {
            Severity::Warning
        } else {
            Severity::Info
        };
        items.push(item(
            HealthItemKind::RebootRecommended,
            severity,
            reasons.join("; "),
            None,
        ));
    }
    if let LockStatus::Locked { holder_running, .. } = &input.lock {
        let (severity, detail) = match holder_running {
            Some(false) => (
                Severity::Critical,
                "db.lck exists but no package manager is running; check before removing it manually",
            ),
            _ => (Severity::Warning, "another package manager is running"),
        };
        items.push(item(
            HealthItemKind::PackageManagerLocked,
            severity,
            detail,
            None,
        ));
        blockers.push("package manager is busy (db.lck)".to_string());
    }
    if let Some(problem) = input.log.and_then(|l| l.last_transaction_problem()) {
        let kind = match problem.outcome {
            LogOutcome::Failed => HealthItemKind::LastOperationFailed,
            _ => HealthItemKind::InterruptedTransaction,
        };
        items.push(item(
            kind,
            Severity::Critical,
            "the last pacman transaction did not complete; check the package state",
            None,
        ));
        blockers.push("last pacman transaction did not complete".to_string());
    } else if let Some(op) = input
        .recent_operations
        .iter()
        .filter(|o| o.kind != OperationKind::UpdateCheck && o.state.is_terminal())
        .max_by_key(|o| o.ended_at.unwrap_or(o.requested_at))
        && matches!(
            op.state,
            OperationState::Failed | OperationState::NeedsAttention
        )
    {
        let severity = if op.state == OperationState::NeedsAttention {
            Severity::Critical
        } else {
            Severity::Warning
        };
        items.push(item(
            HealthItemKind::LastOperationFailed,
            severity,
            op.summary.clone(),
            None,
        ));
        if op.state == OperationState::NeedsAttention {
            blockers.push("last operation needs attention".to_string());
        }
    }
    if let Some(bytes) = input.package_cache_bytes
        && bytes > LARGE_CACHE
    {
        items.push(item(
            HealthItemKind::PackageCacheLarge,
            Severity::Info,
            "package cache is large (paccache can clean it)",
            None,
        ));
    }
    if let BackendStatus::Unavailable { reason } = input.backend {
        items.push(item(
            HealthItemKind::PackageBackendUnavailable,
            Severity::Critical,
            reason.clone(),
            None,
        ));
        blockers.push("package functions are disabled".to_string());
    }
    match input.check.status {
        CheckStatus::PrerequisiteMissing => {
            items.push(item(
                HealthItemKind::PrerequisiteMissing,
                Severity::Warning,
                format!("missing: {}", input.check.missing_prerequisites.join(", ")),
                None,
            ));
            blockers.push("update check prerequisites are missing".to_string());
        }
        CheckStatus::Failed => {
            let detail = input
                .check
                .error
                .as_ref()
                .map(|e| e.message.clone())
                .unwrap_or_default();
            items.push(item(
                HealthItemKind::UpdateCheckFailed,
                Severity::Warning,
                detail,
                None,
            ));
        }
        CheckStatus::Stale => {
            items.push(item(
                HealthItemKind::UpdateCheckStale,
                Severity::Info,
                "update information is outdated",
                None,
            ));
        }
        _ => {}
    }
    if input.offline.prepared {
        items.push(item(
            HealthItemKind::OfflineUpdatePrepared,
            Severity::Info,
            "updates will be installed on the next reboot",
            None,
        ));
    }
    if input.offline.offline_conf_included && !input.offline.offline_conf_ignored.is_empty() {
        items.push(item(
            HealthItemKind::OfflineConfHoldsPackages,
            Severity::Warning,
            format!(
                "held back for online updates: {}",
                input.offline.offline_conf_ignored.join(", ")
            ),
            Some(input.offline.offline_conf_ignored.len() as u32),
        ));
    }
    let active_external: Vec<&ExternalUpdater> = input
        .external_updaters
        .iter()
        .filter(|u| u.active)
        .collect();
    if !active_external.is_empty() {
        items.push(item(
            HealthItemKind::ExternalUpdaterActive,
            Severity::Info,
            active_external
                .iter()
                .map(|u| u.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            Some(active_external.len() as u32),
        ));
        if input.offline.prepare_timer_active {
            blockers
                .push("pacman-offline-prepare.timer is active (externally managed)".to_string());
        }
    }
    match input.news {
        Some(news) if !news.disabled => {
            if news.unread_count > 0 {
                items.push(item(
                    HealthItemKind::NewsUnread,
                    Severity::Warning,
                    "unread Arch Linux/CachyOS news",
                    Some(news.unread_count),
                ));
                blockers.push("unread news".to_string());
            }
            if !news.errors.is_empty() || news.fetched_at.is_none() {
                items.push(item(
                    HealthItemKind::NewsUnavailable,
                    Severity::Info,
                    "news check not possible",
                    None,
                ));
                blockers.push("news check not possible".to_string());
            }
        }
        Some(_) => blockers.push("news check disabled".to_string()),
        None => blockers.push("news check not possible".to_string()),
    }
    if let Some(free) = input.root_available {
        if free < LOW_SPACE_CRITICAL {
            items.push(item(
                HealthItemKind::LowDiskSpace,
                Severity::Critical,
                "less than 1 GiB free on /",
                None,
            ));
            blockers.push("not enough free disk space".to_string());
        } else if free < LOW_SPACE_WARNING {
            items.push(item(
                HealthItemKind::LowDiskSpace,
                Severity::Warning,
                "less than 5 GiB free on /",
                None,
            ));
        }
    }
    items.sort_by_key(|i| std::cmp::Reverse(i.severity));
    HealthReport {
        items,
        pacnew_count: pacnew,
        pacsave_count: pacsave,
        config_files: input.config_files,
        lock: input.lock,
        reboot_recommended: !reasons.is_empty(),
        reboot_reasons: reasons,
        package_cache_bytes: input.package_cache_bytes,
        snapshot: input.snapshot,
        offline_update: input.offline,
        external_updaters: input.external_updaters,
        update_blockers: blockers,
        collected_at: input.now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cachyos_center_packages::pacman_log::LogTransaction;

    fn kernel(missing: bool) -> KernelInfo {
        KernelInfo {
            release: "7.2.8-1-cachyos".into(),
            package: Some("linux-cachyos".into()),
            uptime_seconds: 3600,
            modules_missing: missing,
        }
    }

    fn snapshot() -> SnapshotSupport {
        SnapshotSupport {
            btrfs_root: false,
            snapper_installed: false,
            root_config: None,
            snap_pac_active: false,
            can_request_snapshot: false,
        }
    }

    fn news(unread: u32) -> NewsStatus {
        NewsStatus {
            disabled: false,
            items: vec![],
            fetched_at: Some(1),
            errors: vec![],
            acknowledged_until: None,
            unread_count: unread,
        }
    }

    fn base<'a>(
        k: &'a KernelInfo,
        b: &'a BackendStatus,
        c: &'a UpdateCheckResult,
        n: &'a NewsStatus,
    ) -> HealthInputs<'a> {
        HealthInputs {
            config_files: vec![],
            lock: LockStatus::Free,
            kernel: k,
            log: None,
            recent_operations: &[],
            backend: b,
            check: c,
            package_cache_bytes: Some(10),
            snapshot: snapshot(),
            offline: OfflineUpdateStatus::default(),
            external_updaters: vec![],
            news: Some(n),
            root_available: Some(100 * 1024 * 1024 * 1024),
            now: 10_000,
        }
    }

    #[test]
    fn healthy_system_has_no_items_and_no_blockers() {
        let k = kernel(false);
        let b = BackendStatus::Ready {
            libalpm_version: "16.0.1".into(),
            built_against: "16.0.1".into(),
        };
        let c = UpdateCheckResult::empty(CheckStatus::Fresh);
        let n = news(0);
        let report = build(base(&k, &b, &c, &n));
        assert!(report.items.is_empty(), "{:?}", report.items);
        assert!(report.update_blockers.is_empty());
        assert!(!report.reboot_recommended);
    }

    #[test]
    fn problems_are_reported_and_sorted() {
        let k = kernel(true);
        let b = BackendStatus::Unavailable {
            reason: "libalpm.so.17".into(),
        };
        let c = UpdateCheckResult::empty(CheckStatus::Stale);
        let n = news(2);
        let mut input = base(&k, &b, &c, &n);
        input.lock = LockStatus::Locked {
            since: Some(1),
            holder_running: Some(false),
        };
        input.config_files = vec![ConfigFileHint {
            path: "/etc/a.pacnew".into(),
            kind: "pacnew".into(),
            modified_at: None,
        }];
        input.root_available = Some(512 * 1024 * 1024);
        let report = build(input);
        assert_eq!(report.items.first().unwrap().severity, Severity::Critical);
        assert_eq!(report.pacnew_count, 1);
        assert!(report.reboot_recommended);
        assert!(report.update_blockers.iter().any(|b| b.contains("db.lck")));
        assert!(
            report
                .update_blockers
                .iter()
                .any(|b| b.contains("unread news"))
        );
        assert!(
            report
                .update_blockers
                .iter()
                .any(|b| b.contains("disk space"))
        );
    }

    #[test]
    fn interrupted_log_transaction() {
        let k = kernel(false);
        let b = BackendStatus::Ready {
            libalpm_version: "16".into(),
            built_against: "16".into(),
        };
        let c = UpdateCheckResult::empty(CheckStatus::Fresh);
        let n = news(0);
        let log = LogSummary {
            transactions: vec![LogTransaction {
                started_at: 9_990,
                ended_at: None,
                outcome: LogOutcome::Unknown,
                command: None,
                full_upgrade: true,
                installed: 0,
                upgraded: 1,
                downgraded: 0,
                reinstalled: 0,
                removed: 0,
                config_files: 0,
                packages: vec!["linux-cachyos".into()],
            }],
            last_full_upgrade: None,
        };
        let mut input = base(&k, &b, &c, &n);
        input.log = Some(&log);
        let report = build(input);
        assert!(
            report
                .items
                .iter()
                .any(|i| i.kind == HealthItemKind::InterruptedTransaction)
        );
        assert!(!report.update_blockers.is_empty());
    }

    #[test]
    fn reboot_after_kernel_update_since_boot() {
        let k = kernel(false);
        let log = LogSummary {
            transactions: vec![LogTransaction {
                started_at: 9_000,
                ended_at: Some(9_010),
                outcome: LogOutcome::Completed,
                command: None,
                full_upgrade: true,
                installed: 0,
                upgraded: 2,
                downgraded: 0,
                reinstalled: 0,
                removed: 0,
                config_files: 0,
                packages: vec!["firefox".into(), "amd-ucode".into()],
            }],
            last_full_upgrade: Some(9_010),
        };
        let reasons = reboot_reasons(&k, Some(&log), 10_000);
        assert_eq!(reasons, vec!["updated since boot: amd-ucode".to_string()]);
    }
}
