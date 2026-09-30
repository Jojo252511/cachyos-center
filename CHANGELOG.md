# Changelog

Alle nennenswerten Änderungen dieses Projekts. Format angelehnt an
[Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung nach
[Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Hinzugefügt
- Rust-Workspace mit Kernmodell, libalpm-Bridge (per `dlopen`), Paketdienst, Systeminformationen,
  Application Core, privilegiertem D-Bus-Helper, MCP-Server und Tauri-2-Oberfläche.
- Isolierte Updateprüfung mit `checkupdates`, vollständige Upgrades in drei pacman-Phasen mit
  Plan-Prüfung vor dem Commit, Installation und konservatives Entfernen von Repository-Paketen.
- Gesundheitszentrum (`.pacnew`/`.pacsave`, Neustartempfehlung, Lock, Cache, Snapshots,
  andere Updater), Arch-/CachyOS-News, Diagnosebericht mit Bereinigung.
- Automatische Updates „Nur benachrichtigen“; „Automatisch beim nächsten Neustart installieren“
  über CachyOS `pacman-offline` (in Entwicklung, gesperrt).
- Lokaler, rein lesender MCP-Server (stdio, standardmäßig deaktiviert).
- Arch-Paketierung (PKGBUILD, Polkit-Policy, D-Bus-Aktivierung, systemd-Units), CI.
