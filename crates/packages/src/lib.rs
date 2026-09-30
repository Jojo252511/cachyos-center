//! Package service of cachyos-center.
//!
//! * [`bridge`]  – loads the libalpm bridge at runtime and performs typed calls
//! * [`config`]  – reads the pacman configuration through `pacman-conf`
//! * [`check`]   – update check with an isolated sync database (`checkupdates`)
//! * [`pacman_log`] – parser for `/var/log/pacman.log`
//! * [`lock`]    – state of the pacman database lock
//! * [`service`] – [`PackageService`], the read-only facade used by GUI, MCP and helper
//!
//! Nothing in this crate modifies the system. Package changes are performed
//! exclusively by the privileged helper.

pub mod bridge;
pub mod check;
pub mod config;
pub mod context;
pub mod lock;
pub mod pacman_log;
pub mod service;

pub use service::PackageService;
