//! Scheduled preflight (`cachyos-center-helper preflight`) in development mode.
//! The automatic mode must never prepare anything while a gate is closed.

use std::path::Path;
use std::process::Command;

use cachyos_center_core::operation::{Operation, OperationKind, OperationState};
use cachyos_center_core::policy::{AutoUpdateConfig, AutoUpdatePolicy};

fn run_preflight(
    root: &Path,
    policy: AutoUpdatePolicy,
    experimental: bool,
) -> (bool, Option<Operation>) {
    std::fs::create_dir_all(root.join("etc")).expect("etc");
    let config = AutoUpdateConfig {
        policy,
        ..AutoUpdateConfig::default()
    };
    std::fs::write(
        root.join("etc/auto-update.toml"),
        config.to_toml().expect("toml"),
    )
    .expect("policy");
    if experimental {
        std::fs::write(
            root.join("etc/experimental.toml"),
            "offline_auto_update = true\n",
        )
        .expect("experimental");
    }
    let status = Command::new(env!("CARGO_BIN_EXE_cachyos-center-helper"))
        .args(["preflight", "--dev-root"])
        .arg(root)
        .status()
        .expect("helper");
    let op = std::fs::read_to_string(root.join("state/timer-status.json"))
        .ok()
        .map(|t| serde_json::from_str::<Operation>(&t).expect("status json"));
    (status.success(), op)
}

fn running_as_root() -> bool {
    #[allow(unsafe_code)]
    // SAFETY: plain getter without preconditions.
    let euid = unsafe { libc::geteuid() };
    euid == 0
}

#[test]
fn policy_off_does_nothing() {
    if running_as_root() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let (ok, op) = run_preflight(dir.path(), AutoUpdatePolicy::Off, false);
    assert!(ok);
    assert!(
        op.is_none(),
        "no run is recorded when automatic updates are off"
    );
}

#[test]
fn prepare_mode_is_blocked_while_in_development() {
    if running_as_root() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let (ok, op) = run_preflight(dir.path(), AutoUpdatePolicy::PrepareForNextReboot, false);
    assert!(ok);
    let op = op.expect("status written");
    assert_eq!(op.kind, OperationKind::AutoUpdatePrepare);
    assert_eq!(op.state, OperationState::NeedsAttention, "{op:?}");
    let detail = op.error.and_then(|e| e.detail).unwrap_or_default();
    assert!(detail.contains("experimentalLocked"), "{detail}");
    assert!(!op.commit_started);
}

#[test]
fn prepare_mode_needs_pacman_offline() {
    if running_as_root() || Path::new("/usr/bin/pacman-offline").exists() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let (ok, op) = run_preflight(dir.path(), AutoUpdatePolicy::PrepareForNextReboot, true);
    assert!(ok);
    let op = op.expect("status written");
    assert_eq!(op.state, OperationState::NeedsAttention);
    let detail = op.error.and_then(|e| e.detail).unwrap_or_default();
    assert!(detail.contains("pacmanOfflineMissing"), "{detail}");
    assert!(!detail.contains("experimentalLocked"), "{detail}");
}

/// Needs network access (isolated update check).
#[test]
#[ignore = "network access"]
fn notify_only_checks_without_installing() {
    if running_as_root() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let before = std::fs::metadata("/var/lib/pacman/sync")
        .and_then(|m| m.modified())
        .ok();
    let (ok, op) = run_preflight(dir.path(), AutoUpdatePolicy::NotifyOnly, false);
    assert!(ok);
    let op = op.expect("status written");
    assert_eq!(op.kind, OperationKind::UpdateCheck);
    assert!(
        matches!(op.state, OperationState::Succeeded | OperationState::Failed),
        "{op:?}"
    );
    if op.state == OperationState::Succeeded {
        assert!(op.summary.ends_with("updates available"));
        assert!(op.progress.packages_total.is_some());
    }
    assert!(!op.commit_started);
    let after = std::fs::metadata("/var/lib/pacman/sync")
        .and_then(|m| m.modified())
        .ok();
    assert_eq!(
        before, after,
        "the productive sync database must not be touched"
    );
}
