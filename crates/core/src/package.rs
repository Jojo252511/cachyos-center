//! Package model (`PackageId`, `PackageSummary`, `PackageRecord`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Timestamp;

/// Where an installed or available package comes from.
///
/// `LocalOrAur` is determined via "foreign packages" (installed but not present
/// in any configured sync repository). This is only an approximation: the UI
/// labels it "Lokal/AUR (Quelle nicht sicher bestimmbar)".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PackageOrigin {
    Repo,
    LocalOrAur,
    /// Reserved for a later, separately labelled Flatpak section (P2).
    Flatpak,
}

/// Install reason as recorded in the local pacman database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum InstallReason {
    Explicit,
    Dependency,
}

/// `repository + packageName + architecture`.
///
/// For foreign packages the repository is `"local"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackageId {
    pub repository: String,
    pub name: String,
    pub architecture: String,
}

/// Compact list entry for installed packages and repository search results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackageSummary {
    pub name: String,
    pub description: String,
    /// Sync repository providing the package (`None` for foreign packages).
    pub repository: Option<String>,
    pub origin: PackageOrigin,
    pub architecture: String,
    pub installed_version: Option<String>,
    /// Version in the sync repository (newest over all repositories in pacman order).
    pub available_version: Option<String>,
    pub install_reason: Option<InstallReason>,
    #[ts(type = "number | null")]
    pub installed_size: Option<u64>,
    /// Installed and a newer version is available in a sync repository.
    pub update_available: bool,
    /// Listed in `IgnorePkg`/`IgnoreGroup` (held back by pacman configuration).
    pub ignored: bool,
}

/// Full package record shown in the details view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackageRecord {
    pub id: PackageId,
    pub description: String,
    pub origin: PackageOrigin,
    pub installed_version: Option<String>,
    pub available_version: Option<String>,
    pub install_reason: Option<InstallReason>,
    pub url: Option<String>,
    pub licenses: Vec<String>,
    pub groups: Vec<String>,
    pub dependencies: Vec<String>,
    pub optional_dependencies: Vec<String>,
    pub required_by: Vec<String>,
    pub optional_for: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    #[ts(type = "number | null")]
    pub installed_size: Option<u64>,
    #[ts(type = "number | null")]
    pub download_size: Option<u64>,
    pub packager: Option<String>,
    #[ts(type = "number | null")]
    pub build_date: Option<Timestamp>,
    #[ts(type = "number | null")]
    pub install_date: Option<Timestamp>,
    /// Package is considered system critical and gets an additional warning before removal.
    pub critical: bool,
    pub ignored: bool,
}

/// Filter for the installed package list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum InstalledFilter {
    #[default]
    All,
    Explicit,
    Dependency,
    Repo,
    LocalOrAur,
    UpdateAvailable,
}

/// Query for the installed package list.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstalledQuery {
    pub query: Option<String>,
    pub filter: InstalledFilter,
    pub offset: u32,
    pub limit: u32,
}

/// Installation status filter for the repository catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CatalogInstallFilter {
    #[default]
    Any,
    Installed,
    NotInstalled,
}

/// Query for the repository catalog (configured pacman repositories only).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogQuery {
    pub query: String,
    pub repository: Option<String>,
    pub install_filter: CatalogInstallFilter,
    pub limit: u32,
}

/// One page of results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackagePage {
    pub items: Vec<PackageSummary>,
    pub total: u32,
    pub offset: u32,
    /// Offset of the next page, `None` on the last page.
    pub next_offset: Option<u32>,
}

/// Reference to a package for the details view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackageRef {
    pub name: String,
    /// Sync repository to look in; `None` looks at the installed package first.
    pub repository: Option<String>,
}

/// Configured sync repository as reported by `pacman-conf`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepositoryInfo {
    pub name: String,
    #[ts(type = "number")]
    pub package_count: u64,
}
