//! Transaction planning without the database lock (like `pacman -Sp`/`-Rp`).
//!
//! Plans mirror the non-interactive pacman invocations the helper uses:
//! * system upgrade: `pacman -Su --noconfirm`
//! * install:        `pacman -Su --needed --noconfirm repo/pkg` (targets first, then sysupgrade)
//! * remove:         `pacman -R[s] --noconfirm pkg`

use std::cmp::Ordering;

use alpm::{Alpm, PrepareData, PrepareError, TransFlag};
use cachyos_center_core::bridge::{AlpmConfig, ClassifyContext};
use cachyos_center_core::plan::{
    PlanAction, PlanEntry, PlanKind, PlanSource, PlanWarning, TransactionPlan,
};
use cachyos_center_core::updates::UpdateFlag;
use cachyos_center_core::{AppError, AppResult, ErrorCode, classify, validate};

/// Maximum number of detail lines in a planning error.
const MAX_DETAIL_LINES: usize = 25;

fn alpm_err(context: &str, err: alpm::Error) -> AppError {
    let code = match err {
        alpm::Error::PkgNotFound | alpm::Error::DbNotFound => ErrorCode::NotFound,
        alpm::Error::HandleLock => ErrorCode::Busy,
        alpm::Error::UnsatisfiedDeps | alpm::Error::ConflictingDeps => ErrorCode::DependencyProblem,
        _ => ErrorCode::Internal,
    };
    AppError::new(code, format!("{context}: {err}"))
}

fn prepare_error(err: &PrepareError<'_>) -> AppError {
    let mut lines = Vec::new();
    match err.data() {
        Some(PrepareData::UnsatisfiedDeps(list)) => {
            for missing in list {
                let cause = missing
                    .causing_pkg()
                    .map(|c| format!(" (required by {c})"))
                    .unwrap_or_default();
                lines.push(format!(
                    "{}: requires {}{cause}",
                    missing.target(),
                    missing.depend()
                ));
            }
        }
        Some(PrepareData::ConflictingDeps(list)) => {
            for conflict in list {
                lines.push(format!(
                    "{} and {} are in conflict ({})",
                    conflict.package1().name(),
                    conflict.package2().name(),
                    conflict.reason()
                ));
            }
        }
        Some(PrepareData::PkgInvalidArch(list)) => {
            for pkg in list {
                lines.push(format!("{}: package architecture is not valid", pkg.name()));
            }
        }
        None => {}
    }
    let code = match err.error() {
        alpm::Error::UnsatisfiedDeps
        | alpm::Error::ConflictingDeps
        | alpm::Error::PkgInvalidArch => ErrorCode::DependencyProblem,
        other => return alpm_err("pacman cannot plan the transaction", other),
    };
    let mut error = AppError::new(
        code,
        format!("pacman cannot plan the transaction: {}", err.error()),
    );
    if !lines.is_empty() {
        lines.truncate(MAX_DETAIL_LINES);
        error = error.with_detail(lines.join("\n"));
    }
    error
}

fn to_u64(v: i64) -> Option<u64> {
    u64::try_from(v).ok()
}

fn collect(
    alpm: &Alpm,
    kind: PlanKind,
    targets: &[String],
    recursive: bool,
    config: &AlpmConfig,
    context: &ClassifyContext,
) -> TransactionPlan {
    let local = alpm.localdb();
    let kernel = context.running_kernel_pkg.as_deref();
    let mut entries = Vec::new();
    let mut download: u64 = 0;
    let mut size_delta: i64 = 0;
    for pkg in alpm.trans_add() {
        let old = local.pkg(pkg.name()).ok();
        let action = match old {
            None => PlanAction::Install,
            Some(o) => match pkg.version().vercmp(o.version()) {
                Ordering::Greater => PlanAction::Upgrade,
                Ordering::Less => PlanAction::Downgrade,
                Ordering::Equal => PlanAction::Reinstall,
            },
        };
        let dl = to_u64(pkg.download_size());
        download = download.saturating_add(dl.unwrap_or(0));
        size_delta = size_delta
            .saturating_add(pkg.isize())
            .saturating_sub(old.map(|o| o.isize()).unwrap_or(0));
        entries.push(PlanEntry {
            action,
            name: pkg.name().to_string(),
            repository: pkg.db().map(|d| d.name().to_string()),
            old_version: old.map(|o| o.version().to_string()),
            new_version: Some(pkg.version().to_string()),
            download_size: dl,
            requested: targets.iter().any(|t| t == pkg.name()),
            flags: classify::update_flags(pkg.name(), kernel, context.session_is_hyprland, false),
        });
    }
    for pkg in alpm.trans_remove() {
        size_delta = size_delta.saturating_sub(pkg.isize());
        entries.push(PlanEntry {
            action: PlanAction::Remove,
            name: pkg.name().to_string(),
            repository: None,
            old_version: Some(pkg.version().to_string()),
            new_version: None,
            download_size: None,
            requested: targets.iter().any(|t| t == pkg.name()),
            flags: Vec::new(),
        });
    }
    entries.sort_by(|a, b| a.action.cmp(&b.action).then_with(|| a.name.cmp(&b.name)));

    let mut warnings = Vec::new();
    match kind {
        PlanKind::Install => {
            let upgrades = entries
                .iter()
                .filter(|e| {
                    !e.requested
                        && e.action != PlanAction::Install
                        && e.action != PlanAction::Remove
                })
                .count() as u64;
            if upgrades > 0 {
                warnings.push(PlanWarning::IncludesSystemUpgrade {
                    upgrade_count: upgrades,
                });
            }
        }
        PlanKind::Remove => {
            let critical: Vec<String> = entries
                .iter()
                .filter(|e| {
                    classify::is_critical(
                        &e.name,
                        &config.hold_pkgs,
                        kernel,
                        context.session_is_hyprland,
                    )
                })
                .map(|e| e.name.clone())
                .collect();
            if !critical.is_empty() {
                warnings.push(PlanWarning::CriticalPackages { packages: critical });
            }
        }
        PlanKind::SystemUpgrade => {}
    }
    if kind != PlanKind::Remove {
        let replaced: Vec<String> = entries
            .iter()
            .filter(|e| e.action == PlanAction::Remove)
            .map(|e| e.name.clone())
            .collect();
        if !replaced.is_empty() {
            warnings.push(PlanWarning::Replacements { packages: replaced });
        }
    }
    let reboot: Vec<String> = entries
        .iter()
        .filter(|e| e.flags.contains(&UpdateFlag::RebootRecommended))
        .map(|e| e.name.clone())
        .collect();
    if !reboot.is_empty() {
        warnings.push(PlanWarning::RebootRecommended { packages: reboot });
    }

    TransactionPlan {
        kind,
        targets: targets.to_vec(),
        entries,
        download_size: download,
        install_size_delta: size_delta,
        source: PlanSource::SystemDb,
        computed_at: cachyos_center_core::now(),
        warnings,
        recursive,
        digest: String::new(),
    }
    .seal()
}

/// What to plan.
struct Spec<'a> {
    flags: TransFlag,
    kind: PlanKind,
    targets: &'a [String],
    recursive: bool,
}

/// Initializes a lock-free transaction, lets `setup` add targets, prepares it
/// and always releases it again.
fn plan_with(
    alpm: &mut Alpm,
    spec: Spec<'_>,
    config: &AlpmConfig,
    context: &ClassifyContext,
    setup: impl FnOnce(&Alpm) -> AppResult<()>,
) -> AppResult<TransactionPlan> {
    alpm.trans_init(spec.flags | TransFlag::NO_LOCK)
        .map_err(|e| alpm_err("cannot start planning transaction", e))?;
    let mut result = setup(alpm);
    if result.is_ok()
        && let Err(err) = alpm.trans_prepare()
    {
        result = Err(prepare_error(&err));
    }
    let plan = result.map(|()| {
        collect(
            alpm,
            spec.kind,
            spec.targets,
            spec.recursive,
            config,
            context,
        )
    });
    let released = alpm.trans_release();
    let plan = plan?;
    released.map_err(|e| alpm_err("cannot release planning transaction", e))?;
    Ok(plan)
}

pub fn upgrade(
    alpm: &mut Alpm,
    config: &AlpmConfig,
    context: &ClassifyContext,
) -> AppResult<TransactionPlan> {
    plan_with(
        alpm,
        Spec {
            flags: TransFlag::NONE,
            kind: PlanKind::SystemUpgrade,
            targets: &[],
            recursive: false,
        },
        config,
        context,
        |alpm| {
            alpm.sync_sysupgrade(false)
                .map_err(|e| alpm_err("cannot compute system upgrade", e))
        },
    )
}

pub fn install(
    alpm: &mut Alpm,
    config: &AlpmConfig,
    repository: &str,
    name: &str,
    context: &ClassifyContext,
) -> AppResult<TransactionPlan> {
    validate::repo_name(repository)?;
    validate::package_name(name)?;
    let targets = vec![name.to_string()];
    plan_with(
        alpm,
        Spec {
            flags: TransFlag::NEEDED,
            kind: PlanKind::Install,
            targets: &targets,
            recursive: false,
        },
        config,
        context,
        |alpm| {
            let db = alpm
                .syncdbs()
                .into_iter()
                .find(|db| db.name() == repository)
                .ok_or_else(|| {
                    AppError::not_found(format!("repository '{repository}' is not configured"))
                })?;
            let pkg = db.pkg(name).map_err(|_| {
                AppError::not_found(format!("package '{repository}/{name}' not found"))
            })?;
            match alpm.trans_add_pkg(pkg) {
                Ok(()) => {}
                // Already part of the transaction: nothing to add.
                Err(e) if e.error == alpm::Error::TransDupTarget => {}
                Err(e) => return Err(alpm_err("cannot add target", e.error)),
            }
            alpm.sync_sysupgrade(false)
                .map_err(|e| alpm_err("cannot compute system upgrade", e))
        },
    )
}

pub fn remove(
    alpm: &mut Alpm,
    config: &AlpmConfig,
    name: &str,
    recursive: bool,
    context: &ClassifyContext,
) -> AppResult<TransactionPlan> {
    validate::package_name(name)?;
    let targets = vec![name.to_string()];
    let flags = if recursive {
        TransFlag::RECURSE
    } else {
        TransFlag::NONE
    };
    plan_with(
        alpm,
        Spec {
            flags,
            kind: PlanKind::Remove,
            targets: &targets,
            recursive,
        },
        config,
        context,
        |alpm| {
            let pkg = alpm
                .localdb()
                .pkg(name)
                .map_err(|_| AppError::not_found(format!("package '{name}' is not installed")))?;
            alpm.trans_remove_pkg(pkg)
                .map_err(|e| alpm_err("cannot add removal target", e))
        },
    )
}
