//! Shared domain model of cachyos-center.
//!
//! This crate contains no system access. It defines the data structures that
//! travel between the privileged helper, the desktop UI and the MCP server,
//! the stable error codes, input validation and the operation state machine.
//! Every model that crosses the frontend boundary derives [`ts_rs::TS`]; the
//! TypeScript bindings are regenerated with `cargo test -p cachyos-center-core`.

pub mod bridge;
pub mod classify;
pub mod dbus;
pub mod error;
pub mod health;
pub mod history;
pub mod hyprland;
pub mod mcp;
pub mod news;
pub mod operation;
pub mod package;
pub mod paths;
pub mod plan;
pub mod policy;
pub mod sanitize;
pub mod settings;
pub mod system;
pub mod timefmt;
pub mod ui;
pub mod updates;
pub mod validate;

pub use error::{AppError, AppResult, ErrorCode};

/// Unix timestamp in seconds (UTC). Used for every point in time in the model.
pub type Timestamp = i64;

/// Current time as [`Timestamp`].
pub fn now() -> Timestamp {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Version of this build, shared by all binaries.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Human readable application name.
pub const APP_NAME: &str = "cachyos-center";

/// Desktop application identifier (desktop file name, Wayland app-id, icon name).
///
/// D-Bus names cannot contain hyphens, therefore the D-Bus namespace is
/// `org.cachyos_center` (see [`dbus`]) while desktop and polkit identifiers use
/// the reverse-DNS form `org.cachyos-center`.
pub const APP_ID: &str = "org.cachyos-center.CachyOSCenter";

pub mod dashboard;
