//! Transaction plans: what pacman is going to change.
//!
//! A plan is computed without the database lock (the same way `pacman -Sp`
//! works). The digest identifies a plan; the privileged helper recomputes the
//! plan right before the commit and stops when the digests differ.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::Timestamp;
use crate::updates::UpdateFlag;

/// Kind of package transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlanKind {
    /// `pacman -Syu`
    SystemUpgrade,
    /// `pacman -Syu repo/pkg`
    Install,
    /// `pacman -R pkg` (optionally `-Rs`)
    Remove,
}

impl PlanKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SystemUpgrade => "systemUpgrade",
            Self::Install => "install",
            Self::Remove => "remove",
        }
    }
}

/// Action pacman takes for one package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlanAction {
    Install,
    Upgrade,
    Downgrade,
    Reinstall,
    Remove,
}

impl PlanAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Upgrade => "upgrade",
            Self::Downgrade => "downgrade",
            Self::Reinstall => "reinstall",
            Self::Remove => "remove",
        }
    }
}

/// One package in a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanEntry {
    pub action: PlanAction,
    pub name: String,
    /// Sync repository for installs/upgrades, `None` for removals.
    pub repository: Option<String>,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    #[ts(type = "number | null")]
    pub download_size: Option<u64>,
    /// Explicitly requested by the user (as opposed to pulled in by dependencies or the upgrade).
    pub requested: bool,
    pub flags: Vec<UpdateFlag>,
}

/// Where the plan data came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlanSource {
    /// Isolated sync database of the last update check (never the productive one).
    IsolatedCheckDb,
    /// Productive pacman sync database (`/var/lib/pacman/sync`).
    SystemDb,
}

/// Non-fatal notes attached to a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    tag = "kind",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum PlanWarning {
    /// The install pulls in updates of other packages (full `-Syu`).
    IncludesSystemUpgrade {
        #[ts(type = "number")]
        upgrade_count: u64,
    },
    /// Removing these packages affects the running system.
    CriticalPackages { packages: Vec<String> },
    /// Packages held back by `IgnorePkg`/`IgnoreGroup`; the result is no complete upgrade.
    HeldBackPackages { packages: Vec<String> },
    /// A reboot is recommended after this transaction.
    RebootRecommended { packages: Vec<String> },
    /// Other packages would be removed as replacements.
    Replacements { packages: Vec<String> },
}

/// Complete, reviewable package transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransactionPlan {
    pub kind: PlanKind,
    /// Targets named by the user (empty for a system upgrade).
    pub targets: Vec<String>,
    pub entries: Vec<PlanEntry>,
    #[ts(type = "number")]
    pub download_size: u64,
    /// Change of installed size in bytes (negative when space is freed).
    #[ts(type = "number")]
    pub install_size_delta: i64,
    pub source: PlanSource,
    #[ts(type = "number")]
    pub computed_at: Timestamp,
    pub warnings: Vec<PlanWarning>,
    /// Remove unneeded dependencies as well (`-Rs`), only for [`PlanKind::Remove`].
    pub recursive: bool,
    /// Lower-case hex SHA-256 over the canonical plan (see [`TransactionPlan::digest_of`]).
    pub digest: String,
}

impl TransactionPlan {
    /// Canonical digest over kind, recursion flag and the sorted entries.
    ///
    /// Sizes, timestamps and warnings are deliberately not part of the digest:
    /// only a change of the packages or versions is a *significant* deviation.
    pub fn digest_of(kind: PlanKind, recursive: bool, entries: &[PlanEntry]) -> String {
        let mut lines: Vec<String> = entries
            .iter()
            .map(|e| {
                format!(
                    "{}|{}|{}|{}|{}",
                    e.action.as_str(),
                    e.repository.as_deref().unwrap_or(""),
                    e.name,
                    e.old_version.as_deref().unwrap_or(""),
                    e.new_version.as_deref().unwrap_or("")
                )
            })
            .collect();
        lines.sort();
        let mut hasher = Sha256::new();
        hasher.update(b"cachyos-center-plan-v1\n");
        hasher.update(kind.as_str().as_bytes());
        hasher.update(if recursive {
            &b"|recursive\n"[..]
        } else {
            &b"|plain\n"[..]
        });
        for line in lines {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        }
        hex::encode(hasher.finalize())
    }

    /// Recomputes and stores the digest.
    pub fn seal(mut self) -> Self {
        self.digest = Self::digest_of(self.kind, self.recursive, &self.entries);
        self
    }

    /// `true` when nothing would change.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of entries per action.
    pub fn count(&self, action: PlanAction) -> usize {
        self.entries.iter().filter(|e| e.action == action).count()
    }

    /// Difference between a confirmed plan (`self`) and the actual plan.
    pub fn diff(&self, actual: &TransactionPlan) -> PlanDiff {
        let key = |e: &PlanEntry| (e.name.clone(), e.action);
        let before: std::collections::BTreeMap<_, _> =
            self.entries.iter().map(|e| (key(e), e)).collect();
        let after: std::collections::BTreeMap<_, _> =
            actual.entries.iter().map(|e| (key(e), e)).collect();
        let mut diff = PlanDiff::default();
        for (k, e) in &after {
            match before.get(k) {
                None => diff.added.push((*e).clone()),
                Some(old) if *old != *e => {
                    if old.repository != e.repository
                        || old.old_version != e.old_version
                        || old.new_version != e.new_version
                    {
                        diff.changed.push((*e).clone());
                    }
                }
                Some(_) => {}
            }
        }
        for (k, e) in &before {
            if !after.contains_key(k) {
                diff.removed.push((*e).clone());
            }
        }
        diff
    }
}

/// Difference between two plans.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanDiff {
    pub added: Vec<PlanEntry>,
    pub removed: Vec<PlanEntry>,
    pub changed: Vec<PlanEntry>,
}

impl PlanDiff {
    pub fn is_significant(&self) -> bool {
        !(self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, old: &str, new: &str) -> PlanEntry {
        PlanEntry {
            action: PlanAction::Upgrade,
            name: name.into(),
            repository: Some("extra".into()),
            old_version: Some(old.into()),
            new_version: Some(new.into()),
            download_size: Some(100),
            requested: false,
            flags: vec![],
        }
    }

    fn plan(entries: Vec<PlanEntry>) -> TransactionPlan {
        TransactionPlan {
            kind: PlanKind::SystemUpgrade,
            targets: vec![],
            entries,
            download_size: 0,
            install_size_delta: 0,
            source: PlanSource::SystemDb,
            computed_at: 0,
            warnings: vec![],
            recursive: false,
            digest: String::new(),
        }
        .seal()
    }

    #[test]
    fn digest_is_order_independent() {
        let a = plan(vec![entry("a", "1", "2"), entry("b", "1", "2")]);
        let b = plan(vec![entry("b", "1", "2"), entry("a", "1", "2")]);
        assert_eq!(a.digest, b.digest);
        assert_eq!(a.digest.len(), 64);
        crate::validate::plan_digest(&a.digest).unwrap();
    }

    #[test]
    fn digest_ignores_sizes_but_not_versions() {
        let a = plan(vec![entry("a", "1", "2")]);
        let mut e = entry("a", "1", "2");
        e.download_size = Some(999);
        let b = plan(vec![e]);
        assert_eq!(a.digest, b.digest);
        let c = plan(vec![entry("a", "1", "3")]);
        assert_ne!(a.digest, c.digest);
    }

    #[test]
    fn digest_depends_on_kind_and_recursion() {
        let entries = vec![entry("a", "1", "2")];
        let upgrade = TransactionPlan::digest_of(PlanKind::SystemUpgrade, false, &entries);
        let install = TransactionPlan::digest_of(PlanKind::Install, false, &entries);
        let recursive = TransactionPlan::digest_of(PlanKind::SystemUpgrade, true, &entries);
        assert_ne!(upgrade, install);
        assert_ne!(upgrade, recursive);
    }

    #[test]
    fn diff_reports_changes() {
        let confirmed = plan(vec![entry("a", "1", "2"), entry("b", "1", "2")]);
        let actual = plan(vec![entry("a", "1", "3"), entry("c", "1", "2")]);
        let diff = confirmed.diff(&actual);
        assert!(diff.is_significant());
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].name, "c");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].name, "b");
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].new_version.as_deref(), Some("3"));
        assert!(!confirmed.diff(&confirmed).is_significant());
    }
}
