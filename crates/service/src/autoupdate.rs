//! Status of the automatic updates as shown in the settings.

use std::path::Path;

use cachyos_center_core::operation::Operation;
use cachyos_center_core::paths::{EXPERIMENTAL_FILE, LIBEXEC_DIR, POLICY_FILE, TIMER_STATUS_FILE};
use cachyos_center_core::policy::{
    AutoUpdateConfig, AutoUpdateStatus, ExternalUpdater, OfflineUpdateStatus,
};
use cachyos_center_system::units::{self, Scope, UnitState};
use cachyos_center_system::updaters;
use serde::Deserialize;

/// Stable identifiers of reasons that block `PrepareForNextReboot`.
pub mod blocker {
    pub const HELPER_MISSING: &str = "helperMissing";
    pub const EXPERIMENTAL_LOCKED: &str = "experimentalLocked";
    pub const PACMAN_OFFLINE_MISSING: &str = "pacmanOfflineMissing";
    pub const OFFLINE_CONFIG_UNVERIFIABLE: &str = "offlineConfigUnverifiable";
    pub const EXTERNAL_PREPARE_TIMER: &str = "externalPrepareTimer";
    pub const OFFLINE_CONF_HOLDS_PACKAGES: &str = "offlineConfHoldsPackages";
}

#[derive(Debug, Default, Deserialize)]
struct Experimental {
    #[serde(default)]
    offline_auto_update: bool,
}

/// `PrepareForNextReboot` is still in development and has to be unlocked by
/// the administrator in `/etc/cachyos-center/experimental.toml`.
pub fn experimental_offline_enabled(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| toml::from_str::<Experimental>(&t).ok())
        .is_some_and(|e| e.offline_auto_update)
}

/// Reads the policy file. Missing file = default (`Off`).
pub fn read_policy(path: &Path) -> (AutoUpdateConfig, Option<String>) {
    match std::fs::read_to_string(path) {
        Ok(text) => match AutoUpdateConfig::from_toml(&text) {
            Ok(c) => (c, None),
            Err(e) => (AutoUpdateConfig::default(), Some(e.message)),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (AutoUpdateConfig::default(), None),
        Err(e) => (AutoUpdateConfig::default(), Some(e.to_string())),
    }
}

/// `true` when the helper is installed (binary and D-Bus activation file).
pub fn helper_installed() -> bool {
    Path::new(LIBEXEC_DIR)
        .join("cachyos-center-helper")
        .exists()
        && Path::new("/usr/share/dbus-1/system-services/org.cachyos_center.Packages1.service")
            .exists()
}

/// Reasons why `PrepareForNextReboot` cannot be activated.
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
        // The combination of IgnorePkg for kernels and manual package actions
        // has not been verified in a VM yet (concept 5.6): keep it disabled.
        out.push(blocker::OFFLINE_CONF_HOLDS_PACKAGES.to_string());
    }
    out
}

/// Assembles the status from explicit inputs (testable).
#[allow(clippy::too_many_arguments)]
pub fn status_from(
    config: AutoUpdateConfig,
    config_error: Option<String>,
    timer: Option<UnitState>,
    last_result: Option<Operation>,
    offline: OfflineUpdateStatus,
    external: Vec<ExternalUpdater>,
    experimental: bool,
    helper: bool,
) -> AutoUpdateStatus {
    let blockers = prepare_blockers(helper, experimental, &offline);
    AutoUpdateStatus {
        config,
        config_error,
        timer_enabled: timer.as_ref().is_some_and(|t| t.enabled()),
        next_run: timer.as_ref().and_then(|t| t.next_elapse),
        last_run: timer.as_ref().and_then(|t| t.last_trigger),
        last_result,
        prepared_for_next_reboot: offline.prepared,
        prepare_mode_available: experimental,
        prepare_mode_blockers: blockers,
        offline,
        external_updaters: external,
        helper_available: helper,
    }
}

/// Current status of the system.
pub fn status() -> AutoUpdateStatus {
    let (config, error) = read_policy(Path::new(POLICY_FILE));
    let last = std::fs::read_to_string(TIMER_STATUS_FILE)
        .ok()
        .and_then(|t| serde_json::from_str::<Operation>(&t).ok());
    status_from(
        config,
        error,
        units::show(Scope::System, cachyos_center_core::dbus::PREFLIGHT_TIMER),
        last,
        updaters::offline_status(),
        updaters::external_updaters(),
        experimental_offline_enabled(Path::new(EXPERIMENTAL_FILE)),
        helper_installed(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experimental_switch() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("experimental.toml");
        assert!(!experimental_offline_enabled(&p));
        std::fs::write(&p, "offline_auto_update = true\n").unwrap();
        assert!(experimental_offline_enabled(&p));
        std::fs::write(&p, "offline_auto_update = \"yes\"\n").unwrap();
        assert!(!experimental_offline_enabled(&p));
    }

    #[test]
    fn missing_policy_is_off() {
        let (c, e) = read_policy(Path::new("/nonexistent/auto-update.toml"));
        assert_eq!(c, AutoUpdateConfig::default());
        assert!(e.is_none());
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
        let b = prepare_blockers(true, true, &external);
        assert_eq!(
            b,
            vec![
                blocker::EXTERNAL_PREPARE_TIMER,
                blocker::OFFLINE_CONF_HOLDS_PACKAGES
            ]
        );
    }
}
