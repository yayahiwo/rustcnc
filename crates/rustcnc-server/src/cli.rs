use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "rustcnc", about = "Industrial-grade CNC G-code sender")]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "config/default.toml")]
    pub config: String,

    /// Serial port to connect to on startup
    #[arg(short, long)]
    pub port: Option<String>,

    /// Baud rate (overrides config file)
    #[arg(short, long)]
    pub baud: Option<u32>,

    /// Server listen address (overrides config file)
    #[arg(long)]
    pub host: Option<String>,

    /// Server listen port (overrides config file)
    #[arg(long)]
    pub listen_port: Option<u16>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long)]
    pub log_level: Option<String>,

    /// Read a password from stdin, print an `auth.password_hash` value, and exit.
    ///
    /// Example:
    ///   printf '%s' 'MyPassword' | rustcnc --hash-password-stdin
    #[arg(long)]
    pub hash_password_stdin: bool,
}
