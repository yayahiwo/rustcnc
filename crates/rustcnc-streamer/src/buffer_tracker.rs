use std::collections::VecDeque;

/// Implements the GRBL character-counting streaming protocol.
///
/// Tracks how many bytes are in GRBL's serial receive buffer.
/// A line can be sent only if its `byte_len` fits in the remaining space.
/// On each "ok" response, the byte count of the oldest pending line
/// is subtracted from the running total.
///
/// This is the most critical algorithm in the system -- it prevents
/// serial buffer overflow which would cause data loss and machine errors.
pub struct BufferTracker {
    /// Maximum buffer capacity (128 for GRBL, may be larger for grblHAL)
    capacity: usize,
    /// Current bytes consumed in GRBL's buffer
    used: usize,
    /// Queue of byte counts for each line sent but not yet acknowledged
    pending_lengths: VecDeque<usize>,
}

impl BufferTracker {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            used: 0,
            pending_lengths: VecDeque::with_capacity(32),
        }
    }

    /// Returns true if a line with the given byte length can fit in the buffer
    pub fn can_send(&self, byte_len: usize) -> bool {
        self.used + byte_len <= self.capacity
    }

    /// Record that a line was sent. Call only after `can_send()` returned true.
    pub fn line_sent(&mut self, byte_len: usize) {
        self.used += byte_len;
        self.pending_lengths.push_back(byte_len);
    }

    /// An "ok" or "error:X" was received -- free the oldest pending line.
    /// Returns the byte_len that was freed, or None if no pending lines.
    pub fn line_acknowledged(&mut self) -> Option<usize> {
        if let Some(len) = self.pending_lengths.pop_front() {
            self.used -= len;
            Some(len)
        } else {
            None
        }
    }

    /// Number of lines currently pending acknowledgment
    pub fn pending_count(&self) -> usize {
        self.pending_lengths.len()
    }

    /// Reset on soft-reset or reconnect
    pub fn reset(&mut self) {
        self.used = 0;
        self.pending_lengths.clear();
    }

    /// Current buffer utilization in bytes
    pub fn used(&self) -> usize {
        self.used
    }

    /// Available space in bytes
    pub fn available(&self) -> usize {
        self.capacity - self.used
    }

    /// Buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tracker() {
        let tracker = BufferTracker::new(128);
        assert_eq!(tracker.capacity(), 128);
        assert_eq!(tracker.used(), 0);
        assert_eq!(tracker.available(), 128);
        assert_eq!(tracker.pending_count(), 0);
        assert!(tracker.can_send(128));
    }

    #[test]
    fn test_send_and_ack() {
        let mut tracker = BufferTracker::new(128);

        // Send a 20-byte line
        assert!(tracker.can_send(20));
        tracker.line_sent(20);
        assert_eq!(tracker.used(), 20);
        assert_eq!(tracker.available(), 108);
        assert_eq!(tracker.pending_count(), 1);

        // Acknowledge it
        let freed = tracker.line_acknowledged();
        assert_eq!(freed, Some(20));
        assert_eq!(tracker.used(), 0);
        assert_eq!(tracker.pending_count(), 0);
    }

    #[test]
    fn test_fill_to_capacity() {
        let mut tracker = BufferTracker::new(128);

        // Fill exactly to capacity
        tracker.line_sent(50);
        tracker.line_sent(50);
        tracker.line_sent(28);
        assert_eq!(tracker.used(), 128);
        assert!(!tracker.can_send(1)); // no room for even 1 byte

        // Ack first line, now 50 bytes available
        tracker.line_acknowledged();
        assert_eq!(tracker.used(), 78);
        assert!(tracker.can_send(50));
        assert!(!tracker.can_send(51));
    }

    #[test]
    fn test_cannot_exceed_capacity() {
        let mut tracker = BufferTracker::new(128);
        tracker.line_sent(100);
        assert!(!tracker.can_send(29)); // 100 + 29 = 129 > 128
        assert!(tracker.can_send(28));  // 100 + 28 = 128 = 128
    }

    #[test]
    fn test_exact_capacity_line() {
        let mut tracker = BufferTracker::new(128);
        assert!(tracker.can_send(128));
        tracker.line_sent(128);
        assert!(!tracker.can_send(1));

        tracker.line_acknowledged();
        assert!(tracker.can_send(128));
    }

    #[test]
    fn test_many_small_lines() {
        let mut tracker = BufferTracker::new(128);
        // Send many 5-byte lines
        for _ in 0..25 {
            assert!(tracker.can_send(5));
            tracker.line_sent(5);
        }
        assert_eq!(tracker.used(), 125);
        assert!(tracker.can_send(3));
        assert!(!tracker.can_send(4));

        // Ack all
        for _ in 0..25 {
            tracker.line_acknowledged();
        }
        assert_eq!(tracker.used(), 0);
        assert_eq!(tracker.pending_count(), 0);
    }

    #[test]
    fn test_reset() {
        let mut tracker = BufferTracker::new(128);
        tracker.line_sent(50);
        tracker.line_sent(30);
        assert_eq!(tracker.pending_count(), 2);

        tracker.reset();
        assert_eq!(tracker.used(), 0);
        assert_eq!(tracker.pending_count(), 0);
        assert!(tracker.can_send(128));
    }

    #[test]
    fn test_ack_with_no_pending() {
        let mut tracker = BufferTracker::new(128);
        assert_eq!(tracker.line_acknowledged(), None);
    }

    #[test]
    fn test_fifo_order() {
        let mut tracker = BufferTracker::new(128);
        tracker.line_sent(10);
        tracker.line_sent(20);
        tracker.line_sent(30);

        assert_eq!(tracker.line_acknowledged(), Some(10));
        assert_eq!(tracker.line_acknowledged(), Some(20));
        assert_eq!(tracker.line_acknowledged(), Some(30));
    }

    #[test]
    fn test_interleaved_send_ack() {
        let mut tracker = BufferTracker::new(128);

        tracker.line_sent(40);   // used: 40
        tracker.line_sent(40);   // used: 80
        tracker.line_acknowledged(); // used: 40
        tracker.line_sent(40);   // used: 80
        tracker.line_sent(40);   // used: 120
        assert!(tracker.can_send(8));
        assert!(!tracker.can_send(9));

        tracker.line_acknowledged(); // used: 80
        tracker.line_acknowledged(); // used: 40
        tracker.line_acknowledged(); // used: 0
        assert_eq!(tracker.used(), 0);
    }
}
