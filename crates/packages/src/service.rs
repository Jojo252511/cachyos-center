//! Read-only package facade used by the GUI backend, the MCP server and the helper.

use std::path::{Path, PathBuf};
use std::time::Duration;

use cachyos_center_core::bridge::{AlpmConfig, BridgeRequest, ClassifyContext, UpdatesData};
use cachyos_center_core::package::{
    CatalogInstallFilter, CatalogQuery, InstallReason, InstalledFilter, InstalledQuery,
    PackageOrigin, PackagePage, PackageRecord, PackageRef, PackageSummary, RepositoryInfo,
};
use cachyos_center_core::plan::{PlanAction, PlanSource, PlanWarning, TransactionPlan};
use cachyos_center_core::system::{BackendStatus, LockStatus};
use cachyos_center_core::updates::{
    CheckStatus, INSTALL_MAX_DATA_AGE_SECS, UpdateCandidate, UpdateCheckResult, UpdateFlag,
    freshness,
};
use cachyos_center_core::package::PackageId;
use cachyos_center_core::{AppError, AppResult, ErrorCode, now, validate};

use crate::check::{self, CheckState};
use crate::pacman_log::{self, LogSummary};
use crate::{bridge, config, context, lock};

/// Package service. Cheap to construct; holds the resolved pacman configuration.
#[derive(Debug, Clone)]
pub struct PackageService {
    config: Result<AlpmConfig, AppError>,
    check_db: PathBuf,
    check_state: PathBuf,
    log_path: PathBuf,
    context: ClassifyContext,
}

/// Maximum page size of the installed list.
pub const MAX_PAGE: u32 = 500;

impl PackageService {
    /// Service for the current system configuration. `check_db` and
    /// `check_state` are the locations of the isolated update-check database
    /// and its metadata (per user, or `/var/lib/cachyos-center` for the timer).
    pub fn new(check_db: PathBuf, check_state: PathBuf) -> Self {
        Self {
            config: config::load(None),
            check_db,
            check_state,
            log_path: PathBuf::from(pacman_log::PACMAN_LOG),
            context: context::current(),
        }
    }

    /// Fully injected service (tests, sandboxes).
    pub fn with_parts(
        config: AlpmConfig,
        check_db: PathBuf,
        check_state: PathBuf,
        log_path: PathBuf,
        context: ClassifyContext,
    ) -> Self {
        Self {
            config: Ok(config),
            check_db,
            check_state,
            log_path,
            context,
        }
    }

    pub fn config(&self) -> AppResult<&AlpmConfig> {
        self.config.as_ref().map_err(Clone::clone)
    }

    pub fn context(&self) -> &ClassifyContext {
        &self.context
    }

    pub fn check_db(&self) -> &Path {
        &self.check_db
    }

    /// Configuration for read queries: the isolated database of the last
    /// update check when present (it is fresher), otherwise the system database.
    fn read_config(&self) -> AppResult<(AlpmConfig, PlanSource)> {
        let config = self.config()?;
        if check::has_synced_db(&self.check_db) && self.check_db.join("local").exists() {
            Ok((
                config::with_db_path(config, &self.check_db),
                PlanSource::IsolatedCheckDb,
            ))
        } else {
            Ok((config.clone(), PlanSource::SystemDb))
        }
    }

    pub fn backend_status(&self) -> BackendStatus {
        bridge::status()
    }

    pub fn lock_status(&self) -> LockStatus {
        match self.config() {
            Ok(c) => lock::status(&config::lock_file(c)),
            Err(_) => lock::status(Path::new("/var/lib/pacman/db.lck")),
        }
    }

    pub fn log_summary(&self) -> AppResult<LogSummary> {
        pacman_log::read_summary(&self.log_path)
            .map_err(|e| AppError::unavailable(format!("cannot read the pacman log: {e}")))
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    pub fn repositories(&self) -> AppResult<Vec<RepositoryInfo>> {
        let (config, _) = self.read_config()?;
        bridge::call(&BridgeRequest::Repositories { config })
    }

    /// All installed packages (unfiltered).
    pub fn installed_all(&self) -> AppResult<Vec<PackageSummary>> {
        let (config, _) = self.read_config()?;
        bridge::call(&BridgeRequest::ListInstalled { config })
    }

    /// Filtered and paginated list of installed packages.
    pub fn installed(&self, query: &InstalledQuery) -> AppResult<PackagePage> {
        let limit = if query.limit == 0 {
            100
        } else {
            validate::limit(Some(query.limit), 100, 1, MAX_PAGE)?
        };
        let needle = match &query.query {
            Some(q) => validate::search_query(q)?.to_lowercase(),
            None => String::new(),
        };
        let all = self.installed_all()?;
        let filtered: Vec<PackageSummary> = all
            .into_iter()
            .filter(|p| match query.filter {
                InstalledFilter::All => true,
                InstalledFilter::Explicit => p.install_reason == Some(InstallReason::Explicit),
                InstalledFilter::Dependency => p.install_reason == Some(InstallReason::Dependency),
                InstalledFilter::Repo => p.origin == PackageOrigin::Repo,
                InstalledFilter::LocalOrAur => p.origin == PackageOrigin::LocalOrAur,
                InstalledFilter::UpdateAvailable => p.update_available,
            })
            .filter(|p| {
                needle.is_empty()
                    || p.name.to_lowercase().contains(&needle)
                    || p.description.to_lowercase().contains(&needle)
            })
            .collect();
        let total = u32::try_from(filtered.len()).unwrap_or(u32::MAX);
        let offset = query.offset.min(total);
        let items: Vec<PackageSummary> = filtered
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        let end = offset + u32::try_from(items.len()).unwrap_or(0);
        Ok(PackagePage {
            items,
            total,
            offset,
            next_offset: (end < total).then_some(end),
        })
    }

    /// Search in the configured sync repositories.
    pub fn search(&self, query: &CatalogQuery) -> AppResult<Vec<PackageSummary>> {
        let text = validate::search_query(&query.query)?;
        let limit = validate::limit(Some(query.limit), 50, 1, 500)?;
        if let Some(repo) = &query.repository {
            validate::repo_name(repo)?;
        }
        if text.chars().count() < 2 && query.repository.is_none() {
            return Err(AppError::invalid(
                "the search query needs at least two characters",
            ));
        }
        let (config, _) = self.read_config()?;
        // Request more results when a status filter is applied afterwards.
        let fetch = if query.install_filter == CatalogInstallFilter::Any {
            limit
        } else {
            500
        };
        let results: Vec<PackageSummary> = bridge::call(&BridgeRequest::Search {
            config,
            query: text,
            repository: query.repository.clone(),
            limit: fetch,
        })?;
        Ok(results
            .into_iter()
            .filter(|p| match query.install_filter {
                CatalogInstallFilter::Any => true,
                CatalogInstallFilter::Installed => p.installed_version.is_some(),
                CatalogInstallFilter::NotInstalled => p.installed_version.is_none(),
            })
            .take(limit as usize)
            .collect())
    }

    pub fn details(&self, reference: &PackageRef) -> AppResult<PackageRecord> {
        validate::package_name(&reference.name)?;
        if let Some(repo) = &reference.repository {
            validate::repo_name(repo)?;
        }
        let (config, _) = self.read_config()?;
        bridge::call(&BridgeRequest::Details {
            config,
            name: reference.name.clone(),
            repository: reference.repository.clone(),
            context: self.context.clone(),
        })
    }

    /// Result of the last check, re-evaluated against the *current* local
    /// database. Never touches the network.
    pub fn last_check(&self) -> UpdateCheckResult {
        let state = CheckState::load(&self.check_state);
        let missing = check::missing_prerequisites();
        let Some(checked_at) = state.checked_at.filter(|_| check::has_synced_db(&self.check_db))
        else {
            let status = if !missing.is_empty() {
                CheckStatus::PrerequisiteMissing
            } else if state.last_attempt_failed() {
                CheckStatus::Failed
            } else {
                CheckStatus::NeverChecked
            };
            let mut r = UpdateCheckResult::empty(status);
            r.attempted_at = state.attempted_at;
            r.error = state.error;
            r.missing_prerequisites = missing;
            return r;
        };
        let config = match self.config() {
            Ok(c) => config::with_db_path(c, &self.check_db),
            Err(e) => {
                let mut r = UpdateCheckResult::empty(CheckStatus::Failed);
                r.error = Some(e);
                return r;
            }
        };
        let data: AppResult<UpdatesData> = bridge::call(&BridgeRequest::Updates {
            config,
            context: self.context.clone(),
        });
        let mut result = match data {
            Ok(data) => build_result(data),
            Err(e) => {
                let status = if e.code == ErrorCode::Unsupported {
                    CheckStatus::Unsupported
                } else {
                    CheckStatus::Failed
                };
                let mut r = UpdateCheckResult::empty(status);
                r.error = Some(e);
                r
            }
        };
        result.checked_at = Some(checked_at);
        result.attempted_at = state.attempted_at;
        result.missing_prerequisites = missing;
        if result.status == CheckStatus::Fresh {
            result.status = if state.last_attempt_failed() {
                result.error = state.error;
                CheckStatus::Failed
            } else {
                freshness(checked_at, now())
            };
        }
        result
    }

    /// Runs a new check (network) and returns the evaluated result.
    pub fn check_now(&self) -> UpdateCheckResult {
        self.check_now_with_timeout(check::CHECK_TIMEOUT)
    }

    pub fn check_now_with_timeout(&self, timeout: Duration) -> UpdateCheckResult {
        let mut state = CheckState::load(&self.check_state);
        let attempted = now();
        state.attempted_at = Some(attempted);
        match check::run_checkupdates(&self.check_db, timeout) {
            Ok(_) => {
                state.checked_at = Some(attempted);
                state.error = None;
            }
            Err(e) if e.code == ErrorCode::Busy => {
                // Another check is running; report the existing data.
                let mut r = self.last_check();
                r.error = Some(e);
                return r;
            }
            Err(e) => state.error = Some(e),
        }
        if let Err(e) = state.save(&self.check_state) {
            tracing::warn!("cannot store update check state: {e}");
        }
        self.last_check()
    }

    /// Install plan on the data of the last check. Requires repository data
    /// that is at most [`INSTALL_MAX_DATA_AGE_SECS`] old.
    pub fn plan_install(&self, repository: &str, name: &str) -> AppResult<TransactionPlan> {
        validate::repo_name(repository)?;
        validate::package_name(name)?;
        let state = CheckState::load(&self.check_state);
        let fresh = state
            .checked_at
            .is_some_and(|t| now().saturating_sub(t) <= INSTALL_MAX_DATA_AGE_SECS)
            && check::has_synced_db(&self.check_db);
        if !fresh {
            return Err(AppError::new(
                ErrorCode::Stale,
                "repository data is not current; run an update check first",
            ));
        }
        let config = config::with_db_path(self.config()?, &self.check_db);
        let mut plan: TransactionPlan = bridge::call(&BridgeRequest::PlanInstall {
            config,
            repository: repository.to_string(),
            name: name.to_string(),
            context: self.context.clone(),
        })?;
        plan.source = PlanSource::IsolatedCheckDb;
        self.add_held_back_warning(&mut plan);
        Ok(plan)
    }

    /// Removal plan (local database only).
    pub fn plan_remove(&self, name: &str, recursive: bool) -> AppResult<TransactionPlan> {
        validate::package_name(name)?;
        let (config, source) = self.read_config()?;
        let mut plan: TransactionPlan = bridge::call(&BridgeRequest::PlanRemove {
            config,
            name: name.to_string(),
            recursive,
            context: self.context.clone(),
        })?;
        plan.source = source;
        Ok(plan)
    }

    fn add_held_back_warning(&self, plan: &mut TransactionPlan) {
        let held: Vec<String> = self
            .last_check()
            .held_back
            .into_iter()
            .map(|u| u.package_id.name)
            .collect();
        if !held.is_empty() {
            plan.warnings.push(PlanWarning::HeldBackPackages { packages: held });
        }
    }

    // ---- Plans on the productive database (used by the privileged helper) ----

    /// Upgrade plan on the productive sync database.
    pub fn system_plan_upgrade(&self) -> AppResult<TransactionPlan> {
        let data: UpdatesData = bridge::call(&BridgeRequest::Updates {
            config: self.config()?.clone(),
            context: self.context.clone(),
        })?;
        Ok(data.plan)
    }

    pub fn system_plan_install(&self, repository: &str, name: &str) -> AppResult<TransactionPlan> {
        bridge::call(&BridgeRequest::PlanInstall {
            config: self.config()?.clone(),
            repository: repository.to_string(),
            name: name.to_string(),
            context: self.context.clone(),
        })
    }

    pub fn system_plan_remove(&self, name: &str, recursive: bool) -> AppResult<TransactionPlan> {
        bridge::call(&BridgeRequest::PlanRemove {
            config: self.config()?.clone(),
            name: name.to_string(),
            recursive,
            context: self.context.clone(),
        })
    }

    /// Details from the productive database (helper validation of targets).
    pub fn system_details(&self, name: &str, repository: Option<&str>) -> AppResult<PackageRecord> {
        bridge::call(&BridgeRequest::Details {
            config: self.config()?.clone(),
            name: name.to_string(),
            repository: repository.map(str::to_string),
            context: self.context.clone(),
        })
    }
}

fn build_result(data: UpdatesData) -> UpdateCheckResult {
    let mut plan = data.plan;
    plan.source = PlanSource::IsolatedCheckDb;
    let updates: Vec<UpdateCandidate> = plan
        .entries
        .iter()
        .filter(|e| e.action != PlanAction::Remove)
        .map(|e| UpdateCandidate {
            package_id: PackageId {
                repository: e.repository.clone().unwrap_or_default(),
                name: e.name.clone(),
                architecture: String::new(),
            },
            old_version: e.old_version.clone().unwrap_or_default(),
            new_version: e.new_version.clone().unwrap_or_default(),
            download_size: e.download_size,
            flags: e.flags.clone(),
        })
        .collect();
    if !data.held_back.is_empty() {
        plan.warnings.push(PlanWarning::HeldBackPackages {
            packages: data
                .held_back
                .iter()
                .map(|u| u.package_id.name.clone())
                .collect(),
        });
    }
    let reboot = updates
        .iter()
        .any(|u| u.flags.contains(&UpdateFlag::RebootRecommended));
    UpdateCheckResult {
        status: CheckStatus::Fresh,
        checked_at: None,
        attempted_at: None,
        total_download_size: Some(plan.download_size),
        updates,
        plan: Some(plan),
        held_back: data.held_back,
        reboot_recommended: reboot,
        error: None,
        missing_prerequisites: Vec::new(),
    }
}
