//! Activity list: operations of the app and the timer plus transactions
//! found in the pacman log that were not started by cachyos-center.

use std::collections::HashMap;
use std::path::Path;

use cachyos_center_core::history::{HistoryEntry, HistorySource, LogOutcome};
use cachyos_center_core::operation::{Operation, OperationKind, OperationOrigin, OperationState};
use cachyos_center_packages::pacman_log::{LogSummary, LogTransaction};

/// Reads the helper journal (`/var/lib/cachyos-center/operations/*.json`).
pub fn read_journal(dir: &Path, limit: usize) -> Vec<Operation> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    files
        .into_iter()
        .take(limit)
        .filter_map(|(_, p)| {
            let text = std::fs::read_to_string(p).ok()?;
            let op: Operation = serde_json::from_str(&text).ok()?;
            cachyos_center_core::validate::operation_id(&op.id).ok()?;
            Some(op)
        })
        .collect()
}

fn kind_text(kind: OperationKind) -> &'static str {
    match kind {
        OperationKind::UpdateCheck => "update check",
        OperationKind::SystemUpgrade => "system upgrade",
        OperationKind::Install => "installation",
        OperationKind::Remove => "removal",
        OperationKind::AutoUpdatePrepare => "offline update preparation",
    }
}

pub fn entry_from_operation(op: &Operation) -> HistoryEntry {
    let source = match op.origin {
        OperationOrigin::User => HistorySource::App,
        OperationOrigin::Timer => HistorySource::Timer,
    };
    let state = serde_json::to_value(op.state)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    HistoryEntry {
        id: op.id.clone(),
        source,
        kind: Some(op.kind),
        origin: Some(op.origin),
        state: Some(op.state),
        log_outcome: None,
        started_at: op.started_at.unwrap_or(op.requested_at),
        ended_at: op.ended_at,
        summary: format!("{}: {state}", kind_text(op.kind)),
        error_code: op.error.as_ref().map(|e| e.code),
        installed: op.changes.installed,
        upgraded: op.changes.upgraded,
        removed: op.changes.removed,
        downgraded: op.changes.downgraded,
        packages: op.changes.packages.iter().take(20).cloned().collect(),
        outcome_unknown: op.outcome_unknown,
    }
}

pub fn entry_from_log(tx: &LogTransaction) -> HistoryEntry {
    let outcome = match tx.outcome {
        LogOutcome::Completed => "completed",
        LogOutcome::Failed => "failed",
        LogOutcome::Interrupted => "interrupted",
        LogOutcome::Unknown => "unknown",
    };
    HistoryEntry {
        id: format!("pacman-log-{}", tx.started_at),
        source: HistorySource::ExternalPacman,
        kind: None,
        origin: None,
        state: None,
        log_outcome: Some(tx.outcome),
        started_at: tx.started_at,
        ended_at: tx.ended_at,
        summary: format!(
            "pacman transaction {outcome}{}",
            if tx.full_upgrade {
                " (full system upgrade)"
            } else {
                ""
            }
        ),
        error_code: None,
        installed: tx.installed,
        upgraded: tx.upgraded,
        removed: tx.removed,
        downgraded: tx.downgraded,
        packages: tx.packages.clone(),
        outcome_unknown: tx.outcome == LogOutcome::Unknown,
    }
}

fn better(a: &Operation, b: &Operation) -> bool {
    // Prefer the record with a terminal state, then the most recent end.
    match (a.state.is_terminal(), b.state.is_terminal()) {
        (true, false) => true,
        (false, true) => false,
        _ => a.ended_at.unwrap_or(0) >= b.ended_at.unwrap_or(0),
    }
}

/// Merges all sources, newest first.
pub fn merge(
    app: Vec<Operation>,
    helper: Vec<Operation>,
    log: Option<&LogSummary>,
    now: i64,
    limit: usize,
) -> Vec<HistoryEntry> {
    let mut ops: HashMap<String, Operation> = HashMap::new();
    for op in app.into_iter().chain(helper) {
        match ops.get(&op.id) {
            Some(existing) if better(existing, &op) => {}
            _ => {
                ops.insert(op.id.clone(), op);
            }
        }
    }
    // Time windows of cachyos-center transactions: log entries inside belong to them.
    let windows: Vec<(i64, i64)> = ops
        .values()
        .filter(|o| {
            matches!(
                o.kind,
                OperationKind::SystemUpgrade
                    | OperationKind::Install
                    | OperationKind::Remove
                    | OperationKind::AutoUpdatePrepare
            )
        })
        .filter(|o| o.commit_started || o.state == OperationState::Succeeded)
        .map(|o| {
            let start = o.started_at.unwrap_or(o.requested_at) - 2;
            let end = o.ended_at.unwrap_or(now) + 2;
            (start, end)
        })
        .collect();
    let mut entries: Vec<HistoryEntry> = ops.values().map(entry_from_operation).collect();
    if let Some(log) = log {
        entries.extend(
            log.transactions
                .iter()
                .filter(|t| {
                    !windows
                        .iter()
                        .any(|(s, e)| t.started_at >= *s && t.started_at <= *e)
                })
                .map(entry_from_log),
        );
    }
    entries.sort_by(|a, b| {
        b.started_at
            .cmp(&a.started_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    entries.truncate(limit);
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(id: &str, start: i64, end: i64, state: OperationState) -> Operation {
        let mut o = Operation::new(
            id.into(),
            OperationKind::SystemUpgrade,
            OperationOrigin::User,
            start,
        );
        o.started_at = Some(start);
        o.ended_at = Some(end);
        o.state = state;
        o.commit_started = true;
        o
    }

    fn tx(start: i64) -> LogTransaction {
        LogTransaction {
            started_at: start,
            ended_at: Some(start + 1),
            outcome: LogOutcome::Completed,
            command: None,
            full_upgrade: false,
            installed: 1,
            upgraded: 0,
            downgraded: 0,
            reinstalled: 0,
            removed: 0,
            config_files: 0,
            packages: vec!["x".into()],
        }
    }

    #[test]
    fn external_transactions_outside_app_windows() {
        let app = vec![op("a", 100, 200, OperationState::Succeeded)];
        let log = LogSummary {
            transactions: vec![tx(150), tx(500)],
            last_full_upgrade: None,
        };
        let merged = merge(app, vec![], Some(&log), 1000, 10);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].source, HistorySource::ExternalPacman);
        assert_eq!(merged[0].started_at, 500);
        assert_eq!(merged[1].source, HistorySource::App);
    }

    #[test]
    fn terminal_record_wins() {
        let running = op("a", 100, 0, OperationState::Installing);
        let mut done = op("a", 100, 300, OperationState::Succeeded);
        done.changes.upgraded = 5;
        let merged = merge(vec![running], vec![done], None, 1000, 10);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].state, Some(OperationState::Succeeded));
        assert_eq!(merged[0].upgraded, 5);
    }

    #[test]
    fn journal_reading() {
        let dir = tempfile::tempdir().unwrap();
        let mut o = op(
            "44444444-4444-4444-8444-444444444444",
            1,
            2,
            OperationState::Failed,
        );
        o.origin = OperationOrigin::Timer;
        std::fs::write(
            dir.path().join(format!("{}.json", o.id)),
            serde_json::to_string(&o).unwrap(),
        )
        .unwrap();
        std::fs::write(dir.path().join("garbage.json"), "{").unwrap();
        let list = read_journal(dir.path(), 10);
        assert_eq!(list.len(), 1);
        assert_eq!(entry_from_operation(&list[0]).source, HistorySource::Timer);
    }
}
