#!/usr/bin/env bash
set -euo pipefail

# Run RustCNC in the foreground on a Raspberry Pi (useful for debugging without systemd).
#
# Defaults assume a standard install to /opt/rustcnc.
#
# Usage:
#   ./scripts/run-pi.sh
#   ./scripts/run-pi.sh --config /opt/rustcnc/config.toml
#   ./scripts/run-pi.sh --bin /opt/rustcnc/rustcnc --config /opt/rustcnc/config.toml --log ~/rustcnc-foreground.log
#
# Any extra args after `--` are passed to the binary.
#
# Note: for production use, prefer the systemd service installed by `install.sh`.

BIN="/opt/rustcnc/rustcnc"
CFG="/opt/rustcnc/config.toml"
LOG="${RUSTCNC_RUN_LOG:-$HOME/rustcnc-foreground.log}"

usage() {
  cat <<EOF
Usage: ./scripts/run-pi.sh [options] [-- extra-args...]

Options:
  --bin PATH     Path to rustcnc binary (default: $BIN)
  --config PATH  Path to config.toml (default: $CFG)
  --log PATH     Path to log file (default: $LOG)
  -h, --help     Show this help
EOF
}

EXTRA_ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin) BIN="${2:-}"; shift 2;;
    --config) CFG="${2:-}"; shift 2;;
    --log) LOG="${2:-}"; shift 2;;
    -h|--help) usage; exit 0;;
    --) shift; EXTRA_ARGS+=("$@"); break;;
    *) EXTRA_ARGS+=("$1"); shift;;
  esac
done

if [[ ! -x "$BIN" ]]; then
  echo "ERROR: rustcnc binary not found/executable: $BIN" >&2
  echo "  If installed via tarball: $BIN should exist." >&2
  exit 1
fi
if [[ ! -f "$CFG" ]]; then
  echo "ERROR: config not found: $CFG" >&2
  exit 1
fi

{
  echo "=== RustCNC starting at $(date) ==="
  echo "BIN=$BIN"
  echo "CFG=$CFG"
  echo "ARGS=${EXTRA_ARGS[*]:-}"
} >>"$LOG"

set +e
RUST_BACKTRACE=full "$BIN" --config "$CFG" "${EXTRA_ARGS[@]}" >>"$LOG" 2>&1
EXIT_CODE=$?
set -e

echo "=== RustCNC exited at $(date) with code $EXIT_CODE ===" >>"$LOG"
if [[ "$EXIT_CODE" -gt 128 ]]; then
  SIG=$((EXIT_CODE - 128))
  echo "=== Killed by signal $SIG ($(kill -l "$SIG" 2>/dev/null || echo unknown)) ===" >>"$LOG"
fi
