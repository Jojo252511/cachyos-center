//! End-to-end tests of the privileged helper with the REAL pacman binary.
//!
//! Every test builds an isolated pacman sandbox (own RootDir, DBPath, CacheDir,
//! LogFile, GPGDir, HookDir and a `file://` repository with fixture packages
//! built by makepkg). pacman runs under `fakeroot`, the helper runs in its
//! development mode on a private `dbus-daemon`. Nothing outside the temporary
//! directories is touched; the host's package database is never used.
//!
//! Requirements: pacman, fakeroot, makepkg, repo-add, dbus-daemon and the
//! libalpm bridge in the target directory (`cargo build --workspace`). Missing
//! requirements skip the tests unless `CC_REQUIRE_SANDBOX=1` is set (CI).

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use cachyos_center_core::dbus::{BUS_NAME, INTERFACE, OBJECT_PATH, app_error_from_dbus};
use cachyos_center_core::operation::{Operation, OperationState};
use cachyos_center_core::plan::TransactionPlan;
use cachyos_center_core::ui::OperationLogChunk;
use cachyos_center_core::{AppError, ErrorCode};
use cachyos_center_packages::PackageService;

// ---------------------------------------------------------------------------
// Requirements and fixtures
// ---------------------------------------------------------------------------

fn which(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
        .unwrap_or(false)
}

fn target_debug_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cachyos-center-helper"))
        .parent()
        .expect("binary directory")
        .to_path_buf()
}

fn bridge_path() -> PathBuf {
    target_debug_dir().join("libcachyos_center_alpm.so")
}

/// Returns `false` (and prints why) when the sandbox cannot run.
fn requirements_met() -> bool {
    let mut missing: Vec<String> = [
        "pacman",
        "fakeroot",
        "makepkg",
        "repo-add",
        "dbus-daemon",
        "bsdtar",
    ]
    .iter()
    .filter(|p| !which(p))
    .map(|p| (*p).to_string())
    .collect();
    if !bridge_path().exists() {
        missing.push(format!(
            "{} (run cargo build --workspace)",
            bridge_path().display()
        ));
    }
    // SAFETY: plain getter without preconditions.
    #[allow(unsafe_code)]
    let root = unsafe { libc::geteuid() } == 0;
    if root {
        missing.push("non-root user (makepkg and the development helper refuse root)".into());
    }
    if missing.is_empty() {
        return true;
    }
    let message = format!("sandbox tests skipped, missing: {}", missing.join(", "));
    if std::env::var("CC_REQUIRE_SANDBOX").as_deref() == Ok("1") {
        panic!("{message}");
    }
    eprintln!("{message}");
    false
}

struct Fixture {
    name: &'static str,
    version: &'static str,
    depends: &'static [&'static str],
}

const FIXTURES: [Fixture; 7] = [
    Fixture {
        name: "ccfix-a",
        version: "1.0",
        depends: &[],
    },
    Fixture {
        name: "ccfix-a",
        version: "1.1",
        depends: &[],
    },
    Fixture {
        name: "ccfix-a",
        version: "1.2",
        depends: &[],
    },
    Fixture {
        name: "ccfix-b",
        version: "1.0",
        depends: &["ccfix-a"],
    },
    Fixture {
        name: "ccfix-dep",
        version: "1.0",
        depends: &[],
    },
    Fixture {
        name: "ccfix-c",
        version: "1.0",
        depends: &["ccfix-dep"],
    },
    Fixture {
        name: "ccfix-local",
        version: "1.0",
        depends: &[],
    },
];

fn package_file(dir: &Path, name: &str, version: &str) -> PathBuf {
    dir.join(format!("{name}-{version}-1-any.pkg.tar.zst"))
}

/// Builds the fixture packages once per test process.
fn fixtures() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ccfix-packages-v1");
        let pkgdest = root.join("pkg");
        std::fs::create_dir_all(&pkgdest).expect("fixture dir");
        for f in &FIXTURES {
            let file = package_file(&pkgdest, f.name, f.version);
            if file.exists() {
                continue;
            }
            let build = root.join(format!("build-{}-{}", f.name, f.version));
            let _ = std::fs::remove_dir_all(&build);
            std::fs::create_dir_all(&build).expect("build dir");
            let depends = f
                .depends
                .iter()
                .map(|d| format!("'{d}'"))
                .collect::<Vec<_>>()
                .join(" ");
            let pkgbuild = format!(
                "pkgname={name}\npkgver={version}\npkgrel=1\npkgdesc='cachyos-center test fixture {name}'\narch=('any')\nlicense=('GPL-3.0-or-later')\ndepends=({depends})\noptions=('!debug' '!strip' '!zipman' '!lto')\nbackup=('etc/ccfix/{name}.conf')\npackage() {{\n  install -Dm644 /dev/null \"$pkgdir/usr/share/ccfix/{name}-{version}\"\n  printf 'value=1\\n' | install -Dm644 /dev/stdin \"$pkgdir/etc/ccfix/{name}.conf\"\n}}\n",
                name = f.name,
                version = f.version,
            );
            std::fs::write(build.join("PKGBUILD"), pkgbuild).expect("PKGBUILD");
            let out = Command::new("makepkg")
                .args(["--nodeps", "--noconfirm", "--force", "--clean"])
                .current_dir(&build)
                .env("PKGDEST", &pkgdest)
                .env("BUILDDIR", build.join("work"))
                .env("SRCDEST", build.join("src"))
                .env("LOGDEST", &build)
                .env("PKGEXT", ".pkg.tar.zst")
                .env("PACKAGER", "cachyos-center tests <tests@invalid>")
                .env("LC_ALL", "C")
                .output()
                .expect("makepkg");
            assert!(
                out.status.success() && file.exists(),
                "makepkg failed for {}-{}: {}",
                f.name,
                f.version,
                String::from_utf8_lossy(&out.stderr)
            );
        }
        pkgdest
    })
}

// ---------------------------------------------------------------------------
// pacman sandbox
// ---------------------------------------------------------------------------

struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        for sub in ["root", "db", "cache", "gnupg", "hooks", "repo"] {
            std::fs::create_dir_all(dir.path().join(sub)).expect("sandbox dir");
        }
        let sb = Self { dir };
        sb.write_conf("Never");
        sb
    }

    fn path(&self, sub: &str) -> PathBuf {
        self.dir.path().join(sub)
    }

    fn conf(&self) -> PathBuf {
        self.path("pacman.conf")
    }

    fn log(&self) -> PathBuf {
        self.path("pacman.log")
    }

    fn write_conf(&self, sig_level: &str) {
        let p = |s: &str| self.path(s).display().to_string();
        let conf = format!(
            "[options]\nRootDir = {root}\nDBPath = {db}/\nCacheDir = {cache}/\nLogFile = {log}\nGPGDir = {gnupg}/\nHookDir = {hooks}/\nArchitecture = auto\nSigLevel = {sig_level}\nLocalFileSigLevel = Never\nDisableSandboxFilesystem\nDisableSandboxSyscalls\n\n[fixture]\nServer = file://{repo}\n",
            root = p("root"),
            db = p("db"),
            cache = p("cache"),
            log = p("pacman.log"),
            gnupg = p("gnupg"),
            hooks = p("hooks"),
            repo = p("repo"),
        );
        std::fs::write(self.conf(), conf).expect("pacman.conf");
    }

    /// Adds fixture packages to the repository database.
    fn publish(&self, packages: &[(&str, &str)]) {
        let fixtures = fixtures();
        for (name, version) in packages {
            let src = package_file(fixtures, name, version);
            let dst = self.path("repo").join(src.file_name().expect("file name"));
            std::fs::copy(&src, &dst).expect("copy package");
            let out = Command::new("repo-add")
                .arg("--quiet")
                .arg(self.path("repo/fixture.db.tar.gz"))
                .arg(&dst)
                .env("LC_ALL", "C")
                .output()
                .expect("repo-add");
            assert!(
                out.status.success(),
                "repo-add: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        self.bump_db_mtime();
    }

    /// pacman only downloads a sync database that is newer than its local copy
    /// (one-second resolution). Every publication moves the repository
    /// database further into the future so that `-Sy` always sees it.
    fn bump_db_mtime(&self) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static STEP: AtomicU64 = AtomicU64::new(1);
        let step = STEP.fetch_add(1, Ordering::SeqCst);
        let when = std::time::SystemTime::now() + Duration::from_secs(10 * step);
        let db = self.path("repo/fixture.db.tar.gz");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&db)
            .expect("repo db");
        file.set_modified(when).expect("set mtime");
    }

    /// Runs pacman under fakeroot with the sandbox configuration.
    fn pacman(&self, args: &[&str]) -> std::process::Output {
        Command::new("fakeroot")
            .arg("--")
            .arg("pacman")
            .arg("--config")
            .arg(self.conf())
            .args(["--noconfirm", "--noprogressbar", "--color", "never"])
            .args(args)
            .env("LC_ALL", "C")
            .output()
            .expect("pacman")
    }

    fn pacman_ok(&self, args: &[&str]) {
        let out = self.pacman(args);
        assert!(
            out.status.success(),
            "pacman {args:?} failed: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Installed version of `name` or `None`.
    fn installed(&self, name: &str) -> Option<String> {
        let out = Command::new("pacman")
            .arg("--config")
            .arg(self.conf())
            .args(["-Q", "--", name])
            .env("LC_ALL", "C")
            .output()
            .expect("pacman -Q");
        if !out.status.success() {
            return None;
        }
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .nth(1)
            .map(str::to_string)
    }

    fn service(&self) -> PackageService {
        let config =
            cachyos_center_packages::config::load(Some(&self.conf())).expect("pacman-conf");
        PackageService::with_parts(
            config,
            self.path("checkup-db"),
            self.path("check.json"),
            self.log(),
            Default::default(),
        )
    }

    /// Sandbox with ccfix-a 1.0 installed and 1.1 available.
    fn with_upgrade_available() -> Self {
        let sb = Self::new();
        sb.publish(&[
            ("ccfix-a", "1.0"),
            ("ccfix-b", "1.0"),
            ("ccfix-dep", "1.0"),
            ("ccfix-c", "1.0"),
        ]);
        sb.pacman_ok(&["-Sy"]);
        sb.pacman_ok(&["-S", "--", "fixture/ccfix-a"]);
        sb.publish(&[("ccfix-a", "1.1")]);
        sb.pacman_ok(&["-Sy"]);
        assert_eq!(sb.installed("ccfix-a").as_deref(), Some("1.0-1"));
        sb
    }
}

// ---------------------------------------------------------------------------
// Private bus and helper process
// ---------------------------------------------------------------------------

struct Bus {
    child: Child,
    address: String,
    _dir: tempfile::TempDir,
}

impl Bus {
    fn start() -> Self {
        let dir = tempfile::tempdir().expect("bus dir");
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .arg(format!(
                "--address=unix:path={}",
                dir.path().join("bus").display()
            ))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon");
        let stdout = child.stdout.take().expect("stdout");
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("bus address");
        Self {
            child,
            address: line.trim().to_string(),
            _dir: dir,
        }
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Helper {
    child: Child,
    root: PathBuf,
    _owned: Option<tempfile::TempDir>,
}

impl Helper {
    fn start(bus: &Bus, sb: &Sandbox, extra: &[&str]) -> Self {
        let owned = tempfile::tempdir().expect("helper root");
        let mut helper = Self::start_in(bus, sb, owned.path(), extra);
        helper._owned = Some(owned);
        helper
    }

    /// Starts a helper with an existing state directory (restart scenarios).
    fn start_in(bus: &Bus, sb: &Sandbox, root: &Path, extra: &[&str]) -> Self {
        let stderr = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("helper.stderr"))
            .expect("stderr file");
        let child = Command::new(env!("CARGO_BIN_EXE_cachyos-center-helper"))
            .arg("daemon")
            .arg("--dev-root")
            .arg(root)
            .arg("--pacman-conf")
            .arg(sb.conf())
            .arg("--pacman-log")
            .arg(sb.log())
            .args(["--fakeroot", "--lock-wait-secs", "3", "--idle-secs", "600"])
            .args(extra)
            .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
            .env("CACHYOS_CENTER_ALPM_BRIDGE", bridge_path())
            .env("CACHYOS_CENTER_LOG", "debug")
            .stdout(Stdio::null())
            .stderr(stderr)
            .spawn()
            .expect("helper");
        Self {
            child,
            root: root.to_path_buf(),
            _owned: None,
        }
    }

    fn stderr(&self) -> String {
        std::fs::read_to_string(self.root.join("helper.stderr")).unwrap_or_default()
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.kill();
    }
}

struct Client {
    proxy: zbus::Proxy<'static>,
}

impl Client {
    async fn connect(bus: &Bus) -> Self {
        let conn = zbus::connection::Builder::address(bus.address.as_str())
            .expect("address")
            .build()
            .await
            .expect("connect");
        let dbus = zbus::fdo::DBusProxy::new(&conn).await.expect("dbus proxy");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let owned = dbus
                .name_has_owner(BUS_NAME.try_into().expect("bus name"))
                .await
                .unwrap_or(false);
            if owned {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "helper did not acquire {BUS_NAME}"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let proxy = zbus::Proxy::new(&conn, BUS_NAME, OBJECT_PATH, INTERFACE)
            .await
            .expect("proxy");
        Self { proxy }
    }

    fn map_err(e: zbus::Error) -> AppError {
        match e {
            zbus::Error::MethodError(name, msg, _) => {
                app_error_from_dbus(name.as_str(), msg.as_deref())
            }
            other => AppError::internal(other.to_string()),
        }
    }

    async fn upgrade(&self, digest: &str) -> Result<String, AppError> {
        self.proxy
            .call("UpgradeSystem", &(digest, false))
            .await
            .map_err(Self::map_err)
    }

    async fn install(&self, repo: &str, name: &str, digest: &str) -> Result<String, AppError> {
        self.proxy
            .call("InstallRepoPackage", &(repo, name, digest))
            .await
            .map_err(Self::map_err)
    }

    async fn remove(&self, name: &str, recursive: bool, digest: &str) -> Result<String, AppError> {
        self.proxy
            .call("RemoveRepoPackage", &(name, recursive, digest))
            .await
            .map_err(Self::map_err)
    }

    async fn cancel(&self, id: &str) -> Result<Operation, AppError> {
        let json: String = self
            .proxy
            .call("CancelOperation", &(id,))
            .await
            .map_err(Self::map_err)?;
        Ok(serde_json::from_str(&json).expect("operation json"))
    }

    async fn status(&self, id: &str) -> Result<Operation, AppError> {
        let json: String = self
            .proxy
            .call("ReadOperationStatus", &(id,))
            .await
            .map_err(Self::map_err)?;
        Ok(serde_json::from_str(&json).expect("operation json"))
    }

    async fn log(&self, id: &str) -> OperationLogChunk {
        let json: String = self
            .proxy
            .call("ReadOperationLog", &(id, 0u64))
            .await
            .map_err(Self::map_err)
            .expect("log");
        serde_json::from_str(&json).expect("log json")
    }

    async fn current(&self) -> Option<Operation> {
        let json: String = self
            .proxy
            .call("CurrentOperation", &())
            .await
            .map_err(Self::map_err)
            .expect("current");
        (!json.is_empty()).then(|| serde_json::from_str(&json).expect("operation json"))
    }

    async fn wait_terminal(&self, id: &str, helper: &Helper) -> Operation {
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            let op = self.status(id).await.expect("status");
            if op.state.is_terminal() {
                return op;
            }
            assert!(
                Instant::now() < deadline,
                "operation did not finish: {op:?}\nhelper stderr:\n{}",
                helper.stderr()
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    async fn wait_state(&self, id: &str, state: OperationState) -> Operation {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let op = self.status(id).await.expect("status");
            if op.state == state {
                return op;
            }
            assert!(
                Instant::now() < deadline,
                "state {state:?} not reached: {op:?}"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

fn upgrade_plan(sb: &Sandbox) -> TransactionPlan {
    sb.service().system_plan_upgrade().expect("upgrade plan")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_upgrade_with_real_pacman() {
    if !requirements_met() {
        return;
    }
    let sb = Sandbox::with_upgrade_available();
    let plan = upgrade_plan(&sb);
    assert_eq!(plan.entries.len(), 1, "{plan:?}");
    let bus = Bus::start();
    let state = tempfile::tempdir().expect("state");
    let mut helper = Helper::start_in(&bus, &sb, state.path(), &[]);
    let client = Client::connect(&bus).await;

    let id = client.upgrade(&plan.digest).await.expect("start upgrade");
    let op = client.wait_terminal(&id, &helper).await;
    assert_eq!(
        op.state,
        OperationState::Succeeded,
        "{op:?}\n{}",
        helper.stderr()
    );
    assert!(op.commit_started);
    assert_eq!(op.changes.upgraded, 1);
    assert_eq!(op.changes.packages, vec!["ccfix-a".to_string()]);
    assert_eq!(op.progress.packages_total, Some(1));
    assert_eq!(sb.installed("ccfix-a").as_deref(), Some("1.1-1"));

    // The log shows the three pacman phases of the full upgrade.
    let log = client.log(&id).await;
    let text = log.lines.join("\n");
    assert!(text.contains("pacman -Sy "), "{text}");
    assert!(text.contains("pacman -Suw "), "{text}");
    assert!(text.contains("pacman -Su "), "{text}");
    assert!(log.complete);

    // The result survives a restart of the helper: a restarted GUI still sees it.
    drop(client);
    helper.kill();
    let _helper2 = Helper::start_in(&bus, &sb, state.path(), &[]);
    let client = Client::connect(&bus).await;
    let again = client.status(&id).await.expect("status after restart");
    assert_eq!(again.state, OperationState::Succeeded);
    assert_eq!(client.current().await.map(|o| o.id), Some(id));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn changed_plan_stops_before_commit() {
    if !requirements_met() {
        return;
    }
    let sb = Sandbox::with_upgrade_available();
    let confirmed = upgrade_plan(&sb);
    // A mirror publishes a newer version after the user confirmed the preview.
    sb.publish(&[("ccfix-a", "1.2")]);
    let bus = Bus::start();
    let helper = Helper::start(&bus, &sb, &[]);
    let client = Client::connect(&bus).await;

    let id = client.upgrade(&confirmed.digest).await.expect("start");
    let op = client.wait_terminal(&id, &helper).await;
    assert_eq!(op.state, OperationState::CancelledBeforeCommit, "{op:?}");
    assert!(!op.commit_started);
    assert_eq!(
        op.error.as_ref().map(|e| e.code),
        Some(ErrorCode::PlanChanged)
    );
    let actual = op.actual_plan.expect("actual plan");
    assert_eq!(actual.entries[0].new_version.as_deref(), Some("1.2-1"));
    assert_eq!(
        sb.installed("ccfix-a").as_deref(),
        Some("1.0-1"),
        "nothing installed"
    );

    // Re-confirmation with the new plan succeeds.
    let id = client.upgrade(&actual.digest).await.expect("restart");
    let op = client.wait_terminal(&id, &helper).await;
    assert_eq!(op.state, OperationState::Succeeded, "{op:?}");
    assert_eq!(sb.installed("ccfix-a").as_deref(), Some("1.2-1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn foreign_lock_is_respected_and_never_removed() {
    if !requirements_met() {
        return;
    }
    let sb = Sandbox::with_upgrade_available();
    let plan = upgrade_plan(&sb);
    let lock = sb.path("db/db.lck");
    std::fs::write(&lock, "").expect("create lock");
    let bus = Bus::start();
    let helper = Helper::start(&bus, &sb, &[]);
    let client = Client::connect(&bus).await;

    let id = client.upgrade(&plan.digest).await.expect("start");
    client.wait_state(&id, OperationState::Preparing).await;
    // A second operation is refused while the first one is waiting.
    let busy = client
        .upgrade(&plan.digest)
        .await
        .expect_err("second start");
    assert_eq!(busy.code, ErrorCode::Busy);

    let op = client.wait_terminal(&id, &helper).await;
    assert_eq!(op.state, OperationState::Failed, "{op:?}");
    assert_eq!(op.error.as_ref().map(|e| e.code), Some(ErrorCode::Busy));
    assert!(lock.exists(), "the foreign lock must never be removed");
    assert_eq!(sb.installed("ccfix-a").as_deref(), Some("1.0-1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_before_commit() {
    if !requirements_met() {
        return;
    }
    let sb = Sandbox::with_upgrade_available();
    let plan = upgrade_plan(&sb);
    let lock = sb.path("db/db.lck");
    std::fs::write(&lock, "").expect("create lock");
    let bus = Bus::start();
    let helper = Helper::start(&bus, &sb, &[]);
    let client = Client::connect(&bus).await;

    let id = client.upgrade(&plan.digest).await.expect("start");
    client.wait_state(&id, OperationState::Preparing).await;
    client.cancel(&id).await.expect("cancel");
    let op = client.wait_terminal(&id, &helper).await;
    assert_eq!(op.state, OperationState::CancelledBeforeCommit, "{op:?}");
    assert!(!op.commit_started);
    assert!(lock.exists());
    assert_eq!(sb.installed("ccfix-a").as_deref(), Some("1.0-1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signature_failure_changes_nothing() {
    if !requirements_met() {
        return;
    }
    let sb = Sandbox::with_upgrade_available();
    // Packages must now be signed; the fixtures are not.
    sb.write_conf("PackageRequired DatabaseNever");
    let plan = upgrade_plan(&sb);
    let bus = Bus::start();
    let helper = Helper::start(&bus, &sb, &[]);
    let client = Client::connect(&bus).await;

    let id = client.upgrade(&plan.digest).await.expect("start");
    let op = client.wait_terminal(&id, &helper).await;
    assert_eq!(op.state, OperationState::Failed, "{op:?}");
    assert!(
        !op.commit_started,
        "failure happens in the download/verification phase"
    );
    assert_eq!(
        op.error.as_ref().map(|e| e.code),
        Some(ErrorCode::TransactionFailed)
    );
    // pacman demands the (missing) detached signature before anything is committed.
    let detail = op.error.and_then(|e| e.detail).unwrap_or_default();
    assert!(detail.contains(".sig"), "{detail}");
    assert_eq!(sb.installed("ccfix-a").as_deref(), Some("1.0-1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn install_and_remove_repository_packages() {
    if !requirements_met() {
        return;
    }
    let sb = Sandbox::with_upgrade_available();
    let bus = Bus::start();
    let helper = Helper::start(&bus, &sb, &[]);
    let client = Client::connect(&bus).await;

    // Install ccfix-b: a consistent -Syu also upgrades ccfix-a.
    let plan = sb
        .service()
        .system_plan_install("fixture", "ccfix-b")
        .expect("plan");
    assert_eq!(plan.entries.len(), 2, "{plan:?}");
    let id = client
        .install("fixture", "ccfix-b", &plan.digest)
        .await
        .expect("install");
    let op = client.wait_terminal(&id, &helper).await;
    assert_eq!(
        op.state,
        OperationState::Succeeded,
        "{op:?}\n{}",
        helper.stderr()
    );
    assert_eq!(sb.installed("ccfix-b").as_deref(), Some("1.0-1"));
    assert_eq!(sb.installed("ccfix-a").as_deref(), Some("1.1-1"));

    // Removing ccfix-a is impossible while ccfix-b needs it (planning error).
    let err = sb
        .service()
        .system_plan_remove("ccfix-a", false)
        .expect_err("dependency");
    assert_eq!(err.code, ErrorCode::DependencyProblem);
    let detail = err.detail.as_deref().unwrap_or_default();
    assert!(
        detail
            .lines()
            .any(|l| l.starts_with("removing ccfix-a breaks dependency ")
                && l.ends_with(" required by ccfix-b")),
        "{detail}"
    );

    // Conservative removal of ccfix-b keeps ccfix-a.
    let plan = sb
        .service()
        .system_plan_remove("ccfix-b", false)
        .expect("plan");
    let id = client
        .remove("ccfix-b", false, &plan.digest)
        .await
        .expect("remove");
    let op = client.wait_terminal(&id, &helper).await;
    assert_eq!(op.state, OperationState::Succeeded, "{op:?}");
    assert_eq!(op.changes.removed, 1);
    assert_eq!(sb.installed("ccfix-b"), None);
    assert_eq!(sb.installed("ccfix-a").as_deref(), Some("1.1-1"));

    // Recursive removal (-Rs) takes the now unneeded dependency along.
    sb.pacman_ok(&["-S", "--needed", "--", "fixture/ccfix-c"]);
    assert!(sb.installed("ccfix-dep").is_some());
    let plan = sb
        .service()
        .system_plan_remove("ccfix-c", true)
        .expect("plan");
    assert_eq!(plan.entries.len(), 2, "{plan:?}");
    let id = client
        .remove("ccfix-c", true, &plan.digest)
        .await
        .expect("remove -Rs");
    let op = client.wait_terminal(&id, &helper).await;
    assert_eq!(op.state, OperationState::Succeeded, "{op:?}");
    assert_eq!(sb.installed("ccfix-c"), None);
    assert_eq!(sb.installed("ccfix-dep"), None);
    // Modified backup files are kept by pacman (.pacsave), user data is not deleted.
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_packages_are_not_removed() {
    if !requirements_met() {
        return;
    }
    let sb = Sandbox::with_upgrade_available();
    let local = package_file(fixtures(), "ccfix-local", "1.0");
    sb.pacman_ok(&["-U", "--", local.to_str().expect("path")]);
    let plan = sb
        .service()
        .system_plan_remove("ccfix-local", false)
        .expect("plan");
    let bus = Bus::start();
    let helper = Helper::start(&bus, &sb, &[]);
    let client = Client::connect(&bus).await;
    let id = client
        .remove("ccfix-local", false, &plan.digest)
        .await
        .expect("start");
    let op = client.wait_terminal(&id, &helper).await;
    assert_eq!(op.state, OperationState::Failed, "{op:?}");
    assert_eq!(
        op.error.as_ref().map(|e| e.code),
        Some(ErrorCode::InvalidInput)
    );
    assert!(sb.installed("ccfix-local").is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn denied_authorization_changes_nothing() {
    if !requirements_met() {
        return;
    }
    let sb = Sandbox::with_upgrade_available();
    let plan = upgrade_plan(&sb);
    let bus = Bus::start();
    let helper = Helper::start(
        &bus,
        &sb,
        &["--deny", "org.cachyos-center.packages.upgrade"],
    );
    let client = Client::connect(&bus).await;
    let id = client.upgrade(&plan.digest).await.expect("start");
    let op = client.wait_terminal(&id, &helper).await;
    assert_eq!(op.state, OperationState::Failed, "{op:?}");
    assert_eq!(
        op.error.as_ref().map(|e| e.code),
        Some(ErrorCode::NotAuthorized)
    );
    assert_eq!(sb.installed("ccfix-a").as_deref(), Some("1.0-1"));
    assert_eq!(client.current().await.map(|o| o.id), Some(id));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_input_is_rejected_synchronously() {
    if !requirements_met() {
        return;
    }
    let sb = Sandbox::with_upgrade_available();
    let bus = Bus::start();
    let _helper = Helper::start(&bus, &sb, &[]);
    let client = Client::connect(&bus).await;
    let digest = "0".repeat(64);
    for (res, what) in [
        (client.upgrade("not-a-digest").await, "digest"),
        (
            client.install("fixture", "--overwrite=*", &digest).await,
            "name",
        ),
        (
            client.install("../etc", "ccfix-a", &digest).await,
            "repository",
        ),
        (client.remove("-Rdd", false, &digest).await, "remove name"),
    ] {
        let err = res.expect_err(what);
        assert_eq!(err.code, ErrorCode::InvalidInput, "{what}: {err:?}");
    }
    let unknown = client
        .status("00000000-0000-4000-8000-000000000000")
        .await
        .expect_err("unknown id");
    assert_eq!(unknown.code, ErrorCode::NotFound);
    let bad = client.status("../../etc/passwd").await.expect_err("bad id");
    assert_eq!(bad.code, ErrorCode::InvalidInput);
}
