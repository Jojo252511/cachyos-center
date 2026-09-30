//! Client of the privileged helper (`org.cachyos_center.Packages1`).
//!
//! Production builds always talk to the system bus. Debug builds can be
//! pointed to a development helper on the session bus with
//! `CACHYOS_CENTER_HELPER_BUS=session`.

use cachyos_center_core::dbus::{BUS_NAME, INTERFACE, OBJECT_PATH, app_error_from_dbus};
use cachyos_center_core::operation::Operation;
use cachyos_center_core::policy::{AutoUpdateConfig, Weekday};
use cachyos_center_core::ui::OperationLogChunk;
use cachyos_center_core::{AppError, AppResult, ErrorCode};
use tokio::sync::OnceCell;

/// `true` when the development helper on the session bus is used.
pub fn development_mode() -> bool {
    cfg!(debug_assertions) && std::env::var("CACHYOS_CENTER_HELPER_BUS").as_deref() == Ok("session")
}

#[derive(Debug, Default)]
pub struct HelperClient {
    conn: OnceCell<zbus::Connection>,
}

fn map_err(e: zbus::Error) -> AppError {
    match e {
        zbus::Error::MethodError(name, msg, _) => {
            let name = name.as_str();
            if name == "org.freedesktop.DBus.Error.ServiceUnknown"
                || name == "org.freedesktop.DBus.Error.NameHasNoOwner"
                || name == "org.freedesktop.DBus.Error.Spawn.ServiceNotFound"
            {
                AppError::new(
                    ErrorCode::Unavailable,
                    "the cachyos-center helper is not installed or cannot be started",
                )
                .with_detail(name.to_string())
            } else if name.starts_with("org.freedesktop.DBus.Error.AccessDenied") {
                AppError::new(
                    ErrorCode::NotAuthorized,
                    "access to the helper was denied by the bus policy",
                )
            } else {
                app_error_from_dbus(name, msg.as_deref())
            }
        }
        other => AppError::new(
            ErrorCode::Unavailable,
            format!("helper communication failed: {other}"),
        ),
    }
}

impl HelperClient {
    async fn conn(&self) -> AppResult<&zbus::Connection> {
        self.conn
            .get_or_try_init(|| async {
                let conn = if development_mode() {
                    zbus::Connection::session().await
                } else {
                    zbus::Connection::system().await
                };
                conn.map_err(|e| AppError::unavailable(format!("cannot connect to D-Bus: {e}")))
            })
            .await
    }

    async fn proxy(&self) -> AppResult<zbus::Proxy<'static>> {
        let conn = self.conn().await?.clone();
        zbus::Proxy::new(&conn, BUS_NAME, OBJECT_PATH, INTERFACE)
            .await
            .map_err(map_err)
    }

    /// The helper can be activated (installed D-Bus service) or is running.
    /// Does not start the helper.
    pub async fn available(&self) -> Result<(), String> {
        let conn = self.conn().await.map_err(|e| e.message)?;
        let dbus = zbus::fdo::DBusProxy::new(conn)
            .await
            .map_err(|e| e.to_string())?;
        let running = dbus
            .name_has_owner(
                BUS_NAME
                    .try_into()
                    .map_err(|e: zbus::names::Error| e.to_string())?,
            )
            .await
            .unwrap_or(false);
        if running {
            return Ok(());
        }
        let activatable = dbus
            .list_activatable_names()
            .await
            .map_err(|e| e.to_string())?
            .iter()
            .any(|n| n.as_str() == BUS_NAME);
        if activatable {
            Ok(())
        } else {
            Err("org.cachyos_center.Packages1 is not installed (no D-Bus activation file)".into())
        }
    }

    pub async fn upgrade(&self, digest: &str, snapshot: bool) -> AppResult<String> {
        self.proxy()
            .await?
            .call("UpgradeSystem", &(digest, snapshot))
            .await
            .map_err(map_err)
    }

    pub async fn install(&self, repository: &str, name: &str, digest: &str) -> AppResult<String> {
        self.proxy()
            .await?
            .call("InstallRepoPackage", &(repository, name, digest))
            .await
            .map_err(map_err)
    }

    pub async fn remove(&self, name: &str, recursive: bool, digest: &str) -> AppResult<String> {
        self.proxy()
            .await?
            .call("RemoveRepoPackage", &(name, recursive, digest))
            .await
            .map_err(map_err)
    }

    pub async fn set_policy(&self, config: &AutoUpdateConfig) -> AppResult<()> {
        config.validate()?;
        let mask = Weekday::to_mask(&config.window.weekdays);
        let ack = config.news_acknowledged_until.unwrap_or(-1);
        self.proxy()
            .await?
            .call::<_, _, ()>(
                "SetAutoUpdatePolicy",
                &(
                    config.policy.as_str(),
                    mask,
                    config.window.time.as_str(),
                    config.require_snapshot,
                    ack,
                ),
            )
            .await
            .map_err(map_err)
    }

    pub async fn status(&self, id: &str) -> AppResult<Operation> {
        let json: String = self
            .proxy()
            .await?
            .call("ReadOperationStatus", &(id,))
            .await
            .map_err(map_err)?;
        serde_json::from_str(&json)
            .map_err(|e| AppError::internal(format!("invalid helper answer: {e}")))
    }

    pub async fn log(&self, id: &str, offset: u64) -> AppResult<OperationLogChunk> {
        let json: String = self
            .proxy()
            .await?
            .call("ReadOperationLog", &(id, offset))
            .await
            .map_err(map_err)?;
        serde_json::from_str(&json)
            .map_err(|e| AppError::internal(format!("invalid helper answer: {e}")))
    }

    pub async fn cancel(&self, id: &str) -> AppResult<Operation> {
        let json: String = self
            .proxy()
            .await?
            .call("CancelOperation", &(id,))
            .await
            .map_err(map_err)?;
        serde_json::from_str(&json)
            .map_err(|e| AppError::internal(format!("invalid helper answer: {e}")))
    }

    pub async fn current(&self) -> AppResult<Option<Operation>> {
        let json: String = self
            .proxy()
            .await?
            .call("CurrentOperation", &())
            .await
            .map_err(map_err)?;
        if json.is_empty() {
            return Ok(None);
        }
        serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| AppError::internal(format!("invalid helper answer: {e}")))
    }
}
