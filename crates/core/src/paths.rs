//! File system locations.
//!
//! User data follows the XDG base directory specification; nothing is hard
//! coded to a home directory. System-wide state is only written by the helper.

use std::path::{Path, PathBuf};

/// System-wide configuration written by the helper (`auto-update.toml`).
pub const SYSTEM_CONFIG_DIR: &str = "/etc/cachyos-center";
/// Auto-update policy file.
pub const POLICY_FILE: &str = "/etc/cachyos-center/auto-update.toml";
/// Administrator switch for features that are still in development.
pub const EXPERIMENTAL_FILE: &str = "/etc/cachyos-center/experimental.toml";
/// Helper state (operation journal, timer status). World-readable, root-writable.
pub const SYSTEM_STATE_DIR: &str = "/var/lib/cachyos-center";
/// Operation logs of the helper.
pub const SYSTEM_LOG_DIR: &str = "/var/log/cachyos-center";
/// Status of the last timer run, watched by the user notification unit.
pub const TIMER_STATUS_FILE: &str = "/var/lib/cachyos-center/timer-status.json";
/// Installation directory of the helper and the libalpm bridge.
pub const LIBEXEC_DIR: &str = "/usr/lib/cachyos-center";
/// File name of the libalpm bridge library.
pub const ALPM_BRIDGE_FILE: &str = "libcachyos_center_alpm.so";

/// Per-user directories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDirs {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub state: PathBuf,
}

impl UserDirs {
    /// Resolves the XDG directories from the environment.
    ///
    /// Returns `None` when neither the XDG variables nor `HOME` are set.
    pub fn from_env() -> Option<Self> {
        Self::resolve(|k| std::env::var_os(k).map(PathBuf::from))
    }

    /// Resolution with an injectable environment (for tests).
    pub fn resolve(env: impl Fn(&str) -> Option<PathBuf>) -> Option<Self> {
        let home = env("HOME").filter(|p| p.is_absolute());
        let base = |var: &str, fallback: &str| -> Option<PathBuf> {
            env(var)
                .filter(|p| p.is_absolute())
                .or_else(|| home.as_ref().map(|h| h.join(fallback)))
        };
        Some(Self {
            config: base("XDG_CONFIG_HOME", ".config")?.join(crate::APP_NAME),
            data: base("XDG_DATA_HOME", ".local/share")?.join(crate::APP_NAME),
            cache: base("XDG_CACHE_HOME", ".cache")?.join(crate::APP_NAME),
            state: base("XDG_STATE_HOME", ".local/state")?.join(crate::APP_NAME),
        })
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config.join("settings.toml")
    }

    pub fn history_db(&self) -> PathBuf {
        self.data.join("history.sqlite3")
    }

    pub fn operation_logs(&self) -> PathBuf {
        self.data.join("logs")
    }

    /// Isolated sync database of the update check (`CHECKUPDATES_DB`).
    pub fn check_db(&self) -> PathBuf {
        self.cache.join("checkup-db")
    }

    /// Metadata of the last update check.
    pub fn check_state(&self) -> PathBuf {
        self.cache.join("update-check.json")
    }

    pub fn news_cache(&self) -> PathBuf {
        self.cache.join("news.json")
    }
}

/// `true` when `path` is below `dir` after lexical normalization (no `..`).
pub fn is_below(path: &Path, dir: &Path) -> bool {
    use std::path::Component;
    if path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return false;
    }
    path.starts_with(dir) && path != dir
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<PathBuf> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| PathBuf::from(v))
        }
    }

    #[test]
    fn xdg_variables_take_precedence() {
        let dirs = UserDirs::resolve(env(&[
            ("HOME", "/home/u"),
            ("XDG_CONFIG_HOME", "/cfg"),
            ("XDG_DATA_HOME", "/data"),
        ]))
        .unwrap();
        assert_eq!(dirs.config, PathBuf::from("/cfg/cachyos-center"));
        assert_eq!(dirs.data, PathBuf::from("/data/cachyos-center"));
        assert_eq!(dirs.cache, PathBuf::from("/home/u/.cache/cachyos-center"));
        assert_eq!(
            dirs.state,
            PathBuf::from("/home/u/.local/state/cachyos-center")
        );
    }

    #[test]
    fn relative_xdg_values_are_ignored() {
        let dirs = UserDirs::resolve(env(&[("HOME", "/h"), ("XDG_CONFIG_HOME", "rel")])).unwrap();
        assert_eq!(dirs.config, PathBuf::from("/h/.config/cachyos-center"));
    }

    #[test]
    fn no_home_no_dirs() {
        assert!(UserDirs::resolve(env(&[])).is_none());
    }

    #[test]
    fn below() {
        assert!(is_below(
            Path::new("/etc/pacman.conf.pacnew"),
            Path::new("/etc")
        ));
        assert!(!is_below(Path::new("/etc/../root/x"), Path::new("/etc")));
        assert!(!is_below(Path::new("/etc"), Path::new("/etc")));
        assert!(!is_below(Path::new("/etcetera/x"), Path::new("/etc")));
    }
}
