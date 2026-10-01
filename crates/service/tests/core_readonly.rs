//! Read-only smoke tests of the application core on the machine running the
//! tests. User data goes to a temporary directory.

use std::path::PathBuf;

use cachyos_center_core::paths::UserDirs;
use cachyos_center_core::updates::CheckStatus;
use cachyos_center_packages::PackageService;
use cachyos_center_service::{AppCore, ReadApi};

fn core(dir: &std::path::Path) -> Option<AppCore> {
    if !std::path::Path::new("/etc/pacman.conf").exists() {
        return None;
    }
    let dirs = UserDirs {
        config: dir.join("config"),
        data: dir.join("data"),
        cache: dir.join("cache"),
        state: dir.join("state"),
    };
    let packages = PackageService::new(dirs.check_db(), dirs.check_state());
    Some(AppCore::with_parts(dirs, packages))
}

#[test]
fn dashboard_and_system_information() {
    let dir = tempfile::tempdir().unwrap();
    let Some(core) = core(dir.path()) else { return };
    let dashboard = core.dashboard();
    assert!(!dashboard.system.kernel.is_empty());
    assert!(
        matches!(
            dashboard.updates.status,
            CheckStatus::NeverChecked | CheckStatus::PrerequisiteMissing | CheckStatus::Unsupported
        ),
        "a fresh user directory has no check result: {:?}",
        dashboard.updates.status
    );
    let info = core.system_info();
    assert!(info.disks.iter().any(|d| d.mount_point == "/"));
    let summary = core.system_summary().unwrap();
    assert_eq!(summary.kernel, info.kernel.release);
}

#[test]
fn health_and_activity_do_not_fail() {
    let dir = tempfile::tempdir().unwrap();
    let Some(core) = core(dir.path()) else { return };
    let report = core.health().unwrap();
    assert!(report.collected_at > 0);
    let activity = core.recent_activity(10).unwrap();
    assert!(activity.len() <= 10);
    for entry in &activity {
        assert!(!entry.summary.contains("/home/"), "{entry:?}");
    }
}

#[test]
fn diagnostic_report_is_sanitized() {
    let dir = tempfile::tempdir().unwrap();
    let Some(core) = core(dir.path()) else { return };
    let report = core.diagnostic_report();
    assert!(report.starts_with("cachyos-center diagnostic report (sanitized)"));
    if let Ok(home) = std::env::var("HOME") {
        assert!(!report.contains(&home), "home directory must not appear");
    }
    if let Ok(user) = std::env::var("USER")
        && user.len() >= 3
    {
        assert!(
            !report
                .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                .any(|w| w == user),
            "user name must not appear"
        );
    }
    if let Ok(host) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
        let host = host.trim();
        if host.len() >= 3 {
            assert!(!report.contains(host), "host name must not appear");
        }
    }
}

#[test]
fn settings_and_news_ack_are_stored_in_user_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let Some(core) = core(dir.path()) else { return };
    let mut settings = core.settings();
    assert!(!settings.mcp_enabled);
    settings.mcp_enabled = true;
    settings.news_enabled = false;
    core.save_settings(&settings).unwrap();
    assert!(core.settings().mcp_enabled);
    let news = core.news(false, false);
    assert!(news.disabled);
    let acked = core.acknowledge_news(1_000).unwrap();
    assert_eq!(acked.acknowledged_until, Some(1_000));
    assert!(
        PathBuf::from(dir.path())
            .join("state/news-ack.json")
            .exists()
    );
}

#[test]
fn failed_transaction_after_check_makes_status_stale() {
    use cachyos_center_core::operation::{
        Operation, OperationKind, OperationOrigin, OperationState,
    };
    use cachyos_center_core::updates::CheckStatus;
    let dir = tempfile::tempdir().unwrap();
    let Some(core) = core(dir.path()) else { return };
    // Simulate a successful check right now: the isolated database points to
    // the (read-only) system databases.
    let check_db = core.dirs().check_db();
    std::fs::create_dir_all(&check_db).unwrap();
    std::os::unix::fs::symlink("/var/lib/pacman/sync", check_db.join("sync")).unwrap();
    std::os::unix::fs::symlink("/var/lib/pacman/local", check_db.join("local")).unwrap();
    let before = core.last_check();
    if before.status == CheckStatus::Unsupported {
        return;
    }
    let state = cachyos_center_packages::check::CheckState {
        checked_at: Some(cachyos_center_core::now() - 10),
        attempted_at: Some(cachyos_center_core::now() - 10),
        error: None,
    };
    state.save(&core.dirs().check_state()).unwrap();
    // A failed upgrade after the check (commit had started).
    let mut op = Operation::new(
        "55555555-5555-4555-8555-555555555555".into(),
        OperationKind::SystemUpgrade,
        OperationOrigin::User,
        cachyos_center_core::now() - 5,
    );
    op.state = OperationState::NeedsAttention;
    op.commit_started = true;
    op.ended_at = Some(cachyos_center_core::now());
    core.history().record(&op).unwrap();
    assert_eq!(core.last_check().status, CheckStatus::Stale);
    let result = core.last_check();
    assert_ne!(result.status, CheckStatus::Fresh, "{result:?}");
    assert_eq!(
        result.error.map(|e| e.code),
        Some(cachyos_center_core::ErrorCode::Stale)
    );
}
