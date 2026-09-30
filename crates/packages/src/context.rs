//! Classification context: running kernel package and session type.

use cachyos_center_core::bridge::ClassifyContext;

/// Reads the package base of the running kernel from
/// `/usr/lib/modules/$(uname -r)/pkgbase`.
pub fn running_kernel_pkgbase() -> Option<String> {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").ok()?;
    let release = release.trim();
    if release.is_empty() || release.contains('/') || release.contains("..") {
        return None;
    }
    std::fs::read_to_string(format!("/usr/lib/modules/{release}/pkgbase"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| cachyos_center_core::validate::package_name(s).is_ok())
}

/// `true` inside a Hyprland session.
pub fn session_is_hyprland() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
        || std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.split(':').any(|p| p.eq_ignore_ascii_case("hyprland")))
            .unwrap_or(false)
}

/// Context of the current process.
pub fn current() -> ClassifyContext {
    ClassifyContext {
        running_kernel_pkg: running_kernel_pkgbase(),
        session_is_hyprland: session_is_hyprland(),
    }
}
