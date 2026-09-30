//! Generates the Tauri context and one permission per application command,
//! so that the capability file can grant exactly the commands the UI needs.

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&[
        "get_app_info",
        "get_dashboard",
        "get_system_info",
        "get_hyprland_info",
        "get_updates",
        "check_updates",
        "list_installed",
        "search_packages",
        "get_package_details",
        "list_repositories",
        "get_health",
        "get_news",
        "acknowledge_news",
        "get_activity",
        "get_settings",
        "save_settings",
        "get_auto_update_status",
        "get_diagnostic_report",
        "get_mcp_setup",
        "plan_install",
        "plan_remove",
        "start_upgrade",
        "start_install",
        "start_remove",
        "set_auto_update_policy",
        "cancel_operation",
        "get_operation",
        "get_operation_log",
        "get_current_operation",
        "open_external",
        "reveal_config_file",
        "write_clipboard",
    ]);
    if let Err(e) = tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)) {
        panic!("tauri build failed: {e:#}");
    }
}
