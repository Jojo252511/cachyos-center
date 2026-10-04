# MCP-Schnittstelle für KI-Assistenten

Stand: v0.1. `cachyos-center-mcp` ist ein lokaler, rein lesender Server für das
[Model Context Protocol](https://modelcontextprotocol.io) (MCP). Er gibt einem KI-Assistenten wie
Claude Code oder Claude Desktop Auskunft über diesen Rechner: Systemübersicht, Ergebnis der letzten
Updateprüfung, Pakete, letzte Paketvorgänge und Gesundheitszustand. Er ändert nichts am System.

## Zweck und Grenzen

- **Lokal über stdio:** Der MCP-Host startet `cachyos-center-mcp` als Kindprozess und spricht über
  stdin/stdout JSON-RPC mit ihm. Es gibt keinen Netzwerk-Listener, keinen HTTP-Transport und keine
  automatische Registrierung in fremden Konfigurationsdateien. stdout trägt ausschließlich das
  Protokoll, Protokollmeldungen gehen nach stderr.
- **Nur lesend:** Genau sechs Tools (`cachyos_center_core::mcp::TOOL_NAMES`), alle mit
  `readOnlyHint: true` und `destructiveHint: false`. Keine Resources, keine Prompts, kein Zugriff
  auf beliebige Dateien. Der Server nutzt ausschließlich `ReadApi` aus `crates/service`; das Crate
  hängt weder von zbus noch vom Helper ab und kann den privilegierten Helper nicht erreichen.
- **Keine Netzwerkzugriffe:** Kein Tool startet eine Updateprüfung, lädt News oder kontaktiert
  andere Rechner. Alle Daten stammen von diesem Rechner (pacman-Datenbanken, pacman.log,
  `/proc`, `/sys`, Einstellungen und Verlauf von cachyos-center).
- **Benutzerrechte:** Der Server läuft mit den Rechten des Nutzers, der den MCP-Host startet.

## Aktivierung

Der Zugriff ist **standardmäßig deaktiviert** (`mcpEnabled = false` in
`$XDG_CONFIG_HOME/cachyos-center/settings.toml`). Aktivieren in cachyos-center unter
**Einstellungen → „KI-Zugriff (MCP)“**.

Solange der Zugriff deaktiviert ist, startet der Server trotzdem, beantwortet `initialize` und
listet seine Tools; jeder Tool-Aufruf liefert aber ein Fehlerergebnis mit Code `UNAVAILABLE` und
der Meldung `MCP access is disabled in cachyos-center (Einstellungen → KI-Zugriff (MCP) / Settings → AI access (MCP))`. Die Einstellung wird
bei jedem Aufruf neu gelesen: Ein- und Ausschalten wirkt sofort, ohne den Host neu zu starten.

## Einrichtung im MCP-Host

Die Registrierung im Host nimmt der Nutzer selbst vor; cachyos-center schreibt in keine
Konfigurationsdatei eines Hosts. Der Einstellungsbereich „KI-Zugriff (MCP)“ zeigt die passende Konfiguration
zum Kopieren an. Sie entspricht der Ausgabe von
`cachyos_center_core::mcp::host_config("/usr/bin/cachyos-center-mcp")`:

```json
{
  "mcpServers": {
    "cachyos-center": {
      "args": [],
      "command": "/usr/bin/cachyos-center-mcp",
      "env": {},
      "type": "stdio"
    }
  }
}
```

### Claude Code

Entweder per Befehl (Gültigkeitsbereich `local`, mit `--scope user` für alle Projekte):

```bash
claude mcp add cachyos-center -- /usr/bin/cachyos-center-mcp
claude mcp list
```

oder als Datei `.mcp.json` im Projektverzeichnis mit dem obigen Inhalt. Claude Code fragt bei
Servern aus einer `.mcp.json` vor der ersten Nutzung nach einer Freigabe.

### Claude Desktop

Den `mcpServers`-Block in die Datei `claude_desktop_config.json` übernehmen (Einstellungen →
Entwickler → Konfiguration bearbeiten) und Claude Desktop neu starten.

### Andere Hosts

Jeder Host, der stdio-Server im `mcpServers`-Format unterstützt, kann den Server so starten.
Umgebungsvariablen sind nicht nötig; `CACHYOS_CENTER_LOG` (z. B. `debug`) steuert die
Ausführlichkeit der Meldungen auf stderr.

## Tools

Alle Tools haben feste JSON-Schemas (erzeugt mit `schemars`, Draft 2020-12), englische
Beschreibungen, die mit „Read-only.“ beginnen, und weisen unbekannte Argumente ab
(`additionalProperties: false`). Optionale Argumente dürfen fehlen oder `null` sein.

| Tool | Eingabe | Ausgabe | Grenzen |
|---|---|---|---|
| `system_get_summary` | – | Betriebssystem, Kernel und Laufzeit, CPU, GPUs, RAM, Belegung von `/`, Sitzung (z. B. `hyprland`), Desktop, pacman-Version, Verfügbarkeit der Paketfunktionen | keine Seriennummern, keine Nutzer- oder Hostnamen |
| `updates_list` | – | Status der letzten Prüfung, `checkedAt`, `ageSeconds`, `stale`, `note`, verfügbare Repository-Updates (Paket, Repository, alte und neue Version, Flags, Downloadgröße), zurückgehaltene Pakete getrennt, Gesamtgröße, Neustartempfehlung, letzter Prüffehler | startet nie eine Prüfung und nie ein Upgrade |
| `packages_search` | `query` (Text, 2–100 Zeichen, Pflicht), `limit` (1–50, Standard 20) | Treffer aus den konfigurierten pacman-Repositories mit Repository, Version, installierter Version, Beschreibung | keine AUR- oder Websuche |
| `packages_installed` | `query` (optional, ≤ 100 Zeichen), `limit` (1–100, Standard 50), `cursor` (optional, opak, ≤ 64 Zeichen) | installierte Pakete mit Version, Herkunft (`repo` oder `localOrAur`), Installationsgrund, Updateverfügbarkeit; `total`, `offset`, `nextCursor` | seitenweise; ein Cursor gilt nur zusammen mit derselben `query` |
| `operations_recent` | `limit` (1–20, Standard 10) | bereinigte Zusammenfassungen der letzten Vorgänge (App, Timer, externe pacman-Läufe): Art, Zustand bzw. Ergebnis, Zeiten, Anzahl geänderter Pakete bzw. bei Updateprüfungen der gefundenen Updates, bis zu 20 Paketnamen, Fehlercode | keine Rohlogs, keine Dateipfade |
| `health_get` | – | Anzahl `.pacnew`/`.pacsave`, Zustand des Datenbank-Locks, Neustartempfehlung mit Gründen, vorbereitetes Offline-Update und Timer, andere Update-Mechanismen (Unit-Namen), Gründe gegen unbeaufsichtigte Updates (`updateBlockers`), eigener Auto-Update-Timer (`autoUpdate`: Richtlinie, Timer aktiv, nächster und letzter Lauf, Zustand des letzten Laufs, vorbereitetes Update), Hinweise mit Art, Schweregrad und kurzem Detail, Snapshot-Unterstützung, Größe des Paketcaches und was `paccache -r` davon freigeben würde | keine Dateiinhalte, keine Dateipfade (insbesondere nicht die Pfade der `.pacnew`-Dateien) |

Konventionen der Ausgaben: Feldnamen in camelCase, Zeitpunkte in Unix-Sekunden (UTC), Größen in
Bytes, Aufzählungen als camelCase-Strings des Kernmodells (z. B. `neverChecked`, `localOrAur`).
Jede Ausgabe enthält `truncated`.

### `updates_list` und veraltete Daten

`updates_list` liefert das Ergebnis der **letzten** Prüfung, die cachyos-center (App oder Timer)
durchgeführt hat. Ein altes Ergebnis wird nie als aktueller Zustand ausgegeben:

| `status` | Bedeutung | `stale` | `note` |
|---|---|---|---|
| `fresh` | erfolgreiche Prüfung, höchstens 6 Stunden alt | `false` | – |
| `stale` | letzte erfolgreiche Prüfung älter als 6 Stunden | `true` | Alter und Hinweis auf mögliche neuere Updates |
| `neverChecked` | noch keine Prüfung; eine leere Liste bedeutet **nicht** „aktuell“ | `true` | Hinweis auf die Prüfung in der App |
| `failed` | letzte Prüfung fehlgeschlagen (`lastError`), Liste stammt ggf. von einer älteren Prüfung | `true` | Fehler und Alter |
| `prerequisiteMissing` | `pacman-contrib` oder `fakeroot` fehlt (`missingPrerequisites`) | `true` | fehlende Pakete |
| `unsupported` | Paketfunktionen deaktiviert (libalpm-Bridge fehlt oder ist inkompatibel) | `true` | Erklärung |

`ageSeconds` ist das Alter der letzten erfolgreichen Prüfung (`null`, wenn es keine gibt).
`updateCount` und `heldBackCount` nennen die vollständige Anzahl auch dann, wenn die Listen wegen
der Größengrenze gekürzt wurden. Ohne erfolgreiche Prüfung sind beide `null` (unbekannt), nie 0.

## Ergebnisformat und Fehler

Jedes Ergebnis enthält die Daten zweifach: als `structuredContent` (JSON) und als Textinhalt mit
demselben JSON, formatiert, für Hosts ohne Unterstützung für strukturierte Inhalte. Das
`outputSchema` jedes Tools beschreibt beide möglichen Formen (`anyOf`): die Erfolgsausgabe und das
Fehlerobjekt.

Fehler eines Tool-Aufrufs sind Tool-Ergebnisse mit `isError: true` und dem strukturierten Inhalt
`{ "code": "...", "message": "..." }`. `message` ist technisches Englisch und bereinigt.

| Code | Bedeutung | Typische Ursachen |
|---|---|---|
| `UNAVAILABLE` | nicht verfügbar | MCP-Zugriff deaktiviert; Zeitüberschreitung nach 15 s („timed out“); Komponente, Paket oder Repository nicht gefunden; Voraussetzung fehlt |
| `BUSY` | Paketverwaltung beschäftigt | fremder `db.lck`, laufender Vorgang, vorbereitetes Offline-Update |
| `STALE` | Daten zu alt | veraltete Repository-Daten |
| `UNSUPPORTED` | nicht unterstützt | libalpm-Bridge fehlt oder passt nicht zur installierten libalpm |
| `INTERNAL` | unerwarteter Fehler | interner Fehler; Ausgabe trotz Kürzung über der Größengrenze |
| `INVALID_INPUT` | ungültige Argumente | `limit` außerhalb der Grenzen, `query` zu kurz/lang oder mit Steuerzeichen, falscher Typ, unbekanntes Feld, fremder oder kaputter `cursor` |

Die ersten fünf Codes sind die stabilen öffentlichen MCP-Codes. Fehler des Lesedienstes werden mit
`ErrorCode::mcp_code()` darauf abgebildet: `UNAVAILABLE`, `BUSY`, `STALE` und `UNSUPPORTED` bleiben
erhalten; `NOT_FOUND`, `NOT_AUTHORIZED`, `OFFLINE`, `PREREQUISITE_MISSING` → `UNAVAILABLE`;
`CONFLICT` → `BUSY`; `PLAN_CHANGED` → `STALE`; `INTERNAL`, `INVALID_INPUT`, `BLOCKED`,
`TRANSACTION_FAILED`, `DEPENDENCY_PROBLEM` → `INTERNAL`. `INVALID_INPUT` als Tool-Fehlercode
entsteht nur bei der Argumentprüfung des MCP-Servers selbst.

Ungültige Argumente meldet der Server bewusst als Tool-Ergebnis mit `isError: true` und
`INVALID_INPUT` und nicht als JSON-RPC-Fehler: So behandelt auch rmcp nicht deserialisierbare
Argumente, und die aktuelle MCP-Spezifikation empfiehlt Eingabefehler als Tool-Fehler, damit das
Modell den Aufruf korrigieren kann. Die Argumente prüft der Server erst nach dem Zugriffsschalter;
bei deaktiviertem Zugriff lautet die Antwort immer `UNAVAILABLE`.

Unbekannte Tool-Namen (z. B. `packages_install`) sind dagegen ein Protokollfehler: JSON-RPC-Fehler
`-32602` („tool not found“).

### Größen- und Zeitgrenzen

- Die serialisierte Ausgabe eines Tools (das formatierte JSON des Textinhalts) ist höchstens
  **64 KiB** groß. Sonst werden Listen von hinten gekürzt und `truncated: true` gesetzt. Bei
  `packages_installed` zeigt `nextCursor` dann auf das erste weggelassene Paket, es geht also nichts
  verloren.
- Einzelne Texte über 4096 Zeichen werden mit „…“ abgeschnitten (`truncated: true`).
- Jeder Aufruf liest das System auf einem eigenen Thread (`spawn_blocking`) und hat ein Zeitlimit
  von **15 Sekunden**; danach antwortet der Server mit `UNAVAILABLE` („timed out“). Höchstens vier
  Aufrufe lesen gleichzeitig.

## Datenschutz

**Lesezugriff ist nicht automatisch harmlos: Paketnamen und Hardware können private Details
verraten.** Installierte Pakete und letzte Vorgänge zeigen zum Beispiel Arbeitsgebiete, genutzte
Dienste oder Software aus dem AUR; die Hardware-Ausstattung kann einen Rechner wiedererkennbar
machen. Alles, was ein Tool liefert, gibt der MCP-Host an den KI-Dienst weiter. Deshalb ist der
Zugriff standardmäßig aus, und er sollte nur für Hosts aktiviert werden, denen man diese Daten
anvertraut.

Schutzmaßnahmen des Servers:

- Jede Zeichenkette jeder Ausgabe läuft durch `cachyos_center_core::sanitize::sanitize` mit
  `SanitizeContext::from_env()`: Home-Verzeichnis → `~`, andere `/home/<name>` → `/home/<user>`,
  Nutzername → `<user>`, Hostname → `<host>`, E-Mail-, IP- und MAC-Adressen sowie
  token-ähnliche Werte → Platzhalter.
- Absolute Pfade (`/…`, `~/…`) ersetzt der Server zusätzlich durch `<path>`: in allen
  Fehlermeldungen und in den Ausgaben aller Tools außer `packages_search` und
  `packages_installed`, deren Paketbeschreibungen nur bereinigt werden.
- `health_get` gibt die Pfade der `.pacnew`/`.pacsave`-Dateien nicht aus, nur deren Anzahl.
- `operations_recent` enthält keine Rohlogs und keine pacman-Kommandozeilen, nur Zählungen,
  Zustände und Paketnamen.
- `system_get_summary` enthält keine Seriennummern, Nutzer- oder Hostnamen.
- Auf stderr protokolliert der Server je Aufruf nur Tool-Name, Dauer und gegebenenfalls den
  Fehlercode, nie Argumente oder Ergebnisse.

Die Bereinigung ersetzt auch Treffer in Paket- oder Repository-Namen, wenn diese zufällig genau
dem Nutzer- oder Hostnamen entsprechen.

## Keine schreibenden Tools in V1

Version 1 enthält bewusst kein Tool, das installiert, aktualisiert, entfernt, eine Updateprüfung
startet oder Einstellungen ändert. Gründe:

- Ein KI-Host verarbeitet fremde Inhalte (Webseiten, Dateien, Paketbeschreibungen). Über Prompt
  Injection könnte ein schreibendes Tool ohne Wissen des Nutzers ausgelöst werden.
- Die Freigabe im MCP-Host ersetzt keine Bestätigung am Rechner: Der Host zeigt weder den
  vollständigen Transaktionsplan noch die Sicherheitsprüfungen von cachyos-center (News,
  Snapshot, Planabweichung, Lock).
- Die Rechtegrenze bleibt einfach: Der MCP-Server hat keine D-Bus-Abhängigkeit und kann den
  privilegierten Helper nicht erreichen.

Ein späteres Tool wie `request_system_upgrade` bräuchte ein eigenes Berechtigungsmodell: Es dürfte
nur eine Anfrage stellen, die der Nutzer lokal in der Oberfläche von cachyos-center mit dem
konkreten Plan bestätigt (anschließend Polkit-Authentifizierung wie bei jedem Upgrade), mit
eigenem, standardmäßig deaktiviertem Schalter. Das ist nicht Teil von V1.

## Testen

```bash
# libalpm-Bridge neben die Binärdateien bauen (für Paketdaten in den Tests)
cargo build -p cachyos-center-alpm-bridge

# Unit-Tests (FakeReadApi) und Host-Test (startet den Server als Kindprozess)
cargo test -p cachyos-center-mcp

# wie in CI: Host-Test schlägt fehl, wenn die Bridge fehlt
CC_REQUIRE_BRIDGE=1 cargo test -p cachyos-center-mcp --test host

cargo fmt -p cachyos-center-mcp --check
cargo clippy -p cachyos-center-mcp --all-targets -- -D warnings
```

Die Unit-Tests prüfen mit einer `FakeReadApi` jeden Tool-Aufruf, den deaktivierten Zugriff
(`UNAVAILABLE`, ohne Datenzugriff), das erneute Lesen der Einstellung, Argumentgrenzen,
Fehlerabbildung, Zeitüberschreitung, Kürzung auf 64 KiB, die Kennzeichnung veralteter
Updatedaten, Pfadfreiheit von `health_get` und die Bereinigung. `crates/mcp/tests/host.rs` verhält
sich wie ein MCP-Host (rmcp-Client mit Kindprozess-Transport): `initialize`, `tools/list` (genau
`TOOL_NAMES`), Aufrufe aller Tools mit echten Systemdaten, deaktivierter Zugriff, Umschalten
ohne Neustart, Ende bei EOF auf stdin, `--version` und `--help`. Einstellungen, Verlauf und
Prüfstatus liegen dabei in temporären `XDG_*`-Verzeichnissen.

Schnelltest von Hand (`mcpEnabled = true` vorausgesetzt, sonst antwortet das Tool mit
`UNAVAILABLE`):

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"system_get_summary","arguments":{}}}' \
  | target/debug/cachyos-center-mcp
```

Prüfung mit einem unabhängigen Host, dem offiziellen MCP Inspector (Node.js):

```bash
npx @modelcontextprotocol/inspector --cli target/debug/cachyos-center-mcp --method tools/list
# Tool-Aufruf; mcp.json enthält die obige mcpServers-Konfiguration mit dem Pfad zur Binärdatei
npx @modelcontextprotocol/inspector --cli --config mcp.json --server cachyos-center \
  --method tools/call --tool-name packages_search --tool-arg query=hyprland --tool-arg limit=2
```
