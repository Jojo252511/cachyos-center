//! Desktop notifications (freedesktop, D-Bus) with a rate limit.
//!
//! Without a notification daemon the call simply fails and the app stays
//! fully usable.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use cachyos_center_core::APP_ID;
use cachyos_center_core::operation::{Operation, OperationKind, OperationState};
use cachyos_center_core::settings::{LanguagePreference, Settings};
use zbus::zvariant::Value;

/// Minimum distance between two notifications of the same kind.
const MIN_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    UpdatesAvailable,
    OperationFinished,
    OperationFailed,
    NeedsAttention,
}

#[derive(Debug, Default)]
pub struct Notifier {
    last: Mutex<HashMap<Kind, Instant>>,
}

/// UI language for texts sent by the backend.
pub fn german(settings: &Settings) -> bool {
    match settings.language {
        LanguagePreference::De => true,
        LanguagePreference::En => false,
        LanguagePreference::System => ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
            .is_none_or(|v| v.starts_with("de")),
    }
}

/// Title and body for a finished operation.
pub fn operation_texts(op: &Operation, de: bool) -> (Kind, String, String) {
    let what = match (op.kind, de) {
        (OperationKind::SystemUpgrade, true) => "Systemupdate",
        (OperationKind::SystemUpgrade, false) => "System upgrade",
        (OperationKind::Install, true) => "Installation",
        (OperationKind::Install, false) => "Installation",
        (OperationKind::Remove, true) => "Entfernen",
        (OperationKind::Remove, false) => "Removal",
        (OperationKind::UpdateCheck, true) => "Updateprüfung",
        (OperationKind::UpdateCheck, false) => "Update check",
        (OperationKind::AutoUpdatePrepare, true) => "Update-Vorbereitung",
        (OperationKind::AutoUpdatePrepare, false) => "Update preparation",
    };
    match op.state {
        OperationState::Succeeded => (
            Kind::OperationFinished,
            if de {
                format!("{what} abgeschlossen")
            } else {
                format!("{what} completed")
            },
            if de {
                format!("{} Pakete geändert.", op.changes.total())
            } else {
                format!("{} packages changed.", op.changes.total())
            },
        ),
        OperationState::NeedsAttention => (
            Kind::NeedsAttention,
            if de {
                format!("{what}: Prüfung erforderlich")
            } else {
                format!("{what}: needs attention")
            },
            if de {
                "Die Paketlage ist unklar. Bitte in cachyos-center prüfen.".to_string()
            } else {
                "The package state is unclear. Please check in cachyos-center.".to_string()
            },
        ),
        OperationState::CancelledBeforeCommit => (
            Kind::OperationFailed,
            if de {
                format!("{what} abgebrochen")
            } else {
                format!("{what} cancelled")
            },
            if de {
                "Die installierten Pakete sind unverändert.".to_string()
            } else {
                "The installed packages are unchanged.".to_string()
            },
        ),
        _ => (
            Kind::OperationFailed,
            if de {
                format!("{what} fehlgeschlagen")
            } else {
                format!("{what} failed")
            },
            if op.commit_started {
                if de {
                    "Paketlage unklar – bitte prüfen.".to_string()
                } else {
                    "Package state unclear – please check.".to_string()
                }
            } else if de {
                "Paketlage unverändert.".to_string()
            } else {
                "Package state unchanged.".to_string()
            },
        ),
    }
}

impl Notifier {
    fn allowed(&self, kind: Kind) -> bool {
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        match last.get(&kind) {
            Some(t) if now.duration_since(*t) < MIN_INTERVAL => false,
            _ => {
                last.insert(kind, now);
                true
            }
        }
    }

    /// Sends a notification; errors (no daemon) are logged and ignored.
    pub async fn send(&self, kind: Kind, summary: &str, body: &str, critical: bool) {
        if !self.allowed(kind) {
            return;
        }
        if let Err(e) = send_raw(summary, body, critical).await {
            tracing::info!("notification not delivered: {e}");
        }
    }
}

/// Sends a notification without rate limit (headless timer mode).
pub async fn send_raw(summary: &str, body: &str, critical: bool) -> zbus::Result<()> {
    let conn = zbus::Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await?;
    let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
    hints.insert("desktop-entry", Value::from(APP_ID));
    hints.insert("urgency", Value::from(if critical { 2u8 } else { 1u8 }));
    let actions: Vec<&str> = Vec::new();
    let _: u32 = proxy
        .call(
            "Notify",
            &(
                "cachyos-center",
                0u32,
                APP_ID,
                summary,
                body,
                actions,
                hints,
                -1i32,
            ),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cachyos_center_core::operation::OperationOrigin;

    #[test]
    fn texts_distinguish_commit_state() {
        let mut op = Operation::new(
            "id".into(),
            OperationKind::SystemUpgrade,
            OperationOrigin::User,
            1,
        );
        op.state = OperationState::Failed;
        let (_, _, body) = operation_texts(&op, true);
        assert_eq!(body, "Paketlage unverändert.");
        op.commit_started = true;
        let (_, _, body) = operation_texts(&op, true);
        assert!(body.contains("unklar"));
        op.state = OperationState::Succeeded;
        op.changes.upgraded = 3;
        let (kind, title, body) = operation_texts(&op, false);
        assert_eq!(kind, Kind::OperationFinished);
        assert_eq!(title, "System upgrade completed");
        assert_eq!(body, "3 packages changed.");
    }

    #[test]
    fn rate_limit() {
        let n = Notifier::default();
        assert!(n.allowed(Kind::UpdatesAvailable));
        assert!(!n.allowed(Kind::UpdatesAvailable));
        assert!(n.allowed(Kind::OperationFailed));
    }
}
