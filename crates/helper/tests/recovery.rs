#![allow(clippy::unwrap_used)]
//! Reconstruction of operations after a crash or restart of the helper
//! (concept, scenario 10): the result is taken from the journal and the
//! pacman log; success is only reported with evidence.

use cachyos_center_core::bridge::AlpmConfig;
use cachyos_center_core::operation::{Operation, OperationKind, OperationOrigin, OperationState};
use cachyos_center_helper::config::HelperConfig;
use cachyos_center_helper::engine::Engine;
use cachyos_center_helper::journal::{Journal, RecoveryState};
use cachyos_center_packages::PackageService;

fn engine(root: &std::path::Path) -> std::sync::Arc<Engine> {
    let config = HelperConfig::development(root.to_path_buf());
    let alpm = AlpmConfig {
        root_dir: "/".into(),
        db_path: root.join("db/").display().to_string(),
        gpg_dir: String::new(),
        cache_dirs: vec![],
        architectures: vec![],
        ignore_pkgs: vec![],
        ignore_groups: vec![],
        hold_pkgs: vec![],
        sig_level: vec![],
        repos: vec![],
    };
    let packages = PackageService::with_parts(
        alpm,
        root.join("checkup-db"),
        root.join("check.json"),
        config.pacman_log.clone(),
        Default::default(),
    );
    Engine::new(config, packages)
}

fn interrupted(root: &std::path::Path, id: &str, state: OperationState, log: &str) {
    let journal = Journal::new(HelperConfig::development(root.to_path_buf()).journal_dir());
    let mut op = Operation::new(
        id.into(),
        OperationKind::SystemUpgrade,
        OperationOrigin::User,
        100,
    );
    op.transition(OperationState::AwaitingAuthorization, 100)
        .unwrap();
    op.transition(OperationState::Preparing, 101).unwrap();
    if state == OperationState::Downloading || state == OperationState::Installing {
        op.transition(OperationState::Downloading, 102).unwrap();
    }
    if state == OperationState::Installing {
        op.transition(OperationState::Installing, 103).unwrap();
    }
    journal.save(&op).unwrap();
    std::fs::write(
        root.join("pacman.log"),
        format!("[2026-09-30T10:00:00+0000] [PACMAN] earlier run\n{log}"),
    )
    .unwrap();
    let offset = "[2026-09-30T10:00:00+0000] [PACMAN] earlier run\n".len() as u64;
    journal
        .save_recovery(
            id,
            &RecoveryState {
                pacman_log_offset: Some(offset),
                unit: None,
                initiator_uid: Some(1000),
            },
        )
        .unwrap();
}

const ID: &str = "12345678-1234-4234-8234-123456789abc";

#[test]
fn completed_transaction_is_reconstructed_as_success() {
    let dir = tempfile::tempdir().unwrap();
    interrupted(
        dir.path(),
        ID,
        OperationState::Installing,
        "[2026-09-30T10:00:01+0000] [ALPM] transaction started\n[2026-09-30T10:00:02+0000] [ALPM] upgraded foo (1-1 -> 2-1)\n[2026-09-30T10:00:03+0000] [ALPM] transaction completed\n",
    );
    let engine = engine(dir.path());
    engine.recover();
    let op = engine.status(ID).unwrap();
    assert_eq!(op.state, OperationState::Succeeded, "{op:?}");
    assert_eq!(op.changes.upgraded, 1);
    assert!(op.summary.contains("reconstructed"));
    assert!(!op.outcome_unknown);
}

#[test]
fn unfinished_transaction_needs_attention() {
    let dir = tempfile::tempdir().unwrap();
    interrupted(
        dir.path(),
        ID,
        OperationState::Installing,
        "[2026-09-30T10:00:01+0000] [ALPM] transaction started\n[2026-09-30T10:00:02+0000] [ALPM] upgraded foo (1-1 -> 2-1)\n",
    );
    let engine = engine(dir.path());
    engine.recover();
    let op = engine.status(ID).unwrap();
    assert_eq!(op.state, OperationState::NeedsAttention, "{op:?}");
    assert!(op.outcome_unknown, "no success without evidence");
}

#[test]
fn interruption_before_commit_changed_nothing() {
    let dir = tempfile::tempdir().unwrap();
    interrupted(dir.path(), ID, OperationState::Downloading, "");
    let engine = engine(dir.path());
    engine.recover();
    let op = engine.status(ID).unwrap();
    assert_eq!(op.state, OperationState::Failed, "{op:?}");
    assert!(!op.commit_started);
    assert!(op.summary.contains("no package was changed"));
    assert_eq!(engine.current().map(|o| o.id), Some(ID.to_string()));
}
