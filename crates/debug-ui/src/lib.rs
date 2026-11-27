//! Debug overlay panel: draws an [`engine_diagnostics::DiagnosticsSnapshot`] with egui.
//!
//! Tier 4 crate. Pure `egui` plus the snapshot type: no GPU API and no windowing code, so the
//! panel and its tessellation are testable headless. Painting to a surface and input wiring live
//! in the application and backend crates, not here.

use egui::{Align2, ClippedPrimitive, Color32, Context, RawInput, Rect, Sense, Ui, Vec2, Window};
use engine_diagnostics::{DiagnosticsSnapshot, Histogram};

/// Height of one histogram, in points.
const HIST_HEIGHT: f32 = 36.0;
/// Width of one histogram bar, in points.
const BAR_WIDTH: f32 = 8.0;
/// Most bins drawn per histogram; extra bins are ignored.
const MAX_BINS: usize = 64;

/// Result of running the overlay for one headless frame.
pub struct FrameShapes {
    pub primitives: Vec<ClippedPrimitive>,
    /// Texture uploads the painter must apply (font atlas); call `clear()` to drop them
    /// deliberately, egui panics if they are dropped unapplied.
    pub textures_delta: egui::TexturesDelta,
    pub pixels_per_point: f32,
}

/// Text for a value; non-finite values print as `n/a`.
fn fmt_value(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.2}")
    } else {
        "n/a".to_string()
    }
}

/// Draw the snapshot into `ui`: frame/prepare ms, gauges, one progress bar per budget and one
/// bar histogram per histogram. Non-finite numbers are sanitised, never laid out.
pub fn panel(ui: &mut Ui, snapshot: &DiagnosticsSnapshot) {
    ui.label(format!(
        "frame {} ms  prepare {} ms",
        fmt_value(snapshot.frame_ms),
        fmt_value(snapshot.prepare_ms)
    ));
    for (label, value) in &snapshot.gauges {
        ui.label(format!("{label}: {}", fmt_value(*value)));
    }
    for budget in &snapshot.budgets {
        let text = format!(
            "{}: {} / {}",
            budget.label,
            fmt_value(budget.used),
            fmt_value(budget.cap)
        );
        // `fraction()` is finite and within [0, 1] by construction.
        ui.add(egui::ProgressBar::new(budget.fraction() as f32).text(text));
    }
    for hist in &snapshot.histograms {
        histogram(ui, hist);
    }
}

fn histogram(ui: &mut Ui, hist: &Histogram) {
    ui.label(&hist.label);
    let bins = &hist.bins[..hist.bins.len().min(MAX_BINS)];
    let width = BAR_WIDTH * bins.len().max(1) as f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, HIST_HEIGHT), Sense::hover());
    let max = bins.iter().copied().max().unwrap_or(0).max(1) as f32;
    let painter = ui.painter_at(rect);
    for (i, &count) in bins.iter().enumerate() {
        let h = HIST_HEIGHT * (count as f32 / max).clamp(0.0, 1.0);
        let x = rect.left() + BAR_WIDTH * i as f32;
        let bar = Rect::from_min_max(
            egui::pos2(x + 1.0, rect.bottom() - h),
            egui::pos2(x + BAR_WIDTH - 1.0, rect.bottom()),
        );
        painter.rect_filled(bar, 0.0, Color32::from_rgb(120, 180, 240));
    }
}

/// Show the overlay as a fixed, non-interactive window at the top-left of `ctx`.
pub fn overlay(ctx: &Context, snapshot: &DiagnosticsSnapshot) {
    Window::new("diagnostics")
        .title_bar(false)
        .resizable(false)
        .movable(false)
        .interactable(false)
        .collapsible(false)
        .anchor(Align2::LEFT_TOP, Vec2::new(8.0, 8.0))
        .show(ctx, |ui| panel(ui, snapshot));
}

/// Run one headless egui frame of the overlay and tessellate it. `size` is the screen size in
/// points and `time` the frame time in seconds; both are sanitised when not finite.
#[must_use]
pub fn frame_shapes(
    ctx: &Context,
    size: [f32; 2],
    time: f64,
    snapshot: &DiagnosticsSnapshot,
) -> FrameShapes {
    let sane = |v: f32| if v.is_finite() && v > 1.0 { v } else { 1.0 };
    let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(sane(size[0]), sane(size[1])));
    let time = if time.is_finite() { time } else { 0.0 };
    // egui lays a new window out invisibly on its first pass, so run two passes and keep the
    // second one's shapes; texture deltas of both passes are merged for the painter.
    let mut textures_delta = egui::TexturesDelta::default();
    let mut pixels_per_point = 1.0;
    let mut primitives = Vec::new();
    for pass in 0..2 {
        let input = RawInput {
            screen_rect: Some(screen),
            time: Some(time + f64::from(pass) / 60.0),
            ..RawInput::default()
        };
        let output = ctx.run_ui(input, |ui| overlay(ui.ctx(), snapshot));
        pixels_per_point = output.pixels_per_point;
        primitives = ctx.tessellate(output.shapes, pixels_per_point);
        textures_delta.append(output.textures_delta);
    }
    FrameShapes {
        primitives,
        textures_delta,
        pixels_per_point,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> DiagnosticsSnapshot {
        let mut s = DiagnosticsSnapshot::default()
            .gauge("drawn", 120.0)
            .budget("detail patches", 900.0, 1500.0)
            .histogram("LOD drawn", vec![0, 3, 9, 27]);
        s.frame_ms = 8.3;
        s.prepare_ms = 1.2;
        s
    }

    #[test]
    fn non_empty_snapshot_tessellates() {
        let ctx = Context::default();
        let mut out = frame_shapes(&ctx, [800.0, 600.0], 0.0, &snapshot());
        assert!(!out.primitives.is_empty());
        out.textures_delta.clear();
    }

    #[test]
    fn non_finite_values_do_not_panic() {
        let ctx = Context::default();
        let mut s = DiagnosticsSnapshot::default()
            .gauge("nan", f64::NAN)
            .gauge("inf", f64::INFINITY)
            .budget("b", f64::NAN, f64::INFINITY)
            .budget("zero", 1.0, 0.0)
            .histogram("big", vec![u32::MAX, 0, 1])
            .histogram("empty", Vec::new())
            .histogram("long", vec![1; 10_000]);
        s.frame_ms = f64::NEG_INFINITY;
        s.prepare_ms = f64::NAN;
        let mut out = frame_shapes(&ctx, [f32::NAN, f32::INFINITY], f64::NAN, &s);
        assert!(!out.primitives.is_empty());
        out.textures_delta.clear();
    }
}
