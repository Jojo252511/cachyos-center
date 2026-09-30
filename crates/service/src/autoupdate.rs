//! Status of the automatic updates as shown in the settings.

use std::path::Path;

use cachyos_center_core::operation::Operation;
use cachyos_center_core::paths::{EXPERIMENTAL_FILE, LIBEXEC_DIR, POLICY_FILE, TIMER_STATUS_FILE};
use cachyos_center_core::policy::{
    AutoUpdateConfig, AutoUpdateStatus, ExternalUpdater, OfflineUpdateStatus,
};
use cachyos_center_system::units::{self, Scope, UnitState};
use cachyos_center_system::updaters;

pub use cachyos_center_system::updaters::{
    blocker, experimental_offline_enabled, prepare_blockers,
};

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
    fn missing_policy_is_off() {
        let (c, e) = read_policy(Path::new("/nonexistent/auto-update.toml"));
        assert_eq!(c, AutoUpdateConfig::default());
        assert!(e.is_none());
    }
}
