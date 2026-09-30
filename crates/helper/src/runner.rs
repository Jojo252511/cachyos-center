//! Execution of pacman with fixed argument vectors.
//!
//! There is no shell and no free command: [`Step`] enumerates every pacman
//! invocation the helper can make. Package and repository names are
//! validated before they reach this module and are always passed after `--`.
//! No step ever uses `--overwrite`, `--nodeps`, `-dd`, `--force`, `-Sy`
//! without `-u` on a target, or removes the database lock.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cachyos_center_core::{AppError, AppResult, ErrorCode, validate};

use crate::config::{HelperConfig, SpawnMode};

/// One pacman invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// `pacman -Sy` – refresh the sync databases (first phase of `-Syu`).
    Refresh,
    /// `pacman -Suw` – download and verify all upgrades, change nothing.
    DownloadUpgrade,
    /// `pacman -Su` – commit the full upgrade.
    CommitUpgrade,
    /// `pacman -Suw --needed -- repo/name`
    DownloadInstall { repository: String, name: String },
    /// `pacman -Su --needed -- repo/name`
    CommitInstall { repository: String, name: String },
    /// `pacman -R -- name` or `pacman -Rs -- name`
    Remove { name: String, recursive: bool },
}

impl Step {
    /// Short name used for unit names and logs.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Refresh => "refresh",
            Self::DownloadUpgrade | Self::DownloadInstall { .. } => "download",
            Self::CommitUpgrade | Self::CommitInstall { .. } => "commit",
            Self::Remove { .. } => "remove",
        }
    }

    /// Steps that may be interrupted safely (nothing installed yet).
    pub fn cancellable(&self) -> bool {
        matches!(
            self,
            Self::Refresh | Self::DownloadUpgrade | Self::DownloadInstall { .. }
        )
    }

    /// The step changes installed packages.
    pub fn commits(&self) -> bool {
        matches!(
            self,
            Self::CommitUpgrade | Self::CommitInstall { .. } | Self::Remove { .. }
        )
    }

    /// pacman arguments (without the pacman binary itself).
    pub fn pacman_args(&self, config: Option<&Path>) -> AppResult<Vec<String>> {
        let mut args: Vec<String> = match self {
            Self::Refresh => vec!["-Sy".into()],
            Self::DownloadUpgrade => vec!["-Suw".into()],
            Self::CommitUpgrade => vec!["-Su".into()],
            Self::DownloadInstall { .. } => vec!["-Suw".into(), "--needed".into()],
            Self::CommitInstall { .. } => vec!["-Su".into(), "--needed".into()],
            Self::Remove { recursive, .. } => {
                vec![if *recursive {
                    "-Rs".into()
                } else {
                    "-R".into()
                }]
            }
        };
        args.extend(
            ["--noconfirm", "--noprogressbar", "--color", "never"]
                .iter()
                .map(|s| (*s).to_string()),
        );
        if let Some(conf) = config {
            args.push("--config".into());
            args.push(conf.to_string_lossy().into_owned());
        }
        match self {
            Self::DownloadInstall { repository, name }
            | Self::CommitInstall { repository, name } => {
                validate::repo_name(repository)?;
                validate::package_name(name)?;
                args.push("--".into());
                args.push(format!("{repository}/{name}"));
            }
            Self::Remove { name, .. } => {
                validate::package_name(name)?;
                args.push("--".into());
                args.push(name.clone());
            }
            _ => {}
        }
        Ok(args)
    }
}

/// Cooperative cancellation flag shared between the D-Bus method and the flow.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Result of a step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutcome {
    pub exit_code: Option<i32>,
    pub cancelled: bool,
}

impl StepOutcome {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0) && !self.cancelled
    }
}

/// Transient unit name of a step.
pub fn unit_name(op_id: &str, step: &Step) -> String {
    format!(
        "cachyos-center-op-{}-{}",
        &op_id[..8.min(op_id.len())],
        step.slug()
    )
}

fn open_log(log: &Path) -> AppResult<std::fs::File> {
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(OpenOptions::new().create(true).append(true).open(log)?)
}

fn append_line(log: &Path, line: &str) {
    if let Ok(mut f) = open_log(log) {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
}

#[allow(unsafe_code)]
fn send_signal(pid: u32, signal: libc::c_int) {
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        // SAFETY: signal delivery to our own child process.
        unsafe {
            libc::kill(pid, signal);
        }
    }
}

/// Runs a step and blocks until it has finished. Must be called from a
/// blocking context (`spawn_blocking`).
pub fn run_step(
    config: &HelperConfig,
    op_id: &str,
    step: &Step,
    log: &Path,
    cancel: &CancelFlag,
) -> AppResult<StepOutcome> {
    let args = step.pacman_args(config.pacman_conf.as_deref())?;
    append_line(
        log,
        &format!("==> cachyos-center: pacman {}", args.join(" ")),
    );
    let (program, full_args): (PathBuf, Vec<String>) = match config.spawn {
        SpawnMode::Direct => {
            if config.fakeroot {
                let mut a = vec![
                    "--".to_string(),
                    config.pacman.to_string_lossy().into_owned(),
                ];
                a.extend(args);
                (PathBuf::from("/usr/bin/fakeroot"), a)
            } else {
                (config.pacman.clone(), args)
            }
        }
        SpawnMode::SystemdRun => {
            let log_str = log.to_string_lossy();
            let mut a = vec![
                format!("--unit={}", unit_name(op_id, step)),
                "--description=cachyos-center package operation".to_string(),
                "--quiet".to_string(),
                "--wait".to_string(),
                "--collect".to_string(),
                "--service-type=exec".to_string(),
                format!("--property=StandardOutput=append:{log_str}"),
                format!("--property=StandardError=append:{log_str}"),
                // Give pacman time to finish cleanly when the system shuts down.
                "--property=TimeoutStopSec=15min".to_string(),
                "--property=KillSignal=SIGINT".to_string(),
                "--setenv=LC_ALL=C".to_string(),
                "--".to_string(),
                config.pacman.to_string_lossy().into_owned(),
            ];
            a.extend(args);
            (PathBuf::from("/usr/bin/systemd-run"), a)
        }
    };

    let mut cmd = Command::new(&program);
    cmd.args(&full_args)
        .env_clear()
        .env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin")
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    for (k, v) in &config.extra_env {
        cmd.env(k, v);
    }
    match config.spawn {
        SpawnMode::Direct => {
            let out = open_log(log)?;
            let err = out.try_clone()?;
            cmd.stdout(out).stderr(err);
        }
        SpawnMode::SystemdRun => {
            // systemd-run itself only prints errors; the unit output goes to the log.
            let out = open_log(log)?;
            let err = out.try_clone()?;
            cmd.stdout(out).stderr(err);
        }
    }
    let mut child = cmd.spawn().map_err(|e| {
        AppError::new(
            ErrorCode::Unavailable,
            format!("cannot start {}: {e}", program.display()),
        )
    })?;
    let mut cancelled = false;
    loop {
        if let Some(status) = child.try_wait()? {
            let outcome = StepOutcome {
                exit_code: status.code(),
                cancelled,
            };
            append_line(
                log,
                &format!(
                    "==> cachyos-center: step {} finished with exit code {:?}",
                    step.slug(),
                    outcome.exit_code
                ),
            );
            return Ok(outcome);
        }
        if !cancelled && cancel.is_cancelled() && step.cancellable() {
            cancelled = true;
            append_line(
                log,
                "==> cachyos-center: cancellation requested, interrupting pacman",
            );
            match config.spawn {
                SpawnMode::Direct => send_signal(child.id(), libc::SIGINT),
                SpawnMode::SystemdRun => {
                    let _ = Command::new("/usr/bin/systemctl")
                        .args(["kill", "--signal=SIGINT", "--kill-whom=main", "--"])
                        .arg(format!("{}.service", unit_name(op_id, step)))
                        .env_clear()
                        .env("LC_ALL", "C")
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// `true` when the transient unit of a step is still running (crash recovery).
pub fn unit_active(unit: &str) -> bool {
    cachyos_center_system::units::show(
        cachyos_center_system::units::Scope::System,
        &format!("{unit}.service"),
    )
    .is_some_and(|s| s.active())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(step: Step) -> Vec<String> {
        step.pacman_args(None).unwrap()
    }

    #[test]
    fn fixed_argument_vectors() {
        assert_eq!(
            args(Step::Refresh),
            vec!["-Sy", "--noconfirm", "--noprogressbar", "--color", "never"]
        );
        assert_eq!(args(Step::DownloadUpgrade)[0], "-Suw");
        assert_eq!(args(Step::CommitUpgrade)[0], "-Su");
        let install = args(Step::CommitInstall {
            repository: "extra".into(),
            name: "firefox".into(),
        });
        assert_eq!(install[..2], ["-Su".to_string(), "--needed".to_string()]);
        assert_eq!(
            install[install.len() - 2..],
            ["--".to_string(), "extra/firefox".to_string()]
        );
        let remove = args(Step::Remove {
            name: "firefox".into(),
            recursive: true,
        });
        assert_eq!(remove[0], "-Rs");
        assert_eq!(remove.last().unwrap(), "firefox");
    }

    #[test]
    fn never_dangerous_flags() {
        let steps = [
            Step::Refresh,
            Step::DownloadUpgrade,
            Step::CommitUpgrade,
            Step::DownloadInstall {
                repository: "core".into(),
                name: "a".into(),
            },
            Step::CommitInstall {
                repository: "core".into(),
                name: "a".into(),
            },
            Step::Remove {
                name: "a".into(),
                recursive: false,
            },
            Step::Remove {
                name: "a".into(),
                recursive: true,
            },
        ];
        for step in steps {
            for a in args(step.clone()) {
                for bad in [
                    "--overwrite",
                    "--nodeps",
                    "-dd",
                    "-Rdd",
                    "--force",
                    "-c",
                    "--cascade",
                    "-n",
                    "--nosave",
                    "--dbonly",
                    "--noscriptlet",
                    "--asdeps",
                    "--disable-sandbox",
                ] {
                    assert_ne!(a, bad, "{step:?}");
                }
            }
        }
    }

    #[test]
    fn rejects_invalid_targets() {
        assert!(
            Step::CommitInstall {
                repository: "core".into(),
                name: "--overwrite=*".into()
            }
            .pacman_args(None)
            .is_err()
        );
        assert!(
            Step::Remove {
                name: "-x".into(),
                recursive: false
            }
            .pacman_args(None)
            .is_err()
        );
        assert!(
            Step::DownloadInstall {
                repository: "../x".into(),
                name: "a".into()
            }
            .pacman_args(None)
            .is_err()
        );
    }

    #[test]
    fn cancellable_only_before_commit() {
        assert!(Step::Refresh.cancellable());
        assert!(Step::DownloadUpgrade.cancellable());
        assert!(!Step::CommitUpgrade.cancellable());
        assert!(
            !Step::Remove {
                name: "a".into(),
                recursive: false
            }
            .cancellable()
        );
    }

    #[test]
    fn unit_names() {
        let n = unit_name("123e4567-e89b-42d3-a456-426614174000", &Step::CommitUpgrade);
        assert_eq!(n, "cachyos-center-op-123e4567-commit");
    }
}
