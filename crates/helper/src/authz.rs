//! Authorization of D-Bus callers.
//!
//! Every changing method checks the actual bus sender with polkit against a
//! specific action (`org.cachyos-center.*`). There is no blanket rule; the
//! polkit policy file ships `auth_admin_keep` defaults.

use std::collections::HashMap;
use std::time::Duration;

use cachyos_center_core::{AppError, AppResult, ErrorCode};
use zbus::Connection;
use zbus::message::Header;
use zbus_polkit::policykit1::{AuthorityProxy, CheckAuthorizationFlags, Subject};

use crate::config::AuthMode;

/// Identity of the caller, captured when the method is called.
#[derive(Debug, Clone)]
pub struct Caller {
    /// Unique bus name (`:1.42`).
    pub bus_name: String,
    /// Unix uid of the connection (from the bus daemon).
    pub uid: Option<u32>,
}

impl Caller {
    pub async fn from_header(conn: &Connection, header: &Header<'_>) -> AppResult<Self> {
        let sender = header
            .sender()
            .ok_or_else(|| AppError::new(ErrorCode::NotAuthorized, "message has no sender"))?;
        let uid = match zbus::fdo::DBusProxy::new(conn).await {
            Ok(proxy) => proxy
                .get_connection_unix_user(sender.clone().into())
                .await
                .ok(),
            Err(_) => None,
        };
        Ok(Self {
            bus_name: sender.to_string(),
            uid,
        })
    }
}

/// Checks `action` for `caller`. Blocks while the polkit agent asks the user.
pub async fn authorize(
    mode: &AuthMode,
    conn: &Connection,
    caller: &Caller,
    action: &str,
    cancellation_id: &str,
    timeout: Duration,
) -> AppResult<()> {
    match mode {
        AuthMode::TestDeny(denied) => {
            if denied.iter().any(|d| d == action) {
                Err(AppError::new(
                    ErrorCode::NotAuthorized,
                    format!("{action} denied (test mode)"),
                ))
            } else {
                Ok(())
            }
        }
        AuthMode::Polkit => {
            let authority = AuthorityProxy::new(conn).await.map_err(|e| {
                AppError::new(
                    ErrorCode::Unavailable,
                    format!("polkit is not available: {e}"),
                )
            })?;
            let mut subject_details = HashMap::new();
            let name = zbus::names::OwnedUniqueName::try_from(caller.bus_name.clone())
                .map_err(|e| AppError::internal(format!("invalid sender name: {e}")))?;
            subject_details.insert(
                "name".to_string(),
                zbus::zvariant::OwnedValue::try_from(zbus::zvariant::Value::from(name.as_str()))
                    .map_err(|e| AppError::internal(format!("invalid sender name: {e}")))?,
            );
            let subject = Subject {
                subject_kind: "system-bus-name".to_string(),
                subject_details,
            };
            let details: HashMap<&str, &str> = HashMap::new();
            let check = authority.check_authorization(
                &subject,
                action,
                &details,
                CheckAuthorizationFlags::AllowUserInteraction.into(),
                cancellation_id,
            );
            let result = match tokio::time::timeout(timeout, check).await {
                Ok(r) => r,
                Err(_) => {
                    let _ = authority.cancel_check_authorization(cancellation_id).await;
                    return Err(AppError::new(
                        ErrorCode::NotAuthorized,
                        "authentication timed out",
                    ));
                }
            };
            match result {
                Ok(r) if r.is_authorized => Ok(()),
                Ok(r) if r.is_challenge => Err(AppError::new(
                    ErrorCode::NotAuthorized,
                    "authentication required but no polkit agent answered",
                )),
                Ok(_) => Err(AppError::new(ErrorCode::NotAuthorized, "not authorized")),
                Err(e) => {
                    let text = e.to_string();
                    let code = if text.contains("Cancelled") || text.contains("dismissed") {
                        ErrorCode::NotAuthorized
                    } else {
                        ErrorCode::Unavailable
                    };
                    Err(AppError::new(code, format!("polkit check failed: {text}")))
                }
            }
        }
    }
}

/// Cancels a pending polkit dialog (best effort).
pub async fn cancel(mode: &AuthMode, conn: &Connection, cancellation_id: &str) {
    if *mode == AuthMode::Polkit
        && let Ok(authority) = AuthorityProxy::new(conn).await
    {
        let _ = authority.cancel_check_authorization(cancellation_id).await;
    }
}
