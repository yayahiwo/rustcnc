use std::io::{Read, Write};
use std::time::Duration;

/// Abstraction over serial port for testability and simulator support.
/// Both real hardware ports and the simulator's virtual port implement this trait.
pub trait SerialPort: Read + Write + Send {
    fn set_timeout(&mut self, timeout: Duration) -> std::io::Result<()>;
    fn bytes_to_read(&self) -> std::io::Result<u32>;
    fn name(&self) -> Option<String>;

    /// Write a single real-time command byte. These bypass GRBL's input buffer.
    fn write_rt_command(&mut self, cmd: u8) -> std::io::Result<()> {
        self.write_all(&[cmd])
    }

    /// Clear the OS serial output buffer. Called before soft reset to prevent
    /// stale bytes from corrupting the post-reset command stream.
    fn clear_output_buffer(&mut self) -> std::io::Result<()> {
        Ok(()) // no-op by default (simulator)
    }

    /// Clear the OS serial input buffer. Called after soft reset to discard
    /// stale response bytes from the previous command stream.
    fn clear_input_buffer(&mut self) -> std::io::Result<()> {
        Ok(()) // no-op by default (simulator)
    }
}

/// Wrapper around the `serialport` crate for hardware serial ports
pub struct HardwareSerialPort {
    inner: Box<dyn serialport::SerialPort>,
}

impl HardwareSerialPort {
    pub fn open(port: &str, baud: u32) -> Result<Self, serialport::Error> {
        let inner = serialport::new(port, baud)
            .data_bits(serialport::DataBits::Eight)
            .stop_bits(serialport::StopBits::One)
            .parity(serialport::Parity::None)
            .timeout(Duration::from_millis(10))
            .open()?;
        Ok(Self { inner })
    }
}

impl Read for HardwareSerialPort {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for HardwareSerialPort {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl SerialPort for HardwareSerialPort {
    fn set_timeout(&mut self, timeout: Duration) -> std::io::Result<()> {
        self.inner
            .set_timeout(timeout)
            .map_err(std::io::Error::other)
    }

    fn bytes_to_read(&self) -> std::io::Result<u32> {
        self.inner.bytes_to_read().map_err(std::io::Error::other)
    }

    fn name(&self) -> Option<String> {
        self.inner.name()
    }

    fn clear_output_buffer(&mut self) -> std::io::Result<()> {
        self.inner
            .clear(serialport::ClearBuffer::Output)
            .map_err(std::io::Error::other)
    }

    fn clear_input_buffer(&mut self) -> std::io::Result<()> {
        self.inner
            .clear(serialport::ClearBuffer::Input)
            .map_err(std::io::Error::other)
    }
}

/// List available serial ports on the system
pub fn list_ports() -> Vec<serialport::SerialPortInfo> {
    serialport::available_ports().unwrap_or_default()
}
