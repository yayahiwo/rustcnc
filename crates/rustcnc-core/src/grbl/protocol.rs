/// Standard GRBL baud rate
pub const BAUD_RATE: u32 = 115200;

/// Alternative high-speed baud rate
pub const BAUD_RATE_HIGH: u32 = 250000;

/// GRBL receive buffer size in bytes (standard GRBL)
pub const GRBL_RX_BUFFER_SIZE: usize = 128;

/// grblHAL default buffer size (may be larger, auto-detected via Bf: field)
pub const GRBLHAL_DEFAULT_RX_BUFFER_SIZE: usize = 1024;

/// Line terminator to send with G-code
pub const LINE_TERMINATOR: u8 = b'\n';

/// Maximum recommended status report polling rate (Hz)
pub const STATUS_POLL_RATE_HZ: u32 = 5;

/// Timeout for waiting for a response after connection (ms)
pub const CONNECT_TIMEOUT_MS: u64 = 500;

/// Timeout for 0x87 extended status request (ms)
pub const EXTENDED_STATUS_TIMEOUT_MS: u64 = 250;

/// Maximum G-code line length (GRBL limit)
pub const MAX_LINE_LENGTH: usize = 256;

/// GRBL planner buffer size (typical)
pub const PLANNER_BUFFER_BLOCKS: usize = 16;

/// Supported baud rates for serial connection
pub const SUPPORTED_BAUD_RATES: &[u32] = &[9600, 19200, 38400, 57600, 115200, 250000];
