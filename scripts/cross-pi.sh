#!/usr/bin/env bash
set -euo pipefail

# Cross-compile RustCNC for Raspberry Pi (default: aarch64-unknown-linux-gnu)
# Prerequisites:
#   - target installed: rustup target add <target>
#   - Cross-linker: brew install filosottile/musl-cross/musl-cross (macOS)
#                   OR: apt install gcc-aarch64-linux-gnu (Linux)
#
# Alternatively, use `cross` (recommended):
#   cargo install cross
#   cross build --release --target <target>

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TARGET="${1:-aarch64-unknown-linux-gnu}"

echo "==> Cross-compiling RustCNC ($TARGET)"

# Preflight checks
if ! command -v npm &> /dev/null; then
    echo "ERROR: npm not found (needed to build the embedded frontend)." >&2
    echo "Install Node.js + npm, then re-run." >&2
    exit 1
fi

# If we're not using `cross`, we need a local cross-linker for the target.
if ! command -v cross &> /dev/null; then
    case "$TARGET" in
        aarch64-unknown-linux-gnu)
            if ! command -v aarch64-unknown-linux-gnu-gcc &> /dev/null; then
                echo "ERROR: missing cross linker: aarch64-unknown-linux-gnu-gcc" >&2
                echo "  Fix: install aarch64 cross toolchain, or install/use 'cross'." >&2
                exit 1
            fi
            ;;
        armv7-unknown-linux-gnueabihf)
            if ! command -v arm-linux-gnueabihf-gcc &> /dev/null; then
                echo "ERROR: missing cross linker: arm-linux-gnueabihf-gcc" >&2
                echo "  Fix: install armv7 cross toolchain, or install/use 'cross'." >&2
                exit 1
            fi
            ;;
    esac

    if command -v rustup &> /dev/null; then
        if ! rustup target list --installed | grep -qx "$TARGET"; then
            echo "ERROR: Rust target '$TARGET' is not installed." >&2
            echo "  Fix: rustup target add $TARGET" >&2
            exit 1
        fi
    fi
fi

# Build frontend first
"$SCRIPT_DIR/build-frontend.sh"

# Cross-compile
cd "$PROJECT_ROOT"

if command -v cross &> /dev/null; then
    echo "  Using 'cross' for cross-compilation..."
    cross build --release --target "$TARGET"
else
    # Prefer rustup-managed toolchain/targets over Homebrew Rust.
    # Some setups have a brew `cargo`/`rustc` on PATH without cross targets installed.
    if command -v rustup &> /dev/null; then
        TOOLCHAIN_BIN="$(dirname "$(rustup which cargo)")"
        export PATH="$TOOLCHAIN_BIN:$PATH"
        export RUSTC="$(rustup which rustc)"
    fi

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
