//! logind inhibitor that blocks shutdown and sleep during a commit.
//!
//! Best effort: without logind the commit still runs. The inhibitor is
//! released when the returned value is dropped.

use zbus::zvariant::OwnedFd;

/// Held inhibitor lock (file descriptor from logind).
#[derive(Debug)]
pub struct Inhibitor {
    _fd: OwnedFd,
}

pub async fn acquire() -> Option<Inhibitor> {
    let conn = zbus::Connection::system().await.ok()?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )
    .await
    .ok()?;
    let fd: OwnedFd = proxy
        .call(
            "Inhibit",
            &(
                "shutdown:sleep",
                "cachyos-center",
                "Paketänderungen werden angewendet / package changes are being applied",
                "block",
            ),
        )
        .await
        .map_err(|e| tracing::warn!("cannot take logind inhibitor: {e}"))
        .ok()?;
    Some(Inhibitor { _fd: fd })
}
