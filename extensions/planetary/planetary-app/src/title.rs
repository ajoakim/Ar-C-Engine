//! Window-title readout: altitude, speed and detail level. Pure and deterministic (fixed
//! decimals, no locale, no clock); the throttle counts frames.

/// Default throttle: every 15th frame (about 4 Hz at 60 fps).
pub const TITLE_EVERY_FRAMES: u64 = 15;

/// Format a length in metres: `"850 m"` below 1000 m, `"12.3 km"` otherwise, `"?"` if not finite.
fn length(m: f64) -> String {
    if !m.is_finite() {
        "?".into()
    } else if m.abs() < 1000.0 {
        format!("{m:.0} m")
    } else {
        format!("{:.1} km", m / 1000.0)
    }
}

/// Format a speed in m/s: `"850 m/s"` below 1000, `"1.2 km/s"` otherwise, `"?"` if not finite.
fn speed(v: f64) -> String {
    if !v.is_finite() {
        "?".into()
    } else if v.abs() < 1000.0 {
        format!("{v:.0} m/s")
    } else {
        format!("{:.1} km/s", v / 1000.0)
    }
}

/// Window title, for example `"alt 12.3 km | 1.2 km/s | hex L14"`.
#[must_use]
pub fn format_title(
    altitude_m: f64,
    speed_mps: f64,
    topology_name: &str,
    level: Option<u8>,
) -> String {
    let level = level.map_or_else(|| "L?".to_string(), |l| format!("L{l}"));
    format!(
        "alt {} | {} | {topology_name} {level}",
        length(altitude_m),
        speed(speed_mps)
    )
}

/// True on frames that are multiples of `every` (`every == 0` is treated as 1).
#[must_use]
pub fn should_update_title(frame: u64, every: u64) -> bool {
    frame.is_multiple_of(every.max(1))
}

/// Speed from the displacement over a frame; 0 for a non-positive or non-finite `dt`.
#[must_use]
pub fn speed_from_displacement(distance_m: f64, dt: f64) -> f64 {
    if dt.is_finite() && dt > 0.0 && distance_m.is_finite() {
        distance_m / dt
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_format_is_exact() {
        assert_eq!(
            format_title(12_300.0, 1_200.0, "hex", Some(14)),
            "alt 12.3 km | 1.2 km/s | hex L14"
        );
        assert_eq!(
            format_title(850.0, 40.0, "cube", None),
            "alt 850 m | 40 m/s | cube L?"
        );
        assert_eq!(
            format_title(f64::NAN, f64::INFINITY, "hex", Some(1)),
            "alt ? | ? | hex L1"
        );
    }

    #[test]
    fn throttle_counts_frames() {
        assert!(should_update_title(0, 15));
        assert!(!should_update_title(1, 15));
        assert!(should_update_title(30, 15));
        assert!(should_update_title(7, 0));
        assert!(should_update_title(7, 1));
    }

    #[test]
    fn speed_guards_dt() {
        assert_eq!(speed_from_displacement(10.0, 0.5), 20.0);
        assert_eq!(speed_from_displacement(10.0, 0.0), 0.0);
        assert_eq!(speed_from_displacement(10.0, -1.0), 0.0);
        assert_eq!(speed_from_displacement(f64::NAN, 1.0), 0.0);
    }
}
