# Bestandsaufnahme des Entwicklungssystems

Stand: 2026-09-30. Erhoben vor der Implementierung (Implementierungsauftrag, Schritt 1) auf dem
CachyOS-Rechner, auf dem cachyos-center entwickelt wurde. Personenbezogene Angaben (Hostname,
Benutzername, Pfade) sind bewusst nicht enthalten.

## Ergebnisse

| Bereich | Befund | Bedeutung für cachyos-center |
|---|---|---|
| Betriebssystem | CachyOS (`ID=cachyos`, `ID_LIKE=arch`, `BUILD_ID=rolling`) | Primäre Zielplattform |
| Kernel | `7.2.8-1-cachyos` (Paketbasis `linux-cachyos`) | `/usr/lib/modules/<release>/pkgbase` ist vorhanden und wird für Kernel-Kennzeichnung genutzt |
| pacman / libalpm | pacman 7.1.0 (`7.1.0.r9.g54d9411-4`), libalpm 16.0.1 (`libalpm.so.16`) | `alpm`-Crate 5.0.2 unterstützt genau libalpm 16; Bridge prüft die Hauptversion zur Laufzeit |
| pacman-Konfiguration | Repositories `cachyos-v3`, `cachyos-extra-v3`, `cachyos-core-v3`, `cachyos`, `core`, `extra`, `multilib`; `DownloadUser = alpm`; `SigLevel = PackageRequired PackageTrustedOnly DatabaseOptional DatabaseTrustedOnly` | Repositories werden über `pacman-conf` gelesen, nie hart kodiert |
| pacman-contrib | 1.13.1 installiert (`checkupdates`) | Isolierte Updateprüfung verfügbar; `fakeroot` vorhanden |
| WebKitGTK | `webkit2gtk-4.1` 2.52.6, GTK 3.24.52, libsoup3 3.6.6 | Tauri-2-Voraussetzungen erfüllt |
| Hyprland | 0.56.2, aktive Sitzung, IPC-Sockets unter `$XDG_RUNTIME_DIR/hypr/<Signatur>/` | Lesender IPC-Zugriff und Event-Socket funktionieren |
| Grafik | NVIDIA (proprietärer Treiber, Wayland) | WebKitGTK-DMABUF-Renderer bricht mit „Error 71 (Protokollfehler)“ ab; die App schaltet ihn in dieser Kombination automatisch ab |
| polkit | polkit 127, `polkitd` läuft | Kein separater Polkit-Agent-Prozess gefunden; ob die Desktop-Shell einen Agenten stellt, war ohne installierte Policy nicht prüfbar |
| systemd | 262 | Transiente Units (`systemd-run --wait`) und Timer verfügbar |
| Update-Dienste | keine `arch-update`/Cachy-Update-Units, `pacman-offline` nicht installiert, kein `/system-update` | Keine Kollision; Automatikmodus bleibt auf diesem System ohnehin gesperrt |
| Snapshots | Root-Dateisystem Btrfs, snapper mit Konfiguration `root`, `snap-pac` aktiv | pacman erzeugt bereits Pre-/Post-Snapshots; cachyos-center zeigt das an |
| Rust | rustc/cargo 1.98.1 (Arch-Paket) | Edition 2024, MSRV im Workspace 1.89 |
| Node.js | `nodejs` 26.10.0-1 startete nicht: `libsimdjson.so.33` fehlte, installiert war `simdjson` 5.0.1 (`.so.34`). Mit `nodejs` 26.10.0-2 (Stand 2026-10-01) behoben | Bis dahin lokaler Entwicklungs-Workaround über `LD_LIBRARY_PATH`; am System wurde nichts geändert |

## Abweichungen vom Konzept und Folgen

- **Kein Polkit-Agent nachweisbar:** Der Polkit-Dialog konnte nicht real ausgelöst werden, weil
  dafür die Policy systemweit installiert werden müsste (Root). Getestet wurde die
  Autorisierungslogik im Entwicklungsmodus des Helpers (siehe [Testprotokoll](testprotokoll.md)).
- **Keine VM verfügbar:** Echte Paketoperationen wurden ausschließlich in einer isolierten
  pacman-Sandbox mit dem echten pacman-Binary ausgeführt, nie auf dem Arbeitsrechner.
- **App-ID:** Unter Hyprland meldet das Tauri-Fenster die Klasse `cachyos-center` (nicht den
  Tauri-Identifier). Desktop-Datei, Icon-Name und Benachrichtigungen verwenden deshalb
  `cachyos-center`.
- **`pacman-offline` fehlt:** Der Modus „Automatisch beim nächsten Neustart installieren“ ist
  implementiert, konnte aber nicht gegen die echte Offline-Installation getestet werden und bleibt
  „in Entwicklung“ (siehe [Automatische Updates](automatische-updates.md)).
