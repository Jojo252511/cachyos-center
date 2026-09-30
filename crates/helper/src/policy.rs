//! `SetAutoUpdatePolicy`: writes the policy file and schedules the preflight timer.
//!
//! Only this helper writes `/etc/cachyos-center/auto-update.toml` and the
//! timer drop-in. `/etc/pacman.conf` and `/etc/pacman.d/offline.conf` are never
//! modified by this code path.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use cachyos_center_core::dbus::{PREFLIGHT_TIMER, actions};
use cachyos_center_core::policy::{AutoUpdateConfig, AutoUpdatePolicy};
use cachyos_center_core::{AppError, AppResult, ErrorCode};
use cachyos_center_system::updaters;

use crate::authz::{self, Caller};
use crate::config::HelperConfig;

fn write_file(path: &Path, content: &str) -> AppResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o644)
            .open(&tmp)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Drop-in for the preflight timer (schedule of the update window).
pub fn timer_dropin(config: &AutoUpdateConfig) -> AppResult<String> {
    Ok(format!(
        "# Managed by cachyos-center-helper. Do not edit manually.\n[Timer]\nOnCalendar=\nOnCalendar={}\n",
        config.window.on_calendar()?
    ))
}

fn systemctl(args: &[&str]) -> AppResult<()> {
    let status = Command::new("/usr/bin/systemctl")
        .args(args)
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| AppError::unavailable(format!("cannot run systemctl: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::unavailable(format!(
            "systemctl {} failed",
            args.join(" ")
        )))
    }
}

/// Checks whether `config` may be activated on this system.
pub fn check_activation(helper: &HelperConfig, config: &AutoUpdateConfig) -> AppResult<()> {
    config.validate()?;
    if config.policy == AutoUpdatePolicy::PrepareForNextReboot {
        let experimental = updaters::experimental_offline_enabled(&helper.experimental_file());
        let offline = updaters::offline_status();
        let blockers = updaters::prepare_blockers(true, experimental, &offline);
        if !blockers.is_empty() {
            return Err(AppError::new(
                ErrorCode::Blocked,
                "automatic installation on the next reboot cannot be activated",
            )
            .with_detail(blockers.join(",")));
        }
    }
    Ok(())
}

/// Applies a new policy after polkit authorization.
pub async fn apply(
    helper: &HelperConfig,
    conn: &zbus::Connection,
    caller: &Caller,
    config: AutoUpdateConfig,
) -> AppResult<()> {
    check_activation(helper, &config)?;
    authz::authorize(
        &helper.auth,
        conn,
        caller,
        actions::CONFIGURE_AUTO_UPDATE,
        "configure-auto-update",
        helper.auth_timeout,
    )
    .await?;
    write_file(&helper.policy_file(), &config.to_toml()?)?;
    write_file(
        &helper.timer_dropin_dir.join("schedule.conf"),
        &timer_dropin(&config)?,
    )?;
    if helper.is_production() {
        systemctl(&["daemon-reload"])?;
        if config.policy == AutoUpdatePolicy::Off {
            systemctl(&["disable", "--now", PREFLIGHT_TIMER])?;
        } else {
            systemctl(&["enable", "--now", PREFLIGHT_TIMER])?;
        }
    }
    tracing::info!(
        policy = config.policy.as_str(),
        "auto-update policy changed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cachyos_center_core::policy::{UpdateWindow, Weekday};

    #[test]
    fn dropin_content() {
        let config = AutoUpdateConfig {
            policy: AutoUpdatePolicy::NotifyOnly,
            window: UpdateWindow {
                weekdays: vec![Weekday::Sat, Weekday::Sun],
                time: "09:15".into(),
            },
            require_snapshot: false,
            news_acknowledged_until: None,
        };
        let text = timer_dropin(&config).unwrap();
        assert!(text.contains("OnCalendar=\nOnCalendar=Sat,Sun *-*-* 09:15:00\n"));
    }

    #[test]
    fn prepare_mode_is_blocked_without_unlock() {
        let dir = tempfile::tempdir().unwrap();
        let helper = HelperConfig::development(dir.path().to_path_buf());
        let config = AutoUpdateConfig {
            policy: AutoUpdatePolicy::PrepareForNextReboot,
            ..AutoUpdateConfig::default()
        };
        let err = check_activation(&helper, &config).unwrap_err();
        assert_eq!(err.code, ErrorCode::Blocked);
        assert!(err.detail.unwrap().contains("experimentalLocked"));
        let notify = AutoUpdateConfig {
            policy: AutoUpdatePolicy::NotifyOnly,
            ..AutoUpdateConfig::default()
        };
        check_activation(&helper, &notify).unwrap();
    }
}
