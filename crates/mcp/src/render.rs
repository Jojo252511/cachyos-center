//! Turning tool output into MCP results: privacy filter, size limit and the
//! structured + text representation.

use cachyos_center_core::sanitize::{SanitizeContext, sanitize};
use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;
use serde_json::Value;

use crate::output::{Shrink, ToolErrorBody};

/// Maximum length of a single string value (characters). Longer values are
/// cut and the output is marked as `truncated`.
pub const MAX_STRING_CHARS: usize = 4096;

/// Replacement for absolute paths in path-free outputs.
pub const PATH_PLACEHOLDER: &str = "<path>";

/// Rendered success output.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Rendered {
    /// Structured content.
    pub value: Value,
    /// Pretty-printed JSON of `value` (text content).
    pub text: String,
}

/// Why rendering failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RenderError {
    Encode(String),
    TooLarge,
}

/// Serializes `output`, filters every string value and removes list entries
/// until the pretty-printed JSON is at most `max_bytes` long.
pub(crate) fn render<T: Serialize + Shrink>(
    mut output: T,
    ctx: &SanitizeContext,
    mask_paths: bool,
    max_bytes: usize,
) -> Result<Rendered, RenderError> {
    loop {
        let mut value =
            serde_json::to_value(&output).map_err(|e| RenderError::Encode(e.to_string()))?;
        if scrub(&mut value, ctx, mask_paths)
            && let Some(flag) = value.get_mut("truncated")
        {
            *flag = Value::Bool(true);
        }
        let text =
            serde_json::to_string_pretty(&value).map_err(|e| RenderError::Encode(e.to_string()))?;
        if text.len() <= max_bytes {
            return Ok(Rendered { value, text });
        }
        if !output.shrink() {
            return Err(RenderError::TooLarge);
        }
    }
}

/// Successful tool result: structured content plus the same JSON as text
/// (for hosts without structured-content support).
pub(crate) fn success_result(rendered: Rendered) -> CallToolResult {
    let mut result = CallToolResult::structured(rendered.value);
    result.content = vec![ContentBlock::text(rendered.text)];
    result
}

/// Error result (`isError: true`) with `{ "code", "message" }` as structured
/// content and as text.
pub(crate) fn error_result(body: &ToolErrorBody) -> CallToolResult {
    let value = serde_json::to_value(body).unwrap_or_else(
        |_| serde_json::json!({ "code": body.code.as_str(), "message": body.message }),
    );
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    let mut result = CallToolResult::structured_error(value);
    result.content = vec![ContentBlock::text(text)];
    result
}

/// Applies the privacy filter to every string value of `value`: optionally
/// masks absolute paths, then removes personal data with the core sanitizer
/// and cuts overlong strings. Returns `true` when a string was cut.
pub(crate) fn scrub(value: &mut Value, ctx: &SanitizeContext, mask_paths: bool) -> bool {
    match value {
        Value::String(s) => {
            let (clean, cut) = scrub_text(s, ctx, mask_paths);
            *s = clean;
            cut
        }
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |cut, v| scrub(v, ctx, mask_paths) | cut),
        Value::Object(map) => map
            .values_mut()
            .fold(false, |cut, v| scrub(v, ctx, mask_paths) | cut),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

/// Privacy filter for one text. Returns the filtered text and whether it was cut.
pub(crate) fn scrub_text(text: &str, ctx: &SanitizeContext, mask_paths: bool) -> (String, bool) {
    let masked = if mask_paths {
        mask_absolute_paths(text)
    } else {
        text.to_string()
    };
    let clean = sanitize(&masked, ctx);
    match clean.char_indices().nth(MAX_STRING_CHARS) {
        Some((cut_at, _)) => (format!("{}…", &clean[..cut_at]), true),
        None => (clean, false),
    }
}

/// Characters that may precede a path.
fn is_path_boundary(c: char) -> bool {
    c.is_whitespace() || matches!(c, '(' | '[' | '{' | '"' | '\'' | '`' | '=' | ',' | ';')
}

/// Characters that end a path.
fn ends_path(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '"' | '\'' | '`' | ')' | ']' | '}' | ',' | ';' | '<' | '>'
        )
}

/// Replaces absolute paths (`/x/...`, `~/...`) by [`PATH_PLACEHOLDER`].
///
/// A path starts at the beginning of the text or after whitespace or an
/// opening bracket/quote, so URLs (`https://...`) and fractions (`1/2`) are
/// kept, as is the bare root `/` (e.g. "free on /").
pub(crate) fn mask_absolute_paths(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut prev: Option<char> = None;
    while let Some(c) = rest.chars().next() {
        let at_boundary = prev.is_none_or(is_path_boundary);
        let starts_path = at_boundary && (c == '/' || rest.starts_with("~/"));
        if starts_path {
            let end = rest.find(ends_path).unwrap_or(rest.len());
            let token = &rest[..end];
            let path = token.trim_end_matches([':', '.', '!', '?']);
            if path.len() > 1 {
                out.push_str(PATH_PLACEHOLDER);
                out.push_str(&token[path.len()..]);
                prev = token.chars().last();
                rest = &rest[end..];
                continue;
            }
        }
        out.push(c);
        prev = Some(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::McpErrorCode;

    fn ctx() -> SanitizeContext {
        SanitizeContext {
            home: Some("/home/alice".into()),
            user: Some("alice".into()),
            hostname: Some("alice-laptop".into()),
        }
    }

    #[test]
    fn masks_paths_but_keeps_urls_fractions_and_root() {
        for (input, expected) in [
            (
                "bridge could not be loaded (/usr/lib/cachyos-center/libcachyos_center_alpm.so: cannot open)",
                "bridge could not be loaded (<path>: cannot open)",
            ),
            (
                "not found (searched next to the executable and in /usr/lib/cachyos-center)",
                "not found (searched next to the executable and in <path>)",
            ),
            ("see /etc/pacman.conf.pacnew.", "see <path>."),
            (
                "config ~/.config/cachyos-center/settings.toml",
                "config <path>",
            ),
            ("less than 1 GiB free on /", "less than 1 GiB free on /"),
            (
                "https://archlinux.org/news/foo",
                "https://archlinux.org/news/foo",
            ),
            ("AMD/ATI Navi 31, 1/2 done", "AMD/ATI Navi 31, 1/2 done"),
            ("path='/var/lib/pacman/db.lck'", "path='<path>'"),
            ("/", "/"),
            ("", ""),
            ("ünïcödé /tmp/ä ok", "ünïcödé <path> ok"),
        ] {
            assert_eq!(mask_absolute_paths(input), expected, "{input}");
        }
    }

    #[test]
    fn scrub_sanitizes_all_strings_and_cuts_long_ones() {
        let mut value = serde_json::json!({
            "summary": "upgraded by alice on alice-laptop in /home/alice/build",
            "nested": [{ "detail": "mail bob@example.org" }],
            "long": "x".repeat(MAX_STRING_CHARS + 10),
            "number": 5,
        });
        assert!(scrub(&mut value, &ctx(), false));
        assert_eq!(value["summary"], "upgraded by <user> on <host> in ~/build");
        assert_eq!(value["nested"][0]["detail"], "mail <email>");
        assert_eq!(
            value["long"].as_str().unwrap().chars().count(),
            MAX_STRING_CHARS + 1
        );
        assert_eq!(value["number"], 5);

        let mut value = serde_json::json!({ "detail": "failed in /home/alice/x" });
        assert!(!scrub(&mut value, &ctx(), true));
        assert_eq!(value["detail"], "failed in <path>");
    }

    #[test]
    fn error_result_has_structured_and_text_content() {
        let result = error_result(&ToolErrorBody {
            code: McpErrorCode::Busy,
            message: "database locked".into(),
        });
        assert_eq!(result.is_error, Some(true));
        let value = result.structured_content.clone().unwrap();
        assert_eq!(value["code"], "BUSY");
        assert_eq!(value["message"], "database locked");
        let text = result.content[0].as_text().unwrap().text.clone();
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), value);
    }
}
