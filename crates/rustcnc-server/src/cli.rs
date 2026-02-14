use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "rustcnc", about = "Industrial-grade CNC G-code sender")]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "config/default.toml")]
    pub config: String,

    /// Enable built-in GRBL simulator (no hardware required)
    #[arg(long)]
    pub simulator: bool,

    /// Serial port to connect to on startup
    #[arg(short, long)]
    pub port: Option<String>,

    /// Baud rate
    #[arg(short, long, default_value_t = 115200)]
    pub baud: u32,

    /// Server listen address
    #[arg(long, default_value = "0.0.0.0")]
    pub host: String,

    /// Server listen port
    #[arg(long, default_value_t = 8080)]
    pub listen_port: u16,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,
}
