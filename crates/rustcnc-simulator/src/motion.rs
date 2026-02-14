/// Linear motion interpolation for the simulator.
/// Computes intermediate positions along a move for realistic simulation.

/// A planned linear move
#[derive(Debug, Clone)]
pub struct LinearMove {
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub feed_rate: f64,     // mm/min
    pub is_rapid: bool,
    total_distance: f64,
}

impl LinearMove {
    pub fn new(start: [f64; 3], end: [f64; 3], feed_rate: f64, is_rapid: bool) -> Self {
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let dz = end[2] - start[2];
        let total_distance = (dx * dx + dy * dy + dz * dz).sqrt();

        Self {
            start,
            end,
            feed_rate,
            is_rapid,
            total_distance,
        }
    }

    /// Total distance of the move in mm
    pub fn distance(&self) -> f64 {
        self.total_distance
    }

    /// Duration of the move in seconds
    pub fn duration_secs(&self, speed_factor: f64) -> f64 {
        if self.total_distance == 0.0 || self.feed_rate == 0.0 {
            return 0.0;
        }
        let effective_rate = if self.is_rapid {
            // Rapid moves use max rapid rate (e.g., 5000 mm/min)
            5000.0
        } else {
            self.feed_rate
        };
        // feed_rate is mm/min, convert to seconds
        (self.total_distance / effective_rate) * 60.0 / speed_factor
    }

    /// Interpolate position at progress t (0.0 to 1.0)
    pub fn interpolate(&self, t: f64) -> [f64; 3] {
        let t = t.clamp(0.0, 1.0);
        [
            self.start[0] + (self.end[0] - self.start[0]) * t,
            self.start[1] + (self.end[1] - self.start[1]) * t,
            self.start[2] + (self.end[2] - self.start[2]) * t,
        ]
    }
}

/// Compute the target position for a move, handling absolute vs incremental
pub fn compute_target(
    current: &[f64; 3],
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    absolute: bool,
) -> [f64; 3] {
    if absolute {
        [
            x.unwrap_or(current[0]),
            y.unwrap_or(current[1]),
            z.unwrap_or(current[2]),
        ]
    } else {
        [
            current[0] + x.unwrap_or(0.0),
            current[1] + y.unwrap_or(0.0),
            current[2] + z.unwrap_or(0.0),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_move_distance() {
        let m = LinearMove::new([0.0, 0.0, 0.0], [3.0, 4.0, 0.0], 1000.0, false);
        assert!((m.distance() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_linear_move_interpolate() {
        let m = LinearMove::new([0.0, 0.0, 0.0], [10.0, 20.0, 30.0], 1000.0, false);
        let mid = m.interpolate(0.5);
        assert!((mid[0] - 5.0).abs() < 0.001);
        assert!((mid[1] - 10.0).abs() < 0.001);
        assert!((mid[2] - 15.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_target_absolute() {
        let target = compute_target(&[10.0, 20.0, 30.0], Some(5.0), None, Some(0.0), true);
        assert_eq!(target, [5.0, 20.0, 0.0]);
    }

    #[test]
    fn test_compute_target_incremental() {
        let target = compute_target(&[10.0, 20.0, 30.0], Some(5.0), None, Some(-10.0), false);
        assert_eq!(target, [15.0, 20.0, 20.0]);
    }

    #[test]
    fn test_zero_distance_move() {
        let m = LinearMove::new([5.0, 5.0, 5.0], [5.0, 5.0, 5.0], 1000.0, false);
        assert_eq!(m.distance(), 0.0);
        assert_eq!(m.duration_secs(1.0), 0.0);
    }
}
