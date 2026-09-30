# Tests

| Ort | Inhalt |
|---|---|
| `crates/*/src` (`#[cfg(test)]`) | Unit-Tests |
| `crates/packages/tests/system_readonly.rs` | lesende Tests gegen die pacman-Datenbanken des Testrechners |
| `crates/service/tests/core_readonly.rs` | Application Core auf dem Testrechner (Dashboard, Gesundheit, Diagnosebericht bereinigt) |
| `crates/helper/tests/sandbox.rs` | Helper mit echtem pacman in einer isolierten fakeroot-Sandbox auf privatem D-Bus |
| `crates/helper/tests/recovery.rs` | Wiederherstellung nach Helper-Absturz |
| `crates/core/tests/packaging.rs` | Konsistenz zwischen Code und Paketierung |
| `crates/mcp/tests/host.rs` | MCP-Server gegen einen MCP-Host (rmcp-Client) |
| `apps/desktop/src/**/*.test.tsx` | Oberfläche: kritische Dialoge und Zustände |
| [`vm/`](vm/) | Prüfprotokoll und Skript für den End-to-End-Test auf einer CachyOS-VM |

Befehle und Ergebnisse: [docs/testprotokoll.md](../docs/testprotokoll.md).
