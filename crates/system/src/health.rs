//! System-level health checks: `.pacnew`/`.pacsave` files, snapshot support,
//! package cache size and reboot hints.

use std::path::Path;
use std::time::UNIX_EPOCH;

use cachyos_center_core::health::{ConfigFileHint, SnapshotSupport};

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

/// Size of the package cache (files directly in the cache directories).
pub fn package_cache_bytes(dirs: &[String]) -> Option<u64> {
    let mut total = 0u64;
    let mut any = false;
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        any = true;
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata()
                && meta.is_file()
            {
                total = total.saturating_add(meta.len());
            }
        }
    }
    any.then_some(total)
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
    fn cache_size() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.pkg.tar.zst"), vec![0u8; 100]).unwrap();
        std::fs::write(dir.path().join("b.pkg.tar.zst"), vec![0u8; 50]).unwrap();
        let d = dir.path().to_string_lossy().into_owned();
        assert_eq!(package_cache_bytes(&[d]), Some(150));
        assert_eq!(package_cache_bytes(&["/does/not/exist".into()]), None);
    }
}
