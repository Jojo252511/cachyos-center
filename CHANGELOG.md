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
- Arch-Paketierung (PKGBUILD, Polkit-Policy, D-Bus-Aktivierung, systemd-Units), CI mit
  Secret-Scan der gesamten Git-Historie (gitleaks).
- Statusseite mit Meilensteinen, Abweichungen und nächsten Schritten (`docs/status.md`).
- Tastaturfokus bleibt erhalten, wenn ein Zustandswechsel das fokussierte Bedienelement entfernt
  oder deaktiviert (z. B. die erste Updateprüfung, „Erneut prüfen“, „Übernehmen“); Prüfskript
  `tests/ui/fokus_webkit.py` mit echten Tastendrücken in WebKitGTK.

### Sicherheit
- Jede schreibende Helper-Methode prüft den Bus-Absender per Polkit, auch das Abbrechen (eigene
  Aktion `org.cachyos-center.packages.cancel`); der Polkit-Codepfad ist gegen eine
  Test-Authority getestet.
