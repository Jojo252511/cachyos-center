//! Removal of personal data from diagnostic reports and log excerpts.
//!
//! Applied to everything that leaves the machine through the clipboard or the
//! MCP server. The diagnostic report is built from structured data; this
//! module is the second line of defense for free text (error messages,
//! pacman output).

/// Personal values of the current session that must not appear in output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SanitizeContext {
    pub home: Option<String>,
    pub user: Option<String>,
    pub hostname: Option<String>,
}

impl SanitizeContext {
    /// Collects home directory, user name and host name of the current process.
    pub fn from_env() -> Self {
        let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        Self {
            home: std::env::var("HOME").ok().filter(|h| h.len() > 1),
            user: std::env::var("USER")
                .ok()
                .or_else(|| std::env::var("LOGNAME").ok())
                .filter(|u| !u.is_empty()),
            hostname,
        }
    }
}

/// Replaces personal data in `input`.
pub fn sanitize(input: &str, ctx: &SanitizeContext) -> String {
    let mut out = input.to_string();
    if let Some(home) = ctx.home.as_deref().filter(|h| h.len() > 1) {
        out = out.replace(home, "~");
    }
    out = mask_home_dirs(&out);
    if let Some(user) = ctx.user.as_deref().filter(|u| u.len() >= 3) {
        out = replace_word(&out, user, "<user>");
    }
    if let Some(host) = ctx.hostname.as_deref().filter(|h| h.len() >= 3) {
        out = replace_word(&out, host, "<host>");
    }
    out = mask_tokens(&out);
    out
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.'
}

/// Replaces whole-word occurrences (bounded by non-word characters).
fn replace_word(input: &str, word: &str, replacement: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut last = 0;
    while let Some(pos) = input[i..].find(word) {
        let start = i + pos;
        let end = start + word.len();
        let before_ok = start == 0 || !is_word_byte(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_word_byte(bytes[end]);
        if before_ok && after_ok {
            out.push_str(&input[last..start]);
            out.push_str(replacement);
            last = end;
        }
        i = end;
    }
    out.push_str(&input[last..]);
    out
}

/// `/home/<name>/...` → `/home/<user>/...`, `/root/...` stays (system path).
fn mask_home_dirs(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find("/home/") {
        out.push_str(&rest[..pos + "/home/".len()]);
        let after = &rest[pos + "/home/".len()..];
        let end = after
            .find(|c: char| c == '/' || c.is_whitespace() || c == '\'' || c == '"')
            .unwrap_or(after.len());
        if end > 0 {
            out.push_str("<user>");
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// Masks e-mail addresses, MAC addresses, IPv4 addresses and secret-looking tokens.
fn mask_tokens(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for token in split_keep_whitespace(input) {
        if token.chars().all(char::is_whitespace) {
            out.push_str(token);
            continue;
        }
        let trimmed = token.trim_matches(|c: char| {
            matches!(
                c,
                ',' | ';' | '(' | ')' | '"' | '\'' | '<' | '>' | '[' | ']'
            )
        });
        let masked = if is_email(trimmed) {
            Some("<email>")
        } else if is_mac(trimmed) {
            Some("<mac>")
        } else if is_ipv4(trimmed) {
            Some("<ip>")
        } else if is_secret(trimmed) {
            Some("<secret>")
        } else {
            None
        };
        match masked {
            Some(m) if !trimmed.is_empty() => out.push_str(&token.replacen(trimmed, m, 1)),
            _ => out.push_str(token),
        }
    }
    out
}

fn split_keep_whitespace(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_ws = None;
    for (i, c) in input.char_indices() {
        let ws = c.is_whitespace();
        match in_ws {
            None => in_ws = Some(ws),
            Some(prev) if prev != ws => {
                parts.push(&input[start..i]);
                start = i;
                in_ws = Some(ws);
            }
            _ => {}
        }
    }
    if start < input.len() {
        parts.push(&input[start..]);
    }
    parts
}

fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._%+-".contains(&b))
        && domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}

fn is_mac(s: &str) -> bool {
    let parts: Vec<&str> = s.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn is_ipv4(s: &str) -> bool {
    let s = s.split_once('/').map(|(ip, _)| ip).unwrap_or(s);
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u16>().map(|v| v <= 255).unwrap_or(false)
        })
        && !s.starts_with("0.")
}

fn is_secret(s: &str) -> bool {
    const PREFIXES: [&str; 7] = [
        "ghp_",
        "gho_",
        "github_pat_",
        "sk-",
        "xoxb-",
        "xoxp-",
        "AKIA",
    ];
    PREFIXES
        .iter()
        .any(|p| s.starts_with(p) && s.len() >= p.len() + 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> SanitizeContext {
        SanitizeContext {
            home: Some("/home/alice".into()),
            user: Some("alice".into()),
            hostname: Some("alice-desktop".into()),
        }
    }

    #[test]
    fn removes_home_user_and_host() {
        let text = "error: /home/alice/.cache/yay/foo.pkg.tar.zst on alice-desktop by alice";
        let s = sanitize(text, &ctx());
        assert_eq!(s, "error: ~/.cache/yay/foo.pkg.tar.zst on <host> by <user>");
    }

    #[test]
    fn masks_other_home_directories() {
        let s = sanitize("cp /home/bob/x /home/carol", &SanitizeContext::default());
        assert_eq!(s, "cp /home/<user>/x /home/<user>");
    }

    #[test]
    fn keeps_package_versions() {
        let s = sanitize(
            "upgraded linux-cachyos (6.17.1-2 -> 6.17.2-1) mesa 1:25.2.4-1 1.2.3.4-1",
            &SanitizeContext::default(),
        );
        assert_eq!(
            s,
            "upgraded linux-cachyos (6.17.1-2 -> 6.17.2-1) mesa 1:25.2.4-1 1.2.3.4-1"
        );
    }

    #[test]
    fn masks_network_identifiers_and_secrets() {
        let s = sanitize(
            "from 192.168.1.20, mac aa:bb:cc:dd:ee:ff mail me@example.org token ghp_abcdefghijklmnopqrstuvwxyz",
            &SanitizeContext::default(),
        );
        assert_eq!(s, "from <ip>, mac <mac> mail <email> token <secret>");
    }

    #[test]
    fn word_boundaries() {
        let c = SanitizeContext {
            home: None,
            user: Some("max".into()),
            hostname: None,
        };
        assert_eq!(sanitize("maxima max", &c), "maxima <user>");
    }
}
