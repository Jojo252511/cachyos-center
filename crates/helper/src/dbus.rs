//! D-Bus interface `org.cachyos_center.Packages1`.
//!
//! Changing methods take typed values only; no field accepts a command or a
//! file path. Each of them checks the actual bus sender with polkit
//! (inside the operation flow for package operations, before writing for the
//! policy). Read methods return JSON documents of the shared models.

use std::sync::Arc;

use cachyos_center_core::dbus::PROTOCOL_VERSION;
use cachyos_center_core::policy::{AutoUpdateConfig, AutoUpdatePolicy, UpdateWindow, Weekday};
use cachyos_center_core::{AppError, ErrorCode};
use zbus::message::Header;
use zbus::{Connection, interface};

use crate::authz::Caller;
use crate::engine::{Engine, Request};

/// D-Bus errors: `org.cachyos_center.Packages1.Error.<Code>`; the message is
/// the JSON encoded [`AppError`].
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.cachyos_center.Packages1.Error")]
pub enum HelperError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Unavailable(String),
    Busy(String),
    Stale(String),
    Unsupported(String),
    Internal(String),
    InvalidInput(String),
    NotFound(String),
    NotAuthorized(String),
    Offline(String),
    PrerequisiteMissing(String),
    PlanChanged(String),
    Conflict(String),
    Blocked(String),
    TransactionFailed(String),
    DependencyProblem(String),
}

impl From<AppError> for HelperError {
    fn from(e: AppError) -> Self {
        let json = serde_json::to_string(&e).unwrap_or_else(|_| e.message.clone());
        match e.code {
            ErrorCode::Unavailable => Self::Unavailable(json),
            ErrorCode::Busy => Self::Busy(json),
            ErrorCode::Stale => Self::Stale(json),
            ErrorCode::Unsupported => Self::Unsupported(json),
            ErrorCode::Internal => Self::Internal(json),
            ErrorCode::InvalidInput => Self::InvalidInput(json),
            ErrorCode::NotFound => Self::NotFound(json),
            ErrorCode::NotAuthorized => Self::NotAuthorized(json),
            ErrorCode::Offline => Self::Offline(json),
            ErrorCode::PrerequisiteMissing => Self::PrerequisiteMissing(json),
            ErrorCode::PlanChanged => Self::PlanChanged(json),
            ErrorCode::Conflict => Self::Conflict(json),
            ErrorCode::Blocked => Self::Blocked(json),
            ErrorCode::TransactionFailed => Self::TransactionFailed(json),
            ErrorCode::DependencyProblem => Self::DependencyProblem(json),
        }
    }
}

type Result<T> = std::result::Result<T, HelperError>;

fn to_json<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|e| AppError::internal(e.to_string()).into())
}

/// The exported object.
#[derive(Debug)]
pub struct Packages {
    pub engine: Arc<Engine>,
}

#[interface(name = "org.cachyos_center.Packages1")]
impl Packages {
    /// Full system upgrade (`pacman -Syu`) of the confirmed plan.
    async fn upgrade_system(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        plan_digest: String,
        create_snapshot: bool,
    ) -> Result<String> {
        let caller = Caller::from_header(conn, &header).await?;
        Ok(self.engine.start(
            Request::Upgrade {
                digest: plan_digest,
                snapshot: create_snapshot,
            },
            caller,
        )?)
    }

    /// Installs `repository/name` with a consistent `pacman -Syu --needed`.
    async fn install_repo_package(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        repository: String,
        name: String,
        plan_digest: String,
    ) -> Result<String> {
        let caller = Caller::from_header(conn, &header).await?;
        Ok(self.engine.start(
            Request::Install {
                repository,
                name,
                digest: plan_digest,
            },
            caller,
        )?)
    }

    /// Removes an installed repository package (`pacman -R`, optionally `-Rs`).
    async fn remove_repo_package(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        name: String,
        remove_unneeded_dependencies: bool,
        plan_digest: String,
    ) -> Result<String> {
        let caller = Caller::from_header(conn, &header).await?;
        Ok(self.engine.start(
            Request::Remove {
                name,
                recursive: remove_unneeded_dependencies,
                digest: plan_digest,
            },
            caller,
        )?)
    }

    /// Sets the automatic update policy.
    /// `policy`: `off` | `notifyOnly` | `prepareForNextReboot`;
    /// `weekdays`: bit mask, Monday = bit 0; `time`: `HH:MM`;
    /// `news_acknowledged_until`: Unix time or -1.
    #[allow(clippy::too_many_arguments)]
    async fn set_auto_update_policy(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        policy: String,
        weekdays: u8,
        time: String,
        require_snapshot: bool,
        news_acknowledged_until: i64,
    ) -> Result<()> {
        self.engine.touch();
        let caller = Caller::from_header(conn, &header).await?;
        if time.len() > 5 || weekdays & 0x80 != 0 {
            return Err(AppError::invalid("invalid schedule").into());
        }
        let config = AutoUpdateConfig {
            policy: AutoUpdatePolicy::parse(&policy)?,
            window: UpdateWindow {
                weekdays: Weekday::from_mask(weekdays),
                time,
            },
            require_snapshot,
            news_acknowledged_until: (news_acknowledged_until >= 0)
                .then_some(news_acknowledged_until),
        };
        crate::policy::apply(self.engine.config(), conn, &caller, config).await?;
        Ok(())
    }

    /// State of an operation as JSON (`Operation`).
    async fn read_operation_status(&self, id: String) -> Result<String> {
        self.engine.touch();
        let op = self.engine.status(&id)?;
        to_json(&op)
    }

    /// Log lines of an operation from `offset` as JSON (`OperationLogChunk`).
    async fn read_operation_log(&self, id: String, offset: u64) -> Result<String> {
        self.engine.touch();
        let chunk = self.engine.read_log(&id, offset)?;
        to_json(&chunk)
    }

    /// Cancels an operation before its commit phase.
    async fn cancel_operation(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        id: String,
    ) -> Result<String> {
        let caller = Caller::from_header(conn, &header).await?;
        let op = self.engine.cancel(&id, &caller).await?;
        to_json(&op)
    }

    /// The running or most recent operation as JSON, or an empty string.
    async fn current_operation(&self) -> Result<String> {
        self.engine.touch();
        match self.engine.current() {
            Some(op) => to_json(&op),
            None => Ok(String::new()),
        }
    }

    #[zbus(property)]
    async fn version(&self) -> u32 {
        PROTOCOL_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cachyos_center_core::dbus::app_error_from_dbus;

    #[test]
    fn error_roundtrip() {
        let err = AppError::busy("locked").with_detail("/var/lib/pacman/db.lck");
        let dbus: HelperError = err.clone().into();
        let (name, msg) = match &dbus {
            HelperError::Busy(m) => ("org.cachyos_center.Packages1.Error.Busy", m.clone()),
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!(app_error_from_dbus(name, Some(&msg)), err);
        let fallback = app_error_from_dbus(
            "org.cachyos_center.Packages1.Error.PlanChanged",
            Some("plain"),
        );
        assert_eq!(fallback.code, ErrorCode::PlanChanged);
        let unknown = app_error_from_dbus("org.freedesktop.DBus.Error.ServiceUnknown", None);
        assert_eq!(unknown.code, ErrorCode::Internal);
    }
}
