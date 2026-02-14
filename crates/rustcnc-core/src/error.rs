use thiserror::Error;

#[derive(Error, Debug)]
pub enum RustCncError {
    #[error("Serial port error: {0}")]
    Serial(String),

    #[error("GRBL error {code}: {message}")]
    GrblError { code: u8, message: String },

    #[error("GRBL alarm {code}: {message}")]
    GrblAlarm { code: u8, message: String },

    #[error("G-code parse error at line {line}: {message}")]
    GcodeParse { line: usize, message: String },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Job error: {0}")]
    Job(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Channel send error")]
    ChannelSend,

    #[error("Channel receive error")]
    ChannelRecv,
}

pub type Result<T> = std::result::Result<T, RustCncError>;
