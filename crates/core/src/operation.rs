//! Operations and their state machine.
//!
//! ```text
//! Idle ─► Checking ─► Ready ─► AwaitingAuthorization ─► Preparing ─► Downloading ─► Installing ─► Succeeded
//!   │        │          │               │                  │             │              ├──────► Failed
//!   │        ▼          ▼               ▼                  ▼             ▼              └──────► NeedsAttention
//!   │   Failed/…   Cancelled…   Failed/Cancelled…   Failed/NeedsAttention/CancelledBeforeCommit
//!   └────────────► AwaitingAuthorization / Preparing (timer)
//! ```
//!
//! Once `Installing` has been entered the pacman commit phase may have started.
//! From there on an operation can no longer be cancelled and a failure means the
//! package state is unclear until it has been re-read.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Timestamp;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::plan::TransactionPlan;

/// What an operation does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OperationKind {
    /// Update check with the isolated sync database.
    UpdateCheck,
    /// Full system upgrade (`pacman -Syu`).
    SystemUpgrade,
    /// Install a repository package (`pacman -Syu repo/pkg`).
    Install,
    /// Remove a package (`pacman -R`/`-Rs`).
    Remove,
    /// Preflight and `pacman-offline` preparation started by the timer.
    AutoUpdatePrepare,
}

/// Who started an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OperationOrigin {
    User,
    Timer,
}

/// State of an operation (see module documentation for the transitions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OperationState {
    Idle,
    Checking,
    Ready,
    AwaitingAuthorization,
    Preparing,
    Downloading,
    Installing,
    Succeeded,
    Failed,
    NeedsAttention,
    CancelledBeforeCommit,
}

impl OperationState {
    pub const ALL: [OperationState; 11] = [
        Self::Idle,
        Self::Checking,
        Self::Ready,
        Self::AwaitingAuthorization,
        Self::Preparing,
        Self::Downloading,
        Self::Installing,
        Self::Succeeded,
        Self::Failed,
        Self::NeedsAttention,
        Self::CancelledBeforeCommit,
    ];

    /// Final states; no further transition is allowed.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::NeedsAttention | Self::CancelledBeforeCommit
        )
    }

    /// States in which work is in progress.
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Checking
                | Self::AwaitingAuthorization
                | Self::Preparing
                | Self::Downloading
                | Self::Installing
        )
    }

    /// Cancelling is only offered before the pacman commit phase.
    pub fn can_cancel(self) -> bool {
        matches!(
            self,
            Self::AwaitingAuthorization | Self::Preparing | Self::Downloading
        )
    }

    /// Whether the transition `self -> next` is allowed.
    pub fn can_transition_to(self, next: OperationState) -> bool {
        use OperationState::*;
        match self {
            Idle => matches!(next, Checking | AwaitingAuthorization | Preparing),
            Checking => matches!(next, Ready | Succeeded | Failed | NeedsAttention),
            Ready => matches!(
                next,
                AwaitingAuthorization | Preparing | Succeeded | CancelledBeforeCommit
            ),
            AwaitingAuthorization => matches!(next, Preparing | Failed | CancelledBeforeCommit),
            Preparing => matches!(
                next,
                Downloading
                    | Installing
                    | Succeeded
                    | Failed
                    | NeedsAttention
                    | CancelledBeforeCommit
            ),
            Downloading => matches!(
                next,
                Installing | Succeeded | Failed | NeedsAttention | CancelledBeforeCommit
            ),
            Installing => matches!(next, Succeeded | Failed | NeedsAttention),
            Succeeded | Failed | NeedsAttention | CancelledBeforeCommit => false,
        }
    }
}

/// Coarse progress information. No overall percentage is estimated: only
/// values backed by pacman output are reported.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Progress {
    /// Package pacman is currently working on (from the pacman log).
    pub current_package: Option<String>,
    /// Number of packages pacman has finished in the commit phase.
    pub packages_done: u32,
    /// Number of packages in the verified plan.
    pub packages_total: Option<u32>,
    /// Short phase text from pacman (e.g. "checking keyring"), English.
    pub phase_detail: Option<String>,
}

/// Package changes of an operation, counted from the pacman log (authoritative).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangeCounts {
    pub installed: u32,
    pub upgraded: u32,
    pub downgraded: u32,
    pub reinstalled: u32,
    pub removed: u32,
    /// Affected package names (at most 20).
    pub packages: Vec<String>,
}

impl ChangeCounts {
    pub fn total(&self) -> u32 {
        self.installed + self.upgraded + self.downgraded + self.reinstalled + self.removed
    }
}

/// Result of an explicitly requested snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SnapshotResult {
    /// `true` only when snapper confirmed the snapshot number.
    pub created: bool,
    pub snapper_config: String,
    pub number: Option<u32>,
    pub error: Option<String>,
}

/// A package operation tracked by the helper, the timer or the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Operation {
    pub id: String,
    pub kind: OperationKind,
    pub origin: OperationOrigin,
    #[ts(type = "number")]
    pub requested_at: Timestamp,
    pub state: OperationState,
    /// Packages named by the user (install/remove) – empty for upgrades.
    pub package_targets: Vec<String>,
    #[ts(type = "number | null")]
    pub started_at: Option<Timestamp>,
    #[ts(type = "number | null")]
    pub ended_at: Option<Timestamp>,
    /// Exit code of the pacman step that ended the operation.
    pub exit_code: Option<i32>,
    /// Short English summary; the UI renders a localized text from state and error code.
    pub summary: String,
    pub error: Option<AppError>,
    /// `true` as soon as the pacman commit phase may have started.
    pub commit_started: bool,
    /// Plan confirmed by the user.
    pub confirmed_digest: Option<String>,
    /// Actual plan computed before the commit (set on [`ErrorCode::PlanChanged`]).
    pub actual_plan: Option<TransactionPlan>,
    pub progress: Progress,
    pub snapshot: Option<SnapshotResult>,
    /// Number of `.pacnew`/`.pacsave` files reported by pacman during the transaction.
    pub new_pacnew_files: u32,
    /// Changes recorded by pacman for this operation.
    pub changes: ChangeCounts,
    /// Result could not be determined with certainty (e.g. after a crash).
    pub outcome_unknown: bool,
}

impl Operation {
    pub fn new(id: String, kind: OperationKind, origin: OperationOrigin, now: Timestamp) -> Self {
        Self {
            id,
            kind,
            origin,
            requested_at: now,
            state: OperationState::Idle,
            package_targets: Vec::new(),
            started_at: None,
            ended_at: None,
            exit_code: None,
            summary: String::new(),
            error: None,
            commit_started: false,
            confirmed_digest: None,
            actual_plan: None,
            progress: Progress::default(),
            snapshot: None,
            new_pacnew_files: 0,
            changes: ChangeCounts::default(),
            outcome_unknown: false,
        }
    }

    /// Applies a state transition, enforcing the state machine.
    pub fn transition(&mut self, next: OperationState, now: Timestamp) -> AppResult<()> {
        if !self.state.can_transition_to(next) {
            return Err(AppError::new(
                ErrorCode::Internal,
                format!(
                    "invalid operation transition {:?} -> {:?}",
                    self.state, next
                ),
            ));
        }
        if self.started_at.is_none() && next.is_active() {
            self.started_at = Some(now);
        }
        if next == OperationState::Installing {
            self.commit_started = true;
        }
        if next.is_terminal() {
            self.ended_at = Some(now);
        }
        self.state = next;
        Ok(())
    }

    /// Ends the operation with an error. Chooses `Failed`, or `NeedsAttention`
    /// when the commit phase had started and the package state is unclear.
    pub fn fail(&mut self, error: AppError, now: Timestamp) -> AppResult<()> {
        let next = if self.commit_started {
            OperationState::NeedsAttention
        } else {
            OperationState::Failed
        };
        self.summary = error.message.clone();
        self.error = Some(error);
        self.transition(next, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use OperationState::*;

    #[test]
    fn terminal_states_have_no_successors() {
        for from in OperationState::ALL {
            if from.is_terminal() {
                for to in OperationState::ALL {
                    assert!(!from.can_transition_to(to), "{from:?} -> {to:?}");
                }
            }
        }
    }

    #[test]
    fn no_cancel_after_commit() {
        assert!(!Installing.can_cancel());
        assert!(!Installing.can_transition_to(CancelledBeforeCommit));
        for s in [AwaitingAuthorization, Preparing, Downloading] {
            assert!(s.can_cancel());
            assert!(s.can_transition_to(CancelledBeforeCommit));
        }
    }

    #[test]
    fn happy_path_upgrade() {
        let mut op = Operation::new(
            "id".into(),
            OperationKind::SystemUpgrade,
            OperationOrigin::User,
            1,
        );
        for (i, next) in [
            AwaitingAuthorization,
            Preparing,
            Downloading,
            Installing,
            Succeeded,
        ]
        .into_iter()
        .enumerate()
        {
            op.transition(next, 10 + i as i64).unwrap();
        }
        assert_eq!(op.started_at, Some(10));
        assert_eq!(op.ended_at, Some(14));
        assert!(op.commit_started);
    }

    #[test]
    fn invalid_transition_is_rejected() {
        let mut op = Operation::new(
            "id".into(),
            OperationKind::Install,
            OperationOrigin::User,
            1,
        );
        assert!(op.transition(Installing, 2).is_err());
        assert_eq!(op.state, Idle);
    }

    #[test]
    fn failure_before_and_after_commit() {
        let mut before = Operation::new(
            "a".into(),
            OperationKind::SystemUpgrade,
            OperationOrigin::User,
            1,
        );
        before.transition(AwaitingAuthorization, 1).unwrap();
        before.transition(Preparing, 1).unwrap();
        before.transition(Downloading, 1).unwrap();
        before
            .fail(
                AppError::new(ErrorCode::TransactionFailed, "invalid signature"),
                2,
            )
            .unwrap();
        assert_eq!(before.state, Failed);
        assert!(!before.commit_started);

        let mut after = Operation::new(
            "b".into(),
            OperationKind::SystemUpgrade,
            OperationOrigin::User,
            1,
        );
        after.transition(AwaitingAuthorization, 1).unwrap();
        after.transition(Preparing, 1).unwrap();
        after.transition(Installing, 1).unwrap();
        after
            .fail(AppError::new(ErrorCode::TransactionFailed, "disk full"), 2)
            .unwrap();
        assert_eq!(after.state, NeedsAttention);
    }

    #[test]
    fn check_flow() {
        assert!(Idle.can_transition_to(Checking));
        assert!(Checking.can_transition_to(Ready));
        assert!(Ready.can_transition_to(AwaitingAuthorization));
        assert!(!Checking.can_transition_to(Installing));
    }
}
