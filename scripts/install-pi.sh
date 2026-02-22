#!/usr/bin/env bash
set -euo pipefail

# Install RustCNC on a Raspberry Pi over SSH.
#
# This script:
#   1) Detects the Pi architecture (aarch64 vs armv7l)
#   2) Builds + packages the matching release tarball
#   3) Copies it to the Pi and runs the on-Pi installer (`install.sh`)
#
# Usage: ./install-pi.sh [options] [user@host]
#
# Examples:
#   ./install-pi.sh pi@raspberrypi.local
#   ./install-pi.sh --prefix /opt/rustcnc user@<pi-ip>

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

usage() {
  cat <<EOF
Usage: ./install-pi.sh [options] [user@host]

Options:
  --prefix PATH         Install prefix on the Pi (default: /opt/rustcnc)
  --user NAME           Service user on the Pi (default: rustcnc)
  --group NAME          Service group on the Pi (default: same as user)
  --service-name NAME   systemd service name (default: rustcnc)
  --yes                 Non-interactive on the Pi installer
  -h, --help            Show this help
EOF
}

PI_HOST="pi@raspberrypi.local"
PREFIX="/opt/rustcnc"
USER_NAME="rustcnc"
GROUP_NAME=""
SERVICE_NAME="rustcnc"
ASSUME_YES=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="${2:-}"; shift 2;;
    --user)
      USER_NAME="${2:-}"; shift 2;;
    --group)
      GROUP_NAME="${2:-}"; shift 2;;
    --service-name)
      SERVICE_NAME="${2:-}"; shift 2;;
    --yes)
      ASSUME_YES=1; shift;;
    -h|--help)
      usage; exit 0;;
    *)
      PI_HOST="$1"; shift;;
  esac
done

if [[ -z "$GROUP_NAME" ]]; then
  GROUP_NAME="$USER_NAME"
fi

confirm() {
  local prompt="$1"
  if [[ "$ASSUME_YES" -eq 1 ]]; then
    return 0
  fi
  local ans
  read -r -p "${prompt} [y/N] " ans
  case "${ans,,}" in
    y|yes) return 0;;
    *) return 1;;
  esac
}

echo "==> Detecting Pi architecture ($PI_HOST)"
REMOTE_ARCH="$(ssh "$PI_HOST" 'uname -m')"
case "$REMOTE_ARCH" in
  aarch64|arm64)
    TARGET="aarch64-unknown-linux-gnu"
    ;;
  armv7l)
    TARGET="armv7-unknown-linux-gnueabihf"
    ;;
  *)
    echo "ERROR: unsupported remote architecture: $REMOTE_ARCH" >&2
    echo "  Host: $PI_HOST" >&2
    echo "  Supported: aarch64/arm64, armv7l" >&2
    exit 1
    ;;
esac

TARBALL="$PROJECT_ROOT/dist/rustcnc-pi-$TARGET.tar.gz"
REMOTE_TARBALL="/tmp/$(basename "$TARBALL")"
REMOTE_WORKDIR="/tmp/rustcnc-install-$(date +%s)"

echo "==> Plan"
echo "  Remote:  $PI_HOST ($REMOTE_ARCH)"
echo "  Target:  $TARGET"
echo "  Prefix:  $PREFIX"
echo "  User:    $USER_NAME"
echo "  Group:   $GROUP_NAME"
echo "  Service: $SERVICE_NAME"

if ! confirm "Proceed with build, upload, and install on the Pi?"; then
  echo "Aborting."
  exit 0
fi

echo "==> Building ($TARGET)"
"$SCRIPT_DIR/cross-pi.sh" "$TARGET"

echo "==> Packaging ($TARGET)"
"$SCRIPT_DIR/package-pi.sh" "$TARGET"

if [[ ! -f "$TARBALL" ]]; then
  echo "ERROR: expected tarball not found: $TARBALL" >&2
  exit 1
fi

echo "==> Uploading tarball"
scp "$TARBALL" "$PI_HOST:$REMOTE_TARBALL"

echo "==> Running installer on Pi"
INSTALL_ARGS=(--prefix "$PREFIX" --user "$USER_NAME" --group "$GROUP_NAME" --service-name "$SERVICE_NAME")
if [[ "$ASSUME_YES" -eq 1 ]]; then
  INSTALL_ARGS+=(--yes)
fi

# Use a TTY so sudo can prompt if required.
ssh -tt "$PI_HOST" "set -eu; rm -rf '$REMOTE_WORKDIR'; mkdir -p '$REMOTE_WORKDIR'; tar -xzf '$REMOTE_TARBALL' -C '$REMOTE_WORKDIR'; cd '$REMOTE_WORKDIR/rustcnc-pi-$TARGET'; sudo ./install.sh ${INSTALL_ARGS[*]}"

if confirm "Clean up temporary files on the Pi?"; then
  ssh "$PI_HOST" "rm -rf '$REMOTE_WORKDIR' '$REMOTE_TARBALL'" || true
fi

echo "==> Done"
echo "  UI: http://$(echo "$PI_HOST" | sed -E 's/^.*@//'):8080/"
echo "  Status: ssh $PI_HOST 'systemctl status $SERVICE_NAME.service'"
