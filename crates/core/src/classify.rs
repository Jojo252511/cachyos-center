//! Classification of packages: update flags and system critical packages.
//!
//! The rules are name based and deliberately conservative. They are used for
//! badges and warnings only; they never decide whether an update is installed.

use crate::updates::UpdateFlag;

const KERNEL_EXCLUDED_SUFFIXES: [&str; 5] =
    ["-headers", "-docs", "-api-headers", "-firmware", "-tools"];

/// `true` for kernel image packages (`linux`, `linux-cachyos`, `linux-zen`, ...).
pub fn is_kernel(name: &str) -> bool {
    if name == "linux" {
        return true;
    }
    let Some(rest) = name.strip_prefix("linux-") else {
        return false;
    };
    if name.starts_with("linux-firmware") || name.starts_with("linux-api-headers") {
        return false;
    }
    if KERNEL_EXCLUDED_SUFFIXES.iter().any(|s| name.ends_with(s)) {
        return false;
    }
    // Kernel module packages built per kernel (e.g. linux-cachyos-nvidia-open).
    if is_kernel_module_package(name) {
        return false;
    }
    !rest.is_empty()
        && rest
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn is_kernel_module_package(name: &str) -> bool {
    const MODULE_MARKERS: [&str; 6] = [
        "-nvidia",
        "-zfs",
        "-v4l2loopback",
        "-virtualbox",
        "-r8125",
        "-dkms",
    ];
    name.starts_with("linux-") && MODULE_MARKERS.iter().any(|m| name.contains(m))
}

/// `true` for GPU drivers, kernel modules and the graphics stack.
pub fn is_driver(name: &str) -> bool {
    is_kernel_module_package(name)
        || name.starts_with("nvidia")
        || name.starts_with("lib32-nvidia")
        || name.ends_with("-dkms")
        || name == "mesa"
        || name == "lib32-mesa"
        || name.starts_with("vulkan-")
        || name.starts_with("lib32-vulkan-")
        || name.starts_with("xf86-video-")
        || name.starts_with("opencl-")
}

pub fn is_firmware(name: &str) -> bool {
    name.starts_with("linux-firmware") || name == "sof-firmware" || name == "alsa-firmware"
}

pub fn is_microcode(name: &str) -> bool {
    name == "amd-ucode" || name == "intel-ucode"
}

pub fn is_core_system(name: &str) -> bool {
    matches!(
        name,
        "glibc"
            | "lib32-glibc"
            | "systemd"
            | "systemd-libs"
            | "dbus"
            | "dbus-broker"
            | "openssl"
            | "mkinitcpio"
            | "dracut"
    )
}

pub fn is_package_manager(name: &str) -> bool {
    matches!(
        name,
        "pacman"
            | "archlinux-keyring"
            | "cachyos-keyring"
            | "cachyos-mirrorlist"
            | "pacman-mirrorlist"
    )
}

/// Packages of the Hyprland desktop session.
pub fn is_desktop_session(name: &str, session_is_hyprland: bool) -> bool {
    session_is_hyprland
        && (name == "hyprland"
            || name.starts_with("hyprland-")
            || name == "xdg-desktop-portal-hyprland")
}

/// Computes all flags of an update. `running_kernel_pkg` is the package base of
/// the running kernel if known.
pub fn update_flags(
    name: &str,
    running_kernel_pkg: Option<&str>,
    session_is_hyprland: bool,
    held_back: bool,
) -> Vec<UpdateFlag> {
    let mut flags = Vec::new();
    if is_kernel(name) || running_kernel_pkg == Some(name) {
        flags.push(UpdateFlag::Kernel);
    }
    if is_driver(name) {
        flags.push(UpdateFlag::Driver);
    }
    if is_firmware(name) {
        flags.push(UpdateFlag::Firmware);
    }
    if is_microcode(name) {
        flags.push(UpdateFlag::Microcode);
    }
    if is_core_system(name) {
        flags.push(UpdateFlag::CoreSystem);
    }
    if is_package_manager(name) {
        flags.push(UpdateFlag::PackageManager);
    }
    if is_desktop_session(name, session_is_hyprland) {
        flags.push(UpdateFlag::DesktopSession);
    }
    if held_back {
        flags.push(UpdateFlag::HeldBack);
    }
    if flags.iter().any(|f| {
        matches!(
            f,
            UpdateFlag::Kernel
                | UpdateFlag::Driver
                | UpdateFlag::Firmware
                | UpdateFlag::Microcode
                | UpdateFlag::CoreSystem
        )
    }) {
        flags.push(UpdateFlag::RebootRecommended);
    }
    flags
}

/// Packages whose removal gets an additional warning.
const CRITICAL: [&str; 24] = [
    "base",
    "filesystem",
    "glibc",
    "systemd",
    "systemd-libs",
    "systemd-sysvcompat",
    "pacman",
    "bash",
    "coreutils",
    "util-linux",
    "shadow",
    "sudo",
    "linux-firmware",
    "mkinitcpio",
    "dracut",
    "grub",
    "limine",
    "refind",
    "efibootmgr",
    "archlinux-keyring",
    "cachyos-keyring",
    "cachyos-mirrorlist",
    "networkmanager",
    "dbus",
];

/// `true` when removing `name` may render the system unusable.
///
/// `hold_pkgs` are the `HoldPkg` entries of `pacman.conf`; the running kernel
/// and the active desktop session count as critical as well.
pub fn is_critical(
    name: &str,
    hold_pkgs: &[String],
    running_kernel_pkg: Option<&str>,
    session_is_hyprland: bool,
) -> bool {
    CRITICAL.contains(&name)
        || hold_pkgs.iter().any(|h| h == name)
        || is_kernel(name)
        || running_kernel_pkg == Some(name)
        || is_microcode(name)
        || (session_is_hyprland && name == "hyprland")
        || name == crate::APP_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernels() {
        for k in [
            "linux",
            "linux-cachyos",
            "linux-cachyos-lts",
            "linux-zen",
            "linux-lts",
            "linux-cachyos-bore",
        ] {
            assert!(is_kernel(k), "{k}");
        }
        for n in [
            "linux-firmware",
            "linux-firmware-intel",
            "linux-headers",
            "linux-cachyos-headers",
            "linux-api-headers",
            "linux-cachyos-nvidia-open",
            "linux-tools",
            "linuxconsole",
            "util-linux",
        ] {
            assert!(!is_kernel(n), "{n}");
        }
    }

    #[test]
    fn drivers() {
        for d in [
            "nvidia-open-dkms",
            "nvidia-utils",
            "mesa",
            "lib32-mesa",
            "vulkan-radeon",
            "linux-cachyos-nvidia-open",
            "xf86-video-amdgpu",
        ] {
            assert!(is_driver(d), "{d}");
        }
        assert!(!is_driver("firefox"));
    }

    #[test]
    fn flags_and_reboot() {
        let f = update_flags("linux-cachyos", None, true, false);
        assert!(f.contains(&UpdateFlag::Kernel));
        assert!(f.contains(&UpdateFlag::RebootRecommended));
        let f = update_flags("hyprland", None, true, false);
        assert_eq!(f, vec![UpdateFlag::DesktopSession]);
        let f = update_flags("hyprland", None, false, false);
        assert!(f.is_empty());
        let f = update_flags("firefox", None, true, true);
        assert_eq!(f, vec![UpdateFlag::HeldBack]);
        let f = update_flags("my-kernel", Some("my-kernel"), false, false);
        assert!(f.contains(&UpdateFlag::Kernel));
    }

    #[test]
    fn critical_packages() {
        let hold = vec!["pacman".to_string(), "glibc".to_string()];
        assert!(is_critical("glibc", &hold, None, false));
        assert!(is_critical("linux-cachyos", &hold, None, false));
        assert!(is_critical("hyprland", &hold, None, true));
        assert!(!is_critical("hyprland", &hold, None, false));
        assert!(is_critical(
            "custom-kernel",
            &hold,
            Some("custom-kernel"),
            false
        ));
        assert!(!is_critical("firefox", &hold, None, true));
    }
}
