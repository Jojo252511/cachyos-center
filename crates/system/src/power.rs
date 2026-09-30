//! Power supply state (preflight: "stabiler Stromstatus soweit erkennbar").

use std::path::Path;

/// Power state as far as it can be determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerStatus {
    /// `Some(true)` on mains power, `None` when not determinable.
    pub on_ac: Option<bool>,
    /// Lowest battery charge in percent, if a battery exists.
    pub battery_percent: Option<u8>,
}

impl PowerStatus {
    /// Unattended preparation is acceptable: on AC power, or battery above 50 %.
    pub fn stable(&self) -> Option<bool> {
        match (self.on_ac, self.battery_percent) {
            (Some(true), _) => Some(true),
            (_, Some(p)) => Some(p >= 50),
            (Some(false), None) => Some(false),
            (None, None) => None,
        }
    }
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Reads `/sys/class/power_supply` (or a test directory).
pub fn status_from(dir: &Path) -> PowerStatus {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return PowerStatus {
            on_ac: None,
            battery_percent: None,
        };
    };
    let mut any = false;
    let mut mains: Option<bool> = None;
    let mut battery: Option<u8> = None;
    for entry in entries.flatten() {
        any = true;
        let p = entry.path();
        match read(&p.join("type")).as_deref() {
            Some("Mains") | Some("USB") => {
                let online = read(&p.join("online")).as_deref() == Some("1");
                mains = Some(mains.unwrap_or(false) || online);
            }
            Some("Battery") => {
                if read(&p.join("scope")).as_deref() == Some("Device") {
                    continue; // peripheral batteries (mouse, headset)
                }
                if let Some(c) = read(&p.join("capacity")).and_then(|c| c.parse::<u8>().ok()) {
                    battery = Some(battery.map_or(c, |b| b.min(c)));
                }
            }
            _ => {}
        }
    }
    PowerStatus {
        // A machine without any power supply entry is a desktop on mains power.
        on_ac: if !any || (battery.is_none() && mains.is_none()) {
            Some(true)
        } else {
            mains
        },
        battery_percent: battery,
    }
}

pub fn status() -> PowerStatus {
    status_from(Path::new("/sys/class/power_supply"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supply(dir: &Path, name: &str, files: &[(&str, &str)]) {
        let p = dir.join(name);
        std::fs::create_dir_all(&p).unwrap();
        for (f, v) in files {
            std::fs::write(p.join(f), v).unwrap();
        }
    }

    #[test]
    fn desktop_without_supplies() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(status_from(dir.path()).stable(), Some(true));
    }

    #[test]
    fn laptop_on_battery() {
        let dir = tempfile::tempdir().unwrap();
        supply(dir.path(), "AC", &[("type", "Mains"), ("online", "0")]);
        supply(
            dir.path(),
            "BAT0",
            &[("type", "Battery"), ("capacity", "35"), ("scope", "System")],
        );
        supply(
            dir.path(),
            "hidpp_battery_0",
            &[("type", "Battery"), ("capacity", "5"), ("scope", "Device")],
        );
        let s = status_from(dir.path());
        assert_eq!(s.on_ac, Some(false));
        assert_eq!(s.battery_percent, Some(35));
        assert_eq!(s.stable(), Some(false));
    }

    #[test]
    fn laptop_on_ac() {
        let dir = tempfile::tempdir().unwrap();
        supply(dir.path(), "AC", &[("type", "Mains"), ("online", "1")]);
        supply(
            dir.path(),
            "BAT0",
            &[("type", "Battery"), ("capacity", "10")],
        );
        assert_eq!(status_from(dir.path()).stable(), Some(true));
    }
}
