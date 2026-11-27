//! Read-only debug overlay host (feature `egui-overlay`): runs `debug_ui::frame_shapes` for the
//! snapshot an app returns from `App::overlay_frame` and stages it on the wgpu backend.

use engine_diagnostics::DiagnosticsSnapshot;
use engine_render::RenderBackend;
use engine_render_wgpu::{OverlayData, WgpuBackend};

use crate::app::App;

/// Owns the egui context and the overlay clock.
#[derive(Default)]
pub(crate) struct OverlayHost {
    ctx: egui::Context,
    time: f64,
}

impl OverlayHost {
    /// Tessellate `snapshot` for a `size` pixel target (1 point = 1 pixel) and advance the clock.
    pub(crate) fn build(
        &mut self,
        size: (u32, u32),
        dt: f64,
        snapshot: &DiagnosticsSnapshot,
    ) -> OverlayData {
        if dt.is_finite() && dt > 0.0 {
            self.time += dt;
        }
        let shapes = debug_ui::frame_shapes(
            &self.ctx,
            [size.0 as f32, size.1 as f32],
            self.time,
            snapshot,
        );
        OverlayData {
            primitives: shapes.primitives,
            textures_delta: shapes.textures_delta,
            pixels_per_point: shapes.pixels_per_point,
        }
    }

    /// Ask the app for this frame's overlay and stage it on the backend.
    pub(crate) fn stage(&mut self, backend: &mut WgpuBackend, app: &mut dyn App, dt: f64) {
        if let Some(snapshot) = app.overlay_frame() {
            let data = self.build(backend.size(), dt, &snapshot);
            backend.set_overlay(data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_becomes_overlay_data() {
        let snap = DiagnosticsSnapshot::default()
            .gauge("drawn", 12.0)
            .budget("detail", 3.0, 10.0);
        let mut host = OverlayHost::default();
        let mut data = host.build((320, 200), 0.016, &snap);
        assert!(!data.primitives.is_empty());
        assert!(!data.textures_delta.is_empty());
        assert!(data.pixels_per_point > 0.0);
        data.textures_delta.clear();
    }

    #[test]
    fn non_finite_dt_and_size_do_not_panic() {
        let mut host = OverlayHost::default();
        let mut data = host.build((0, 0), f64::NAN, &DiagnosticsSnapshot::default());
        data.textures_delta.clear();
        assert!(host.time.is_finite());
    }
}
