//! libalpm bridge of cachyos-center.
//!
//! This library is the only part of cachyos-center that links libalpm. The
//! binaries load it with `dlopen` (see `cachyos-center-packages`), so a
//! missing or incompatible libalpm disables package functions instead of
//! preventing the application from starting.
//!
//! The bridge is strictly read-only: it opens libalpm handles, reads the local
//! and sync databases and computes transaction plans with
//! `ALPM_TRANS_FLAG_NOLOCK` (like `pacman -Sp`). It never commits a
//! transaction, never refreshes a database and never takes the database lock.
//!
//! C ABI:
//! * `cc_alpm_bridge_protocol() -> u32`
//! * `cc_alpm_bridge_call(request_json) -> response_json` (caller frees)
//! * `cc_alpm_bridge_free(response_json)`

// The exported functions form an FFI boundary. Unsafe code is limited to this
// file and to the `Send` wrapper of the cached handle in `handle.rs`.
#![allow(unsafe_code)]

mod handle;
mod plan;
mod query;
mod siglevel;

use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};

use cachyos_center_core::bridge::{
    BRIDGE_PROTOCOL, BridgeInfo, BridgeRequest, BridgeResponse, SUPPORTED_LIBALPM_MAJOR,
};
use cachyos_center_core::{AppError, AppResult, ErrorCode};

/// Largest accepted request document (1 MiB).
const MAX_REQUEST_LEN: usize = 1024 * 1024;

/// Version of libalpm the bridge was compiled against.
pub const BUILT_AGAINST: &str = env!("CC_LIBALPM_BUILD_VERSION");

/// Information about the loaded libalpm and its compatibility.
pub fn info() -> BridgeInfo {
    let runtime = alpm::version().to_string();
    BridgeInfo {
        protocol: BRIDGE_PROTOCOL,
        compatible: is_compatible(&runtime),
        libalpm_version: runtime,
        built_against: BUILT_AGAINST.to_string(),
        bridge_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

fn is_compatible(runtime: &str) -> bool {
    runtime
        .split('.')
        .next()
        .and_then(|m| m.parse::<u32>().ok())
        .is_some_and(|major| major == SUPPORTED_LIBALPM_MAJOR)
}

/// Executes a request. Public for tests and in-process use.
pub fn handle_request(request: BridgeRequest) -> AppResult<serde_json::Value> {
    if !matches!(request, BridgeRequest::Info) {
        let info = info();
        if !info.compatible {
            return Err(AppError::new(
                ErrorCode::Unsupported,
                format!(
                    "libalpm {} is not supported (supported major version: {SUPPORTED_LIBALPM_MAJOR})",
                    info.libalpm_version
                ),
            ));
        }
    }
    let value = match request {
        BridgeRequest::Info => serde_json::to_value(info()),
        BridgeRequest::Repositories { config } => {
            serde_json::to_value(handle::with_handle(&config, query::repositories)?)
        }
        BridgeRequest::ListInstalled { config } => {
            serde_json::to_value(handle::with_handle(&config, |alpm| {
                query::installed(alpm, &config)
            })?)
        }
        BridgeRequest::Search {
            config,
            query: q,
            repository,
            limit,
        } => serde_json::to_value(handle::with_handle(&config, |alpm| {
            query::search(alpm, &config, &q, repository.as_deref(), limit)
        })?),
        BridgeRequest::Details {
            config,
            name,
            repository,
            context,
        } => serde_json::to_value(handle::with_handle(&config, |alpm| {
            query::details(alpm, &config, &name, repository.as_deref(), &context)
        })?),
        BridgeRequest::Updates { config, context } => {
            serde_json::to_value(handle::with_handle(&config, |alpm| {
                query::updates(alpm, &config, &context)
            })?)
        }
        BridgeRequest::PlanInstall {
            config,
            repository,
            name,
            context,
        } => serde_json::to_value(handle::with_handle(&config, |alpm| {
            plan::install(alpm, &config, &repository, &name, &context)
        })?),
        BridgeRequest::PlanRemove {
            config,
            name,
            recursive,
            context,
        } => serde_json::to_value(handle::with_handle(&config, |alpm| {
            plan::remove(alpm, &config, &name, recursive, &context)
        })?),
    };
    value.map_err(|e| AppError::internal(format!("cannot serialize bridge result: {e}")))
}

fn dispatch(raw: &[u8]) -> BridgeResponse {
    if raw.len() > MAX_REQUEST_LEN {
        return BridgeResponse::Err(AppError::invalid("bridge request too large"));
    }
    let request: BridgeRequest = match serde_json::from_slice(raw) {
        Ok(r) => r,
        Err(e) => {
            return BridgeResponse::Err(AppError::invalid(format!(
                "malformed bridge request: {e}"
            )));
        }
    };
    match handle_request(request) {
        Ok(v) => BridgeResponse::Ok(v),
        Err(e) => BridgeResponse::Err(e),
    }
}

fn into_c_string(response: &BridgeResponse) -> *mut c_char {
    let text = serde_json::to_string(response).unwrap_or_else(|_| {
        r#"{"status":"err","value":{"code":"INTERNAL","message":"cannot encode response","detail":null}}"#
            .to_string()
    });
    // JSON never contains interior NUL bytes (they are escaped as \u0000).
    CString::new(text)
        .unwrap_or_else(|_| CString::from(c"{\"status\":\"err\",\"value\":{\"code\":\"INTERNAL\",\"message\":\"nul byte\",\"detail\":null}}"))
        .into_raw()
}

/// Protocol version of this bridge.
#[unsafe(no_mangle)]
pub extern "C" fn cc_alpm_bridge_protocol() -> u32 {
    BRIDGE_PROTOCOL
}

/// Executes a JSON request and returns a JSON response that must be released
/// with [`cc_alpm_bridge_free`]. Never unwinds across the FFI boundary.
///
/// # Safety
/// `request` must be a valid, NUL-terminated C string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_alpm_bridge_call(request: *const c_char) -> *mut c_char {
    if request.is_null() {
        return into_c_string(&BridgeResponse::Err(AppError::invalid("null request")));
    }
    // SAFETY: the caller guarantees a valid NUL-terminated string.
    let raw = unsafe { CStr::from_ptr(request) }.to_bytes().to_vec();
    let response = catch_unwind(AssertUnwindSafe(|| dispatch(&raw))).unwrap_or_else(|_| {
        handle::reset_cache();
        BridgeResponse::Err(AppError::internal("the libalpm bridge panicked"))
    });
    into_c_string(&response)
}

/// Releases a response returned by [`cc_alpm_bridge_call`].
///
/// # Safety
/// `ptr` must originate from [`cc_alpm_bridge_call`] and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_alpm_bridge_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        // SAFETY: `ptr` was created by `CString::into_raw` in this library.
        drop(unsafe { CString::from_raw(ptr) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_rule() {
        assert!(is_compatible("16.0.1"));
        assert!(is_compatible("16.2.0"));
        assert!(!is_compatible("15.0.0"));
        assert!(!is_compatible("17.0.0"));
        assert!(!is_compatible("garbage"));
    }

    #[test]
    fn ffi_roundtrip_info() {
        let req = CString::new(r#"{"type":"info"}"#).unwrap();
        let resp = unsafe { cc_alpm_bridge_call(req.as_ptr()) };
        let text = unsafe { CStr::from_ptr(resp) }
            .to_str()
            .unwrap()
            .to_string();
        unsafe { cc_alpm_bridge_free(resp) };
        let parsed: BridgeResponse = serde_json::from_str(&text).unwrap();
        match parsed {
            BridgeResponse::Ok(v) => {
                let info: BridgeInfo = serde_json::from_value(v).unwrap();
                assert_eq!(info.protocol, BRIDGE_PROTOCOL);
                assert!(info.compatible, "{info:?}");
            }
            BridgeResponse::Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn ffi_rejects_garbage() {
        let req = CString::new("{not json").unwrap();
        let resp = unsafe { cc_alpm_bridge_call(req.as_ptr()) };
        let text = unsafe { CStr::from_ptr(resp) }
            .to_str()
            .unwrap()
            .to_string();
        unsafe { cc_alpm_bridge_free(resp) };
        let parsed: BridgeResponse = serde_json::from_str(&text).unwrap();
        assert!(matches!(parsed, BridgeResponse::Err(e) if e.code == ErrorCode::InvalidInput));
        let null = unsafe { cc_alpm_bridge_call(std::ptr::null()) };
        assert!(!null.is_null());
        unsafe { cc_alpm_bridge_free(null) };
    }
}
