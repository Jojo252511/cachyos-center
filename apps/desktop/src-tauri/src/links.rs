//! Validation of external links and revealable files.

use std::path::Path;

use cachyos_center_core::{AppError, AppResult};

/// Hosts that may be opened in the browser (official documentation and news).
const ALLOWED_HOSTS: [&str; 6] = [
    "archlinux.org",
    "wiki.archlinux.org",
    "man.archlinux.org",
    "cachyos.org",
    "wiki.cachyos.org",
    "github.com",
];

/// Only `https://` URLs on the allow-listed hosts; GitHub only for the project repository.
pub fn validate_url(url: &str) -> AppResult<()> {
    if url.len() > 2048 || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(AppError::invalid("invalid URL"));
    }
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| AppError::invalid("only https links can be opened"))?;
    let (authority, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
    if authority.contains('@') || authority.contains(':') {
        return Err(AppError::invalid(
            "URL with credentials or port is not allowed",
        ));
    }
    let host = authority.to_ascii_lowercase();
    if !ALLOWED_HOSTS.contains(&host.as_str()) {
        return Err(AppError::invalid(format!("host {host} is not allowed")));
    }
    if host == "github.com" {
        let repo = path.to_ascii_lowercase();
        if !(repo == "jojo252511/cachyos-center" || repo.starts_with("jojo252511/cachyos-center/"))
        {
            return Err(AppError::invalid(
                "only the project repository can be opened on GitHub",
            ));
        }
    }
    Ok(())
}

/// Only `.pacnew`/`.pacsave` files below `/etc` from the current health report.
pub fn validate_config_file(path: &str, known: &[String]) -> AppResult<()> {
    let p = Path::new(path);
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let kind_ok =
        name.ends_with(".pacnew") || name.ends_with(".pacsave") || name.contains(".pacsave.");
    if !kind_ok || !cachyos_center_core::paths::is_below(p, Path::new("/etc")) {
        return Err(AppError::invalid(
            "only .pacnew/.pacsave files below /etc can be shown",
        ));
    }
    if !known.iter().any(|k| k == path) {
        return Err(AppError::not_found(
            "file is not part of the current health report",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        for ok in [
            "https://archlinux.org/news/some-news/",
            "https://wiki.cachyos.org/configuration/post_install_setup/",
            "https://github.com/Jojo252511/cachyos-center",
            "https://github.com/Jojo252511/cachyos-center/issues",
            "https://cachyos.org/blog/2608-august-release/",
        ] {
            validate_url(ok).unwrap();
        }
        for bad in [
            "http://archlinux.org/",
            "https://evil.example/",
            "https://archlinux.org.evil.example/",
            "https://user@archlinux.org/",
            "https://archlinux.org:8443/",
            "https://github.com/someone/else",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://archlinux.org/ news",
        ] {
            assert!(validate_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn config_files() {
        let known = vec!["/etc/pacman.conf.pacnew".to_string()];
        validate_config_file("/etc/pacman.conf.pacnew", &known).unwrap();
        assert!(validate_config_file("/etc/passwd", &known).is_err());
        assert!(validate_config_file("/etc/../root/x.pacnew", &known).is_err());
        assert!(validate_config_file("/etc/other.pacnew", &known).is_err());
        assert!(validate_config_file("/home/u/x.pacnew", &known).is_err());
    }
}
