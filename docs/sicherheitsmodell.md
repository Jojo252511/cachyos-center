# Sicherheitsmodell

## Schutzziele

1. Kein unbefugter Nutzer und keine manipulierte Eingabe (Oberfläche, D-Bus, MCP-Host) kann
   Paketänderungen oder beliebige Befehle mit Root-Rechten auslösen.
2. pacman bleibt die einzige Instanz für Transaktionen, Signaturprüfung, Hooks und Locking.
   cachyos-center umgeht keine dieser Prüfungen.
3. Das System gerät nicht in einen Teil-Upgrade-Zustand; eine fehlgeschlagene Transaktion wird
   sichtbar gemacht und nicht als Erfolg ausgegeben.
4. Persönliche Daten verlassen den Rechner nicht ohne ausdrückliche Nutzeraktion.

## Rechtegrenze

| Komponente | Rechte | Darf | Darf nicht |
|---|---|---|---|
| Oberfläche (`cachyos-center`) | Benutzer | lesen, Pläne berechnen, typisierte Aufträge an den Helper senden | Paketdatenbank ändern, Root-Befehle ausführen, Passwörter abfragen |
| MCP-Server | Benutzer | sechs Lese-Tools | den Helper erreichen (keine D-Bus-Abhängigkeit im Crate), Dateien lesen, Updates starten |
| Helper | root | vier festgelegte ändernde Methoden, Abbrechen vor dem Commit, Lesemethoden | Shell, freie Argumente, beliebige Pfade, Lock löschen, Signaturprüfung umgehen |

## Privilegierter Helper

- **Aktivierung:** D-Bus-Systemdienst `org.cachyos_center.Packages1` (Typ `dbus`). Nur root darf
  den Namen besitzen (`/usr/share/dbus-1/system.d/org.cachyos_center.Packages1.conf`).
- **Methoden:** `UpgradeSystem`, `InstallRepoPackage`, `RemoveRepoPackage`,
  `SetAutoUpdatePolicy`, `ReadOperationStatus` sowie die ergänzenden Methoden
  `ReadOperationLog`, `CancelOperation` und `CurrentOperation`. Alle Parameter sind typisiert
  (Strings mit strenger Validierung, Booleans, Bitmasken); kein Feld nimmt Befehle oder Pfade
  entgegen.
- **Autorisierung:** Jede schreibende Methode prüft den tatsächlichen Bus-Absender
  (`system-bus-name`, eindeutiger Name aus dem Nachrichtenkopf) per Polkit gegen eine eigene
  Aktion: `org.cachyos-center.packages.upgrade`, `…install`, `…remove`,
  `org.cachyos-center.autoupdate.configure` mit `auth_admin_keep` für aktive Sitzungen und `no`
  für inaktive und entfernte. `CancelOperation` prüft für den Initiator die Aktion
  `org.cachyos-center.packages.cancel` (aktive lokale Sitzung ohne Passwort, `no` für inaktive und
  entfernte; ein Abbruch vor dem Commit ändert nichts am System). Andere Benutzer brauchen zum
  Abbrechen die Berechtigung des Vorgangs selbst. Es wird keine Polkit-Regel ausgeliefert, die
  Aktionen pauschal freigibt. Das Passwort fragt der Polkit-Agent der Sitzung ab; die App enthält
  kein Passwortfeld. Antwortet Polkit nicht innerhalb von fünf Minuten, bricht der Helper die
  Prüfung ab (`NOT_AUTHORIZED`).
- **Test der Autorisierung:** Die Sandbox-Tests starten den Helper mit dem echten Polkit-Codepfad
  gegen eine Test-Authority auf dem privaten Bus und prüfen Subjekt (Bus-Name des Aufrufers),
  Action-ID je Methode, Ablehnung, Challenge ohne Agent, Zeitüberschreitung mit Rücknahme des
  Dialogs sowie den Abbruch. Der echte Polkit-Dialog ist nur auf einer VM prüfbar.
- **Validierung:** Paketnamen nach makepkg-Regeln (kein führendes `-` oder `.`), Repository nur aus
  der pacman-Konfiguration, Plan-Digest als 64-stelliges Hex, Vorgangs-IDs als UUID. Entfernen nur
  für installierte Repository-Pakete; `HoldPkg`-Pakete werden abgelehnt.
- **Feste Argumente:** Jeder pacman-Aufruf ist ein fester Vektor
  (`-Sy`, `-Suw`, `-Su`, `-Su --needed -- repo/pkg`, `-R`/`-Rs -- pkg`) mit
  `--noconfirm --noprogressbar --color never`. Es gibt keinen Codepfad mit `--overwrite`,
  `--nodeps`, `-dd`, `--cascade`, `--nosave`, `--dbonly` oder `--noscriptlet` (Tests in
  `crates/helper/src/runner.rs`). Ausgeführt wird ohne Shell mit geleerter Umgebung (`LC_ALL=C`).
- **Plan-Prüfung vor dem Commit:** Der Helper berechnet den Plan nach dem Refresh selbst und
  vergleicht ihn mit dem bestätigten Digest; bei jeder Abweichung stoppt er vor dem Commit.
- **Lock:** Existiert `db.lck`, wartet der Helper mit begrenztem Backoff und bricht dann mit
  `BUSY` ab. Der Lock wird nie gelöscht.
- **Abbruch:** Nur vor dem Commit (Autorisierung, Vorbereitung, Download). Während der
  Installation gibt es keinen Abbruch und kein Beenden des Prozesses; ein logind-Inhibitor
  blockiert Herunterfahren und Ruhezustand.
- **Isolation:** pacman läuft in transienten systemd-Units; ein Absturz des Helpers beendet pacman
  nicht. Der Helper-Dienst selbst läuft mit `ProtectSystem=full`, `ProtectHome=yes`,
  `NoNewPrivileges=yes` und weiteren Einschränkungen.
- **Entwicklungsmodus:** `--dev-root` (Session-Bus, Sandbox-Pfade, Test-Autorisierung) wird
  verweigert, wenn der Prozess als root läuft. Die Produktivkonfiguration ist im Code fest.

## Auto-Update

Die Policy wird nur vom Helper nach Polkit-Autorisierung geschrieben. `/etc/pacman.conf` und
`/etc/pacman.d/offline.conf` werden nie verändert. Der Modus „Automatisch beim nächsten Neustart
installieren“ ist zusätzlich durch eine Freischaltung des Administrators gesperrt, solange der
Offline-Updatepfad nicht in einer VM verifiziert ist. Details: [Automatische Updates](automatische-updates.md).

## Oberfläche

- Tauri-Capability `main-window` erlaubt genau die Commands des [IPC-Vertrags](ipc-vertrag.md)
  und `event:listen`/`unlisten`; keine Shell-, Dateisystem- oder Prozessrechte im Webview.
- Content Security Policy ohne fremde Quellen (`default-src 'self'`), keine externen Fonts oder
  Bilder, `freezePrototype`.
- Externe Links nur nach Nutzeraktion und nur `https://` auf freigegebenen Hosts (Arch Linux,
  CachyOS, Projekt-Repository). `.pacnew`-Dateien können nur im Dateimanager gezeigt werden,
  wenn sie unter `/etc` liegen und im aktuellen Gesundheitsbericht stehen.
- Eingaben werden im Frontend und erneut im Backend validiert.

## Datenschutz

- Keine Telemetrie, kein Modellaufruf, keine Netzwerkfreigabe. Netzwerkzugriffe: Repository-Spiegel
  (über `checkupdates`/pacman) und – abschaltbar – die offiziellen News-Feeds von archlinux.org und
  cachyos.org (HTTPS, Größen- und Zeitlimit).
- Diagnosebericht: aus strukturierten Daten erzeugt und durch den Sanitizer bereinigt
  (Home-Pfade, Benutzer- und Hostname, IP-/MAC-Adressen, E-Mail-Adressen, Token); Kopieren erst nach
  Vorschau und Klick.
- Hyprland: Monitor-Seriennummern werden nicht gelesen; keine Fenstertitel fremder Anwendungen.
- pacman-Kommandozeilen aus `pacman.log` (können Home-Pfade enthalten) werden nie ausgegeben;
  Aktivität und MCP zeigen nur Zählungen und Paketnamen.
- MCP ist standardmäßig deaktiviert und nur lokal über stdio erreichbar.

## Abhängigkeiten

CI prüft mit `cargo deny` Advisories, Lizenzen und Quellen sowie mit `npm audit` die
Frontend-Abhängigkeiten. Ein eigener Job durchsucht bei jedem Lauf alle Commits mit `gitleaks`
nach Zugangsdaten (Version und SHA-256 der Programmdatei sind in der Workflow-Datei festgelegt).
cachyos-center selbst verwendet keine Zugangsdaten; die CI nutzt nur das Standard-Token mit
Leserechten. Stand 2026-09-30 meldet `cargo audit` zwei Warnungen, beide transitiv
über den Linux-Stack von Tauri 2 (gtk-rs 0.18) und nicht von cachyos-center-Code genutzt:
`RUSTSEC-2024-0370` (proc-macro-error nicht mehr gepflegt, nur zur Compile-Zeit) und
`RUSTSEC-2024-0429` (`glib::VariantStrIter`, wird nicht verwendet). Sie entfallen mit einem
Tauri-Release auf Basis neuerer gtk-rs-Versionen.

## Bekannte Grenzen

- Der Polkit-Dialog und der Helper als echter Systemdienst wurden nicht real getestet (erfordert
  Root-Installation, siehe [Testprotokoll](testprotokoll.md)).
- Pakete werden nicht signiert ausgeliefert; Release-Artefakte tragen SHA-256-Prüfsummen.
- „Lokal/AUR“ ist eine Näherung (fremde Pakete); die Herkunft ist nicht sicher bestimmbar.
