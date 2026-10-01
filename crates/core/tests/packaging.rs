//! The names used in the code must match the packaging files
//! (D-Bus name, polkit action ids, unit names, installation paths).

use std::path::PathBuf;

use cachyos_center_core::{APP_ID, dbus, mcp, paths};

fn packaging(file: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging/arch")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn dbus_activation_and_policy() {
    let service = packaging("org.cachyos_center.Packages1.service");
    assert!(service.contains(&format!("Name={}", dbus::BUS_NAME)));
    assert!(service.contains(&format!("SystemdService={}", dbus::HELPER_UNIT)));
    assert!(service.contains(&format!(
        "Exec={}/cachyos-center-helper daemon",
        paths::LIBEXEC_DIR
    )));
    let conf = packaging("org.cachyos_center.Packages1.conf");
    assert!(conf.contains(&format!("<allow own=\"{}\"/>", dbus::BUS_NAME)));
    assert!(conf.contains(&format!("send_interface=\"{}\"", dbus::INTERFACE)));
    let unit = packaging(&format!("systemd/{}", dbus::HELPER_UNIT));
    assert!(unit.contains(&format!("BusName={}", dbus::BUS_NAME)));
    assert!(unit.contains("Type=dbus"));
}

#[test]
fn polkit_actions_exist_without_blanket_rules() {
    let policy = packaging("org.cachyos-center.policy");
    for action in dbus::actions::ALL {
        assert!(
            policy.contains(&format!("<action id=\"{action}\">")),
            "{action}"
        );
    }
    assert_eq!(
        policy.matches("<action id=").count(),
        dbus::actions::ALL.len()
    );
    // Only cancelling one's own operation (which never changes the system) is
    // granted to active local sessions without authentication.
    for block in policy.split("<action id=").skip(1) {
        if block.contains("<allow_active>yes</allow_active>") {
            assert!(
                block.starts_with(&format!("\"{}\"", dbus::actions::CANCEL)),
                "only the cancel action may be granted without authentication"
            );
        }
        assert!(
            block.contains("<allow_inactive>no</allow_inactive>"),
            "{block}"
        );
    }
    assert!(!policy.contains("<allow_any>yes"));
}

#[test]
fn timer_and_notification_units() {
    let timer = packaging(&format!("systemd/{}", dbus::PREFLIGHT_TIMER));
    assert!(timer.contains("Persistent=true"));
    let service = packaging(&format!("systemd/{}", dbus::PREFLIGHT_SERVICE));
    assert!(service.contains("cachyos-center-helper preflight"));
    assert!(service.contains("ConditionPathExists=!/var/lib/pacman/db.lck"));
    let path = packaging(&format!("systemd/user/{}", dbus::NOTIFY_PATH_UNIT));
    assert!(path.contains(paths::TIMER_STATUS_FILE));
    let notify = packaging("systemd/user/cachyos-center-notify.service");
    assert!(notify.contains("--notify-timer-status"));
    // No automatic reboot timer is ever shipped.
    assert!(!timer.contains("reboot") && !service.contains("reboot"));
}

#[test]
fn desktop_entry_matches_the_window_class() {
    let desktop = packaging(&format!("{APP_ID}.desktop"));
    assert!(desktop.contains(&format!("StartupWMClass={APP_ID}")));
    assert!(desktop.contains(&format!("Icon={APP_ID}")));
}

#[test]
fn pkgbuild_installs_the_expected_paths() {
    let pkgbuild = packaging("PKGBUILD");
    for needle in [
        format!("$pkgdir/usr/lib/cachyos-center/{}", paths::ALPM_BRIDGE_FILE),
        "$pkgdir/usr/lib/cachyos-center/cachyos-center-helper".to_string(),
        format!("$pkgdir/usr/bin/{}", mcp::BINARY_NAME),
        "$pkgdir/usr/share/polkit-1/actions/org.cachyos-center.policy".to_string(),
        format!(
            "$pkgdir/usr/share/dbus-1/system-services/{}.service",
            dbus::BUS_NAME
        ),
        format!("$pkgdir/usr/share/applications/{APP_ID}.desktop"),
    ] {
        assert!(pkgbuild.contains(&needle), "{needle}");
    }
    assert_eq!(paths::LIBEXEC_DIR, "/usr/lib/cachyos-center");
    let tmpfiles = packaging("cachyos-center.tmpfiles");
    for dir in [
        paths::SYSTEM_CONFIG_DIR,
        paths::SYSTEM_STATE_DIR,
        paths::SYSTEM_OPERATIONS_DIR,
        paths::SYSTEM_LOG_DIR,
    ] {
        assert!(tmpfiles.contains(dir), "{dir}");
    }
}
