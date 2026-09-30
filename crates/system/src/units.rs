//! Read-only systemd unit queries via `systemctl show` (no privileges needed).

use std::collections::HashMap;
use std::process::{Command, Stdio};

use cachyos_center_core::Timestamp;

/// Scope of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    System,
    User,
}

/// Selected properties of a unit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnitState {
    pub load_state: String,
    pub active_state: String,
    pub unit_file_state: String,
    pub next_elapse: Option<Timestamp>,
    pub last_trigger: Option<Timestamp>,
    pub result: String,
}

impl UnitState {
    /// The unit file exists.
    pub fn exists(&self) -> bool {
        self.load_state == "loaded"
    }

    pub fn enabled(&self) -> bool {
        self.exists() && matches!(self.unit_file_state.as_str(), "enabled" | "enabled-runtime")
    }

    pub fn active(&self) -> bool {
        matches!(
            self.active_state.as_str(),
            "active" | "activating" | "reloading"
        )
    }
}

/// Parses `systemctl show` output (`Key=Value` lines).
pub fn parse_show(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Timestamps are requested with `--timestamp=unix` (`@1790789950`); raw
/// microsecond values are accepted as well.
fn parse_ts(value: Option<&String>) -> Option<Timestamp> {
    let value = value?.trim();
    if let Some(secs) = value.strip_prefix('@') {
        return secs.parse::<i64>().ok().filter(|s| *s > 0);
    }
    let v: u64 = value.parse().ok()?;
    if v == 0 || v == u64::MAX {
        return None;
    }
    i64::try_from(v / 1_000_000).ok()
}

pub fn state_from(map: &HashMap<String, String>) -> UnitState {
    UnitState {
        load_state: map.get("LoadState").cloned().unwrap_or_default(),
        active_state: map.get("ActiveState").cloned().unwrap_or_default(),
        unit_file_state: map.get("UnitFileState").cloned().unwrap_or_default(),
        next_elapse: parse_ts(map.get("NextElapseUSecRealtime")),
        last_trigger: parse_ts(map.get("LastTriggerUSec")),
        result: map.get("Result").cloned().unwrap_or_default(),
    }
}

fn valid_unit_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'@' | b'\\'))
        && !name.starts_with('-')
}

/// Queries a unit. Returns `None` when systemctl is unavailable.
pub fn show(scope: Scope, unit: &str) -> Option<UnitState> {
    if !valid_unit_name(unit) {
        return None;
    }
    let mut cmd = Command::new("systemctl");
    if scope == Scope::User {
        cmd.arg("--user");
    }
    let output = cmd
        .args([
            "show",
            "--property=LoadState,ActiveState,UnitFileState,NextElapseUSecRealtime,LastTriggerUSec,Result",
            "--timestamp=unix",
            "--",
            unit,
        ])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(state_from(&parse_show(&text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_timer_state() {
        let map = parse_show(
            "LoadState=loaded\nActiveState=active\nUnitFileState=enabled\nNextElapseUSecRealtime=@1790800000\nLastTriggerUSec=\nResult=success\n",
        );
        let s = state_from(&map);
        assert!(s.exists() && s.enabled() && s.active());
        assert_eq!(s.next_elapse, Some(1_790_800_000));
        assert_eq!(s.last_trigger, None);
    }

    #[test]
    fn missing_unit() {
        let s = state_from(&parse_show(
            "LoadState=not-found\nActiveState=inactive\nUnitFileState=\n",
        ));
        assert!(!s.exists());
        assert!(!s.enabled());
        assert!(!s.active());
    }

    #[test]
    fn rejects_odd_unit_names() {
        assert!(show(Scope::System, "--help").is_none());
        assert!(show(Scope::System, "a b").is_none());
    }
}
