use tracing::{info, warn};

/// Pin the current thread to a specific CPU core.
/// Returns true if successful. Logs a warning on failure.
pub fn pin_to_core(core_id: usize) -> bool {
    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
    if let Some(id) = core_ids.get(core_id) {
        let result = core_affinity::set_for_current(*id);
        if result {
            info!("Pinned streamer thread to CPU core {}", core_id);
        } else {
            warn!("Failed to pin streamer thread to CPU core {}", core_id);
        }
        result
    } else {
        warn!(
            "CPU core {} not available (system has {} cores)",
            core_id,
            core_ids.len()
        );
        false
    }
}

/// Set the current thread to SCHED_FIFO real-time scheduling.
/// Only works on Linux with appropriate permissions (root or CAP_SYS_NICE).
#[cfg(target_os = "linux")]
pub fn set_realtime_priority(priority: i32) -> bool {
    unsafe {
        let param = libc::sched_param {
            sched_priority: priority,
        };
        let result = libc::sched_setscheduler(0, libc::SCHED_FIFO, &param);
        if result == 0 {
            info!("Set SCHED_FIFO real-time priority to {}", priority);
            true
        } else {
            let errno = *libc::__errno_location();
            warn!(
                "Failed to set SCHED_FIFO priority {}: errno={}",
                priority, errno
            );
            false
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn set_realtime_priority(_priority: i32) -> bool {
    warn!("SCHED_FIFO not supported on this platform, skipping RT scheduling");
    false
}

/// Apply all real-time optimizations for the streamer thread.
/// Called at the start of the streamer thread.
pub fn apply_rt_config(cpu_core: Option<usize>, rt_priority: Option<i32>) {
    if let Some(core) = cpu_core {
        pin_to_core(core);
    }
    if let Some(priority) = rt_priority {
        set_realtime_priority(priority);
    }
}
