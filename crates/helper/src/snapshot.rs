//! Snapshot through an existing snapper setup.
//!
//! "Snapshot erstellt" is only reported when snapper confirmed a snapshot
//! number. No subvolumes are created or changed, and no restore is ever
//! triggered.

use std::path::Path;
use std::process::{Command, Stdio};

use cachyos_center_core::operation::SnapshotResult;

fn valid_config_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Parses the number printed by `snapper create --print-number`.
pub fn parse_number(stdout: &str) -> Option<u32> {
    stdout.trim().lines().last()?.trim().parse().ok()
}

/// Creates a single snapshot of `config` before a system upgrade.
pub fn create(snapper: &Path, config: &str) -> SnapshotResult {
    if !valid_config_name(config) {
        return SnapshotResult {
            created: false,
            snapper_config: config.chars().take(64).collect(),
            number: None,
            error: Some("invalid snapper configuration name".into()),
        };
    }
    let output = Command::new(snapper)
        .args([
            "--no-dbus",
            "-c",
            config,
            "create",
            "--type",
            "single",
            "--cleanup-algorithm",
            "number",
            "--print-number",
            "--description",
            "cachyos-center: before system upgrade",
        ])
        .env_clear()
        .env("LC_ALL", "C")
        .env("PATH", "/usr/bin")
        .stdin(Stdio::null())
        .output();
    match output {
        Ok(out) if out.status.success() => {
            match parse_number(&String::from_utf8_lossy(&out.stdout)) {
                Some(number) => SnapshotResult {
                    created: true,
                    snapper_config: config.to_string(),
                    number: Some(number),
                    error: None,
                },
                None => SnapshotResult {
                    created: false,
                    snapper_config: config.to_string(),
                    number: None,
                    error: Some("snapper did not report a snapshot number".into()),
                },
            }
        }
        Ok(out) => SnapshotResult {
            created: false,
            snapper_config: config.to_string(),
            number: None,
            error: Some(
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .last()
                    .unwrap_or("snapper failed")
                    .to_string(),
            ),
        },
        Err(e) => SnapshotResult {
            created: false,
            snapper_config: config.to_string(),
            number: None,
            error: Some(format!("cannot run snapper: {e}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(parse_number("42\n"), Some(42));
        assert_eq!(parse_number("something\n17"), Some(17));
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("x"), None);
    }

    #[test]
    fn rejects_bad_config_names() {
        let r = create(Path::new("/bin/false"), "../root");
        assert!(!r.created);
        assert!(r.error.unwrap().contains("invalid"));
    }

    #[test]
    fn failing_snapper_is_not_success() {
        let r = create(Path::new("/bin/false"), "root");
        assert!(!r.created);
        assert!(r.number.is_none());
    }
}
