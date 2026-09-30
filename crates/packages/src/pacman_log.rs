//! Parser for `/var/log/pacman.log`.
//!
//! The pacman log is the authoritative record of what happened to the
//! package set. It is used to show transactions made outside of
//! cachyos-center, to determine the time of the last full upgrade, to detect
//! interrupted transactions and to follow the progress of a running
//! transaction. Raw command lines are never exposed because they can contain
//! personal paths.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use cachyos_center_core::Timestamp;
use cachyos_center_core::history::LogOutcome;
use cachyos_center_core::timefmt;

/// Default location of the pacman log.
pub const PACMAN_LOG: &str = "/var/log/pacman.log";

/// Only the last part of large logs is read.
pub const MAX_READ_BYTES: u64 = 8 * 1024 * 1024;

/// Kind of pacman invocation, derived from the command line without keeping it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    SyncUpgrade,
    Sync,
    Remove,
    Upgrade,
    Database,
    Other,
}

/// One event of the pacman log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    Running(CommandKind),
    FullUpgradeStarted,
    TransactionStarted,
    TransactionCompleted,
    TransactionFailed,
    TransactionInterrupted,
    Installed(String, String),
    Upgraded(String, String, String),
    Downgraded(String, String, String),
    Reinstalled(String, String),
    Removed(String, String),
    /// `.pacnew`/`.pacsave` created (the path is not kept).
    ConfigFile,
    /// ALPM-SCRIPTLET output or hook messages (text not kept).
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedEvent {
    pub at: Timestamp,
    pub event: LogEvent,
}

/// A transaction reconstructed from the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogTransaction {
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub outcome: LogOutcome,
    pub command: Option<CommandKind>,
    pub full_upgrade: bool,
    pub installed: u32,
    pub upgraded: u32,
    pub downgraded: u32,
    pub reinstalled: u32,
    pub removed: u32,
    pub config_files: u32,
    /// Affected package names (at most 20).
    pub packages: Vec<String>,
}

/// Summary of a log.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogSummary {
    pub transactions: Vec<LogTransaction>,
    /// End of the last successfully completed full system upgrade.
    pub last_full_upgrade: Option<Timestamp>,
}

impl LogSummary {
    /// The most recent transaction ended unsuccessfully or without an end marker.
    pub fn last_transaction_problem(&self) -> Option<&LogTransaction> {
        self.transactions
            .last()
            .filter(|t| t.outcome != LogOutcome::Completed)
    }
}

fn command_kind(cmd: &str) -> CommandKind {
    // pacman logs e.g. `pacman -S -y -u --config /etc/pacman.conf -- pkg`.
    let flags: Vec<&str> = cmd
        .split_whitespace()
        .take_while(|t| *t != "--")
        .filter(|t| t.starts_with('-') && !t.starts_with("--"))
        .collect();
    let has = |c: char| flags.iter().any(|f| f.contains(c));
    if has('S') && has('u') {
        CommandKind::SyncUpgrade
    } else if has('S') {
        CommandKind::Sync
    } else if has('R') {
        CommandKind::Remove
    } else if has('U') {
        CommandKind::Upgrade
    } else if has('D') {
        CommandKind::Database
    } else {
        CommandKind::Other
    }
}

/// `name (a -> b)` or `name (v)`.
fn parse_pkg(rest: &str) -> Option<(String, String, Option<String>)> {
    let (name, versions) = rest.split_once(" (")?;
    let versions = versions.strip_suffix(')')?;
    let name = name.trim().to_string();
    if cachyos_center_core::validate::package_name(&name).is_err() {
        return None;
    }
    match versions.split_once(" -> ") {
        Some((a, b)) => Some((name, a.to_string(), Some(b.to_string()))),
        None => Some((name, versions.to_string(), None)),
    }
}

/// Parses one log line.
pub fn parse_line(line: &str) -> Option<TimedEvent> {
    let rest = line.strip_prefix('[')?;
    let (ts, rest) = rest.split_once("] ")?;
    let at = timefmt::parse_pacman(ts)?;
    let (tag, msg) = rest.strip_prefix('[')?.split_once("] ")?;
    let msg = msg.trim_end();
    let event = match tag {
        "PACMAN" => {
            if let Some(cmd) = msg.strip_prefix("Running '") {
                LogEvent::Running(command_kind(cmd.trim_end_matches('\'')))
            } else if msg == "starting full system upgrade" {
                LogEvent::FullUpgradeStarted
            } else {
                LogEvent::Other
            }
        }
        "ALPM" => match msg {
            "transaction started" => LogEvent::TransactionStarted,
            "transaction completed" => LogEvent::TransactionCompleted,
            "transaction failed" => LogEvent::TransactionFailed,
            "transaction interrupted" => LogEvent::TransactionInterrupted,
            _ if msg.starts_with("failed to commit transaction") => LogEvent::TransactionFailed,
            _ if msg.starts_with("warning: ")
                && (msg.ends_with(".pacnew") || msg.contains(" saved as ")) =>
            {
                LogEvent::ConfigFile
            }
            _ => {
                let (verb, rest) = msg.split_once(' ')?;
                match (verb, parse_pkg(rest)) {
                    ("installed", Some((n, v, None))) => LogEvent::Installed(n, v),
                    ("reinstalled", Some((n, v, None))) => LogEvent::Reinstalled(n, v),
                    ("removed", Some((n, v, None))) => LogEvent::Removed(n, v),
                    ("upgraded", Some((n, a, Some(b)))) => LogEvent::Upgraded(n, a, b),
                    ("downgraded", Some((n, a, Some(b)))) => LogEvent::Downgraded(n, a, b),
                    _ => LogEvent::Other,
                }
            }
        },
        _ => LogEvent::Other,
    };
    Some(TimedEvent { at, event })
}

/// Parses a complete log text.
pub fn parse_events(text: &str) -> Vec<TimedEvent> {
    text.lines().filter_map(parse_line).collect()
}

/// Groups events into transactions.
pub fn summarize(events: &[TimedEvent]) -> LogSummary {
    let mut summary = LogSummary::default();
    let mut command: Option<CommandKind> = None;
    let mut full_upgrade = false;
    let mut current: Option<LogTransaction> = None;

    let close = |t: &mut Option<LogTransaction>, s: &mut LogSummary| {
        if let Some(tx) = t.take() {
            if tx.full_upgrade && tx.outcome == LogOutcome::Completed {
                s.last_full_upgrade = tx.ended_at.or(Some(tx.started_at));
            }
            s.transactions.push(tx);
        }
    };

    for ev in events {
        match &ev.event {
            LogEvent::Running(kind) => {
                // A new pacman run without end marker of the previous transaction.
                if let Some(tx) = current.as_mut()
                    && tx.ended_at.is_none()
                {
                    tx.outcome = LogOutcome::Unknown;
                }
                close(&mut current, &mut summary);
                command = Some(*kind);
                full_upgrade = false;
            }
            LogEvent::FullUpgradeStarted => full_upgrade = true,
            LogEvent::TransactionStarted => {
                if let Some(tx) = current.as_mut()
                    && tx.ended_at.is_none()
                {
                    tx.outcome = LogOutcome::Unknown;
                }
                close(&mut current, &mut summary);
                current = Some(LogTransaction {
                    started_at: ev.at,
                    ended_at: None,
                    outcome: LogOutcome::Unknown,
                    command,
                    full_upgrade,
                    installed: 0,
                    upgraded: 0,
                    downgraded: 0,
                    reinstalled: 0,
                    removed: 0,
                    config_files: 0,
                    packages: Vec::new(),
                });
            }
            LogEvent::TransactionCompleted
            | LogEvent::TransactionFailed
            | LogEvent::TransactionInterrupted => {
                if let Some(tx) = current.as_mut() {
                    tx.ended_at = Some(ev.at);
                    tx.outcome = match ev.event {
                        LogEvent::TransactionCompleted => LogOutcome::Completed,
                        LogEvent::TransactionFailed => LogOutcome::Failed,
                        _ => LogOutcome::Interrupted,
                    };
                }
                close(&mut current, &mut summary);
                full_upgrade = false;
            }
            LogEvent::Installed(n, _)
            | LogEvent::Upgraded(n, _, _)
            | LogEvent::Downgraded(n, _, _)
            | LogEvent::Reinstalled(n, _)
            | LogEvent::Removed(n, _) => {
                if let Some(tx) = current.as_mut() {
                    match ev.event {
                        LogEvent::Installed(..) => tx.installed += 1,
                        LogEvent::Upgraded(..) => tx.upgraded += 1,
                        LogEvent::Downgraded(..) => tx.downgraded += 1,
                        LogEvent::Reinstalled(..) => tx.reinstalled += 1,
                        _ => tx.removed += 1,
                    }
                    if tx.packages.len() < 20 {
                        tx.packages.push(n.clone());
                    }
                }
            }
            LogEvent::ConfigFile => {
                if let Some(tx) = current.as_mut() {
                    tx.config_files += 1;
                }
            }
            LogEvent::Other => {}
        }
    }
    close(&mut current, &mut summary);
    summary
}

/// Reads (the tail of) a log file.
pub fn read_tail(path: &Path, max_bytes: u64) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity(usize::try_from(len - start).unwrap_or(0));
    file.read_to_end(&mut buf)?;
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if start > 0 {
        // Drop the first, possibly partial line.
        if let Some(pos) = text.find('\n') {
            text.drain(..=pos);
        }
    }
    Ok(text)
}

/// Reads the log from byte `offset` (used to follow a running transaction).
/// Returns the new text and the new offset (end of the last complete line).
pub fn read_from(path: &Path, offset: u64) -> std::io::Result<(String, u64)> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    if len < offset {
        // Log was rotated or truncated.
        return Ok((String::new(), len));
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::new();
    file.take(MAX_READ_BYTES).read_to_end(&mut buf)?;
    let complete = buf.iter().rposition(|b| *b == b'\n').map_or(0, |p| p + 1);
    buf.truncate(complete);
    Ok((String::from_utf8_lossy(&buf).into_owned(), offset + complete as u64))
}

/// Current size of the log (start offset for [`read_from`]).
pub fn size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Reads and summarizes the log at `path`.
pub fn read_summary(path: &Path) -> std::io::Result<LogSummary> {
    let text = read_tail(path, MAX_READ_BYTES)?;
    Ok(summarize(&parse_events(&text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
[2026-09-30T18:20:00+0200] [PACMAN] Running 'pacman -S -y -u --config /etc/pacman.conf --'
[2026-09-30T18:20:01+0200] [PACMAN] synchronizing package lists
[2026-09-30T18:20:05+0200] [PACMAN] starting full system upgrade
[2026-09-30T18:24:13+0200] [ALPM] transaction started
[2026-09-30T18:24:14+0200] [ALPM] upgraded simdjson (1:4.6.11-1.1 -> 1:5.0.1-1.1)
[2026-09-30T18:24:15+0200] [ALPM] installed newdep (1.0-1)
[2026-09-30T18:24:16+0200] [ALPM] warning: /etc/foo.conf installed as /etc/foo.conf.pacnew
[2026-09-30T18:24:17+0200] [ALPM-SCRIPTLET] some output with /home/alice/secret
[2026-09-30T18:24:20+0200] [ALPM] transaction completed
[2026-09-30T18:26:30+0200] [PACMAN] Running 'pacman -U --config /etc/pacman.conf -- /home/alice/.cache/yay/x/x-1-1-x86_64.pkg.tar.zst'
[2026-09-30T18:26:31+0200] [ALPM] transaction started
[2026-09-30T18:26:31+0200] [ALPM] installed x (1-1)
[2026-09-30T18:26:31+0200] [ALPM] transaction completed
[2026-09-30T18:28:04+0200] [PACMAN] Running 'pacman -R -s -u --noconfirm --config /etc/pacman.conf -- a b'
[2026-09-30T18:28:04+0200] [ALPM] transaction started
[2026-09-30T18:28:04+0200] [ALPM] removed a (1-1)
";

    #[test]
    fn groups_transactions() {
        let summary = summarize(&parse_events(SAMPLE));
        assert_eq!(summary.transactions.len(), 3);
        let first = &summary.transactions[0];
        assert!(first.full_upgrade);
        assert_eq!(first.outcome, LogOutcome::Completed);
        assert_eq!(first.upgraded, 1);
        assert_eq!(first.installed, 1);
        assert_eq!(first.config_files, 1);
        assert_eq!(first.command, Some(CommandKind::SyncUpgrade));
        assert_eq!(first.packages, vec!["simdjson", "newdep"]);
        assert_eq!(
            summary.last_full_upgrade,
            timefmt::parse_pacman("2026-09-30T18:24:20+0200")
        );
        assert_eq!(summary.transactions[1].command, Some(CommandKind::Upgrade));
        let last = &summary.transactions[2];
        assert_eq!(last.command, Some(CommandKind::Remove));
        assert_eq!(last.outcome, LogOutcome::Unknown);
        assert!(summary.last_transaction_problem().is_some());
    }

    #[test]
    fn no_paths_are_kept() {
        let summary = summarize(&parse_events(SAMPLE));
        let debug = format!("{summary:?}");
        assert!(!debug.contains("/home"));
        assert!(!debug.contains("/etc/foo.conf"));
    }

    #[test]
    fn failed_and_interrupted() {
        let text = "\
[2026-09-01T10:00:00+0000] [PACMAN] Running 'pacman -S foo'
[2026-09-01T10:00:01+0000] [ALPM] transaction started
[2026-09-01T10:00:02+0000] [ALPM] failed to commit transaction (conflicting files)
[2026-09-01T10:00:03+0000] [PACMAN] Running 'pacman -S bar'
[2026-09-01T10:00:04+0000] [ALPM] transaction started
[2026-09-01T10:00:05+0000] [ALPM] transaction interrupted
";
        let summary = summarize(&parse_events(text));
        assert_eq!(summary.transactions[0].outcome, LogOutcome::Failed);
        assert_eq!(summary.transactions[1].outcome, LogOutcome::Interrupted);
        assert_eq!(summary.last_full_upgrade, None);
    }

    #[test]
    fn command_kinds() {
        assert_eq!(command_kind("pacman -Syu"), CommandKind::SyncUpgrade);
        assert_eq!(command_kind("pacman -S -y -u -- pkg"), CommandKind::SyncUpgrade);
        assert_eq!(command_kind("pacman -S foo"), CommandKind::Sync);
        assert_eq!(command_kind("pacman -D -q --asdeps -- a"), CommandKind::Database);
        assert_eq!(command_kind("pacman -U -- /x"), CommandKind::Upgrade);
    }

    #[test]
    fn read_from_offsets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pacman.log");
        std::fs::write(&path, "line1\nline2\npartial").unwrap();
        let (text, off) = read_from(&path, 0).unwrap();
        assert_eq!(text, "line1\nline2\n");
        assert_eq!(off, 12);
        let (text, off2) = read_from(&path, off).unwrap();
        assert_eq!(text, "");
        assert_eq!(off2, 12);
        let (_, rotated) = read_from(&path, 1000).unwrap();
        assert_eq!(rotated, 19);
    }

    #[test]
    fn reads_the_real_log_if_present() {
        let path = Path::new(PACMAN_LOG);
        if path.exists() {
            let summary = read_summary(path).unwrap();
            // A real system has at least one transaction.
            assert!(!summary.transactions.is_empty());
        }
    }
}
