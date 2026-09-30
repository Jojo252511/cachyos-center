//! Operation journal (`/var/lib/cachyos-center/operations`).
//!
//! `<id>.json` holds the public [`Operation`] (world-readable, no secrets,
//! no paths). `<id>.state` holds private recovery data (pacman log offset,
//! transient unit) that only the helper reads.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use cachyos_center_core::operation::Operation;
use cachyos_center_core::{AppResult, validate};
use serde::{Deserialize, Serialize};

/// Number of journal entries that are kept.
pub const KEEP: usize = 50;

/// Private recovery data of an operation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryState {
    /// Size of the pacman log before the commit step started.
    pub pacman_log_offset: Option<u64>,
    /// Transient unit of the running step.
    pub unit: Option<String>,
    /// Unix uid of the caller who started the operation.
    pub initiator_uid: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Journal {
    dir: PathBuf,
}

fn write_atomic(path: &Path, content: &[u8], mode: u32) -> AppResult<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(mode)
            .open(&tmp)?;
        f.write_all(content)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

impl Journal {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn ensure_dir(&self) -> AppResult<()> {
        std::fs::create_dir_all(&self.dir)?;
        Ok(())
    }

    pub fn save(&self, op: &Operation) -> AppResult<()> {
        validate::operation_id(&op.id)?;
        self.ensure_dir()?;
        let text = serde_json::to_vec_pretty(op).map_err(|e| {
            cachyos_center_core::AppError::internal(format!("cannot encode operation: {e}"))
        })?;
        write_atomic(&self.dir.join(format!("{}.json", op.id)), &text, 0o644)
    }

    pub fn save_recovery(&self, id: &str, state: &RecoveryState) -> AppResult<()> {
        validate::operation_id(id)?;
        self.ensure_dir()?;
        let text = serde_json::to_vec(state).map_err(|e| {
            cachyos_center_core::AppError::internal(format!("cannot encode state: {e}"))
        })?;
        write_atomic(&self.dir.join(format!("{id}.state")), &text, 0o600)
    }

    pub fn load(&self, id: &str) -> Option<Operation> {
        validate::operation_id(id).ok()?;
        let text = std::fs::read_to_string(self.dir.join(format!("{id}.json"))).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn load_recovery(&self, id: &str) -> RecoveryState {
        std::fs::read_to_string(self.dir.join(format!("{id}.state")))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// All journal entries, newest first.
    pub fn list(&self) -> Vec<Operation> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut ops: Vec<Operation> = entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .filter_map(|t| serde_json::from_str::<Operation>(&t).ok())
            .filter(|op| validate::operation_id(&op.id).is_ok())
            .collect();
        ops.sort_by(|a, b| {
            b.requested_at
                .cmp(&a.requested_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        ops
    }

    /// Removes the oldest terminal entries beyond [`KEEP`].
    pub fn prune(&self, log_dir: &Path) {
        for op in self
            .list()
            .into_iter()
            .skip(KEEP)
            .filter(|o| o.state.is_terminal())
        {
            let _ = std::fs::remove_file(self.dir.join(format!("{}.json", op.id)));
            let _ = std::fs::remove_file(self.dir.join(format!("{}.state", op.id)));
            let _ = std::fs::remove_file(log_dir.join(format!("{}.log", op.id)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cachyos_center_core::operation::{OperationKind, OperationOrigin};

    fn op(n: u32) -> Operation {
        let id = format!("{n:08x}-0000-4000-8000-000000000000");
        let mut op = Operation::new(
            id,
            OperationKind::Install,
            OperationOrigin::User,
            i64::from(n),
        );
        op.state = cachyos_center_core::operation::OperationState::Succeeded;
        op
    }

    #[test]
    fn save_load_list_prune() {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::new(dir.path().join("operations"));
        for n in 0..(KEEP as u32 + 3) {
            j.save(&op(n)).unwrap();
        }
        assert_eq!(j.list().len(), KEEP + 3);
        assert_eq!(j.list()[0].requested_at, i64::from(KEEP as u32 + 2));
        let first = op(0);
        assert_eq!(j.load(&first.id).unwrap().id, first.id);
        j.prune(dir.path());
        assert_eq!(j.list().len(), KEEP);
        assert!(j.load(&first.id).is_none());
    }

    #[test]
    fn recovery_state_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::new(dir.path().to_path_buf());
        let id = op(1).id;
        j.save_recovery(
            &id,
            &RecoveryState {
                pacman_log_offset: Some(42),
                unit: None,
                initiator_uid: Some(1000),
            },
        )
        .unwrap();
        assert_eq!(j.load_recovery(&id).pacman_log_offset, Some(42));
        let mode = std::fs::metadata(dir.path().join(format!("{id}.state")))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(
            j.save_recovery("../../etc/x", &RecoveryState::default())
                .is_err()
        );
    }
}
