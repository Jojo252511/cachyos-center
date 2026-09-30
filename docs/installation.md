# Installation und Deinstallation

cachyos-center wird als Arch-Paket gebaut. Voraussetzungen zur Laufzeit: CachyOS oder ein anderes
Arch-basiertes System (experimentell) mit pacman 7.1 / libalpm 16, polkit mit einem laufenden
Polkit-Agenten in der Sitzung, systemd, WebKitGTK 4.1.

## Paket bauen und installieren

```bash
git clone https://github.com/Jojo252511/cachyos-center.git
cd cachyos-center/packaging/arch
makepkg -si
```

`makepkg -si` baut den veröffentlichten Tag. Den aktuellen Arbeitsstand baut
`CC_LOCAL_SOURCE=1 makepkg -si` (nur committete Änderungen).

Das Paket installiert:

| Pfad | Inhalt |
|---|---|
| `/usr/bin/cachyos-center` | Oberfläche |
| `/usr/bin/cachyos-center-mcp` | lokaler MCP-Server (standardmäßig deaktiviert) |
| `/usr/lib/cachyos-center/cachyos-center-helper` | privilegierter Helper (D-Bus-aktiviert) |
| `/usr/lib/cachyos-center/libcachyos_center_alpm.so` | libalpm-Bridge |
| `/usr/share/dbus-1/system-services/org.cachyos_center.Packages1.service`, `/usr/share/dbus-1/system.d/org.cachyos_center.Packages1.conf` | D-Bus-Aktivierung und Bus-Policy |
| `/usr/share/polkit-1/actions/org.cachyos-center.policy` | Polkit-Aktionen |
| `/usr/lib/systemd/system/cachyos-center-helper.service`, `cachyos-center-preflight.{service,timer}` | Systemdienste |
| `/usr/lib/systemd/user/cachyos-center-notify.{path,service}` | Benachrichtigung über Timer-Ergebnisse |
| `/usr/lib/tmpfiles.d/cachyos-center.conf` | Verzeichnisse unter `/etc`, `/var/lib`, `/var/log` |

Es wird kein Dienst automatisch aktiviert. Der Timer wird erst durch die Einstellung
„Automatische Updates“ aktiviert; die Benachrichtigungs-Unit aktiviert die Oberfläche beim Setzen der
Einstellung, alternativ manuell:

```bash
systemctl --user enable --now cachyos-center-notify.path
```

## Erster Start

- Start über das Anwendungsmenü oder `cachyos-center`. Hyprland-Fensterregeln verwenden die
  Klasse `cachyos-center`.
- Die erste Updateprüfung erfolgt nach der Einstellung „Updateprüfung“ (Standard alle 6 Stunden,
  erste Prüfung etwa 20 Sekunden nach dem Start) oder über „Jetzt prüfen“.
- Paketänderungen fordern eine Authentifizierung über den Polkit-Agenten der Sitzung an. Unter
  Hyprland muss ein Agent laufen (z. B. `hyprpolkitagent` oder der Agent der Desktop-Shell).

## Deinstallation

```bash
sudo pacman -R cachyos-center
```

Vor dem Entfernen wird `cachyos-center-preflight.timer` deaktiviert. Konfiguration und Verlauf
bleiben erhalten und können bei Bedarf gelöscht werden:

```bash
sudo rm -r /etc/cachyos-center /var/lib/cachyos-center /var/log/cachyos-center
rm -r ~/.config/cachyos-center ~/.local/share/cachyos-center ~/.cache/cachyos-center ~/.local/state/cachyos-center
systemctl --user disable cachyos-center-notify.path
```

## Fehlerbehebung

| Symptom | Ursache und Abhilfe |
|---|---|
| Fenster stürzt mit „Error 71 (Protokollfehler) dispatching to Wayland display“ ab | WebKitGTK-DMABUF-Renderer mit NVIDIA unter Wayland. cachyos-center setzt `WEBKIT_DISABLE_DMABUF_RENDERER=1` in dieser Kombination automatisch; bei anderen Treibern manuell setzen. |
| „Paketfunktionen deaktiviert: libalpm nicht kompatibel“ | pacman wurde auf eine neue libalpm-Hauptversion aktualisiert. cachyos-center neu bauen (`makepkg -si`); System- und Einstellungsfunktionen bleiben nutzbar. |
| „Helper nicht verfügbar“ | Paket vollständig installiert? Normalerweise lädt ein pacman-Hook die D-Bus-Konfiguration automatisch neu; andernfalls `sudo systemctl reload dbus` oder Neustart. Prüfen: `busctl --system list --activatable \| grep cachyos_center`. |
| Authentifizierung schlägt sofort fehl | Kein Polkit-Agent in der Sitzung. Agent starten und erneut versuchen. |
| „Paketverwaltung beschäftigt“ | Ein anderer Paketmanager hält `/var/lib/pacman/db.lck`. Den anderen Vorgang beenden lassen; einen verwaisten Lock nur nach eigener Prüfung entfernen (cachyos-center löscht ihn nie). |
| „Voraussetzung fehlt: pacman-contrib/fakeroot“ | `sudo pacman -S pacman-contrib fakeroot` |
