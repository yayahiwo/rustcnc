use rustcnc_core::gcode::{GCodeFile, MoveType};

/// Estimate the total machining time for a G-code file.
///
/// Uses a simplified model:
/// - Tracks modal feed rate from F words
/// - Rapid moves use max rapid rate
/// - Calculates distance for each move
/// - Ignores acceleration/deceleration (conservative estimate)
pub fn estimate_duration(file: &GCodeFile, max_rapid_rate: f64) -> f64 {
    let mut total_secs = 0.0;
    let mut current_feed = 0.0;
    let mut current_pos = vec![0.0f64; 3];

    for line in &file.lines {
        // Extract feed rate if present
        if let Some(f_pos) = line.text.find('F') {
            let rest = &line.text[f_pos + 1..];
            let num: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(feed) = num.parse::<f64>() {
                current_feed = feed;
            }
        }

        if let (Some(move_type), Some(endpoint)) = (&line.move_type, &line.endpoint) {
            let dist = distance(&current_pos, endpoint);
            match move_type {
                MoveType::Rapid => {
                    if max_rapid_rate > 0.0 {
                        total_secs += (dist / max_rapid_rate) * 60.0;
                    }
                }
                MoveType::Linear | MoveType::ArcCW | MoveType::ArcCCW => {
                    if current_feed > 0.0 {
                        total_secs += (dist / current_feed) * 60.0;
                    }
                }
                _ => {}
            }
            current_pos = endpoint.clone();
        }
    }

    total_secs
}

fn distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter())
        .map(|(ai, bi)| (bi - ai).powi(2))
        .sum::<f64>()
        .sqrt()
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

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30.0), "30s");
        assert_eq!(format_duration(90.0), "1m 30s");
        assert_eq!(format_duration(3661.0), "1h 01m 01s");
    }
}
