//! Headless mode `cachyos-center --notify-timer-status`.
//!
//! Started by the user unit `cachyos-center-notify.service` whenever the
//! helper's timer writes a new status. Sends one desktop notification per
//! timer run (only if notifications are enabled) and exits. No window, no GTK.

use std::path::Path;

use cachyos_center_core::operation::{Operation, OperationKind, OperationState};
use cachyos_center_core::paths::{TIMER_STATUS_FILE, UserDirs};
use cachyos_center_service::SettingsStore;

use crate::notify;

fn texts(op: &Operation, de: bool) -> Option<(String, String, bool)> {
    match (op.kind, op.state) {
        (OperationKind::UpdateCheck, OperationState::Succeeded) => {
            let n = op.progress.packages_total.unwrap_or(0);
            (n > 0).then(|| {
                if de {
                    (
                        format!("{n} Updates verfügbar"),
                        "Details und Installation in cachyos-center.".into(),
                        false,
                    )
                } else {
                    (
                        format!("{n} updates available"),
                        "Details and installation in cachyos-center.".into(),
                        false,
                    )
                }
            })
        }
        (OperationKind::AutoUpdatePrepare, OperationState::Succeeded)
            if op.summary.starts_with("prepared") =>
        {
            Some(if de {
                (
                "Update vorbereitet".into(),
                "Die Updates werden beim nächsten Neustart installiert. Den Zeitpunkt bestimmen Sie selbst.".into(),
                false,
            )
            } else {
                (
                    "Update prepared".into(),
                    "The updates will be installed on the next reboot. You decide when to reboot."
                        .into(),
                    false,
                )
            })
        }
        (_, OperationState::NeedsAttention) => Some(if de {
            (
                "Automatische Updates: Prüfung erforderlich".into(),
                "Die Vorbereitung wurde aus Sicherheitsgründen nicht durchgeführt. Details in cachyos-center.".into(),
                true,
            )
        } else {
            (
                "Automatic updates need attention".into(),
                "The preparation was not carried out for safety reasons. Details in cachyos-center.".into(),
                true,
            )
        }),
        (_, OperationState::Failed) => Some(if de {
            (
                "Automatische Updates fehlgeschlagen".into(),
                "Details in cachyos-center.".into(),
                true,
            )
        } else {
            (
                "Automatic updates failed".into(),
                "Details in cachyos-center.".into(),
                true,
            )
        }),
        _ => None,
    }
}

pub fn run() -> i32 {
    let Some(dirs) = UserDirs::from_env() else {
        return 1;
    };
    let settings = SettingsStore::new(dirs.settings_file()).load();
    if !settings.notifications {
        return 0;
    }
    let Some(op) = std::fs::read_to_string(TIMER_STATUS_FILE)
        .ok()
        .and_then(|t| serde_json::from_str::<Operation>(&t).ok())
    else {
        return 0;
    };
    if !op.state.is_terminal() {
        return 0;
    }
    let marker = dirs.state.join("last-timer-notification");
    if std::fs::read_to_string(&marker).is_ok_and(|id| id.trim() == op.id) {
        return 0;
    }
    let Some((title, body, critical)) = texts(&op, notify::german(&settings)) else {
        let _ = write_marker(&marker, &op.id);
        return 0;
    };
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return 1;
    };
    match rt.block_on(notify::send_raw(&title, &body, critical)) {
        Ok(()) => {
            let _ = write_marker(&marker, &op.id);
            0
        }
        Err(e) => {
            tracing::warn!("notification failed: {e}");
            1
        }
    }
}

fn write_marker(path: &Path, id: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cachyos_center_core::operation::OperationOrigin;

    #[test]
    fn only_relevant_results_notify() {
        let mut op = Operation::new(
            "a".into(),
            OperationKind::UpdateCheck,
            OperationOrigin::Timer,
            1,
        );
        op.state = OperationState::Succeeded;
        op.progress.packages_total = Some(0);
        assert!(
            texts(&op, true).is_none(),
            "no notification without updates"
        );
        op.progress.packages_total = Some(4);
        assert_eq!(texts(&op, true).unwrap().0, "4 Updates verfügbar");
        op.state = OperationState::NeedsAttention;
        assert!(
            texts(&op, false).unwrap().2,
            "blocked preparation is critical"
        );
    }
}
