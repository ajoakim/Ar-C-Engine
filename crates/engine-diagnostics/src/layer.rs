//! Span timing collection layer.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use tracing::Id;
use tracing::span::Attributes;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;

/// Accumulated timing for one span name.
///
/// Timing semantic is creation-to-close (not busy time): the clock starts when
/// the span is created and stops when it closes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanTiming {
    /// Span name.
    pub name: &'static str,
    /// Total creation-to-close duration across all closed spans of this name.
    pub total: Duration,
    /// Number of closed spans of this name.
    pub count: u32,
}

type Accumulator = BTreeMap<&'static str, (Duration, u32)>;

/// Shared read handle onto the timings recorded by a [`FrameTimingLayer`].
#[derive(Clone, Default)]
pub struct FrameTimingHandle {
    inner: Arc<Mutex<Accumulator>>,
}

impl std::fmt::Debug for FrameTimingHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameTimingHandle").finish_non_exhaustive()
    }
}

fn to_vec(map: &Accumulator) -> Vec<SpanTiming> {
    map.iter()
        .map(|(name, (total, count))| SpanTiming {
            name,
            total: *total,
            count: *count,
        })
        .collect()
}

impl FrameTimingHandle {
    fn lock(&self) -> MutexGuard<'_, Accumulator> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Returns timings sorted by name without resetting them.
    #[must_use]
    pub fn snapshot(&self) -> Vec<SpanTiming> {
        to_vec(&self.lock())
    }

    /// Returns timings sorted by name and resets the accumulator.
    #[must_use]
    pub fn take(&self) -> Vec<SpanTiming> {
        let mut guard = self.lock();
        let out = to_vec(&guard);
        guard.clear();
        out
    }

    fn record(&self, name: &'static str, elapsed: Duration) {
        let mut guard = self.lock();
        let entry = guard.entry(name).or_insert((Duration::ZERO, 0));
        entry.0 = entry.0.saturating_add(elapsed);
        entry.1 = entry.1.saturating_add(1);
    }
}

struct SpanStart(Instant);

/// A `tracing_subscriber` layer that records per-name span durations.
pub struct FrameTimingLayer {
    handle: FrameTimingHandle,
}

impl FrameTimingLayer {
    /// Creates a layer and the handle used to read its timings.
    #[must_use]
    pub fn new() -> (Self, FrameTimingHandle) {
        let handle = FrameTimingHandle::default();
        (
            Self {
                handle: handle.clone(),
            },
            handle,
        )
    }
}

impl<S> Layer<S> for FrameTimingLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, _attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(SpanStart(Instant::now()));
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(&id) {
            let elapsed = span.extensions().get::<SpanStart>().map(|s| s.0.elapsed());
            if let Some(elapsed) = elapsed {
                self.handle.record(span.metadata().name(), elapsed);
            }
        }
    }
}

/// Registry + [`FrameTimingLayer`] only (no fmt, no filter).
///
/// For scoped use with `tracing::subscriber::with_default` in tests and
/// headless tools.
#[must_use]
pub fn frame_timing_subscriber() -> (
    impl tracing::Subscriber + Send + Sync + 'static,
    FrameTimingHandle,
) {
    let (layer, handle) = FrameTimingLayer::new();
    (tracing_subscriber::registry().with(layer), handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::info_span;

    #[test]
    fn layer_records_span_timing() {
        let (layer, handle) = FrameTimingLayer::new();
        let sub = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(sub, || {
            for _ in 0..2 {
                let _s = info_span!("physics").entered();
            }
        });
        let snap = handle.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].name, "physics");
        assert_eq!(snap[0].count, 2);
    }

    #[test]
    fn layer_take_resets() {
        let (sub, handle) = frame_timing_subscriber();
        tracing::subscriber::with_default(sub, || {
            let _s = info_span!("a").entered();
        });
        assert_eq!(handle.take().len(), 1);
        assert!(handle.snapshot().is_empty());
    }

    #[test]
    fn layer_frame_timing_subscriber() {
        let (sub, h) = frame_timing_subscriber();
        tracing::subscriber::with_default(sub, || {
            let _s = info_span!("tick").entered();
        });
        assert!(h.snapshot().iter().any(|t| t.name == "tick"));
    }

    #[test]
    fn layer_nested_spans() {
        let (sub, h) = frame_timing_subscriber();
        tracing::subscriber::with_default(sub, || {
            let _o = info_span!("outer").entered();
            {
                let _i = info_span!("inner").entered();
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        let snap = h.snapshot();
        let outer = snap.iter().find(|t| t.name == "outer");
        let inner = snap.iter().find(|t| t.name == "inner");
        let (Some(outer), Some(inner)) = (outer, inner) else {
            panic!("missing spans: {snap:?}");
        };
        assert_eq!(outer.count, 1);
        assert_eq!(inner.count, 1);
        assert!(outer.total >= inner.total);
    }
}
