# Umsetzungsstand

Stand: 2026-10-01, Version 0.1.0 (Vorabversion, kein Release-Tag). Maßstab sind das Konzept und
der Implementierungsauftrag des Projekts; Nachweise stehen im [Testprotokoll](testprotokoll.md).

## Meilensteine

| Meilenstein | Umfang | Abnahmekriterium laut Konzept | Status | Nachweis |
|---|---|---|---|---|
| M0 – Spike | Tauri-Fenster auf CachyOS/Hyprland, libalpm-ABI-Prüfung, Polkit- und Timer-Prototyp | Start, Rendering, Paketabfrage und Polkit-Dialog real getestet | **teilweise:** Start, Rendering und Paketabfrage unter Hyprland real geprüft; Polkit-Codepfad gegen eine Test-Authority geprüft; seit 2026-10-01 Systemupgrades mit Polkit-Freigabe im Realbetrieb auf dem Entwicklungsrechner | [Bestandsaufnahme](bestandsaufnahme.md), Testprotokoll „Manuelle Prüfungen“, Sandbox-Tests `polkit_*` |
| M1 – Lesemodus | Dashboard, Systeminfo, installierte Pakete, Repository-Suche, Updateprüfung | keine Root-Rechte nötig; Offline-/Fehlerzustände sichtbar | **erfüllt** | GUI aus dem Paket mit realen Daten (X11, Hyprland), Offline-Prüfung im Netzwerk-Namespace, UI-Tests der Zustände |
| M2 – manuelle Aktionen | Installieren, Entfernen, vollständiges Update mit Helper und Verlauf | echte Testtransaktionen, Lock-/Signatur-/Abbruchfälle bestanden | **erfüllt in der Sandbox:** echte pacman-Transaktionen inklusive Lock, Signaturfehler, Abbruch, Planabweichung und Wiederherstellung; im Realbetrieb drei Systemupgrades über den Helper als Systemdienst (Installieren und Entfernen dort noch nicht) | `crates/helper/tests/sandbox.rs` (12 Tests), `recovery.rs`, CI |
| M3 – Auto-Update | Policy, systemd-Preflight, `pacman-offline`-Integration, Stop-Regeln, Benachrichtigungen | Vorbereitung ohne GUI; Installation beim nächsten manuellen Neustart; Blockaden erkannt | **teilweise:** „Nur benachrichtigen“ fertig; Automatikmodus implementiert, aber gesperrt und als „in Entwicklung“ gekennzeichnet, bis der `pacman-offline`-Pfad auf einer VM verifiziert ist | `preflight.rs`, [Automatische Updates](automatische-updates.md) |
| M4 – MCP | read-only stdio-Server und Host-Beispiel | Tools liefern Daten, keine Schreiboperation erreichbar | **erfüllt** | `crates/mcp/tests/host.rs`, [MCP](mcp.md) |
| M5 – Release | Paketierung, Doku, i18n, Barrierefreiheit, CI, Testmatrix | Installation/Deinstallation, Upgrade und Start auf frischem CachyOS geprüft | **offen:** Paketbau, Doku, Deutsch/Englisch, Tastatur- und Screenreader-Bedienung (Fokusführung bei Dialogen, Detailbereichen, Vorgangsergebnissen und bei Bedienelementen, die durch einen Zustandswechsel verschwinden oder deaktiviert werden; getestet in jsdom und mit echten Tastendrücken in WebKitGTK 2.52 und Chromium) und CI fertig; die VM-Testmatrix (frische Installation, Upgrade, Deinstallation) fehlt | Testprotokoll „VM-Testmatrix“, UI-Tests |

## Unabhängige Prüfung

Ein separater Prüfagent hat den Stand in fünf Durchläufen gegen Konzept und Auftrag geprüft (Tests,
Paketbau, CI, Doku, Bedienung mit echten Tastendrücken). Keiner der Durchläufe fand einen Blocker
oder einen Mangel der Stufe Hoch; die gefundenen Mängel wurden jeweils behoben. Durchlauf 5 endete
mit „nicht perfekt“ wegen eines Mangels der Stufe Niedrig (Tastaturfokus beim Start in den
Transaktionsdialogen). Er ist danach behoben und durch UI-Tests, `tests/ui/fokus_webkit.py` und CI
belegt, aber nicht mehr durch einen weiteren Prüfdurchlauf bestätigt.

## Bewusste Abweichungen vom Konzept

| Punkt | Entscheidung | Begründung |
|---|---|---|
| MCP-Fehlercodes | Zusätzlich zu `UNAVAILABLE`, `BUSY`, `STALE`, `UNSUPPORTED`, `INTERNAL` meldet der Server ungültige Argumente als `INVALID_INPUT` | Ein KI-Host soll Eingabefehler von Systemfehlern unterscheiden können; Fehler des Lesedienstes werden weiterhin auf die fünf Codes abgebildet ([MCP](mcp.md)) |
| Polkit für `CancelOperation` | Eigene Aktion `org.cachyos-center.packages.cancel`, für aktive lokale Sitzungen ohne Passwort; Abbrechen fremder Vorgänge braucht die Berechtigung des Vorgangs | Ein Abbruch vor dem Commit ändert das System nie; eine Passwortabfrage zum Abbrechen wäre hinderlich ([Sicherheitsmodell](sicherheitsmodell.md)) |
| Upgrade nur nach frischer Prüfung | „Installieren“ ist erst nach einer höchstens sechs Stunden alten, erfolgreichen Prüfung aktiv | vorsichtigste Auslegung von „nie ohne frische Prüfung“ |
| Paketdateien in den Details | nicht umgesetzt (im Konzept optional) | – |
| System-Tray | nicht umgesetzt (im Konzept „nur bei nachgewiesener Umgebung“) | alle Funktionen sind ohne Tray erreichbar |

## Bekannte Einschränkungen

- Helper als Systemdienst, Polkit-Freigabe und „Nur benachrichtigen“ laufen seit 2026-10-01 im
  Realbetrieb auf dem Entwicklungsrechner (drei Systemupgrades über die App, siehe Testprotokoll
  „Realbetrieb“). Ohne VM nicht geprüft sind eine frische Installation auf einem sauberen System,
  die Deinstallation, die Fehlerszenarien auf einem echten System und der `pacman-offline`-Pfad.
- Der Automatikmodus „Automatisch beim nächsten Neustart installieren“ ist gesperrt; freischalten
  kann ihn nur die Administration über `/etc/cachyos-center/experimental.toml`.
- „Lokal/AUR“ ist eine Näherung (fremde Pakete); die Herkunft ist nicht sicher bestimmbar. Lokale
  Pakete werden in V1 weder aktualisiert noch entfernt.
- Nach einer neuen libalpm-Hauptversion bleiben die Paketfunktionen deaktiviert, bis cachyos-center
  neu gebaut ist.
- Release-Artefakte werden nur mit SHA-256 veröffentlicht, nicht signiert.
- Nach einem Stopp vor dem Commit sind die Paketdatenbanken möglicherweise schon synchronisiert;
  die Oberfläche warnt dann vor einzelnen `pacman -S`-Installationen.

## Nächste Schritte

1. CachyOS-VM mit Hyprland und Polkit-Agent bereitstellen; das Paket mit
   `CC_LOCAL_SOURCE=1 makepkg -si` installieren und `bash tests/vm/pruefen.sh` ausführen.
2. Die Szenarien A1–A10 und F1–F7 aus [tests/vm/README.md](../tests/vm/README.md) durchspielen und
   im Testprotokoll mit Datum, Paketversion, pacman-Version und Kernel eintragen.
3. Den `pacman-offline`-Pfad prüfen (Kernel-Update, Signaturfehler, abgebrochener Neustart,
   Kombination mit `offline.conf`); erst danach den Automatikmodus freigeben. Vor der Freigabe
   bekommen die Blockadegründe des Preflights stabile Kennungen mit deutschen Texten (heute
   erscheinen sie im gesperrten Modus nur als englisches technisches Detail) und die Wartezeit auf
   einen fremden Paketmanager wird auf der VM geprüft (A6b).
4. Bei bestandener Matrix `v0.1.0` taggen, ein Release mit SHA-256 anlegen und die Signatur der
   Artefakte einrichten.
5. Namens- und Markenprüfung zu „CachyOS“ vor einer breiteren Veröffentlichung.
