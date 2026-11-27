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

/// Text for a gauge or budget number: whole numbers print as integers (`593`), fractional ones
/// keep two decimals, non-finite values print `n/a`. A ratio that is exactly `1.0` therefore
/// prints as `1`; times use [`fmt_ms`] and always keep decimals.
fn fmt_count(v: f64) -> String {
    if !v.is_finite() {
        "n/a".to_string()
    } else if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v:.2}")
    }
}

/// Text for a duration in milliseconds: always two decimals, non-finite prints `n/a`.
fn fmt_ms(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.2}")
    } else {
        "n/a".to_string()
    }
}

/// Header line of the panel (frame and prepare times).
fn header_text(snapshot: &DiagnosticsSnapshot) -> String {
    format!(
        "frame {} ms  prepare {} ms",
        fmt_ms(snapshot.frame_ms),
        fmt_ms(snapshot.prepare_ms)
    )
}

/// Text of one gauge row.
fn gauge_text(label: &str, value: f64) -> String {
    format!("{label}: {}", fmt_count(value))
}

/// Draw the snapshot into `ui`: frame/prepare ms, gauges, one progress bar per budget and one
/// bar histogram per histogram. Non-finite numbers are sanitised, never laid out.
pub fn panel(ui: &mut Ui, snapshot: &DiagnosticsSnapshot) {
    ui.label(header_text(snapshot));
    for (label, value) in &snapshot.gauges {
        ui.label(gauge_text(label, *value));
    }
    for budget in &snapshot.budgets {
        let text = format!(
            "{}: {} / {}",
            budget.label,
            fmt_count(budget.used),
            fmt_count(budget.cap)
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
/// physical pixels, `scale_factor` the window scale (pixels per point; invalid values fall back
/// to 1.0, capped at 8.0) and `time` the frame time in seconds; all are sanitised when not
/// finite.
#[must_use]
pub fn frame_shapes(
    ctx: &Context,
    size: [f32; 2],
    scale_factor: f32,
    time: f64,
    snapshot: &DiagnosticsSnapshot,
) -> FrameShapes {
    frame_shapes_observed(ctx, size, scale_factor, time, snapshot, |_| {})
}

/// [`frame_shapes`] that reports the clock egui sees at the start of each pass to `on_pass`.
fn frame_shapes_observed(
    ctx: &Context,
    size: [f32; 2],
    scale_factor: f32,
    time: f64,
    snapshot: &DiagnosticsSnapshot,
    mut on_pass: impl FnMut(f64),
) -> FrameShapes {
    let sane = |v: f32| if v.is_finite() && v > 1.0 { v } else { 1.0 };
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor.min(8.0)
    } else {
        1.0
    };
    ctx.set_pixels_per_point(scale);
    let screen = Rect::from_min_size(
        egui::Pos2::ZERO,
        Vec2::new(sane(size[0]) / scale, sane(size[1]) / scale),
    );
    let time = if time.is_finite() { time } else { 0.0 };
    // egui lays a new window out invisibly on its first pass, so run two passes and keep the
    // second one's shapes; texture deltas of both passes are merged for the painter. Both passes
    // use the same `time`: offsetting the second by a fixed step would run egui's clock ahead of
    // the caller's next frame whenever its dt is shorter than that step, so time went backwards.
    let mut textures_delta = egui::TexturesDelta::default();
    let mut pixels_per_point = 1.0;
    let mut primitives = Vec::new();
    for _pass in 0..2 {
        let input = RawInput {
            screen_rect: Some(screen),
            time: Some(time),
            ..RawInput::default()
        };
        let output = ctx.run_ui(input, |ui| {
            on_pass(ui.ctx().input(|i| i.time));
            overlay(ui.ctx(), snapshot);
        });
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
            .budget("held meshes (base+detail)", 900.0, 1500.0)
            .histogram("LOD drawn", vec![0, 3, 9, 27]);
        s.frame_ms = 8.3;
        s.prepare_ms = 1.2;
        s
    }

    #[test]
    fn non_empty_snapshot_tessellates() {
        let ctx = Context::default();
        let mut out = frame_shapes(&ctx, [800.0, 600.0], 1.0, 0.0, &snapshot());
        assert!(!out.primitives.is_empty());
        out.textures_delta.clear();
    }

    #[test]
    fn clock_never_goes_backwards_at_144hz() {
        let ctx = Context::default();
        let dt = 1.0 / 144.0;
        let snap = snapshot();
        let mut time = 0.0;
        let mut last = f64::NEG_INFINITY;
        let mut passes = 0;
        for _ in 0..200 {
            time += dt;
            let mut out = frame_shapes_observed(&ctx, [800.0, 600.0], 1.0, time, &snap, |t| {
                assert!(t >= last, "egui clock went backwards: {last} -> {t}");
                last = t;
                passes += 1;
            });
            out.textures_delta.clear();
        }
        assert_eq!(passes, 400);
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
        let mut out = frame_shapes(&ctx, [f32::NAN, f32::INFINITY], f32::NAN, f64::NAN, &s);
        assert!(!out.primitives.is_empty());
        out.textures_delta.clear();
    }

    #[test]
    fn counts_are_integers_times_keep_decimals() {
        assert_eq!(fmt_count(593.0), "593");
        assert_eq!(fmt_count(0.0), "0");
        assert_eq!(fmt_count(2.5), "2.50");
        assert_eq!(fmt_count(-3.0), "-3");
        assert_eq!(fmt_count(f64::NAN), "n/a");
        assert_eq!(fmt_count(f64::INFINITY), "n/a");
        let big = fmt_count(1e20);
        assert!(
            big.contains('.'),
            "large values must not use a lossy cast: {big}"
        );
        assert_eq!(fmt_ms(8.3), "8.30");
        assert_eq!(fmt_ms(8.0), "8.00");
        assert_eq!(fmt_ms(f64::NAN), "n/a");
    }

    #[test]
    fn row_text_uses_integer_counts() {
        assert_eq!(gauge_text("selected", 593.0), "selected: 593");
        assert_eq!(header_text(&snapshot()), "frame 8.30 ms  prepare 1.20 ms");
    }

    #[test]
    fn scale_factor_scales_layout_and_ppp() {
        let first_x = |scale: f32, size: [f32; 2]| {
            let ctx = Context::default();
            let mut out = frame_shapes(&ctx, size, scale, 0.0, &snapshot());
            out.textures_delta.clear();
            let x = out
                .primitives
                .iter()
                .find_map(|p| match &p.primitive {
                    egui::epaint::Primitive::Mesh(m) => m.vertices.first().map(|v| v.pos.x),
                    egui::epaint::Primitive::Callback(_) => None,
                })
                .expect("a mesh");
            (out.pixels_per_point, x)
        };
        let (p1, x1) = first_x(1.0, [800.0, 600.0]);
        let (p2, x2) = first_x(2.0, [1600.0, 1200.0]);
        assert!((p1 - 1.0).abs() < 1e-6);
        assert!((p2 - 2.0).abs() < 1e-6);
        // Tessellated vertices are in points (the painter multiplies by pixels_per_point), so
        // the same 800x600-point screen gives the same point layout at both scales.
        assert!((x2 - x1).abs() < 0.5, "x1 {x1} x2 {x2}");
        for bad in [f32::NAN, 0.0, -2.0, f32::INFINITY] {
            let (p, _) = first_x(bad, [800.0, 600.0]);
            assert!((p - 1.0).abs() < 1e-6, "scale {bad} -> ppp {p}");
        }
    }
}
