# IPC-Vertrag zwischen Oberfläche und Tauri-Backend

Stand: v0.1. Dieser Vertrag beschreibt alle Tauri-Commands und Events, die das React-Frontend
(`apps/desktop/src`) verwenden darf. Die Datentypen werden aus den Rust-Modellen erzeugt
(`apps/desktop/src/bindings/*.ts`, Quelle `crates/core`); sie dürfen nicht von Hand geändert werden.

## Konventionen

- Aufruf mit `invoke<T>(command, args)` aus `@tauri-apps/api/core`.
- Argumentnamen sind **camelCase** (Tauri wandelt Rust-`snake_case` automatisch um).
- Zeitpunkte sind Unix-Sekunden (`number`), Größen Bytes (`number`).
- Fehler: Das Promise wird mit einem Objekt vom Typ `AppError` abgelehnt
  (`{ code: ErrorCode, message: string, detail: string | null }`). `message` ist technisches
  Englisch; die Oberfläche zeigt einen lokalisierten Text zum `code` und bietet `message`/`detail`
  als Details an.
- Kein Command führt freie Befehle aus. Paketänderungen laufen ausschließlich über den
  privilegierten Helper (D-Bus `org.cachyos_center.Packages1`) mit Polkit.

## Lesende Commands

| Command | Argumente | Rückgabe | Hinweise |
|---|---|---|---|
| `get_app_info` | – | `AppInfo` | Version, App-ID, Systemsprache, Farbschema, Helper-Verfügbarkeit, Backend-Status |
| `get_dashboard` | – | `Dashboard` | Startseite; ohne Netzwerkzugriff |
| `get_system_info` | – | `SystemInfo` | |
| `get_hyprland_info` | – | `HyprlandInfo` | `available=false` + `reason` ohne Hyprland-Sitzung |
| `get_updates` | – | `UpdateCheckResult` | letzte Prüfung, **kein** Netzwerkzugriff; `status` beachten (`neverChecked` ≠ 0 Updates) |
| `check_updates` | – | `UpdateCheckResult` | isolierte Prüfung mit `checkupdates` (Netzwerk, kann bis 3 min dauern) |
| `list_installed` | `query: InstalledQuery` | `PackagePage` | `limit` 1–500 (0 = 100), `offset` für Paginierung |
| `search_packages` | `query: CatalogQuery` | `PackageSummary[]` | nur konfigurierte Repositories, `query` ≥ 2 Zeichen (ohne Repo-Filter), `limit` 1–500 |
| `get_package_details` | `reference: PackageRef` | `PackageRecord` | |
| `list_repositories` | – | `RepositoryInfo[]` | |
| `get_health` | – | `HealthReport` | |
| `get_news` | `refresh: boolean, force: boolean` | `NewsStatus` | `refresh` lädt Feeds, wenn der Cache älter als 1 h ist |
| `acknowledge_news` | `until: number` | `NewsStatus` | markiert News bis Zeitpunkt als gelesen |
| `get_activity` | `limit: number` | `HistoryEntry[]` | 1–200 |
| `get_settings` | – | `Settings` | |
| `save_settings` | `settings: Settings` | `Settings` | validiert; ungültige Werte → `INVALID_INPUT` |
| `get_auto_update_status` | – | `AutoUpdateStatus` | |
| `get_diagnostic_report` | – | `string` | bereinigter Bericht (Englisch) zur **Vorschau**; Kopieren erst nach Nutzeraktion |
| `get_mcp_setup` | – | `McpSetup` | Host-Konfiguration zum Kopieren |

## Planungs-Commands (ohne Änderungen, ohne Root)

| Command | Argumente | Rückgabe | Hinweise |
|---|---|---|---|
| `plan_install` | `repository: string, name: string` | `TransactionPlan` | benötigt Repository-Daten ≤ 1 h alt, sonst `STALE` → Oberfläche bietet „Jetzt aktualisieren“ (`check_updates`) an |
| `plan_remove` | `name: string, recursive: boolean` | `TransactionPlan` | `DEPENDENCY_PROBLEM` wenn andere Pakete abhängen (`detail` nennt sie) |

Der Upgrade-Plan steht in `UpdateCheckResult.plan`.

## Systemändernde Commands (Helper, Polkit)

| Command | Argumente | Rückgabe | Hinweise |
|---|---|---|---|
| `start_upgrade` | `planDigest: string, createSnapshot: boolean` | `string` (Operation-ID) | vollständiges `pacman -Syu`, Digest aus `UpdateCheckResult.plan.digest` |
| `start_install` | `repository: string, name: string, planDigest: string` | `string` | `pacman -Syu --needed repo/name` |
| `start_remove` | `name: string, recursive: boolean, planDigest: string` | `string` | nur Repository-Pakete; `pacman -R` bzw. `-Rs` |
| `set_auto_update_policy` | `config: AutoUpdateConfig` | `AutoUpdateStatus` | Polkit-Aktion `org.cachyos-center.autoupdate.configure` |
| `cancel_operation` | `id: string` | `Operation` | nur vor dem Commit (`OperationState.canCancel`), sonst `INVALID_INPUT` |

Fehler dieser Commands: `UNAVAILABLE` (Helper nicht installiert/erreichbar), `BUSY` (anderer Vorgang
oder fremder `db.lck`), `CONFLICT` (vorbereitetes Offline-Update), `INVALID_INPUT`, `NOT_AUTHORIZED`
(Polkit abgelehnt/abgebrochen).

## Vorgangsstatus

| Command | Argumente | Rückgabe | Hinweise |
|---|---|---|---|
| `get_operation` | `id: string` | `Operation` | Polling alle 750 ms, solange `state` aktiv ist |
| `get_operation_log` | `id: string, offset: number` | `OperationLogChunk` | inkrementell, `nextOffset` für den nächsten Aufruf |
| `get_current_operation` | – | `Operation \| null` | laufender oder zuletzt beendeter Vorgang des Helpers (Rekonstruktion nach GUI-Neustart) |

Zustände (`OperationState`): `awaitingAuthorization` (Polkit-Dialog offen), `preparing`
(Lock, Refresh, Planprüfung), `downloading` (Pakete laden und Signaturen prüfen, abbrechbar),
`installing` (Commit, **nicht** abbrechbar: „Installation läuft; Abbruch möglicherweise
gefährlich“), Endzustände `succeeded`, `failed` (vor Commit: Paketlage unverändert),
`needsAttention` (Commit begonnen, Ergebnis unklar oder Handlungsbedarf),
`cancelledBeforeCommit`.

Weicht der tatsächliche Plan vor dem Commit vom bestätigten ab, endet der Vorgang mit
`cancelledBeforeCommit`, `error.code = PLAN_CHANGED` und `actualPlan` gesetzt. Die Oberfläche zeigt
die Abweichung und startet nach erneuter Bestätigung einen neuen Vorgang mit `actualPlan.digest`.

## Weitere Aktionen

| Command | Argumente | Rückgabe | Hinweise |
|---|---|---|---|
| `open_external` | `url: string` | `void` | nur `https://` auf freigegebenen Hosts (archlinux.org, wiki.archlinux.org, cachyos.org, wiki.cachyos.org, github.com/Jojo252511/cachyos-center, man.archlinux.org); nur nach Nutzeraktion |
| `reveal_config_file` | `path: string` | `void` | nur `.pacnew`/`.pacsave` unter `/etc` aus `HealthReport.configFiles` |
| `write_clipboard` | `text: string` | `void` | nur nach Nutzeraktion (Diagnose, Host-Konfiguration) |

## Events

| Event | Payload | Bedeutung |
|---|---|---|
| `hyprland-changed` | – | Monitor/Workspace geändert (Hyprland-Event-Socket), `get_hyprland_info` neu laden |
| `updates-checked` | `UpdateCheckResult` | Hintergrundprüfung (Einstellung „Prüfintervall“) abgeschlossen |
| `operation-finished` | `Operation` | ein Helper-Vorgang wurde beendet (Benachrichtigung wurde ggf. gesendet) |
