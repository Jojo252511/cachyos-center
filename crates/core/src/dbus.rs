//! Names of the privileged helper interface.
//!
//! These constants must match `packaging/arch/` (D-Bus service and policy
//! files, polkit actions, systemd units). `packaging` tests verify this.

/// Well-known bus name on the system bus.
pub const BUS_NAME: &str = "org.cachyos_center.Packages1";
/// Object path of the helper.
pub const OBJECT_PATH: &str = "/org/cachyos_center/Packages1";
/// Versioned interface name.
pub const INTERFACE: &str = "org.cachyos_center.Packages1";
/// Protocol version reported by the `Version` property.
pub const PROTOCOL_VERSION: u32 = 1;

/// systemd unit of the D-Bus activated helper.
pub const HELPER_UNIT: &str = "cachyos-center-helper.service";
/// systemd timer of the scheduled preflight.
pub const PREFLIGHT_TIMER: &str = "cachyos-center-preflight.timer";
/// systemd service started by the preflight timer.
pub const PREFLIGHT_SERVICE: &str = "cachyos-center-preflight.service";
/// User unit that turns helper status changes into desktop notifications.
pub const NOTIFY_PATH_UNIT: &str = "cachyos-center-notify.path";

/// polkit action ids. polkit only allows `[a-z0-9.-]`, therefore the
/// reverse-DNS form with a hyphen is used here.
pub mod actions {
    pub const UPGRADE: &str = "org.cachyos-center.packages.upgrade";
    pub const INSTALL: &str = "org.cachyos-center.packages.install";
    pub const REMOVE: &str = "org.cachyos-center.packages.remove";
    pub const CONFIGURE_AUTO_UPDATE: &str = "org.cachyos-center.autoupdate.configure";

    pub const ALL: [&str; 4] = [UPGRADE, INSTALL, REMOVE, CONFIGURE_AUTO_UPDATE];
}

/// Upper bound for method arguments that carry lists or strings.
pub const MAX_ARGUMENT_LEN: usize = 256;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polkit_action_ids_are_valid() {
        for id in actions::ALL {
            assert!(
                id.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'.'
                    || b == b'-'),
                "{id} contains characters polkit does not accept"
            );
        }
    }

    #[test]
    fn dbus_names_have_no_hyphen() {
        for name in [BUS_NAME, INTERFACE] {
            assert!(!name.contains('-'));
            assert!(
                name.split('.')
                    .all(|part| !part.is_empty() && !part.as_bytes()[0].is_ascii_digit())
            );
        }
        assert!(OBJECT_PATH.starts_with('/'));
        assert!(
            OBJECT_PATH
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'/')
        );
    }
}
