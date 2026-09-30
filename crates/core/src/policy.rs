//! Automatic update policy (`/etc/cachyos-center/auto-update.toml`).
//!
//! The policy file is written exclusively by the privileged helper and read by
//! everyone. The default is [`AutoUpdatePolicy::Off`].

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Timestamp;
use crate::error::{AppError, AppResult};
use crate::operation::Operation;
use crate::validate;

/// `Off | NotifyOnly | PrepareForNextReboot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AutoUpdatePolicy {
    #[default]
    Off,
    /// Check on schedule and notify; never installs.
    NotifyOnly,
    /// Check, run the preflight and prepare the update with `pacman-offline`;
    /// installation happens on the next reboot started by the user.
    PrepareForNextReboot,
}

impl AutoUpdatePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::NotifyOnly => "notifyOnly",
            Self::PrepareForNextReboot => "prepareForNextReboot",
        }
    }

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "off" => Ok(Self::Off),
            "notifyOnly" => Ok(Self::NotifyOnly),
            "prepareForNextReboot" => Ok(Self::PrepareForNextReboot),
            _ => Err(AppError::invalid("unknown auto-update policy")),
        }
    }
}

/// Days of the week, Monday first (systemd calendar names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    pub const ALL: [Weekday; 7] = [
        Self::Mon,
        Self::Tue,
        Self::Wed,
        Self::Thu,
        Self::Fri,
        Self::Sat,
        Self::Sun,
    ];

    pub fn systemd_name(self) -> &'static str {
        match self {
            Self::Mon => "Mon",
            Self::Tue => "Tue",
            Self::Wed => "Wed",
            Self::Thu => "Thu",
            Self::Fri => "Fri",
            Self::Sat => "Sat",
            Self::Sun => "Sun",
        }
    }

    /// Bit in a D-Bus weekday mask (Monday = bit 0).
    pub fn bit(self) -> u8 {
        1 << (self as u8)
    }

    pub fn from_mask(mask: u8) -> Vec<Weekday> {
        Self::ALL
            .into_iter()
            .filter(|d| mask & d.bit() != 0)
            .collect()
    }

    pub fn to_mask(days: &[Weekday]) -> u8 {
        days.iter().fold(0, |m, d| m | d.bit())
    }
}

/// Weekdays and time for the scheduled preparation/notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateWindow {
    pub weekdays: Vec<Weekday>,
    /// `HH:MM`, 24 hour clock, local time.
    pub time: String,
}

impl Default for UpdateWindow {
    fn default() -> Self {
        Self {
            weekdays: Weekday::ALL.to_vec(),
            time: "12:00".into(),
        }
    }
}

impl UpdateWindow {
    pub fn validate(&self) -> AppResult<()> {
        if self.weekdays.is_empty() {
            return Err(AppError::invalid("at least one weekday is required"));
        }
        validate::time_of_day(&self.time)?;
        Ok(())
    }

    /// systemd `OnCalendar=` expression, e.g. `Mon,Wed *-*-* 03:30:00`.
    pub fn on_calendar(&self) -> AppResult<String> {
        self.validate()?;
        let minutes = validate::time_of_day(&self.time)?;
        let mut days = self.weekdays.clone();
        days.sort();
        days.dedup();
        let days = days
            .iter()
            .map(|d| d.systemd_name())
            .collect::<Vec<_>>()
            .join(",");
        Ok(format!(
            "{days} *-*-* {:02}:{:02}:00",
            minutes / 60,
            minutes % 60
        ))
    }
}

/// Content of `/etc/cachyos-center/auto-update.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AutoUpdateConfig {
    pub policy: AutoUpdatePolicy,
    pub window: UpdateWindow,
    /// Require a successful snapper snapshot before preparing an update.
    pub require_snapshot: bool,
    /// News published up to this time have been read and acknowledged in the UI.
    #[ts(type = "number | null")]
    pub news_acknowledged_until: Option<Timestamp>,
}

impl Default for AutoUpdateConfig {
    fn default() -> Self {
        Self {
            policy: AutoUpdatePolicy::Off,
            window: UpdateWindow::default(),
            require_snapshot: false,
            news_acknowledged_until: None,
        }
    }
}

impl AutoUpdateConfig {
    pub fn validate(&self) -> AppResult<()> {
        self.window.validate()
    }

    pub fn to_toml(&self) -> AppResult<String> {
        let file = PolicyFile {
            schema: 1,
            policy: self.policy.as_str().to_string(),
            weekdays: self
                .window
                .weekdays
                .iter()
                .map(|d| d.systemd_name().to_string())
                .collect(),
            time: self.window.time.clone(),
            require_snapshot: self.require_snapshot,
            news_acknowledged_until: self.news_acknowledged_until,
        };
        let body = toml::to_string(&file)
            .map_err(|e| AppError::internal(format!("cannot serialize policy: {e}")))?;
        Ok(format!(
            "# Managed by cachyos-center-helper. Do not edit manually.\n{body}"
        ))
    }

    pub fn from_toml(text: &str) -> AppResult<Self> {
        let file: PolicyFile = toml::from_str(text)
            .map_err(|e| AppError::invalid(format!("invalid policy file: {e}")))?;
        if file.schema != 1 {
            return Err(AppError::new(
                crate::ErrorCode::Unsupported,
                "unsupported policy file schema",
            ));
        }
        let weekdays = file
            .weekdays
            .iter()
            .map(|d| {
                Weekday::ALL
                    .into_iter()
                    .find(|w| w.systemd_name() == d)
                    .ok_or_else(|| AppError::invalid("unknown weekday in policy file"))
            })
            .collect::<AppResult<Vec<_>>>()?;
        let config = Self {
            policy: AutoUpdatePolicy::parse(&file.policy)?,
            window: UpdateWindow {
                weekdays,
                time: file.time,
            },
            require_snapshot: file.require_snapshot,
            news_acknowledged_until: file.news_acknowledged_until,
        };
        config.validate()?;
        Ok(config)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyFile {
    schema: u32,
    policy: String,
    weekdays: Vec<String>,
    time: String,
    require_snapshot: bool,
    news_acknowledged_until: Option<i64>,
}

/// Status of the external `pacman-offline` integration.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OfflineUpdateStatus {
    /// `/usr/bin/pacman-offline` exists.
    pub installed: bool,
    /// `/system-update` points to the pacman cache: an update is prepared for the next reboot.
    pub prepared: bool,
    /// `pacman-offline-prepare.timer` is enabled or active (externally managed).
    pub prepare_timer_active: bool,
    /// `pacman-offline-reboot.timer` is enabled or active.
    pub reboot_timer_active: bool,
    /// `Include = /etc/pacman.d/offline.conf` is active in `/etc/pacman.conf`.
    pub offline_conf_included: bool,
    /// Packages held back for online updates by `offline.conf`.
    pub offline_conf_ignored: Vec<String>,
    /// The configuration could be inspected completely.
    pub configuration_verifiable: bool,
}

/// Other update mechanisms found on the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExternalUpdater {
    /// Unit or program name, e.g. `arch-update.timer`.
    pub name: String,
    /// `system` or `user` unit.
    pub scope: String,
    pub active: bool,
    pub description: String,
}

/// Everything the settings page needs to show about automatic updates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AutoUpdateStatus {
    pub config: AutoUpdateConfig,
    /// The policy file could not be read (defaults are shown).
    pub config_error: Option<String>,
    /// The app timer `cachyos-center-preflight.timer` is enabled.
    pub timer_enabled: bool,
    #[ts(type = "number | null")]
    pub next_run: Option<Timestamp>,
    #[ts(type = "number | null")]
    pub last_run: Option<Timestamp>,
    /// Result of the last timer run (written by the helper).
    pub last_result: Option<Operation>,
    /// The update is prepared and will be installed on the next reboot.
    pub prepared_for_next_reboot: bool,
    pub offline: OfflineUpdateStatus,
    pub external_updaters: Vec<ExternalUpdater>,
    /// `PrepareForNextReboot` is still in development and must be unlocked by the
    /// administrator (`/etc/cachyos-center/experimental.toml`).
    pub prepare_mode_available: bool,
    /// Reasons why `PrepareForNextReboot` cannot be activated right now.
    pub prepare_mode_blockers: Vec<String>,
    /// The privileged helper is installed and reachable.
    pub helper_available: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_calendar_expression() {
        let w = UpdateWindow {
            weekdays: vec![Weekday::Wed, Weekday::Mon, Weekday::Mon],
            time: "03:05".into(),
        };
        assert_eq!(w.on_calendar().unwrap(), "Mon,Wed *-*-* 03:05:00");
        let bad = UpdateWindow {
            weekdays: vec![],
            time: "03:05".into(),
        };
        assert!(bad.on_calendar().is_err());
    }

    #[test]
    fn weekday_mask_roundtrip() {
        let days = vec![Weekday::Mon, Weekday::Fri, Weekday::Sun];
        let mask = Weekday::to_mask(&days);
        assert_eq!(mask, 0b0101_0001);
        assert_eq!(Weekday::from_mask(mask), days);
    }

    #[test]
    fn toml_roundtrip_and_default_off() {
        let default = AutoUpdateConfig::default();
        assert_eq!(default.policy, AutoUpdatePolicy::Off);
        let config = AutoUpdateConfig {
            policy: AutoUpdatePolicy::NotifyOnly,
            window: UpdateWindow {
                weekdays: vec![Weekday::Sat],
                time: "10:30".into(),
            },
            require_snapshot: true,
            news_acknowledged_until: Some(1_700_000_000),
        };
        let text = config.to_toml().unwrap();
        assert!(text.starts_with("# Managed by cachyos-center-helper"));
        assert_eq!(AutoUpdateConfig::from_toml(&text).unwrap(), config);
    }

    #[test]
    fn toml_rejects_unknown_content() {
        assert!(AutoUpdateConfig::from_toml("schema = 1\npolicy = \"always\"\nweekdays = [\"Mon\"]\ntime = \"10:00\"\nrequire_snapshot = false\n").is_err());
        assert!(AutoUpdateConfig::from_toml("schema = 2\npolicy = \"off\"\nweekdays = [\"Mon\"]\ntime = \"10:00\"\nrequire_snapshot = false\n").is_err());
        assert!(AutoUpdateConfig::from_toml("schema = 1\npolicy = \"off\"\nweekdays = [\"Mon\"]\ntime = \"10:00\"\nrequire_snapshot = false\nexec = \"rm\"\n").is_err());
    }
}
