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
  oder deaktiviert (z. B. die erste Updateprüfung, „Erneut prüfen“, „Übernehmen“, der Start eines
  Upgrades im Dialog); Prüfskript `tests/ui/fokus_webkit.py` mit echten Tastendrücken in WebKitGTK.

### Behoben
- Aktivität und Diagnosebericht nennen bei Updateprüfungen die Zahl der gefundenen Updates; der
  Bericht zeigte dort „(+0 ~0 -0)“. Der Diagnosebericht nennt außerdem die Quelle jedes Eintrags
  (App, Timer, extern) und die Anzahl bei Gesundheitshinweisen.
- Der Cache-Hinweis richtet sich nach dem, was `paccache -r` tatsächlich freigeben würde (alle
  außer den drei neuesten Versionen je Paket, Versionsvergleich wie `vercmp`), nicht mehr nach
  der Gesamtgröße. Vorher blieb er auch nach dem Aufräumen stehen.

### Sicherheit
- Jede schreibende Helper-Methode prüft den Bus-Absender per Polkit, auch das Abbrechen (eigene
  Aktion `org.cachyos-center.packages.cancel`); der Polkit-Codepfad ist gegen eine
  Test-Authority getestet.
