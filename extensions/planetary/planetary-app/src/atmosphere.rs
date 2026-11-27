//! Minimal atmosphere (PLAN-08): a sun-tinted sky colour and an opaque camera-facing limb ring.
//!
//! Everything here is pure arithmetic on plain values. The engine offers no shader hook and only
//! opaque (`REPLACE`) blending, so the glow is a mesh of `VertexPosColor` vertices drawn with the
//! existing unlit pipeline; its outer edge carries exactly the clear colour so the ring fades
//! seamlessly into the background (ADR-0019). Every planet number lives in this module.

use engine_sdk::{DVec3, Quat, Vec3, VertexPosColor};

use crate::{
    HAZE_FULL_ALTITUDE, HAZE_NONE_ALTITUDE, Haze, SKY_COLOR, haze_for_altitude, haze_with_sky,
};

/// Height of the visible atmosphere above the surface (m).
pub const ATMOSPHERE_HEIGHT: f64 = 100_000.0;
/// The ring's radial thickness is `GLOW_SPREAD` times the real atmosphere height (the real height
/// would be a sub-pixel sliver from orbit).
pub const GLOW_SPREAD: f64 = 2.0;
/// Segments of the ring annulus (each is two triangles, so `6 * RING_SEGMENTS` vertices).
pub const RING_SEGMENTS: u32 = 64;
/// Upper bound on the segment count accepted by [`ring_vertices`].
pub const MAX_RING_SEGMENTS: u32 = 1024;
/// The ring's inner edge is pulled in by this factor so terrain relief overlaps (and occludes) it.
pub const RING_INNER_SCALE: f64 = 0.98;
/// A camera closer than `r * (1 + RING_EPS)` to the centre has no ring (the tangent cone
/// degenerates).
pub const RING_EPS: f64 = 1.0e-6;
/// Twilight colour (linear RGB) blended into the sky near the terminator.
pub const WARM_COLOR: [f32; 3] = [0.95, 0.50, 0.25];
/// Sine of the sun elevation at and above which the sky is the plain day colour.
pub const DAY_SIN: f64 = 0.30;
/// Sine of the sun elevation at and below which the sky is at night brightness.
pub const NIGHT_SIN: f64 = -0.10;
/// Sky brightness multiplier at night.
pub const NIGHT_SCALE: f32 = 0.15;
/// Largest fraction of [`WARM_COLOR`] mixed in (at the horizon sun).
pub const WARM_STRENGTH: f32 = 0.6;

/// Radius of the ring's tangent cone seen in the plane through the planet centre perpendicular to
/// the view axis: `r_in = d * r / sqrt(d^2 - r^2)` for a camera at distance `d` from the centre of
/// a sphere of radius `r`. Written as `r / sqrt(1 - (r/d)^2)` so huge `d` cannot overflow.
///
/// `None` for non-finite input, `r <= 0`, or `d <= r * (1 + RING_EPS)`.
#[must_use]
pub fn ring_inner_radius(d: f64, r: f64) -> Option<f64> {
    if !(d.is_finite() && r.is_finite() && r > 0.0) || d <= r * (1.0 + RING_EPS) {
        return None;
    }
    let t = r / d;
    let denom = (1.0 - t * t).sqrt();
    let out = r / denom;
    (out.is_finite() && out > 0.0).then_some(out)
}

/// Outer radius of the unit ring (inner radius 1) for a planet of the given outer `radius`.
#[must_use]
pub fn ring_outer_ratio(radius: f64) -> f64 {
    if radius.is_finite() && radius > 0.0 {
        1.0 + GLOW_SPREAD * ATMOSPHERE_HEIGHT / radius
    } else {
        1.0
    }
}

/// Ring visibility for a camera altitude: 0 at and below [`HAZE_FULL_ALTITUDE`] (fog and clear
/// colour take over), 1 at and above [`HAZE_NONE_ALTITUDE`], linear in between. Non-finite
/// altitude gives 0.
#[must_use]
pub fn ring_alpha(altitude: f64) -> f32 {
    if !altitude.is_finite() {
        return 0.0;
    }
    let t = (altitude - HAZE_FULL_ALTITUDE) / (HAZE_NONE_ALTITUDE - HAZE_FULL_ALTITUDE);
    t.clamp(0.0, 1.0) as f32
}

/// Sky colour (linear RGB) for a local vertical `up` and the light travel direction `light`
/// (from the sun towards the planet). At and above [`DAY_SIN`] the sine of the sun elevation the
/// result is exactly [`SKY_COLOR`]; below it the sky warms towards [`WARM_COLOR`] near the horizon
/// sun and dims to [`NIGHT_SCALE`] when the sun is down. Degenerate or non-finite vectors give
/// [`SKY_COLOR`].
#[must_use]
pub fn sky_color(up: DVec3, light: Vec3) -> [f32; 3] {
    let ul = up.length();
    let ll = f64::from(light.length());
    if !(ul.is_finite() && ul > 0.0 && ll.is_finite() && ll > 0.0) {
        return SKY_COLOR;
    }
    let to_sun = DVec3::new(
        -f64::from(light.x),
        -f64::from(light.y),
        -f64::from(light.z),
    ) / ll;
    let s = (up / ul).dot(to_sun);
    if !s.is_finite() || s >= DAY_SIN {
        return SKY_COLOR;
    }
    let bright_t = ((s - NIGHT_SIN) / (DAY_SIN - NIGHT_SIN)).clamp(0.0, 1.0) as f32;
    let brightness = NIGHT_SCALE + (1.0 - NIGHT_SCALE) * bright_t;
    let warm = WARM_STRENGTH * (1.0 - (s.abs() / DAY_SIN).clamp(0.0, 1.0) as f32);
    let mut out = [0.0; 3];
    for k in 0..3 {
        out[k] = (SKY_COLOR[k] * (1.0 - warm) + WARM_COLOR[k] * warm) * brightness;
    }
    out
}

/// Haze and sky colour for a camera at `altitude` whose local vertical is `up`, lit by `light`.
/// Same density and altitude fade as [`haze_for_altitude`]; only the sky colour (fog colour and
/// the clear colour's sky share, kept equal so the horizon matches the sky) gets the sun-elevation
/// tint. With the sun above [`DAY_SIN`] (and for degenerate vectors) the result is bit-identical
/// to [`haze_for_altitude`].
#[must_use]
pub fn atmosphere_for_view(altitude: f64, light: Vec3, up: DVec3) -> Haze {
    let sky = sky_color(up, light);
    if sky == SKY_COLOR {
        haze_for_altitude(altitude)
    } else {
        haze_with_sky(altitude, sky)
    }
}

/// Triangle-list mesh of the annulus in the plane `z = 0`, facing `+Z`: inner radius 1, outer
/// radius `outer_ratio` (at least 1). `6 * segments` vertices (`segments` clamped to
/// `3..=MAX_RING_SEGMENTS`). The vertex at unit direction `(c, s)` on the inner edge gets
/// `inner(c, s)`; every outer-edge vertex gets exactly `outer`. A non-finite `outer_ratio` gives
/// 1.
#[must_use]
pub fn ring_vertices(
    segments: u32,
    outer_ratio: f32,
    inner: &dyn Fn(f32, f32) -> [f32; 3],
    outer: [f32; 3],
) -> Vec<VertexPosColor> {
    let n = segments.clamp(3, MAX_RING_SEGMENTS);
    let ratio = if outer_ratio.is_finite() {
        outer_ratio.max(1.0)
    } else {
        1.0
    };
    let dir = |i: u32| {
        let a = (i % n) as f32 / n as f32 * std::f32::consts::TAU;
        (a.cos(), a.sin())
    };
    let vtx = |i: u32, radius: f32, color: [f32; 3]| {
        let (c, s) = dir(i);
        VertexPosColor {
            position: [c * radius, s * radius, 0.0],
            color,
        }
    };
    let mut out = Vec::with_capacity(6 * n as usize);
    for i in 0..n {
        let (c0, s0) = dir(i);
        let (c1, s1) = dir(i + 1);
        let in0 = vtx(i, 1.0, inner(c0, s0));
        let in1 = vtx(i + 1, 1.0, inner(c1, s1));
        let out0 = vtx(i, ratio, outer);
        let out1 = vtx(i + 1, ratio, outer);
        out.extend_from_slice(&[in0, out0, out1, in0, out1, in1]);
    }
    out
}

/// The ring for one frame: item rotation and scale plus the recoloured unit-ring vertices.
#[derive(Clone, Debug, PartialEq)]
pub struct RingFrame {
    /// Rotation taking the ring's `+Z` to the planet-centre-to-camera direction.
    pub rotation: Quat,
    /// Uniform scale of the unit ring (`RING_INNER_SCALE * r_in`).
    pub scale: f32,
    /// Triangle-list vertices of the unit ring (see [`ring_vertices`]).
    pub vertices: Vec<VertexPosColor>,
}

/// Ring for a camera at `cam` (planet-centre relative, metres) over a planet of outer `radius`
/// (nominal radius plus maximum relief) at `altitude` above the surface. `light` is the light
/// travel direction and `clear` the frame's clear colour; the outer edge equals `clear` exactly
/// and the inner edge is the sky colour at that limb point, mixed towards `clear` by
/// [`ring_alpha`]. `None` (no ring) when the ring is invisible, the camera is inside the tangent
/// cone's validity range, or any number is non-finite.
#[must_use]
pub fn ring_frame(
    cam: DVec3,
    radius: f64,
    altitude: f64,
    light: Vec3,
    clear: [f32; 4],
) -> Option<RingFrame> {
    let alpha = ring_alpha(altitude);
    if alpha <= 0.0 || clear.iter().any(|c| !c.is_finite()) {
        return None;
    }
    let d = cam.length();
    let r_in = ring_inner_radius(d, radius)?;
    let scale = (r_in * RING_INNER_SCALE) as f32;
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let dir = (cam / d).as_vec3();
    if !dir.is_finite() {
        return None;
    }
    let rotation = Quat::from_rotation_arc(Vec3::Z, dir);
    let clear_rgb = [clear[0], clear[1], clear[2]];
    let inner = |c: f32, s: f32| {
        let u = rotation * Vec3::new(c, s, 0.0);
        let glow = sky_color(
            DVec3::new(f64::from(u.x), f64::from(u.y), f64::from(u.z)),
            light,
        );
        [
            clear_rgb[0] * (1.0 - alpha) + glow[0] * alpha,
            clear_rgb[1] * (1.0 - alpha) + glow[1] * alpha,
            clear_rgb[2] * (1.0 - alpha) + glow[2] * alpha,
        ]
    };
    let vertices = ring_vertices(
        RING_SEGMENTS,
        ring_outer_ratio(radius) as f32,
        &inner,
        clear_rgb,
    );
    Some(RingFrame {
        rotation,
        scale,
        vertices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HAZE_DENSITY, SPACE_COLOR};

    const R: f64 = 6_371_000.0;

    fn travel_from_elevation(up: DVec3, deg: f64) -> Vec3 {
        // Sun at `deg` above the horizon, azimuth towards +Y (up is not parallel to Y here).
        let east = DVec3::Y.cross(up).normalize();
        let north = up.cross(east);
        let e = deg.to_radians();
        let to_sun = up * e.sin() + north * e.cos();
        Vec3::new(-to_sun.x as f32, -to_sun.y as f32, -to_sun.z as f32)
    }

    #[test]
    fn ring_radius_is_the_tangent_cone_radius() {
        for k in [1.0001, 1.01, 1.5, 2.0, 10.0, 1000.0] {
            let d = R * k;
            let r_in = ring_inner_radius(d, R).unwrap();
            let want = d * R / (d * d - R * R).sqrt();
            assert!((r_in - want).abs() <= want * 1e-9, "{k}: {r_in} vs {want}");
            // r_in / d is tan(asin(R/d)): the tangent cone's half angle.
            let tan = (R / d).asin().tan();
            assert!((r_in / d - tan).abs() <= tan * 1e-9, "{k}");
            assert!(r_in > R);
        }
        let two = ring_inner_radius(2.0 * R, R).unwrap();
        assert!((two - 2.0 * R / 3.0_f64.sqrt()).abs() < 1e-3);
    }

    #[test]
    fn ring_radius_decreases_towards_the_planet_radius_with_distance() {
        let mut last = f64::INFINITY;
        let mut d = R * 1.0001;
        while d < R * 1.0e6 {
            let r = ring_inner_radius(d, R).unwrap();
            assert!(
                r.is_finite() && r > R * (1.0 - 1e-12) && r <= last,
                "{d}: {r}"
            );
            last = r;
            d *= 1.3;
        }
        assert!((ring_inner_radius(1.0e300, R).unwrap() - R).abs() < 1e-6);
    }

    #[test]
    fn no_ring_at_or_inside_the_surface_and_for_bad_input() {
        assert_eq!(ring_inner_radius(R, R), None);
        assert_eq!(ring_inner_radius(0.5 * R, R), None);
        assert_eq!(ring_inner_radius(R * (1.0 + 1e-12), R), None);
        assert_eq!(ring_inner_radius(0.0, R), None);
        assert_eq!(ring_inner_radius(-5.0, R), None);
        assert_eq!(ring_inner_radius(2.0 * R, 0.0), None);
        assert_eq!(ring_inner_radius(2.0 * R, -R), None);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(ring_inner_radius(bad, R), None);
            assert_eq!(ring_inner_radius(2.0 * R, bad), None);
        }
        // Just outside the epsilon the radius is finite.
        let r = ring_inner_radius(R * (1.0 + 1e-5), R).unwrap();
        assert!(r.is_finite() && r > R);
    }

    #[test]
    fn ring_alpha_bands_and_monotone() {
        assert_eq!(ring_alpha(HAZE_FULL_ALTITUDE), 0.0);
        assert_eq!(ring_alpha(0.0), 0.0);
        assert_eq!(ring_alpha(-1.0e6), 0.0);
        assert_eq!(ring_alpha(HAZE_NONE_ALTITUDE), 1.0);
        assert_eq!(ring_alpha(1.0e12), 1.0);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(ring_alpha(bad), 0.0);
        }
        let mut last = 0.0;
        let mut a = -1000.0;
        while a < 400_000.0 {
            let v = ring_alpha(a);
            assert!((0.0..=1.0).contains(&v) && v >= last, "{a}: {v}");
            last = v;
            a += 997.0;
        }
    }

    #[test]
    fn ring_outer_ratio_is_finite_and_at_least_one() {
        let o = ring_outer_ratio(R);
        assert!(o > 1.0 && o < 1.1, "{o}");
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(ring_outer_ratio(bad), 1.0);
        }
    }

    #[test]
    fn ring_vertex_count_and_edge_colours() {
        let inner = |_c: f32, _s: f32| [0.1, 0.2, 0.3];
        let outer = [0.5, 0.6, 0.7];
        let v = ring_vertices(RING_SEGMENTS, 1.2, &inner, outer);
        assert_eq!(v.len(), 6 * RING_SEGMENTS as usize);
        let mut inner_n = 0;
        let mut outer_n = 0;
        for p in &v {
            assert!(p.position.iter().all(|x| x.is_finite()));
            assert_eq!(p.position[2], 0.0);
            let rad = p.position[0].hypot(p.position[1]);
            if (rad - 1.0).abs() < 1e-5 {
                inner_n += 1;
                assert_eq!(p.color, [0.1, 0.2, 0.3]);
            } else {
                outer_n += 1;
                assert!((rad - 1.2).abs() < 1e-5, "{rad}");
                assert_eq!(p.color, outer);
            }
        }
        assert_eq!(inner_n, 3 * RING_SEGMENTS as usize);
        assert_eq!(outer_n, 3 * RING_SEGMENTS as usize);
    }

    #[test]
    fn ring_vertices_clamp_segments_and_bad_ratio() {
        let inner = |_: f32, _: f32| [0.0; 3];
        assert_eq!(ring_vertices(0, 1.5, &inner, [0.0; 3]).len(), 18);
        assert_eq!(
            ring_vertices(u32::MAX, 1.5, &inner, [0.0; 3]).len(),
            6 * MAX_RING_SEGMENTS as usize
        );
        for ratio in [f32::NAN, f32::INFINITY, -3.0, 0.2] {
            let v = ring_vertices(8, ratio, &inner, [0.0; 3]);
            assert!(v.iter().all(|p| p.position.iter().all(|x| x.is_finite())));
        }
    }

    #[test]
    fn ring_frame_outer_edge_is_exactly_the_clear_colour() {
        let light = Vec3::new(-0.3, -0.5, -0.8).normalize();
        for alt in [
            25_000.0, 100_000.0, 150_000.0, 200_000.0, 1.0e6, 1.0e9, 1.0e12,
        ] {
            let cam = DVec3::new(R + alt, 0.2 * alt, -0.1 * alt);
            let h = haze_for_altitude(alt);
            let f = ring_frame(cam, R + 9000.0, alt, light, h.clear_color).unwrap();
            assert!(f.scale.is_finite() && f.scale > 0.0, "{alt}");
            assert_eq!(f.vertices.len(), 6 * RING_SEGMENTS as usize);
            let mut saw_outer = false;
            for v in &f.vertices {
                assert!(v.position.iter().all(|x| x.is_finite()), "{alt}");
                assert!(v.color.iter().all(|x| x.is_finite() && *x >= 0.0), "{alt}");
                if v.position[0].hypot(v.position[1]) > 1.001 {
                    saw_outer = true;
                    assert_eq!(
                        v.color,
                        [h.clear_color[0], h.clear_color[1], h.clear_color[2]]
                    );
                }
            }
            assert!(saw_outer);
            let q = f.rotation * Vec3::Z;
            let want = (cam / cam.length()).as_vec3();
            assert!((q - want).length() < 1e-4, "{alt}");
        }
    }

    #[test]
    fn ring_frame_scale_follows_the_tangent_cone() {
        let cam = DVec3::new(2.0 * R, 0.0, 0.0);
        let alt = 2.0 * R - R;
        let f = ring_frame(cam, R, alt, Vec3::NEG_Y, SPACE_COLOR).unwrap();
        let want = ring_inner_radius(2.0 * R, R).unwrap() * RING_INNER_SCALE;
        assert!((f64::from(f.scale) - want).abs() < want * 1e-6);
    }

    #[test]
    fn ring_frame_is_none_when_invisible_or_degenerate() {
        let l = Vec3::NEG_Y;
        let at = |alt: f64| ring_frame(DVec3::new(R + alt, 0.0, 0.0), R, alt, l, SPACE_COLOR);
        assert!(at(0.0).is_none());
        assert!(at(HAZE_FULL_ALTITUDE).is_none());
        assert!(at(HAZE_FULL_ALTITUDE + 1.0).is_some());
        assert!(at(f64::NAN).is_none());
        // Camera at or inside the radius: no ring even with a forced altitude.
        assert!(ring_frame(DVec3::new(0.5 * R, 0.0, 0.0), R, 1.0e6, l, SPACE_COLOR).is_none());
        assert!(ring_frame(DVec3::ZERO, R, 1.0e6, l, SPACE_COLOR).is_none());
        assert!(ring_frame(DVec3::new(f64::NAN, 0.0, 0.0), R, 1.0e6, l, SPACE_COLOR).is_none());
        assert!(ring_frame(DVec3::new(2.0 * R, 0.0, 0.0), R, 1.0e6, l, [f32::NAN; 4]).is_none());
    }

    #[test]
    fn ring_frame_is_deterministic() {
        let cam = DVec3::new(1.5 * R, 0.3 * R, 0.1 * R);
        let l = Vec3::new(-0.2, -0.4, -0.9).normalize();
        let a = ring_frame(cam, R, 2.0e6, l, SPACE_COLOR);
        assert!(a.is_some());
        assert_eq!(a, ring_frame(cam, R, 2.0e6, l, SPACE_COLOR));
    }

    #[test]
    fn high_sun_sky_equals_the_haze() {
        let up = DVec3::new(1.0, 0.0, 0.0);
        let light = travel_from_elevation(up, 35.0);
        for alt in [-10.0, 0.0, 20_000.0, 100_000.0, 200_000.0, 1.0e7] {
            assert_eq!(atmosphere_for_view(alt, light, up), haze_for_altitude(alt));
        }
        assert_eq!(sky_color(up, light), SKY_COLOR);
    }

    #[test]
    fn degenerate_vectors_give_the_plain_haze() {
        let l = Vec3::new(0.0, -1.0, 0.0);
        for (up, light) in [
            (DVec3::ZERO, l),
            (DVec3::X, Vec3::ZERO),
            (DVec3::new(f64::NAN, 0.0, 0.0), l),
            (DVec3::X, Vec3::new(f32::NAN, 0.0, 0.0)),
            (DVec3::new(f64::INFINITY, 0.0, 0.0), l),
        ] {
            assert_eq!(sky_color(up, light), SKY_COLOR);
            assert_eq!(
                atmosphere_for_view(50_000.0, light, up),
                haze_for_altitude(50_000.0)
            );
        }
    }

    #[test]
    fn low_sun_tints_warm_and_the_night_sky_is_dim() {
        let up = DVec3::X;
        let sky_at = |deg: f64| sky_color(up, travel_from_elevation(up, deg));
        let horizon = sky_at(0.0);
        // Warmer than the day sky: more red, less blue.
        assert!(horizon[0] > SKY_COLOR[0] * NIGHT_SCALE);
        assert!(horizon[2] < SKY_COLOR[2]);
        let night = sky_at(-60.0);
        for k in 0..3 {
            assert!(night[k] > 0.0 && night[k] < SKY_COLOR[k], "{night:?}");
            assert!(night[k] <= SKY_COLOR[k] * NIGHT_SCALE + WARM_COLOR[k] * NIGHT_SCALE);
        }
        // Brightness (sum) is monotone non-decreasing in elevation from night to the day threshold.
        let mut last = 0.0f32;
        for deg in (-90..=17).step_by(1) {
            let c = sky_at(f64::from(deg));
            let b = c[0] + c[1] + c[2];
            assert!(b.is_finite() && b >= 0.0);
            if deg >= 0 {
                assert!(b >= last - 1e-6, "{deg}: {b} after {last}");
            }
            last = b;
        }
    }

    #[test]
    fn tint_keeps_the_horizon_equal_to_the_sky_and_the_fog_valid() {
        let up = DVec3::new(0.3, 0.9, 0.1).normalize();
        for deg in [-80.0, -10.0, 0.0, 5.0, 12.0, 17.0, 30.0] {
            let light = travel_from_elevation(up, deg);
            for alt in [
                -50.0, 0.0, 10_000.0, 20_000.0, 60_000.0, 150_000.0, 200_000.0, 1.0e9,
            ] {
                let h = atmosphere_for_view(alt, light, up);
                assert!(h.fog.validate().is_ok(), "{deg} {alt}");
                assert!(h.clear_color.iter().all(|c| c.is_finite()));
                assert_eq!(h.clear_color[3], 1.0);
                let base = haze_for_altitude(alt);
                // Same density and fade as the haze.
                assert_eq!(h.fog.density, base.fog.density);
                let fade = h.fog.density / HAZE_DENSITY;
                for (k, space) in SPACE_COLOR.iter().take(3).enumerate() {
                    let want = h.fog.color[k] * fade + space * (1.0 - fade);
                    assert!((h.clear_color[k] - want).abs() < 1e-5, "{deg} {alt}");
                }
            }
        }
        // Full haze: fog colour is exactly the clear colour.
        let light = travel_from_elevation(up, 3.0);
        let h = atmosphere_for_view(0.0, light, up);
        assert_eq!(
            h.fog.color,
            [h.clear_color[0], h.clear_color[1], h.clear_color[2]]
        );
        // Space: the clear colour is the space colour whatever the tint.
        assert_eq!(
            atmosphere_for_view(1.0e9, light, up).clear_color,
            SPACE_COLOR
        );
    }

    #[test]
    fn atmosphere_is_nan_safe_in_altitude() {
        let up = DVec3::X;
        let light = travel_from_elevation(up, 2.0);
        for a in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let h = atmosphere_for_view(a, light, up);
            assert_eq!(h.fog.density, 0.0);
            assert_eq!(h.clear_color, SPACE_COLOR);
            assert!(h.fog.validate().is_ok());
        }
    }
}
