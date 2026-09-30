//! Service entry of the scheduled timer (`cachyos-center-helper preflight`).
//!
//! * `NotifyOnly`: isolated update check, result for the user notification.
//! * `PrepareForNextReboot` (in development, unlocked by the administrator):
//!   all safety gates must pass, then CachyOS `pacman-offline -y` prepares
//!   the update; installation happens on the next reboot started by the user.
//!   Blocked gates end the run as `NeedsAttention` without preparing
//!   anything; the policy stays active.

use std::path::Path;
use std::process::{Command, Stdio};

use cachyos_center_core::operation::{Operation, OperationKind, OperationOrigin, OperationState};
use cachyos_center_core::paths;
use cachyos_center_core::plan::PlanWarning;
use cachyos_center_core::policy::{AutoUpdateConfig, AutoUpdatePolicy};
use cachyos_center_core::updates::CheckStatus;
use cachyos_center_core::{AppError, ErrorCode, now};
use cachyos_center_packages::{PackageService, check, lock};
use cachyos_center_system::news::{self, NewsCache};
use cachyos_center_system::{power, updaters};

use crate::config::HelperConfig;
use crate::engine::new_id;
use crate::journal::Journal;

/// Minimum free space on `/` for an unattended preparation (2 GiB).
const MIN_FREE: u64 = 2 * 1024 * 1024 * 1024;

fn read_policy(path: &Path) -> Result<AutoUpdateConfig, AppError> {
    match std::fs::read_to_string(path) {
        Ok(text) => AutoUpdateConfig::from_toml(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AutoUpdateConfig::default()),
        Err(e) => Err(e.into()),
    }
}

struct Run<'a> {
    config: &'a HelperConfig,
    journal: Journal,
    op: Operation,
}

impl Run<'_> {
    fn save(&self) {
        if let Err(e) = self.journal.save(&self.op) {
            tracing::error!("cannot write journal: {e}");
        }
        let status = self.config.timer_status_file();
        if let Ok(text) = serde_json::to_vec_pretty(&self.op) {
            let tmp = status.with_extension("tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &status);
            }
        }
    }

    fn state(&mut self, next: OperationState) {
        let _ = self.op.transition(next, now());
        self.save();
    }

    fn needs_attention(&mut self, reasons: &[String]) -> i32 {
        self.op.summary = format!("not prepared: {}", reasons.join("; "));
        self.op.error = Some(
            AppError::new(
                ErrorCode::Blocked,
                "automatic update preparation was blocked",
            )
            .with_detail(reasons.join("\n")),
        );
        self.state(OperationState::NeedsAttention);
        0
    }
}

/// Runs the scheduled task once. Returns the process exit code.
pub async fn run(config: &HelperConfig) -> i32 {
    let policy = match read_policy(&config.policy_file()) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("cannot read policy: {e}");
            return 1;
        }
    };
    if policy.policy == AutoUpdatePolicy::Off {
        tracing::info!("automatic updates are off; nothing to do");
        return 0;
    }
    let kind = if policy.policy == AutoUpdatePolicy::NotifyOnly {
        OperationKind::UpdateCheck
    } else {
        OperationKind::AutoUpdatePrepare
    };
    let mut run = Run {
        config,
        journal: Journal::new(config.journal_dir()),
        op: Operation::new(new_id(), kind, OperationOrigin::Timer, now()),
    };
    run.state(OperationState::Checking);

    let packages = PackageService::new(
        Path::new(paths::SYSTEM_CHECK_DB).to_path_buf(),
        Path::new(paths::SYSTEM_CHECK_STATE).to_path_buf(),
    );
    if policy.policy == AutoUpdatePolicy::NotifyOnly {
        let result = tokio::task::spawn_blocking(move || packages.check_now()).await;
        match result {
            Ok(r) if r.status == CheckStatus::Fresh => {
                run.op.summary = format!("{} updates available", r.updates.len());
                run.op.progress.packages_total = u32::try_from(r.updates.len()).ok();
                run.state(OperationState::Succeeded);
            }
            Ok(r) => {
                let err = r
                    .error
                    .unwrap_or_else(|| AppError::unavailable("update check failed"));
                run.op.summary = err.message.clone();
                run.op.error = Some(err);
                run.state(OperationState::Failed);
            }
            Err(e) => {
                run.op.error = Some(AppError::internal(e.to_string()));
                run.state(OperationState::Failed);
            }
        }
        return 0;
    }
    prepare(&mut run, &policy, packages).await
}

async fn prepare(run: &mut Run<'_>, policy: &AutoUpdateConfig, packages: PackageService) -> i32 {
    let config = run.config;
    let mut reasons: Vec<String> = Vec::new();

    // Static gates.
    let experimental = updaters::experimental_offline_enabled(&config.experimental_file());
    let offline = updaters::offline_status();
    for b in updaters::prepare_blockers(true, experimental, &offline) {
        reasons.push(format!("configuration: {b}"));
    }
    if offline.prepared {
        run.op.summary = "an update is already prepared for the next reboot".into();
        run.state(OperationState::Succeeded);
        return 0;
    }
    if lock::is_locked(Path::new("/var/lib/pacman/db.lck"))
        || lock::package_manager_running() == Some(true)
    {
        reasons.push("another package manager is running".into());
    }
    if power::status().stable() == Some(false) {
        reasons.push("running on battery with low charge".into());
    }
    if !check::has_default_route() {
        reasons.push("no network connection".into());
    }
    if cachyos_center_system::hardware::free_bytes(Path::new("/")).is_some_and(|f| f < MIN_FREE) {
        reasons.push("less than 2 GiB free on /".into());
    }
    // News gate: unread or not checkable news block unattended installation.
    let news_path = Path::new(paths::SYSTEM_NEWS_CACHE);
    let cache = NewsCache::load(news_path);
    let t = now();
    let cache = tokio::task::spawn_blocking(move || news::fetch_all(&cache, t))
        .await
        .unwrap_or_default();
    let _ = cache.save(news_path);
    let news_status = news::status(&cache, policy.news_acknowledged_until, false, now());
    if !news_status.errors.is_empty() {
        reasons.push("news check not possible".into());
    }
    if news_status.unread_count > 0 {
        reasons.push(format!(
            "{} unread Arch Linux/CachyOS news",
            news_status.unread_count
        ));
    }
    if !reasons.is_empty() {
        return run.needs_attention(&reasons);
    }

    // Update check with the isolated database and plan review.
    let result = tokio::task::spawn_blocking(move || packages.check_now()).await;
    let result = match result {
        Ok(r) => r,
        Err(e) => return run.needs_attention(&[format!("update check failed: {e}")]),
    };
    if result.status != CheckStatus::Fresh {
        let msg = result
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| "update check failed".into());
        return run.needs_attention(&[format!(
            "update check failed (mirror, network or signature): {msg}"
        )]);
    }
    let Some(plan) = result.plan else {
        return run.needs_attention(&["no upgrade plan available".into()]);
    };
    if plan.entries.is_empty() {
        run.op.summary = "the system is up to date".into();
        run.state(OperationState::Succeeded);
        return 0;
    }
    for w in &plan.warnings {
        match w {
            PlanWarning::Replacements { packages } => reasons.push(format!(
                "package replacements need a confirmation: {}",
                packages.join(", ")
            )),
            PlanWarning::HeldBackPackages { packages } => reasons.push(format!(
                "packages are held back by the pacman configuration: {}",
                packages.join(", ")
            )),
            _ => {}
        }
    }
    if cachyos_center_system::hardware::free_bytes(Path::new("/"))
        .is_some_and(|f| f < MIN_FREE.max(plan.download_size.saturating_mul(3)))
    {
        reasons.push("not enough free space for the download".into());
    }
    if !reasons.is_empty() {
        return run.needs_attention(&reasons);
    }
    run.op.progress.packages_total = u32::try_from(plan.entries.len()).ok();
    run.state(OperationState::Ready);
    run.state(OperationState::Preparing);

    if policy.require_snapshot {
        let support = cachyos_center_system::health::snapshot_support();
        let name = support.root_config.clone().unwrap_or_else(|| "root".into());
        let result = if support.can_request_snapshot {
            crate::snapshot::create(&config.snapper, &name)
        } else {
            cachyos_center_core::operation::SnapshotResult {
                created: false,
                snapper_config: name,
                number: None,
                error: Some("snapper is not set up".into()),
            }
        };
        let ok = result.created;
        run.op.snapshot = Some(result);
        if !ok {
            let _ = run.op.transition(OperationState::NeedsAttention, now());
            run.op.summary = "not prepared: the required snapshot could not be created".into();
            run.save();
            return 0;
        }
    }

    // Preparation with the documented CachyOS mechanism.
    run.state(OperationState::Downloading);
    let log = config.operation_log(&run.op.id);
    let bin = config.pacman_offline.clone();
    let status = tokio::task::spawn_blocking(move || {
        if let Some(dir) = log.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)?;
        let err = out.try_clone()?;
        Command::new(&bin)
            .arg("-y")
            .env_clear()
            .env("PATH", "/usr/bin")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .status()
    })
    .await;
    let prepared = updaters::offline_status().prepared;
    match status {
        Ok(Ok(s)) if s.success() && prepared => {
            run.op.exit_code = s.code();
            run.op.summary = "prepared; the update will be installed on the next reboot".into();
            run.state(OperationState::Succeeded);
        }
        Ok(Ok(s)) => {
            run.op.exit_code = s.code();
            run.op.summary =
                "pacman-offline did not prepare the update; nothing was installed".into();
            run.op.error = Some(AppError::new(
                ErrorCode::TransactionFailed,
                run.op.summary.clone(),
            ));
            run.state(OperationState::Failed);
        }
        Ok(Err(e)) => {
            let msg = format!("cannot run pacman-offline: {e}");
            run.op.summary = msg.clone();
            run.op.error = Some(AppError::unavailable(msg));
            run.state(OperationState::Failed);
        }
        Err(e) => {
            let msg = format!("worker failed: {e}");
            run.op.summary = msg.clone();
            run.op.error = Some(AppError::internal(msg));
            run.state(OperationState::Failed);
        }
    }
    0
}
