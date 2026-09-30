//! Input validation shared by UI backend, helper and MCP server.
//!
//! Every value that reaches the privileged helper is validated here *and*
//! re-checked against the repository data. Validation is intentionally strict:
//! the helper never forwards free text to a command line.

use crate::error::{AppError, AppResult};

/// Maximum accepted length of a package name.
pub const MAX_PACKAGE_NAME_LEN: usize = 128;
/// Maximum accepted length of a repository name.
pub const MAX_REPO_NAME_LEN: usize = 64;
/// Maximum accepted length of a version string (`epoch:pkgver-pkgrel`).
pub const MAX_VERSION_LEN: usize = 128;
/// Maximum accepted length of a free text search query.
pub const MAX_QUERY_LEN: usize = 100;

/// Validates a package name according to the makepkg rules: alphanumeric
/// characters and `@ . _ + -`, not starting with a hyphen or a dot.
pub fn package_name(name: &str) -> AppResult<()> {
    if name.is_empty() || name.len() > MAX_PACKAGE_NAME_LEN {
        return Err(AppError::invalid("package name has an invalid length"));
    }
    if name.starts_with('-') || name.starts_with('.') {
        return Err(AppError::invalid(
            "package name must not start with '-' or '.'",
        ));
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'@' | b'.' | b'_' | b'+' | b'-'))
    {
        return Err(AppError::invalid(
            "package name contains invalid characters",
        ));
    }
    Ok(())
}

/// Validates a repository name as used in `pacman.conf` sections.
pub fn repo_name(name: &str) -> AppResult<()> {
    if name.is_empty() || name.len() > MAX_REPO_NAME_LEN {
        return Err(AppError::invalid("repository name has an invalid length"));
    }
    if name == "local" || name == "options" {
        return Err(AppError::invalid("reserved repository name"));
    }
    if name.starts_with('-') || name.starts_with('.') {
        return Err(AppError::invalid(
            "repository name must not start with '-' or '.'",
        ));
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'))
    {
        return Err(AppError::invalid(
            "repository name contains invalid characters",
        ));
    }
    Ok(())
}

/// Validates a package version (`[epoch:]pkgver-pkgrel`).
pub fn version(v: &str) -> AppResult<()> {
    if v.is_empty() || v.len() > MAX_VERSION_LEN {
        return Err(AppError::invalid("version has an invalid length"));
    }
    if !v
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b':' | b'~' | b'-'))
    {
        return Err(AppError::invalid("version contains invalid characters"));
    }
    Ok(())
}

/// Validates an operation id (lower-case UUID in hyphenated form).
pub fn operation_id(id: &str) -> AppResult<()> {
    let bytes = id.as_bytes();
    if bytes.len() != 36 {
        return Err(AppError::invalid("operation id has an invalid length"));
    }
    for (i, b) in bytes.iter().enumerate() {
        let ok = match i {
            8 | 13 | 18 | 23 => *b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(b),
        };
        if !ok {
            return Err(AppError::invalid("operation id is not a lower-case UUID"));
        }
    }
    Ok(())
}

/// Validates a plan digest (lower-case hex SHA-256).
pub fn plan_digest(digest: &str) -> AppResult<()> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(AppError::invalid(
            "plan digest must be a lower-case hex SHA-256",
        ));
    }
    Ok(())
}

/// Normalizes a free text search query. Control characters are rejected,
/// surrounding whitespace is trimmed. The query is used for plain substring
/// matching only, never as a regular expression or command argument.
pub fn search_query(query: &str) -> AppResult<String> {
    let trimmed = query.trim();
    if trimmed.chars().count() > MAX_QUERY_LEN {
        return Err(AppError::invalid("search query is too long"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(AppError::invalid(
            "search query contains control characters",
        ));
    }
    Ok(trimmed.to_string())
}

/// Clamps a limit into `min..=max`, using `default` when absent.
pub fn limit(value: Option<u32>, default: u32, min: u32, max: u32) -> AppResult<u32> {
    match value {
        None => Ok(default),
        Some(v) if (min..=max).contains(&v) => Ok(v),
        Some(_) => Err(AppError::invalid(format!(
            "limit must be between {min} and {max}"
        ))),
    }
}

/// Validates an update window time in `HH:MM` (24h) format and returns minutes after midnight.
pub fn time_of_day(value: &str) -> AppResult<u16> {
    let err = || AppError::invalid("time must use the HH:MM format");
    let (h, m) = value.split_once(':').ok_or_else(err)?;
    if h.len() != 2 || m.len() != 2 {
        return Err(err());
    }
    let h: u16 = h.parse().map_err(|_| err())?;
    let m: u16 = m.parse().map_err(|_| err())?;
    if h > 23 || m > 59 {
        return Err(err());
    }
    Ok(h * 60 + m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names() {
        for ok in [
            "linux",
            "linux-cachyos",
            "lib32-mesa",
            "gtk+",
            "python3.12",
            "foo_bar",
            "c++utilities",
            "@scope",
            "a",
        ] {
            assert!(package_name(ok).is_ok(), "{ok} should be valid");
        }
        for bad in [
            "",
            "-rf",
            ".hidden",
            "foo bar",
            "foo;rm",
            "foo/bar",
            "foo$",
            "ü",
            "a\nb",
            "--noconfirm",
            &"x".repeat(MAX_PACKAGE_NAME_LEN + 1),
        ] {
            assert!(package_name(bad).is_err(), "{bad:?} should be invalid");
        }
    }

    #[test]
    fn repo_names() {
        for ok in [
            "core",
            "extra",
            "multilib",
            "cachyos-v3",
            "cachyos-extra-v3",
        ] {
            assert!(repo_name(ok).is_ok(), "{ok}");
        }
        for bad in ["", "local", "options", "-x", "foo/bar", "a b", "../etc"] {
            assert!(repo_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn versions() {
        for ok in [
            "1.0-1",
            "1:2.3.4-5",
            "6.17.1.arch1-1",
            "2.0~rc1-1",
            "r123.abc-2",
        ] {
            assert!(version(ok).is_ok(), "{ok}");
        }
        for bad in ["", "1.0 -1", "1.0;x", "1/2"] {
            assert!(version(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn operation_ids() {
        assert!(operation_id("123e4567-e89b-42d3-a456-426614174000").is_ok());
        assert!(operation_id("123E4567-E89B-42D3-A456-426614174000").is_err());
        assert!(operation_id("123e4567e89b42d3a456426614174000").is_err());
        assert!(operation_id("../../etc/passwd").is_err());
    }

    #[test]
    fn digests() {
        assert!(plan_digest(&"a".repeat(64)).is_ok());
        assert!(plan_digest(&"A".repeat(64)).is_err());
        assert!(plan_digest(&"a".repeat(63)).is_err());
        assert!(plan_digest("zz").is_err());
    }

    #[test]
    fn queries() {
        assert_eq!(search_query("  firefox ").unwrap(), "firefox");
        assert!(search_query("a\u{7}b").is_err());
        assert!(search_query(&"x".repeat(MAX_QUERY_LEN + 1)).is_err());
        assert_eq!(search_query("[.*").unwrap(), "[.*");
    }

    #[test]
    fn limits() {
        assert_eq!(limit(None, 20, 1, 50).unwrap(), 20);
        assert_eq!(limit(Some(50), 20, 1, 50).unwrap(), 50);
        assert!(limit(Some(0), 20, 1, 50).is_err());
        assert!(limit(Some(51), 20, 1, 50).is_err());
    }

    #[test]
    fn times() {
        assert_eq!(time_of_day("00:00").unwrap(), 0);
        assert_eq!(time_of_day("23:59").unwrap(), 23 * 60 + 59);
        assert_eq!(time_of_day("03:30").unwrap(), 210);
        for bad in ["24:00", "3:30", "12:60", "12-30", "", "ab:cd"] {
            assert!(time_of_day(bad).is_err(), "{bad}");
        }
    }
}
