//! CPU, GPU, memory and disks.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use cachyos_center_core::system::{CpuInfo, DiskInfo, GpuInfo, MemoryInfo};

pub fn cpu_info_from(cpuinfo: &str) -> CpuInfo {
    let mut model = None;
    let mut vendor = None;
    let mut threads = 0u32;
    let mut cores = BTreeSet::new();
    let mut physical = String::new();
    let mut flags: Vec<String> = Vec::new();
    for line in cpuinfo.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "processor" => threads += 1,
            "model name" if model.is_none() => model = Some(value.to_string()),
            "vendor_id" if vendor.is_none() => vendor = Some(value.to_string()),
            "physical id" => physical = value.to_string(),
            "core id" => {
                cores.insert(format!("{physical}:{value}"));
            }
            "flags" if flags.is_empty() => {
                flags = value.split_whitespace().map(str::to_string).collect();
            }
            _ => {}
        }
    }
    let has = |f: &str| flags.iter().any(|x| x == f);
    let v2 = ["cx16", "lahf_lm", "popcnt", "sse4_1", "sse4_2", "ssse3"]
        .iter()
        .all(|f| has(f));
    let v3 = v2
        && [
            "avx", "avx2", "bmi1", "bmi2", "f16c", "fma", "abm", "movbe", "xsave",
        ]
        .iter()
        .all(|f| has(f));
    let v4 = v3
        && ["avx512f", "avx512bw", "avx512cd", "avx512dq", "avx512vl"]
            .iter()
            .all(|f| has(f));
    let isa_level = if flags.is_empty() {
        None
    } else if v4 {
        Some("x86-64-v4".to_string())
    } else if v3 {
        Some("x86-64-v3".to_string())
    } else if v2 {
        Some("x86-64-v2".to_string())
    } else {
        Some("x86-64".to_string())
    };
    CpuInfo {
        model: model.unwrap_or_else(|| "unknown".into()),
        vendor,
        cores: if cores.is_empty() {
            threads
        } else {
            u32::try_from(cores.len()).unwrap_or(threads)
        },
        threads,
        isa_level,
    }
}

pub fn cpu_info() -> CpuInfo {
    cpu_info_from(&std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default())
}

pub fn memory_from(meminfo: &str) -> MemoryInfo {
    let mut map = HashMap::new();
    for line in meminfo.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let kb = v
                .split_whitespace()
                .next()
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(0);
            map.insert(k.trim().to_string(), kb.saturating_mul(1024));
        }
    }
    let get = |k: &str| map.get(k).copied().unwrap_or(0);
    MemoryInfo {
        total_bytes: get("MemTotal"),
        available_bytes: get("MemAvailable"),
        swap_total_bytes: get("SwapTotal"),
        swap_free_bytes: get("SwapFree"),
    }
}

pub fn memory() -> MemoryInfo {
    memory_from(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default())
}

/// Parses `pci.ids` for the given vendor/device pairs only.
pub fn pci_names(
    ids: &str,
    wanted: &[(String, String)],
) -> HashMap<(String, String), (String, String)> {
    let mut out = HashMap::new();
    let mut vendor: Option<(String, String)> = None;
    for line in ids.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            let (id, name) = line.split_once("  ").unwrap_or((line, ""));
            let id = id.trim().to_ascii_lowercase();
            vendor = wanted
                .iter()
                .any(|(v, _)| *v == id)
                .then(|| (id, name.trim().to_string()));
        } else if let Some((vid, vname)) = &vendor
            && !line.starts_with("\t\t")
        {
            let (id, name) = line.trim_start().split_once("  ").unwrap_or((line, ""));
            let id = id.trim().to_ascii_lowercase();
            if wanted.iter().any(|(v, d)| v == vid && *d == id) {
                out.insert((vid.clone(), id), (vname.clone(), name.trim().to_string()));
            }
        }
    }
    out
}

fn vendor_fallback(vendor: &str) -> &'static str {
    match vendor {
        "10de" => "NVIDIA",
        "1002" => "AMD",
        "8086" => "Intel",
        "1af4" => "Virtio",
        "15ad" => "VMware",
        "80ee" => "VirtualBox",
        _ => "Unknown",
    }
}

fn short_vendor(name: &str) -> String {
    if name.contains("NVIDIA") {
        "NVIDIA".into()
    } else if name.contains("AMD") || name.contains("ATI") {
        "AMD".into()
    } else if name.contains("Intel") {
        "Intel".into()
    } else {
        name.to_string()
    }
}

/// GPUs from `/sys/class/drm/cardN/device` (no serial numbers).
pub fn gpus_from(drm: &Path, pci_ids: Option<&str>) -> Vec<GpuInfo> {
    let Ok(entries) = std::fs::read_dir(drm) else {
        return Vec::new();
    };
    let mut raw = Vec::new();
    let mut seen = BTreeSet::new();
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.starts_with("card") && n[4..].bytes().all(|b| b.is_ascii_digit()) && n.len() > 4
        })
        .collect();
    names.sort();
    for name in names {
        let dev = drm.join(&name).join("device");
        let read = |f: &str| {
            std::fs::read_to_string(dev.join(f))
                .map(|s| s.trim().trim_start_matches("0x").to_ascii_lowercase())
                .unwrap_or_default()
        };
        let (vendor, device) = (read("vendor"), read("device"));
        if vendor.is_empty() {
            continue;
        }
        let key = std::fs::canonicalize(&dev).unwrap_or(dev.clone());
        if !seen.insert(key) {
            continue;
        }
        let driver = std::fs::read_to_string(dev.join("uevent"))
            .ok()
            .and_then(|u| {
                u.lines()
                    .find_map(|l| l.strip_prefix("DRIVER=").map(str::to_string))
            });
        raw.push((vendor, device, driver));
    }
    let wanted: Vec<(String, String)> =
        raw.iter().map(|(v, d, _)| (v.clone(), d.clone())).collect();
    let names = pci_ids
        .map(|ids| pci_names(ids, &wanted))
        .unwrap_or_default();
    raw.into_iter()
        .map(|(vendor, device, driver)| {
            let (vendor_name, model) = names
                .get(&(vendor.clone(), device.clone()))
                .cloned()
                .unwrap_or_else(|| {
                    (
                        vendor_fallback(&vendor).to_string(),
                        format!("Device {device}"),
                    )
                });
            GpuInfo {
                vendor: short_vendor(&vendor_name),
                model,
                driver,
                pci_id: format!("{vendor}:{device}"),
            }
        })
        .collect()
}

pub fn gpus() -> Vec<GpuInfo> {
    let ids = std::fs::read_to_string("/usr/share/hwdata/pci.ids").ok();
    gpus_from(Path::new("/sys/class/drm"), ids.as_deref())
}

/// Filesystem type of the mount point `target` from `/proc/self/mounts` content.
pub fn fs_type(mounts: &str, target: &str) -> Option<String> {
    mounts
        .lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let _src = parts.next()?;
            let mount = parts.next()?;
            let fstype = parts.next()?;
            (mount == target).then(|| fstype.to_string())
        })
        .next_back()
}

fn statvfs(path: &Path) -> Option<(u64, u64)> {
    let st = rustix::fs::statvfs(path).ok()?;
    let frsize = if st.f_frsize > 0 {
        st.f_frsize
    } else {
        st.f_bsize
    };
    Some((
        st.f_blocks.saturating_mul(frsize),
        st.f_bavail.saturating_mul(frsize),
    ))
}

/// Mount point that contains `path` (longest matching mount point).
fn mount_point_of(mounts: &str, path: &str) -> String {
    mounts
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .filter(|m| *m == "/" || path == *m || path.starts_with(&format!("{m}/")))
        .max_by_key(|m| m.len())
        .unwrap_or("/")
        .to_string()
}

/// Root filesystem and, if separate, the pacman cache.
pub fn disks() -> Vec<DiskInfo> {
    let mounts = std::fs::read_to_string("/proc/self/mounts").unwrap_or_default();
    let mut out = Vec::new();
    for path in ["/", "/var/cache/pacman/pkg"] {
        let mount = mount_point_of(&mounts, path);
        if out.iter().any(|d: &DiskInfo| d.mount_point == mount) {
            continue;
        }
        if let Some((total, available)) = statvfs(Path::new(&mount)) {
            out.push(DiskInfo {
                filesystem: fs_type(&mounts, &mount).unwrap_or_else(|| "unknown".into()),
                mount_point: mount,
                total_bytes: total,
                available_bytes: available,
            });
        }
    }
    out
}

/// Free bytes on the filesystem containing `path`.
pub fn free_bytes(path: &Path) -> Option<u64> {
    statvfs(path).map(|(_, avail)| avail)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CPUINFO: &str = "\
processor\t: 0
vendor_id\t: AuthenticAMD
model name\t: AMD Ryzen 7 7800X3D 8-Core Processor
physical id\t: 0
core id\t\t: 0
flags\t\t: fpu cx16 lahf_lm popcnt sse4_1 sse4_2 ssse3 avx avx2 bmi1 bmi2 f16c fma abm movbe xsave

processor\t: 1
vendor_id\t: AuthenticAMD
model name\t: AMD Ryzen 7 7800X3D 8-Core Processor
physical id\t: 0
core id\t\t: 0
flags\t\t: fpu

processor\t: 2
physical id\t: 0
core id\t\t: 1
";

    #[test]
    fn cpu() {
        let cpu = cpu_info_from(CPUINFO);
        assert_eq!(cpu.model, "AMD Ryzen 7 7800X3D 8-Core Processor");
        assert_eq!(cpu.threads, 3);
        assert_eq!(cpu.cores, 2);
        assert_eq!(cpu.isa_level.as_deref(), Some("x86-64-v3"));
        assert_eq!(cpu.vendor.as_deref(), Some("AuthenticAMD"));
    }

    #[test]
    fn memory_values() {
        let m = memory_from(
            "MemTotal:       32000000 kB\nMemAvailable:   16000000 kB\nSwapTotal: 0 kB\n",
        );
        assert_eq!(m.total_bytes, 32_000_000 * 1024);
        assert_eq!(m.available_bytes, 16_000_000 * 1024);
        assert_eq!(m.swap_free_bytes, 0);
    }

    #[test]
    fn pci_database_lookup() {
        let ids = "# comment\n10de  NVIDIA Corporation\n\t2d05  GB206 [GeForce RTX 5060 Ti]\n\t\t1043 1234  subsystem\n1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\t744c  Navi 31\n";
        let names = pci_names(ids, &[("10de".into(), "2d05".into())]);
        assert_eq!(
            names.get(&("10de".into(), "2d05".into())),
            Some(&(
                "NVIDIA Corporation".into(),
                "GB206 [GeForce RTX 5060 Ti]".into()
            ))
        );
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn gpus_from_fake_sysfs() {
        let dir = tempfile::tempdir().unwrap();
        let dev = dir.path().join("card0/device");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::write(dev.join("vendor"), "0x1002\n").unwrap();
        std::fs::write(dev.join("device"), "0x744c\n").unwrap();
        std::fs::write(dev.join("uevent"), "DRIVER=amdgpu\nPCI_ID=1002:744C\n").unwrap();
        std::fs::create_dir_all(dir.path().join("card0-DP-1")).unwrap();
        let gpus = gpus_from(dir.path(), None);
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].vendor, "AMD");
        assert_eq!(gpus[0].driver.as_deref(), Some("amdgpu"));
        assert_eq!(gpus[0].pci_id, "1002:744c");
    }

    #[test]
    fn mounts() {
        let mounts =
            "/dev/a / btrfs rw 0 0\n/dev/b /var/cache btrfs rw 0 0\n/dev/c /home ext4 rw 0 0\n";
        assert_eq!(fs_type(mounts, "/").as_deref(), Some("btrfs"));
        assert_eq!(
            mount_point_of(mounts, "/var/cache/pacman/pkg"),
            "/var/cache"
        );
        assert_eq!(mount_point_of(mounts, "/usr"), "/");
        assert_eq!(mount_point_of(mounts, "/homework"), "/");
    }

    #[test]
    fn disks_of_this_machine() {
        let d = disks();
        assert!(d.iter().any(|d| d.mount_point == "/" && d.total_bytes > 0));
    }
}
