//! `cachyos-center-helper` – privileged helper and timer entry point.
//!
//! Production use (started by systemd, never by hand):
//! * `cachyos-center-helper daemon`    – D-Bus service `org.cachyos_center.Packages1`
//! * `cachyos-center-helper preflight` – scheduled update check/preparation
//!
//! Development (unprivileged only): `cachyos-center-helper daemon --dev-root DIR`
//! serves on the session bus with sandbox directories.

use std::path::PathBuf;
use std::process::ExitCode;

use cachyos_center_helper::config::{AuthMode, HelperConfig, is_root};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "cachyos-center-helper",
    version,
    about = "Privileged helper of cachyos-center"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Serve the D-Bus interface (default).
    Daemon {
        /// Development mode on the session bus below this directory (not as root).
        #[arg(long)]
        dev_root: Option<PathBuf>,
        /// Development mode: pacman configuration of a sandbox.
        #[arg(long, requires = "dev_root")]
        pacman_conf: Option<PathBuf>,
        /// Development mode: pacman binary (e.g. a test double).
        #[arg(long, requires = "dev_root")]
        pacman: Option<PathBuf>,
        /// Development mode: run pacman through fakeroot (sandbox root).
        #[arg(long, requires = "dev_root")]
        fakeroot: bool,
        /// Development mode: pacman log of the sandbox.
        #[arg(long, requires = "dev_root")]
        pacman_log: Option<PathBuf>,
        /// Development mode: deny these polkit actions (all others are allowed).
        #[arg(long, requires = "dev_root", conflicts_with = "polkit")]
        deny: Vec<String>,
        /// Development mode: ask the polkit authority on the development bus
        /// (tests with a fake authority) instead of the built-in test decision.
        #[arg(long, requires = "dev_root")]
        polkit: bool,
        /// Development mode: maximum time for a polkit check in seconds.
        #[arg(long, requires = "dev_root")]
        auth_timeout_secs: Option<u64>,
        /// Development mode: maximum wait for a foreign pacman lock in seconds.
        #[arg(long, requires = "dev_root")]
        lock_wait_secs: Option<u64>,
        /// Development mode: exit after this many idle seconds.
        #[arg(long, requires = "dev_root")]
        idle_secs: Option<u64>,
    },
    /// Run the scheduled preflight once (started by cachyos-center-preflight.service).
    Preflight {
        /// Development mode: state, policy and logs below this directory (not as root).
        #[arg(long)]
        dev_root: Option<PathBuf>,
    },
}

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("CACHYOS_CENTER_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .without_time()
        .init();
}

fn main() -> ExitCode {
    init_logging();
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("cannot start runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match cli.command.unwrap_or(Cmd::Daemon {
        dev_root: None,
        pacman_conf: None,
        pacman: None,
        fakeroot: false,
        pacman_log: None,
        deny: Vec::new(),
        polkit: false,
        auth_timeout_secs: None,
        lock_wait_secs: None,
        idle_secs: None,
    }) {
        Cmd::Daemon {
            dev_root,
            pacman_conf,
            pacman,
            fakeroot,
            pacman_log,
            deny,
            polkit,
            auth_timeout_secs,
            lock_wait_secs,
            idle_secs,
        } => {
            let config = match dev_root {
                None => {
                    if !is_root() {
                        eprintln!("the helper must run as root (use --dev-root for development)");
                        return ExitCode::FAILURE;
                    }
                    HelperConfig::production()
                }
                Some(root) => {
                    if is_root() {
                        eprintln!("development mode is refused when running as root");
                        return ExitCode::FAILURE;
                    }
                    let mut c = HelperConfig::development(root);
                    c.pacman_conf = pacman_conf;
                    if let Some(p) = pacman {
                        c.pacman = p;
                    }
                    if let Some(l) = pacman_log {
                        c.pacman_log = l;
                    }
                    c.fakeroot = fakeroot;
                    c.auth = if polkit {
                        AuthMode::Polkit
                    } else {
                        AuthMode::TestDeny(deny)
                    };
                    if let Some(secs) = auth_timeout_secs {
                        c.auth_timeout = std::time::Duration::from_secs(secs);
                    }
                    if let Some(secs) = lock_wait_secs {
                        c.lock_wait = std::time::Duration::from_secs(secs);
                    }
                    if let Some(secs) = idle_secs {
                        c.idle_timeout = std::time::Duration::from_secs(secs);
                    }
                    c
                }
            };
            match runtime.block_on(cachyos_center_helper::serve(config)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    tracing::error!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Cmd::Preflight { dev_root } => {
            let config = match dev_root {
                None if is_root() => HelperConfig::production(),
                None => {
                    eprintln!("the preflight must run as root (use --dev-root for development)");
                    return ExitCode::FAILURE;
                }
                Some(_) if is_root() => {
                    eprintln!("development mode is refused when running as root");
                    return ExitCode::FAILURE;
                }
                Some(root) => HelperConfig::development(root),
            };
            let code = runtime.block_on(cachyos_center_helper::preflight::run(&config));
            if code == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
