//! System information model.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OsInfo {
    pub id: String,
    pub name: String,
    pub pretty_name: String,
    pub build_id: Option<String>,
    pub is_cachyos: bool,
    /// `ID=arch` or `ID_LIKE` contains `arch`.
    pub is_arch_based: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KernelInfo {
    pub release: String,
    /// Package base of the running kernel (`/usr/lib/modules/$(uname -r)/pkgbase`).
    pub package: Option<String>,
    #[ts(type = "number")]
    pub uptime_seconds: u64,
    /// The module directory of the running kernel is missing (kernel was updated).
    pub modules_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CpuInfo {
    pub model: String,
    pub vendor: Option<String>,
    pub cores: u32,
    pub threads: u32,
    /// Highest supported x86-64 micro-architecture level (e.g. `x86-64-v3`).
    pub isa_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GpuInfo {
    pub vendor: String,
    pub model: String,
    pub driver: Option<String>,
    /// PCI id `vvvv:dddd` (no serial numbers).
    pub pci_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryInfo {
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number")]
    pub available_bytes: u64,
    #[ts(type = "number")]
    pub swap_total_bytes: u64,
    #[ts(type = "number")]
    pub swap_free_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiskInfo {
    /// Mount point (`/`, `/var/cache/pacman/pkg`, ...). Never a home directory.
    pub mount_point: String,
    pub filesystem: String,
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number")]
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SessionKind {
    Hyprland,
    OtherWayland,
    X11,
    Tty,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionInfo {
    pub kind: SessionKind,
    /// `XDG_CURRENT_DESKTOP` (e.g. `Hyprland`, `KDE`).
    pub desktop: Option<String>,
    /// `XDG_SESSION_TYPE` (`wayland`, `x11`, `tty`).
    pub session_type: Option<String>,
}

/// State of the pacman database lock (`db.lck`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    tag = "state",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum LockStatus {
    Free,
    Locked {
        #[ts(type = "number | null")]
        since: Option<Timestamp>,
        /// A running pacman-like process was found (`None` when not determinable).
        holder_running: Option<bool>,
    },
}

/// Availability of the libalpm bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    tag = "state",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum BackendStatus {
    Ready {
        libalpm_version: String,
        built_against: String,
    },
    /// Package functions are disabled; `reason` explains why (e.g. ABI mismatch).
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PacmanStatus {
    pub pacman_version: Option<String>,
    pub backend: BackendStatus,
    pub lock: LockStatus,
    #[ts(type = "number | null")]
    pub last_full_upgrade: Option<Timestamp>,
    #[ts(type = "number")]
    pub installed_count: u64,
    #[ts(type = "number")]
    pub foreign_count: u64,
    pub repositories: Vec<String>,
}

/// Complete system information for the "System" page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemInfo {
    pub os: OsInfo,
    pub kernel: KernelInfo,
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub session: SessionInfo,
    pub pacman: PacmanStatus,
    #[ts(type = "number")]
    pub collected_at: Timestamp,
}

/// Reduced summary for the dashboard and the MCP tool `system_get_summary`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemSummary {
    pub os: String,
    pub is_cachyos: bool,
    pub kernel: String,
    #[ts(type = "number")]
    pub uptime_seconds: u64,
    pub cpu: String,
    pub gpus: Vec<String>,
    #[ts(type = "number")]
    pub memory_total_bytes: u64,
    #[ts(type = "number")]
    pub memory_available_bytes: u64,
    #[ts(type = "number")]
    pub root_total_bytes: u64,
    #[ts(type = "number")]
    pub root_available_bytes: u64,
    pub root_filesystem: String,
    pub session: SessionKind,
    pub desktop: Option<String>,
    pub pacman_version: Option<String>,
    pub package_backend_ready: bool,
}

impl SystemInfo {
    pub fn summary(&self) -> SystemSummary {
        let root = self.disks.iter().find(|d| d.mount_point == "/");
        SystemSummary {
            os: self.os.pretty_name.clone(),
            is_cachyos: self.os.is_cachyos,
            kernel: self.kernel.release.clone(),
            uptime_seconds: self.kernel.uptime_seconds,
            cpu: self.cpu.model.clone(),
            gpus: self
                .gpus
                .iter()
                .map(|g| format!("{} {}", g.vendor, g.model))
                .collect(),
            memory_total_bytes: self.memory.total_bytes,
            memory_available_bytes: self.memory.available_bytes,
            root_total_bytes: root.map(|d| d.total_bytes).unwrap_or(0),
            root_available_bytes: root.map(|d| d.available_bytes).unwrap_or(0),
            root_filesystem: root.map(|d| d.filesystem.clone()).unwrap_or_default(),
            session: self.session.kind,
            desktop: self.session.desktop.clone(),
            pacman_version: self.pacman.pacman_version.clone(),
            package_backend_ready: matches!(self.pacman.backend, BackendStatus::Ready { .. }),
        }
    }
}
