//! Read-only Hyprland IPC.
//!
//! The socket is discovered only through `HYPRLAND_INSTANCE_SIGNATURE` and
//! `XDG_RUNTIME_DIR` (no blind connection to arbitrary paths). Every request
//! uses a new connection with a timeout that is closed right after the
//! answer. Changes are observed through the event socket instead of polling.
//! Monitor serial numbers are never read into the model.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cachyos_center_core::hyprland::{HyprMonitor, HyprUnavailableReason, HyprlandInfo};
use cachyos_center_core::{APP_ID, Timestamp};
use serde_json::Value;

/// Timeout of a single IPC request.
pub const IPC_TIMEOUT: Duration = Duration::from_millis(800);
/// Maximum accepted answer size.
const MAX_ANSWER: u64 = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct HyprIpc {
    dir: PathBuf,
    timeout: Duration,
}

fn valid_signature(sig: &str) -> bool {
    !sig.is_empty()
        && sig.len() <= 200
        && sig.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn is_own_socket(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            meta.file_type().is_socket() && meta.uid() == rustix::process::getuid().as_raw()
        }
        Err(_) => false,
    }
}

impl HyprIpc {
    /// Discovers the IPC directory of the current session.
    pub fn discover() -> Result<Self, HyprUnavailableReason> {
        Self::discover_with(|k| std::env::var(k).ok())
    }

    pub fn discover_with(
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, HyprUnavailableReason> {
        let sig = env("HYPRLAND_INSTANCE_SIGNATURE").ok_or(HyprUnavailableReason::NoSession)?;
        if !valid_signature(&sig) {
            return Err(HyprUnavailableReason::SocketMissing);
        }
        let runtime = env("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or(HyprUnavailableReason::SocketMissing)?;
        let dir = runtime.join("hypr").join(sig);
        if !is_own_socket(&dir.join(".socket.sock")) {
            return Err(HyprUnavailableReason::SocketMissing);
        }
        Ok(Self {
            dir,
            timeout: IPC_TIMEOUT,
        })
    }

    /// Path of the event socket (`.socket2.sock`).
    pub fn event_socket(&self) -> PathBuf {
        self.dir.join(".socket2.sock")
    }

    /// Sends one request (e.g. `j/monitors`) and returns the answer.
    pub fn request(&self, command: &str) -> Result<String, HyprUnavailableReason> {
        let mut stream = UnixStream::connect(self.dir.join(".socket.sock"))
            .map_err(|_| HyprUnavailableReason::SocketMissing)?;
        stream
            .set_read_timeout(Some(self.timeout))
            .and_then(|()| stream.set_write_timeout(Some(self.timeout)))
            .map_err(|_| HyprUnavailableReason::SocketMissing)?;
        stream
            .write_all(command.as_bytes())
            .map_err(|_| HyprUnavailableReason::Timeout)?;
        let mut answer = String::new();
        stream
            .take(MAX_ANSWER)
            .read_to_string(&mut answer)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
                    HyprUnavailableReason::Timeout
                }
                _ => HyprUnavailableReason::InvalidResponse,
            })?;
        Ok(answer)
    }

    fn json(&self, command: &str) -> Result<Value, HyprUnavailableReason> {
        let answer = self.request(command)?;
        serde_json::from_str(&answer).map_err(|_| HyprUnavailableReason::InvalidResponse)
    }

    /// Opens the event socket for line based events (`event>>data`).
    pub fn events(&self) -> std::io::Result<impl Iterator<Item = String>> {
        let stream = UnixStream::connect(self.event_socket())?;
        Ok(BufReader::new(stream).lines().map_while(Result::ok))
    }
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Builds the model from the JSON answers (no serial numbers).
pub fn info_from(
    version: &Value,
    monitors: &Value,
    active: &Value,
    workspaces: &Value,
    now: Timestamp,
) -> HyprlandInfo {
    let monitors = monitors
        .as_array()
        .map(|list| {
            list.iter()
                .filter(|m| !m.get("disabled").and_then(Value::as_bool).unwrap_or(false))
                .map(|m| {
                    let make = str_of(m, "make").unwrap_or_default();
                    let model = str_of(m, "model").unwrap_or_default();
                    HyprMonitor {
                        name: str_of(m, "name").unwrap_or_default(),
                        description: format!("{make} {model}").trim().to_string(),
                        width: m.get("width").and_then(Value::as_u64).unwrap_or(0) as u32,
                        height: m.get("height").and_then(Value::as_u64).unwrap_or(0) as u32,
                        refresh_rate: m.get("refreshRate").and_then(Value::as_f64).unwrap_or(0.0)
                            as f32,
                        scale: m.get("scale").and_then(Value::as_f64).unwrap_or(1.0) as f32,
                        focused: m.get("focused").and_then(Value::as_bool).unwrap_or(false),
                        active_workspace: m.get("activeWorkspace").and_then(|w| str_of(w, "name")),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    HyprlandInfo {
        available: true,
        reason: None,
        version: str_of(version, "version").or_else(|| str_of(version, "tag")),
        monitors,
        active_workspace: str_of(active, "name"),
        workspace_count: workspaces.as_array().map(|a| a.len() as u32),
        app_class: APP_ID.to_string(),
        collected_at: now,
    }
}

/// Current Hyprland information or the reason why it is unavailable.
pub fn info() -> HyprlandInfo {
    let now = cachyos_center_core::now();
    let ipc = match HyprIpc::discover() {
        Ok(ipc) => ipc,
        Err(reason) => return HyprlandInfo::unavailable(reason, now),
    };
    let fetch = || -> Result<HyprlandInfo, HyprUnavailableReason> {
        Ok(info_from(
            &ipc.json("j/version")?,
            &ipc.json("j/monitors")?,
            &ipc.json("j/activeworkspace")?,
            &ipc.json("j/workspaces")?,
            now,
        ))
    };
    fetch().unwrap_or_else(|reason| HyprlandInfo::unavailable(reason, now))
}

/// Events that change the displayed information.
pub fn is_relevant_event(line: &str) -> bool {
    let name = line.split_once(">>").map(|(n, _)| n).unwrap_or(line);
    matches!(
        name,
        "workspace"
            | "workspacev2"
            | "focusedmon"
            | "focusedmonv2"
            | "monitoradded"
            | "monitoraddedv2"
            | "monitorremoved"
            | "monitorremovedv2"
            | "createworkspace"
            | "createworkspacev2"
            | "destroyworkspace"
            | "destroyworkspacev2"
            | "configreloaded"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn no_session_without_signature() {
        let r = HyprIpc::discover_with(|_| None);
        assert_eq!(r.unwrap_err(), HyprUnavailableReason::NoSession);
    }

    #[test]
    fn rejects_path_like_signatures() {
        let r = HyprIpc::discover_with(|k| match k {
            "HYPRLAND_INSTANCE_SIGNATURE" => Some("../../etc".into()),
            "XDG_RUNTIME_DIR" => Some("/run/user/1000".into()),
            _ => None,
        });
        assert_eq!(r.unwrap_err(), HyprUnavailableReason::SocketMissing);
    }

    #[test]
    fn missing_socket() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().to_string_lossy().into_owned();
        let r = HyprIpc::discover_with(|k| match k {
            "HYPRLAND_INSTANCE_SIGNATURE" => Some("abc_123".into()),
            "XDG_RUNTIME_DIR" => Some(runtime.clone()),
            _ => None,
        });
        assert_eq!(r.unwrap_err(), HyprUnavailableReason::SocketMissing);
    }

    #[test]
    fn talks_to_a_fake_socket() {
        let dir = tempfile::tempdir().unwrap();
        let hypr = dir.path().join("hypr/abc");
        std::fs::create_dir_all(&hypr).unwrap();
        let listener = std::os::unix::net::UnixListener::bind(hypr.join(".socket.sock")).unwrap();
        let server = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut buf = [0u8; 64];
            let n = conn.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"j/version");
            conn.write_all(br#"{"version":"0.56.2"}"#).unwrap();
        });
        let runtime = dir.path().to_string_lossy().into_owned();
        let ipc = HyprIpc::discover_with(|k| match k {
            "HYPRLAND_INSTANCE_SIGNATURE" => Some("abc".into()),
            "XDG_RUNTIME_DIR" => Some(runtime.clone()),
            _ => None,
        })
        .unwrap();
        let v = ipc.json("j/version").unwrap();
        assert_eq!(v["version"], "0.56.2");
        server.join().unwrap();
    }

    #[test]
    fn model_without_serials() {
        let info = info_from(
            &json!({"version": "0.56.2"}),
            &json!([{"name": "DP-1", "make": "Xiaomi Corporation", "model": "Mi monitor", "serial": "5505610195415",
                     "description": "Xiaomi Corporation Mi monitor 5505610195415",
                     "width": 3440, "height": 1440, "refreshRate": 180.0, "scale": 1.0, "focused": true,
                     "activeWorkspace": {"id": 3, "name": "3"}},
                    {"name": "HDMI-A-1", "disabled": true}]),
            &json!({"id": 3, "name": "3"}),
            &json!([{}, {}, {}]),
            1,
        );
        assert!(info.available);
        assert_eq!(info.monitors.len(), 1);
        assert_eq!(
            info.monitors[0].description,
            "Xiaomi Corporation Mi monitor"
        );
        assert!(!format!("{info:?}").contains("5505610195415"));
        assert_eq!(info.active_workspace.as_deref(), Some("3"));
        assert_eq!(info.workspace_count, Some(3));
        assert_eq!(info.app_class, APP_ID);
    }

    #[test]
    fn relevant_events() {
        assert!(is_relevant_event("workspace>>3"));
        assert!(is_relevant_event("monitoraddedv2>>1,DP-1,desc"));
        assert!(!is_relevant_event("activewindow>>kitty,title"));
        assert!(!is_relevant_event("openwindow>>x"));
    }
}
