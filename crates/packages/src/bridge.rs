//! Runtime loader for the libalpm bridge (`libcachyos_center_alpm.so`).
//!
//! The binaries of cachyos-center do not link libalpm. The bridge is opened
//! with `dlopen` on first use. If libalpm is missing or has another soname
//! (after a pacman update), loading fails and every package call returns an
//! [`ErrorCode::Unsupported`] error with an explanation, while system
//! information, settings and the rest of the UI keep working.

// dlopen and calls through foreign function pointers are inherently unsafe;
// the unsafe code is confined to this module.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use cachyos_center_core::bridge::{BRIDGE_PROTOCOL, BridgeInfo, BridgeRequest, BridgeResponse};
use cachyos_center_core::paths::{ALPM_BRIDGE_FILE, LIBEXEC_DIR};
use cachyos_center_core::system::BackendStatus;
use cachyos_center_core::{AppError, AppResult, ErrorCode};
use serde::de::DeserializeOwned;

/// Environment variable to point unprivileged development builds to a bridge.
pub const BRIDGE_ENV: &str = "CACHYOS_CENTER_ALPM_BRIDGE";

type ProtocolFn = unsafe extern "C" fn() -> u32;
type CallFn = unsafe extern "C" fn(*const c_char) -> *mut c_char;
type FreeFn = unsafe extern "C" fn(*mut c_char);

struct Loaded {
    // Keeps the library mapped for the lifetime of the process.
    _lib: libloading::Library,
    call: CallFn,
    free: FreeFn,
    path: PathBuf,
}

// SAFETY: the bridge serializes access to libalpm internally (mutex around the
// cached handle); the function pointers themselves are plain code addresses.
unsafe impl Send for Loaded {}
unsafe impl Sync for Loaded {}

static BRIDGE: OnceLock<Result<Loaded, String>> = OnceLock::new();

fn is_root() -> bool {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() == 0 }
}

/// Candidate locations of the bridge, in search order.
pub fn candidates() -> Vec<PathBuf> {
    let mut list = Vec::new();
    let privileged = is_root();
    if !privileged && let Some(p) = std::env::var_os(BRIDGE_ENV) {
        list.push(PathBuf::from(p));
    }
    // Development builds: next to the executable (`target/debug`) or one level
    // up for test binaries (`target/debug/deps`). Release builds running as
    // root only accept the installed location.
    if (!privileged || cfg!(debug_assertions))
        && let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        list.push(dir.join(ALPM_BRIDGE_FILE));
        if let Some(parent) = dir.parent() {
            list.push(parent.join(ALPM_BRIDGE_FILE));
        }
    }
    list.push(Path::new(LIBEXEC_DIR).join(ALPM_BRIDGE_FILE));
    list
}

fn load_from(path: &Path) -> Result<Loaded, String> {
    // SAFETY: loading a library runs its initializers. Only the bridge built
    // from this repository is expected at these locations; release builds
    // running as root only accept the root-owned installation directory.
    let lib = unsafe { libloading::Library::new(path) }.map_err(|e| e.to_string())?;
    // SAFETY: the symbols are declared with exactly these signatures in the bridge.
    let (protocol, call, free) = unsafe {
        let protocol: ProtocolFn = *lib
            .get::<ProtocolFn>(b"cc_alpm_bridge_protocol\0")
            .map_err(|e| e.to_string())?;
        let call: CallFn = *lib
            .get::<CallFn>(b"cc_alpm_bridge_call\0")
            .map_err(|e| e.to_string())?;
        let free: FreeFn = *lib
            .get::<FreeFn>(b"cc_alpm_bridge_free\0")
            .map_err(|e| e.to_string())?;
        (protocol, call, free)
    };
    // SAFETY: no arguments, returns a constant.
    let version = unsafe { protocol() };
    if version != BRIDGE_PROTOCOL {
        return Err(format!(
            "bridge protocol {version} does not match {BRIDGE_PROTOCOL}"
        ));
    }
    Ok(Loaded {
        _lib: lib,
        call,
        free,
        path: path.to_path_buf(),
    })
}

fn load() -> Result<Loaded, String> {
    let mut errors = Vec::new();
    for candidate in candidates() {
        if !candidate.exists() {
            continue;
        }
        match load_from(&candidate) {
            Ok(loaded) => return Ok(loaded),
            Err(e) => errors.push(format!("{}: {e}", candidate.display())),
        }
    }
    if errors.is_empty() {
        Err(format!(
            "{ALPM_BRIDGE_FILE} was not found (searched next to the executable and in {LIBEXEC_DIR})"
        ))
    } else {
        Err(errors.join("; "))
    }
}

fn loaded() -> Result<&'static Loaded, AppError> {
    BRIDGE
        .get_or_init(load)
        .as_ref()
        .map_err(|reason| AppError::new(ErrorCode::Unsupported, unavailable_text(reason)))
}

fn unavailable_text(reason: &str) -> String {
    if reason.contains("libalpm.so") {
        format!(
            "package functions are disabled: the installed libalpm is not compatible with this build of cachyos-center ({reason})"
        )
    } else {
        format!("package functions are disabled: the libalpm bridge could not be loaded ({reason})")
    }
}

/// Sends a raw request and returns the raw JSON value.
fn call_raw(request: &BridgeRequest) -> AppResult<serde_json::Value> {
    let bridge = loaded()?;
    let text = serde_json::to_string(request)
        .map_err(|e| AppError::internal(format!("cannot encode bridge request: {e}")))?;
    let c_request =
        CString::new(text).map_err(|_| AppError::invalid("bridge request contains NUL"))?;
    // SAFETY: `c_request` is a valid C string; the returned pointer is owned
    // by the bridge and released with its own `free` function below.
    let response_ptr = unsafe { (bridge.call)(c_request.as_ptr()) };
    if response_ptr.is_null() {
        return Err(AppError::internal("bridge returned no response"));
    }
    // SAFETY: the bridge returns a NUL-terminated string.
    let response = unsafe { CStr::from_ptr(response_ptr) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: pointer originates from `call` and is freed exactly once.
    unsafe { (bridge.free)(response_ptr) };
    let parsed: BridgeResponse = serde_json::from_str(&response)
        .map_err(|e| AppError::internal(format!("malformed bridge response: {e}")))?;
    match parsed {
        BridgeResponse::Ok(v) => Ok(v),
        BridgeResponse::Err(e) => Err(e),
    }
}

/// Typed call.
pub fn call<T: DeserializeOwned>(request: &BridgeRequest) -> AppResult<T> {
    let value = call_raw(request)?;
    serde_json::from_value(value)
        .map_err(|e| AppError::internal(format!("unexpected bridge result: {e}")))
}

/// Location of the loaded bridge (for diagnostics).
pub fn loaded_path() -> Option<PathBuf> {
    BRIDGE
        .get()
        .and_then(|r| r.as_ref().ok())
        .map(|l| l.path.clone())
}

/// Status for the UI: ready (with versions) or unavailable (with reason).
pub fn status() -> BackendStatus {
    match call::<BridgeInfo>(&BridgeRequest::Info) {
        Ok(info) if info.compatible => BackendStatus::Ready {
            libalpm_version: info.libalpm_version,
            built_against: info.built_against,
        },
        Ok(info) => BackendStatus::Unavailable {
            reason: format!(
                "libalpm {} is not supported by this build (built against {})",
                info.libalpm_version, info.built_against
            ),
        },
        Err(e) => BackendStatus::Unavailable { reason: e.message },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_end_with_system_location() {
        let list = candidates();
        assert_eq!(
            list.last().unwrap(),
            &Path::new(LIBEXEC_DIR).join(ALPM_BRIDGE_FILE)
        );
    }

    #[test]
    fn unavailable_texts() {
        assert!(unavailable_text("libalpm.so.17: cannot open shared object file").contains("not compatible"));
        assert!(unavailable_text("not found").contains("could not be loaded"));
    }
}
