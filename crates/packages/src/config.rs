//! pacman configuration as seen by `pacman-conf`.
//!
//! The repository list is never hard coded: it is taken from the active
//! pacman configuration, including `Include` files and `IgnorePkg` entries.

use std::path::{Path, PathBuf};

use cachyos_center_core::bridge::{AlpmConfig, RepoConfig};
use cachyos_center_core::{AppError, AppResult, ErrorCode};

/// Default pacman configuration file.
pub const PACMAN_CONF: &str = "/etc/pacman.conf";

/// Reads the configuration from `pacman.conf` (or an alternative file for tests).
pub fn load(conf_file: Option<&Path>) -> AppResult<AlpmConfig> {
    let mut opts = pacmanconf::Options::new();
    if let Some(file) = conf_file {
        opts.pacman_conf(file.to_string_lossy().into_owned());
    }
    let conf = opts.read().map_err(|e| {
        AppError::new(
            ErrorCode::Unavailable,
            format!("cannot read the pacman configuration: {e}"),
        )
    })?;
    Ok(from_pacmanconf(conf))
}

fn from_pacmanconf(conf: pacmanconf::Config) -> AlpmConfig {
    AlpmConfig {
        root_dir: conf.root_dir,
        db_path: conf.db_path,
        gpg_dir: conf.gpg_dir,
        cache_dirs: conf.cache_dir,
        architectures: conf.architecture,
        ignore_pkgs: conf.ignore_pkg,
        ignore_groups: conf.ignore_group,
        hold_pkgs: conf.hold_pkg,
        sig_level: conf.sig_level,
        repos: conf
            .repos
            .into_iter()
            .map(|r| RepoConfig {
                name: r.name,
                sig_level: r.sig_level,
                usage: r.usage,
            })
            .collect(),
    }
}

/// Copy of `config` that reads the sync databases from `db_path` instead
/// (used for the isolated database of the update check).
pub fn with_db_path(config: &AlpmConfig, db_path: &Path) -> AlpmConfig {
    let mut c = config.clone();
    let mut p = db_path.to_string_lossy().into_owned();
    if !p.ends_with('/') {
        p.push('/');
    }
    c.db_path = p;
    c
}

/// Path of the productive database lock file.
pub fn lock_file(config: &AlpmConfig) -> PathBuf {
    PathBuf::from(&config.db_path).join("db.lck")
}

/// Path of the productive local database.
pub fn local_db(config: &AlpmConfig) -> PathBuf {
    PathBuf::from(&config.db_path).join("local")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_path_override_has_trailing_slash() {
        let config = AlpmConfig {
            root_dir: "/".into(),
            db_path: "/var/lib/pacman/".into(),
            gpg_dir: String::new(),
            cache_dirs: vec![],
            architectures: vec![],
            ignore_pkgs: vec![],
            ignore_groups: vec![],
            hold_pkgs: vec![],
            sig_level: vec![],
            repos: vec![],
        };
        let c = with_db_path(&config, Path::new("/tmp/check"));
        assert_eq!(c.db_path, "/tmp/check/");
        assert_eq!(lock_file(&config), PathBuf::from("/var/lib/pacman/db.lck"));
    }

    #[test]
    fn reads_the_system_configuration() {
        // Every supported system has a pacman configuration.
        if !Path::new(PACMAN_CONF).exists() {
            return;
        }
        let config = load(None).expect("pacman-conf must work on an Arch based system");
        assert!(!config.db_path.is_empty());
        assert!(!config.repos.is_empty());
    }
}
