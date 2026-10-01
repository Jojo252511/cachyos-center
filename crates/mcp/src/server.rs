//! MCP server handler: tool dispatch, access switch, timeouts and error mapping.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cachyos_center_core::mcp::SERVER_NAME;
use cachyos_center_core::package::{
    CatalogInstallFilter, CatalogQuery, InstalledFilter, InstalledQuery,
};
use cachyos_center_core::sanitize::SanitizeContext;
use cachyos_center_core::{AppError, now};
use cachyos_center_service::ReadApi;
use rmcp::model::{
    CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, Implementation,
    JsonObject, ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities,
    ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};
use tokio::sync::Semaphore;

use crate::args::{
    InvalidArguments, NoArguments, OperationsRecentArguments, PackagesInstalledArguments,
    PackagesSearchArguments, parse,
};
use crate::output::{
    AutoUpdateOutput, HealthOutput, McpErrorCode, OperationsRecentOutput, PackagesInstalledOutput,
    PackagesSearchOutput, SystemSummaryOutput, ToolErrorBody, UpdatesListOutput,
};
use crate::render::{RenderError, Rendered, error_result, render, scrub_text, success_result};
use crate::tools::{ToolKind, definitions};

/// Message of every tool call while MCP access is switched off.
pub const DISABLED_MESSAGE: &str = "MCP access is disabled in cachyos-center (Einstellungen → KI-Zugriff (MCP) / Settings → AI access (MCP))";

/// Maximum time for one tool call (including waiting for a free slot).
pub const TOOL_TIMEOUT: Duration = Duration::from_secs(15);

/// Maximum size of the serialized output of one tool (pretty-printed JSON,
/// identical to the text content).
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Maximum number of tool calls that read the system at the same time.
pub const MAX_CONCURRENT_CALLS: usize = 4;

/// `instructions` of the `initialize` result.
pub const INSTRUCTIONS: &str = "cachyos-center MCP server: local and read-only. All data \
comes from this computer (system information, pacman package databases, the result of the last \
update check, operation history, health checks). No tool changes the system, installs, updates \
or removes packages, runs an update check or uses the network. updates_list returns the result of \
the last update check made by cachyos-center, which may be stale: check `status`, `stale`, \
`ageSeconds` and `note` before calling it the current state. Access must be enabled by the user \
in cachyos-center (Einstellungen → KI-Zugriff (MCP) / Settings → AI access (MCP)); otherwise every \
tool returns the error code UNAVAILABLE. \
Outputs are sanitized (home paths, user and host names are masked) and limited to 64 KiB \
(`truncated` is then true).";

/// Tunables of the server (tests use short timeouts and a fixed sanitizer context).
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// Timeout of one tool call.
    pub timeout: Duration,
    /// Size limit of one tool output in bytes.
    pub max_output_bytes: usize,
    /// Concurrent tool calls.
    pub max_concurrent_calls: usize,
    /// Personal values removed from all outputs.
    pub sanitize: SanitizeContext,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            timeout: TOOL_TIMEOUT,
            max_output_bytes: MAX_OUTPUT_BYTES,
            max_concurrent_calls: MAX_CONCURRENT_CALLS,
            sanitize: SanitizeContext::from_env(),
        }
    }
}

/// Why a tool call did not produce output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ToolFailure {
    /// `mcpEnabled` is off in the user settings.
    Disabled,
    InvalidInput(String),
    App(AppError),
    TimedOut(Duration),
    Internal(String),
}

impl From<AppError> for ToolFailure {
    fn from(error: AppError) -> Self {
        Self::App(error)
    }
}

impl From<InvalidArguments> for ToolFailure {
    fn from(error: InvalidArguments) -> Self {
        Self::InvalidInput(error.0)
    }
}

impl From<RenderError> for ToolFailure {
    fn from(error: RenderError) -> Self {
        match error {
            RenderError::Encode(e) => Self::Internal(format!("cannot encode the tool output: {e}")),
            RenderError::TooLarge => {
                Self::Internal("the tool output exceeds the size limit".to_string())
            }
        }
    }
}

impl ToolFailure {
    /// Structured error body with a sanitized message.
    pub(crate) fn body(&self, ctx: &SanitizeContext) -> ToolErrorBody {
        let (code, message) = match self {
            Self::Disabled => {
                return ToolErrorBody {
                    code: McpErrorCode::Unavailable,
                    message: DISABLED_MESSAGE.to_string(),
                };
            }
            Self::InvalidInput(message) => (McpErrorCode::InvalidInput, message.clone()),
            Self::App(error) => (McpErrorCode::from_app(error.code), error.message.clone()),
            Self::TimedOut(limit) => (
                McpErrorCode::Unavailable,
                format!(
                    "the tool timed out after {} while reading the system state",
                    format_duration(*limit)
                ),
            ),
            Self::Internal(message) => (McpErrorCode::Internal, message.clone()),
        };
        ToolErrorBody {
            code,
            message: scrub_text(&message, ctx, true).0,
        }
    }
}

fn format_duration(d: Duration) -> String {
    if d.subsec_millis() == 0 {
        format!("{} s", d.as_secs())
    } else {
        format!("{} ms", d.as_millis())
    }
}

/// The MCP server. Cheap to clone; all clones share the read service.
#[derive(Clone)]
pub struct McpServer {
    api: Arc<dyn ReadApi>,
    options: Arc<ServerOptions>,
    permits: Arc<Semaphore>,
}

impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer")
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl McpServer {
    /// Server on top of `api` with the default options.
    pub fn new(api: Arc<dyn ReadApi>) -> Self {
        Self::with_options(api, ServerOptions::default())
    }

    pub fn with_options(api: Arc<dyn ReadApi>, options: ServerOptions) -> Self {
        Self {
            api,
            permits: Arc::new(Semaphore::new(options.max_concurrent_calls.max(1))),
            options: Arc::new(options),
        }
    }

    /// Definitions of the offered tools (exactly `TOOL_NAMES`).
    pub fn tools() -> Vec<Tool> {
        definitions()
    }

    /// Server identity, capabilities (tools only) and instructions.
    pub fn config() -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(SERVER_NAME, env!("CARGO_PKG_VERSION"))
                    .with_title("cachyos-center (read-only)")
                    .with_description("Local, read-only MCP server of cachyos-center"),
            )
            .with_instructions(INSTRUCTIONS)
    }

    /// Runs one tool call. Never fails: every problem becomes an error result.
    pub async fn call(&self, tool: ToolKind, arguments: JsonObject) -> CallToolResult {
        let started = Instant::now();
        let outcome = self.run(tool, arguments).await;
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match outcome {
            Ok(rendered) => {
                tracing::info!(
                    tool = tool.name(),
                    elapsed_ms,
                    bytes = rendered.text.len(),
                    "tool call answered"
                );
                success_result(rendered)
            }
            Err(failure) => {
                let body = failure.body(&self.options.sanitize);
                tracing::info!(
                    tool = tool.name(),
                    elapsed_ms,
                    code = body.code.as_str(),
                    "tool call refused"
                );
                error_result(&body)
            }
        }
    }

    async fn run(&self, tool: ToolKind, arguments: JsonObject) -> Result<Rendered, ToolFailure> {
        let timeout = self.options.timeout;
        let deadline = tokio::time::Instant::now() + timeout;
        let permit = match tokio::time::timeout_at(
            deadline,
            Arc::clone(&self.permits).acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => return Err(ToolFailure::Internal("the server is shutting down".into())),
            Err(_) => return Err(ToolFailure::TimedOut(timeout)),
        };
        let api = Arc::clone(&self.api);
        let options = Arc::clone(&self.options);
        // All file and database access happens on the blocking pool. After a
        // timeout the task keeps its permit until it returns, so stuck reads
        // cannot pile up beyond `max_concurrent_calls`.
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            execute(tool, api.as_ref(), arguments, &options)
        });
        match tokio::time::timeout_at(deadline, task).await {
            Ok(Ok(result)) => result,
            Ok(Err(join_error)) => Err(ToolFailure::Internal(if join_error.is_panic() {
                "the tool failed unexpectedly".to_string()
            } else {
                "the tool was cancelled".to_string()
            })),
            Err(_) => Err(ToolFailure::TimedOut(timeout)),
        }
    }
}

/// Blocking part of a tool call. The access switch is read first, on every
/// call, so that switching MCP off in the app takes effect immediately.
fn execute(
    tool: ToolKind,
    api: &dyn ReadApi,
    arguments: JsonObject,
    options: &ServerOptions,
) -> Result<Rendered, ToolFailure> {
    if !api.user_settings().mcp_enabled {
        return Err(ToolFailure::Disabled);
    }
    let ctx = &options.sanitize;
    let mask = tool.masks_paths();
    let max = options.max_output_bytes;
    let rendered = match tool {
        ToolKind::SystemGetSummary => {
            parse::<NoArguments>(arguments)?;
            render(
                SystemSummaryOutput::from(api.system_summary()?),
                ctx,
                mask,
                max,
            )
        }
        ToolKind::UpdatesList => {
            parse::<NoArguments>(arguments)?;
            render(
                UpdatesListOutput::new(api.updates()?, now()),
                ctx,
                mask,
                max,
            )
        }
        ToolKind::PackagesSearch => {
            let request = parse::<PackagesSearchArguments>(arguments)?.validate()?;
            let hits = api.search(&CatalogQuery {
                query: request.query.clone(),
                repository: None,
                install_filter: CatalogInstallFilter::Any,
                limit: request.limit,
            })?;
            render(
                PackagesSearchOutput::new(request.query, request.limit, hits),
                ctx,
                mask,
                max,
            )
        }
        ToolKind::PackagesInstalled => {
            let request = parse::<PackagesInstalledArguments>(arguments)?.validate()?;
            let page = api.installed(&InstalledQuery {
                query: request.query.clone(),
                filter: InstalledFilter::All,
                offset: request.offset,
                limit: request.limit,
            })?;
            render(
                PackagesInstalledOutput::new(page, &request.filter_key),
                ctx,
                mask,
                max,
            )
        }
        ToolKind::OperationsRecent => {
            let limit = parse::<OperationsRecentArguments>(arguments)?.validate()?;
            let entries = api.recent_activity(limit)?;
            render(OperationsRecentOutput::new(entries, limit), ctx, mask, max)
        }
        ToolKind::HealthGet => {
            parse::<NoArguments>(arguments)?;
            let mut output = HealthOutput::from(api.health()?);
            output.auto_update = api.auto_update().ok().map(AutoUpdateOutput::from);
            render(output, ctx, mask, max)
        }
    };
    Ok(rendered?)
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerConfig {
        Self::config()
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut result = ListToolsResult::with_all_items(definitions());
        // Cache hints are required from protocol version 2026-07-28 on.
        if context
            .protocol_version()
            .is_some_and(|v| v >= ProtocolVersion::V_2026_07_28)
        {
            result.ttl_ms = Some(0);
            result.cache_scope = Some(CacheScope::Public);
        }
        Ok(result)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let Some(tool) = ToolKind::from_name(&request.name) else {
            tracing::warn!("call of an unknown tool rejected");
            return Err(ErrorData::invalid_params("tool not found", None));
        };
        Ok(self
            .call(tool, request.arguments.unwrap_or_default())
            .await
            .into())
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        ToolKind::from_name(name).map(ToolKind::definition)
    }
}
