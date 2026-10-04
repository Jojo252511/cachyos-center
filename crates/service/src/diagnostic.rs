//! Sanitized diagnostic report ("Diagnose kopieren").
//!
//! The report is built from structured data and then passed through the
//! sanitizer. It is shown to the user for review before it is copied. It is
//! written in English so that it can be used in support forums.

use std::fmt::Write as _;

use cachyos_center_core::health::{HealthItem, HealthReport};
use cachyos_center_core::history::{HistoryEntry, HistorySource};
use cachyos_center_core::operation::OperationKind;
use cachyos_center_core::policy::AutoUpdateStatus;
use cachyos_center_core::sanitize::{SanitizeContext, sanitize};
use cachyos_center_core::system::{BackendStatus, LockStatus, SystemInfo};
use cachyos_center_core::timefmt::format_utc;
use cachyos_center_core::updates::{CheckStatus, UpdateCheckResult};
use cachyos_center_core::{APP_VERSION, Timestamp};

fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
}

fn ts(value: Option<Timestamp>) -> String {
    value.map(format_utc).unwrap_or_else(|| "unknown".into())
}

#[derive(Debug)]
pub struct ReportInputs<'a> {
    pub system: &'a SystemInfo,
    pub health: &'a HealthReport,
    pub updates: &'a UpdateCheckResult,
    pub auto_update: &'a AutoUpdateStatus,
    pub activity: &'a [HistoryEntry],
    pub now: Timestamp,
}

/// A count from the update check. Only a fresh check states a number as
/// current; the report never claims "0 updates" without one.
fn update_count(result: &UpdateCheckResult, n: usize) -> String {
    match result.status {
        CheckStatus::Fresh => n.to_string(),
        CheckStatus::Stale => format!("{n} (outdated check)"),
        _ => "unknown".to_string(),
    }
}

fn health_line(item: &HealthItem) -> String {
    let count = item
        .count
        .map(|n| format!(" (count: {n})"))
        .unwrap_or_default();
    format!(
        "- {:?} {:?}: {}{count}",
        item.severity, item.kind, item.detail
    )
}

/// One activity entry: source, outcome and the result (updates found by a
/// check, package changes of a transaction).
fn activity_line(e: &HistoryEntry) -> String {
    let source = match e.source {
        HistorySource::App => "app",
        HistorySource::Timer => "timer",
        HistorySource::ExternalPacman => "external",
    };
    let result = if e.kind == Some(OperationKind::UpdateCheck) {
        e.updates_found
            .map(|n| format!(" ({n} updates found)"))
            .unwrap_or_default()
    } else {
        let downgraded = if e.downgraded > 0 {
            format!(", {} downgraded", e.downgraded)
        } else {
            String::new()
        };
        format!(
            " (+{} ~{} -{}{downgraded})",
            e.installed, e.upgraded, e.removed
        )
    };
    let error = e
        .error_code
        .map(|c| format!(" error={c}"))
        .unwrap_or_default();
    format!(
        "- {} [{source}] {}{result}{error}",
        format_utc(e.started_at),
        e.summary
    )
}

pub fn build(input: &ReportInputs<'_>, ctx: &SanitizeContext) -> String {
    let s = input.system;
    let mut r = String::new();
    let _ = writeln!(r, "cachyos-center diagnostic report (sanitized)");
    let _ = writeln!(r, "Created: {}", format_utc(input.now));
    let _ = writeln!(r, "cachyos-center: {APP_VERSION}");
    let _ = writeln!(r);
    let _ = writeln!(r, "[System]");
    let _ = writeln!(
        r,
        "OS: {} (id={}{})",
        s.os.pretty_name,
        s.os.id,
        s.os.build_id
            .as_deref()
            .map(|b| format!(", build={b}"))
            .unwrap_or_default()
    );
    let _ = writeln!(
        r,
        "Kernel: {} (package: {}), uptime: {} h{}",
        s.kernel.release,
        s.kernel.package.as_deref().unwrap_or("unknown"),
        s.kernel.uptime_seconds / 3600,
        if s.kernel.modules_missing {
            ", module directory missing (reboot pending)"
        } else {
            ""
        }
    );
    let _ = writeln!(
        r,
        "CPU: {} ({} cores/{} threads, {})",
        s.cpu.model,
        s.cpu.cores,
        s.cpu.threads,
        s.cpu.isa_level.as_deref().unwrap_or("unknown ISA level")
    );
    for gpu in &s.gpus {
        let _ = writeln!(
            r,
            "GPU: {} {} [{}] driver={}",
            gpu.vendor,
            gpu.model,
            gpu.pci_id,
            gpu.driver.as_deref().unwrap_or("none")
        );
    }
    let _ = writeln!(
        r,
        "Memory: {} total, {} available; swap {}",
        gib(s.memory.total_bytes),
        gib(s.memory.available_bytes),
        gib(s.memory.swap_total_bytes)
    );
    for d in &s.disks {
        let _ = writeln!(
            r,
            "Disk {}: {}, {} total, {} free",
            d.mount_point,
            d.filesystem,
            gib(d.total_bytes),
            gib(d.available_bytes)
        );
    }
    let _ = writeln!(
        r,
        "Session: {:?} ({}, {})",
        s.session.kind,
        s.session.desktop.as_deref().unwrap_or("unknown desktop"),
        s.session.session_type.as_deref().unwrap_or("unknown type")
    );
    let _ = writeln!(r);
    let _ = writeln!(r, "[Package management]");
    let _ = writeln!(
        r,
        "pacman: {}",
        s.pacman.pacman_version.as_deref().unwrap_or("unknown")
    );
    match &s.pacman.backend {
        BackendStatus::Ready {
            libalpm_version,
            built_against,
        } => {
            let _ = writeln!(
                r,
                "libalpm: {libalpm_version} (bridge built against {built_against})"
            );
        }
        BackendStatus::Unavailable { reason, .. } => {
            let _ = writeln!(r, "libalpm bridge: unavailable ({reason})");
        }
    }
    let _ = writeln!(r, "Repositories: {}", s.pacman.repositories.join(", "));
    let _ = writeln!(
        r,
        "Installed packages: {} (local/AUR: {})",
        s.pacman.installed_count, s.pacman.foreign_count
    );
    let _ = writeln!(
        r,
        "Lock: {}",
        match &s.pacman.lock {
            LockStatus::Free => "free".to_string(),
            LockStatus::Locked { holder_running, .. } =>
                format!("locked (package manager running: {holder_running:?})"),
        }
    );
    let _ = writeln!(r, "Last full upgrade: {}", ts(s.pacman.last_full_upgrade));
    let _ = writeln!(r);
    let _ = writeln!(r, "[Updates]");
    let u = input.updates;
    let _ = writeln!(
        r,
        "Status: {:?}, checked: {}, updates: {}, held back: {}, reboot recommended: {}",
        u.status,
        ts(u.checked_at),
        update_count(u, u.updates.len()),
        update_count(u, u.held_back.len()),
        u.reboot_recommended
    );
    if let Some(err) = &u.error {
        let _ = writeln!(r, "Last check error: {} {}", err.code, err.message);
    }
    let _ = writeln!(r);
    let _ = writeln!(r, "[Health]");
    if input.health.items.is_empty() {
        let _ = writeln!(r, "No findings.");
    }
    for item in &input.health.items {
        let _ = writeln!(r, "{}", health_line(item));
    }
    let _ = writeln!(
        r,
        "Snapshots: btrfs={}, snapper={}, root config={}, snap-pac={}",
        input.health.snapshot.btrfs_root,
        input.health.snapshot.snapper_installed,
        input.health.snapshot.root_config.is_some(),
        input.health.snapshot.snap_pac_active
    );
    let _ = writeln!(r);
    let _ = writeln!(r, "[Automatic updates]");
    let a = input.auto_update;
    let _ = writeln!(
        r,
        "Policy: {}, timer enabled: {}, next run: {}, pacman-offline installed: {}, update prepared: {}",
        a.config.policy.as_str(),
        a.timer_enabled,
        ts(a.next_run),
        a.offline.installed,
        a.prepared_for_next_reboot
    );
    for u in &a.external_updaters {
        let _ = writeln!(
            r,
            "Other updater: {} ({}, active: {})",
            u.name, u.scope, u.active
        );
    }
    let _ = writeln!(r);
    let _ = writeln!(r, "[Recent activity]");
    for e in input.activity.iter().take(5) {
        let _ = writeln!(r, "{}", activity_line(e));
    }
    sanitize(&r, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(source: HistorySource, kind: Option<OperationKind>, summary: &str) -> HistoryEntry {
        HistoryEntry {
            id: "x".into(),
            source,
            kind,
            origin: None,
            state: None,
            log_outcome: None,
            started_at: 1_791_117_582,
            ended_at: None,
            summary: summary.into(),
            error_code: None,
            installed: 0,
            upgraded: 0,
            removed: 0,
            downgraded: 0,
            updates_found: None,
            packages: vec![],
            outcome_unknown: false,
        }
    }

    #[test]
    fn activity_lines_name_the_source_and_the_result() {
        let mut check = entry(
            HistorySource::Timer,
            Some(OperationKind::UpdateCheck),
            "update check: succeeded",
        );
        check.updates_found = Some(6);
        assert_eq!(
            activity_line(&check),
            "- 2026-10-04 12:39 UTC [timer] update check: succeeded (6 updates found)"
        );
        // A check recorded without a count never claims a number.
        check.updates_found = None;
        assert_eq!(
            activity_line(&check),
            "- 2026-10-04 12:39 UTC [timer] update check: succeeded"
        );
        let mut upgrade = entry(
            HistorySource::App,
            Some(OperationKind::SystemUpgrade),
            "system upgrade: succeeded",
        );
        upgrade.upgraded = 6;
        assert_eq!(
            activity_line(&upgrade),
            "- 2026-10-04 12:39 UTC [app] system upgrade: succeeded (+0 ~6 -0)"
        );
        let mut external = entry(
            HistorySource::ExternalPacman,
            None,
            "pacman transaction completed",
        );
        external.removed = 6;
        external.downgraded = 1;
        assert_eq!(
            activity_line(&external),
            "- 2026-10-04 12:39 UTC [external] pacman transaction completed (+0 ~0 -6, 1 downgraded)"
        );
    }

    #[test]
    fn health_lines_carry_the_count() {
        use cachyos_center_core::health::{HealthItemKind, Severity};
        let mut item = HealthItem {
            kind: HealthItemKind::PacnewFiles,
            severity: Severity::Warning,
            detail: "configuration files need a manual merge".into(),
            count: Some(6),
        };
        assert_eq!(
            health_line(&item),
            "- Warning PacnewFiles: configuration files need a manual merge (count: 6)"
        );
        item.count = None;
        assert_eq!(
            health_line(&item),
            "- Warning PacnewFiles: configuration files need a manual merge"
        );
    }

    #[test]
    fn counts_are_only_current_after_a_fresh_check() {
        let fresh = UpdateCheckResult::empty(CheckStatus::Fresh);
        assert_eq!(update_count(&fresh, 0), "0");
        let stale = UpdateCheckResult::empty(CheckStatus::Stale);
        assert_eq!(update_count(&stale, 3), "3 (outdated check)");
        for status in [
            CheckStatus::NeverChecked,
            CheckStatus::Failed,
            CheckStatus::Unsupported,
            CheckStatus::PrerequisiteMissing,
        ] {
            let result = UpdateCheckResult::empty(status);
            assert_eq!(update_count(&result, 0), "unknown", "{status:?}");
        }
    }
}
