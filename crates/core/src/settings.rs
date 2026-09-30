//! User preferences (`$XDG_CONFIG_HOME/cachyos-center/settings.toml`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ThemePreference {
    /// Follow the desktop color scheme; dark when unknown.
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LanguagePreference {
    /// Follow `LANG`/`LC_MESSAGES`; German when unknown.
    #[default]
    System,
    De,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

/// Allowed values for the background check interval in hours (0 = off).
pub const CHECK_INTERVAL_CHOICES: [u32; 6] = [0, 1, 3, 6, 12, 24];
/// Allowed values for the history/log retention in days.
pub const LOG_RETENTION_CHOICES: [u32; 5] = [7, 30, 90, 180, 365];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    pub theme: ThemePreference,
    pub language: LanguagePreference,
    pub density: Density,
    /// Desktop notifications for updates, results, errors and `NeedsAttention`.
    pub notifications: bool,
    /// Background update check while the app is running (hours, 0 = off).
    pub check_interval_hours: u32,
    /// Retention of the local history and operation logs.
    pub log_retention_days: u32,
    /// Fetch Arch Linux and CachyOS news (network access to archlinux.org/cachyos.org).
    pub news_enabled: bool,
    /// The local read-only MCP server answers requests (off by default).
    pub mcp_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            language: LanguagePreference::System,
            density: Density::Comfortable,
            notifications: true,
            check_interval_hours: 6,
            log_retention_days: 90,
            news_enabled: true,
            mcp_enabled: false,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> AppResult<()> {
        if !CHECK_INTERVAL_CHOICES.contains(&self.check_interval_hours) {
            return Err(AppError::invalid("unsupported check interval"));
        }
        if !LOG_RETENTION_CHOICES.contains(&self.log_retention_days) {
            return Err(AppError::invalid("unsupported log retention"));
        }
        Ok(())
    }

    /// Parses the settings file. Unknown keys are ignored; invalid values fall
    /// back to the defaults so that a damaged file never blocks the app.
    pub fn from_toml(text: &str) -> Self {
        match toml::from_str::<Settings>(text) {
            Ok(s) if s.validate().is_ok() => s,
            _ => Settings::default(),
        }
    }

    pub fn to_toml(&self) -> AppResult<String> {
        toml::to_string(self)
            .map_err(|e| AppError::internal(format!("cannot serialize settings: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe() {
        let s = Settings::default();
        assert!(!s.mcp_enabled, "MCP must be disabled by default");
        s.validate().unwrap();
    }

    #[test]
    fn roundtrip() {
        let s = Settings {
            theme: ThemePreference::Light,
            language: LanguagePreference::En,
            density: Density::Compact,
            notifications: false,
            check_interval_hours: 24,
            log_retention_days: 30,
            news_enabled: false,
            mcp_enabled: true,
        };
        assert_eq!(Settings::from_toml(&s.to_toml().unwrap()), s);
    }

    #[test]
    fn damaged_file_falls_back_to_defaults() {
        assert_eq!(Settings::from_toml("not toml ["), Settings::default());
        assert_eq!(
            Settings::from_toml("checkIntervalHours = 5"),
            Settings::default()
        );
        let partial = Settings::from_toml("mcpEnabled = true");
        assert!(partial.mcp_enabled);
        assert_eq!(partial.check_interval_hours, 6);
    }
}
