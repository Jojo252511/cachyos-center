//! Tauri commands (see `docs/ipc-vertrag.md`).
//!
//! Every command validates its input again on the Rust side. Nothing here can
//! change packages directly: changing commands forward typed requests to the
//! privileged helper, which authorizes them with polkit.

use std::sync::Arc;

use cachyos_center_core::dashboard::Dashboard;
use cachyos_center_core::health::HealthReport;
use cachyos_center_core::history::HistoryEntry;
use cachyos_center_core::hyprland::HyprlandInfo;
use cachyos_center_core::mcp;
use cachyos_center_core::news::NewsStatus;
use cachyos_center_core::operation::Operation;
use cachyos_center_core::package::{
    CatalogQuery, InstalledQuery, PackagePage, PackageRecord, PackageRef, PackageSummary,
    RepositoryInfo,
};
use cachyos_center_core::plan::TransactionPlan;
use cachyos_center_core::policy::{AutoUpdateConfig, AutoUpdateStatus};
use cachyos_center_core::settings::Settings;
use cachyos_center_core::system::SystemInfo;
use cachyos_center_core::ui::{AppInfo, McpSetup, OperationLogChunk};
use cachyos_center_core::updates::UpdateCheckResult;
use cachyos_center_core::{APP_ID, APP_VERSION, AppError, AppResult, validate};
use cachyos_center_service::{AppCore, ReadApi};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

use crate::state::AppState;
use crate::{background, helper_client, links};

type CmdResult<T> = Result<T, AppError>;

/// Runs blocking work of the application core on the blocking thread pool.
async fn blocking<T: Send + 'static>(
    core: &Arc<AppCore>,
    f: impl FnOnce(&AppCore) -> AppResult<T> + Send + 'static,
) -> CmdResult<T> {
    let core = Arc::clone(core);
    tauri::async_runtime::spawn_blocking(move || f(&core))
        .await
        .map_err(|e| AppError::internal(format!("worker failed: {e}")))?
}

fn system_language() -> String {
    let value = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default();
    if value.is_empty() || value.starts_with("de") {
        "de".into()
    } else {
        "en".into()
    }
}

/// Color scheme from the XDG desktop portal (`dark`, `light`, `unknown`).
async fn system_color_scheme() -> String {
    let read = async {
        let conn = zbus::Connection::session().await.ok()?;
        let proxy = zbus::Proxy::new(
            &conn,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        )
        .await
        .ok()?;
        let value: zbus::zvariant::OwnedValue = proxy
            .call("ReadOne", &("org.freedesktop.appearance", "color-scheme"))
            .await
            .ok()?;
        u32::try_from(value).ok()
    };
    match tokio::time::timeout(std::time::Duration::from_millis(800), read).await {
        Ok(Some(1)) => "dark".into(),
        Ok(Some(2)) => "light".into(),
        _ => "unknown".into(),
    }
}

#[tauri::command]
pub async fn get_app_info(state: State<'_, AppState>) -> CmdResult<AppInfo> {
    let helper = state.helper.available().await;
    let backend = blocking(&state.core, |core| Ok(core.packages().backend_status())).await?;
    Ok(AppInfo {
        version: APP_VERSION.to_string(),
        app_id: APP_ID.to_string(),
        system_language: system_language(),
        system_color_scheme: system_color_scheme().await,
        helper_available: helper.is_ok(),
        helper_error: helper.err(),
        backend,
        development_mode: helper_client::development_mode(),
    })
}

#[tauri::command]
pub async fn get_dashboard(state: State<'_, AppState>) -> CmdResult<Dashboard> {
    blocking(&state.core, |core| Ok(core.dashboard())).await
}

#[tauri::command]
pub async fn get_system_info(state: State<'_, AppState>) -> CmdResult<SystemInfo> {
    blocking(&state.core, |core| Ok(core.system_info())).await
}

#[tauri::command]
pub async fn get_hyprland_info(state: State<'_, AppState>) -> CmdResult<HyprlandInfo> {
    blocking(&state.core, |core| Ok(core.hyprland())).await
}

#[tauri::command]
pub async fn get_updates(state: State<'_, AppState>) -> CmdResult<UpdateCheckResult> {
    blocking(&state.core, |core| core.updates()).await
}

#[tauri::command]
pub async fn check_updates(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<UpdateCheckResult> {
    let result = blocking(&state.core, |core| Ok(core.check_updates())).await?;
    let _ = app.emit("updates-checked", &result);
    Ok(result)
}

#[tauri::command]
pub async fn list_installed(
    state: State<'_, AppState>,
    query: InstalledQuery,
) -> CmdResult<PackagePage> {
    blocking(&state.core, move |core| core.installed(&query)).await
}

#[tauri::command]
pub async fn search_packages(
    state: State<'_, AppState>,
    query: CatalogQuery,
) -> CmdResult<Vec<PackageSummary>> {
    blocking(&state.core, move |core| core.search(&query)).await
}

#[tauri::command]
pub async fn get_package_details(
    state: State<'_, AppState>,
    reference: PackageRef,
) -> CmdResult<PackageRecord> {
    blocking(&state.core, move |core| core.details(&reference)).await
}

#[tauri::command]
pub async fn list_repositories(state: State<'_, AppState>) -> CmdResult<Vec<RepositoryInfo>> {
    blocking(&state.core, |core| core.repositories()).await
}

#[tauri::command]
pub async fn get_health(state: State<'_, AppState>) -> CmdResult<HealthReport> {
    blocking(&state.core, |core| Ok(core.health_report())).await
}

#[tauri::command]
pub async fn get_news(
    state: State<'_, AppState>,
    refresh: bool,
    force: bool,
) -> CmdResult<NewsStatus> {
    blocking(&state.core, move |core| Ok(core.news(refresh, force))).await
}

#[tauri::command]
pub async fn acknowledge_news(state: State<'_, AppState>, until: i64) -> CmdResult<NewsStatus> {
    if until < 0 {
        return Err(AppError::invalid("invalid timestamp"));
    }
    blocking(&state.core, move |core| core.acknowledge_news(until)).await
}

#[tauri::command]
pub async fn get_activity(state: State<'_, AppState>, limit: u32) -> CmdResult<Vec<HistoryEntry>> {
    let limit = validate::limit(Some(limit), 50, 1, 200)?;
    blocking(&state.core, move |core| Ok(core.activity(limit))).await
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> CmdResult<Settings> {
    blocking(&state.core, |core| Ok(core.settings())).await
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, settings: Settings) -> CmdResult<Settings> {
    blocking(&state.core, move |core| {
        core.save_settings(&settings)?;
        let _ = core.prune();
        Ok(core.settings())
    })
    .await
}

#[tauri::command]
pub async fn get_auto_update_status(state: State<'_, AppState>) -> CmdResult<AutoUpdateStatus> {
    let mut status = blocking(&state.core, |core| Ok(core.auto_update_status())).await?;
    refine_helper_availability(&state, &mut status).await;
    Ok(status)
}

async fn refine_helper_availability(state: &State<'_, AppState>, status: &mut AutoUpdateStatus) {
    let available = state.helper.available().await.is_ok();
    status.helper_available = available;
    let missing = cachyos_center_system::updaters::blocker::HELPER_MISSING.to_string();
    if available {
        status.prepare_mode_blockers.retain(|b| *b != missing);
    } else if !status.prepare_mode_blockers.contains(&missing) {
        status.prepare_mode_blockers.insert(0, missing);
    }
}

#[tauri::command]
pub async fn get_diagnostic_report(state: State<'_, AppState>) -> CmdResult<String> {
    blocking(&state.core, |core| Ok(core.diagnostic_report())).await
}

fn mcp_binary() -> Option<String> {
    let installed = std::path::Path::new("/usr/bin").join(mcp::BINARY_NAME);
    if installed.is_file() {
        return Some(installed.display().to_string());
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join(mcp::BINARY_NAME)))
        .filter(|p| p.is_file())
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub async fn get_mcp_setup(state: State<'_, AppState>) -> CmdResult<McpSetup> {
    let settings = blocking(&state.core, |core| Ok(core.settings())).await?;
    let binary = mcp_binary();
    let path = binary
        .clone()
        .unwrap_or_else(|| format!("/usr/bin/{}", mcp::BINARY_NAME));
    Ok(McpSetup {
        enabled: settings.mcp_enabled,
        binary_path: binary,
        host_config: mcp::host_config(&path),
        tools: mcp::TOOL_NAMES.iter().map(|s| (*s).to_string()).collect(),
    })
}

// ---- plans --------------------------------------------------------------------

#[tauri::command]
pub async fn plan_install(
    state: State<'_, AppState>,
    repository: String,
    name: String,
) -> CmdResult<TransactionPlan> {
    validate::repo_name(&repository)?;
    validate::package_name(&name)?;
    blocking(&state.core, move |core| {
        core.plan_install(&repository, &name)
    })
    .await
}

#[tauri::command]
pub async fn plan_remove(
    state: State<'_, AppState>,
    name: String,
    recursive: bool,
) -> CmdResult<TransactionPlan> {
    validate::package_name(&name)?;
    blocking(&state.core, move |core| core.plan_remove(&name, recursive)).await
}

// ---- helper operations --------------------------------------------------------

#[tauri::command]
pub async fn start_upgrade(
    app: AppHandle,
    state: State<'_, AppState>,
    plan_digest: String,
    create_snapshot: bool,
) -> CmdResult<String> {
    validate::plan_digest(&plan_digest)?;
    let id = state.helper.upgrade(&plan_digest, create_snapshot).await?;
    background::watch_operation(&app, &state, id.clone());
    Ok(id)
}

#[tauri::command]
pub async fn start_install(
    app: AppHandle,
    state: State<'_, AppState>,
    repository: String,
    name: String,
    plan_digest: String,
) -> CmdResult<String> {
    validate::repo_name(&repository)?;
    validate::package_name(&name)?;
    validate::plan_digest(&plan_digest)?;
    let id = state
        .helper
        .install(&repository, &name, &plan_digest)
        .await?;
    background::watch_operation(&app, &state, id.clone());
    Ok(id)
}

#[tauri::command]
pub async fn start_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    recursive: bool,
    plan_digest: String,
) -> CmdResult<String> {
    validate::package_name(&name)?;
    validate::plan_digest(&plan_digest)?;
    let id = state.helper.remove(&name, recursive, &plan_digest).await?;
    background::watch_operation(&app, &state, id.clone());
    Ok(id)
}

#[tauri::command]
pub async fn set_auto_update_policy(
    state: State<'_, AppState>,
    config: AutoUpdateConfig,
) -> CmdResult<AutoUpdateStatus> {
    config.validate()?;
    state.helper.set_policy(&config).await?;
    get_auto_update_status(state).await
}

#[tauri::command]
pub async fn cancel_operation(state: State<'_, AppState>, id: String) -> CmdResult<Operation> {
    validate::operation_id(&id)?;
    state.helper.cancel(&id).await
}

#[tauri::command]
pub async fn get_operation(state: State<'_, AppState>, id: String) -> CmdResult<Operation> {
    validate::operation_id(&id)?;
    match state.helper.status(&id).await {
        Ok(op) => Ok(op),
        // Finished operations are also available without the helper.
        Err(e) => blocking(&state.core, move |core| core.history().get(&id)?.ok_or(e)).await,
    }
}

#[tauri::command]
pub async fn get_operation_log(
    state: State<'_, AppState>,
    id: String,
    offset: u64,
) -> CmdResult<OperationLogChunk> {
    validate::operation_id(&id)?;
    state.helper.log(&id, offset).await
}

#[tauri::command]
pub async fn get_current_operation(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Option<Operation>> {
    // The journal is world-readable: only contact (and thereby start) the
    // helper when an operation is still running.
    let latest = blocking(&state.core, |_| {
        Ok(cachyos_center_service::activity::read_journal(
            std::path::Path::new(cachyos_center_core::paths::SYSTEM_OPERATIONS_DIR),
            1,
        )
        .into_iter()
        .next())
    })
    .await?;
    let running = latest.as_ref().is_some_and(|op| !op.state.is_terminal());
    if running || helper_client::development_mode() {
        let current = state.helper.current().await?;
        if let Some(op) = &current
            && !op.state.is_terminal()
        {
            background::watch_operation(&app, &state, op.id.clone());
        }
        return Ok(current);
    }
    Ok(latest)
}

// ---- other actions ------------------------------------------------------------

#[tauri::command]
pub async fn open_external(app: AppHandle, url: String) -> CmdResult<()> {
    links::validate_url(&url)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::unavailable(format!("cannot open the browser: {e}")))
}

#[tauri::command]
pub async fn reveal_config_file(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> CmdResult<()> {
    let known = blocking(&state.core, |_| {
        Ok(
            cachyos_center_system::health::config_files(std::path::Path::new("/etc"))
                .into_iter()
                .map(|f| f.path)
                .collect::<Vec<_>>(),
        )
    })
    .await?;
    links::validate_config_file(&path, &known)?;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| AppError::unavailable(format!("cannot open the file manager: {e}")))
}

#[tauri::command]
pub async fn write_clipboard(app: AppHandle, text: String) -> CmdResult<()> {
    if text.len() > 1024 * 1024 {
        return Err(AppError::invalid("text is too large for the clipboard"));
    }
    app.clipboard()
        .write_text(text)
        .map_err(|e| AppError::unavailable(format!("clipboard not available: {e}")))
}
