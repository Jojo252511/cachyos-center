//! Read-only queries against the local and sync databases.

use std::cmp::Ordering;

use alpm::{Alpm, Db, Package, PackageReason};
use cachyos_center_core::bridge::{AlpmConfig, ClassifyContext, UpdatesData};
use cachyos_center_core::classify;
use cachyos_center_core::package::{
    InstallReason, PackageId, PackageOrigin, PackageRecord, PackageSummary, RepositoryInfo,
};
use cachyos_center_core::updates::UpdateCandidate;
use cachyos_center_core::{AppError, AppResult};

use crate::plan;

fn to_u64(v: i64) -> Option<u64> {
    u64::try_from(v).ok()
}

fn reason(pkg: &Package) -> InstallReason {
    match pkg.reason() {
        PackageReason::Explicit => InstallReason::Explicit,
        PackageReason::Depend => InstallReason::Dependency,
    }
}

/// First sync database (pacman order) that contains `name`.
fn find_sync<'a>(alpm: &'a Alpm, name: &str) -> Option<(&'a Db, &'a Package)> {
    alpm.syncdbs()
        .into_iter()
        .find_map(|db| db.pkg(name).ok().map(|p| (db, p)))
}

fn is_ignored(config: &AlpmConfig, pkg: &Package) -> bool {
    config.ignore_pkgs.iter().any(|i| i == pkg.name())
        || pkg
            .groups()
            .into_iter()
            .any(|g| config.ignore_groups.iter().any(|i| i == g))
}

pub fn repositories(alpm: &mut Alpm) -> AppResult<Vec<RepositoryInfo>> {
    Ok(alpm
        .syncdbs()
        .into_iter()
        .map(|db| RepositoryInfo {
            name: db.name().to_string(),
            package_count: db.pkgs().len() as u64,
        })
        .collect())
}

fn summary_installed(alpm: &Alpm, config: &AlpmConfig, pkg: &Package) -> PackageSummary {
    let sync = find_sync(alpm, pkg.name());
    let available = sync.map(|(_, p)| p.version().to_string());
    let update_available = sync
        .map(|(_, p)| p.version().vercmp(pkg.version()) == Ordering::Greater)
        .unwrap_or(false);
    PackageSummary {
        name: pkg.name().to_string(),
        description: pkg.desc().unwrap_or_default().to_string(),
        repository: sync.map(|(db, _)| db.name().to_string()),
        origin: if sync.is_some() {
            PackageOrigin::Repo
        } else {
            PackageOrigin::LocalOrAur
        },
        architecture: pkg.arch().unwrap_or("any").to_string(),
        installed_version: Some(pkg.version().to_string()),
        available_version: available,
        install_reason: Some(reason(pkg)),
        installed_size: to_u64(pkg.isize()),
        update_available,
        ignored: is_ignored(config, pkg),
    }
}

pub fn installed(alpm: &mut Alpm, config: &AlpmConfig) -> AppResult<Vec<PackageSummary>> {
    let alpm = &*alpm;
    let mut list: Vec<PackageSummary> = alpm
        .localdb()
        .pkgs()
        .into_iter()
        .map(|p| summary_installed(alpm, config, p))
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(list)
}

fn rank(name: &str, desc: &str, q: &str) -> Option<u8> {
    if q.is_empty() {
        return Some(4);
    }
    if name == q {
        Some(0)
    } else if name.starts_with(q) {
        Some(1)
    } else if name.contains(q) {
        Some(2)
    } else if desc.contains(q) {
        Some(3)
    } else {
        None
    }
}

pub fn search(
    alpm: &mut Alpm,
    config: &AlpmConfig,
    query: &str,
    repository: Option<&str>,
    limit: u32,
) -> AppResult<Vec<PackageSummary>> {
    let alpm = &*alpm;
    let q = query.to_lowercase();
    if let Some(repo) = repository
        && !alpm.syncdbs().into_iter().any(|db| db.name() == repo)
    {
        return Err(AppError::not_found(format!(
            "repository '{repo}' is not configured"
        )));
    }
    let local = alpm.localdb();
    let mut hits: Vec<(u8, PackageSummary)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for db in alpm.syncdbs() {
        if repository.is_some_and(|r| r != db.name()) {
            continue;
        }
        for pkg in db.pkgs() {
            let name_lc = pkg.name().to_lowercase();
            let desc_lc = pkg.desc().unwrap_or_default().to_lowercase();
            let Some(r) = rank(&name_lc, &desc_lc, &q) else {
                continue;
            };
            // Same package name in several repositories: the first one wins
            // (pacman order), unless a repository filter is set.
            if repository.is_none() && !seen.insert(pkg.name().to_string()) {
                continue;
            }
            let installed = local.pkg(pkg.name()).ok();
            hits.push((
                r,
                PackageSummary {
                    name: pkg.name().to_string(),
                    description: pkg.desc().unwrap_or_default().to_string(),
                    repository: Some(db.name().to_string()),
                    origin: PackageOrigin::Repo,
                    architecture: pkg.arch().unwrap_or("any").to_string(),
                    installed_version: installed.map(|p| p.version().to_string()),
                    available_version: Some(pkg.version().to_string()),
                    install_reason: installed.map(reason),
                    installed_size: to_u64(pkg.isize()),
                    update_available: installed
                        .is_some_and(|p| pkg.version().vercmp(p.version()) == Ordering::Greater),
                    ignored: is_ignored(config, pkg),
                },
            ));
        }
    }
    hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));
    Ok(hits
        .into_iter()
        .take(limit as usize)
        .map(|(_, s)| s)
        .collect())
}

fn deps(list: alpm::AlpmList<'_, &alpm::Dep>) -> Vec<String> {
    list.into_iter().map(|d| d.to_string()).collect()
}

fn strings(list: alpm::AlpmList<'_, &str>) -> Vec<String> {
    list.into_iter().map(str::to_string).collect()
}

pub fn details(
    alpm: &mut Alpm,
    config: &AlpmConfig,
    name: &str,
    repository: Option<&str>,
    context: &ClassifyContext,
) -> AppResult<PackageRecord> {
    cachyos_center_core::validate::package_name(name)?;
    let alpm = &*alpm;
    let local = alpm.localdb().pkg(name).ok();
    let sync = match repository {
        Some(repo) => {
            let db = alpm
                .syncdbs()
                .into_iter()
                .find(|db| db.name() == repo)
                .ok_or_else(|| {
                    AppError::not_found(format!("repository '{repo}' is not configured"))
                })?;
            db.pkg(name).ok().map(|p| (db, p))
        }
        None => find_sync(alpm, name),
    };
    let primary = match (local, sync) {
        (Some(l), _) if repository.is_none() => l,
        (_, Some((_, s))) => s,
        (Some(l), None) => l,
        (None, None) => return Err(AppError::not_found(format!("package '{name}' not found"))),
    };
    let (required_by, optional_for) = match local {
        Some(l) => (
            l.required_by().into_iter().collect(),
            l.optional_for().into_iter().collect(),
        ),
        None => (Vec::new(), Vec::new()),
    };
    let repo_name = sync
        .map(|(db, _)| db.name().to_string())
        .unwrap_or_else(|| "local".to_string());
    Ok(PackageRecord {
        id: PackageId {
            repository: repo_name,
            name: primary.name().to_string(),
            architecture: primary.arch().unwrap_or("any").to_string(),
        },
        description: primary.desc().unwrap_or_default().to_string(),
        origin: if sync.is_some() {
            PackageOrigin::Repo
        } else {
            PackageOrigin::LocalOrAur
        },
        installed_version: local.map(|p| p.version().to_string()),
        available_version: sync.map(|(_, p)| p.version().to_string()),
        install_reason: local.map(reason),
        url: primary.url().map(str::to_string),
        licenses: strings(primary.licenses()),
        groups: strings(primary.groups()),
        dependencies: deps(primary.depends()),
        optional_dependencies: deps(primary.optdepends()),
        required_by,
        optional_for,
        provides: deps(primary.provides()),
        conflicts: deps(primary.conflicts()),
        replaces: deps(primary.replaces()),
        installed_size: local.and_then(|p| to_u64(p.isize())),
        download_size: sync.and_then(|(_, p)| to_u64(p.download_size())),
        packager: primary.packager().map(str::to_string),
        build_date: Some(primary.build_date()).filter(|d| *d > 0),
        install_date: local.and_then(|p| p.install_date()),
        critical: classify::is_critical(
            name,
            &config.hold_pkgs,
            context.running_kernel_pkg.as_deref(),
            context.session_is_hyprland,
        ),
        ignored: is_ignored(config, primary),
    })
}

pub fn updates(
    alpm: &mut Alpm,
    config: &AlpmConfig,
    context: &ClassifyContext,
) -> AppResult<UpdatesData> {
    let plan = plan::upgrade(alpm, config, context)?;
    let alpm = &*alpm;
    let syncdbs = alpm.syncdbs();
    let mut held_back = Vec::new();
    for pkg in alpm.localdb().pkgs() {
        let Some(new) = pkg.sync_new_version(syncdbs) else {
            continue;
        };
        if !(is_ignored(config, pkg) || is_ignored(config, new)) {
            continue;
        }
        let repo = new.db().map(|d| d.name().to_string()).unwrap_or_default();
        held_back.push(UpdateCandidate {
            package_id: PackageId {
                repository: repo,
                name: pkg.name().to_string(),
                architecture: new.arch().unwrap_or("any").to_string(),
            },
            old_version: pkg.version().to_string(),
            new_version: new.version().to_string(),
            download_size: to_u64(new.download_size()),
            flags: classify::update_flags(
                pkg.name(),
                context.running_kernel_pkg.as_deref(),
                context.session_is_hyprland,
                true,
            ),
        });
    }
    held_back.sort_by(|a, b| a.package_id.name.cmp(&b.package_id.name));
    Ok(UpdatesData { plan, held_back })
}

#[cfg(test)]
mod tests {
    use super::rank;

    #[test]
    fn ranking() {
        assert_eq!(rank("firefox", "web browser", "firefox"), Some(0));
        assert_eq!(rank("firefox-i18n-de", "", "firefox"), Some(1));
        assert_eq!(rank("xfirefox", "", "firefox"), Some(2));
        assert_eq!(rank("icecat", "fork of firefox", "firefox"), Some(3));
        assert_eq!(rank("vim", "editor", "firefox"), None);
        assert_eq!(rank("vim", "editor", ""), Some(4));
    }
}
