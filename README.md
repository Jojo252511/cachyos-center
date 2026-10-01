# cachyos-center

Deutschsprachige Desktop-App für CachyOS mit Hyprland: den Zustand des Systems verstehen, Updates
sicher durchführen und Software finden, ohne Terminalwissen vorauszusetzen. Dazu ein lokaler, rein
lesender MCP-Server für KI-Assistenten.

> **Unabhängiges Community-Projekt, kein offizielles CachyOS-Produkt.**
> Status: **Vorabversion 0.1.0**. Paketoperationen wurden mit dem echten pacman in einer isolierten
> Sandbox getestet, aber noch nicht auf einer VM mit Polkit-Agent (siehe
> [Testprotokoll](docs/testprotokoll.md)).

## Funktionen

| Bereich | Umfang |
|---|---|
| Übersicht | Updatestatus (nie „0 Updates“ ohne frische Prüfung), System, Software, Gesundheit, letzte Aktivität, nächste Prüfung |
| Updates | isolierte Prüfung mit `checkupdates` (kein `pacman -Sy` auf der echten Datenbank), Liste mit Versionen, Repository, Downloadgröße und Kennzeichen (Kernel, Treiber, Neustart), Vorabprüfungen inkl. Arch-/CachyOS-News, vollständiges Upgrade mit Live-Fortschritt und Protokoll |
| Software | installierte Pakete (Repository und Lokal/AUR getrennt, Filter, Details, Installationsgrund), Suche in den konfigurierten Repositories, Installieren und konservatives Entfernen nach Folgenübersicht |
| System | OS, Kernel, CPU, GPU, RAM, Datenträger, Sitzung, pacman/libalpm, Hyprland (Monitore, Workspaces, Fensterklasse), Gesundheitszentrum, bereinigter Diagnosebericht |
| Aktivität | Vorgänge der App, des Timers und externe pacman-Transaktionen |
| Einstellungen | Theme, Sprache (Deutsch/Englisch), Dichte, Benachrichtigungen, Prüfintervall, automatische Updates mit Zeitfenster, Log-Aufbewahrung, News, MCP |
| MCP | lokaler stdio-Server mit sechs Lese-Tools, standardmäßig deaktiviert ([docs/mcp.md](docs/mcp.md)) |

**Automatische Updates:** „Aus“ (Standard), „Nur benachrichtigen“ und „Automatisch beim nächsten
Neustart installieren“ über CachyOS `pacman-offline`. Letzteres ist **in Entwicklung** und
gesperrt, bis der Offline-Updatepfad auf einer VM verifiziert ist
([docs/automatische-updates.md](docs/automatische-updates.md)). cachyos-center startet nie
selbst einen Neustart.

## Sicherheit in Kürze

- Die Oberfläche läuft ohne Root. Paketänderungen führt ein D-Bus-aktivierter Helper aus, der
  jeden Aufruf per Polkit für den tatsächlichen Absender autorisiert und pacman nur mit festen
  Argumenten startet (keine Shell, keine freien Befehle, nie `--overwrite`/`--nodeps`).
- Vor dem Commit berechnet der Helper den Plan neu und bricht bei jeder Abweichung von der
  bestätigten Vorschau ab. Der pacman-Lock wird nie gelöscht.
- Keine Telemetrie, keine externen Ressourcen in der Oberfläche; Diagnose nur nach Vorschau.

Details: [Sicherheitsmodell](docs/sicherheitsmodell.md), [Architektur](docs/architektur.md).

## Installation

```bash
git clone https://github.com/Jojo252511/cachyos-center.git
cd cachyos-center/packaging/arch
CC_LOCAL_SOURCE=1 makepkg -si
```

Voraussetzungen, installierte Dateien, Deinstallation und Fehlerbehebung:
[docs/installation.md](docs/installation.md). Bedienung: [docs/bedienung.md](docs/bedienung.md).

## Entwicklung

```bash
(cd apps/desktop && npm ci && npm run build)
cargo build --workspace
cargo test --workspace
(cd apps/desktop && npm run typecheck && npm run lint && npm test)
```

Aufbau, Testarten (inkl. Sandbox-Tests mit echtem pacman) und Release-Gates:
[docs/entwicklung.md](docs/entwicklung.md). Schnittstelle zwischen Oberfläche und Backend:
[docs/ipc-vertrag.md](docs/ipc-vertrag.md).

```text
apps/desktop/         Tauri 2 + React/TypeScript (Oberfläche)
crates/core/          Modelle, Validierung, Zustandsmaschine, Fehlercodes
crates/alpm-bridge/   einzige libalpm-Anbindung (zur Laufzeit geladen)
crates/packages/      Paketdienst, isolierte Updateprüfung, pacman.log
crates/system/        Systeminfo, Hyprland-IPC, Gesundheit, News
crates/service/       Application Core für GUI und MCP
crates/helper/        privilegierter D-Bus-Helper
crates/mcp/           lokaler MCP-Server (stdio)
packaging/arch/       PKGBUILD, Polkit, D-Bus, systemd
docs/                 Architektur, Bedienung, Sicherheitsmodell, Status, Tests
tests/                Übersicht und VM-Prüfprotokoll
```

## Offene Entscheidungen

- **Name und Marke:** Vor einer breiteren Veröffentlichung Namens- und Markenkonflikte mit
  „CachyOS“ prüfen.
- **Automatikmodus:** Freigabe erst nach VM-Tests des `pacman-offline`-Pfads (Kernel,
  Signaturfehler, abgebrochener Neustart) und der Kombination mit `offline.conf`.
- **Entfernen lokaler/AUR-Pakete:** in V1 bewusst nicht über die App.
- **System-Tray und KI-Erklärung in der App:** nicht in V1 (Tray optional laut Konzept).
- **Paketsignatur:** Release-Artefakte werden noch nicht signiert, nur mit SHA-256 veröffentlicht.
- **MCP-Fehlercode `INVALID_INPUT`:** zusätzlich zu den fünf Codes des Konzepts, damit ein Host
  Eingabefehler erkennt ([docs/mcp.md](docs/mcp.md)).
- **Abbrechen per Polkit:** eigene Aktion `org.cachyos-center.packages.cancel` ohne Passwort für
  aktive lokale Sitzungen, weil ein Abbruch vor dem Commit nichts am System ändert.

Stand nach Meilensteinen, Abweichungen, Einschränkungen und nächste Schritte:
[docs/status.md](docs/status.md).

## Lizenz

GPL-3.0-or-later, siehe [LICENSE](LICENSE). Die libalpm-Bindings (`alpm`) stehen unter GPL-3.0.
