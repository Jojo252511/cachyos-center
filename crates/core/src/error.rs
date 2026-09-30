//! Stable error codes shared by helper, UI and MCP server.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Stable, machine readable error code.
///
/// The UI maps each code to a localized message and a concrete next step.
/// The MCP server reduces the codes to the five public MCP codes via
/// [`ErrorCode::mcp_code`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ErrorCode {
    /// A required component is not available (helper, libalpm bridge, Hyprland session, ...).
    Unavailable,
    /// The package manager or the helper is busy (foreign `db.lck`, running operation).
    Busy,
    /// Data is too old to be presented as the current state.
    Stale,
    /// The platform or library version is not supported.
    Unsupported,
    /// Unexpected internal error.
    Internal,
    /// Input failed validation.
    InvalidInput,
    /// Requested package, repository or operation does not exist.
    NotFound,
    /// Authorization was denied or dismissed (polkit).
    NotAuthorized,
    /// No network connection.
    Offline,
    /// A required program or package is missing (e.g. `pacman-contrib`).
    PrerequisiteMissing,
    /// The package plan computed before commit differs from the confirmed preview.
    PlanChanged,
    /// A conflicting updater or a prepared offline update exists.
    Conflict,
    /// A safety gate blocks the action (news, snapshot, disk space, ...).
    Blocked,
    /// pacman reported an error while executing the transaction.
    TransactionFailed,
    /// pacman cannot plan the transaction (unsatisfied dependencies, conflicts).
    DependencyProblem,
}

impl ErrorCode {
    /// String form as used on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "UNAVAILABLE",
            Self::Busy => "BUSY",
            Self::Stale => "STALE",
            Self::Unsupported => "UNSUPPORTED",
            Self::Internal => "INTERNAL",
            Self::InvalidInput => "INVALID_INPUT",
            Self::NotFound => "NOT_FOUND",
            Self::NotAuthorized => "NOT_AUTHORIZED",
            Self::Offline => "OFFLINE",
            Self::PrerequisiteMissing => "PREREQUISITE_MISSING",
            Self::PlanChanged => "PLAN_CHANGED",
            Self::Conflict => "CONFLICT",
            Self::Blocked => "BLOCKED",
            Self::TransactionFailed => "TRANSACTION_FAILED",
            Self::DependencyProblem => "DEPENDENCY_PROBLEM",
        }
    }

    /// Reduction to the public MCP error codes
    /// (`UNAVAILABLE`, `BUSY`, `STALE`, `UNSUPPORTED`, `INTERNAL`).
    pub fn mcp_code(self) -> &'static str {
        match self {
            Self::Unavailable
            | Self::Offline
            | Self::PrerequisiteMissing
            | Self::NotFound
            | Self::NotAuthorized => "UNAVAILABLE",
            Self::Busy | Self::Conflict => "BUSY",
            Self::Stale | Self::PlanChanged => "STALE",
            Self::Unsupported => "UNSUPPORTED",
            Self::Internal
            | Self::InvalidInput
            | Self::Blocked
            | Self::TransactionFailed
            | Self::DependencyProblem => "INTERNAL",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error value that crosses process boundaries (Tauri IPC, D-Bus, MCP).
///
/// `message` is a technical English description for logs and AI hosts. The UI
/// never shows it as the primary text; it renders a localized text for `code`
/// and offers `message` as detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[error("{code}: {message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    /// Optional technical detail (e.g. sanitized pacman error line).
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unavailable, message)
    }

    pub fn busy(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Busy, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::internal(format!("I/O error: {err}"))
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format_is_screaming_snake_case() {
        let json = serde_json::to_string(&ErrorCode::PrerequisiteMissing).unwrap();
        assert_eq!(json, "\"PREREQUISITE_MISSING\"");
        let code: ErrorCode = serde_json::from_str("\"PLAN_CHANGED\"").unwrap();
        assert_eq!(code, ErrorCode::PlanChanged);
    }

    #[test]
    fn as_str_matches_serde() {
        for code in [
            ErrorCode::Unavailable,
            ErrorCode::Busy,
            ErrorCode::Stale,
            ErrorCode::Unsupported,
            ErrorCode::Internal,
            ErrorCode::InvalidInput,
            ErrorCode::NotFound,
            ErrorCode::NotAuthorized,
            ErrorCode::Offline,
            ErrorCode::PrerequisiteMissing,
            ErrorCode::PlanChanged,
            ErrorCode::Conflict,
            ErrorCode::Blocked,
            ErrorCode::TransactionFailed,
            ErrorCode::DependencyProblem,
        ] {
            let json = serde_json::to_string(&code).unwrap();
            assert_eq!(json, format!("\"{}\"", code.as_str()));
            assert!(matches!(
                code.mcp_code(),
                "UNAVAILABLE" | "BUSY" | "STALE" | "UNSUPPORTED" | "INTERNAL"
            ));
        }
    }

    #[test]
    fn app_error_display() {
        let err = AppError::busy("database locked").with_detail("/var/lib/pacman/db.lck");
        assert_eq!(err.to_string(), "BUSY: database locked");
        assert_eq!(err.detail.as_deref(), Some("/var/lib/pacman/db.lck"));
    }
}
