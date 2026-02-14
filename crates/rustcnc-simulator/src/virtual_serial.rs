use std::io::{Read, Write};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use rustcnc_streamer::serial::SerialPort;

/// A virtual serial port backed by crossbeam channels.
/// Implements the SerialPort trait so it can be used interchangeably
/// with HardwareSerialPort in the streamer.
pub struct VirtualSerialPort {
    /// Data we write "to the controller"
    tx: Sender<Vec<u8>>,
    /// Data we read "from the controller"
    rx: Receiver<Vec<u8>>,
    /// Local read buffer for partial reads
    read_buffer: Vec<u8>,
    read_pos: usize,
}

impl VirtualSerialPort {
    pub fn new(tx: Sender<Vec<u8>>, rx: Receiver<Vec<u8>>) -> Self {
        Self {
            tx,
            rx,
            read_buffer: Vec::new(),
            read_pos: 0,
        }
    }
}

impl Read for VirtualSerialPort {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // If local buffer is exhausted, try to receive from channel
        if self.read_pos >= self.read_buffer.len() {
            match self.rx.try_recv() {
                Ok(data) => {
                    self.read_buffer = data;
                    self.read_pos = 0;
                }
                Err(TryRecvError::Empty) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        "no data available",
                    ));
                }
                Err(TryRecvError::Disconnected) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "simulator channel closed",
                    ));
                }
            }
        }

        let available = self.read_buffer.len() - self.read_pos;
        let to_copy = available.min(buf.len());
        buf[..to_copy]
            .copy_from_slice(&self.read_buffer[self.read_pos..self.read_pos + to_copy]);
        self.read_pos += to_copy;
        Ok(to_copy)
    }
}

impl Write for VirtualSerialPort {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.tx.send(buf.to_vec()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "simulator channel closed")
        })?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl SerialPort for VirtualSerialPort {
    fn set_timeout(&mut self, _timeout: Duration) -> std::io::Result<()> {
        Ok(()) // timeouts handled via channel try_recv
    }

    fn bytes_to_read(&self) -> std::io::Result<u32> {
        let remaining = if self.read_pos < self.read_buffer.len() {
            self.read_buffer.len() - self.read_pos
        } else {
            0
        };
        Ok(remaining as u32)
    }

    fn name(&self) -> Option<String> {
        Some("SIMULATOR".to_string())
    }
}
