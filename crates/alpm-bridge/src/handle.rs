//! Creation and caching of libalpm handles.
//!
//! Opening a handle and loading the sync databases takes a few hundred
//! milliseconds, so one handle is cached per configuration. The cache key
//! contains the modification times of the local database directory and of all
//! sync database files: any change made by pacman (inside or outside of
//! cachyos-center) invalidates the handle. Handles are additionally dropped
//! after [`MAX_AGE`].

use std::sync::Mutex;
use std::time::{Duration, Instant, UNIX_EPOCH};

use alpm::{Alpm, Question, Usage};
use cachyos_center_core::bridge::AlpmConfig;
use cachyos_center_core::{AppError, AppResult, ErrorCode};

use crate::siglevel;

const MAX_AGE: Duration = Duration::from_secs(30);

struct Cached {
    key: String,
    alpm: Alpm,
    created: Instant,
}

struct CacheCell(Option<Cached>);

// SAFETY: libalpm handles have no thread affinity. Every access to the cached
// handle happens while holding the `CACHE` mutex, so the handle is never used
// by two threads at the same time.
unsafe impl Send for CacheCell {}

static CACHE: Mutex<CacheCell> = Mutex::new(CacheCell(None));

/// Drops the cached handle (after errors or panics).
pub fn reset_cache() {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    guard.0 = None;
}

/// Runs `f` with a handle for `config`. The handle is discarded when `f` fails
/// so that a half-initialized transaction can never leak into the next call.
pub fn with_handle<T>(
    config: &AlpmConfig,
    f: impl FnOnce(&mut Alpm) -> AppResult<T>,
) -> AppResult<T> {
    let key = cache_key(config)?;
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let reusable = matches!(&guard.0, Some(c) if c.key == key && c.created.elapsed() < MAX_AGE);
    if !reusable {
        guard.0 = None;
        guard.0 = Some(Cached {
            alpm: open(config)?,
            key,
            created: Instant::now(),
        });
    }
    let Some(cached) = guard.0.as_mut() else {
        return Err(AppError::internal("handle cache unexpectedly empty"));
    };
    let result = f(&mut cached.alpm);
    if result.is_err() {
        guard.0 = None;
    }
    result
}

fn mtime_key(path: &std::path::Path) -> String {
    match std::fs::metadata(path) {
        Ok(meta) => {
            let modified = meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            format!("{modified}:{}", meta.len())
        }
        Err(_) => "missing".to_string(),
    }
}

fn cache_key(config: &AlpmConfig) -> AppResult<String> {
    let mut key = serde_json::to_string(config)
        .map_err(|e| AppError::internal(format!("cannot encode config: {e}")))?;
    let db = std::path::Path::new(&config.db_path);
    key.push('|');
    key.push_str(&mtime_key(&db.join("local")));
    for repo in &config.repos {
        key.push('|');
        key.push_str(&mtime_key(
            &db.join("sync").join(format!("{}.db", repo.name)),
        ));
    }
    Ok(key)
}

fn alpm_err(context: &str, err: alpm::Error) -> AppError {
    let code = match err {
        alpm::Error::HandleLock => ErrorCode::Busy,
        alpm::Error::DbNotFound | alpm::Error::DbOpen | alpm::Error::DbNull => {
            ErrorCode::Unavailable
        }
        _ => ErrorCode::Internal,
    };
    AppError::new(code, format!("{context}: {err}"))
}

fn usage_of(values: &[String]) -> Usage {
    if values.is_empty() {
        return Usage::ALL;
    }
    let mut usage = Usage::empty();
    for v in values {
        match v.as_str() {
            "Sync" => usage |= Usage::SYNC,
            "Search" => usage |= Usage::SEARCH,
            "Install" => usage |= Usage::INSTALL,
            "Upgrade" => usage |= Usage::UPGRADE,
            "All" => usage |= Usage::ALL,
            _ => {}
        }
    }
    if usage.is_empty() { Usage::ALL } else { usage }
}

/// Opens a new handle with the given configuration and registers all sync
/// databases in pacman order.
pub fn open(config: &AlpmConfig) -> AppResult<Alpm> {
    let mut alpm = Alpm::new(config.root_dir.as_str(), config.db_path.as_str())
        .map_err(|e| alpm_err("cannot initialize libalpm", e))?;
    if !config.gpg_dir.is_empty() {
        alpm.set_gpgdir(config.gpg_dir.as_str())
            .map_err(|e| alpm_err("cannot set GPGDir", e))?;
    }
    for dir in &config.cache_dirs {
        alpm.add_cachedir(dir.as_str())
            .map_err(|e| alpm_err("cannot set CacheDir", e))?;
    }
    for arch in &config.architectures {
        alpm.add_architecture(arch.as_str())
            .map_err(|e| alpm_err("cannot set Architecture", e))?;
    }
    for pkg in &config.ignore_pkgs {
        alpm.add_ignorepkg(pkg.as_str())
            .map_err(|e| alpm_err("cannot set IgnorePkg", e))?;
    }
    for group in &config.ignore_groups {
        alpm.add_ignoregroup(group.as_str())
            .map_err(|e| alpm_err("cannot set IgnoreGroup", e))?;
    }
    let (global, _) = siglevel::process(&config.sig_level, siglevel::pacman_default())?;
    alpm.set_default_siglevel(global)
        .map_err(|e| alpm_err("cannot set SigLevel", e))?;
    for repo in &config.repos {
        cachyos_center_core::validate::repo_name(&repo.name)?;
        let level = siglevel::repo_level(global, &repo.sig_level)?;
        let db = alpm
            .register_syncdb(repo.name.as_str(), level)
            .map_err(|e| alpm_err(&format!("cannot register repository {}", repo.name), e))?;
        db.set_usage(usage_of(&repo.usage))
            .map_err(|e| alpm_err("cannot set Usage", e))?;
    }
    // Answer the questions that shape a plan (ignored packages, replacements,
    // conflicts, removals, providers) like `pacman --noconfirm`, so that the
    // plan matches the helper's non-interactive pacman run. Planning never
    // downloads, so corrupted packages and key imports cannot come up here;
    // they are declined because the bridge must never delete files or import
    // keys (pacman itself decides about them during the real transaction).
    alpm.set_question_cb((), |mut question, _| match question.question() {
        Question::InstallIgnorepkg(mut q) => q.set_install(true),
        Question::Replace(q) => q.set_replace(true),
        Question::Conflict(mut q) => q.set_remove(false),
        Question::Corrupted(mut q) => q.set_remove(false),
        Question::RemovePkgs(mut q) => q.set_skip(false),
        Question::SelectProvider(mut q) => q.set_index(0),
        Question::ImportKey(mut q) => {
            q.set_import(false);
            question.set_answer(false);
        }
    });
    Ok(alpm)
}
