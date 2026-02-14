use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use tracing::trace;

use rustcnc_core::machine::{
    AccessoryState, BufferState, InputPins, MachineSnapshot, MachineState, FirmwareType,
};
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
    let active_interval = Duration::from_micros(1_000_000 / active_rate_hz as u64);
    let idle_interval = Duration::from_micros(1_000_000 / idle_rate_hz as u64);

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
    let state_byte = s.state.load(Ordering::Relaxed);
    let mpos = s.machine_pos();
    let wpos = s.work_pos();

    MachineSnapshot {
        state: MachineState::from_byte(state_byte),
        machine_pos: rustcnc_core::machine::Position::new(mpos[0], mpos[1], mpos[2]),
        work_pos: rustcnc_core::machine::Position::new(wpos[0], wpos[1], wpos[2]),
        feed_rate: s.feed_rate_x1000.load(Ordering::Relaxed) as f64 / 1000.0,
        spindle_speed: s.spindle_rpm_x1000.load(Ordering::Relaxed) as f64 / 1000.0,
        overrides: Overrides {
            feed: s.feed_override.load(Ordering::Relaxed),
            rapids: s.rapid_override.load(Ordering::Relaxed),
            spindle: s.spindle_override.load(Ordering::Relaxed),
        },
        accessories: AccessoryState::default(),
        input_pins: InputPins::default(),
        buffer: BufferState::default(),
        line_number: s.line_number.load(Ordering::Relaxed),
        connected: s.connected.load(Ordering::Relaxed),
        firmware: FirmwareType::Unknown,
    }
}
