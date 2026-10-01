//! Operation engine: serial execution of package operations.
//!
//! Upgrade flow (installation analogous with `--needed repo/pkg`):
//! 1. polkit authorization of the actual bus sender (`AwaitingAuthorization`)
//! 2. preconditions: no prepared offline update, bounded wait for a foreign
//!    `db.lck` (never removed), optional snapper snapshot (`Preparing`)
//! 3. `pacman -Sy`, recompute the plan with libalpm (NOLOCK) and compare its
//!    digest with the confirmed preview – stop on any deviation
//! 4. `pacman -Suw` downloads and verifies signatures without changing
//!    anything (`Downloading`, still cancellable)
//! 5. re-verify the plan, then `pacman -Su` commits (`Installing`, not cancellable)
//! 6. the result is taken from the pacman log (authoritative), not from the
//!    exit code alone

use std::path::Path;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use cachyos_center_core::dbus::actions;
use cachyos_center_core::history::LogOutcome;
use cachyos_center_core::operation::{
    ChangeCounts, Operation, OperationKind, OperationOrigin, OperationState, OperationStep,
    SnapshotResult,
};
use cachyos_center_core::package::PackageOrigin;
use cachyos_center_core::plan::TransactionPlan;
use cachyos_center_core::ui::OperationLogChunk;
use cachyos_center_core::{AppError, AppResult, ErrorCode, now, validate};
use cachyos_center_packages::PackageService;
use cachyos_center_packages::pacman_log::{self, LogEvent};
use zbus::Connection;

use crate::authz::{self, Caller};
use crate::config::HelperConfig;
use crate::journal::{Journal, RecoveryState};
use crate::runner::{self, CancelFlag, Step, StepOutcome};

/// Maximum bytes returned by one log read.
const LOG_CHUNK: u64 = 64 * 1024;

/// A requested package operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Upgrade {
        digest: String,
        snapshot: bool,
    },
    Install {
        repository: String,
        name: String,
        digest: String,
    },
    Remove {
        name: String,
        recursive: bool,
        digest: String,
    },
}

impl Request {
    pub fn kind(&self) -> OperationKind {
        match self {
            Self::Upgrade { .. } => OperationKind::SystemUpgrade,
            Self::Install { .. } => OperationKind::Install,
            Self::Remove { .. } => OperationKind::Remove,
        }
    }

    pub fn action(&self) -> &'static str {
        match self {
            Self::Upgrade { .. } => actions::UPGRADE,
            Self::Install { .. } => actions::INSTALL,
            Self::Remove { .. } => actions::REMOVE,
        }
    }

    fn digest(&self) -> &str {
        match self {
            Self::Upgrade { digest, .. }
            | Self::Install { digest, .. }
            | Self::Remove { digest, .. } => digest,
        }
    }

    /// Validates all fields before anything else happens.
    pub fn validate(&self) -> AppResult<()> {
        validate::plan_digest(self.digest())?;
        match self {
            Self::Upgrade { .. } => Ok(()),
            Self::Install {
                repository, name, ..
            } => {
                validate::repo_name(repository)?;
                validate::package_name(name)
            }
            Self::Remove { name, .. } => validate::package_name(name),
        }
    }

    fn targets(&self) -> Vec<String> {
        match self {
            Self::Upgrade { .. } => Vec::new(),
            Self::Install {
                repository, name, ..
            } => vec![format!("{repository}/{name}")],
            Self::Remove { name, .. } => vec![name.clone()],
        }
    }
}

/// Why a flow stopped early.
#[derive(Debug)]
enum Stop {
    Error(AppError),
    Cancelled,
    /// Already recorded in the operation (plan changed, nothing to do).
    Done,
}

impl From<AppError> for Stop {
    fn from(e: AppError) -> Self {
        Stop::Error(e)
    }
}

struct Active {
    op: Operation,
    cancel: CancelFlag,
    initiator_uid: Option<u32>,
}

/// The engine. Shared between the D-Bus interface and the flow tasks.
pub struct Engine {
    config: HelperConfig,
    journal: Journal,
    packages: PackageService,
    active: Arc<Mutex<Option<Active>>>,
    last_activity: AtomicI64,
    bus: OnceLock<Connection>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("config", &self.config)
            .finish()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Engine {
    pub fn new(config: HelperConfig, packages: PackageService) -> Arc<Self> {
        Arc::new(Self {
            journal: Journal::new(config.journal_dir()),
            config,
            packages,
            active: Arc::new(Mutex::new(None)),
            last_activity: AtomicI64::new(now()),
            bus: OnceLock::new(),
        })
    }

    pub fn config(&self) -> &HelperConfig {
        &self.config
    }

    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    /// Connection used for polkit (set once the bus connection exists).
    pub fn set_bus(&self, conn: Connection) {
        let _ = self.bus.set(conn);
    }

    pub fn touch(&self) {
        self.last_activity.store(now(), Ordering::Relaxed);
    }

    /// Idle: no active operation and no call for `idle_timeout`.
    pub fn is_idle(&self) -> bool {
        let busy = lock(&self.active)
            .as_ref()
            .is_some_and(|a| !a.op.state.is_terminal());
        let idle_for = now().saturating_sub(self.last_activity.load(Ordering::Relaxed));
        !busy && idle_for >= i64::try_from(self.config.idle_timeout.as_secs()).unwrap_or(600)
    }

    // ---- state handling ----------------------------------------------------

    fn with_op<R>(&self, id: &str, f: impl FnOnce(&mut Operation) -> R) -> Option<R> {
        let mut guard = lock(&self.active);
        let active = guard.as_mut().filter(|a| a.op.id == id)?;
        let r = f(&mut active.op);
        if let Err(e) = self.journal.save(&active.op) {
            tracing::error!("cannot write journal: {e}");
        }
        Some(r)
    }

    fn transition(&self, id: &str, next: OperationState) -> AppResult<()> {
        self.with_op(id, |op| op.transition(next, now()))
            .unwrap_or_else(|| Err(AppError::internal("operation is not active")))
    }

    fn cancel_flag(&self, id: &str) -> Option<CancelFlag> {
        lock(&self.active)
            .as_ref()
            .filter(|a| a.op.id == id)
            .map(|a| a.cancel.clone())
    }

    fn check_cancel(&self, id: &str) -> Result<(), Stop> {
        if self.cancel_flag(id).is_some_and(|c| c.is_cancelled()) {
            Err(Stop::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Current state of an operation (active or from the journal).
    pub fn status(&self, id: &str) -> AppResult<Operation> {
        validate::operation_id(id)?;
        if let Some(op) = lock(&self.active)
            .as_ref()
            .filter(|a| a.op.id == id)
            .map(|a| a.op.clone())
        {
            return Ok(op);
        }
        self.journal
            .load(id)
            .ok_or_else(|| AppError::not_found("unknown operation"))
    }

    /// The running operation, or the most recent one.
    pub fn current(&self) -> Option<Operation> {
        if let Some(active) = lock(&self.active).as_ref() {
            return Some(active.op.clone());
        }
        self.journal.list().into_iter().next()
    }

    /// Reads the operation log from `offset`.
    pub fn read_log(&self, id: &str, offset: u64) -> AppResult<OperationLogChunk> {
        let op = self.status(id)?;
        let path = self.config.operation_log(id);
        let complete = op.state.is_terminal();
        let Ok(meta) = std::fs::metadata(&path) else {
            return Ok(OperationLogChunk {
                lines: Vec::new(),
                next_offset: 0,
                complete,
            });
        };
        let len = meta.len();
        if offset >= len {
            return Ok(OperationLogChunk {
                lines: Vec::new(),
                next_offset: len,
                complete,
            });
        }
        use std::io::{Read, Seek, SeekFrom};
        let mut file = std::fs::File::open(&path)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = Vec::new();
        file.take(LOG_CHUNK).read_to_end(&mut buf)?;
        // Only complete lines, unless the chunk limit was hit.
        let end = if buf.len() as u64 == LOG_CHUNK {
            buf.len()
        } else {
            buf.iter().rposition(|b| *b == b'\n').map_or(0, |p| p + 1)
        };
        buf.truncate(end);
        let text = String::from_utf8_lossy(&buf);
        Ok(OperationLogChunk {
            lines: text
                .lines()
                .map(|l| l.chars().take(2000).collect())
                .collect(),
            next_offset: offset + end as u64,
            complete: complete && offset + (end as u64) >= len,
        })
    }

    // ---- starting and cancelling -------------------------------------------

    /// Registers a new operation and starts its flow. Returns the id.
    pub fn start(self: &Arc<Self>, request: Request, caller: Caller) -> AppResult<String> {
        request.validate()?;
        self.touch();
        let id = new_id();
        {
            let mut guard = lock(&self.active);
            if let Some(active) = guard.as_ref()
                && !active.op.state.is_terminal()
            {
                return Err(AppError::busy(
                    "another cachyos-center operation is running",
                ));
            }
            let mut op = Operation::new(id.clone(), request.kind(), OperationOrigin::User, now());
            op.package_targets = request.targets();
            op.confirmed_digest = Some(request.digest().to_string());
            op.transition(OperationState::AwaitingAuthorization, now())?;
            op.summary = "waiting for authorization".into();
            self.journal.save(&op)?;
            *guard = Some(Active {
                op,
                cancel: CancelFlag::default(),
                initiator_uid: caller.uid,
            });
        }
        let _ = self.journal.save_recovery(
            &id,
            &RecoveryState {
                initiator_uid: caller.uid,
                ..RecoveryState::default()
            },
        );
        let engine = Arc::clone(self);
        let run_id = id.clone();
        tokio::spawn(async move {
            engine.run(run_id, request, caller).await;
        });
        Ok(id)
    }

    /// Requests cancellation. Only possible before the commit phase.
    pub async fn cancel(&self, id: &str, caller: &Caller) -> AppResult<Operation> {
        validate::operation_id(id)?;
        self.touch();
        let (state, flag, uid, kind) = {
            let guard = lock(&self.active);
            let active = guard
                .as_ref()
                .filter(|a| a.op.id == id)
                .ok_or_else(|| AppError::not_found("operation is not running"))?;
            (
                active.op.state,
                active.cancel.clone(),
                active.initiator_uid,
                active.op.kind,
            )
        };
        if !state.can_cancel() {
            return Err(AppError::invalid(
                "the operation can no longer be cancelled (commit phase or finished)",
            ));
        }
        // Every call is checked with polkit for the actual bus sender. The
        // initiator cancels with the dedicated action (active local sessions,
        // no password: cancelling never changes the system); anybody else
        // needs the authorization of the operation itself.
        let same_user = caller.uid.is_some() && caller.uid == uid;
        let action = if same_user {
            actions::CANCEL
        } else {
            match kind {
                OperationKind::Install => actions::INSTALL,
                OperationKind::Remove => actions::REMOVE,
                _ => actions::UPGRADE,
            }
        };
        let conn = self
            .bus
            .get()
            .ok_or_else(|| AppError::unavailable("no bus connection"))?;
        authz::authorize(
            &self.config.auth,
            conn,
            caller,
            action,
            &format!("cancel-{id}"),
            self.config.auth_timeout,
        )
        .await?;
        flag.cancel();
        if state == OperationState::AwaitingAuthorization
            && let Some(conn) = self.bus.get()
        {
            authz::cancel(&self.config.auth, conn, id).await;
        }
        self.status(id)
    }

    // ---- flow ----------------------------------------------------------------

    async fn run(self: Arc<Self>, id: String, request: Request, caller: Caller) {
        let result = self.flow(&id, &request, &caller).await;
        match result {
            Ok(()) | Err(Stop::Done) => {}
            Err(Stop::Cancelled) => {
                self.with_op(&id, |op| {
                    if !op.state.is_terminal() {
                        op.summary = "cancelled before the commit; no package was changed".into();
                        let _ = op.transition(OperationState::CancelledBeforeCommit, now());
                    }
                });
            }
            Err(Stop::Error(err)) => {
                tracing::warn!(operation = %id, "operation failed: {err}");
                self.with_op(&id, |op| {
                    if !op.state.is_terminal() {
                        let _ = op.fail(err, now());
                    }
                });
            }
        }
        self.touch();
        // Keep the finished operation visible as "current" but release the slot.
        self.journal.prune(&self.config.log_dir);
    }

    async fn flow(&self, id: &str, request: &Request, caller: &Caller) -> Result<(), Stop> {
        // 1. Authorization of the actual bus sender.
        let conn = self
            .bus
            .get()
            .cloned()
            .ok_or_else(|| AppError::unavailable("no bus connection"))?;
        let auth = authz::authorize(
            &self.config.auth,
            &conn,
            caller,
            request.action(),
            id,
            self.config.auth_timeout,
        )
        .await;
        self.check_cancel(id)?;
        auth?;
        self.transition(id, OperationState::Preparing)?;
        self.with_op(id, |op| op.summary = "preparing".into());

        // 2. Preconditions.
        let offline = cachyos_center_system::updaters::offline_status();
        if offline.prepared && self.config.is_production() {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "an offline update is prepared for the next reboot; online package actions are locked until it has been installed or aborted",
            )
            .into());
        }
        if self.config.is_production()
            && cachyos_center_system::units::show(
                cachyos_center_system::units::Scope::System,
                cachyos_center_core::dbus::PREFLIGHT_SERVICE,
            )
            .is_some_and(|s| s.active())
        {
            return Err(AppError::busy("the scheduled update preparation is running").into());
        }
        let config = self.packages.config().map_err(Stop::Error)?.clone();
        if let Request::Install { repository, .. } = request
            && !config.repos.iter().any(|r| &r.name == repository)
        {
            return Err(AppError::invalid("repository is not configured").into());
        }
        if let Request::Remove { name, .. } = request {
            let packages = self.packages.clone();
            let n = name.clone();
            let record = blocking(move || packages.system_details(&n, None)).await?;
            if record.installed_version.is_none() {
                return Err(AppError::not_found("package is not installed").into());
            }
            if record.origin != PackageOrigin::Repo {
                return Err(AppError::invalid(
                    "only packages from configured repositories can be removed in this version",
                )
                .into());
            }
            if config.hold_pkgs.iter().any(|h| h == name) {
                return Err(
                    AppError::new(ErrorCode::Blocked, "package is listed in HoldPkg").into(),
                );
            }
        }
        self.wait_for_lock(id, &config.db_path).await?;

        // Snapshot (upgrade only, explicit request).
        if let Request::Upgrade { snapshot: true, .. } = request {
            let result = self.snapshot().await;
            let created = result.created;
            let error = result.error.clone();
            self.with_op(id, |op| op.snapshot = Some(result));
            if !created {
                return Err(AppError::new(
                    ErrorCode::Blocked,
                    format!(
                        "the requested snapshot could not be created: {}",
                        error.unwrap_or_default()
                    ),
                )
                .into());
            }
        }

        // 3. Refresh and verify the plan.
        if !matches!(request, Request::Remove { .. }) {
            self.with_op(id, |op| {
                op.progress.step = Some(OperationStep::SynchronizingDatabases);
            });
            let outcome = self.step(id, Step::Refresh).await?;
            self.check_cancel(id)?;
            if !outcome.success() {
                return Err(self
                    .step_error(
                        id,
                        "synchronizing the package databases failed; no package was changed",
                    )
                    .into());
            }
        }
        let plan = self.verify_plan(id, request).await?;
        if plan.entries.is_empty() {
            self.with_op(id, |op| {
                op.summary = "nothing to do".into();
                let _ = op.transition(OperationState::Succeeded, now());
            });
            return Err(Stop::Done);
        }
        let total = u32::try_from(plan.entries.len()).unwrap_or(u32::MAX);
        self.with_op(id, |op| op.progress.packages_total = Some(total));

        // 4. Download and verification (no changes yet).
        let commit_step = match request {
            Request::Upgrade { .. } => {
                self.download(id, Step::DownloadUpgrade).await?;
                Step::CommitUpgrade
            }
            Request::Install {
                repository, name, ..
            } => {
                self.download(
                    id,
                    Step::DownloadInstall {
                        repository: repository.clone(),
                        name: name.clone(),
                    },
                )
                .await?;
                Step::CommitInstall {
                    repository: repository.clone(),
                    name: name.clone(),
                }
            }
            Request::Remove {
                name, recursive, ..
            } => Step::Remove {
                name: name.clone(),
                recursive: *recursive,
            },
        };
        if !matches!(request, Request::Remove { .. }) {
            // 5. Re-verify immediately before the commit.
            let again = self.compute_plan(request).await?;
            if again.digest != plan.digest {
                return Err(self.plan_changed(id, again));
            }
        }
        self.check_cancel(id)?;

        // 6. Commit.
        self.commit(id, commit_step).await
    }

    async fn download(&self, id: &str, step: Step) -> Result<(), Stop> {
        self.transition(id, OperationState::Downloading)?;
        self.with_op(id, |op| {
            op.summary = "downloading and verifying packages".into();
            op.progress.step = Some(OperationStep::DownloadingPackages);
        });
        let outcome = self.step(id, step).await?;
        self.check_cancel(id)?;
        if !outcome.success() {
            return Err(self
                .step_error(
                    id,
                    "downloading or verifying the packages failed (mirror, network or signature problem); no package was changed",
                )
                .into());
        }
        Ok(())
    }

    async fn commit(&self, id: &str, step: Step) -> Result<(), Stop> {
        let offset = pacman_log::size(&self.config.pacman_log);
        let unit = runner::unit_name(id, &step);
        let mut recovery = self.journal.load_recovery(id);
        recovery.pacman_log_offset = Some(offset);
        recovery.unit = Some(unit);
        let _ = self.journal.save_recovery(id, &recovery);
        self.transition(id, OperationState::Installing)?;
        self.with_op(id, |op| {
            op.summary = "installing; interrupting may be dangerous".into();
            op.progress.step = Some(OperationStep::ApplyingChanges);
        });

        let inhibitor = crate::inhibit::acquire().await;
        let watcher = self.spawn_progress_watcher(id.to_string(), offset);
        let outcome = self.step(id, step).await;
        watcher.abort();
        drop(inhibitor);
        let outcome = outcome?;
        self.finish_from_log(id, offset, Some(&outcome));
        Ok(())
    }

    /// Evaluates the pacman log written since `offset` and ends the operation.
    pub fn finish_from_log(&self, id: &str, offset: u64, outcome: Option<&StepOutcome>) {
        let text = pacman_log::read_from(&self.config.pacman_log, offset)
            .map(|(t, _)| t)
            .unwrap_or_default();
        let events = pacman_log::parse_events(&text);
        let summary = pacman_log::summarize(&events);
        let started = events
            .iter()
            .any(|e| e.event == LogEvent::TransactionStarted);
        let mut changes = ChangeCounts::default();
        let mut pacnew = 0;
        for tx in &summary.transactions {
            changes.installed += tx.installed;
            changes.upgraded += tx.upgraded;
            changes.downgraded += tx.downgraded;
            changes.reinstalled += tx.reinstalled;
            changes.removed += tx.removed;
            pacnew += tx.config_files;
            for p in &tx.packages {
                if changes.packages.len() < 20 {
                    changes.packages.push(p.clone());
                }
            }
        }
        let log_outcome = summary.transactions.last().map(|t| t.outcome);
        // Without an exit code (reconstruction after a restart) the log alone decides.
        let exit_ok = outcome.is_none_or(StepOutcome::success);
        self.with_op(id, |op| {
            op.changes = changes;
            op.new_pacnew_files = pacnew;
            op.exit_code = outcome.and_then(|o| o.exit_code);
            op.progress.current_package = None;
            op.progress.step = None;
            let t = now();
            match (log_outcome, exit_ok) {
                (Some(LogOutcome::Completed), true) => {
                    op.summary = "completed".into();
                    let _ = op.transition(OperationState::Succeeded, t);
                }
                (Some(LogOutcome::Completed), false) => {
                    op.summary = "the transaction completed but pacman reported an error (e.g. a hook failed); please check".into();
                    op.error = Some(AppError::new(ErrorCode::TransactionFailed, op.summary.clone()));
                    let _ = op.transition(OperationState::NeedsAttention, t);
                }
                (None, _) if !started => {
                    // pacman stopped before the transaction started: nothing changed.
                    op.commit_started = false;
                    op.summary = "pacman stopped before the transaction started; no package was changed".into();
                    op.error = Some(AppError::new(ErrorCode::TransactionFailed, op.summary.clone()));
                    let _ = op.transition(OperationState::Failed, t);
                }
                (other, _) => {
                    op.outcome_unknown = matches!(other, Some(LogOutcome::Unknown) | None);
                    op.summary = match other {
                        Some(LogOutcome::Failed) => "the transaction failed; the package state must be checked",
                        Some(LogOutcome::Interrupted) => "the transaction was interrupted; the package state must be checked",
                        _ => "the result of the transaction is unknown; the package state must be checked",
                    }
                    .into();
                    op.error = Some(AppError::new(ErrorCode::TransactionFailed, op.summary.clone()));
                    let _ = op.transition(OperationState::NeedsAttention, t);
                }
            }
        });
    }

    fn spawn_progress_watcher(&self, id: String, offset: u64) -> tokio::task::JoinHandle<()> {
        let log = self.config.pacman_log.clone();
        let journal = self.journal.clone();
        let shared = Arc::clone(&self.active);
        tokio::spawn(async move {
            let mut pos = offset;
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let Ok((text, next)) = pacman_log::read_from(&log, pos) else {
                    continue;
                };
                pos = next;
                for ev in pacman_log::parse_events(&text) {
                    let pkg = match ev.event {
                        LogEvent::Installed(n, _)
                        | LogEvent::Upgraded(n, _, _)
                        | LogEvent::Downgraded(n, _, _)
                        | LogEvent::Reinstalled(n, _)
                        | LogEvent::Removed(n, _) => n,
                        _ => continue,
                    };
                    let mut guard = lock(&shared);
                    if let Some(active) = guard.as_mut().filter(|a| a.op.id == id) {
                        active.op.progress.packages_done += 1;
                        active.op.progress.current_package = Some(pkg);
                        let _ = journal.save(&active.op);
                    }
                }
            }
        })
    }

    async fn step(&self, id: &str, step: Step) -> Result<StepOutcome, Stop> {
        let config = self.config.clone();
        let cancel = self.cancel_flag(id).unwrap_or_default();
        let log = self.config.operation_log(id);
        let op_id = id.to_string();
        blocking(move || runner::run_step(&config, &op_id, &step, &log, &cancel))
            .await
            .map_err(Stop::Error)
    }

    fn step_error(&self, id: &str, message: &str) -> AppError {
        let tail = last_error_lines(&self.config.operation_log(id));
        let err = AppError::new(ErrorCode::TransactionFailed, message);
        if tail.is_empty() {
            err
        } else {
            err.with_detail(tail)
        }
    }

    async fn compute_plan(&self, request: &Request) -> Result<TransactionPlan, Stop> {
        let packages = self.packages.clone();
        let request = request.clone();
        blocking(move || match &request {
            Request::Upgrade { .. } => packages.system_plan_upgrade(),
            Request::Install {
                repository, name, ..
            } => packages.system_plan_install(repository, name),
            Request::Remove {
                name, recursive, ..
            } => packages.system_plan_remove(name, *recursive),
        })
        .await
        .map_err(Stop::Error)
    }

    async fn verify_plan(&self, id: &str, request: &Request) -> Result<TransactionPlan, Stop> {
        let plan = self.compute_plan(request).await?;
        if plan.digest != request.digest() {
            return Err(self.plan_changed(id, plan));
        }
        Ok(plan)
    }

    /// Records a plan deviation and ends the operation before the commit.
    fn plan_changed(&self, id: &str, actual: TransactionPlan) -> Stop {
        self.with_op(id, |op| {
            op.actual_plan = Some(actual);
            op.error = Some(AppError::new(
                ErrorCode::PlanChanged,
                "the actual pacman plan differs from the confirmed preview; please confirm again",
            ));
            op.summary = "stopped before the commit: the plan has changed".into();
            let _ = op.transition(OperationState::CancelledBeforeCommit, now());
        });
        Stop::Done
    }

    async fn wait_for_lock(&self, id: &str, db_path: &str) -> Result<(), Stop> {
        let lock_file = self.config.db_lock(db_path);
        let deadline = std::time::Instant::now() + self.config.lock_wait;
        let mut delay = Duration::from_secs(2);
        while lock_file.exists() {
            self.check_cancel(id)?;
            if std::time::Instant::now() >= deadline {
                return Err(AppError::busy(
                    "the package database is locked by another package manager; nothing was started (the lock is never removed automatically)",
                )
                .with_detail(lock_file.display().to_string())
                .into());
            }
            self.with_op(id, |op| {
                op.progress.step = Some(OperationStep::WaitingForLock)
            });
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(Duration::from_secs(15));
        }
        Ok(())
    }

    async fn snapshot(&self) -> SnapshotResult {
        let support = cachyos_center_system::health::snapshot_support();
        let snapper = self.config.snapper.clone();
        let config_name = support.root_config.clone().unwrap_or_else(|| "root".into());
        if !support.can_request_snapshot {
            return SnapshotResult {
                created: false,
                snapper_config: config_name,
                number: None,
                error: Some("snapper with a root configuration on Btrfs is not set up".into()),
            };
        }
        let name = config_name.clone();
        blocking(move || Ok(crate::snapshot::create(&snapper, &name)))
            .await
            .unwrap_or_else(|e| SnapshotResult {
                created: false,
                snapper_config: config_name,
                number: None,
                error: Some(e.message),
            })
    }

    // ---- recovery --------------------------------------------------------------

    /// Reconstructs operations that were active when the helper stopped.
    pub fn recover(self: &Arc<Self>) {
        for op in self.journal.list() {
            if op.state.is_terminal() {
                continue;
            }
            let recovery = self.journal.load_recovery(&op.id);
            tracing::warn!(operation = %op.id, state = ?op.state, "recovering interrupted operation");
            {
                let mut guard = lock(&self.active);
                *guard = Some(Active {
                    op: op.clone(),
                    cancel: CancelFlag::default(),
                    initiator_uid: recovery.initiator_uid,
                });
            }
            let still_running = self.config.is_production()
                && recovery.unit.as_deref().is_some_and(runner::unit_active);
            if still_running {
                let engine = Arc::clone(self);
                let id = op.id.clone();
                let unit = recovery.unit.clone().unwrap_or_default();
                let offset = recovery.pacman_log_offset.unwrap_or(0);
                tokio::spawn(async move {
                    while runner::unit_active(&unit) {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    engine.finish_from_log(&id, offset, None);
                });
                // Only one operation can be active.
                break;
            }
            if op.commit_started || op.state == OperationState::Installing {
                self.finish_from_log(&op.id, recovery.pacman_log_offset.unwrap_or(0), None);
                self.with_op(&op.id, |o| {
                    if o.state == OperationState::Succeeded {
                        o.summary = "completed (reconstructed from the pacman log after a restart of the helper)".into();
                    }
                });
            } else {
                self.with_op(&op.id, |o| {
                    let _ = o.fail(
                        AppError::new(
                            ErrorCode::Unavailable,
                            "the helper was restarted before the commit; no package was changed",
                        ),
                        now(),
                    );
                });
            }
        }
        let mut guard = lock(&self.active);
        if guard.as_ref().is_some_and(|a| a.op.state.is_terminal()) {
            *guard = None;
        }
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::internal(format!("worker failed: {e}")))?
}

/// Last error lines of a log (for error details).
pub fn last_error_lines(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("error:") || l.contains("error:") || l.starts_with("warning:"))
        .collect();
    lines[lines.len().saturating_sub(8)..].join("\n")
}

/// New operation id (UUID v4).
pub fn new_id() -> String {
    let mut bytes = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut bytes);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}
