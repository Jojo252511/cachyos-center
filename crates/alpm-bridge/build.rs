//! Records the libalpm version the bridge is compiled against.
//!
//! The `alpm` crate itself refuses to build against an unsupported libalpm
//! (feature `checkver`). The recorded version is reported at runtime next to
//! the version of the library that is actually loaded.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    let version = Command::new("pkg-config")
        .args(["--modversion", "libalpm"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=CC_LIBALPM_BUILD_VERSION={version}");
}
