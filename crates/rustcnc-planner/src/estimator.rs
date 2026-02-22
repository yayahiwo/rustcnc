use rustcnc_core::gcode::{extract_word, GCodeFile, MoveType};

/// Parameters for time estimation (trapezoidal motion profile).
pub struct EstimationParams {
    /// Acceleration in mm/s² (default 500.0, maps to GRBL $120/$121/$122)
    pub acceleration: f64,
    /// Maximum rapid traverse rate in mm/min (default 5000.0, maps to GRBL $110/$111/$112)
    pub max_rapid_rate: f64,
}

impl Default for EstimationParams {
    fn default() -> Self {
        Self {
            acceleration: 500.0,
            max_rapid_rate: 5000.0,
        }
    }
}

/// Returns cumulative seconds at each line index.
/// `cumulative[i]` = total estimated seconds from line 0 through line i (inclusive).
pub fn estimate_line_times(file: &GCodeFile, params: &EstimationParams) -> Vec<f64> {
    let mut cumulative = Vec::with_capacity(file.lines.len());
    let mut total_secs = 0.0;
    let mut current_feed: f64 = 0.0; // mm/min, modal
    let mut current_pos: Vec<f64> = vec![0.0; 3];

    for line in &file.lines {
        // Update modal feed rate from F word
        if let Some(f) = extract_word(&line.text, 'F') {
            if f > 0.0 {
                current_feed = f;
            }
        }

        let mut line_time = 0.0;

        match line.move_type {
            Some(MoveType::Dwell) => {
                // G4 P<seconds>
                if let Some(p) = extract_word(&line.text, 'P') {
                    line_time = p;
                }
            }
            Some(MoveType::Rapid) => {
                if let Some(ref endpoint) = line.endpoint {
                    let dist = linear_distance(&current_pos, endpoint);
                    if dist > 0.0 {
                        let speed = params.max_rapid_rate; // mm/min
                        line_time = move_time(dist, speed, params.acceleration);
                    }
                    update_pos(&mut current_pos, endpoint);
                }
            }
            Some(MoveType::Linear) => {
                if let Some(ref endpoint) = line.endpoint {
                    let dist = linear_distance(&current_pos, endpoint);
                    if dist > 0.0 && current_feed > 0.0 {
                        line_time = move_time(dist, current_feed, params.acceleration);
                    }
                    update_pos(&mut current_pos, endpoint);
                }
            }
            Some(MoveType::ArcCW) | Some(MoveType::ArcCCW) => {
                if let Some(ref endpoint) = line.endpoint {
                    let dist = if let Some(ref arc) = line.arc {
                        arc_length(&current_pos, endpoint, arc)
                    } else {
                        linear_distance(&current_pos, endpoint)
                    };
                    if dist > 0.0 && current_feed > 0.0 {
                        line_time = move_time(dist, current_feed, params.acceleration);
                    }
                    update_pos(&mut current_pos, endpoint);
                }
            }
            _ => {
                // Non-motion lines: update position if they have an endpoint
                if let Some(ref endpoint) = line.endpoint {
                    update_pos(&mut current_pos, endpoint);
                }
            }
        }

        total_secs += line_time;
        cumulative.push(total_secs);
    }

    cumulative
}

/// Compute time for a single move using trapezoidal motion profile.
/// Assumes start-from-rest and end-at-rest (no junction planning).
/// `distance` in mm, `speed` in mm/min, `accel` in mm/s².
fn move_time(distance: f64, speed_mm_min: f64, accel: f64) -> f64 {
    if distance <= 0.0 || speed_mm_min <= 0.0 || accel <= 0.0 {
        return 0.0;
    }

    let v_max = speed_mm_min / 60.0; // mm/s
    let d = distance;
    let a = accel;

    // Distance needed to accelerate to v_max
    let d_accel = v_max * v_max / (2.0 * a);

    if 2.0 * d_accel > d {
        // Triangular profile: never reaches v_max
        let v_peak = (d * a).sqrt();
        2.0 * v_peak / a
    } else {
        // Trapezoidal profile
        let t_accel = v_max / a;
        let d_cruise = d - 2.0 * d_accel;
        let t_cruise = d_cruise / v_max;
        2.0 * t_accel + t_cruise
    }
}

/// 3D Euclidean distance between two positions.
fn linear_distance(a: &[f64], b: &[f64]) -> f64 {
    let len = a.len().min(b.len()).min(3);
    let mut sum = 0.0;
    for i in 0..len {
        let d = b[i] - a[i];
        sum += d * d;
    }
    sum.sqrt()
}

/// Compute arc length from start position to endpoint given arc center offsets.
/// Uses incremental I/J/K offsets (G91.1 mode, which we enforce).
fn arc_length(start: &[f64], end: &[f64], arc: &rustcnc_core::gcode::ArcData) -> f64 {
    // Determine which axes are the arc plane axes and the linear axis
    let (a1, a2, linear_axis) = match arc.plane {
        17 => (0usize, 1usize, 2usize), // G17: XY plane, Z linear
        18 => (2, 0, 1),                // G18: ZX plane, Y linear
        19 => (1, 2, 0),                // G19: YZ plane, X linear
        _ => (0, 1, 2),
    };

    let s1 = if a1 < start.len() { start[a1] } else { 0.0 };
    let s2 = if a2 < start.len() { start[a2] } else { 0.0 };
    let e1 = if a1 < end.len() { end[a1] } else { 0.0 };
    let e2 = if a2 < end.len() { end[a2] } else { 0.0 };

    // Arc center offsets are incremental from start position
    let (offset1, offset2) = match arc.plane {
        17 => (arc.i, arc.j),
        18 => (arc.k, arc.i),
        19 => (arc.j, arc.k),
        _ => (arc.i, arc.j),
    };

    let center1 = s1 + offset1;
    let center2 = s2 + offset2;

    let radius = ((s1 - center1).powi(2) + (s2 - center2).powi(2)).sqrt();

    if radius < 1e-6 {
        return linear_distance(start, end);
    }

    // Compute angles
    let start_angle = (s2 - center2).atan2(s1 - center1);
    let end_angle = (e2 - center2).atan2(e1 - center1);

    let mut sweep = end_angle - start_angle;

    // Determine if this is CW or CCW from the move type context
    // The caller passes ArcData which doesn't carry CW/CCW, but we can check
    // the sign convention: for CW arcs sweep should be negative, for CCW positive.
    // Since we don't have the move type here, we'll use the convention that
    // if sweep is near zero, it's a full circle.
    // Actually, let's just ensure sweep is in [-2*PI, 0] for CW or [0, 2*PI] for CCW.
    // Without knowing CW/CCW, assume the shorter arc. This is a reasonable heuristic.
    if sweep > std::f64::consts::PI {
        sweep -= 2.0 * std::f64::consts::PI;
    } else if sweep < -std::f64::consts::PI {
        sweep += 2.0 * std::f64::consts::PI;
    }

    let arc_2d_length = radius * sweep.abs();

    // For helical arcs, compute linear distance along the linear axis
    let s_lin = if linear_axis < start.len() {
        start[linear_axis]
    } else {
        0.0
    };
    let e_lin = if linear_axis < end.len() {
        end[linear_axis]
    } else {
        0.0
    };
    let dz = e_lin - s_lin;

    if dz.abs() < 1e-6 {
        arc_2d_length
    } else {
        (arc_2d_length * arc_2d_length + dz * dz).sqrt()
    }
}

/// Update current position from an endpoint, growing the vec if needed.
fn update_pos(pos: &mut Vec<f64>, endpoint: &[f64]) {
    while pos.len() < endpoint.len() {
        pos.push(0.0);
    }
    pos[..endpoint.len()].copy_from_slice(endpoint);
}

/// Format seconds into a human-readable duration string
pub fn format_duration(secs: f64) -> String {
    let total_secs = secs as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {:02}m {:02}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {:02}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcnc_core::gcode::GCodeFile;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30.0), "30s");
        assert_eq!(format_duration(90.0), "1m 30s");
        assert_eq!(format_duration(3661.0), "1h 01m 01s");
    }

    #[test]
    fn test_move_time_trapezoidal() {
        // 100mm at 6000mm/min (100mm/s) with 500mm/s² accel
        // d_accel = 100^2/(2*500) = 10mm, so trapezoidal (2*10 < 100)
        // t_accel = 100/500 = 0.2s
        // d_cruise = 100 - 20 = 80mm
        // t_cruise = 80/100 = 0.8s
        // total = 2*0.2 + 0.8 = 1.2s
        let t = move_time(100.0, 6000.0, 500.0);
        assert!((t - 1.2).abs() < 0.001, "expected 1.2, got {}", t);
    }

    #[test]
    fn test_move_time_triangular() {
        // 1mm at 6000mm/min (100mm/s) with 500mm/s² accel
        // d_accel = 100^2/(2*500) = 10mm, so triangular (2*10 > 1)
        // v_peak = sqrt(1*500) = 22.36 mm/s
        // time = 2 * 22.36 / 500 = 0.0894s
        let t = move_time(1.0, 6000.0, 500.0);
        let expected = 2.0 * (1.0f64 * 500.0).sqrt() / 500.0;
        assert!(
            (t - expected).abs() < 0.001,
            "expected {}, got {}",
            expected,
            t
        );
    }

    #[test]
    fn test_move_time_zero_cases() {
        assert_eq!(move_time(0.0, 1000.0, 500.0), 0.0);
        assert_eq!(move_time(10.0, 0.0, 500.0), 0.0);
        assert_eq!(move_time(10.0, 1000.0, 0.0), 0.0);
    }

    #[test]
    fn test_linear_distance() {
        assert!((linear_distance(&[0.0, 0.0, 0.0], &[3.0, 4.0, 0.0]) - 5.0).abs() < 0.001);
        assert!((linear_distance(&[0.0, 0.0, 0.0], &[0.0, 0.0, 10.0]) - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_arc_length_quarter_circle() {
        // Quarter circle: start (10, 0), end (0, 10), center (0, 0)
        // I=-10, J=0 (incremental from start to center)
        let start = vec![10.0, 0.0, 0.0];
        let end = vec![0.0, 10.0, 0.0];
        let arc = rustcnc_core::gcode::ArcData {
            i: -10.0,
            j: 0.0,
            k: 0.0,
            plane: 17,
        };
        let len = arc_length(&start, &end, &arc);
        let expected = 10.0 * std::f64::consts::FRAC_PI_2; // pi/2 * r
        assert!(
            (len - expected).abs() < 0.1,
            "expected {}, got {}",
            expected,
            len
        );
    }

    #[test]
    fn test_estimate_line_times_simple() {
        // G0 X100 at 5000mm/min rapid, 500mm/s² accel
        let content = "G0 X100\nG1 X200 F1000\n";
        let file = GCodeFile::parse("test.nc".into(), content);
        let params = EstimationParams::default();
        let cum = estimate_line_times(&file, &params);

        assert_eq!(cum.len(), 2);
        assert!(cum[0] > 0.0, "first line should have time");
        assert!(cum[1] > cum[0], "cumulative should increase");
    }

    #[test]
    fn test_estimate_line_times_with_dwell() {
        let content = "G4 P2.5\nG0 X10\n";
        let file = GCodeFile::parse("test.nc".into(), content);
        let params = EstimationParams::default();
        let cum = estimate_line_times(&file, &params);

        assert_eq!(cum.len(), 2);
        assert!(
            (cum[0] - 2.5).abs() < 0.001,
            "dwell should be 2.5s, got {}",
            cum[0]
        );
    }

    #[test]
    fn test_cumulative_is_monotonic() {
        let content = "G0 X10\nG1 X20 F1000\nG1 X30\nG1 X40\nG0 Z5\n";
        let file = GCodeFile::parse("test.nc".into(), content);
        let params = EstimationParams::default();
        let cum = estimate_line_times(&file, &params);

        for i in 1..cum.len() {
            assert!(
                cum[i] >= cum[i - 1],
                "cumulative must be monotonic at index {}",
                i
            );
        }
    }
}
