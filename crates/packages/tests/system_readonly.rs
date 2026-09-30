//! Read-only integration tests against the pacman databases of the machine
//! running the tests. Nothing is modified: plans are computed without the
//! database lock (like `pacman -Sp`).
//!
//! The libalpm bridge must be built (`cargo build -p cachyos-center-alpm-bridge`).
//! Without it the tests are skipped unless `CC_REQUIRE_BRIDGE=1` is set (CI).

use std::path::Path;

use cachyos_center_core::ErrorCode;
use cachyos_center_core::package::{CatalogInstallFilter, CatalogQuery, InstalledFilter, InstalledQuery, PackageRef};
use cachyos_center_core::plan::{PlanKind, PlanWarning};
use cachyos_center_core::system::BackendStatus;
use cachyos_center_core::updates::CheckStatus;
use cachyos_center_packages::PackageService;

fn service(dir: &Path) -> Option<PackageService> {
    if !Path::new("/etc/pacman.conf").exists() {
        eprintln!("skipped: no pacman configuration");
        return None;
    }
    let svc = PackageService::new(dir.join("checkup-db"), dir.join("state.json"));
    match svc.backend_status() {
        BackendStatus::Ready { .. } => Some(svc),
        BackendStatus::Unavailable { reason } => {
            if std::env::var("CC_REQUIRE_BRIDGE").as_deref() == Ok("1") {
                panic!("bridge required but unavailable: {reason}");
            }
            eprintln!("skipped: {reason}");
            None
        }
    }
}

#[test]
fn installed_list_contains_pacman() {
    let dir = tempfile::tempdir().unwrap();
    let Some(svc) = service(dir.path()) else { return };
    let page = svc
        .installed(&InstalledQuery {
            query: Some("pacman".into()),
            filter: InstalledFilter::All,
            offset: 0,
            limit: 50,
        })
        .unwrap();
    assert!(page.items.iter().any(|p| p.name == "pacman"), "{page:?}");
    let all = svc.installed_all().unwrap();
    assert!(all.len() >= page.items.len());
    assert!(all.windows(2).all(|w| w[0].name <= w[1].name), "sorted by name");
}

#[test]
fn pagination_is_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let Some(svc) = service(dir.path()) else { return };
    let first = svc
        .installed(&InstalledQuery {
            query: None,
            filter: InstalledFilter::All,
            offset: 0,
            limit: 10,
        })
        .unwrap();
    assert!(first.items.len() <= 10);
    if let Some(next) = first.next_offset {
        let second = svc
            .installed(&InstalledQuery {
                query: None,
                filter: InstalledFilter::All,
                offset: next,
                limit: 10,
            })
            .unwrap();
        assert_ne!(first.items[0].name, second.items[0].name);
        assert_eq!(first.total, second.total);
    }
}

#[test]
fn search_finds_pacman_in_repositories() {
    let dir = tempfile::tempdir().unwrap();
    let Some(svc) = service(dir.path()) else { return };
    let results = svc
        .search(&CatalogQuery {
            query: "pacman".into(),
            repository: None,
            install_filter: CatalogInstallFilter::Any,
            limit: 20,
        })
        .unwrap();
    assert_eq!(results.first().map(|p| p.name.as_str()), Some("pacman"), "exact match first");
    assert!(results.iter().all(|p| p.repository.is_some()));
    // Regex metacharacters are plain text.
    svc.search(&CatalogQuery {
        query: "[(".into(),
        repository: None,
        install_filter: CatalogInstallFilter::Any,
        limit: 5,
    })
    .unwrap();
    let err = svc
        .search(&CatalogQuery {
            query: "pacman".into(),
            repository: Some("does-not-exist".into()),
            install_filter: CatalogInstallFilter::Any,
            limit: 5,
        })
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[test]
fn details_of_pacman() {
    let dir = tempfile::tempdir().unwrap();
    let Some(svc) = service(dir.path()) else { return };
    let record = svc
        .details(&PackageRef {
            name: "pacman".into(),
            repository: None,
        })
        .unwrap();
    assert_eq!(record.id.name, "pacman");
    assert!(record.installed_version.is_some());
    assert!(record.critical, "pacman is a HoldPkg and system critical");
    let missing = svc
        .details(&PackageRef {
            name: "cachyos-center-does-not-exist".into(),
            repository: None,
        })
        .unwrap_err();
    assert_eq!(missing.code, ErrorCode::NotFound);
}

#[test]
fn plans_are_computed_without_taking_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let Some(svc) = service(dir.path()) else { return };
    let lock = Path::new("/var/lib/pacman/db.lck");
    let locked_before = lock.exists();

    let upgrade = svc.system_plan_upgrade().unwrap();
    assert_eq!(upgrade.kind, PlanKind::SystemUpgrade);
    assert_eq!(upgrade.digest.len(), 64);

    match svc.plan_remove("pacman", false) {
        Ok(plan) => assert!(
            plan.warnings
                .iter()
                .any(|w| matches!(w, PlanWarning::CriticalPackages { .. })),
            "{plan:?}"
        ),
        Err(e) => assert_eq!(e.code, ErrorCode::DependencyProblem, "{e:?}"),
    }
    let not_installed = svc.plan_remove("cachyos-center-does-not-exist", false).unwrap_err();
    assert_eq!(not_installed.code, ErrorCode::NotFound);

    assert_eq!(lock.exists(), locked_before, "planning must not create db.lck");
}

#[test]
fn install_plan_requires_fresh_repository_data() {
    let dir = tempfile::tempdir().unwrap();
    let Some(svc) = service(dir.path()) else { return };
    let err = svc.plan_install("core", "pacman").unwrap_err();
    assert_eq!(err.code, ErrorCode::Stale);
}

#[test]
fn never_checked_is_not_zero_updates() {
    let dir = tempfile::tempdir().unwrap();
    let Some(svc) = service(dir.path()) else { return };
    let result = svc.last_check();
    assert!(
        matches!(result.status, CheckStatus::NeverChecked | CheckStatus::PrerequisiteMissing),
        "{result:?}"
    );
    assert!(!result.is_current());
}

/// Needs network access: synchronizes an isolated copy of the sync databases.
#[test]
#[ignore = "network access"]
fn isolated_update_check() {
    let dir = tempfile::tempdir().unwrap();
    let Some(svc) = service(dir.path()) else { return };
    let before = std::fs::metadata("/var/lib/pacman/sync").and_then(|m| m.modified()).ok();
    let result = svc.check_now();
    assert!(
        matches!(result.status, CheckStatus::Fresh | CheckStatus::PrerequisiteMissing),
        "{result:?}"
    );
    if result.status == CheckStatus::Fresh {
        let plan = result.plan.expect("fresh result has a plan");
        assert_eq!(plan.entries.len(), result.updates.len() + plan.count(cachyos_center_core::plan::PlanAction::Remove));
        let install = svc.plan_install("core", "pacman").unwrap();
        assert_eq!(install.kind, PlanKind::Install);
    }
    let after = std::fs::metadata("/var/lib/pacman/sync").and_then(|m| m.modified()).ok();
    assert_eq!(before, after, "the productive sync database must not be touched");
}
