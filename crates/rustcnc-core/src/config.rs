use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub serial: SerialConfig,
    #[serde(default)]
    pub streamer: StreamerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub simulator: SimulatorConfig,
    #[serde(default)]
    pub files: FileConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    /// Status broadcast rate when machine is active (Hz)
    pub ws_tick_rate_hz: u32,
    /// Status broadcast rate when machine is idle (Hz)
    pub ws_idle_tick_rate_hz: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 8080,
            ws_tick_rate_hz: 100,
            ws_idle_tick_rate_hz: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialConfig {
    pub default_port: Option<String>,
    pub baud_rate: u32,
    pub status_poll_rate_hz: u32,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            default_port: None,
            baud_rate: 115200,
            status_poll_rate_hz: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamerConfig {
    /// GRBL receive buffer size (128 for GRBL, auto-detected for grblHAL)
    pub rx_buffer_size: usize,
    /// CPU core to pin the streamer thread to (None = no pinning)
    pub cpu_pin_core: Option<usize>,
    /// Real-time scheduling priority (SCHED_FIFO, Linux only)
    pub rt_priority: Option<i32>,
    /// Timeout for GRBL response (ms)
    pub response_timeout_ms: u64,
}

impl Default for StreamerConfig {
    fn default() -> Self {
        Self {
            rx_buffer_size: 128,
            cpu_pin_core: None,
            rt_priority: None,
            response_timeout_ms: 5000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub log_dir: Option<String>,
    pub console_output: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            log_dir: None,
            console_output: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatorConfig {
    pub enabled: bool,
    /// Speed multiplier for simulated motion (1.0 = real-time)
    pub motion_speed_factor: f64,
    /// Startup delay before simulator responds (ms)
    pub startup_delay_ms: u64,
}

impl Default for SimulatorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            motion_speed_factor: 10.0,
            startup_delay_ms: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    pub upload_dir: String,
    pub max_file_size_mb: u64,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            upload_dir: "./gcode_files".into(),
            max_file_size_mb: 100,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            serial: SerialConfig::default(),
            streamer: StreamerConfig::default(),
            logging: LoggingConfig::default(),
            simulator: SimulatorConfig::default(),
            files: FileConfig::default(),
        }
    }
}

impl AppConfig {
    /// Load configuration from a TOML file, falling back to defaults
    pub fn load(path: &str) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load configuration or use defaults if file doesn't exist
    pub fn load_or_default(path: &str) -> Self {
        match Self::load(path) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!("Failed to load config from {}: {}, using defaults", path, e);
                Self::default()
            }
        }
    }
}
