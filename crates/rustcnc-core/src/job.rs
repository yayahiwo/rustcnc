use serde::{Deserialize, Serialize};

/// Job lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum JobState {
    #[default]
    Idle,
    Running,
    Paused,
    Completed,
    Error,
    Cancelled,
}

impl JobState {
    /// Returns true if the job is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Error | Self::Cancelled)
    }

    /// Returns true if the job is actively running or paused
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }
}
