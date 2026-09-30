# Automatische Updates

## Modi

| Modus | Verhalten | Status |
|---|---|---|
| **Aus** (Standard) | kein Timer, keine automatischen Aktionen | produktiv |
| **Nur benachrichtigen** | Der Timer prüft zum gewählten Zeitpunkt mit isolierter Datenbank und meldet verfügbare Updates per Desktop-Benachrichtigung. Es wird nie etwas installiert. | produktiv |
| **Automatisch beim nächsten Neustart installieren** | Der Timer führt den Preflight aus und bereitet bei Erfolg das Update mit CachyOS `pacman-offline` vor. Installiert wird beim **nächsten vom Nutzer gestarteten Neustart**. | **in Entwicklung**, gesperrt |

cachyos-center aktiviert **nie** einen automatischen Neustart (`pacman-offline-reboot.timer` wird
weder aktiviert noch mit `-t` angestoßen).

## Ablauf

1. Die Einstellung wird über `SetAutoUpdatePolicy` gesetzt (Polkit-Aktion
   `org.cachyos-center.autoupdate.configure`). Der Helper schreibt
   `/etc/cachyos-center/auto-update.toml`, den Zeitplan als Drop-in
   `/etc/systemd/system/cachyos-center-preflight.timer.d/schedule.conf` (`OnCalendar=` aus Wochentagen
   und Uhrzeit) und aktiviert bzw. deaktiviert `cachyos-center-preflight.timer`.
2. Die Oberfläche aktiviert für den angemeldeten Benutzer `cachyos-center-notify.path`
   (systemd-User-Unit). Sie beobachtet `/var/lib/cachyos-center/timer-status.json` und startet
   `cachyos-center --notify-timer-status`, das genau eine Benachrichtigung pro Timer-Lauf sendet.
3. Der Timer startet `cachyos-center-helper preflight` (nicht, solange `db.lck` existiert).

## Preflight (Automatikmodus)

Alle Bedingungen müssen erfüllt sein, sonst wird **nichts** vorbereitet, der Lauf endet mit
`needsAttention`, die Policy bleibt aktiv und der Nutzer erhält Begründung und Hinweis:

- Freischaltung durch den Administrator (siehe unten)
- `pacman-offline` installiert, Konfiguration prüfbar, kein aktiver `pacman-offline-prepare.timer`
  (fremder Vorbereitungstimer = „extern verwaltet“)
- `Include = /etc/pacman.d/offline.conf` ist **nicht** aktiv (siehe „Paketkonsistenz“)
- kein laufender Paketmanager, kein `db.lck`
- Netzbetrieb oder Akku ≥ 50 % (soweit erkennbar), Netzwerkverbindung vorhanden
- mindestens 2 GiB frei auf `/` bzw. das Dreifache der Downloadgröße
- alle News-Feeds abrufbar und keine ungelesenen Arch-/CachyOS-News seit der letzten Bestätigung
  in der Oberfläche (keine automatische Relevanzbewertung)
- Updateprüfung erfolgreich (keine Spiegel-/Signaturfehler), Plan ohne Abhängigkeitsprobleme,
  ohne Paketersetzungen und ohne zurückgehaltene Pakete
- ein verlangter Snapshot wurde von snapper mit Nummer bestätigt

Danach führt der Helper `pacman-offline -y` aus (Refresh, Download, Signaturprüfung,
`/system-update`). Erfolg wird nur gemeldet, wenn `/system-update` danach auf den pacman-Cache
zeigt.

## Freischaltung für Testsysteme

Das Konzept verlangt, den Automatikmodus erst nach realen Tests des Offline-Updatepfads (Kernel,
Signaturfehler, abgebrochener Neustart) produktiv freizugeben. Bis dahin ist er in der Oberfläche
als „in Entwicklung“ sichtbar und gesperrt. Auf einem **Testsystem/VM** kann ein Administrator ihn
freischalten:

```bash
echo 'offline_auto_update = true' | sudo tee /etc/cachyos-center/experimental.toml
```

## Paketkonsistenz mit `offline.conf`

Die von CachyOS dokumentierte optionale `offline.conf` hält Kernelpakete per `IgnorePkg` für
normale Online-Updates zurück. Das steht in Spannung zur Arch-Regel gegen Teil-Upgrades.
cachyos-center:

- fügt selbst nie `IgnorePkg`-Einträge hinzu und ändert keine pacman-Konfiguration,
- zeigt zurückgehaltene Pakete getrennt an und nennt ein Online-Upgrade dann nicht „vollständig“,
- sperrt Online-Paketaktionen, solange ein Offline-Update vorbereitet ist (`CONFLICT`),
- deaktiviert den Automatikmodus für Systeme mit aktivem `offline.conf`, bis in einer VM belegt ist,
  welche manuellen Paketaktionen in dieser Konfiguration zulässig sind.

## Kollisionen

Erkannt und angezeigt werden `pacman-offline-prepare.timer`, `pacman-offline-clean-prepare.timer`,
`pacman-offline-reboot.timer`, `arch-update.timer` (Cachy-Update, Benutzer-Unit) und
`packagekit-offline-update.service`. cachyos-center deaktiviert keine fremden Units.
