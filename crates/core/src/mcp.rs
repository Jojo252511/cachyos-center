//! Constants of the local read-only MCP server (shared with the settings UI).

/// Name under which the server is registered in MCP hosts.
pub const SERVER_NAME: &str = "cachyos-center";

/// Binary name of the MCP server.
pub const BINARY_NAME: &str = "cachyos-center-mcp";

/// Exactly these tools are offered. None of them changes the system.
pub const TOOL_NAMES: [&str; 6] = [
    "system_get_summary",
    "updates_list",
    "packages_search",
    "packages_installed",
    "operations_recent",
    "health_get",
];

/// Example host configuration in the common `mcpServers` format
/// (Claude Desktop, Claude Code `.mcp.json` and others).
pub fn host_config(binary_path: &str) -> String {
    let value = serde_json::json!({
        "mcpServers": {
            SERVER_NAME: {
                "type": "stdio",
                "command": binary_path,
                "args": [],
                "env": {}
            }
        }
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_config_contains_absolute_path() {
        let cfg = host_config("/usr/bin/cachyos-center-mcp");
        let v: serde_json::Value = serde_json::from_str(&cfg).unwrap();
        assert_eq!(
            v["mcpServers"]["cachyos-center"]["command"],
            "/usr/bin/cachyos-center-mcp"
        );
        assert_eq!(v["mcpServers"]["cachyos-center"]["type"], "stdio");
    }

    #[test]
    fn no_mutating_tool_names() {
        const VERBS: [&str; 10] = [
            "install", "upgrade", "remove", "delete", "write", "exec", "shell", "apply", "run",
            "set",
        ];
        for name in TOOL_NAMES {
            for segment in name.split('_') {
                assert!(!VERBS.contains(&segment), "{name}");
            }
        }
    }
}
