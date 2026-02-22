#!/usr/bin/env bash
set -euo pipefail

# Install RustCNC from an extracted release bundle on the Raspberry Pi.
#
# Run from the directory that contains:
#   - rustcnc
#   - config.toml
#   - packaging/systemd/rustcnc.service (optional template)
#
# Default install prefix: /opt/rustcnc
# Default service user:  rustcnc
#
# Usage:
#   sudo ./install.sh
#   sudo ./install.sh --yes
#   sudo ./install.sh --prefix /opt/rustcnc --user rustcnc --service-name rustcnc

usage() {
  cat <<EOF
Usage: sudo ./install.sh [options]

Options:
  --prefix PATH         Install prefix (default: /opt/rustcnc)
  --user NAME           Service user (default: rustcnc)
  --group NAME          Service group (default: same as user)
  --service-name NAME   systemd service name (default: rustcnc)
  --no-service          Do not install/enable systemd service (manual run only)
  --no-auth             Do not configure Web UI authentication
  --yes                 Non-interactive; assume "yes" to prompts
  -h, --help            Show this help

Environment variable overrides (optional):
  RUSTCNC_PREFIX, RUSTCNC_USER, RUSTCNC_GROUP, RUSTCNC_SERVICE_NAME
  RUSTCNC_AUTH_USERNAME, RUSTCNC_AUTH_PASSWORD_HASH
EOF
}

ASSUME_YES=0
INSTALL_SERVICE=1
CONFIGURE_AUTH=1

SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${RUSTCNC_PREFIX:-/opt/rustcnc}"
USER_NAME="${RUSTCNC_USER:-rustcnc}"
GROUP_NAME="${RUSTCNC_GROUP:-$USER_NAME}"
SERVICE_NAME="${RUSTCNC_SERVICE_NAME:-rustcnc}"

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
    --no-service)
      INSTALL_SERVICE=0; shift;;
    --no-auth)
      CONFIGURE_AUTH=0; shift;;
    --yes)
      ASSUME_YES=1; shift;;
    -h|--help)
      usage; exit 0;;
    *)
      echo "ERROR: unknown argument: $1" >&2
      usage
      exit 2;;
  esac
done

UNIT_PATH="/etc/systemd/system/${SERVICE_NAME}.service"

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo "ERROR: run as root (try: sudo ./install.sh)" >&2
  exit 1
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

confirm_default_yes() {
  local prompt="$1"
  if [[ "$ASSUME_YES" -eq 1 ]]; then
    return 0
  fi
  local ans
  read -r -p "${prompt} [Y/n] " ans
  case "${ans,,}" in
    n|no) return 1;;
    *) return 0;;
  esac
}

have_cmd() { command -v "$1" >/dev/null 2>&1; }

apt_install_if_possible() {
  if ! have_cmd apt-get; then
    return 1
  fi
  if [[ $# -eq 0 ]]; then
    return 0
  fi
  if confirm "Install missing packages via apt-get: $*?"; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y "$@"
    return 0
  fi
  return 1
}

require_cmd() {
  local cmd="$1"
  local hint="${2:-}"
  if ! have_cmd "$cmd"; then
    echo "ERROR: missing required command: $cmd" >&2
    if [[ -n "$hint" ]]; then
      echo "  Hint: $hint" >&2
    fi
    exit 1
  fi
}

machine_arch="$(uname -m 2>/dev/null || echo unknown)"

if [[ ! -f "$SRC_DIR/rustcnc" ]]; then
  echo "ERROR: missing $SRC_DIR/rustcnc" >&2
  exit 1
fi
if [[ ! -f "$SRC_DIR/config.toml" ]]; then
  echo "ERROR: missing $SRC_DIR/config.toml" >&2
  exit 1
fi

# Basic sanity: verify the binary can execute on this machine.
if ! "$SRC_DIR/rustcnc" --help >/dev/null 2>&1; then
  echo "ERROR: bundled binary does not run on this machine." >&2
  echo "  Machine arch: $machine_arch" >&2
  echo "  Likely cause: wrong tarball for this OS/CPU." >&2
  echo "  Fix: download the correct release bundle for your Pi (aarch64 vs armv7l)." >&2
  exit 1
fi

require_cmd install "On Debian/Raspberry Pi OS: sudo apt-get install coreutils"
require_cmd id
require_cmd getent "On Debian/Raspberry Pi OS: sudo apt-get install libc-bin"
require_cmd mkdir
require_cmd chown
require_cmd awk "On Debian/Raspberry Pi OS: sudo apt-get install gawk"

if [[ "$INSTALL_SERVICE" -eq 1 ]]; then
  if ! have_cmd systemctl; then
    echo "WARNING: systemctl not found; cannot install auto-start service." >&2
    echo "  You can still install files and run manually." >&2
    if confirm "Proceed with manual (no-service) install?"; then
      INSTALL_SERVICE=0
    else
      echo "Aborting." >&2
      exit 1
    fi
  fi
fi

echo "==> RustCNC installer"
echo "  Machine arch: $machine_arch"
echo "  Prefix:       $PREFIX"
echo "  User:         $USER_NAME"
echo "  Group:        $GROUP_NAME"
if [[ "$INSTALL_SERVICE" -eq 1 ]]; then
  echo "  systemd unit: $UNIT_PATH"
else
  echo "  systemd unit: (skipped)"
fi

if ! confirm "Proceed with installation?"; then
  echo "Aborting."
  exit 0
fi

ts="$(date +%Y%m%d%H%M%S)"

if [[ -f "$PREFIX/rustcnc" ]]; then
  echo "==> Existing installation detected at $PREFIX/rustcnc"
  if ! confirm "Overwrite existing binary?"; then
    echo "Aborting (no changes applied)."
    exit 0
  fi
fi

create_group() {
  local group="$1"
  if getent group "$group" >/dev/null 2>&1; then
    return 0
  fi
  if have_cmd groupadd; then
    groupadd --system "$group"
  elif have_cmd addgroup; then
    addgroup --system "$group"
  else
    apt_install_if_possible passwd adduser || true
    if have_cmd groupadd; then
      groupadd --system "$group"
    elif have_cmd addgroup; then
      addgroup --system "$group"
    else
      echo "ERROR: need groupadd/addgroup to create group '$group'." >&2
      echo "  On Debian/Raspberry Pi OS: sudo apt-get install passwd adduser" >&2
      exit 1
    fi
  fi
}

create_user() {
  local user="$1"
  local group="$2"
  local home="$3"
  if id -u "$user" >/dev/null 2>&1; then
    return 0
  fi
  if have_cmd useradd; then
    useradd \
      --system \
      --home-dir "$home" \
      --shell /usr/sbin/nologin \
      --gid "$group" \
      "$user"
  elif have_cmd adduser; then
    adduser \
      --system \
      --home "$home" \
      --shell /usr/sbin/nologin \
      --ingroup "$group" \
      "$user"
  else
    apt_install_if_possible passwd adduser || true
    if have_cmd useradd; then
      useradd \
        --system \
        --home-dir "$home" \
        --shell /usr/sbin/nologin \
        --gid "$group" \
        "$user"
    elif have_cmd adduser; then
      adduser \
        --system \
        --home "$home" \
        --shell /usr/sbin/nologin \
        --ingroup "$group" \
        "$user"
    else
      echo "ERROR: need useradd/adduser to create user '$user'." >&2
      echo "  On Debian/Raspberry Pi OS: sudo apt-get install passwd adduser" >&2
      exit 1
    fi
  fi
}

if ! id -u "$USER_NAME" >/dev/null 2>&1; then
  echo "==> Service user '$USER_NAME' does not exist"
  if confirm "Create system user '$USER_NAME' (group '$GROUP_NAME')?"; then
    create_group "$GROUP_NAME"
    create_user "$USER_NAME" "$GROUP_NAME" "$PREFIX"
  else
    echo "Aborting."
    exit 1
  fi
fi

mkdir -p "$PREFIX"

if [[ -f "$PREFIX/rustcnc" ]]; then
  echo "==> Backing up existing binary"
  cp -a "$PREFIX/rustcnc" "$PREFIX/rustcnc.bak.$ts" || true
fi
install -m 0755 "$SRC_DIR/rustcnc" "$PREFIX/rustcnc"

AUTH_BEGIN="# --- BEGIN rustcnc installer managed: auth ---"
AUTH_END="# --- END rustcnc installer managed: auth ---"

remove_managed_auth_block() {
  local cfg="$1"
  local tmp="$cfg.tmp.$$"
  awk -v begin="$AUTH_BEGIN" -v end="$AUTH_END" '
    $0 == begin { in_block=1; next }
    $0 == end { in_block=0; next }
    !in_block { print }
  ' "$cfg" >"$tmp"
  mv "$tmp" "$cfg"
}

append_managed_auth_block() {
  local cfg="$1"
  local enabled="$2"
  local username="${3:-}"
  local password_hash="${4:-}"

  {
    echo ""
    echo "$AUTH_BEGIN"
    echo "[auth]"
    echo "enabled = $enabled"
    if [[ "$enabled" == "true" ]]; then
      echo "username = \"$username\""
      echo "password_hash = \"$password_hash\""
    fi
    echo "session_ttl_secs = 86400"
    echo "$AUTH_END"
  } >>"$cfg"
}

is_safe_username() {
  local u="$1"
  [[ "$u" =~ ^[A-Za-z0-9._-]{1,64}$ ]]
}

if [[ -f "$PREFIX/config.toml" ]]; then
  echo "==> Existing config found at $PREFIX/config.toml"
  if confirm "Overwrite config.toml?"; then
    cp -a "$PREFIX/config.toml" "$PREFIX/config.toml.bak.$ts" || true
    install -m 0644 "$SRC_DIR/config.toml" "$PREFIX/config.toml"
  else
    install -m 0644 "$SRC_DIR/config.toml" "$PREFIX/config.toml.new"
    echo "  Wrote new config to $PREFIX/config.toml.new"
  fi
else
  install -m 0644 "$SRC_DIR/config.toml" "$PREFIX/config.toml"
fi

if [[ "$CONFIGURE_AUTH" -eq 1 ]]; then
  CFG_PATH="$PREFIX/config.toml"
  if [[ "$ASSUME_YES" -eq 1 ]]; then
    if [[ -n "${RUSTCNC_AUTH_USERNAME:-}" && -n "${RUSTCNC_AUTH_PASSWORD_HASH:-}" ]]; then
      if ! is_safe_username "$RUSTCNC_AUTH_USERNAME"; then
        echo "ERROR: RUSTCNC_AUTH_USERNAME contains unsupported characters." >&2
        echo "  Allowed: A-Z a-z 0-9 . _ - (max 64 chars)" >&2
        exit 1
      fi
      remove_managed_auth_block "$CFG_PATH"
      append_managed_auth_block "$CFG_PATH" "true" "$RUSTCNC_AUTH_USERNAME" "$RUSTCNC_AUTH_PASSWORD_HASH"
      echo "==> Configured Web UI authentication from environment variables"
    else
      echo "WARNING: --yes set but RUSTCNC_AUTH_USERNAME/RUSTCNC_AUTH_PASSWORD_HASH not provided; leaving Web UI auth unconfigured." >&2
      echo "  Re-run without --yes to configure auth interactively, or edit $CFG_PATH." >&2
    fi
  else
    if confirm_default_yes "Configure Web UI authentication (recommended)?"; then
      local_user=""
      while true; do
        read -r -p "Auth username (letters/numbers/._-): " local_user
        local_user="${local_user:-}"
        if is_safe_username "$local_user"; then
          break
        fi
        echo "Invalid username. Allowed: A-Z a-z 0-9 . _ - (max 64 chars)" >&2
      done

      pass1=""
      pass2=""
      while true; do
        read -r -s -p "Auth password: " pass1
        echo
        read -r -s -p "Confirm password: " pass2
        echo
        if [[ -z "$pass1" ]]; then
          echo "Password must not be empty." >&2
          continue
        fi
        if [[ "$pass1" != "$pass2" ]]; then
          echo "Passwords do not match. Try again." >&2
          continue
        fi
        break
      done

      pw_hash="$(printf '%s' "$pass1" | "$PREFIX/rustcnc" --hash-password-stdin)"
      pass1=""
      pass2=""
      remove_managed_auth_block "$CFG_PATH"
      append_managed_auth_block "$CFG_PATH" "true" "$local_user" "$pw_hash"
      echo "==> Web UI authentication enabled"
    else
      echo "==> Web UI authentication not configured (UI will be open on the LAN)"
    fi
  fi
fi

mkdir -p "$PREFIX/gcode_files"
chown -R "$USER_NAME:$GROUP_NAME" "$PREFIX"
chmod 0640 "$PREFIX/config.toml" || true

if [[ "$INSTALL_SERVICE" -eq 1 ]]; then
  echo "==> Installing systemd service"

  SERIAL_GROUP="${RUSTCNC_SERIAL_GROUP:-dialout}"
  if ! getent group "$SERIAL_GROUP" >/dev/null 2>&1; then
    echo "WARNING: group '$SERIAL_GROUP' not found; serial device permissions may fail." >&2
    echo "  Most Raspberry Pi OS installs use group 'dialout' for /dev/ttyACM* and /dev/ttyUSB*." >&2
    if confirm "Create group '$SERIAL_GROUP' now?"; then
      create_group "$SERIAL_GROUP"
    fi
  fi

  if have_cmd ss; then
    if ss -ltn 2>/dev/null | awk '{print $4}' | grep -Eq '(:|\\.)8080$'; then
      echo "WARNING: TCP port 8080 appears to be in use on this machine." >&2
      echo "  If RustCNC fails to start, change [server].port in $PREFIX/config.toml." >&2
    fi
  fi

  if [[ -f "$UNIT_PATH" ]]; then
    echo "  Existing unit found: $UNIT_PATH"
    if confirm "Overwrite the existing systemd unit?"; then
      cp -a "$UNIT_PATH" "$UNIT_PATH.bak.$ts" || true
    else
      echo "  Keeping existing unit; skipping unit install."
      systemctl daemon-reload || true
      echo "==> Done"
      echo "  UI:     http://<pi-ip>:8080/"
      echo "  Status: systemctl status $SERVICE_NAME.service"
      exit 0
    fi
  fi

  cat >"$UNIT_PATH" <<EOF
[Unit]
Description=RustCNC CNC Controller
After=network.target

[Service]
Type=simple
User=$USER_NAME
Group=$GROUP_NAME
SupplementaryGroups=$SERIAL_GROUP
WorkingDirectory=$PREFIX
ExecStart=$PREFIX/rustcnc --config $PREFIX/config.toml
Restart=on-failure
RestartSec=2

# Allow SCHED_FIFO (streamer.rt_priority) without setcap(8).
AmbientCapabilities=CAP_SYS_NICE
CapabilityBoundingSet=CAP_SYS_NICE
LimitRTPRIO=90

# Ensure /var/log/rustcnc exists and is writable by the service user.
LogsDirectory=$SERVICE_NAME

[Install]
WantedBy=multi-user.target
EOF

  systemctl daemon-reload

  if confirm "Enable and start $SERVICE_NAME.service now?"; then
    systemctl enable --now "$SERVICE_NAME.service"
  else
    echo "  Skipped enabling/starting the service."
    echo "  To start manually: systemctl start $SERVICE_NAME.service"
  fi
fi

echo "==> Done"
echo "  UI:     http://<pi-ip>:8080/"
if [[ "$INSTALL_SERVICE" -eq 1 ]]; then
  echo "  Status: systemctl status $SERVICE_NAME.service"
  echo "  Logs:   journalctl -u $SERVICE_NAME.service -n 200 --no-pager"
else
  echo "  Run:    $PREFIX/rustcnc --config $PREFIX/config.toml"
fi
