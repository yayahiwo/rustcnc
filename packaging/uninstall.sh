#!/usr/bin/env bash
set -euo pipefail

# Uninstall RustCNC installed by packaging/install.sh.
#
# Usage:
#   sudo ./uninstall.sh
#   sudo RUSTCNC_PREFIX=/opt/rustcnc RUSTCNC_SERVICE_NAME=rustcnc ./uninstall.sh
#
# Note: This does not remove the service user by default.

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo "ERROR: run as root (try: sudo ./uninstall.sh)" >&2
  exit 1
fi

PREFIX="${RUSTCNC_PREFIX:-/opt/rustcnc}"
SERVICE_NAME="${RUSTCNC_SERVICE_NAME:-rustcnc}"
UNIT_PATH="/etc/systemd/system/${SERVICE_NAME}.service"

if command -v systemctl >/dev/null 2>&1; then
  systemctl disable --now "$SERVICE_NAME.service" >/dev/null 2>&1 || true
  rm -f "$UNIT_PATH"
  systemctl daemon-reload || true
fi

rm -rf "$PREFIX"

echo "==> Uninstalled (kept user/group as-is)"

