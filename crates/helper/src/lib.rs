//! Privileged helper of cachyos-center.
//!
//! Runs as a D-Bus activated system service (`org.cachyos_center.Packages1`)
//! and offers a closed set of package operations. It never executes free
//! commands, never uses a shell and never removes the pacman lock. The same
//! binary is the entry point of the scheduled preflight timer.

pub mod authz;
pub mod config;
pub mod dbus;
pub mod engine;
pub mod inhibit;
pub mod journal;
pub mod policy;
pub mod preflight;
pub mod runner;
pub mod snapshot;

use std::sync::Arc;
use std::time::Duration;

use cachyos_center_core::dbus::{BUS_NAME, OBJECT_PATH};
use cachyos_center_core::{AppError, AppResult};
use cachyos_center_packages::PackageService;

use crate::config::{BusKind, HelperConfig};
use crate::engine::Engine;

/// Package service for the helper: productive database, or the sandbox
/// configuration given in `config.pacman_conf`.
pub fn package_service(config: &HelperConfig) -> AppResult<PackageService> {
    let check_db = config.state_dir.join("checkup-db");
    let check_state = config.state_dir.join("update-check.json");
    match &config.pacman_conf {
        None => Ok(PackageService::new(check_db, check_state)),
        Some(conf) => {
            let alpm = cachyos_center_packages::config::load(Some(conf))?;
            Ok(PackageService::with_parts(
                alpm,
                check_db,
                check_state,
                config.pacman_log.clone(),
                cachyos_center_packages::context::current(),
            ))
        }
    }
}

/// Serves the D-Bus interface until the helper has been idle long enough.
pub async fn serve(config: HelperConfig) -> AppResult<()> {
    for dir in [&config.state_dir, &config.log_dir, &config.journal_dir()] {
        std::fs::create_dir_all(dir)?;
    }
    let packages = package_service(&config)?;
    let engine = Engine::new(config.clone(), packages);
    engine.recover();
    let builder = match config.bus {
        BusKind::System => zbus::connection::Builder::system(),
        BusKind::Session => zbus::connection::Builder::session(),
    }
    .map_err(|e| AppError::unavailable(format!("cannot connect to the bus: {e}")))?;
    let conn = builder
        .serve_at(
            OBJECT_PATH,
            dbus::Packages {
                engine: Arc::clone(&engine),
            },
        )
        .and_then(|b| b.name(BUS_NAME))
        .map_err(|e| AppError::unavailable(format!("cannot export the interface: {e}")))?
        .build()
        .await
        .map_err(|e| AppError::unavailable(format!("cannot acquire {BUS_NAME}: {e}")))?;
    engine.set_bus(conn.clone());
    tracing::info!("cachyos-center-helper ready on {BUS_NAME}");
    loop {
        tokio::time::sleep(Duration::from_secs(15)).await;
        if engine.is_idle() {
            tracing::info!("idle, exiting (D-Bus activation restarts the helper on demand)");
            break;
        }
    }
    drop(conn);
    Ok(())
}
