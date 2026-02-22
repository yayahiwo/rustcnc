#!/usr/bin/env bash
set -euo pipefail

# Create a distributable tarball for Raspberry Pi 4 (aarch64).
#
# Output:
#   dist/rustcnc-pi-aarch64-unknown-linux-gnu.tar.gz
#
# Contents:
#   - rustcnc (server binary with embedded UI)
#   - config.toml (pi4 defaults)
#   - install.sh / uninstall.sh (run on the Pi)
#   - packaging/systemd/rustcnc.service (template)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TARGET="${1:-aarch64-unknown-linux-gnu}"
BINARY="$PROJECT_ROOT/target/$TARGET/release/rustcnc"

DIST_DIR="$PROJECT_ROOT/dist"
PKG_NAME="rustcnc-pi-$TARGET"
PKG_DIR="$DIST_DIR/$PKG_NAME"
TARBALL="$DIST_DIR/$PKG_NAME.tar.gz"

if [ ! -f "$BINARY" ]; then
    echo "Binary not found at $BINARY"
    echo "Building first..."
    "$SCRIPT_DIR/cross-pi.sh"
fi

rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/packaging/systemd"

cp "$BINARY" "$PKG_DIR/rustcnc"
cp "$PROJECT_ROOT/config/pi4.toml" "$PKG_DIR/config.toml"
cp "$PROJECT_ROOT/packaging/install.sh" "$PKG_DIR/install.sh"
cp "$PROJECT_ROOT/packaging/uninstall.sh" "$PKG_DIR/uninstall.sh"
cp "$PROJECT_ROOT/packaging/systemd/rustcnc.service" "$PKG_DIR/packaging/systemd/rustcnc.service"
cp "$PROJECT_ROOT/README.md" "$PKG_DIR/README.md"
cp "$PROJECT_ROOT/USER_GUIDE.md" "$PKG_DIR/USER_GUIDE.md"
cp "$PROJECT_ROOT/CHANGELOG.md" "$PKG_DIR/CHANGELOG.md"
cp "$PROJECT_ROOT/LICENSE-APACHE" "$PKG_DIR/LICENSE-APACHE"
cp "$PROJECT_ROOT/LICENSE-MIT" "$PKG_DIR/LICENSE-MIT"

chmod +x "$PKG_DIR/rustcnc" "$PKG_DIR/install.sh" "$PKG_DIR/uninstall.sh" || true

tar -czf "$TARBALL" -C "$DIST_DIR" "$PKG_NAME"

echo "==> Wrote $TARBALL"
if command -v shasum &>/dev/null; then
    shasum -a 256 "$TARBALL"
elif command -v sha256sum &>/dev/null; then
    sha256sum "$TARBALL"
fi
