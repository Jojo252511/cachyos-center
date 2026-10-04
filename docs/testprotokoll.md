# Testprotokoll

Stand: 2026-10-01, cachyos-center 0.1.0.

## Testumgebung

| | |
|---|---|
| Rechner | Entwicklungsrechner mit CachyOS, Hyprland 0.56.2 (Wayland), NVIDIA-GPU |
| Kernel | 7.2.8-1-cachyos |
| pacman / libalpm | 7.1.0 / libalpm 16 |
| Rust | 1.98.1 (MSRV laut `Cargo.toml`: 1.89) |
| Node.js / npm | 26.10.0 / 12.1.0 |
| WebKitGTK | 2.52.6 (`webkit2gtk-4.1`) |

## Sicherheitsrahmen der Tests

- Auf dem Entwicklungsrechner wurden **keine echten Paketänderungen** ausgeführt und kein Test
  verwendet `sudo`.
- Schreibende pacman-Läufe fanden ausschließlich in einer **isolierten Sandbox** statt: eigene
  Root, Datenbank, Cache, Log und Keyring, ein `file://`-Repository mit per makepkg gebauten
  Fixture-Paketen, pacman unter `fakeroot`, der Helper auf einem privaten `dbus-daemon`
  (`crates/helper/tests/sandbox.rs`).
- Lesende Tests gegen das echte System prüfen ausdrücklich, dass kein `db.lck` entsteht und die
  produktive Sync-Datenbank unverändert bleibt.
- Polkit-Dialog, Helper als echter Systemdienst und der `pacman-offline`-Pfad erfordern eine
  Root-Installation und sind deshalb der VM vorbehalten (Abschnitt „VM-Testmatrix“).

## Befehle

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
CC_REQUIRE_BRIDGE=1 CC_REQUIRE_SANDBOX=1 cargo test --workspace --locked
cargo test --workspace --locked -- --ignored          # Netzwerk-Tests
cargo deny check advisories licenses bans sources
(cd apps/desktop && npm run typecheck && npm run lint && npm test && npm run build)
(cd apps/desktop && npm audit --audit-level=high)
python3 tests/ui/fokus_webkit.py                      # Fokus in WebKitGTK, eigenes Xvfb (auch in CI)
(cd packaging/arch && CC_LOCAL_SOURCE=1 makepkg -f)  # baut nur, installiert nicht
```

## Ergebnisse

Lauf vom 2026-10-04 auf dem Entwicklungsrechner, Rust und Oberfläche auf Commit `cb6aab9` (Nachbesserung nach dem ersten Realbetrieb). Der Fokus-Lauf mit `fokus_webkit.py` stammt vom Commit `1605ef1`; die Oberfläche hat sich seitdem nur bei Aktivität und Gesundheitszentrum geändert, in der CI läuft er für jeden Commit.

| Befehl | Ergebnis |
|---|---|
| `cargo fmt --all --check` | sauber |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | keine Warnungen |
| `CC_REQUIRE_BRIDGE=1 CC_REQUIRE_SANDBOX=1 cargo test --workspace --locked` | **304 bestanden, 0 fehlgeschlagen**, 3 ignoriert (Netzwerk) |
| `cargo test --workspace --locked -- --ignored` | 3 von 3 bestanden: `isolated_update_check`, `fetches_the_official_feeds`, `notify_only_checks_without_installing` |
| `npm run typecheck`, `npm run lint` | sauber |
| `npm test` | **110 bestanden** in 21 Testdateien |
| `npm run build` | erfolgreich |
| `npm audit --audit-level=high` | 0 Schwachstellen |
| `python3 tests/ui/fokus_webkit.py` | WebKitGTK 2.52.6: 5 Szenarien, **13 von 13 Erwartungen erfüllt**; Gegenproben: ohne Fokus-Rettung 4 Fehler (Fokus auf `<body>`), mit deaktiviertem statt beschäftigtem Start-Button 1 Fehler (Start im Upgrade-Dialog) |
| `cargo deny check advisories licenses bans sources` | in CI bestanden (lokal nicht installiert) |
| `CC_LOCAL_SOURCE=1 makepkg -f` | Paket gebaut, ohne Warnungen (siehe „Manuelle Prüfungen“) |

Rust-Tests je Testprogramm:

| Crate | Testprogramm | bestanden | ignoriert |
|---|---|---|---|
| `alpm-bridge` | Unit-Tests | 8 | – |
| `core` | Unit-Tests | 133 | – |
| `core` | `tests/packaging.rs` | 5 | – |
| `desktop` (Tauri-Backend) | Unit-Tests | 5 | – |
| `helper` | Unit-Tests | 14 | – |
| `helper` | `tests/preflight.rs` | 3 | 1 (Netzwerk) |
| `helper` | `tests/recovery.rs` | 3 | – |
| `helper` | `tests/sandbox.rs` (echtes pacman, Polkit-Codepfad) | 12 | – |
| `mcp` | Unit-Tests | 34 | – |
| `mcp` | `tests/host.rs` (MCP-Host) | 5 | – |
| `packages` | Unit-Tests | 16 | – |
| `packages` | `tests/system_readonly.rs` | 7 | 1 (Netzwerk) |
| `service` | Unit-Tests | 19 | – |
| `service` | `tests/core_readonly.rs` | 5 | – |
| `system` | Unit-Tests | 35 | 1 (Netzwerk) |

**CI (GitHub Actions, Container `archlinux:base-devel`):** Fünf Jobs: Rust inklusive
Sandbox-Tests mit echtem pacman, Prüfung der TypeScript-Bindings und GUI-Start unter Xvfb ohne
Hyprland; Frontend; Secret-Scan der gesamten Git-Historie mit gitleaks; cargo-deny; Arch-Paketbau
mit Artefakt und SHA-256. Alle Läufe auf `main` seit dem ersten (Commit `999a498`) waren grün,
soweit sie nicht durch einen neueren Push abgebrochen wurden; gitleaks fand keine Zugangsdaten.

## Abdeckung der kritischen Abläufe

| Anforderung | Nachweis | Ergebnis |
|---|---|---|
| Vollständiges Upgrade mit echtem pacman (`-Sy`, `-Suw`, `-Su`), Live-Protokoll, Erfolg nur mit Nachweis | `sandbox.rs`: `full_upgrade_with_real_pacman` | bestanden |
| Plan ändert sich nach Bestätigung → Stopp vor dem Commit mit Unterschieden | `sandbox.rs`: `changed_plan_stops_before_commit` | bestanden |
| Fremder `db.lck` wird respektiert und nie gelöscht | `sandbox.rs`: `foreign_lock_is_respected_and_never_removed` | bestanden |
| Abbruch nur vor dem Commit, Paketlage unverändert | `sandbox.rs`: `cancel_before_commit`; `runner.rs`: `cancellable_only_before_commit` | bestanden |
| Signaturfehler stoppt vor dem Commit, kein Bypass | `sandbox.rs`: `signature_failure_changes_nothing` | bestanden |
| Installation und Entfernen von Repository-Paketen (inkl. `-Rs`) | `sandbox.rs`: `install_and_remove_repository_packages` | bestanden |
| Lokale/AUR-Pakete werden nicht entfernt | `sandbox.rs`: `local_packages_are_not_removed` | bestanden |
| Verweigerte Autorisierung ändert nichts | `sandbox.rs`: `denied_authorization_changes_nothing` | bestanden |
| Polkit-Codepfad: Subjekt `system-bus-name` des tatsächlichen Aufrufers, eigene Action-ID je Methode, `AllowUserInteraction` | `sandbox.rs`: `polkit_checks_the_bus_sender_for_every_action` (Test-Authority auf dem privaten Bus) | bestanden |
| Polkit-Ablehnung, Challenge ohne Agent und Zeitüberschreitung ändern nichts; der Dialog wird zurückgezogen | `sandbox.rs`: `polkit_denial_challenge_and_timeout_change_nothing` | bestanden |
| Abbrechen nur nach Polkit-Prüfung, offener Dialog wird zurückgezogen | `sandbox.rs`: `cancel_is_checked_by_polkit_and_withdraws_the_dialog` | bestanden |
| Ungültige Eingaben (Optionen als Paketname, falsches Repository, falscher Digest) | `sandbox.rs`: `invalid_input_is_rejected_synchronously`; Unit-Tests in `core::validate` | bestanden |
| Feste pacman-Argumente, nie `--overwrite`/`--nodeps`/`-dd` | `runner.rs`: `fixed_argument_vectors`, `never_dangerous_flags` | bestanden |
| Wiederherstellung nach Helper-Absturz aus Journal und `pacman.log` | `recovery.rs` (3 Tests) | bestanden |
| Nie „0 Updates“ ohne frische Prüfung; fehlgeschlagener Vorgang macht den Stand veraltet | `system_readonly.rs`: `never_checked_is_not_zero_updates`; `core_readonly.rs`: `failed_transaction_after_check_makes_status_stale` | bestanden |
| Pläne ohne Lock, produktive Sync-Datenbank unverändert | `system_readonly.rs`: `plans_are_computed_without_taking_the_lock`, `isolated_update_check` | bestanden |
| Automatikmodus gesperrt, Vorabprüfungen vor Netzwerkzugriff; begrenztes Warten auf einen fremden Paketmanager ohne Lock-Löschen | `preflight.rs` (Integration und `waits_for_a_foreign_lock_and_never_removes_it`), `updaters.rs`: `blockers`, `experimental_switch` | bestanden |
| Diagnosebericht ohne Benutzer-, Hostnamen, Home-Pfade, Adressen; ohne frische Prüfung „updates: unknown“ | `core_readonly.rs`: `diagnostic_report_is_sanitized`; `core::sanitize`; `diagnostic.rs`: `counts_are_only_current_after_a_fresh_check` | bestanden |
| `checkupdates` blockiert nie an einer vollen Pipe; Zeitüberschreitung wird gemeldet | `check.rs`: `long_output_never_blocks_the_check`, `timeout_is_reported` | bestanden |
| MCP nur lesend, standardmäßig aus, stdout nur Protokoll; ohne Prüfung `updateCount: null` statt 0 | `crates/mcp/tests/host.rs`, `crates/mcp/src/tests.rs` | bestanden |
| Paketierung passt zum Code (Bus-Name, Polkit-Aktionen, Units, Fensterklasse, Pfade) | `crates/core/tests/packaging.rs` | bestanden |
| Oberfläche: Upgrade-/Installations-/Entfernen-Dialog, Planabweichung, Fortschritt, Auto-Update, Fokusführung (Dialoge, Paketdetails, Vorgangsergebnis, Abbrechen, News, Buttons im Zustand „busy“, Bedienelemente, die ein Zustandswechsel entfernt oder deaktiviert: erste Prüfung aus dem Leerzustand und auf der Übersicht, „Erneut prüfen“, „Übernehmen“, Seitenblättern, „Zum Ende springen“ im Protokoll, Start-Button im Upgrade-Dialog während des Starts und nach einem Startfehler, deaktivierte Bedienelemente in Dialogen, Rückgabe aus Dialogen, Startfehler-Bildschirm), Hinweis auf Teilaktualisierung, zurückgehaltene Pakete | `apps/desktop/src/**/*.test.ts(x)`, darunter `lib/focus.test.ts` und `components/Button.test.tsx`; die Fokus-Tests sind per Mutationsprobe gegen das alte Verhalten geprüft (ohne Fokus-Rettung, ohne Fokusgruppe, ohne Fokusziele, ohne `MutationObserver`, Nachfolger erst nach dem Entfernen gesucht, ohne Busy-Sperre, `busy` ohne Vorrang vor `disabled`, ohne Fokus-Rettung in Dialogen, ohne Fokus nach dem Neustartversuch: jeweils rot) | bestanden |

## Manuelle Prüfungen auf dem Entwicklungsrechner

Alle Prüfungen ohne Root und ohne Änderung an Paketen oder Systemdiensten.

| Prüfung | Vorgehen | Ergebnis |
|---|---|---|
| Paketbau | `CC_LOCAL_SOURCE=1 makepkg -f` (ohne `-i`, nichts installiert) | Paket `cachyos-center-0.1.0-1-x86_64.pkg.tar.zst` (9,7 MB) gebaut, `check()` mit allen Unit-Tests bestanden. Der erste Lauf meldete „Paket enthält einen Verweis auf $srcdir“ (Panic-Pfade der Abhängigkeiten); behoben mit `--remap-path-prefix`, der zweite Lauf ist warnungsfrei. Inhalt: drei Programme, libalpm-Bridge, Desktop-Datei, Icons (SVG, 32–512 px), Polkit-Policy, D-Bus-Aktivierung und -Policy, System- und User-Units, tmpfiles, Lizenz, Dokumentation; die Demodaten (Mock) der Oberfläche sind nicht enthalten |
| GUI aus dem Paket, X11 ohne Hyprland (A1, A7) | gepacktes `cachyos-center` unter Xvfb (1280×860) mit eigenem Session-Bus und leeren Benutzerverzeichnissen, libalpm-Bridge aus dem Paket, Helper nicht installiert; Bedienung per XTest | Start ohne Root mit realen Daten (CachyOS, Kernel 7.2.8-1-cachyos, Ryzen 7 5700X, RTX 5060, 1.479 Pakete, davon 14 Lokal/AUR, 6 `.pacnew`). Zuerst „Noch nicht geprüft“ statt „0 Updates“; die Hintergrundprüfung fand nach 20 s 35 Updates (1,1 GiB) in der isolierten Datenbank. Installieren, Katalog-Installation und Auto-Update-Richtlinie sind mit „Hilfsdienst nicht verfügbar“ gesperrt. Vorabprüfungen inkl. realer Arch-News (News-Gate aktiv), Katalogsuche, Filter „Lokal/AUR“ mit Herkunftshinweis, Systemseite, Hinweis „Keine Hyprland-Sitzung erkannt“, Gesundheitszentrum, Aktivität (App-Prüfung und externe pacman-Transaktionen), Einstellungen, MCP-Host-Konfiguration, dunkles Theme |
| GUI in der echten Hyprland-Sitzung (A1) | Debug-Build mit eingebetteter Oberfläche, Hyprland 0.56.2, Wayland, NVIDIA | natives Wayland-Fenster (kein XWayland), Klasse und Titel `cachyos-center`, Fokus im Fenster, fehlerfreies Rendering (DMABUF-Workaround aktiv). Die App folgt der Portal-Einstellung `prefer-light`; ohne Portal-Präferenz ist Dunkel Standard (dabei gefunden und behoben) |
| Hyprland-IPC | `cargo run -p cachyos-center-service --example dump -- hyprland` | Version 0.56.2, zwei Monitore mit Auflösung, Bildrate, Skalierung und aktivem Workspace, Fensterklasse; keine Seriennummern |
| Offline (F1) | `unshare --user --net` (eigener Netzwerk-Namespace ohne Verbindung), `dump check` und `dump dashboard` | Prüfung endet mit Status `failed`, Code `OFFLINE`, kein „0 Updates“; installierte Pakete (1.479), Lock-Status und Systeminformationen bleiben lesbar; kein `db.lck` |
| Nicht unterstützte libalpm (F7) | Kopie der Bridge mit `patchelf --replace-needed libalpm.so.16 libalpm.so.17`, Start von `dump` und der GUI aus dem Paket | App startet, „Paketfunktionen nicht verfügbar“ mit deutscher Erklärung (libalpm passt nicht zu diesem Build) und technischem Grund; Systeminformationen bleiben verfügbar. Dabei zwei Fehler gefunden und behoben: die dlopen-Ursache ging verloren, und die Oberfläche zeigte nur englischen Text |
| Produktive Sync-Datenbank | Zeitstempel von `/var/lib/pacman/sync` vor und nach der isolierten Prüfung (`isolated_update_check`) | unverändert |
| Fokusverhalten der Engines | statische Testseite in WebKitGTK 2.52.6 unter Xvfb: fokussierten Button per Tab erreichen, dann entfernen bzw. deaktivieren | Entfernen: Fokus sofort auf `<body>`, **kein** `blur`/`focusout`. Deaktivieren: Fokus bleibt zunächst, wenige Millisekunden später folgt ein `blur` mit Fokus auf `<body>`. Chromium meldet das Entfernen dagegen mit `focusout`. Die Fokus-Rettung beobachtet deshalb den DOM mit einem `MutationObserver` und wertet zusätzlich `focusout` aus |
| Fokusführung mit echten Tastendrücken | `tests/ui/fokus_webkit.py` (WebKitGTK 2.52.6, eigenes Xvfb, Enter/Leertaste per XTest, Mock-Daten) und die Vite-Vorschau im Chromium-Browserbereich; Fokus über `document.activeElement` und `focusin`/`focusout` protokolliert | Erste Prüfung aus dem Leerzustand der Updates-Seite: Der Leerzustand verschwindet, der Fokus springt nach 6 bis 27 ms (drei Läufe) auf den Kopf-Button „Prüfung läuft …“ und bleibt danach auf „Jetzt prüfen“. Erste Prüfung auf der Übersicht: Der Fokus bleibt während der Prüfung auf dem Button und geht danach auf den Link „Updates ansehen“, der ihn ersetzt. „Übernehmen“: Er bleibt während des Speicherns fokussiert, nach dem verzögerten `blur` des deaktivierten Buttons liegt der Fokus auf „Richtlinie übernommen.“. „Als gelesen markieren“ springt auf die News-Statuszeile, „Abbrechen“ auf den Titel des Vorgangs, beides ohne Umweg über die Seitenüberschrift. Während „Upgrade starten“ läuft, bleibt der Fokus auf dem beschäftigten Start-Button; danach ist „Installieren“ gesperrt und der Fokus liegt auf „Jetzt prüfen“. Chromium zeigt dieselben Ziele für Leerzustand und Übersicht; einen deaktivierten Button ließ es im verborgenen Browserbereich fokussiert, bei fokussiertem Fenster entzieht es ihm den Fokus und die Rettung setzt ihn auf „Richtlinie übernommen.“ (Prüfdurchlauf 5) |

## Realbetrieb auf dem Entwicklungsrechner

Der Nutzer hat das Paket 0.1.0-1 am 2026-10-01 selbst mit `makepkg -si` installiert und seitdem
im Alltag verwendet. Ausgewertet am 2026-10-04, nur lesend: Helper-Journal unter
`/var/lib/cachyos-center/operations`, `/var/log/pacman.log`, `systemctl`, Journal des Helpers und
der Diagnosebericht der App. Kernel 7.2.9-1-cachyos, pacman 7.1.0, libalpm 16.0.1.

| Nachweis | Ergebnis |
|---|---|
| Systemupgrades über die App | drei erfolgreich: 2026-10-01 (38 Pakete), 2026-10-02 (15), 2026-10-04 (6), jeweils mit Commit; die Zählungen stimmen mit `pacman.log` überein. Der Helper lief per D-Bus-Aktivierung als Systemdienst und beendete sich nach zehn Minuten Leerlauf. Die Freigabe lief über Polkit (Aktion `auth_admin_keep`); ob der Agent dabei nach dem Passwort fragte, geht aus den Protokollen nicht hervor |
| „Nur benachrichtigen“ | Timer aktiviert, tägliche Prüfung mit Zufallsverzögerung; zwei Timer-Prüfungen im Journal (15 und 6 Updates), keine Installation. Ein verpasster Termin wurde beim nächsten Start nachgeholt |
| Richtlinie ändern | Der Helper schrieb das Drop-in `schedule.conf`, systemd plante den nächsten Lauf neu |
| Diagnosebericht | Werte stimmen mit dem System überein (Repositories in der Reihenfolge der `pacman.conf`, Paketzahlen, Kernel nach dem Neustart, Lock, Timer). Dabei zwei Schwächen gefunden und behoben: Updateprüfungen erschienen als „(+0 ~0 -0)“ statt mit der Zahl der gefundenen Updates, und der Cache-Hinweis empfahl `paccache -r`, obwohl es nichts aufzuräumen gab (17,9 GiB Cache, aber höchstens drei Versionen je Paket) |
| Cache-Messung | `package_cache_usage` gegen `paccache -d` auf dem realen Cache (2.137 Dateien): `-k3` 0 B, `-k2` 2,25 GiB, `-k1` 5,92 GiB, jeweils identisch; 650 Versionspaare aus dem Cache gegen `vercmp` geprüft, keine Abweichung |

Nicht abgedeckt: frische Installation auf einem sauberen System, Deinstallation, Installieren und
Entfernen einzelner Pakete, Fehlerszenarien und `pacman-offline`. Der Realbetrieb ersetzt die
VM-Testmatrix nicht.

## VM-Testmatrix

Eine CachyOS-VM wurde für diese Version **nicht bereitgestellt**. Die Szenarien aus
[tests/vm/README.md](../tests/vm/README.md) sind deshalb nicht auf einer VM gelaufen. Soweit
möglich ist die Logik ohne Root-Rechte in der Sandbox abgedeckt; die Spalte „Ersatznachweis“
nennt den Test. Ein Ersatznachweis ersetzt den VM-Lauf nicht.

| Nr. | Szenario | VM-Lauf | Ersatznachweis |
|---|---|---|---|
| A1 | App ohne Root starten, reale Daten | offen | GUI aus dem Paket unter X11 und Debug-Build in der Hyprland-Sitzung des Entwicklungsrechners, jeweils mit realen Daten (siehe „Manuelle Prüfungen“) |
| A2 | Updates prüfen und installieren mit Polkit | offen | `full_upgrade_with_real_pacman` (ohne Polkit-Dialog); Realbetrieb: drei Systemupgrades über die App mit Polkit-Freigabe (siehe „Realbetrieb“) |
| A3 | Paketverwaltung durch andere Instanz gesperrt | offen | `foreign_lock_is_respected_and_never_removed` |
| A4 | Repository-Paket installieren | offen | `install_and_remove_repository_packages` |
| A5 | Lokales Paket bleibt unangetastet | offen | `local_packages_are_not_removed` |
| A6 | „Nur benachrichtigen“ mit Timer | offen | `notify_only_checks_without_installing` (Netzwerk, ohne systemd-Timer); Realbetrieb: Timer seit 2026-10-01 aktiv, Prüfungen ohne Installation |
| A6b | Automatikmodus mit `pacman-offline` | offen (Modus gesperrt) | `preflight.rs`, `updaters.rs` |
| A7 | Andere Desktop-Sitzung ohne Hyprland | offen | GUI aus dem Paket unter X11 ohne Hyprland vollständig bedient; CI-Start unter Xvfb |
| A8 | MCP-Host mit Beispielkonfiguration | offen | `crates/mcp/tests/host.rs` |
| A9 | Signaturfehler / Spiegel unerreichbar | offen | `signature_failure_changes_nothing` |
| A10 | GUI- und Helper-Neustart während eines Vorgangs | offen | `recovery.rs` |
| F1 | Offline | offen | auf dem Entwicklungsrechner in einem Netzwerk-Namespace ohne Verbindung geprüft (siehe „Manuelle Prüfungen“) |
| F2 | Voller Datenträger | offen | – |
| F3 | Kein Polkit-Agent | offen | `polkit_denial_challenge_and_timeout_change_nothing` (Challenge ohne Agent und Zeitüberschreitung gegen eine Test-Authority) |
| F4 | Abbruch während Download | offen | `cancel_before_commit` |
| F5 | Kernel-Update, Neustartempfehlung | offen | Unit-Tests in `core::classify` |
| F6 | `.pacnew` im Gesundheitszentrum | offen | Gesundheitszentrum zeigt die sechs realen `.pacnew`-Dateien des Entwicklungsrechners; Unit-Tests in `system::health` |
| F7 | Nicht unterstützte libalpm-Hauptversion | offen | per `patchelf` simuliert, GUI und Kern geprüft (siehe „Manuelle Prüfungen“); Unit-Tests in `packages::bridge` |

## Offene Punkte vor einer stabilen Version

1. VM-Testmatrix vollständig durchführen und hier mit Datum, Paketversion, pacman-Version und
   Kernel eintragen.
2. Automatikmodus erst nach A6b, Kernel-Update über `pacman-offline` und abgebrochenem Neustart
   freigeben.
3. Release-Artefakte signieren.
