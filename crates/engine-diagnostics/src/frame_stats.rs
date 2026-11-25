//! Per-frame statistics and the frame clock.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::error::DiagnosticsError;
use crate::layer::{FrameTimingHandle, SpanTiming};

/// Statistics for one completed frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameStats {
    /// Zero-based frame index.
    pub frame_index: u64,
    /// Wall-clock duration between `begin_frame` and `end_frame`.
    pub frame_time: Duration,
    /// Span timings recorded during the frame, sorted by name.
    pub spans: Vec<SpanTiming>,
}

/// Produces [`FrameStats`] and keeps a bounded history.
pub struct FrameClock {
    timing: Option<FrameTimingHandle>,
    started: Option<Instant>,
    next_index: u64,
    history: VecDeque<FrameStats>,
    capacity: usize,
}

impl FrameClock {
    /// Creates a clock; `history_capacity` is clamped to at least 1.
    #[must_use]
    pub fn new(timing: Option<FrameTimingHandle>, history_capacity: usize) -> Self {
        Self {
            timing,
            started: None,
            next_index: 0,
            history: VecDeque::new(),
            capacity: history_capacity.max(1),
        }
    }

    /// Replaces the span timing source.
    pub fn set_timing(&mut self, timing: Option<FrameTimingHandle>) {
        self.timing = timing;
    }

    /// Marks the start of a frame, discarding spans recorded before it.
    pub fn begin_frame(&mut self) {
        if let Some(t) = &self.timing {
            let _ = t.take();
        }
        self.started = Some(Instant::now());
    }

    /// Finishes the frame and returns its stats.
    ///
    /// # Errors
    /// Returns [`DiagnosticsError::FrameNotStarted`] without a prior `begin_frame`.
    pub fn end_frame(&mut self) -> Result<FrameStats, DiagnosticsError> {
        let started = self
            .started
            .take()
            .ok_or(DiagnosticsError::FrameNotStarted)?;
        let spans = self
            .timing
            .as_ref()
            .map(FrameTimingHandle::take)
            .unwrap_or_default();
        let stats = FrameStats {
            frame_index: self.next_index,
            frame_time: started.elapsed(),
            spans,
        };
        self.next_index = self.next_index.saturating_add(1);
        self.history.push_back(stats.clone());
        while self.history.len() > self.capacity {
            self.history.pop_front();
        }
        Ok(stats)
    }

    /// Most recent completed frame.
    #[must_use]
    pub fn last(&self) -> Option<&FrameStats> {
        self.history.back()
    }

    /// History, oldest first.
    pub fn history(&self) -> impl Iterator<Item = &FrameStats> + '_ {
        self.history.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::frame_timing_subscriber;

    #[test]
    fn frame_clock_end_without_begin() {
        let mut c = FrameClock::new(None, 4);
        assert!(matches!(
            c.end_frame(),
            Err(DiagnosticsError::FrameNotStarted)
        ));
    }

    #[test]
    fn frame_clock_stats() {
        let mut c = FrameClock::new(None, 4);
        c.begin_frame();
        std::thread::sleep(Duration::from_millis(2));
        let s = c.end_frame().expect("frame");
        assert_eq!(s.frame_index, 0);
        assert!(s.frame_time >= Duration::from_millis(2));
        c.begin_frame();
        assert_eq!(c.end_frame().expect("frame").frame_index, 1);
    }

    #[test]
    fn frame_clock_collects_spans_per_frame() {
        let (sub, h) = frame_timing_subscriber();
        let mut c = FrameClock::new(Some(h), 4);
        tracing::subscriber::with_default(sub, || {
            drop(tracing::info_span!("before").entered());
            c.begin_frame();
            drop(tracing::info_span!("during").entered());
            let s = c.end_frame().expect("frame");
            assert_eq!(s.spans.len(), 1);
            assert_eq!(s.spans[0].name, "during");
            c.begin_frame();
            let s2 = c.end_frame().expect("frame");
            assert!(s2.spans.is_empty());
        });
    }

    #[test]
    fn frame_clock_history_bounded() {
        let mut c = FrameClock::new(None, 3);
        for _ in 0..5 {
            c.begin_frame();
            c.end_frame().expect("frame");
        }
        let idx: Vec<u64> = c.history().map(|s| s.frame_index).collect();
        assert_eq!(idx, vec![2, 3, 4]);
        assert_eq!(c.last().map(|s| s.frame_index), Some(4));

        let mut z = FrameClock::new(None, 0);
        for _ in 0..3 {
            z.begin_frame();
            z.end_frame().expect("frame");
        }
        assert_eq!(z.history().count(), 1);
    }
}
