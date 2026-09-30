//! System information for cachyos-center (read-only, no root required).
//!
//! Every function degrades gracefully: missing files lead to "unknown"
//! values instead of errors, so the UI can always render a system page.

pub mod hardware;
pub mod health;
pub mod hyprland;
pub mod news;
pub mod os;
pub mod power;
pub mod units;
pub mod updaters;

use cachyos_center_core::system::{PacmanStatus, SystemInfo};

/// Collects the complete system information. `pacman` is provided by the
/// package service (backend status, lock, counts).
pub fn collect(pacman: PacmanStatus) -> SystemInfo {
    SystemInfo {
        os: os::os_info(),
        kernel: os::kernel_info(),
        cpu: hardware::cpu_info(),
        gpus: hardware::gpus(),
        memory: hardware::memory(),
        disks: hardware::disks(),
        session: os::session_info(),
        pacman,
        collected_at: cachyos_center_core::now(),
    }
}
