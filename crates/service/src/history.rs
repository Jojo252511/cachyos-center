//! Operation history in SQLite (`$XDG_DATA_HOME/cachyos-center/history.sqlite3`).
//!
//! Only cachyos-center operations are stored; package lists are read from the
//! system. Entries may contain "unknown" outcomes when a result could not be
//! reconstructed with certainty.

use std::path::{Path, PathBuf};

use cachyos_center_core::operation::Operation;
use cachyos_center_core::{AppError, AppResult, Timestamp};
use rusqlite::{Connection, OpenFlags, params};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone)]
pub struct HistoryStore {
    path: PathBuf,
}

fn db_err(e: rusqlite::Error) -> AppError {
    AppError::internal(format!("history database: {e}"))
}

impl HistoryStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn open_rw(&self) -> AppResult<Connection> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(&self.path).map_err(db_err)?;
        conn.busy_timeout(std::time::Duration::from_secs(2))
            .map_err(db_err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_err)?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db_err)?;
        if version > SCHEMA_VERSION {
            return Err(AppError::new(
                cachyos_center_core::ErrorCode::Unsupported,
                "history database was created by a newer version",
            ));
        }
        if version < 1 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS operations (
                    id TEXT PRIMARY KEY,
                    kind TEXT NOT NULL,
                    origin TEXT NOT NULL,
                    state TEXT NOT NULL,
                    requested_at INTEGER NOT NULL,
                    ended_at INTEGER,
                    data TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_operations_requested ON operations(requested_at DESC);
                PRAGMA user_version = 1;",
            )
            .map_err(db_err)?;
        }
        Ok(conn)
    }

    fn open_ro(&self) -> AppResult<Option<Connection>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(db_err)?;
        conn.busy_timeout(std::time::Duration::from_secs(2))
            .map_err(db_err)?;
        Ok(Some(conn))
    }

    /// Inserts or updates an operation.
    pub fn record(&self, op: &Operation) -> AppResult<()> {
        cachyos_center_core::validate::operation_id(&op.id)?;
        let conn = self.open_rw()?;
        let data = serde_json::to_string(op)
            .map_err(|e| AppError::internal(format!("cannot encode operation: {e}")))?;
        let enum_str = |v: serde_json::Value| v.as_str().unwrap_or_default().to_string();
        conn.execute(
            "INSERT INTO operations (id, kind, origin, state, requested_at, ended_at, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET state = excluded.state, ended_at = excluded.ended_at, data = excluded.data",
            params![
                op.id,
                enum_str(serde_json::to_value(op.kind).unwrap_or_default()),
                enum_str(serde_json::to_value(op.origin).unwrap_or_default()),
                enum_str(serde_json::to_value(op.state).unwrap_or_default()),
                op.requested_at,
                op.ended_at,
                data
            ],
        )
        .map_err(db_err)?;
        Ok(())
    }

    /// Most recent operations first. A missing database is an empty history.
    pub fn recent(&self, limit: u32, offset: u32) -> AppResult<Vec<Operation>> {
        let Some(conn) = self.open_ro()? else {
            return Ok(Vec::new());
        };
        let mut stmt = conn
            .prepare(
                "SELECT data FROM operations ORDER BY requested_at DESC, id LIMIT ?1 OFFSET ?2",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map(params![limit, offset], |r| r.get::<_, String>(0))
            .map_err(db_err)?;
        let mut out = Vec::new();
        for row in rows {
            let text = row.map_err(db_err)?;
            match serde_json::from_str::<Operation>(&text) {
                Ok(op) => out.push(op),
                Err(e) => tracing::warn!("skipping unreadable history entry: {e}"),
            }
        }
        Ok(out)
    }

    pub fn get(&self, id: &str) -> AppResult<Option<Operation>> {
        let Some(conn) = self.open_ro()? else {
            return Ok(None);
        };
        let text: Option<String> = conn
            .query_row(
                "SELECT data FROM operations WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(db_err(other)),
            })?;
        Ok(text.and_then(|t| serde_json::from_str(&t).ok()))
    }

    /// Removes entries requested before `before`. Returns the number of rows.
    pub fn prune(&self, before: Timestamp) -> AppResult<usize> {
        if !self.path.exists() {
            return Ok(0);
        }
        let conn = self.open_rw()?;
        conn.execute(
            "DELETE FROM operations WHERE requested_at < ?1",
            params![before],
        )
        .map_err(db_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cachyos_center_core::operation::{OperationKind, OperationOrigin, OperationState};

    fn op(id: &str, at: i64) -> Operation {
        Operation::new(
            id.into(),
            OperationKind::SystemUpgrade,
            OperationOrigin::User,
            at,
        )
    }

    const A: &str = "11111111-1111-4111-8111-111111111111";
    const B: &str = "22222222-2222-4222-8222-222222222222";

    #[test]
    fn record_update_list_prune() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::new(dir.path().join("h/history.sqlite3"));
        assert!(store.recent(10, 0).unwrap().is_empty());
        store.record(&op(A, 100)).unwrap();
        store.record(&op(B, 200)).unwrap();
        let mut a = op(A, 100);
        a.transition(OperationState::AwaitingAuthorization, 101)
            .unwrap();
        store.record(&a).unwrap();
        let list = store.recent(10, 0).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, B, "newest first");
        assert_eq!(list[1].state, OperationState::AwaitingAuthorization);
        assert_eq!(store.get(A).unwrap().unwrap().id, A);
        assert!(
            store
                .get("33333333-3333-4333-8333-333333333333")
                .unwrap()
                .is_none()
        );
        assert_eq!(store.prune(150).unwrap(), 1);
        assert_eq!(store.recent(10, 0).unwrap().len(), 1);
    }

    #[test]
    fn rejects_invalid_ids() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::new(dir.path().join("history.sqlite3"));
        assert!(store.record(&op("../x", 1)).is_err());
    }
}
