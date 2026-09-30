//! Operating system, kernel and session.

use std::collections::HashMap;
use std::path::Path;

use cachyos_center_core::system::{KernelInfo, OsInfo, SessionInfo, SessionKind};

/// Parses `os-release` content (`KEY=value`, optionally quoted).
pub fn parse_os_release(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (k, v) = line.split_once('=')?;
            let v = v.trim();
            let v = v
                .strip_prefix('"')
                .and_then(|x| x.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')))
                .unwrap_or(v);
            Some((k.trim().to_string(), v.replace("\\\"", "\"")))
        })
        .collect()
}

pub fn os_info_from(text: &str) -> OsInfo {
    let map = parse_os_release(text);
    let id = map.get("ID").cloned().unwrap_or_else(|| "linux".into());
    let like = map.get("ID_LIKE").cloned().unwrap_or_default();
    let name = map.get("NAME").cloned().unwrap_or_else(|| "Linux".into());
    OsInfo {
        pretty_name: map
            .get("PRETTY_NAME")
            .cloned()
            .unwrap_or_else(|| name.clone()),
        name,
        build_id: map.get("BUILD_ID").cloned(),
        is_cachyos: id == "cachyos",
        is_arch_based: id == "arch"
            || id == "cachyos"
            || like.split_whitespace().any(|l| l == "arch"),
        id,
    }
}

pub fn os_info() -> OsInfo {
    let text = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();
    os_info_from(&text)
}

pub fn kernel_info() -> KernelInfo {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let uptime = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(str::to_string))
        .and_then(|s| s.parse::<f64>().ok())
        .map(|s| s.max(0.0) as u64)
        .unwrap_or(0);
    let safe = !release.contains('/') && !release.contains("..") && release != "unknown";
    let modules = Path::new("/usr/lib/modules").join(&release);
    KernelInfo {
        package: if safe {
            std::fs::read_to_string(modules.join("pkgbase"))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        } else {
            None
        },
        modules_missing: safe && !modules.exists(),
        release,
        uptime_seconds: uptime,
    }
}

/// Session information from an environment lookup function.
pub fn session_from(env: impl Fn(&str) -> Option<String>) -> SessionInfo {
    let desktop = env("XDG_CURRENT_DESKTOP").filter(|s| !s.is_empty());
    let session_type = env("XDG_SESSION_TYPE").filter(|s| !s.is_empty());
    let hyprland = env("HYPRLAND_INSTANCE_SIGNATURE").is_some()
        || desktop
            .as_deref()
            .is_some_and(|d| d.split(':').any(|p| p.eq_ignore_ascii_case("hyprland")));
    let kind = if hyprland {
        SessionKind::Hyprland
    } else {
        match session_type.as_deref() {
            Some("wayland") => SessionKind::OtherWayland,
            Some("x11") => SessionKind::X11,
            Some("tty") => SessionKind::Tty,
            _ if env("WAYLAND_DISPLAY").is_some() => SessionKind::OtherWayland,
            _ if env("DISPLAY").is_some() => SessionKind::X11,
            _ => SessionKind::Unknown,
        }
    };
    SessionInfo {
        kind,
        desktop,
        session_type,
    }
}

pub fn session_info() -> SessionInfo {
    session_from(|k| std::env::var(k).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cachyos_release() {
        let info = os_info_from(
            "NAME=\"CachyOS Linux\"\nPRETTY_NAME=\"CachyOS\"\nID=cachyos\nID_LIKE=arch\nBUILD_ID=rolling\n",
        );
        assert!(info.is_cachyos);
        assert!(info.is_arch_based);
        assert_eq!(info.pretty_name, "CachyOS");
        assert_eq!(info.build_id.as_deref(), Some("rolling"));
    }

    #[test]
    fn other_distributions() {
        let arch = os_info_from("NAME=\"Arch Linux\"\nID=arch\n");
        assert!(arch.is_arch_based && !arch.is_cachyos);
        let endeavour = os_info_from("ID=endeavouros\nID_LIKE=\"arch\"\n");
        assert!(endeavour.is_arch_based);
        let fedora = os_info_from("ID=fedora\n");
        assert!(!fedora.is_arch_based);
        assert_eq!(os_info_from("").id, "linux");
    }

    #[test]
    fn sessions() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert_eq!(
            session_from(env(&[
                ("HYPRLAND_INSTANCE_SIGNATURE", "x"),
                ("XDG_SESSION_TYPE", "wayland")
            ]))
            .kind,
            SessionKind::Hyprland
        );
        assert_eq!(
            session_from(env(&[
                ("XDG_SESSION_TYPE", "wayland"),
                ("XDG_CURRENT_DESKTOP", "KDE")
            ]))
            .kind,
            SessionKind::OtherWayland
        );
        assert_eq!(
            session_from(env(&[("DISPLAY", ":0")])).kind,
            SessionKind::X11
        );
        assert_eq!(session_from(env(&[])).kind, SessionKind::Unknown);
    }

    #[test]
    fn kernel_of_this_machine() {
        let k = kernel_info();
        assert_ne!(k.release, "");
    }
}
