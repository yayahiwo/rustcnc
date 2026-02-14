/// Linear motion interpolation for the simulator.
/// Computes intermediate positions along a move for realistic simulation.

/// Number of axes supported by grblHAL
pub const NUM_AXES: usize = 8;

/// A planned linear move
#[derive(Debug, Clone)]
pub struct LinearMove {
    pub start: [f64; NUM_AXES],
    pub end: [f64; NUM_AXES],
    pub feed_rate: f64,     // mm/min
    pub is_rapid: bool,
    total_distance: f64,
}

impl LinearMove {
    pub fn new(start: [f64; NUM_AXES], end: [f64; NUM_AXES], feed_rate: f64, is_rapid: bool) -> Self {
        let total_distance = start.iter().zip(end.iter())
            .map(|(s, e)| (e - s).powi(2))
            .sum::<f64>()
            .sqrt();

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
    pub fn interpolate(&self, t: f64) -> [f64; NUM_AXES] {
        let t = t.clamp(0.0, 1.0);
        let mut result = [0.0; NUM_AXES];
        for i in 0..NUM_AXES {
            result[i] = self.start[i] + (self.end[i] - self.start[i]) * t;
        }
        result
    }
}

/// Compute the target position for a move, handling absolute vs incremental.
/// Takes an array of optional axis values in grblHAL order: X,Y,Z,A,B,C,U,V
pub fn compute_target(
    current: &[f64; NUM_AXES],
    axes: &[Option<f64>; NUM_AXES],
    absolute: bool,
) -> [f64; NUM_AXES] {
    let mut target = [0.0; NUM_AXES];
    for i in 0..NUM_AXES {
        target[i] = if absolute {
            axes[i].unwrap_or(current[i])
        } else {
            current[i] + axes[i].unwrap_or(0.0)
        };
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos3(x: f64, y: f64, z: f64) -> [f64; NUM_AXES] {
        [x, y, z, 0.0, 0.0, 0.0, 0.0, 0.0]
    }

    fn axes(x: Option<f64>, y: Option<f64>, z: Option<f64>) -> [Option<f64>; NUM_AXES] {
        [x, y, z, None, None, None, None, None]
    }

    #[test]
    fn test_linear_move_distance() {
        let m = LinearMove::new(pos3(0.0, 0.0, 0.0), pos3(3.0, 4.0, 0.0), 1000.0, false);
        assert!((m.distance() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_linear_move_interpolate() {
        let m = LinearMove::new(pos3(0.0, 0.0, 0.0), pos3(10.0, 20.0, 30.0), 1000.0, false);
        let mid = m.interpolate(0.5);
        assert!((mid[0] - 5.0).abs() < 0.001);
        assert!((mid[1] - 10.0).abs() < 0.001);
        assert!((mid[2] - 15.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_target_absolute() {
        let target = compute_target(&pos3(10.0, 20.0, 30.0), &axes(Some(5.0), None, Some(0.0)), true);
        assert_eq!(target[0], 5.0);
        assert_eq!(target[1], 20.0);
        assert_eq!(target[2], 0.0);
    }

    #[test]
    fn test_compute_target_incremental() {
        let target = compute_target(&pos3(10.0, 20.0, 30.0), &axes(Some(5.0), None, Some(-10.0)), false);
        assert_eq!(target[0], 15.0);
        assert_eq!(target[1], 20.0);
        assert_eq!(target[2], 20.0);
    }

    #[test]
    fn test_zero_distance_move() {
        let m = LinearMove::new(pos3(5.0, 5.0, 5.0), pos3(5.0, 5.0, 5.0), 1000.0, false);
        assert_eq!(m.distance(), 0.0);
        assert_eq!(m.duration_secs(1.0), 0.0);
    }
}
