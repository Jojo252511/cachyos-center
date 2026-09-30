#!/usr/bin/env bash
# Lesende Installations- und Integrationsprüfung von cachyos-center.
# Ändert keine Pakete. Für die Test-VM gedacht (Abschnitt „Vorbereitung“ in README.md).
set -u
LOG="vm-pruefung-$(date +%Y%m%d-%H%M%S).log"
pass=0; fail=0
check() {
  local name="$1"; shift
  if "$@" >>"$LOG" 2>&1; then
    printf 'OK    %s\n' "$name" | tee -a "$LOG"; pass=$((pass + 1))
  else
    printf 'FEHLER %s\n' "$name" | tee -a "$LOG"; fail=$((fail + 1))
  fi
}

{
  echo "== Umgebung =="
  grep -E '^(ID|PRETTY_NAME|BUILD_ID)=' /etc/os-release
  uname -r
  pacman -Q pacman cachyos-center 2>&1
  echo "XDG_CURRENT_DESKTOP=${XDG_CURRENT_DESKTOP:-}"
} >>"$LOG" 2>&1

check "Binärdateien installiert" test -x /usr/bin/cachyos-center -a -x /usr/bin/cachyos-center-mcp -a -x /usr/lib/cachyos-center/cachyos-center-helper
check "libalpm-Bridge installiert" test -f /usr/lib/cachyos-center/libcachyos_center_alpm.so
check "Bridge findet libalpm" bash -c "ldd /usr/lib/cachyos-center/libcachyos_center_alpm.so | grep -q 'libalpm.so'"
check "Programme linken libalpm nicht direkt" bash -c "! ldd /usr/bin/cachyos-center /usr/bin/cachyos-center-mcp /usr/lib/cachyos-center/cachyos-center-helper | grep -q libalpm"
check "D-Bus-Aktivierung registriert" bash -c "busctl --system list --activatable | grep -q org.cachyos_center.Packages1"
for a in org.cachyos-center.packages.upgrade org.cachyos-center.packages.install org.cachyos-center.packages.remove org.cachyos-center.autoupdate.configure; do
  check "Polkit-Aktion $a" pkaction --action-id "$a"
done
check "Keine Polkit-Regel für cachyos-center" bash -c "! grep -rl cachyos /etc/polkit-1/rules.d /usr/share/polkit-1/rules.d 2>/dev/null"
check "Helper-Unit bekannt" systemctl cat cachyos-center-helper.service
check "Timer-Unit bekannt" systemctl cat cachyos-center-preflight.timer
check "Benachrichtigungs-Unit bekannt" systemctl --user cat cachyos-center-notify.path
check "Helper antwortet (Version)" busctl --system get-property org.cachyos_center.Packages1 /org/cachyos_center/Packages1 org.cachyos_center.Packages1 Version
check "Lesezugriff ohne Autorisierung (CurrentOperation)" busctl --system call org.cachyos_center.Packages1 /org/cachyos_center/Packages1 org.cachyos_center.Packages1 CurrentOperation
check "Ungültige Eingabe wird abgelehnt" bash -c "! busctl --system call -- org.cachyos_center.Packages1 /org/cachyos_center/Packages1 org.cachyos_center.Packages1 InstallRepoPackage sss core --overwrite=x 0000 2>&1 | grep -q '^s '"
check "MCP tools/list ohne schreibende Tools" bash -c '
  printf "%s\n%s\n%s\n" \
    "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"vm-check\",\"version\":\"1\"}}}" \
    "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}" \
    "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}" \
  | timeout 20 /usr/bin/cachyos-center-mcp 2>/dev/null | tee /dev/stderr | grep -q "system_get_summary" && \
  ! (printf "%s\n%s\n%s\n" \
    "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"vm-check\",\"version\":\"1\"}}}" \
    "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}" \
    "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}" \
  | timeout 20 /usr/bin/cachyos-center-mcp 2>/dev/null | grep -E -q "\"name\":\"([a-z]+_)*(install|upgrade|remove|update|apply|exec)(_[a-z]+)*\"")'
check "GUI startet (10 s)" bash -c "timeout 10 cachyos-center; test \$? -eq 124"

echo "Ergebnis: $pass OK, $fail FEHLER (Details: $LOG)" | tee -a "$LOG"
test "$fail" -eq 0
