# Entwicklung

## Voraussetzungen

`rust` (≥ 1.89, Edition 2024), `nodejs`, `npm`, `webkit2gtk-4.1`, `libsoup3`, `gtk3`, `pkgconf`,
`pacman-contrib`, `fakeroot`, `dbus` sowie für die Sandbox-Tests `base-devel` (makepkg).

## Bauen

```bash
# Frontend (wird vom Tauri-Crate eingebettet)
cd apps/desktop && npm ci && npm run build && cd ../..

# Alle Crates inkl. libalpm-Bridge (target/debug/libcachyos_center_alpm.so)
cargo build --workspace
```

Die GUI im Entwicklungsmodus mit Hot-Reload:

```bash
cd apps/desktop && npm run tauri dev
```

Die Oberfläche läuft auch im Browser mit Mock-Daten (`npm run dev`, dann
`http://localhost:1420/?scenario=default`; weitere Szenarien siehe `apps/desktop/src/api/mock.ts`).

## Tests

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
(cd apps/desktop && npm run typecheck && npm run lint && npm test && npm run build)
```

| Testart | Ort | Hinweis |
|---|---|---|
| Unit-Tests | alle Crates | Zustandsmaschine, Validierung, Parser, Plan-Digest, Sanitizer, feste pacman-Argumente |
| System lesend | `crates/packages/tests/system_readonly.rs`, `crates/service/tests/core_readonly.rs` | lesen die echten pacman-Datenbanken; Pläne ohne Lock; prüfen, dass kein `db.lck` entsteht |
| Netzwerk | `cargo test -p cachyos-center-packages -- --ignored`, `cargo test -p cachyos-center-system -- --ignored` | isolierte Updateprüfung und News-Feeds; die produktive Sync-Datenbank bleibt unverändert |
| pacman-Sandbox | `crates/helper/tests/sandbox.rs` | Helper + **echtes pacman** unter `fakeroot` in einer isolierten Sandbox (eigene Root, DB, Cache, Log, Keyring, `file://`-Repository mit makepkg-Fixtures) auf privatem `dbus-daemon` |
| Wiederherstellung | `crates/helper/tests/recovery.rs` | Rekonstruktion nach Helper-Absturz aus Journal und pacman.log |
| Paketierung | `crates/core/tests/packaging.rs` | Namen und Pfade in Code und `packaging/arch` stimmen überein |
| MCP-Host | `crates/mcp/tests/host.rs` | startet den MCP-Server als Kindprozess und prüft `tools/list` und Tool-Aufrufe |
| Oberfläche | `apps/desktop/src/**/*.test.tsx` | kritische Dialoge (Upgrade, Installation, Entfernen, Planabweichung, Fortschritt, Auto-Update, Diagnose) |

Fehlende Werkzeuge überspringen die System-/Sandbox-Tests mit Hinweis. In CI erzwingen
`CC_REQUIRE_BRIDGE=1` und `CC_REQUIRE_SANDBOX=1` ihre Ausführung.

## Helper im Entwicklungsmodus

Der Helper kann unprivilegiert auf dem Session-Bus gegen eine pacman-Sandbox laufen (nie als root):

```bash
target/debug/cachyos-center-helper daemon --dev-root /tmp/cc-dev \
  --pacman-conf /tmp/cc-sandbox/pacman.conf --pacman-log /tmp/cc-sandbox/pacman.log --fakeroot
```

Eine Debug-Build der Oberfläche nutzt ihn mit `CACHYOS_CENTER_HELPER_BUS=session`.

Weitere Schalter des Entwicklungsmodus (nur zusammen mit `--dev-root`, nie als root):

| Schalter | Wirkung |
|---|---|
| `--pacman PATH` | anderes pacman-Programm (z. B. Testattrappe) |
| `--deny ACTION` | Test-Autorisierung: diese Polkit-Aktion ablehnen, alle anderen erlauben |
| `--polkit` | statt der Test-Autorisierung die Polkit-Authority auf dem Entwicklungsbus fragen (die Sandbox-Tests registrieren dort eine Test-Authority) |
| `--auth-timeout-secs N` | Zeitlimit einer Polkit-Prüfung |
| `--lock-wait-secs N` | maximale Wartezeit auf einen fremden pacman-Lock |
| `--idle-secs N` | Beenden nach N Sekunden ohne Vorgang |
Die Sandbox-Einrichtung zeigt `crates/helper/tests/sandbox.rs`.

## TypeScript-Typen

Die Typen in `apps/desktop/src/bindings` werden aus `crates/core` erzeugt:

```bash
cargo test -p cachyos-center-core
```

CI schlägt fehl, wenn die erzeugten Dateien nicht committet sind.

## Release-Gates

`cargo fmt`, `cargo clippy -D warnings`, `cargo test`, TypeScript-Typecheck, Lint, UI-Tests,
Frontend-Build, `cargo deny` (Advisories, Lizenzen, Quellen), `npm audit --audit-level=high`,
Secret-Scan der gesamten Git-Historie mit `gitleaks`,
Paketbau mit makepkg und ein manueller End-to-End-Lauf auf einer VM bzw. echtem
CachyOS-Hyprland-System (siehe [Testprotokoll](testprotokoll.md)). Keine Freigabe nur aufgrund
grüner Mock-Tests.
