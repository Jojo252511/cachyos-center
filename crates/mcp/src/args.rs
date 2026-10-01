//! Tool arguments: fixed JSON schemas (schemars) and validation.
//!
//! The arguments are parsed by this crate, not by rmcp, so that the
//! "MCP disabled" check runs before any argument handling and every argument
//! error is reported the same way (`isError` result with code `INVALID_INPUT`).
//! Unknown fields are rejected (`additionalProperties: false`). Optional
//! arguments are `Option<T>` (absent or `null` = default) but are published
//! with a single JSON type (`schemars(with = ...)`, not required), which maps
//! better onto the function-calling dialects of model providers. The
//! `skip_serializing_if` attributes only keep schemars from publishing
//! `"default": null`; the structs are never serialized.

use cachyos_center_core::validate;
use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// Minimum length of the `packages_search` query (characters, after trimming).
pub const SEARCH_QUERY_MIN_CHARS: usize = 2;
/// Maximum length of a query (characters, after trimming).
pub const QUERY_MAX_CHARS: usize = validate::MAX_QUERY_LEN;
/// Maximum length of a `packages_installed` cursor.
pub const CURSOR_MAX_LEN: usize = 64;

/// `packages_search.limit`: default and bounds.
pub const SEARCH_LIMIT_DEFAULT: u32 = 20;
pub const SEARCH_LIMIT_MIN: u32 = 1;
pub const SEARCH_LIMIT_MAX: u32 = 50;

/// `packages_installed.limit`: default and bounds.
pub const INSTALLED_LIMIT_DEFAULT: u32 = 50;
pub const INSTALLED_LIMIT_MIN: u32 = 1;
pub const INSTALLED_LIMIT_MAX: u32 = 100;

/// `operations_recent.limit`: default and bounds.
pub const OPERATIONS_LIMIT_DEFAULT: u32 = 10;
pub const OPERATIONS_LIMIT_MIN: u32 = 1;
pub const OPERATIONS_LIMIT_MAX: u32 = 20;

/// Arguments failed to parse or validate. The message is shown to the caller
/// (English, names the offending argument).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidArguments(pub String);

impl std::fmt::Display for InvalidArguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InvalidArguments {}

/// Deserializes the raw `arguments` object of a `tools/call` request.
pub fn parse<T: DeserializeOwned>(arguments: JsonObject) -> Result<T, InvalidArguments> {
    serde_json::from_value(serde_json::Value::Object(arguments))
        .map_err(|e| InvalidArguments(format!("invalid arguments: {e}")))
}

/// Arguments of `system_get_summary`, `updates_list` and `health_get`: none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArguments {}

/// Arguments of `packages_search`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackagesSearchArguments {
    /// Text to look for in package names and descriptions (case-insensitive substring, 2-100 characters).
    #[schemars(length(min = SEARCH_QUERY_MIN_CHARS, max = QUERY_MAX_CHARS))]
    pub query: String,
    /// Maximum number of results (1-50, default 20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "u32",
        range(min = SEARCH_LIMIT_MIN, max = SEARCH_LIMIT_MAX),
        extend("default" = SEARCH_LIMIT_DEFAULT)
    )]
    pub limit: Option<u32>,
}

/// Validated `packages_search` arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    /// Trimmed query.
    pub query: String,
    pub limit: u32,
}

impl PackagesSearchArguments {
    pub fn validate(self) -> Result<SearchRequest, InvalidArguments> {
        let query = validate::search_query(&self.query)
            .map_err(|e| InvalidArguments(format!("query: {}", e.message)))?;
        if query.chars().count() < SEARCH_QUERY_MIN_CHARS {
            return Err(InvalidArguments(format!(
                "query: the search query needs at least {SEARCH_QUERY_MIN_CHARS} characters"
            )));
        }
        let limit = bounded(
            "limit",
            self.limit,
            SEARCH_LIMIT_DEFAULT,
            SEARCH_LIMIT_MIN,
            SEARCH_LIMIT_MAX,
        )?;
        Ok(SearchRequest { query, limit })
    }
}

/// Arguments of `packages_installed`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackagesInstalledArguments {
    /// Optional filter: case-insensitive substring of package name or description (at most 100 characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String", length(max = QUERY_MAX_CHARS))]
    pub query: Option<String>,
    /// Maximum number of packages per page (1-100, default 50).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "u32",
        range(min = INSTALLED_LIMIT_MIN, max = INSTALLED_LIMIT_MAX),
        extend("default" = INSTALLED_LIMIT_DEFAULT)
    )]
    pub limit: Option<u32>,
    /// Opaque cursor from `nextCursor` of the previous page (same `query` required); omit for the first page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String", length(max = CURSOR_MAX_LEN))]
    pub cursor: Option<String>,
}

/// Validated `packages_installed` arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRequest {
    /// Trimmed filter, `None` when absent or empty.
    pub query: Option<String>,
    pub limit: u32,
    pub offset: u32,
    /// Normalized filter the cursors are bound to.
    pub filter_key: String,
}

impl PackagesInstalledArguments {
    pub fn validate(self) -> Result<InstalledRequest, InvalidArguments> {
        let query = match self.query {
            Some(q) => {
                let q = validate::search_query(&q)
                    .map_err(|e| InvalidArguments(format!("query: {}", e.message)))?;
                (!q.is_empty()).then_some(q)
            }
            None => None,
        };
        let limit = bounded(
            "limit",
            self.limit,
            INSTALLED_LIMIT_DEFAULT,
            INSTALLED_LIMIT_MIN,
            INSTALLED_LIMIT_MAX,
        )?;
        let filter_key = query.as_deref().unwrap_or_default().to_lowercase();
        let offset = match self.cursor.as_deref().map(str::trim) {
            None | Some("") => 0,
            Some(cursor) => decode_cursor(cursor, &filter_key)?,
        };
        Ok(InstalledRequest {
            query,
            limit,
            offset,
            filter_key,
        })
    }
}

/// Arguments of `operations_recent`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationsRecentArguments {
    /// Maximum number of operations, newest first (1-20, default 10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "u32",
        range(min = OPERATIONS_LIMIT_MIN, max = OPERATIONS_LIMIT_MAX),
        extend("default" = OPERATIONS_LIMIT_DEFAULT)
    )]
    pub limit: Option<u32>,
}

impl OperationsRecentArguments {
    /// Validated limit.
    pub fn validate(self) -> Result<u32, InvalidArguments> {
        bounded(
            "limit",
            self.limit,
            OPERATIONS_LIMIT_DEFAULT,
            OPERATIONS_LIMIT_MIN,
            OPERATIONS_LIMIT_MAX,
        )
    }
}

fn bounded(
    name: &str,
    value: Option<u32>,
    default: u32,
    min: u32,
    max: u32,
) -> Result<u32, InvalidArguments> {
    validate::limit(value, default, min, max)
        .map_err(|_| InvalidArguments(format!("{name} must be between {min} and {max}")))
}

// ---- Pagination cursor ------------------------------------------------------

const CURSOR_PREFIX: &str = "c1";

/// Opaque cursor for the page starting at `offset`, bound to `filter_key`.
pub fn encode_cursor(offset: u32, filter_key: &str) -> String {
    format!("{CURSOR_PREFIX}{offset:08x}{:08x}", fingerprint(filter_key))
}

/// Offset encoded in `cursor`; fails when the cursor is malformed or was
/// issued for another filter.
pub fn decode_cursor(cursor: &str, filter_key: &str) -> Result<u32, InvalidArguments> {
    let invalid = || InvalidArguments("cursor: not a cursor returned by packages_installed".into());
    let body = cursor
        .strip_prefix(CURSOR_PREFIX)
        .filter(|b| b.len() == 16 && b.bytes().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(invalid)?;
    let offset = u32::from_str_radix(&body[..8], 16).map_err(|_| invalid())?;
    let print = u32::from_str_radix(&body[8..], 16).map_err(|_| invalid())?;
    if print != fingerprint(filter_key) {
        return Err(InvalidArguments(
            "cursor: the cursor belongs to a different query; start again without cursor".into(),
        ));
    }
    Ok(offset)
}

/// FNV-1a (32 bit) of the normalized filter.
fn fingerprint(filter_key: &str) -> u32 {
    filter_key.bytes().fold(0x811c_9dc5_u32, |hash, b| {
        (hash ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(value: serde_json::Value) -> JsonObject {
        match value {
            serde_json::Value::Object(map) => map,
            other => panic!("not an object: {other}"),
        }
    }

    #[test]
    fn search_defaults_and_bounds() {
        let ok = parse::<PackagesSearchArguments>(obj(json!({"query": "  firefox "})))
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(ok.query, "firefox");
        assert_eq!(ok.limit, SEARCH_LIMIT_DEFAULT);

        for (args, fragment) in [
            (json!({"query": "a"}), "at least 2"),
            (json!({"query": "  a  "}), "at least 2"),
            (json!({"query": "x".repeat(101)}), "too long"),
            (json!({"query": "a\u{7}b"}), "control"),
            (json!({"query": "linux", "limit": 0}), "between 1 and 50"),
            (json!({"query": "linux", "limit": 51}), "between 1 and 50"),
        ] {
            let err = parse::<PackagesSearchArguments>(obj(args.clone()))
                .and_then(PackagesSearchArguments::validate)
                .unwrap_err();
            assert!(err.0.contains(fragment), "{args}: {err}");
        }
    }

    #[test]
    fn type_errors_and_unknown_fields_are_rejected() {
        for args in [
            json!({}),
            json!({"query": 5}),
            json!({"query": "linux", "limit": "5"}),
            json!({"query": "linux", "limit": -1}),
            json!({"query": "linux", "limit": 2.5}),
            json!({"query": "linux", "repository": "core"}),
        ] {
            let err = parse::<PackagesSearchArguments>(obj(args.clone())).unwrap_err();
            assert!(err.0.starts_with("invalid arguments:"), "{args}: {err}");
        }
        assert!(parse::<NoArguments>(obj(json!({"verbose": true}))).is_err());
        assert!(parse::<NoArguments>(JsonObject::new()).is_ok());
        // `null` is accepted for optional arguments.
        let args = parse::<OperationsRecentArguments>(obj(json!({"limit": null}))).unwrap();
        assert_eq!(args.validate().unwrap(), OPERATIONS_LIMIT_DEFAULT);
    }

    #[test]
    fn installed_and_operations_bounds() {
        let first = parse::<PackagesInstalledArguments>(JsonObject::new())
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(first.limit, INSTALLED_LIMIT_DEFAULT);
        assert_eq!(first.offset, 0);
        assert_eq!(first.query, None);
        let empty_query = parse::<PackagesInstalledArguments>(obj(json!({"query": "   "})))
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(empty_query.query, None);
        for limit in [0, 101] {
            assert!(
                parse::<PackagesInstalledArguments>(obj(json!({ "limit": limit })))
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        for (limit, ok) in [(1, true), (20, true), (0, false), (21, false)] {
            let result = parse::<OperationsRecentArguments>(obj(json!({ "limit": limit })))
                .unwrap()
                .validate();
            assert_eq!(result.is_ok(), ok, "{limit}");
        }
    }

    #[test]
    fn cursor_roundtrip_and_binding() {
        let cursor = encode_cursor(150, "lib");
        assert_eq!(cursor.len(), 18);
        assert_eq!(decode_cursor(&cursor, "lib").unwrap(), 150);
        assert!(
            decode_cursor(&cursor, "")
                .unwrap_err()
                .0
                .contains("different query")
        );
        for bad in [
            "",
            "c1",
            "c1zz",
            "x100000096811c9dc5",
            "c1000000960000000g",
            "150",
        ] {
            assert!(decode_cursor(bad, "lib").is_err(), "{bad}");
        }
        let args = parse::<PackagesInstalledArguments>(obj(json!({
            "query": "LIB",
            "cursor": cursor,
        })))
        .unwrap()
        .validate()
        .unwrap();
        assert_eq!(args.offset, 150);
        assert_eq!(args.query.as_deref(), Some("LIB"));
    }
}
