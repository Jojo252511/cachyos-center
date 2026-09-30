//! cachyos-center desktop application (Tauri 2).
//!
//! `cachyos-center` starts the GUI. `cachyos-center --notify-timer-status`
//! runs headless (user notification unit) and never opens a window.

// Release builds on Linux do not need a console; keeps the binary a normal app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod background;
mod commands;
mod helper_client;
mod links;
mod notify;
mod state;
mod timer_notify;

use tauri::Manager;

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("CACHYOS_CENTER_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .try_init();
}

/// WebKitGTK's DMABUF renderer fails on Wayland with the proprietary NVIDIA
/// driver ("Error 71 (Protocol error) dispatching to Wayland display",
/// reproduced on CachyOS/Hyprland with an RTX 50 series card). Disable it in
/// that case unless the user decided otherwise.
#[allow(unsafe_code)]
fn apply_webkit_workarounds() {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let nvidia = std::path::Path::new("/proc/driver/nvidia/version").exists();
    if wayland && nvidia && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: called at the very start of `main`, before any thread exists.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}

fn main() {
    apply_webkit_workarounds();
    init_logging();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--notify-timer-status") {
        std::process::exit(timer_notify::run());
    }
    if args.iter().any(|a| a == "--version") {
        eprintln!("cachyos-center {}", cachyos_center_core::APP_VERSION);
        return;
    }

    let core = match cachyos_center_service::AppCore::from_env() {
        Ok(core) => core,
        Err(e) => {
            eprintln!("cachyos-center cannot start: {e}");
            std::process::exit(1);
        }
    };

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(state::AppState::new(core))
        .setup(|app| {
            background::start(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_dashboard,
            commands::get_system_info,
            commands::get_hyprland_info,
            commands::get_updates,
            commands::check_updates,
            commands::list_installed,
            commands::search_packages,
            commands::get_package_details,
            commands::list_repositories,
            commands::get_health,
            commands::get_news,
            commands::acknowledge_news,
            commands::get_activity,
            commands::get_settings,
            commands::save_settings,
            commands::get_auto_update_status,
            commands::get_diagnostic_report,
            commands::get_mcp_setup,
            commands::plan_install,
            commands::plan_remove,
            commands::start_upgrade,
            commands::start_install,
            commands::start_remove,
            commands::set_auto_update_policy,
            commands::cancel_operation,
            commands::get_operation,
            commands::get_operation_log,
            commands::get_current_operation,
            commands::open_external,
            commands::reveal_config_file,
            commands::write_clipboard,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("cachyos-center failed: {e}");
        std::process::exit(1);
    }
}
