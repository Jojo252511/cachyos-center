//! Background work of the desktop app: periodic update check, watching of
//! helper operations and Hyprland events. Nothing here polls Hyprland: its
//! event socket is used instead.

use std::sync::Arc;
use std::time::Duration;

use cachyos_center_core::now;
use cachyos_center_core::updates::CheckStatus;
use tauri::{AppHandle, Emitter, Manager};

use crate::notify::{self, Kind};
use crate::state::AppState;

/// Starts the background tasks.
pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    let core = Arc::clone(&state.core);
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = core.prune() {
            tracing::warn!("history pruning failed: {e}");
        }
    });
    spawn_periodic_check(app.clone());
    spawn_hyprland_events(app.clone());
}

fn spawn_periodic_check(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Give the UI time to start before any network access.
        tokio::time::sleep(Duration::from_secs(20)).await;
        let mut last_notified: Option<String> = None;
        loop {
            let state = app.state::<AppState>();
            let core = Arc::clone(&state.core);
            let due = tauri::async_runtime::spawn_blocking(move || {
                let settings = core.settings();
                if settings.check_interval_hours == 0 {
                    return None;
                }
                let last = core.last_check();
                let base = last.attempted_at.or(last.checked_at);
                let interval = i64::from(settings.check_interval_hours) * 3600;
                let due = base.is_none_or(|t| now().saturating_sub(t) >= interval);
                due.then_some(settings)
            })
            .await
            .ok()
            .flatten();
            if let Some(settings) = due {
                let core = Arc::clone(&state.core);
                if let Ok(result) =
                    tauri::async_runtime::spawn_blocking(move || core.check_updates()).await
                {
                    let _ = app.emit("updates-checked", &result);
                    let fingerprint = result.plan.as_ref().map(|p| p.digest.clone());
                    if settings.notifications
                        && result.status == CheckStatus::Fresh
                        && !result.updates.is_empty()
                        && fingerprint != last_notified
                    {
                        let de = notify::german(&settings);
                        let n = result.updates.len();
                        let (title, body) = if de {
                            (
                                format!("{n} Updates verfügbar"),
                                "Details und Installation in cachyos-center.".to_string(),
                            )
                        } else {
                            (
                                format!("{n} updates available"),
                                "Details and installation in cachyos-center.".to_string(),
                            )
                        };
                        state
                            .notifier
                            .send(Kind::UpdatesAvailable, &title, &body, false)
                            .await;
                        last_notified = fingerprint;
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

fn spawn_hyprland_events(app: AppHandle) {
    let Ok(ipc) = cachyos_center_system::hyprland::HyprIpc::discover() else {
        return;
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    std::thread::spawn(move || {
        let Ok(events) = ipc.events() else {
            return;
        };
        for line in events {
            if cachyos_center_system::hyprland::is_relevant_event(&line) && tx.send(()).is_err() {
                break;
            }
        }
    });
    tauri::async_runtime::spawn(async move {
        while rx.recv().await.is_some() {
            // Debounce bursts (e.g. several workspace events on a monitor change).
            tokio::time::sleep(Duration::from_millis(300)).await;
            while rx.try_recv().is_ok() {}
            let _ = app.emit("hyprland-changed", ());
        }
    });
}

/// Watches an operation until it ends: records it in the history, sends a
/// notification and emits `operation-finished`.
pub fn watch_operation(app: &AppHandle, state: &AppState, id: String) {
    {
        let mut watched = state.watched.lock().unwrap_or_else(|e| e.into_inner());
        if !watched.insert(id.clone()) {
            return;
        }
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let mut failures = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            match state.helper.status(&id).await {
                Ok(op) if op.state.is_terminal() => {
                    let core = Arc::clone(&state.core);
                    let record = op.clone();
                    let settings = tauri::async_runtime::spawn_blocking(move || {
                        if let Err(e) = core.history().record(&record) {
                            tracing::warn!("cannot record operation: {e}");
                        }
                        core.settings()
                    })
                    .await
                    .unwrap_or_default();
                    if settings.notifications {
                        let (kind, title, body) =
                            notify::operation_texts(&op, notify::german(&settings));
                        let critical = matches!(kind, Kind::NeedsAttention);
                        state.notifier.send(kind, &title, &body, critical).await;
                    }
                    let _ = app.emit("operation-finished", &op);
                    break;
                }
                Ok(_) => failures = 0,
                Err(e) => {
                    failures += 1;
                    if failures > 30 {
                        tracing::warn!("stopped watching operation {id}: {e}");
                        break;
                    }
                }
            }
        }
        state
            .watched
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
    });
}
