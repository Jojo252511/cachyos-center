//! System-level health checks: `.pacnew`/`.pacsave` files, snapshot support,
//! package cache size and reboot hints.

use std::collections::HashMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use cachyos_center_core::health::{ConfigFileHint, SnapshotSupport};
use cachyos_center_core::version::vercmp;

/// Maximum directory depth below `/etc` that is scanned.
const MAX_DEPTH: usize = 6;
/// Maximum number of reported files.
const MAX_FILES: usize = 200;

/// Finds `.pacnew` and `.pacsave` files below `root` (usually `/etc`).
/// Unreadable directories are skipped; symlinks are not followed.
pub fn config_files(root: &Path) -> Vec<ConfigFileHint> {
    let mut out = Vec::new();
    walk(root, 0, &mut out);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<ConfigFileHint>) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let path = entry.path();
        if ft.is_dir() {
            walk(&path, depth + 1, out);
        } else if ft.is_file() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let kind = if name.ends_with(".pacnew") {
                "pacnew"
            } else if name.ends_with(".pacsave") || name.contains(".pacsave.") {
                "pacsave"
            } else {
                continue;
            };
            out.push(ConfigFileHint {
                path: path.to_string_lossy().into_owned(),
                kind: kind.into(),
                modified_at: entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .and_then(|d| i64::try_from(d.as_secs()).ok()),
            });
            if out.len() >= MAX_FILES {
                return;
            }
        }
    }
}

/// Snapper configuration names (file names below `/etc/snapper/configs`).
fn snapper_configs(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default()
}

/// Snapshot support from explicit sources (testable).
pub fn snapshot_support_from(
    root_fs: Option<&str>,
    snapper_bin: bool,
    configs: &[String],
    snap_pac_hook: bool,
) -> SnapshotSupport {
    let btrfs = root_fs == Some("btrfs");
    let root_config = configs.iter().find(|c| *c == "root").cloned();
    SnapshotSupport {
        btrfs_root: btrfs,
        snapper_installed: snapper_bin,
        can_request_snapshot: btrfs && snapper_bin && root_config.is_some(),
        root_config,
        snap_pac_active: snap_pac_hook && snapper_bin,
    }
}

pub fn snapshot_support() -> SnapshotSupport {
    let mounts = std::fs::read_to_string("/proc/self/mounts").unwrap_or_default();
    let root_fs = crate::hardware::fs_type(&mounts, "/");
    snapshot_support_from(
        root_fs.as_deref(),
        Path::new("/usr/bin/snapper").exists(),
        &snapper_configs(Path::new("/etc/snapper/configs")),
        Path::new("/usr/share/libalpm/hooks/05-snap-pac-pre.hook").exists(),
    )
}

/// Versions of each package that `paccache -r` keeps.
pub const PACCACHE_KEEP: usize = 3;

/// Size of the package cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheUsage {
    /// All files directly in the cache directories.
    pub total: u64,
    /// What `paccache -rk<keep>` would remove: all but the `keep` newest
    /// versions of each package and architecture, with their signatures.
    pub reclaimable: u64,
}

/// Measures the package cache like `paccache -rk<keep>` sees it.
pub fn package_cache_usage(dirs: &[String], keep: usize) -> Option<CacheUsage> {
    let mut total = 0u64;
    let mut any = false;
    // (name, arch) -> version -> bytes of the package file and its signature
    let mut packages: HashMap<(String, String), HashMap<String, u64>> = HashMap::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        any = true;
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            total = total.saturating_add(meta.len());
            let file = entry.file_name();
            if let Some((name, version, arch)) = file.to_str().and_then(package_file) {
                let bytes = packages
                    .entry((name.to_string(), arch.to_string()))
                    .or_default()
                    .entry(version.to_string())
                    .or_default();
                *bytes = bytes.saturating_add(meta.len());
            }
        }
    }
    let reclaimable = packages
        .into_values()
        .map(|versions| {
            let mut versions: Vec<(String, u64)> = versions.into_iter().collect();
            versions.sort_by(|a, b| vercmp(&b.0, &a.0));
            versions.iter().skip(keep).map(|v| v.1).sum::<u64>()
        })
        .sum();
    any.then_some(CacheUsage { total, reclaimable })
}

/// Name, `[epoch:]pkgver-pkgrel` and architecture of a package file or its
/// signature (`<name>-<pkgver>-<pkgrel>-<arch>.pkg.tar.<ext>[.sig]`).
fn package_file(file: &str) -> Option<(&str, &str, &str)> {
    let stem = &file[..file.find(".pkg.tar")?];
    let (rest, arch) = stem.rsplit_once('-')?;
    let (name, _) = rest.rsplit_once('-')?.0.rsplit_once('-')?;
    Some((name, &rest[name.len() + 1..], arch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_config_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("pacman.d")).unwrap();
        std::fs::write(dir.path().join("pacman.conf.pacnew"), "x").unwrap();
        std::fs::write(dir.path().join("pacman.d/mirrorlist.pacsave"), "x").unwrap();
        std::fs::write(dir.path().join("fstab"), "x").unwrap();
        let files = config_files(dir.path());
        assert_eq!(files.len(), 2);
        assert!(
            files
                .iter()
                .any(|f| f.kind == "pacnew" && f.path.ends_with("pacman.conf.pacnew"))
        );
        assert!(files.iter().any(|f| f.kind == "pacsave"));
    }

    #[test]
    fn snapshot_detection() {
        let root = vec!["root".to_string()];
        let s = snapshot_support_from(Some("btrfs"), true, &root, true);
        assert!(s.can_request_snapshot && s.snap_pac_active);
        let s = snapshot_support_from(Some("ext4"), true, &root, false);
        assert!(!s.can_request_snapshot && !s.btrfs_root);
        let s = snapshot_support_from(Some("btrfs"), false, &[], true);
        assert!(!s.can_request_snapshot && !s.snap_pac_active);
    }

    #[test]
    fn package_file_names() {
        assert_eq!(
            package_file("linux-cachyos-7.2.9-1-x86_64_v3.pkg.tar.zst.sig"),
            Some(("linux-cachyos", "7.2.9-1", "x86_64_v3"))
        );
        assert_eq!(
            package_file("lib32-foo-1:2.0-3.1-x86_64.pkg.tar.xz"),
            Some(("lib32-foo", "1:2.0-3.1", "x86_64"))
        );
        assert_eq!(package_file("notes.txt"), None);
        assert_eq!(package_file("broken-1.pkg.tar.zst"), None);
    }

    #[test]
    fn cache_usage_counts_what_paccache_removes() {
        let dir = tempfile::tempdir().unwrap();
        let file = |name: &str, size: usize| {
            std::fs::write(dir.path().join(name), vec![0u8; size]).unwrap()
        };
        // Five versions in mixed name order: the two oldest (with signature) can go.
        for (version, size) in [
            ("1.10-1", 50),
            ("1.9-1", 40),
            ("1.2-1", 20),
            ("1.10-2", 60),
            ("1:0.1-1", 70),
        ] {
            file(&format!("foo-{version}-x86_64.pkg.tar.zst"), size);
        }
        file("foo-1.2-1-x86_64.pkg.tar.zst.sig", 1);
        // Another architecture and another package are counted on their own.
        file("foo-1.0-1-x86_64_v3.pkg.tar.zst", 7);
        file("bar-2.0-1-any.pkg.tar.zst", 5);
        file("notes.txt", 3);
        let d = dir.path().to_string_lossy().into_owned();
        assert_eq!(
            package_cache_usage(std::slice::from_ref(&d), PACCACHE_KEEP),
            Some(CacheUsage {
                total: 256,
                reclaimable: 40 + 20 + 1
            })
        );
        assert_eq!(
            package_cache_usage(&[d], 1).map(|u| u.reclaimable),
            Some(60 + 50 + 40 + 21)
        );
        assert_eq!(
            package_cache_usage(&["/does/not/exist".into()], PACCACHE_KEEP),
            None
        );
    }
}
