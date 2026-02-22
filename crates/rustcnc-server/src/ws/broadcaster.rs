use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use tracing::warn;

use rustcnc_core::machine::{AccessoryState, MachineSnapshot, MachineState};
use rustcnc_core::overrides::Overrides;
use rustcnc_core::ws_protocol::ServerMessage;
use rustcnc_streamer::streamer::SharedMachineState;

/// Background task that reads atomic SharedMachineState
/// and broadcasts MachineSnapshot to all WebSocket clients.
///
/// Runs at ws_tick_rate_hz when machine is active,
/// drops to ws_idle_tick_rate_hz when idle.
pub async fn broadcaster_task(
    shared_state: Arc<SharedMachineState>,
    tx: broadcast::Sender<ServerMessage>,
    active_rate_hz: u32,
    idle_rate_hz: u32,
) {
    let active_rate_hz_clamped = active_rate_hz.clamp(1, 1_000_000);
    if active_rate_hz_clamped != active_rate_hz {
        warn!(
            "Clamping server.ws_tick_rate_hz from {} to {}",
            active_rate_hz, active_rate_hz_clamped
        );
    }
    let idle_rate_hz_clamped = idle_rate_hz.clamp(1, 1_000_000);
    if idle_rate_hz_clamped != idle_rate_hz {
        warn!(
            "Clamping server.ws_idle_tick_rate_hz from {} to {}",
            idle_rate_hz, idle_rate_hz_clamped
        );
    }

    let active_interval = Duration::from_micros(1_000_000 / active_rate_hz_clamped as u64);
    let idle_interval = Duration::from_micros(1_000_000 / idle_rate_hz_clamped as u64);

    let mut ticker = interval(idle_interval);
    let mut is_active = false;

    loop {
        ticker.tick().await;

        let snapshot = read_snapshot(&shared_state);

        // Adjust tick rate based on machine state
        let now_active = snapshot.state.is_active();
        if now_active != is_active {
            is_active = now_active;
            let new_period = if is_active {
                active_interval
            } else {
                idle_interval
            };
            ticker = interval(new_period);
            ticker.tick().await; // consume the first immediate tick
        }

        // Broadcast (drops message if no receivers -- that's fine)
        let _ = tx.send(ServerMessage::MachineState(snapshot));
    }
}

/// Read a complete snapshot from atomic shared state (public for handler use)
pub fn read_snapshot_pub(s: &SharedMachineState) -> MachineSnapshot {
    read_snapshot(s)
}

/// Read a complete snapshot from atomic shared state
fn read_snapshot(s: &SharedMachineState) -> MachineSnapshot {
    let state_u16 = s.state.load(Ordering::Acquire);

    MachineSnapshot {
        state: MachineState::from_u16(state_u16),
        machine_pos: s.machine_pos(),
        work_pos: s.work_pos(),
        feed_rate: s.feed_rate_x1000.load(Ordering::Acquire) as f64 / 1000.0,
        spindle_speed: s.spindle_rpm_x1000.load(Ordering::Acquire) as f64 / 1000.0,
        overrides: Overrides {
            feed: s.feed_override.load(Ordering::Acquire),
            rapids: s.rapid_override.load(Ordering::Acquire),
            spindle: s.spindle_override.load(Ordering::Acquire),
        },
        accessories: AccessoryState {
            spindle_cw: s.spindle_cw.load(Ordering::Acquire),
            spindle_ccw: s.spindle_ccw.load(Ordering::Acquire),
            flood_coolant: s.coolant_flood.load(Ordering::Acquire),
            mist_coolant: s.coolant_mist.load(Ordering::Acquire),
        },
        input_pins: s.input_pins(),
        buffer: s.buffer_state(),
        line_number: s.line_number.load(Ordering::Acquire),
        connected: s.connected.load(Ordering::Acquire),
        firmware: s.firmware(),
    }
}
