#!/usr/bin/env bash
set -euo pipefail

# Deploy RustCNC binary to Raspberry Pi 4
# Usage: ./deploy-pi.sh [user@host] [remote-path]
#
# Examples:
#   ./deploy-pi.sh pi@<pi-ip>
#   ./deploy-pi.sh pi@cnc.local /opt/rustcnc

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TARGET="aarch64-unknown-linux-gnu"
BINARY="$PROJECT_ROOT/target/$TARGET/release/rustcnc"

PI_HOST="${1:-pi@raspberrypi.local}"
REMOTE_DIR="${2:-~/rustcnc}"

if [ ! -f "$BINARY" ]; then
    echo "Binary not found at $BINARY"
    echo "Run ./cross-pi.sh first"
    exit 1
fi

echo "==> Deploying RustCNC to $PI_HOST:$REMOTE_DIR"

# Create remote directory
ssh "$PI_HOST" "mkdir -p $REMOTE_DIR"

# Copy binary
echo "  Copying binary..."
scp "$BINARY" "$PI_HOST:$REMOTE_DIR/rustcnc"
ssh "$PI_HOST" "chmod +x $REMOTE_DIR/rustcnc"

# Copy config
echo "  Copying config..."
scp "$PROJECT_ROOT/config/pi4.toml" "$PI_HOST:$REMOTE_DIR/config.toml"

# Create upload dir (pi4.toml defaults to ./gcode_files relative to WorkingDirectory)
ssh "$PI_HOST" "mkdir -p $REMOTE_DIR/gcode_files"

echo "==> Deployed successfully!"
echo ""
echo "To run on Pi:"
echo "  ssh $PI_HOST"
echo "  cd $REMOTE_DIR"
echo "  sudo ./rustcnc --config config.toml"
echo ""
echo "For RT scheduling, set capability:"
echo "  sudo setcap cap_sys_nice+ep $REMOTE_DIR/rustcnc"
echo "  ./rustcnc --config config.toml"
