//! Developer tool: prints what the application core sees on this machine.
//!
//! `cargo run -p cachyos-center-service --example dump -- [dashboard|system|health|updates|activity|autoupdate|report|hyprland]`

use cachyos_center_service::AppCore;

fn main() {
    let core = match AppCore::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let what = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "dashboard".into());
    let json = match what.as_str() {
        "dashboard" => serde_json::to_string_pretty(&core.dashboard()),
        "system" => serde_json::to_string_pretty(&core.system_info()),
        "health" => serde_json::to_string_pretty(&core.health_report()),
        "updates" => serde_json::to_string_pretty(&core.last_check()),
        "activity" => serde_json::to_string_pretty(&core.activity(20)),
        "autoupdate" => serde_json::to_string_pretty(&core.auto_update_status()),
        "hyprland" => serde_json::to_string_pretty(&core.hyprland()),
        "report" => {
            eprintln!("{}", core.diagnostic_report());
            return;
        }
        other => {
            eprintln!("unknown view '{other}'");
            std::process::exit(2);
        }
    };
    match json {
        Ok(text) => eprintln!("{text}"),
        Err(e) => eprintln!("{e}"),
    }
}
