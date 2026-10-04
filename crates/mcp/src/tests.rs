//! Behavior tests of the server with a fake read service.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cachyos_center_core::health::{
    ConfigFileHint, HealthItem, HealthItemKind, HealthReport, RebootReason, Severity,
    SnapshotSupport, UpdateBlocker,
};
use cachyos_center_core::history::{HistoryEntry, HistorySource, LogOutcome};
use cachyos_center_core::mcp::{SERVER_NAME, TOOL_NAMES};
use cachyos_center_core::operation::{Operation, OperationKind, OperationOrigin, OperationState};
use cachyos_center_core::package::{
    CatalogQuery, InstallReason, InstalledQuery, PackageId, PackageOrigin, PackagePage,
    PackageSummary,
};
use cachyos_center_core::policy::{
    AutoUpdateConfig, AutoUpdatePolicy, AutoUpdateStatus, ExternalUpdater, OfflineUpdateStatus,
};
use cachyos_center_core::sanitize::SanitizeContext;
use cachyos_center_core::settings::Settings;
use cachyos_center_core::system::{LockStatus, SessionKind, SystemSummary};
use cachyos_center_core::updates::{CheckStatus, UpdateCandidate, UpdateCheckResult, UpdateFlag};
use cachyos_center_core::{AppError, AppResult, ErrorCode, Timestamp, now};
use cachyos_center_service::ReadApi;
use rmcp::model::{CallToolRequestParams, CallToolResult, JsonObject};
use rmcp::{ServiceError, ServiceExt};
use serde_json::{Value, json};

use crate::args::decode_cursor;
use crate::output::{McpErrorCode, ToolErrorBody};
use crate::{DISABLED_MESSAGE, MAX_OUTPUT_BYTES, McpServer, ServerOptions, ToolKind};

// ---- Fake read service -----------------------------------------------------------

struct FakeReadApi {
    enabled: AtomicBool,
    data_calls: AtomicUsize,
    delay: Option<Duration>,
    failure: Option<AppError>,
    summary: SystemSummary,
    updates: UpdateCheckResult,
    repository: Vec<PackageSummary>,
    installed: Vec<PackageSummary>,
    activity: Vec<HistoryEntry>,
    health: HealthReport,
    last_search: Mutex<Option<CatalogQuery>>,
    last_installed: Mutex<Option<InstalledQuery>>,
    last_activity_limit: Mutex<Option<u32>>,
}

impl FakeReadApi {
    fn enabled() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            data_calls: AtomicUsize::new(0),
            delay: None,
            failure: None,
            summary: summary(),
            updates: updates(CheckStatus::Fresh, Some(now() - 600)),
            repository: vec![
                package("linux-cachyos", Some("cachyos-core-v3")),
                package("linux-firmware", Some("core")),
                package("firefox", Some("extra")),
            ],
            installed: (0..7)
                .map(|i| package(&format!("lib{i}"), Some("extra")))
                .chain([package("my-tool", None)])
                .collect(),
            activity: (0..30).map(history).collect(),
            health: health(),
            last_search: Mutex::new(None),
            last_installed: Mutex::new(None),
            last_activity_limit: Mutex::new(None),
        }
    }

    fn disabled() -> Self {
        let fake = Self::enabled();
        fake.enabled.store(false, Ordering::SeqCst);
        fake
    }

    fn enter(&self) -> AppResult<()> {
        self.data_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(delay) = self.delay {
            std::thread::sleep(delay);
        }
        match &self.failure {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

impl ReadApi for FakeReadApi {
    fn system_summary(&self) -> AppResult<SystemSummary> {
        self.enter()?;
        Ok(self.summary.clone())
    }

    fn updates(&self) -> AppResult<UpdateCheckResult> {
        self.enter()?;
        Ok(self.updates.clone())
    }

    fn search(&self, query: &CatalogQuery) -> AppResult<Vec<PackageSummary>> {
        self.enter()?;
        *self.last_search.lock().expect("lock") = Some(query.clone());
        let needle = query.query.to_lowercase();
        Ok(self
            .repository
            .iter()
            .filter(|p| p.name.contains(&needle) || p.description.contains(&needle))
            .take(query.limit as usize)
            .cloned()
            .collect())
    }

    fn installed(&self, query: &InstalledQuery) -> AppResult<PackagePage> {
        self.enter()?;
        *self.last_installed.lock().expect("lock") = Some(query.clone());
        let needle = query.query.as_deref().unwrap_or_default().to_lowercase();
        let all: Vec<PackageSummary> = self
            .installed
            .iter()
            .filter(|p| needle.is_empty() || p.name.to_lowercase().contains(&needle))
            .cloned()
            .collect();
        let total = u32::try_from(all.len()).expect("small list");
        let offset = query.offset.min(total);
        let items: Vec<PackageSummary> = all
            .into_iter()
            .skip(offset as usize)
            .take(query.limit as usize)
            .collect();
        let end = offset + u32::try_from(items.len()).expect("small list");
        Ok(PackagePage {
            items,
            total,
            offset,
            next_offset: (end < total).then_some(end),
        })
    }

    fn recent_activity(&self, limit: u32) -> AppResult<Vec<HistoryEntry>> {
        self.enter()?;
        *self.last_activity_limit.lock().expect("lock") = Some(limit);
        Ok(self.activity.iter().take(limit as usize).cloned().collect())
    }

    fn health(&self) -> AppResult<HealthReport> {
        self.enter()?;
        Ok(self.health.clone())
    }

    fn auto_update(&self) -> AppResult<AutoUpdateStatus> {
        let mut last = Operation::new(
            "66666666-6666-4666-8666-666666666666".into(),
            OperationKind::UpdateCheck,
            OperationOrigin::Timer,
            900,
        );
        last.state = OperationState::Succeeded;
        Ok(AutoUpdateStatus {
            config: AutoUpdateConfig {
                policy: AutoUpdatePolicy::NotifyOnly,
                ..AutoUpdateConfig::default()
            },
            config_error: None,
            timer_enabled: true,
            next_run: Some(2_000),
            last_run: Some(900),
            last_result: Some(last),
            prepared_for_next_reboot: false,
            offline: OfflineUpdateStatus::default(),
            external_updaters: vec![],
            prepare_mode_available: false,
            prepare_mode_blockers: vec!["experimentalLocked".into()],
            helper_available: true,
        })
    }

    fn user_settings(&self) -> Settings {
        Settings {
            mcp_enabled: self.enabled.load(Ordering::SeqCst),
            ..Settings::default()
        }
    }
}

// ---- Fixtures ------------------------------------------------------------------------

fn summary() -> SystemSummary {
    SystemSummary {
        os: "CachyOS".into(),
        is_cachyos: true,
        kernel: "6.17.2-2-cachyos".into(),
        uptime_seconds: 3600,
        cpu: "AMD Ryzen 7 5700X 8-Core Processor".into(),
        gpus: vec!["AMD Navi 31".into()],
        memory_total_bytes: 32 << 30,
        memory_available_bytes: 16 << 30,
        root_total_bytes: 500 << 30,
        root_available_bytes: 200 << 30,
        root_filesystem: "btrfs".into(),
        session: SessionKind::Hyprland,
        desktop: Some("Hyprland".into()),
        pacman_version: Some("7.1.0-4".into()),
        package_backend_ready: true,
    }
}

fn package(name: &str, repository: Option<&str>) -> PackageSummary {
    PackageSummary {
        name: name.into(),
        description: format!("{name} package"),
        repository: repository.map(Into::into),
        origin: if repository.is_some() {
            PackageOrigin::Repo
        } else {
            PackageOrigin::LocalOrAur
        },
        architecture: "x86_64".into(),
        installed_version: Some("1.0-1".into()),
        available_version: repository.map(|_| "1.1-1".into()),
        install_reason: Some(InstallReason::Explicit),
        installed_size: Some(1024),
        update_available: repository.is_some(),
        ignored: false,
    }
}

fn candidate(name: &str) -> UpdateCandidate {
    UpdateCandidate {
        package_id: PackageId {
            repository: "core".into(),
            name: name.into(),
            architecture: String::new(),
        },
        old_version: "1.0-1".into(),
        new_version: "1.1-1".into(),
        download_size: Some(2048),
        flags: vec![UpdateFlag::Kernel, UpdateFlag::RebootRecommended],
    }
}

fn updates(status: CheckStatus, checked_at: Option<Timestamp>) -> UpdateCheckResult {
    let mut result = UpdateCheckResult::empty(status);
    result.checked_at = checked_at;
    result.attempted_at = checked_at;
    if checked_at.is_some() {
        result.updates = vec![candidate("linux-cachyos"), candidate("mesa")];
        result.held_back = vec![candidate("nvidia-utils")];
        result.total_download_size = Some(4096);
        result.reboot_recommended = true;
    }
    result
}

fn history(i: i64) -> HistoryEntry {
    HistoryEntry {
        id: format!("pacman-log-{i}"),
        source: HistorySource::ExternalPacman,
        kind: None,
        origin: None,
        state: None,
        log_outcome: Some(LogOutcome::Completed),
        started_at: 1_000 + i,
        ended_at: Some(1_010 + i),
        summary: "installed by alice from /home/alice/pkg/foo.pkg.tar.zst".into(),
        error_code: (i == 0).then_some(ErrorCode::TransactionFailed),
        installed: 1,
        upgraded: 2,
        removed: 0,
        downgraded: 0,
        updates_found: None,
        packages: vec!["linux".into(), "mesa".into()],
        outcome_unknown: false,
    }
}

fn health() -> HealthReport {
    HealthReport {
        items: vec![
            HealthItem {
                kind: HealthItemKind::PackageBackendUnavailable,
                severity: Severity::Critical,
                detail: "package functions are disabled: the libalpm bridge could not be loaded \
                         (/usr/lib/cachyos-center/libcachyos_center_alpm.so: cannot open shared \
                         object file)"
                    .into(),
                count: None,
            },
            HealthItem {
                kind: HealthItemKind::PacnewFiles,
                severity: Severity::Warning,
                detail: "configuration files need a manual merge".into(),
                count: Some(2),
            },
            HealthItem {
                kind: HealthItemKind::LastOperationFailed,
                severity: Severity::Warning,
                detail: "cannot write /home/alice/.cache/x on alice-laptop".into(),
                count: None,
            },
        ],
        pacnew_count: 2,
        pacsave_count: 1,
        config_files: vec![
            ConfigFileHint {
                path: "/etc/pacman.conf.pacnew".into(),
                kind: "pacnew".into(),
                modified_at: Some(1_000),
            },
            ConfigFileHint {
                path: "/etc/mkinitcpio.conf.pacnew".into(),
                kind: "pacnew".into(),
                modified_at: None,
            },
            ConfigFileHint {
                path: "/etc/secret-app.conf.pacsave".into(),
                kind: "pacsave".into(),
                modified_at: None,
            },
        ],
        lock: LockStatus::Locked {
            since: Some(1_000),
            holder_running: Some(true),
        },
        reboot_recommended: true,
        reboot_reasons: vec![RebootReason::KernelReplaced],
        package_cache_bytes: Some(1 << 30),
        package_cache_reclaimable_bytes: Some(1 << 20),
        snapshot: SnapshotSupport {
            btrfs_root: true,
            snapper_installed: true,
            root_config: Some("root".into()),
            snap_pac_active: true,
            can_request_snapshot: true,
        },
        offline_update: OfflineUpdateStatus {
            installed: true,
            prepared: false,
            prepare_timer_active: false,
            reboot_timer_active: false,
            offline_conf_included: true,
            offline_conf_ignored: vec!["linux-cachyos".into()],
            configuration_verifiable: true,
        },
        external_updaters: vec![ExternalUpdater {
            name: "arch-update.timer".into(),
            scope: "user".into(),
            active: true,
            description: "Cachy-Update/Arch-Update: update checks and notifications".into(),
        }],
        update_blockers: vec![UpdateBlocker::PackageManagerBusy],
        collected_at: 1_000,
    }
}

// ---- Helpers -------------------------------------------------------------------------

fn options() -> ServerOptions {
    ServerOptions {
        timeout: Duration::from_secs(10),
        max_output_bytes: MAX_OUTPUT_BYTES,
        max_concurrent_calls: 4,
        sanitize: SanitizeContext {
            home: Some("/home/alice".into()),
            user: Some("alice".into()),
            hostname: Some("alice-laptop".into()),
        },
    }
}

fn setup_with(fake: FakeReadApi, options: ServerOptions) -> (McpServer, Arc<FakeReadApi>) {
    let fake = Arc::new(fake);
    let api: Arc<dyn ReadApi> = fake.clone();
    (McpServer::with_options(api, options), fake)
}

fn setup(fake: FakeReadApi) -> (McpServer, Arc<FakeReadApi>) {
    setup_with(fake, options())
}

fn object(value: Value) -> JsonObject {
    match value {
        Value::Object(map) => map,
        other => panic!("arguments must be an object: {other}"),
    }
}

async fn call(server: &McpServer, tool: ToolKind, arguments: Value) -> CallToolResult {
    server.call(tool, object(arguments)).await
}

fn text_of(result: &CallToolResult) -> String {
    assert_eq!(result.content.len(), 1, "exactly one text content");
    result.content[0]
        .as_text()
        .expect("text content")
        .text
        .clone()
}

/// Structured content of a successful result; checks that the text content
/// is the same JSON, pretty-printed.
fn structured(result: &CallToolResult) -> Value {
    assert_eq!(result.is_error, Some(false), "unexpected error: {result:?}");
    let value = result
        .structured_content
        .clone()
        .expect("structured content");
    let text = text_of(result);
    assert_eq!(text, serde_json::to_string_pretty(&value).expect("json"));
    assert!(text.len() <= MAX_OUTPUT_BYTES);
    value
}

fn error_body(result: &CallToolResult) -> ToolErrorBody {
    assert_eq!(result.is_error, Some(true), "expected an error: {result:?}");
    let value = result
        .structured_content
        .clone()
        .expect("structured content");
    let text: Value = serde_json::from_str(&text_of(result)).expect("text is JSON");
    assert_eq!(text, value);
    serde_json::from_value(value).expect("error body")
}

fn default_args(tool: ToolKind) -> Value {
    match tool {
        ToolKind::PackagesSearch => json!({ "query": "linux" }),
        _ => json!({}),
    }
}

// ---- Access switch -------------------------------------------------------------------

#[tokio::test]
async fn disabled_server_answers_every_tool_with_unavailable() {
    let (server, fake) = setup(FakeReadApi::disabled());
    for tool in ToolKind::ALL {
        for arguments in [default_args(tool), json!({ "bogus": true })] {
            let body = error_body(&call(&server, tool, arguments).await);
            assert_eq!(body.code, McpErrorCode::Unavailable, "{tool:?}");
            assert_eq!(body.message, DISABLED_MESSAGE);
        }
    }
    assert_eq!(
        fake.data_calls.load(Ordering::SeqCst),
        0,
        "no data may be read"
    );
    // Listing the tools still works while disabled.
    assert_eq!(McpServer::tools().len(), TOOL_NAMES.len());
}

#[tokio::test]
async fn setting_is_read_again_on_every_call() {
    let (server, fake) = setup(FakeReadApi::disabled());
    let tool = ToolKind::SystemGetSummary;
    assert_eq!(
        error_body(&call(&server, tool, json!({})).await).code,
        McpErrorCode::Unavailable
    );
    fake.enabled.store(true, Ordering::SeqCst);
    structured(&call(&server, tool, json!({})).await);
    fake.enabled.store(false, Ordering::SeqCst);
    assert_eq!(
        error_body(&call(&server, tool, json!({})).await).code,
        McpErrorCode::Unavailable
    );
}

// ---- Happy paths ---------------------------------------------------------------------

#[tokio::test]
async fn system_summary() {
    let (server, _) = setup(FakeReadApi::enabled());
    let value = structured(&call(&server, ToolKind::SystemGetSummary, json!({})).await);
    assert_eq!(value["os"], "CachyOS");
    assert_eq!(value["kernel"], "6.17.2-2-cachyos");
    assert_eq!(value["session"], "hyprland");
    assert_eq!(value["gpus"], json!(["AMD Navi 31"]));
    assert_eq!(value["rootFilesystem"], "btrfs");
    assert_eq!(value["memoryTotalBytes"], 32u64 << 30);
    assert_eq!(value["packageBackendReady"], true);
    assert_eq!(value["truncated"], false);
}

#[tokio::test]
async fn updates_list_fresh() {
    let (server, _) = setup(FakeReadApi::enabled());
    let value = structured(&call(&server, ToolKind::UpdatesList, json!({})).await);
    assert_eq!(value["status"], "fresh");
    assert_eq!(value["stale"], false);
    assert_eq!(value["note"], Value::Null);
    let age = value["ageSeconds"].as_i64().unwrap();
    assert!((600..700).contains(&age), "{age}");
    assert_eq!(value["updateCount"], 2);
    assert_eq!(
        value["updates"][0],
        json!({
            "package": "linux-cachyos",
            "repository": "core",
            "oldVersion": "1.0-1",
            "newVersion": "1.1-1",
            "flags": ["kernel", "rebootRecommended"],
            "downloadSize": 2048,
        })
    );
    assert_eq!(value["heldBackCount"], 1);
    assert_eq!(value["heldBack"][0]["package"], "nvidia-utils");
    assert_eq!(value["totalDownloadSize"], 4096);
    assert_eq!(value["rebootRecommended"], true);
}

#[tokio::test]
async fn packages_search_uses_validated_arguments() {
    let (server, fake) = setup(FakeReadApi::enabled());
    let value = structured(
        &call(
            &server,
            ToolKind::PackagesSearch,
            json!({ "query": "  linux " }),
        )
        .await,
    );
    let query = fake.last_search.lock().unwrap().clone().unwrap();
    assert_eq!(query.query, "linux");
    assert_eq!(query.limit, 20);
    assert_eq!(query.repository, None, "no repository filter, no AUR");
    assert_eq!(value["query"], "linux");
    assert_eq!(value["limit"], 20);
    let items = value["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        json!({
            "name": "linux-cachyos",
            "repository": "cachyos-core-v3",
            "origin": "repo",
            "version": "1.1-1",
            "installedVersion": "1.0-1",
            "updateAvailable": true,
            "description": "linux-cachyos package",
        })
    );

    let value = structured(
        &call(
            &server,
            ToolKind::PackagesSearch,
            json!({ "query": "linux", "limit": 1 }),
        )
        .await,
    );
    assert_eq!(value["items"].as_array().unwrap().len(), 1);
    assert_eq!(fake.last_search.lock().unwrap().as_ref().unwrap().limit, 1);
}

#[tokio::test]
async fn packages_installed_is_paginated_with_cursors() {
    let (server, fake) = setup(FakeReadApi::enabled());
    let first =
        structured(&call(&server, ToolKind::PackagesInstalled, json!({ "limit": 3 })).await);
    assert_eq!(first["total"], 8);
    assert_eq!(first["offset"], 0);
    assert_eq!(first["items"].as_array().unwrap().len(), 3);
    assert_eq!(first["items"][0]["origin"], "repo");
    assert_eq!(first["items"][0]["installReason"], "explicit");
    let cursor = first["nextCursor"].as_str().unwrap().to_string();

    let mut names = Vec::new();
    let mut next = Some(cursor);
    let mut pages = 1;
    while let Some(cursor) = next {
        let page = structured(
            &call(
                &server,
                ToolKind::PackagesInstalled,
                json!({ "limit": 3, "cursor": cursor }),
            )
            .await,
        );
        names.extend(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p["name"].as_str().unwrap().to_string()),
        );
        next = page["nextCursor"].as_str().map(str::to_string);
        pages += 1;
    }
    assert_eq!(pages, 3);
    assert_eq!(names.len(), 5);
    assert_eq!(names.last().unwrap(), "my-tool");
    let last = fake.last_installed.lock().unwrap().clone().unwrap();
    assert_eq!(last.offset, 6);
    assert_eq!(last.limit, 3);

    let foreign = structured(
        &call(
            &server,
            ToolKind::PackagesInstalled,
            json!({ "query": "MY-" }),
        )
        .await,
    );
    assert_eq!(foreign["total"], 1);
    assert_eq!(foreign["items"][0]["origin"], "localOrAur");
    assert_eq!(foreign["items"][0]["repository"], Value::Null);
    assert_eq!(foreign["nextCursor"], Value::Null);
    let query = fake.last_installed.lock().unwrap().clone().unwrap();
    assert_eq!(query.limit, 50, "default limit");
    assert_eq!(query.query.as_deref(), Some("MY-"));
}

#[tokio::test]
async fn operations_recent_is_limited_and_sanitized() {
    let (server, fake) = setup(FakeReadApi::enabled());
    let value = structured(&call(&server, ToolKind::OperationsRecent, json!({})).await);
    assert_eq!(*fake.last_activity_limit.lock().unwrap(), Some(10));
    let items = value["items"].as_array().unwrap();
    assert_eq!(items.len(), 10);
    assert_eq!(items[0]["source"], "externalPacman");
    assert_eq!(items[0]["logOutcome"], "completed");
    assert_eq!(items[0]["errorCode"], "TRANSACTION_FAILED");
    assert_eq!(items[0]["summary"], "installed by <user> from <path>");
    assert_eq!(items[0]["packages"], json!(["linux", "mesa"]));

    let value =
        structured(&call(&server, ToolKind::OperationsRecent, json!({ "limit": 20 })).await);
    assert_eq!(value["items"].as_array().unwrap().len(), 20);
}

#[tokio::test]
async fn health_contains_counts_but_no_paths() {
    let (server, _) = setup(FakeReadApi::enabled());
    let value = structured(&call(&server, ToolKind::HealthGet, json!({})).await);
    assert_eq!(value["pacnewCount"], 2);
    assert_eq!(value["pacsaveCount"], 1);
    assert_eq!(value["worstSeverity"], "critical");
    assert_eq!(
        value["lock"],
        json!({ "state": "locked", "since": 1000, "holderRunning": true })
    );
    assert_eq!(value["rebootRecommended"], true);
    assert_eq!(
        value["rebootReasons"],
        json!(["running kernel was replaced by an update"])
    );
    assert_eq!(
        value["offlineUpdate"]["offlineConfHeldPackages"],
        json!(["linux-cachyos"])
    );
    assert_eq!(value["externalUpdaters"][0]["name"], "arch-update.timer");
    assert_eq!(
        value["updateBlockers"],
        json!(["package manager is busy (db.lck)"])
    );
    assert_eq!(
        value["autoUpdate"],
        json!({
            "policy": "notifyOnly",
            "timerEnabled": true,
            "nextRun": 2000,
            "lastRun": 900,
            "lastResultState": "succeeded",
            "preparedForNextReboot": false
        })
    );
    assert_eq!(value["items"][0]["kind"], "packageBackendUnavailable");
    assert_eq!(value["items"][2]["detail"], "cannot write <path> on <host>");

    let text = value.to_string();
    assert!(value.get("configFiles").is_none());
    for forbidden in [
        "/etc/",
        "/usr/lib",
        "/home/",
        ".pacnew",
        "secret-app",
        "alice",
    ] {
        assert!(!text.contains(forbidden), "{forbidden} in {text}");
    }
}

// ---- Argument validation -------------------------------------------------------------

#[tokio::test]
async fn invalid_arguments_are_rejected_before_reading_data() {
    let (server, fake) = setup(FakeReadApi::enabled());
    for (tool, arguments, fragment) in [
        (ToolKind::PackagesSearch, json!({}), "missing field `query`"),
        (
            ToolKind::PackagesSearch,
            json!({ "query": "a" }),
            "at least 2",
        ),
        (
            ToolKind::PackagesSearch,
            json!({ "query": "x".repeat(101) }),
            "too long",
        ),
        (
            ToolKind::PackagesSearch,
            json!({ "query": "linux", "limit": 0 }),
            "between 1 and 50",
        ),
        (
            ToolKind::PackagesSearch,
            json!({ "query": "linux", "limit": 51 }),
            "between 1 and 50",
        ),
        (
            ToolKind::PackagesSearch,
            json!({ "query": "linux", "limit": "5" }),
            "invalid type",
        ),
        (
            ToolKind::PackagesSearch,
            json!({ "query": "linux", "aur": true }),
            "unknown field",
        ),
        (
            ToolKind::PackagesInstalled,
            json!({ "limit": 101 }),
            "between 1 and 100",
        ),
        (
            ToolKind::PackagesInstalled,
            json!({ "cursor": "garbage" }),
            "cursor",
        ),
        (
            ToolKind::OperationsRecent,
            json!({ "limit": 21 }),
            "between 1 and 20",
        ),
        (
            ToolKind::OperationsRecent,
            json!({ "limit": -1 }),
            "invalid value",
        ),
        (
            ToolKind::SystemGetSummary,
            json!({ "verbose": true }),
            "unknown field",
        ),
        (
            ToolKind::HealthGet,
            json!({ "paths": true }),
            "unknown field",
        ),
    ] {
        let body = error_body(&call(&server, tool, arguments.clone()).await);
        assert_eq!(
            body.code,
            McpErrorCode::InvalidInput,
            "{tool:?} {arguments}"
        );
        assert!(
            body.message.contains(fragment),
            "{tool:?} {arguments}: {}",
            body.message
        );
    }
    assert_eq!(fake.data_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cursor_of_another_query_is_rejected() {
    let (server, _) = setup(FakeReadApi::enabled());
    let first =
        structured(&call(&server, ToolKind::PackagesInstalled, json!({ "limit": 2 })).await);
    let cursor = first["nextCursor"].as_str().unwrap();
    let body = error_body(
        &call(
            &server,
            ToolKind::PackagesInstalled,
            json!({ "query": "lib", "cursor": cursor }),
        )
        .await,
    );
    assert_eq!(body.code, McpErrorCode::InvalidInput);
    assert!(body.message.contains("different query"));
}

// ---- Error mapping and timeouts ------------------------------------------------------

#[tokio::test]
async fn service_errors_use_the_public_codes() {
    for (error, expected) in [
        (
            AppError::busy("database locked (/var/lib/pacman/db.lck)"),
            McpErrorCode::Busy,
        ),
        (
            AppError::new(ErrorCode::Conflict, "offline update prepared"),
            McpErrorCode::Busy,
        ),
        (
            AppError::unavailable("bridge missing"),
            McpErrorCode::Unavailable,
        ),
        (
            AppError::not_found("repository missing"),
            McpErrorCode::Unavailable,
        ),
        (
            AppError::new(ErrorCode::Stale, "too old"),
            McpErrorCode::Stale,
        ),
        (
            AppError::new(ErrorCode::Unsupported, "libalpm 16"),
            McpErrorCode::Unsupported,
        ),
        (AppError::internal("boom"), McpErrorCode::Internal),
        (
            AppError::invalid("rejected by the service"),
            McpErrorCode::Internal,
        ),
    ] {
        let mut fake = FakeReadApi::enabled();
        fake.failure = Some(error.clone());
        let (server, _) = setup(fake);
        for tool in ToolKind::ALL {
            let body = error_body(&call(&server, tool, default_args(tool)).await);
            assert_eq!(body.code, expected, "{tool:?} {error}");
            assert_eq!(body.code.as_str(), error.code.mcp_code());
            assert!(
                !body.message.contains("/var/lib"),
                "paths are masked: {}",
                body.message
            );
        }
    }
}

#[tokio::test]
async fn slow_reads_time_out_with_unavailable() {
    let mut fake = FakeReadApi::enabled();
    fake.delay = Some(Duration::from_millis(500));
    let mut opts = options();
    opts.timeout = Duration::from_millis(100);
    let (server, _) = setup_with(fake, opts);
    let started = Instant::now();
    let body = error_body(&call(&server, ToolKind::HealthGet, json!({})).await);
    assert!(started.elapsed() < Duration::from_millis(450));
    assert_eq!(body.code, McpErrorCode::Unavailable);
    assert!(body.message.contains("timed out"), "{}", body.message);
}

#[tokio::test]
async fn concurrent_reads_are_limited() {
    let mut fake = FakeReadApi::enabled();
    fake.delay = Some(Duration::from_millis(150));
    let mut opts = options();
    opts.max_concurrent_calls = 1;
    let (server, _) = setup_with(fake, opts);
    let started = Instant::now();
    let (a, b) = tokio::join!(
        call(&server, ToolKind::SystemGetSummary, json!({})),
        call(&server, ToolKind::HealthGet, json!({}))
    );
    structured(&a);
    structured(&b);
    assert!(
        started.elapsed() >= Duration::from_millis(300),
        "calls ran one after another"
    );
}

// ---- Staleness of updates_list -------------------------------------------------------

async fn updates_for(result: UpdateCheckResult) -> Value {
    let mut fake = FakeReadApi::enabled();
    fake.updates = result;
    let (server, _) = setup(fake);
    structured(&call(&server, ToolKind::UpdatesList, json!({})).await)
}

#[tokio::test]
async fn updates_list_never_presents_old_data_as_current() {
    let never = updates_for(updates(CheckStatus::NeverChecked, None)).await;
    assert_eq!(never["status"], "neverChecked");
    assert_eq!(never["stale"], true);
    assert_eq!(never["checkedAt"], Value::Null);
    assert_eq!(never["ageSeconds"], Value::Null);
    assert_eq!(never["updates"], json!([]));
    // Unknown is never reported as 0 updates.
    assert_eq!(never["updateCount"], Value::Null);
    assert_eq!(never["heldBackCount"], Value::Null);
    assert!(
        never["note"]
            .as_str()
            .unwrap()
            .contains("No update check has been run yet")
    );

    let old = updates_for(updates(CheckStatus::Stale, Some(now() - 7 * 3600 - 60))).await;
    assert_eq!(old["status"], "stale");
    assert_eq!(old["stale"], true);
    assert!(old["ageSeconds"].as_i64().unwrap() >= 7 * 3600);
    let note = old["note"].as_str().unwrap();
    assert!(
        note.contains("outdated") && note.contains("7 hours"),
        "{note}"
    );
    assert_eq!(old["updateCount"], 2, "the last known data is still shown");

    let mut failed = updates(CheckStatus::Failed, Some(now() - 3 * 86_400));
    failed.error = Some(AppError::unavailable(
        "cannot synchronize /home/alice/.cache/cachyos-center/checkup-db",
    ));
    let failed = updates_for(failed).await;
    assert_eq!(failed["stale"], true);
    assert!(failed["note"].as_str().unwrap().contains("failed"));
    assert!(failed["note"].as_str().unwrap().contains("3 days"));
    assert_eq!(failed["lastError"]["code"], "UNAVAILABLE");
    assert_eq!(failed["lastError"]["message"], "cannot synchronize <path>");

    let mut missing = updates(CheckStatus::PrerequisiteMissing, None);
    missing.missing_prerequisites = vec!["pacman-contrib".into()];
    let missing = updates_for(missing).await;
    assert_eq!(missing["stale"], true);
    assert!(missing["note"].as_str().unwrap().contains("pacman-contrib"));
    assert_eq!(missing["missingPrerequisites"], json!(["pacman-contrib"]));

    let unsupported = updates_for(updates(CheckStatus::Unsupported, None)).await;
    assert_eq!(unsupported["stale"], true);
    assert!(unsupported["note"].as_str().unwrap().contains("disabled"));
}

// ---- Size limit ----------------------------------------------------------------------

#[tokio::test]
async fn large_installed_pages_are_truncated_and_continue_on_the_next_page() {
    let mut fake = FakeReadApi::enabled();
    fake.installed = (0..150)
        .map(|i| {
            let mut p = package(&format!("pkg{i:03}"), Some("extra"));
            p.description = "d".repeat(1500);
            p
        })
        .collect();
    let (server, _) = setup(fake);
    let page = structured(
        &call(
            &server,
            ToolKind::PackagesInstalled,
            json!({ "limit": 100 }),
        )
        .await,
    );
    assert_eq!(page["truncated"], true);
    let returned = page["items"].as_array().unwrap().len();
    assert!(returned > 0 && returned < 100, "{returned}");
    let cursor = page["nextCursor"].as_str().unwrap();
    assert_eq!(
        decode_cursor(cursor, "").unwrap(),
        u32::try_from(returned).unwrap()
    );
    let next = structured(
        &call(
            &server,
            ToolKind::PackagesInstalled,
            json!({ "limit": 1, "cursor": cursor }),
        )
        .await,
    );
    assert_eq!(next["items"][0]["name"], format!("pkg{returned:03}"));
}

#[tokio::test]
async fn large_update_lists_are_truncated_with_counts() {
    let mut result = updates(CheckStatus::Fresh, Some(now()));
    result.updates = (0..3000)
        .map(|i| candidate(&format!("package-{i}")))
        .collect();
    let value = updates_for(result).await;
    assert_eq!(value["truncated"], true);
    assert_eq!(value["updateCount"], 3000);
    let shown = value["updates"].as_array().unwrap().len();
    assert!(shown > 0 && shown < 3000, "{shown}");
    assert_eq!(value["heldBack"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn overlong_strings_are_cut() {
    let mut fake = FakeReadApi::enabled();
    fake.repository[2].description = "y".repeat(10_000);
    let (server, _) = setup(fake);
    let value = structured(
        &call(
            &server,
            ToolKind::PackagesSearch,
            json!({ "query": "firefox" }),
        )
        .await,
    );
    assert_eq!(value["truncated"], true);
    let description = value["items"][0]["description"].as_str().unwrap();
    assert_eq!(description.chars().count(), crate::MAX_STRING_CHARS + 1);
    assert!(description.ends_with('…'));
}

#[tokio::test]
async fn output_that_cannot_be_reduced_is_an_internal_error() {
    let mut opts = options();
    opts.max_output_bytes = 100;
    let (server, _) = setup_with(FakeReadApi::enabled(), opts);
    let body = error_body(&call(&server, ToolKind::SystemGetSummary, json!({})).await);
    assert_eq!(body.code, McpErrorCode::Internal);
    assert!(body.message.contains("size limit"));
}

// ---- Privacy -------------------------------------------------------------------------

#[tokio::test]
async fn personal_data_is_removed_from_all_strings() {
    let mut fake = FakeReadApi::enabled();
    fake.summary.cpu = "Ryzen of alice".into();
    fake.summary.desktop = Some("Hyprland on alice-laptop".into());
    fake.installed[7].description =
        "scripts from /home/alice/src, mail alice.doe@example.org".into();
    let (server, _) = setup(fake);
    let summary = structured(&call(&server, ToolKind::SystemGetSummary, json!({})).await);
    assert_eq!(summary["cpu"], "Ryzen of <user>");
    assert_eq!(summary["desktop"], "Hyprland on <host>");
    let page = structured(
        &call(
            &server,
            ToolKind::PackagesInstalled,
            json!({ "query": "my-tool" }),
        )
        .await,
    );
    assert_eq!(
        page["items"][0]["description"],
        "scripts from ~/src, mail <email>"
    );
}

// ---- Server identity and protocol ----------------------------------------------------

#[test]
fn server_config_identity_and_instructions() {
    let config = McpServer::config();
    assert_eq!(config.server_info.name, SERVER_NAME);
    assert_eq!(config.server_info.version, env!("CARGO_PKG_VERSION"));
    assert!(config.capabilities.tools.is_some());
    assert!(config.capabilities.resources.is_none());
    assert!(config.capabilities.prompts.is_none());
    let instructions = config.instructions.unwrap();
    for fragment in ["read-only", "this computer", "stale", "UNAVAILABLE"] {
        assert!(instructions.contains(fragment), "{fragment}");
    }
}

#[tokio::test]
async fn protocol_roundtrip_in_process() {
    let (server, _) = setup(FakeReadApi::enabled());
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    let server_task = tokio::spawn(async move {
        let running = server.serve(server_io).await.expect("server initialized");
        running.waiting().await.expect("server loop")
    });
    let client = ().serve(client_io).await.unwrap();

    let info = client.peer_info().unwrap();
    assert_eq!(info.server_info.as_ref().unwrap().name, SERVER_NAME);
    assert!(info.capabilities.resources.is_none());
    assert!(info.capabilities.prompts.is_none());

    let tools = client.list_all_tools().await.unwrap();
    let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    names.sort();
    let mut expected: Vec<String> = TOOL_NAMES.iter().map(|n| n.to_string()).collect();
    expected.sort();
    assert_eq!(names, expected);

    let result = client
        .call_tool(
            CallToolRequestParams::new("packages_search")
                .with_arguments(object(json!({ "query": "firefox", "limit": 5 }))),
        )
        .await
        .unwrap();
    assert_eq!(structured(&result)["items"][0]["name"], "firefox");

    let invalid = client
        .call_tool(
            CallToolRequestParams::new("packages_search")
                .with_arguments(object(json!({ "query": "f" }))),
        )
        .await
        .unwrap();
    assert_eq!(error_body(&invalid).code, McpErrorCode::InvalidInput);

    // Unknown tools are a JSON-RPC error (-32602), not a tool result.
    match client
        .call_tool(CallToolRequestParams::new("install_package"))
        .await
    {
        Err(ServiceError::McpError(error)) => assert_eq!(error.code.0, -32602),
        other => panic!("unexpected result: {other:?}"),
    }

    client.cancel().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), server_task)
        .await
        .expect("server stops after the client disconnects")
        .unwrap();
}
