//! Port of pacman's `SigLevel` parsing (`src/pacman/conf.c: process_siglevel`).

use alpm::SigLevel;
use cachyos_center_core::{AppError, AppResult};

/// pacman's built-in level before any `SigLevel` line is applied:
/// optional signatures for packages and databases.
pub fn pacman_default() -> SigLevel {
    SigLevel::PACKAGE
        | SigLevel::PACKAGE_OPTIONAL
        | SigLevel::DATABASE
        | SigLevel::DATABASE_OPTIONAL
}

struct Levels {
    level: SigLevel,
    mask: SigLevel,
}

impl Levels {
    fn set(&mut self, bits: SigLevel) {
        self.level |= bits;
        self.mask |= bits;
    }

    fn unset(&mut self, bits: SigLevel) {
        self.level.remove(bits);
        self.mask |= bits;
    }
}

/// Applies `values` on top of `base`. Returns the resulting level and the mask
/// of bits that were explicitly set or unset.
pub fn process(values: &[String], base: SigLevel) -> AppResult<(SigLevel, SigLevel)> {
    let mut l = Levels {
        level: base,
        mask: SigLevel::empty(),
    };
    for original in values {
        let (value, package, database) = if let Some(v) = original.strip_prefix("Package") {
            (v, true, false)
        } else if let Some(v) = original.strip_prefix("Database") {
            (v, false, true)
        } else {
            (original.as_str(), true, true)
        };
        match value {
            "Never" => {
                if package {
                    l.unset(SigLevel::PACKAGE);
                }
                if database {
                    l.unset(SigLevel::DATABASE);
                }
            }
            "Optional" => {
                if package {
                    l.set(SigLevel::PACKAGE | SigLevel::PACKAGE_OPTIONAL);
                }
                if database {
                    l.set(SigLevel::DATABASE | SigLevel::DATABASE_OPTIONAL);
                }
            }
            "Required" => {
                if package {
                    l.set(SigLevel::PACKAGE);
                    l.unset(SigLevel::PACKAGE_OPTIONAL);
                }
                if database {
                    l.set(SigLevel::DATABASE);
                    l.unset(SigLevel::DATABASE_OPTIONAL);
                }
            }
            "TrustedOnly" => {
                if package {
                    l.unset(SigLevel::PACKAGE_MARGINAL_OK | SigLevel::PACKAGE_UNKNOWN_OK);
                }
                if database {
                    l.unset(SigLevel::DATABASE_MARGINAL_OK | SigLevel::DATABASE_UNKNOWN_OK);
                }
            }
            "TrustAll" => {
                if package {
                    l.set(SigLevel::PACKAGE_MARGINAL_OK | SigLevel::PACKAGE_UNKNOWN_OK);
                }
                if database {
                    l.set(SigLevel::DATABASE_MARGINAL_OK | SigLevel::DATABASE_UNKNOWN_OK);
                }
            }
            other => {
                return Err(AppError::invalid(format!(
                    "unknown SigLevel value '{other}'"
                )));
            }
        }
        l.level.remove(SigLevel::USE_DEFAULT);
    }
    Ok((l.level, l.mask))
}

/// Level of a repository: the repository values merged over the global level
/// (pacman's `merge_siglevel`).
pub fn repo_level(global: SigLevel, repo_values: &[String]) -> AppResult<SigLevel> {
    if repo_values.is_empty() {
        return Ok(SigLevel::USE_DEFAULT);
    }
    let (over, mask) = process(repo_values, SigLevel::USE_DEFAULT)?;
    Ok((global & !mask) | (over & mask))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn arch_default_config() {
        // SigLevel = Required DatabaseOptional  (Arch default pacman.conf)
        let (level, _) = process(&v(&["Required", "DatabaseOptional"]), pacman_default()).unwrap();
        assert!(level.contains(SigLevel::PACKAGE));
        assert!(!level.contains(SigLevel::PACKAGE_OPTIONAL));
        assert!(level.contains(SigLevel::DATABASE | SigLevel::DATABASE_OPTIONAL));
    }

    #[test]
    fn cachyos_config() {
        // pacman-conf output of a CachyOS system.
        let (level, _) = process(
            &v(&[
                "PackageRequired",
                "PackageTrustedOnly",
                "DatabaseOptional",
                "DatabaseTrustedOnly",
            ]),
            pacman_default(),
        )
        .unwrap();
        assert!(level.contains(SigLevel::PACKAGE));
        assert!(!level.contains(SigLevel::PACKAGE_OPTIONAL));
        assert!(!level.contains(SigLevel::PACKAGE_MARGINAL_OK));
        assert!(level.contains(SigLevel::DATABASE_OPTIONAL));
        assert!(!level.contains(SigLevel::USE_DEFAULT));
    }

    #[test]
    fn never_and_repo_merge() {
        let global = process(&v(&["Required"]), pacman_default()).unwrap().0;
        let repo = repo_level(global, &v(&["DatabaseNever"])).unwrap();
        assert!(repo.contains(SigLevel::PACKAGE));
        assert!(!repo.contains(SigLevel::DATABASE));
        assert_eq!(repo_level(global, &[]).unwrap(), SigLevel::USE_DEFAULT);
    }

    #[test]
    fn unknown_value_is_rejected() {
        assert!(process(&v(&["Sometimes"]), pacman_default()).is_err());
    }
}
