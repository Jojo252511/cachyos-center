//! Configuration of the helper.
//!
//! The production configuration is fixed at compile time. A development
//! configuration (session bus, sandbox directories, alternative pacman) can
//! only be selected by an unprivileged process; the helper refuses it when
//! running as root.

use std::path::PathBuf;
use std::time::Duration;

use cachyos_center_core::paths;

/// Bus to serve on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusKind {
    System,
    /// Development and tests only (never as root).
    Session,
}

/// How pacman steps are executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnMode {
    /// Each step runs in a transient systemd unit (`systemd-run --wait`), so a
    /// crash or restart of the helper never kills pacman during a commit.
    SystemdRun,
    /// Direct child process (tests, development).
    Direct,
}

/// How authorization is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMode {
    /// polkit `CheckAuthorization` for the D-Bus sender (production).
    Polkit,
    /// Development/tests: allow everything except the listed actions.
    TestDeny(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct HelperConfig {
    pub bus: BusKind,
    pub spawn: SpawnMode,
    pub auth: AuthMode,
    /// pacman executable.
    pub pacman: PathBuf,
    /// Alternative pacman configuration (sandbox tests); `None` = `/etc/pacman.conf`.
    pub pacman_conf: Option<PathBuf>,
    /// Prefix pacman with `fakeroot` (sandbox tests with a private root).
    pub fakeroot: bool,
    /// pacman log used to evaluate transactions.
    pub pacman_log: PathBuf,
    /// Journal and status files.
    pub state_dir: PathBuf,
    /// Operation logs.
    pub log_dir: PathBuf,
    /// `/etc/cachyos-center`.
    pub config_dir: PathBuf,
    /// systemd drop-in directory of the preflight timer.
    pub timer_dropin_dir: PathBuf,
    pub snapper: PathBuf,
    pub pacman_offline: PathBuf,
    /// Exit after this idle time without active operation.
    pub idle_timeout: Duration,
    /// Maximum time to wait for a foreign pacman lock (bounded backoff).
    pub lock_wait: Duration,
    /// Maximum time for a polkit dialog.
    pub auth_timeout: Duration,
    /// Extra environment for pacman (tests only).
    pub extra_env: Vec<(String, String)>,
}

impl HelperConfig {
    /// Fixed production configuration.
    pub fn production() -> Self {
        Self {
            bus: BusKind::System,
            spawn: SpawnMode::SystemdRun,
            auth: AuthMode::Polkit,
            pacman: PathBuf::from("/usr/bin/pacman"),
            pacman_conf: None,
            fakeroot: false,
            pacman_log: PathBuf::from("/var/log/pacman.log"),
            state_dir: PathBuf::from(paths::SYSTEM_STATE_DIR),
            log_dir: PathBuf::from(paths::SYSTEM_LOG_DIR),
            config_dir: PathBuf::from(paths::SYSTEM_CONFIG_DIR),
            timer_dropin_dir: PathBuf::from("/etc/systemd/system/cachyos-center-preflight.timer.d"),
            snapper: PathBuf::from("/usr/bin/snapper"),
            pacman_offline: PathBuf::from("/usr/bin/pacman-offline"),
            idle_timeout: Duration::from_secs(600),
            lock_wait: Duration::from_secs(60),
            auth_timeout: Duration::from_secs(300),
            extra_env: Vec::new(),
        }
    }

    /// Development configuration below `root` (session bus, direct spawn).
    pub fn development(root: PathBuf) -> Self {
        Self {
            bus: BusKind::Session,
            spawn: SpawnMode::Direct,
            auth: AuthMode::TestDeny(Vec::new()),
            pacman: PathBuf::from("/usr/bin/pacman"),
            pacman_conf: None,
            fakeroot: false,
            pacman_log: root.join("pacman.log"),
            state_dir: root.join("state"),
            log_dir: root.join("log"),
            config_dir: root.join("etc"),
            timer_dropin_dir: root.join("timer.d"),
            snapper: PathBuf::from("/usr/bin/snapper"),
            pacman_offline: PathBuf::from("/usr/bin/pacman-offline"),
            idle_timeout: Duration::from_secs(3600),
            lock_wait: Duration::from_secs(10),
            auth_timeout: Duration::from_secs(60),
            extra_env: Vec::new(),
        }
    }

    pub fn is_production(&self) -> bool {
        self.bus == BusKind::System
    }

    pub fn journal_dir(&self) -> PathBuf {
        self.state_dir.join("operations")
    }

    pub fn policy_file(&self) -> PathBuf {
        self.config_dir.join("auto-update.toml")
    }

    pub fn experimental_file(&self) -> PathBuf {
        self.config_dir.join("experimental.toml")
    }

    pub fn timer_status_file(&self) -> PathBuf {
        self.state_dir.join("timer-status.json")
    }

    /// Isolated update-check database of the timer.
    pub fn check_db(&self) -> PathBuf {
        self.state_dir.join("checkup-db")
    }

    /// Metadata of the timer's update check.
    pub fn check_state(&self) -> PathBuf {
        self.state_dir.join("update-check.json")
    }

    /// News cache of the timer.
    pub fn news_cache(&self) -> PathBuf {
        self.state_dir.join("news.json")
    }

    pub fn operation_log(&self, id: &str) -> PathBuf {
        self.log_dir.join(format!("{id}.log"))
    }

    /// Lock file of the pacman database used by this configuration.
    pub fn db_lock(&self, db_path: &str) -> PathBuf {
        PathBuf::from(db_path).join("db.lck")
    }
}

/// `true` when the process runs as root.
pub fn is_root() -> bool {
    rustix_like_geteuid() == 0
}

#[allow(unsafe_code)]
fn rustix_like_geteuid() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}
