//! Update check with an isolated sync database.
//!
//! `checkupdates` (pacman-contrib) synchronizes a *copy* of the sync
//! databases into `CHECKUPDATES_DB` and never touches the productive
//! database, so no partial-upgrade state (`pacman -Sy` without `-u`) is left
//! behind. The resulting database is then evaluated with libalpm.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use cachyos_center_core::{AppError, AppResult, ErrorCode, Timestamp};
use serde::{Deserialize, Serialize};

/// `checkupdates` binary shipped by pacman-contrib.
pub const CHECKUPDATES: &str = "/usr/bin/checkupdates";
/// Default timeout of an update check.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(180);

/// Persistent metadata of the update check.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckState {
    pub checked_at: Option<Timestamp>,
    pub attempted_at: Option<Timestamp>,
    pub error: Option<AppError>,
}

impl CheckState {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::internal(format!("cannot encode check state: {e}")))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// The last attempt failed after the last success.
    pub fn last_attempt_failed(&self) -> bool {
        self.error.is_some() && self.attempted_at.unwrap_or(0) >= self.checked_at.unwrap_or(0)
    }
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into());
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// Packages that must be installed for the update check.
pub fn missing_prerequisites() -> Vec<String> {
    let mut missing = Vec::new();
    if !Path::new(CHECKUPDATES).is_file() {
        missing.push("pacman-contrib".to_string());
    }
    if find_in_path("fakeroot").is_none() {
        missing.push("fakeroot".to_string());
    }
    missing
}

/// `true` when a default route exists (IPv4 or IPv6).
pub fn has_default_route() -> bool {
    let v4 = std::fs::read_to_string("/proc/net/route")
        .map(|t| {
            t.lines()
                .skip(1)
                .any(|l| l.split_whitespace().nth(1) == Some("00000000"))
        })
        .unwrap_or(true);
    let v6 = std::fs::read_to_string("/proc/net/ipv6_route")
        .map(|t| {
            t.lines().any(|l| {
                let mut cols = l.split_whitespace();
                cols.next() == Some("00000000000000000000000000000000")
                    && cols.next() == Some("00")
                    && l.split_whitespace().last() != Some("lo")
            })
        })
        .unwrap_or(false);
    v4 || v6
}

#[allow(unsafe_code)]
fn send_sigterm(pid: u32) {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return;
    };
    // SAFETY: plain signal delivery to our own child process.
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
}

fn terminate(child: &mut std::process::Child) {
    // SIGTERM first: checkupdates removes its private db.lck in its trap.
    send_sigterm(child.id());
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs `checkupdates` against `check_db`. Returns `Ok(true)` when updates
/// are available and `Ok(false)` when the system is up to date.
///
/// Only one check runs at a time (file lock next to the database).
pub fn run_checkupdates(check_db: &Path, timeout: Duration) -> AppResult<bool> {
    let missing = missing_prerequisites();
    if !missing.is_empty() {
        return Err(AppError::new(
            ErrorCode::PrerequisiteMissing,
            format!("required packages are missing: {}", missing.join(", ")),
        ));
    }
    std::fs::create_dir_all(check_db)?;
    let lock_path = check_db.with_extension("lock");
    let lock = std::fs::File::create(&lock_path)?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            return Err(AppError::busy("another update check is running"));
        }
        Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
    }

    let mut child = Command::new(CHECKUPDATES)
        .arg("--nocolor")
        .env("CHECKUPDATES_DB", check_db)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::unavailable(format!("cannot start checkupdates: {e}")))?;

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > timeout => {
                terminate(&mut child);
                return Err(AppError::unavailable(format!(
                    "update check timed out after {} s",
                    timeout.as_secs()
                )));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.into()),
        }
    };
    let mut stderr = String::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }
    drop(lock);

    match status.code() {
        Some(0) => Ok(true),
        Some(2) => Ok(false),
        _ => {
            let detail = stderr
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .take(5)
                .collect::<Vec<_>>()
                .join("\n");
            let err = if !has_default_route() {
                AppError::new(ErrorCode::Offline, "no network connection")
            } else {
                AppError::unavailable(
                    "the package databases could not be synchronized (mirror, network or signature problem)",
                )
            };
            Err(if detail.is_empty() {
                err
            } else {
                err.with_detail(detail)
            })
        }
    }
}

/// `true` when `check_db` contains synchronized databases.
pub fn has_synced_db(check_db: &Path) -> bool {
    std::fs::read_dir(check_db.join("sync"))
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.path().extension().is_some_and(|x| x == "db"))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip_and_failure_flag() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        assert_eq!(CheckState::load(&path), CheckState::default());
        let state = CheckState {
            checked_at: Some(100),
            attempted_at: Some(200),
            error: Some(AppError::new(ErrorCode::Offline, "offline")),
        };
        state.save(&path).unwrap();
        let loaded = CheckState::load(&path);
        assert_eq!(loaded, state);
        assert!(loaded.last_attempt_failed());
        let ok = CheckState {
            checked_at: Some(300),
            attempted_at: Some(300),
            error: None,
        };
        assert!(!ok.last_attempt_failed());
    }

    #[test]
    fn synced_db_detection() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!has_synced_db(dir.path()));
        std::fs::create_dir_all(dir.path().join("sync")).unwrap();
        std::fs::write(dir.path().join("sync/core.db"), b"").unwrap();
        assert!(has_synced_db(dir.path()));
    }
}
