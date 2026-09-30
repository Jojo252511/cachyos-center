//! Detection of other update mechanisms and of the `pacman-offline` setup.
//!
//! cachyos-center never disables foreign units. It only reports them so that
//! it does not start a competing preparation.

use std::path::Path;

use cachyos_center_core::policy::{ExternalUpdater, OfflineUpdateStatus};

use crate::units::{self, Scope};

/// Known update mechanisms (unit, scope, description).
pub const KNOWN_UPDATERS: [(&str, Scope, &str); 5] = [
    (
        "pacman-offline-prepare.timer",
        Scope::System,
        "pacman-offline: scheduled preparation of offline updates",
    ),
    (
        "pacman-offline-clean-prepare.timer",
        Scope::System,
        "pacman-offline: cache cleaning and preparation",
    ),
    (
        "pacman-offline-reboot.timer",
        Scope::System,
        "pacman-offline: automatic reboot for pending updates",
    ),
    (
        "arch-update.timer",
        Scope::User,
        "Cachy-Update/Arch-Update: update checks and notifications",
    ),
    (
        "packagekit-offline-update.service",
        Scope::System,
        "PackageKit: offline updates (e.g. Discover, GNOME Software)",
    ),
];

pub const PACMAN_OFFLINE_BIN: &str = "/usr/bin/pacman-offline";
pub const OFFLINE_CONF: &str = "/etc/pacman.d/offline.conf";
pub const SYSTEM_UPDATE_LINK: &str = "/system-update";
pub const PACMAN_CACHE: &str = "/var/cache/pacman/pkg";

/// Stable identifiers of reasons that block `PrepareForNextReboot`.
pub mod blocker {
    pub const HELPER_MISSING: &str = "helperMissing";
    pub const EXPERIMENTAL_LOCKED: &str = "experimentalLocked";
    pub const PACMAN_OFFLINE_MISSING: &str = "pacmanOfflineMissing";
    pub const OFFLINE_CONFIG_UNVERIFIABLE: &str = "offlineConfigUnverifiable";
    pub const EXTERNAL_PREPARE_TIMER: &str = "externalPrepareTimer";
    pub const OFFLINE_CONF_HOLDS_PACKAGES: &str = "offlineConfHoldsPackages";
}

/// `PrepareForNextReboot` is still in development (concept: "Bis dahin in
/// Builds sichtbar als in Entwicklung") and has to be unlocked by the
/// administrator in `/etc/cachyos-center/experimental.toml` on test systems.
pub fn experimental_offline_enabled(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| toml_bool(&t, "offline_auto_update"))
        .unwrap_or(false)
}

/// Minimal parser for `key = true|false` lines (the file is root-owned and tiny).
fn toml_bool(text: &str, key: &str) -> Option<bool> {
    let mut value = None;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            value = match v.trim() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            };
        }
    }
    value
}

/// Reasons why `PrepareForNextReboot` cannot be activated (ids from [`blocker`]).
pub fn prepare_blockers(
    helper: bool,
    experimental: bool,
    offline: &OfflineUpdateStatus,
) -> Vec<String> {
    let mut out = Vec::new();
    if !helper {
        out.push(blocker::HELPER_MISSING.to_string());
    }
    if !experimental {
        out.push(blocker::EXPERIMENTAL_LOCKED.to_string());
    }
    if !offline.installed {
        out.push(blocker::PACMAN_OFFLINE_MISSING.to_string());
    }
    if !offline.configuration_verifiable {
        out.push(blocker::OFFLINE_CONFIG_UNVERIFIABLE.to_string());
    }
    if offline.prepare_timer_active {
        out.push(blocker::EXTERNAL_PREPARE_TIMER.to_string());
    }
    if offline.offline_conf_included {
        // The combination of IgnorePkg for kernels (offline.conf) and manual
        // package actions has not been verified in a VM yet (concept 5.6):
        // the automatic mode stays disabled for this configuration.
        out.push(blocker::OFFLINE_CONF_HOLDS_PACKAGES.to_string());
    }
    out
}

/// Installed update mechanisms other than cachyos-center.
pub fn external_updaters() -> Vec<ExternalUpdater> {
    KNOWN_UPDATERS
        .iter()
        .filter_map(|(unit, scope, description)| {
            let state = units::show(*scope, unit)?;
            state.exists().then(|| ExternalUpdater {
                name: (*unit).to_string(),
                scope: match scope {
                    Scope::System => "system".into(),
                    Scope::User => "user".into(),
                },
                active: state.enabled() || state.active(),
                description: (*description).to_string(),
            })
        })
        .collect()
}

/// `true` when `pacman.conf` contains an active `Include` of `target`.
pub fn includes(pacman_conf: &str, target: &str) -> bool {
    pacman_conf.lines().any(|line| {
        let line = line.trim();
        if line.starts_with('#') {
            return false;
        }
        match line.split_once('=') {
            Some((key, value)) => key.trim() == "Include" && value.trim() == target,
            None => false,
        }
    })
}

/// Values of all `IgnorePkg` lines.
pub fn ignore_pkgs(conf: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in conf.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == "IgnorePkg"
        {
            out.extend(value.split_whitespace().map(str::to_string));
        }
    }
    out
}

/// Evaluates the offline update setup from explicit sources (testable).
pub fn offline_status_from(
    installed: bool,
    system_update_target: Option<&Path>,
    pacman_conf: Option<&str>,
    offline_conf: Option<&str>,
    prepare_timer_active: bool,
    reboot_timer_active: bool,
) -> OfflineUpdateStatus {
    let included = pacman_conf.is_some_and(|c| includes(c, OFFLINE_CONF));
    OfflineUpdateStatus {
        installed,
        prepared: system_update_target == Some(Path::new(PACMAN_CACHE)),
        prepare_timer_active,
        reboot_timer_active,
        offline_conf_included: included,
        offline_conf_ignored: if included {
            offline_conf.map(ignore_pkgs).unwrap_or_default()
        } else {
            Vec::new()
        },
        configuration_verifiable: pacman_conf.is_some() && (!included || offline_conf.is_some()),
    }
}

/// Current offline update status of the system.
pub fn offline_status() -> OfflineUpdateStatus {
    let target = std::fs::read_link(SYSTEM_UPDATE_LINK).ok();
    let pacman_conf = std::fs::read_to_string("/etc/pacman.conf").ok();
    let offline_conf = std::fs::read_to_string(OFFLINE_CONF).ok();
    let active =
        |unit: &str| units::show(Scope::System, unit).is_some_and(|s| s.enabled() || s.active());
    offline_status_from(
        Path::new(PACMAN_OFFLINE_BIN).exists(),
        target.as_deref(),
        pacman_conf.as_deref(),
        offline_conf.as_deref(),
        active("pacman-offline-prepare.timer") || active("pacman-offline-clean-prepare.timer"),
        active("pacman-offline-reboot.timer"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn include_detection() {
        let conf = "[options]\n#Include = /etc/pacman.d/offline.conf\nHoldPkg = pacman\n";
        assert!(!includes(conf, OFFLINE_CONF));
        let conf = "[options]\nInclude = /etc/pacman.d/offline.conf\n";
        assert!(includes(conf, OFFLINE_CONF));
        let conf = "[options]\n  Include=/etc/pacman.d/offline.conf  \n";
        assert!(includes(conf, OFFLINE_CONF));
    }

    #[test]
    fn ignore_lines() {
        let conf = "# comment\nIgnorePkg = linux linux-headers\nIgnorePkg = linux-cachyos\n#IgnorePkg = x\n";
        assert_eq!(
            ignore_pkgs(conf),
            vec!["linux", "linux-headers", "linux-cachyos"]
        );
    }

    #[test]
    fn prepared_update_and_held_back_kernels() {
        let s = offline_status_from(
            true,
            Some(Path::new("/var/cache/pacman/pkg")),
            Some("Include = /etc/pacman.d/offline.conf\n"),
            Some("IgnorePkg = linux-cachyos linux-cachyos-headers\n"),
            true,
            false,
        );
        assert!(s.prepared && s.offline_conf_included && s.configuration_verifiable);
        assert_eq!(
            s.offline_conf_ignored,
            vec!["linux-cachyos", "linux-cachyos-headers"]
        );
    }

    #[test]
    fn unverifiable_configuration() {
        let s = offline_status_from(
            true,
            None,
            Some("Include = /etc/pacman.d/offline.conf\n"),
            None,
            false,
            false,
        );
        assert!(!s.configuration_verifiable);
        assert!(!s.prepared);
        let other = offline_status_from(
            false,
            Some(Path::new("/other")),
            Some(""),
            None,
            false,
            false,
        );
        assert!(!other.prepared, "a foreign system-update is not ours");
    }

    #[test]
    fn experimental_switch() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("experimental.toml");
        assert!(!experimental_offline_enabled(&p));
        std::fs::write(&p, "# test system\noffline_auto_update = true\n").unwrap();
        assert!(experimental_offline_enabled(&p));
        std::fs::write(&p, "offline_auto_update = \"yes\"\n").unwrap();
        assert!(!experimental_offline_enabled(&p));
    }

    #[test]
    fn blockers() {
        let ok = OfflineUpdateStatus {
            installed: true,
            configuration_verifiable: true,
            ..OfflineUpdateStatus::default()
        };
        assert!(prepare_blockers(true, true, &ok).is_empty());
        let b = prepare_blockers(false, false, &OfflineUpdateStatus::default());
        assert!(b.contains(&blocker::HELPER_MISSING.to_string()));
        assert!(b.contains(&blocker::EXPERIMENTAL_LOCKED.to_string()));
        assert!(b.contains(&blocker::PACMAN_OFFLINE_MISSING.to_string()));
        let external = OfflineUpdateStatus {
            prepare_timer_active: true,
            offline_conf_included: true,
            ..ok
        };
        assert_eq!(
            prepare_blockers(true, true, &external),
            vec![
                blocker::EXTERNAL_PREPARE_TIMER,
                blocker::OFFLINE_CONF_HOLDS_PACKAGES
            ]
        );
    }
}
