//! State of the pacman database lock.
//!
//! cachyos-center never removes `db.lck`. A lock without a running package
//! manager is reported so that the user can repair it deliberately.

use std::path::Path;
use std::time::UNIX_EPOCH;

use cachyos_center_core::system::LockStatus;

/// Process names (`/proc/<pid>/comm`, max. 15 characters) that hold or may
/// hold the pacman lock.
const PACKAGE_MANAGERS: [&str; 8] = [
    "pacman",
    "yay",
    "paru",
    "pamac-daemon",
    "packagekitd",
    "shelly",
    "octopi",
    "pikaur",
];

/// Reads the lock state of `lock_file`.
pub fn status(lock_file: &Path) -> LockStatus {
    match std::fs::metadata(lock_file) {
        Ok(meta) => LockStatus::Locked {
            since: meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .and_then(|d| i64::try_from(d.as_secs()).ok()),
            holder_running: package_manager_running(),
        },
        Err(_) => LockStatus::Free,
    }
}

/// `true` when the lock file exists.
pub fn is_locked(lock_file: &Path) -> bool {
    lock_file.exists()
}

/// Looks for a running package manager process in `/proc`.
/// Returns `None` when `/proc` cannot be read.
pub fn package_manager_running() -> Option<bool> {
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str() else { continue };
        if !pid.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) else {
            continue;
        };
        let comm = comm.trim();
        if PACKAGE_MANAGERS.iter().any(|p| comm == *p) {
            return Some(true);
        }
    }
    Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_and_locked() {
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("db.lck");
        assert_eq!(status(&lock), LockStatus::Free);
        std::fs::write(&lock, "").unwrap();
        match status(&lock) {
            LockStatus::Locked { since, .. } => assert!(since.is_some()),
            other => panic!("unexpected {other:?}"),
        }
        assert!(is_locked(&lock));
    }
}
