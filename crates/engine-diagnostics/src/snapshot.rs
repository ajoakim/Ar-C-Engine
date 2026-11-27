//! Planet-agnostic diagnostics snapshot: plain data a debug overlay can draw.
//!
//! Applications fill a [`DiagnosticsSnapshot`] each frame; a UI crate renders it. Nothing here
//! knows about any renderer, UI toolkit or application domain.

/// A used/cap budget (memory, uploads, queue slots, ...).
#[derive(Clone, Debug, PartialEq)]
pub struct Budget {
    pub label: String,
    pub used: f64,
    pub cap: f64,
}

impl Budget {
    /// `used / cap` clamped to `[0, 1]`; `0` when the cap is not positive or the ratio is not
    /// finite.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        if self.cap > 0.0 && self.cap.is_finite() {
            let f = self.used / self.cap;
            if f.is_finite() {
                f.clamp(0.0, 1.0)
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

/// Bin counts of a histogram; bin `i` is whatever the producer defines (for example a level).
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    pub label: String,
    pub bins: Vec<u32>,
}

/// One frame of overlay data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiagnosticsSnapshot {
    pub frame_ms: f64,
    pub prepare_ms: f64,
    pub gauges: Vec<(String, f64)>,
    pub budgets: Vec<Budget>,
    pub histograms: Vec<Histogram>,
}

impl DiagnosticsSnapshot {
    /// Append a labelled value.
    #[must_use]
    pub fn gauge(mut self, label: impl Into<String>, value: f64) -> Self {
        self.gauges.push((label.into(), value));
        self
    }

    /// Append a used/cap budget.
    #[must_use]
    pub fn budget(mut self, label: impl Into<String>, used: f64, cap: f64) -> Self {
        self.budgets.push(Budget {
            label: label.into(),
            used,
            cap,
        });
        self
    }

    /// Append a histogram.
    #[must_use]
    pub fn histogram(mut self, label: impl Into<String>, bins: Vec<u32>) -> Self {
        self.histograms.push(Histogram {
            label: label.into(),
            bins,
        });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let s = DiagnosticsSnapshot::default();
        assert!(s.gauges.is_empty() && s.budgets.is_empty() && s.histograms.is_empty());
        assert_eq!(s.frame_ms, 0.0);
    }

    #[test]
    fn builders_append_in_order() {
        let s = DiagnosticsSnapshot::default()
            .gauge("a", 1.0)
            .gauge("b", 2.0)
            .budget("m", 1.0, 2.0)
            .histogram("h", vec![1, 2]);
        assert_eq!(s.gauges[0].0, "a");
        assert_eq!(s.gauges[1], ("b".to_string(), 2.0));
        assert_eq!(s.budgets[0].label, "m");
        assert_eq!(s.histograms[0].bins, vec![1, 2]);
        assert_eq!(s.clone(), s);
    }

    #[test]
    fn fraction_clamps_and_never_nan() {
        let b = |used, cap| Budget {
            label: String::new(),
            used,
            cap,
        };
        assert_eq!(b(1.0, 2.0).fraction(), 0.5);
        assert_eq!(b(5.0, 2.0).fraction(), 1.0);
        assert_eq!(b(-1.0, 2.0).fraction(), 0.0);
        assert_eq!(b(1.0, 0.0).fraction(), 0.0);
        assert_eq!(b(0.0, 0.0).fraction(), 0.0);
        assert_eq!(b(f64::NAN, 2.0).fraction(), 0.0);
        assert_eq!(b(1.0, f64::NAN).fraction(), 0.0);
        assert_eq!(b(f64::INFINITY, f64::INFINITY).fraction(), 0.0);
    }
}
