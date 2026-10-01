# End-to-End-Test auf einer CachyOS-VM

Diese Prüfungen verändern Pakete und dürfen **nur auf einer Test-VM** ausgeführt werden, nie auf
einem Arbeitsrechner. Sie decken die Abnahmeszenarien aus dem Konzept (Abschnitt 10) und die
Fehlerfälle aus Abschnitt 8 ab, die sich ohne echtes System mit Polkit-Agent nicht prüfen lassen.

## Vorbereitung

1. Frische CachyOS-VM mit Hyprland (Snapshot der VM anlegen, um zurückkehren zu können).
2. Polkit-Agent in der Sitzung (z. B. `hyprpolkitagent`).
3. Paket bauen und installieren: `cd packaging/arch && CC_LOCAL_SOURCE=1 makepkg -si` (baut den
   ausgecheckten Stand; ein Release-Tag existiert erst nach bestandener Matrix).
4. Automatische Prüfungen: `bash tests/vm/pruefen.sh` (nur lesend, Ergebnis in `vm-pruefung-<Datum>.log`).

## Manuelle Szenarien

Ergebnis je Zeile mit Datum, Paketversion von cachyos-center, pacman-Version und Kernel in
[docs/testprotokoll.md](../../docs/testprotokoll.md) eintragen.

| Nr. | Szenario | Erwartung |
|---|---|---|
| A1 | App ohne Root starten | reale OS-/Kernel-/Paketdaten, Sprache Deutsch, Fokus im Fenster, Klasse `cachyos-center` |
| A2 | Updates prüfen, dann „Installieren“ | alte/neue Version und Zeitpunkt sichtbar; Polkit-Dialog; vollständiges Upgrade; Ergebnis nach GUI-Neustart weiterhin sichtbar |
| A3 | Zweiter Paketmanager hält `db.lck` (`sudo pacman -S --needed base` in anderem Terminal, bei der Rückfrage warten) | „Paketverwaltung beschäftigt“, nichts parallel, Lock bleibt |
| A4 | Repository-Paket suchen und installieren | Vorschau zeigt zusätzliche Systemupdates; Polkit; danach unter „Installiert“ |
| A5 | Lokales Paket (`pacman -U`) | „Lokal/AUR – Herkunft unklar“; kein Entfernen/Update durch die App |
| A6 | Auto-Update „Nur benachrichtigen“, Zeitfenster in 2 Minuten | Timer und nächster Lauf sichtbar; Benachrichtigung ohne geöffnete GUI |
| A6b | Automatikmodus (VM mit `pacman-offline`, `experimental.toml` freigeschaltet) | Vorbereitung ohne GUI, Installation erst beim manuellen Neustart; bei ungelesener News/fehlendem Snapshot/anderem Timer keine Vorbereitung und verständliche Meldung |
| A7 | Hyprland beenden, App in anderer Sitzung (z. B. GNOME/KDE) starten | App funktioniert, Hyprland-Bereich erklärt fehlende Sitzung |
| A8 | MCP-Host mit Beispielkonfiguration | `system_get_summary`/`updates_list` liefern Daten; `tools/list` ohne schreibende Tools |
| A9 | Signaturfehler (Paket im Cache beschädigen oder Keyring entfernen) bzw. Spiegel unerreichbar | Vorgang stoppt vor dem Commit, „Paketlage unverändert“, kein Bypass |
| A10 | GUI während eines Upgrades beenden und neu starten; Helper-Prozess während der Installation beenden (`sudo systemctl kill cachyos-center-helper`) | Status wird aus Helper/Journal/pacman.log rekonstruiert, kein Erfolg ohne Nachweis; pacman läuft in eigener Unit weiter |
| F1 | Offline (Netzwerk trennen) | Prüfung „keine Netzwerkverbindung“; installierte Pakete und Systeminfo weiter lesbar |
| F2 | Voller Datenträger (kleine VM-Disk füllen) | Fehler mit Ursache, keine falsche Erfolgsmeldung |
| F3 | Kein Polkit-Agent | Authentifizierung schlägt verständlich fehl, nichts wird geändert |
| F4 | Abbruch während Download | `abgebrochen`, Paketlage unverändert; während Installation kein Abbruch angeboten |
| F5 | Kernel-Update, danach Neustartempfehlung | Hinweis „Neustart empfohlen“ mit Begründung |
| F6 | `.pacnew` erzeugen (Konfigurationsdatei ändern, Paket neu installieren) | Hinweis im Gesundheitszentrum, „Im Dateimanager zeigen“ |
| F7 | pacman auf neue libalpm-Hauptversion simulieren (`libalpm.so.16` umbenennen, nur VM!) | App startet, Paketfunktionen deaktiviert mit Erklärung |
