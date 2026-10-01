//! The six read-only tools: names, descriptions, JSON schemas and annotations.

use std::sync::{Arc, LazyLock};

use cachyos_center_core::mcp::TOOL_NAMES;
use rmcp::handler::server::tool::{schema_for_input, schema_for_output};
use rmcp::model::{JsonObject, Tool, ToolAnnotations};
use schemars::JsonSchema;
use serde::Serialize;

use crate::args::{
    NoArguments, OperationsRecentArguments, PackagesInstalledArguments, PackagesSearchArguments,
};
use crate::output::{
    HealthOutput, OperationsRecentOutput, PackagesInstalledOutput, PackagesSearchOutput,
    SystemSummaryOutput, ToolErrorBody, UpdatesListOutput,
};

/// The tools offered by the server. The order matches
/// [`cachyos_center_core::mcp::TOOL_NAMES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolKind {
    SystemGetSummary,
    UpdatesList,
    PackagesSearch,
    PackagesInstalled,
    OperationsRecent,
    HealthGet,
}

impl ToolKind {
    pub const ALL: [ToolKind; 6] = [
        Self::SystemGetSummary,
        Self::UpdatesList,
        Self::PackagesSearch,
        Self::PackagesInstalled,
        Self::OperationsRecent,
        Self::HealthGet,
    ];

    /// Tool name as offered to MCP hosts (taken from the core constant).
    pub fn name(self) -> &'static str {
        TOOL_NAMES[self as usize]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    /// Whether absolute paths are masked in all string values of the output.
    /// Package tools keep the public package metadata (descriptions) as is.
    pub fn masks_paths(self) -> bool {
        !matches!(self, Self::PackagesSearch | Self::PackagesInstalled)
    }

    fn title(self) -> &'static str {
        match self {
            Self::SystemGetSummary => "System summary",
            Self::UpdatesList => "Available updates (last check)",
            Self::PackagesSearch => "Search repository packages",
            Self::PackagesInstalled => "Installed packages",
            Self::OperationsRecent => "Recent package operations",
            Self::HealthGet => "System health",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::SystemGetSummary => {
                "Read-only. Returns a sanitized summary of this computer: operating system, \
                 running kernel and uptime, CPU and GPU models, memory, usage of the root \
                 filesystem, desktop session and whether package functions are available. \
                 Contains no serial numbers, user names or host names. Changes nothing."
            }
            Self::UpdatesList => {
                "Read-only. Returns the result of the last update check made by cachyos-center: \
                 available repository updates (package, repository, old and new version, flags, \
                 download size) and held-back packages. Never runs a check and never downloads \
                 or installs anything. The data can be outdated: present it as the current state \
                 only if `stale` is false (`status` = `fresh`); otherwise tell the user what \
                 `note` says."
            }
            Self::PackagesSearch => {
                "Read-only. Searches the pacman repositories configured on this computer \
                 (case-insensitive substring of package name or description) and returns \
                 matching packages with repository, version and installation state. No AUR or \
                 web search; installs nothing."
            }
            Self::PackagesInstalled => {
                "Read-only. Lists the packages installed on this computer with version, origin \
                 (`repo` or `localOrAur` for foreign/AUR packages), install reason and update \
                 availability, optionally filtered by a substring. Paginated: pass `nextCursor` \
                 of a result as `cursor` (with the same `query`) to get the next page."
            }
            Self::OperationsRecent => {
                "Read-only. Returns sanitized summaries of recent package operations (actions in \
                 cachyos-center, its timer and external pacman transactions from the pacman log), \
                 newest first: kind, state or outcome, times, change counts and up to 20 package \
                 names. No raw logs and no file paths."
            }
            Self::HealthGet => {
                "Read-only. Returns the health report of this computer: number of .pacnew and \
                 .pacsave files, pacman database lock, reboot recommendation with reasons, \
                 prepared offline update and other update mechanisms (unit names), reasons that \
                 block unattended updates and health hints with kind, severity and short detail. \
                 No file contents and no file paths."
            }
        }
    }

    fn input_schema(self) -> Arc<JsonObject> {
        let schema = match self {
            Self::SystemGetSummary | Self::UpdatesList | Self::HealthGet => {
                schema_for_input::<NoArguments>()
            }
            Self::PackagesSearch => schema_for_input::<PackagesSearchArguments>(),
            Self::PackagesInstalled => schema_for_input::<PackagesInstalledArguments>(),
            Self::OperationsRecent => schema_for_input::<OperationsRecentArguments>(),
        };
        let schema = schema.unwrap_or_else(|e| {
            tracing::error!(tool = self.name(), "invalid input schema: {e}");
            Arc::new(empty_object_schema())
        });
        if schema.contains_key("properties") {
            return schema;
        }
        // Tools without arguments: some hosts expect `properties` on every object schema.
        let mut schema = schema.as_ref().clone();
        schema.insert(
            "properties".into(),
            serde_json::Value::Object(JsonObject::new()),
        );
        Arc::new(schema)
    }

    fn output_schema(self) -> Arc<JsonObject> {
        match self {
            Self::SystemGetSummary => output_schema::<SystemSummaryOutput>(),
            Self::UpdatesList => output_schema::<UpdatesListOutput>(),
            Self::PackagesSearch => output_schema::<PackagesSearchOutput>(),
            Self::PackagesInstalled => output_schema::<PackagesInstalledOutput>(),
            Self::OperationsRecent => output_schema::<OperationsRecentOutput>(),
            Self::HealthGet => output_schema::<HealthOutput>(),
        }
    }

    /// Complete MCP tool definition.
    pub fn definition(self) -> Tool {
        let annotations = ToolAnnotations::with_title(self.title())
            .read_only(true)
            .destructive(false)
            .idempotent(true)
            .open_world(false);
        Tool::new(self.name(), self.description(), self.input_schema())
            .with_title(self.title())
            .with_raw_output_schema(self.output_schema())
            .with_annotations(annotations)
    }
}

/// Definitions of all tools, in the order of `TOOL_NAMES`.
pub fn definitions() -> Vec<Tool> {
    static TOOLS: LazyLock<Vec<Tool>> = LazyLock::new(|| {
        ToolKind::ALL
            .into_iter()
            .map(ToolKind::definition)
            .collect()
    });
    TOOLS.clone()
}

/// Structured content of a tool: either the success output or, for results
/// with `isError: true`, the error body. Only used for the schema.
#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
enum StructuredContent<T> {
    /// Successful result.
    Success(T),
    /// Error result (`isError: true`).
    Error(ToolErrorBody),
}

/// Output schema of a tool. It describes the success output and the error
/// body, because error results carry structured content as well; the root is
/// `type: object` for hosts that require it.
fn output_schema<T: JsonSchema + 'static>() -> Arc<JsonObject> {
    let mut schema = schema_for_output::<StructuredContent<T>>().as_ref().clone();
    schema.insert("type".into(), serde_json::Value::String("object".into()));
    Arc::new(schema)
}

fn empty_object_schema() -> JsonObject {
    let mut schema = JsonObject::new();
    schema.insert("type".into(), serde_json::Value::String("object".into()));
    schema
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn names_come_from_the_core_constant() {
        let names: Vec<&str> = ToolKind::ALL.iter().map(|t| t.name()).collect();
        assert_eq!(names, TOOL_NAMES);
        for tool in ToolKind::ALL {
            assert_eq!(ToolKind::from_name(tool.name()), Some(tool));
        }
        assert_eq!(ToolKind::from_name("install_package"), None);
        assert_eq!(ToolKind::from_name(""), None);
    }

    #[test]
    fn definitions_are_read_only_and_have_fixed_schemas() {
        let tools = definitions();
        assert_eq!(tools.len(), TOOL_NAMES.len());
        for (tool, name) in tools.iter().zip(TOOL_NAMES) {
            assert_eq!(tool.name, name);
            let description = tool.description.as_deref().unwrap();
            assert!(description.starts_with("Read-only."), "{name}");
            let annotations = tool.annotations.as_ref().unwrap();
            assert_eq!(annotations.read_only_hint, Some(true), "{name}");
            assert_eq!(annotations.destructive_hint, Some(false), "{name}");
            assert_eq!(annotations.open_world_hint, Some(false), "{name}");

            let input = Value::Object(tool.input_schema.as_ref().clone());
            assert_eq!(input["type"], "object", "{name}");
            assert_eq!(input["additionalProperties"], false, "{name}: {input}");

            let output = Value::Object(tool.output_schema.as_ref().unwrap().as_ref().clone());
            assert_eq!(output["type"], "object", "{name}");
            let branches = output["anyOf"].as_array().unwrap();
            assert_eq!(branches.len(), 2, "{name}: {output}");
            let text = output.to_string();
            assert!(text.contains("ToolErrorBody"), "{name}");
            assert!(text.contains("truncated"), "{name}");
        }
    }

    #[test]
    fn input_schemas_declare_the_bounds() {
        let search = Value::Object(ToolKind::PackagesSearch.input_schema().as_ref().clone());
        assert_eq!(search["required"], serde_json::json!(["query"]));
        assert_eq!(search["properties"]["query"]["minLength"], 2);
        assert_eq!(search["properties"]["query"]["maxLength"], 100);
        assert_eq!(search["properties"]["limit"]["minimum"], 1);
        assert_eq!(search["properties"]["limit"]["maximum"], 50);
        assert_eq!(search["properties"]["limit"]["default"], 20);

        let installed = Value::Object(ToolKind::PackagesInstalled.input_schema().as_ref().clone());
        assert!(
            installed
                .get("required")
                .is_none_or(|r| r == &serde_json::json!([]))
        );
        assert_eq!(installed["properties"]["limit"]["maximum"], 100);
        assert_eq!(installed["properties"]["limit"]["default"], 50);
        assert_eq!(installed["properties"]["query"]["maxLength"], 100);
        assert_eq!(installed["properties"]["cursor"]["maxLength"], 64);

        let operations = Value::Object(ToolKind::OperationsRecent.input_schema().as_ref().clone());
        assert_eq!(operations["properties"]["limit"]["minimum"], 1);
        assert_eq!(operations["properties"]["limit"]["maximum"], 20);
        assert_eq!(operations["properties"]["limit"]["default"], 10);
    }

    #[test]
    fn health_schema_has_no_config_file_paths() {
        let output = Value::Object(ToolKind::HealthGet.output_schema().as_ref().clone());
        let text = output.to_string();
        assert!(!text.contains("configFiles"));
        assert!(text.contains("pacnewCount"));
        assert!(text.contains("updateBlockers"));
    }
}
