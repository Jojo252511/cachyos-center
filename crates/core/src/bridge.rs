//! Request/response protocol of the libalpm bridge.
//!
//! The bridge (`libcachyos_center_alpm.so`) is the only component that links
//! libalpm. It is loaded at runtime; if libalpm is missing or has an
//! incompatible soname, loading fails and package functions are disabled
//! with an understandable error instead of the whole application failing to
//! start. Requests and responses are JSON documents defined here.

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::package::{PackageRecord, PackageSummary, RepositoryInfo};
use crate::plan::TransactionPlan;
use crate::updates::UpdateCandidate;

/// Protocol version; the loader refuses bridges with a different version.
pub const BRIDGE_PROTOCOL: u32 = 1;

/// libalpm major version (soname) this code base supports.
pub const SUPPORTED_LIBALPM_MAJOR: u32 = 16;

/// Resolved pacman configuration (from `pacman-conf`), passed with every request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlpmConfig {
    pub root_dir: String,
    pub db_path: String,
    pub gpg_dir: String,
    pub cache_dirs: Vec<String>,
    pub architectures: Vec<String>,
    pub ignore_pkgs: Vec<String>,
    pub ignore_groups: Vec<String>,
    pub hold_pkgs: Vec<String>,
    /// Global `SigLevel` values as printed by `pacman-conf` (e.g. `PackageRequired`).
    pub sig_level: Vec<String>,
    pub repos: Vec<RepoConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoConfig {
    pub name: String,
    pub sig_level: Vec<String>,
    pub usage: Vec<String>,
}

/// Context for the classification of updates.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyContext {
    pub running_kernel_pkg: Option<String>,
    pub session_is_hyprland: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    rename_all_fields = "camelCase"
)]
pub enum BridgeRequest {
    Info,
    Repositories {
        config: AlpmConfig,
    },
    ListInstalled {
        config: AlpmConfig,
    },
    Search {
        config: AlpmConfig,
        query: String,
        repository: Option<String>,
        limit: u32,
    },
    Details {
        config: AlpmConfig,
        name: String,
        repository: Option<String>,
        context: ClassifyContext,
    },
    /// Plan of a full system upgrade plus packages held back by the configuration.
    Updates {
        config: AlpmConfig,
        context: ClassifyContext,
    },
    PlanInstall {
        config: AlpmConfig,
        repository: String,
        name: String,
        context: ClassifyContext,
    },
    PlanRemove {
        config: AlpmConfig,
        name: String,
        recursive: bool,
        context: ClassifyContext,
    },
}

/// Result of [`BridgeRequest::Info`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeInfo {
    pub protocol: u32,
    /// Version reported by the loaded libalpm at runtime.
    pub libalpm_version: String,
    /// libalpm version the bridge was compiled against.
    pub built_against: String,
    pub compatible: bool,
    pub bridge_version: String,
}

/// Result of [`BridgeRequest::Updates`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesData {
    pub plan: TransactionPlan,
    pub held_back: Vec<UpdateCandidate>,
}

/// Envelope returned by the bridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status", content = "value")]
pub enum BridgeResponse {
    Ok(serde_json::Value),
    Err(AppError),
}

/// Typed result helpers (used by the loader).
pub type InstalledList = Vec<PackageSummary>;
pub type SearchResult = Vec<PackageSummary>;
pub type Details = PackageRecord;
pub type Repositories = Vec<RepositoryInfo>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_wire_format() {
        let json = serde_json::to_value(BridgeRequest::Info).unwrap();
        assert_eq!(json, serde_json::json!({"type": "info"}));
        let req = BridgeRequest::PlanRemove {
            config: AlpmConfig {
                root_dir: "/".into(),
                db_path: "/var/lib/pacman/".into(),
                gpg_dir: "/etc/pacman.d/gnupg/".into(),
                cache_dirs: vec![],
                architectures: vec!["x86_64".into()],
                ignore_pkgs: vec![],
                ignore_groups: vec![],
                hold_pkgs: vec![],
                sig_level: vec![],
                repos: vec![],
            },
            name: "foo".into(),
            recursive: true,
            context: ClassifyContext::default(),
        };
        let text = serde_json::to_string(&req).unwrap();
        assert!(text.contains("\"type\":\"planRemove\""));
        assert!(text.contains("\"dbPath\""));
        let back: BridgeRequest = serde_json::from_str(&text).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn response_wire_format() {
        let ok = BridgeResponse::Ok(serde_json::json!([1, 2]));
        assert_eq!(
            serde_json::to_value(&ok).unwrap(),
            serde_json::json!({"status": "ok", "value": [1, 2]})
        );
        let err = BridgeResponse::Err(AppError::busy("locked"));
        let v = serde_json::to_value(&err).unwrap();
        assert_eq!(v["status"], "err");
        assert_eq!(v["value"]["code"], "BUSY");
    }
}
