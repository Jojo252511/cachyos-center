//! Acts as an MCP host: starts the compiled `cachyos-center-mcp` as a child
//! process (stdio transport of the rmcp client) and talks MCP to it.
//!
//! System and package data are read (read-only) from the machine running the
//! tests. User data (settings, history, update check state) lives in
//! temporary XDG directories, so the update check is always "never checked".
//! Without the libalpm bridge next to the binary
//! (`cargo build -p cachyos-center-alpm-bridge`) the package tools answer
//! with an error code; the assertions accept that.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use cachyos_center_core::mcp::{SERVER_NAME, TOOL_NAMES};
use rmcp::model::{CallToolRequestParams, CallToolResult, JsonObject};
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{RoleClient, ServiceError, ServiceExt};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

const BINARY: &str = env!("CARGO_BIN_EXE_cachyos-center-mcp");
const DISABLED: &str = "MCP access is disabled in cachyos-center (Einstellungen → KI-Zugriff (MCP) / Settings → AI access (MCP))";

/// Temporary XDG directories of one server process.
struct UserDirs {
    root: tempfile::TempDir,
}

impl UserDirs {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary directory");
        for dir in ["config", "data", "cache", "state"] {
            std::fs::create_dir_all(root.path().join(dir)).expect("XDG directory");
        }
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    fn env(&self) -> Vec<(&'static str, PathBuf)> {
        vec![
            ("XDG_CONFIG_HOME", self.path("config")),
            ("XDG_DATA_HOME", self.path("data")),
            ("XDG_CACHE_HOME", self.path("cache")),
            ("XDG_STATE_HOME", self.path("state")),
        ]
    }

    /// Writes `settings.toml` the way the app does (camelCase keys).
    fn set_mcp_enabled(&self, enabled: bool) {
        let dir = self.path("config").join("cachyos-center");
        std::fs::create_dir_all(&dir).expect("settings directory");
        std::fs::write(
            dir.join("settings.toml"),
            format!("mcpEnabled = {enabled}\n"),
        )
        .expect("settings file");
    }

    fn command(&self) -> Command {
        let env = self.env();
        Command::new(BINARY).configure(|cmd| {
            for (key, value) in &env {
                cmd.env(key, value);
            }
            cmd.env("CACHYOS_CENTER_LOG", "error");
        })
    }
}

type Client = RunningService<RoleClient, ()>;

async fn connect(dirs: &UserDirs) -> Client {
    let transport = TokioChildProcess::new(dirs.command()).expect("spawn cachyos-center-mcp");
    tokio::time::timeout(Duration::from_secs(20), ().serve(transport))
        .await
        .expect("initialize in time")
        .expect("initialize")
}

fn object(value: Value) -> JsonObject {
    match value {
        Value::Object(map) => map,
        other => panic!("arguments must be an object: {other}"),
    }
}

async fn call(client: &Client, name: &'static str, arguments: Value) -> CallToolResult {
    let request = CallToolRequestParams::new(name).with_arguments(object(arguments));
    tokio::time::timeout(Duration::from_secs(30), client.call_tool(request))
        .await
        .expect("tool answered in time")
        .expect("tool call is not a protocol error")
}

fn text(result: &CallToolResult) -> String {
    result
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .expect("text content")
}

/// `Ok(structured content)` or `Err((code, message))`; both representations
/// (structured and text) must carry the same JSON.
fn outcome(result: &CallToolResult) -> Result<Value, (String, String)> {
    let value = result
        .structured_content
        .clone()
        .expect("structured content");
    let parsed: Value = serde_json::from_str(&text(result)).expect("text content is JSON");
    assert_eq!(parsed, value, "text and structured content differ");
    if result.is_error == Some(true) {
        let code = value["code"].as_str().expect("code").to_string();
        let message = value["message"].as_str().expect("message").to_string();
        Err((code, message))
    } else {
        Ok(value)
    }
}

fn assert_no_personal_data(text: &str) {
    if let Ok(home) = std::env::var("HOME")
        && home.len() > 1
    {
        assert!(!text.contains(&home), "home directory in output");
    }
    if let Ok(host) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
        let host = host.trim();
        if host.len() >= 3 {
            assert!(!text.contains(host), "host name in output");
        }
    }
}

/// Package tools answer `UNSUPPORTED` on machines without libalpm bridge;
/// `CC_REQUIRE_BRIDGE=1` (CI) turns that into a failure.
fn package_result(tool: &str, result: &CallToolResult) -> Option<Value> {
    match outcome(result) {
        Ok(value) => Some(value),
        Err((code, message)) => {
            assert!(
                matches!(code.as_str(), "UNSUPPORTED" | "UNAVAILABLE"),
                "{tool}: {code} {message}"
            );
            if std::env::var("CC_REQUIRE_BRIDGE").as_deref() == Ok("1") {
                panic!("{tool}: bridge required but unavailable ({code}: {message})");
            }
            eprintln!("{tool}: skipped, package functions unavailable ({code}: {message})");
            None
        }
    }
}

#[tokio::test]
async fn host_sees_exactly_the_read_only_tools_and_real_data() {
    let dirs = UserDirs::new();
    dirs.set_mcp_enabled(true);
    let client = connect(&dirs).await;

    let info = client.peer_info().expect("server info after initialize");
    let server = info.server_info.as_ref().expect("implementation info");
    assert_eq!(server.name, SERVER_NAME);
    assert_eq!(server.version, env!("CARGO_PKG_VERSION"));
    let instructions = info.instructions.as_deref().unwrap_or_default();
    assert!(instructions.contains("read-only"), "{instructions}");
    assert!(instructions.contains("stale"), "{instructions}");
    assert!(info.capabilities.tools.is_some());
    assert!(info.capabilities.resources.is_none(), "no resources");
    assert!(info.capabilities.prompts.is_none(), "no prompts");

    // tools/list: exactly TOOL_NAMES, all annotated as read-only.
    let tools = client.list_all_tools().await.expect("tools/list");
    let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    names.sort_unstable();
    let mut expected = TOOL_NAMES.to_vec();
    expected.sort_unstable();
    assert_eq!(names, expected);
    for tool in &tools {
        for verb in [
            "install", "update", "upgrade", "remove", "delete", "write", "exec", "run",
        ] {
            assert!(!tool.name.split('_').any(|s| s == verb), "{}", tool.name);
        }
        let annotations = tool.annotations.as_ref().expect("annotations");
        assert_eq!(annotations.read_only_hint, Some(true), "{}", tool.name);
        assert_eq!(annotations.destructive_hint, Some(false), "{}", tool.name);
        assert!(tool.output_schema.is_some(), "{}", tool.name);
    }

    // system_get_summary works without the package backend.
    let result = call(&client, "system_get_summary", json!({})).await;
    let summary = outcome(&result).expect("system_get_summary");
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").expect("kernel release");
    assert_eq!(summary["kernel"], release.trim());
    assert!(!summary["os"].as_str().expect("os").is_empty());
    assert!(summary["memoryTotalBytes"].as_u64().expect("memory") > 0);
    assert_eq!(summary["truncated"], false);
    assert_no_personal_data(&text(&result));

    // updates_list: the temporary cache has no check result, so the data can
    // never be presented as current.
    let updates = outcome(&call(&client, "updates_list", json!({})).await).expect("updates_list");
    let status = updates["status"].as_str().expect("status");
    assert!(
        matches!(
            status,
            "neverChecked" | "prerequisiteMissing" | "unsupported"
        ),
        "{status}"
    );
    assert_eq!(updates["stale"], true);
    assert!(!updates["note"].as_str().expect("note").is_empty());
    assert_eq!(updates["checkedAt"], Value::Null);
    assert_eq!(updates["updates"], json!([]));
    assert_eq!(updates["updateCount"], Value::Null);

    // Package tools (read-only, local databases).
    let result = call(
        &client,
        "packages_search",
        json!({ "query": "pacman", "limit": 5 }),
    )
    .await;
    if let Some(search) = package_result("packages_search", &result) {
        let items = search["items"].as_array().expect("items");
        assert!(items.len() <= 5);
        assert!(items.iter().all(|i| i["origin"] == "repo"));
        // Exact name matches rank first (empty without synchronized databases).
        if let Some(first) = items.first() {
            assert_eq!(first["name"], "pacman", "{search}");
        }
    }

    let result = call(&client, "packages_installed", json!({ "limit": 3 })).await;
    if let Some(page) = package_result("packages_installed", &result) {
        let items = page["items"].as_array().expect("items");
        assert!(items.len() <= 3);
        let total = page["total"].as_u64().expect("total");
        assert!(total >= items.len() as u64);
        if let Some(cursor) = page["nextCursor"].as_str() {
            let next = call(
                &client,
                "packages_installed",
                json!({ "limit": 3, "cursor": cursor }),
            )
            .await;
            let next = outcome(&next).expect("second page");
            assert_eq!(next["offset"], 3);
            assert_ne!(next["items"][0]["name"], items[0]["name"]);
        }
    }

    let result = call(&client, "operations_recent", json!({ "limit": 5 })).await;
    let operations = outcome(&result).expect("operations_recent");
    assert!(operations["items"].as_array().expect("items").len() <= 5);
    assert_no_personal_data(&text(&result));

    let result = call(&client, "health_get", json!({})).await;
    let health = outcome(&result).expect("health_get");
    assert!(health.get("configFiles").is_none());
    assert!(health["pacnewCount"].is_u64());
    let health_text = text(&result);
    assert!(
        !health_text.contains("/etc/"),
        "no file paths: {health_text}"
    );
    assert_no_personal_data(&health_text);

    // Invalid arguments: isError result with INVALID_INPUT.
    let result = call(&client, "packages_search", json!({ "query": "a" })).await;
    let (code, message) = outcome(&result).expect_err("too short query");
    assert_eq!(code, "INVALID_INPUT");
    assert!(message.contains("at least 2"), "{message}");

    // Unknown (e.g. mutating) tools do not exist: JSON-RPC error -32602.
    match client
        .call_tool(CallToolRequestParams::new("packages_install"))
        .await
    {
        Err(ServiceError::McpError(error)) => assert_eq!(error.code.0, -32602),
        other => panic!("unexpected answer: {other:?}"),
    }

    client.cancel().await.expect("close");
}

#[tokio::test]
async fn disabled_by_default_every_call_is_unavailable() {
    let dirs = UserDirs::new(); // no settings file: defaults, MCP off
    let client = connect(&dirs).await;
    let tools = client
        .list_all_tools()
        .await
        .expect("tools/list works while disabled");
    assert_eq!(tools.len(), TOOL_NAMES.len());
    for name in TOOL_NAMES {
        let arguments = if name == "packages_search" {
            json!({ "query": "linux" })
        } else {
            json!({})
        };
        let (code, message) = outcome(&call(&client, name, arguments).await).expect_err(name);
        assert_eq!(code, "UNAVAILABLE", "{name}");
        assert_eq!(message, DISABLED, "{name}");
    }
    client.cancel().await.expect("close");
}

#[tokio::test]
async fn switching_access_takes_effect_without_restart() {
    let dirs = UserDirs::new();
    dirs.set_mcp_enabled(false);
    let client = connect(&dirs).await;
    let (code, _) =
        outcome(&call(&client, "system_get_summary", json!({})).await).expect_err("disabled");
    assert_eq!(code, "UNAVAILABLE");

    dirs.set_mcp_enabled(true);
    outcome(&call(&client, "system_get_summary", json!({})).await).expect("enabled");

    dirs.set_mcp_enabled(false);
    let (code, _) =
        outcome(&call(&client, "health_get", json!({})).await).expect_err("disabled again");
    assert_eq!(code, "UNAVAILABLE");
    client.cancel().await.expect("close");
}

#[tokio::test]
async fn stdout_carries_only_protocol_and_eof_ends_the_server() {
    let dirs = UserDirs::new();
    let mut child = dirs
        .command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "host-test", "version": "0" }
        }
    });
    let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
    let mut input = format!("{initialize}\n");
    input.push_str(&format!(
        "{}\n{list}\n",
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
    ));
    stdin
        .write_all(input.as_bytes())
        .await
        .expect("write requests");
    stdin.flush().await.expect("flush");

    let mut lines = BufReader::new(stdout).lines();
    let mut responses = Vec::new();
    while responses.len() < 2 {
        let line = tokio::time::timeout(Duration::from_secs(20), lines.next_line())
            .await
            .expect("response in time")
            .expect("read stdout")
            .expect("response line");
        let message: Value = serde_json::from_str(&line).expect("every stdout line is JSON-RPC");
        assert_eq!(message["jsonrpc"], "2.0");
        responses.push(message);
    }
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], SERVER_NAME);
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(
        responses[1]["result"]["tools"]
            .as_array()
            .expect("tools")
            .len(),
        TOOL_NAMES.len()
    );

    drop(stdin); // EOF
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("server exits after EOF")
        .expect("exit status");
    assert!(status.success(), "{status}");
    assert!(
        lines.next_line().await.expect("read stdout").is_none(),
        "nothing else on stdout"
    );
}

#[test]
fn version_and_help() {
    let output = std::process::Command::new(BINARY)
        .arg("--version")
        .output()
        .expect("run --version");
    assert!(output.status.success());
    let version = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        version.trim(),
        format!("cachyos-center-mcp {}", env!("CARGO_PKG_VERSION"))
    );

    let output = std::process::Command::new(BINARY)
        .arg("--help")
        .output()
        .expect("run --help");
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    for fragment in [
        "read-only",
        "stdin/stdout",
        "KI-Zugriff (MCP)",
        "CACHYOS_CENTER_LOG",
    ] {
        assert!(help.contains(fragment), "{fragment}: {help}");
    }
    assert!(Path::new(BINARY).is_file());
}
