# Bedienung

cachyos-center ist in sechs Bereiche gegliedert, erreichbar über die Seitenleiste: **Übersicht**,
**Updates**, **Software**, **System**, **Aktivität** und **Einstellungen**. Auf schmalen Fenstern
klappt die Seitenleiste ein. Alle Funktionen sind per Tastatur bedienbar.

## Übersicht

Die Startseite zeigt oben den Gesamtstatus, zum Beispiel „System aktuell“, „12 Updates verfügbar“,
„Prüfung fehlgeschlagen“, „Installation läuft“ oder „Noch nicht geprüft“. „0 Updates“ erscheint nur
nach einer frischen, erfolgreichen Prüfung; ist die letzte Prüfung älter als sechs Stunden oder ist
danach ein Vorgang fehlgeschlagen, gilt der Stand als veraltet.

Darunter stehen vier Karten:

- **Updates:** Anzahl, Zeitpunkt der letzten Prüfung, Neustartempfehlung
- **System:** Kernel, Hardware, freier Speicher
- **Software:** installierte Pakete, davon lokal/AUR
- **Gesundheit:** wichtigste Hinweise aus dem Gesundheitszentrum

Außerdem: letztes vollständiges Update, nächste automatische Prüfung und die letzte Aktivität.
Ist ein Update für den nächsten Neustart vorbereitet, weist ein Hinweis darauf hin.

## Updates

1. **Jetzt prüfen** synchronisiert eine Kopie der Paketdatenbanken und vergleicht sie mit dem
   System. Die echte Paketdatenbank wird dabei nicht verändert.
2. Die Liste zeigt je Paket Repository, installierte und neue Version, Downloadgröße und
   Kennzeichen (Kernel, Treiber, Firmware, Microcode, Systemkern, Paketverwaltung,
   Desktop-Sitzung, Neustart empfohlen). Die Liste ist eine Erklärung, keine Auswahl: installiert
   wird immer das vollständige Systemupgrade. Von der pacman-Konfiguration zurückgehaltene Pakete
   (`IgnorePkg`) stehen getrennt; das Upgrade ist dann nicht vollständig.
3. **Vor dem Upgrade** zeigt Netzwerk, freien Speicher, Paketmanager-Lock, Spiegel- und
   Signaturfehler, Arch-/CachyOS-News, andere Update-Dienste und den Snapshot-Status. Was nicht
   sicher geprüft werden kann, steht als „unbekannt“ da.
4. **Installieren** öffnet die Bestätigung mit Paketanzahl, Downloadmenge, Repositories und
   möglichen Folgen. Bei ungelesenen oder nicht abrufbaren News ist eine ausdrückliche Bestätigung
   nötig. Die Authentifizierung übernimmt der Polkit-Dialog der Sitzung.
5. Die Live-Ansicht zeigt die Phasen „Authentifizierung“, „Vorbereitung“, „Download“,
   „Installation“ und „Abschluss“, das aktuelle Paket, „n von N Paketen“ (nur wenn pacman die
   Gesamtzahl liefert) und das Protokoll. Abbrechen ist nur bis zum Ende des Downloads möglich;
   während der Installation erscheint „Installation läuft; Abbruch möglicherweise gefährlich“.
6. Das Ergebnis nennt die Änderungen, neue `.pacnew`-Dateien und eine Neustartempfehlung. Bei
   einem Fehler vor der Installation bleibt die Paketlage unverändert; nach Beginn der Installation
   gilt sie als unklar und muss geprüft werden.

Hat sich der Plan seit der Bestätigung geändert (z. B. neue Version auf dem Spiegel), bricht der
Vorgang vor der Installation ab und zeigt die Unterschiede zur erneuten Bestätigung.

## Software

**Installiert:** Suche nach Name oder Beschreibung, Filter „Explizit installiert“, „Abhängigkeit“,
„Repository“, „Lokal/AUR“ und „Update verfügbar“. Die Details zeigen Version, Quelle, Größe,
Installationsgrund, Beschreibung, Homepage und Abhängigkeiten. Pakete ohne Repository heißen
„Lokal/AUR (Quelle nicht sicher bestimmbar)“; sie werden in V1 weder aktualisiert noch über die App
entfernt.

**Entfernen** (Repository-Pakete) zeigt alle Pakete, die pacman entfernen würde. Optional werden
nicht mehr benötigte Abhängigkeiten mit entfernt. Wird das Paket von anderen benötigt, ist das
Entfernen blockiert. Systemkritische Pakete erfordern eine zusätzliche Bestätigung. Geänderte
Konfigurationsdateien sichert pacman als `.pacsave`.

**Katalog:** Suche in den konfigurierten pacman-Repositories mit Filter nach Repository und
Installationsstatus. **Installieren** führt eine konsistente Transaktion `pacman -Syu repo/paket`
aus; dabei können auch andere Systempakete aktualisiert werden. Sind die Repository-Daten älter als
eine Stunde, bietet die App zuerst „Jetzt aktualisieren“ an.

## System

Betriebssystem, Kernel, Laufzeit, CPU, GPU, Arbeitsspeicher, Datenträger, Sitzung sowie pacman und
libalpm. In einer Hyprland-Sitzung zusätzlich Version, Monitore, aktiver Workspace und die
Fensterklasse `cachyos-center` für eigene Fensterregeln.

Das **Gesundheitszentrum** listet `.pacnew`/`.pacsave`-Dateien (manuell zusammenführen, z. B. mit
`pacdiff`), Neustartempfehlungen, Paketcache, Snapshot-Unterstützung, andere Update-Dienste und
Links zu den offiziellen CachyOS- und Arch-Hinweisen.

**Diagnose kopieren** erzeugt einen bereinigten Bericht (ohne Benutzername, Hostname, Home-Pfade
und Adressen), zeigt ihn zur Kontrolle an und kopiert ihn erst nach Klick auf „Kopieren“.

## Aktivität

Alle Vorgänge der App und des Timers sowie pacman-Transaktionen außerhalb der App mit Zeitpunkt,
Ergebnis und Paketänderungen. Ergebnisse, die nicht sicher rekonstruiert werden konnten, stehen als
„Ergebnis unbekannt“ da.

## Einstellungen

Änderungen an Darstellung, Benachrichtigungen, Updateprüfung, Log-Aufbewahrung, News und MCP
werden sofort gespeichert.

- **Darstellung:** Farbschema (System, Dunkel, Hell), Sprache (System, Deutsch, English), Dichte
  (Komfortabel, Kompakt)
- **Benachrichtigungen** für verfügbare Updates, Ergebnisse, Fehler und Handlungsbedarf
- **Updateprüfung:** Prüfintervall, solange die App läuft (Aus, jede Stunde, alle 3, 6, 12 oder
  24 Stunden)
- **Automatische Updates:** Modus „Aus“, „Nur benachrichtigen“ oder „Automatisch beim nächsten
  Neustart installieren“ (in Entwicklung, gesperrt) mit Wochentagen, Uhrzeit und optional
  verlangtem Snapshot; „Übernehmen“ fragt per Polkit nach der Berechtigung. Darunter stehen Timer,
  nächster und letzter Lauf, „Zum nächsten Neustart vorbereitet“ und andere Update-Dienste
  („extern verwaltet“). Mit „Aktuelle News als gelesen bestätigen“ gibt man die News für die
  geplante Vorbereitung frei. Details: [Automatische Updates](automatische-updates.md)
- **Log-Aufbewahrung** von Verlauf und Vorgangsprotokollen
- **News:** Abruf der Arch-/CachyOS-News abschaltbar (ohne News ist der News-Check vor Upgrades
  nicht möglich)
- **KI-Zugriff (MCP):** lokalen MCP-Server erlauben, bereitgestellte Daten und Werkzeuge ansehen,
  Host-Konfiguration kopieren; siehe [MCP](mcp.md)
