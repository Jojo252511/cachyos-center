//! `cachyos-center-mcp` – local, read-only MCP server of cachyos-center.
//!
//! Started by an MCP host as a child process; speaks MCP over stdin/stdout
//! until the host closes stdin. stdout carries only the protocol, all logs go
//! to stderr.

use std::io::IsTerminal;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use cachyos_center_core::mcp::BINARY_NAME;
use cachyos_center_service::AppCore;
use clap::Parser;

const LONG_ABOUT: &str = "\
Local, read-only MCP (Model Context Protocol) server of cachyos-center.

An MCP host such as Claude Code or Claude Desktop starts this program and
talks to it over stdin/stdout. It opens no network socket and changes
nothing on the system. It offers six read-only tools: system_get_summary,
updates_list, packages_search, packages_installed, operations_recent and
health_get.

Access is disabled by default. Enable it in cachyos-center under
Einstellungen -> KI/MCP; while it is disabled every tool call returns the
error code UNAVAILABLE.

Logs are written to stderr; the level is set with CACHYOS_CENTER_LOG
(e.g. CACHYOS_CENTER_LOG=debug).";

#[derive(Debug, Parser)]
#[command(
    name = BINARY_NAME,
    version,
    about = "Local, read-only MCP server of cachyos-center (stdio)",
    long_about = LONG_ABOUT
)]
struct Cli {}

fn init_logging() {
    // Default: one line per tool call (tool, duration, error code; never
    // arguments or results), warnings of the MCP library.
    let filter = tracing_subscriber::EnvFilter::try_from_env("CACHYOS_CENTER_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,cachyos_center_mcp=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .init();
}

fn main() -> ExitCode {
    let Cli {} = Cli::parse();
    init_logging();
    if std::io::stdin().is_terminal() {
        tracing::warn!(
            "{BINARY_NAME} speaks MCP over stdin/stdout and is meant to be started by an MCP host (see --help)"
        );
    }
    let core = match AppCore::from_env() {
        Ok(core) => core,
        Err(e) => {
            tracing::error!("cannot start: {e}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(16)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            tracing::error!("cannot start the async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(cachyos_center_mcp::serve_stdio(Arc::new(core)));
    // Do not wait for reads that are still stuck after a tool timeout.
    runtime.shutdown_timeout(Duration::from_secs(2));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e:#}");
            ExitCode::FAILURE
        }
    }
}
