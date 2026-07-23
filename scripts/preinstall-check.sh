#!/usr/bin/env bash
set -euo pipefail

: "${CLSC_HTTP_PORT:=8080}"
: "${CLSC_POSTGRES_PORT:=5432}"
: "${CLSC_BACKUP_DIR:=./backups}"

fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "OK: $*"; }

check_port() {
  local port=$1
  if ss -tlnp 2>/dev/null | awk -F':' '{print $NF}' | awk '{print $1}' | grep -qx "$port"; then
    fail "port $port is already in use"
  fi
  ok "port $port is free"
}

check_disk() {
  local avail
  avail=$(df -BG . | tail -1 | awk '{print $4}' | tr -d 'G')
  if [[ "$avail" -lt 10 ]]; then
    fail "less than 10 GB of disk space available"
  fi
  ok "disk space sufficient (${avail}G)"
}

check_clock() {
  if ! command -v chronyd &>/dev/null && ! command -v ntpd &>/dev/null && ! timedatectl show 2>/dev/null | grep -q 'NTP=yes'; then
    fail "no NTP sync service detected"
  fi
  ok "clock sync service present"
}

check_postgres_client() {
  if ! command -v psql &>/dev/null; then
    fail "psql not found"
  fi
  ok "psql available"
}

check_backup_dir() {
  if [[ ! -d "$CLSC_BACKUP_DIR" ]]; then
    mkdir -p "$CLSC_BACKUP_DIR" || fail "cannot create backup directory $CLSC_BACKUP_DIR"
  fi
  ok "backup directory $CLSC_BACKUP_DIR exists"
}

check_certs() {
  local cert_dir="${CLSC_CERT_DIR:-./certs}"
  if [[ -d "$cert_dir" ]]; then
    if [[ ! -f "$cert_dir/tls.crt" || ! -f "$cert_dir/tls.key" ]]; then
      fail "cert directory exists but is missing tls.crt/tls.key"
    fi
    ok "TLS certificates present in $cert_dir"
  else
    ok "no certificate directory configured, using plaintext"
  fi
}

echo "=== Cloud Leopard Secure Center pre-install check ==="
check_port "$CLSC_HTTP_PORT"
check_port "$CLSC_POSTGRES_PORT"
check_disk
check_clock
check_postgres_client
check_backup_dir
check_certs
echo "=== All pre-install checks passed ==="
