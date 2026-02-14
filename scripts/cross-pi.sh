#!/usr/bin/env bash
set -euo pipefail

# Cross-compile RustCNC for Raspberry Pi 4 (aarch64-unknown-linux-gnu)
# Prerequisites:
#   - aarch64-unknown-linux-gnu target: rustup target add aarch64-unknown-linux-gnu
#   - Cross-linker: brew install filosottile/musl-cross/musl-cross (macOS)
#                   OR: apt install gcc-aarch64-linux-gnu (Linux)
#
# Alternatively, use `cross` (recommended):
#   cargo install cross
#   cross build --release --target aarch64-unknown-linux-gnu

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TARGET="aarch64-unknown-linux-gnu"

echo "==> Cross-compiling RustCNC for Pi 4 ($TARGET)"

# Build frontend first
"$SCRIPT_DIR/build-frontend.sh"

# Cross-compile
cd "$PROJECT_ROOT"

if command -v cross &> /dev/null; then
    echo "  Using 'cross' for cross-compilation..."
    cross build --release --target "$TARGET"
else
    echo "  Using cargo with local toolchain..."
    echo "  (If this fails, install 'cross': cargo install cross)"
    cargo build --release --target "$TARGET"
fi

BINARY="$PROJECT_ROOT/target/$TARGET/release/rustcnc"
if [ -f "$BINARY" ]; then
    echo "==> Build successful!"
    echo "    Binary: $BINARY"
    echo "    Size: $(du -sh "$BINARY" | cut -f1)"
else
    echo "==> Build failed: binary not found"
    exit 1
fi
