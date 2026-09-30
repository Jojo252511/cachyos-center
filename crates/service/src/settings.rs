//! Persistent user settings.

use std::path::{Path, PathBuf};

use cachyos_center_core::AppResult;
use cachyos_center_core::settings::Settings;

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Current settings; defaults when the file is missing or damaged.
    pub fn load(&self) -> Settings {
        std::fs::read_to_string(&self.path)
            .map(|t| Settings::from_toml(&t))
            .unwrap_or_default()
    }

    /// Validates and writes the settings atomically (mode 0600).
    pub fn save(&self, settings: &Settings) -> AppResult<()> {
        settings.validate()?;
        let text = settings.to_toml()?;
        write_private(&self.path, text.as_bytes())
    }
}

/// Atomic write with permissions `0600` (settings may reveal usage habits).
pub fn write_private(path: &Path, content: &[u8]) -> AppResult<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(content)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn roundtrip_and_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("cfg/settings.toml"));
        assert_eq!(store.load(), Settings::default());
        let s = Settings {
            mcp_enabled: true,
            ..Settings::default()
        };
        store.save(&s).unwrap();
        assert_eq!(store.load(), s);
        let mode = std::fs::metadata(store.path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn invalid_settings_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.toml"));
        let s = Settings {
            check_interval_hours: 7,
            ..Settings::default()
        };
        assert!(store.save(&s).is_err());
        assert!(!store.path().exists());
    }
}
