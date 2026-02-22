# RustCNC User Guide

## Safety first

Running CNC machines can cause injury or property damage.

- Always be ready to hit the machine’s physical E‑STOP.
- Start with conservative feeds/speeds and verify toolpaths.
- Keep the work area clear and secure cables (USB/serial disconnects can happen).

## What RustCNC is

RustCNC is a Raspberry Pi–friendly G‑code sender for GRBL / grblHAL controllers with a Web UI.
It runs a local HTTP/WebSocket server and streams G‑code over USB serial.

## Installation (recommended: release tarball)

1) Copy the tarball to the Pi and extract it:
- `scp rustcnc-pi-aarch64-unknown-linux-gnu.tar.gz pi@<pi-ip>:~/`
- `ssh pi@<pi-ip> 'tar -xzf rustcnc-pi-aarch64-unknown-linux-gnu.tar.gz'`

2) Run the installer on the Pi:
- `ssh pi@<pi-ip> 'cd rustcnc-pi-aarch64-unknown-linux-gnu && sudo ./install.sh'`

This installs to `/opt/rustcnc` by default and enables a `systemd` service (`rustcnc.service`).

### Uninstall

- `ssh pi@<pi-ip> 'cd rustcnc-pi-aarch64-unknown-linux-gnu && sudo ./uninstall.sh'`

## Accessing the UI

Open in a browser:
- `http://<pi-ip>:8080/`

If the UI doesn’t load, check:
- `sudo systemctl status rustcnc.service`
- `sudo journalctl -u rustcnc.service -n 200 --no-pager`

## Authentication (recommended on LANs)

During installation, `install.sh` can configure a username/password login to prevent random users on the LAN from
connecting and controlling the machine.

When enabled:
- The UI shows a login prompt.
- The server requires a valid session cookie for `/api/*` and `/ws`.

### Change/reset credentials

1) Generate a new password hash on the Pi:
- `printf '%s' 'NEW_PASSWORD' | /opt/rustcnc/rustcnc --hash-password-stdin`

2) Edit `/opt/rustcnc/config.toml`:
- `[auth] enabled = true`
- `[auth] username = "..."` (match what you will type in the UI)
- `[auth] password_hash = "sha256-iter:v1:..."` (from the command above)

3) Restart the service:
- `sudo systemctl restart rustcnc.service`

If you get locked out, set `[auth].enabled = false` and restart.

## Connecting to a controller

1) Plug the controller into the Pi over USB.
2) In the UI, open the connection panel.
3) Choose the detected serial port (commonly `/dev/ttyACM0` or `/dev/ttyUSB0`).
4) Choose the correct baud rate (GRBL defaults are often `115200`).
5) Click **Connect**.

### Serial permissions

On Debian/Raspberry Pi OS, USB serial devices are usually owned by group `dialout`.
The provided service runs with `SupplementaryGroups=dialout`. If you run the binary manually,
ensure your user is in `dialout`:

- `sudo usermod -a -G dialout $USER`
- Log out/in.

## Uploading G‑code

- Upload files from the UI.
- Files are stored in the configured `files.upload_dir` (default: `./gcode_files` relative to the service working dir).
- RustCNC enforces `files.max_file_size_mb` and streams uploads to disk (to avoid RAM spikes).

## Running a job

Typical flow:
1) Upload and load a file.
2) Verify preview/estimates (especially for coordinate mode changes like `G90/G91`).
3) Start the job.
4) Use pause/resume/cancel as needed.

## Macros

The UI includes a macros panel for frequently used snippets (probe routines, warmup, etc.).
Use macros carefully—treat them like code that can move the machine immediately.

## Logs and troubleshooting

### Service status
- `sudo systemctl status rustcnc.service`
- `sudo systemctl restart rustcnc.service`

### Logs
Depending on config:
- journald: `sudo journalctl -u rustcnc.service -f`
- file logs (Pi config): `/var/log/rustcnc/`

### Common issues

- **Can’t connect to serial / port missing**
  - Check `http://<pi-ip>:8080/api/ports`
  - Verify the USB device exists: `ls -l /dev/ttyACM* /dev/ttyUSB*`
  - Check permissions (dialout group).

- **UI opens but doesn’t update / WS issues**
  - Reverse proxies/dev tooling can trigger Origin checks.
  - Adjust `server.allowed_origins` only if you understand the risk.

- **Jerky streaming / missed steps**
  - Consider enabling CPU pinning and RT priority in `[streamer]` (requires CAP_SYS_NICE; systemd unit enables it).
  - Reduce other load on the Pi; use a stable PSU to avoid throttling/undervoltage.

## Configuration quick reference

Config is TOML. The main knobs:

- `[server]`
  - `host`, `port`
  - `ws_tick_rate_hz`, `ws_idle_tick_rate_hz`
  - `allowed_origins`
- `[serial]`
  - `default_port` (optional auto-connect)
  - `baud_rate`
  - `status_poll_rate_hz`
- `[streamer]`
  - `rx_buffer_size`
  - `cpu_pin_core`, `rt_priority`
  - `response_timeout_ms`
- `[logging]`
  - `level`
  - `log_dir` (optional)
  - `console_output`
- `[files]`
  - `upload_dir`
  - `max_file_size_mb`
- `[auth]`
  - `enabled`
  - `username`, `password_hash`
  - `session_ttl_secs`
