//! Exact-op cell distance, horizon and frustum culling for the cube-sphere quadtree.
//!
//! Only `+ - * /`, `sqrt` and comparisons are used. A cell is described by its [`Footprint`]
//! (a cone around its center direction that contains the whole patch) extruded radially between
//! a [`HeightRange`]; this is much tighter than a bounding sphere inflated by the global relief.

use core::f64::consts::FRAC_PI_2;

use engine_sdk::DVec3;

use crate::lod::model::{Bound, View};
use crate::mapping::{lattice_dir, patch_center_dir, patch_corner_dirs};
use crate::patch_id::PatchId;

/// Allowance (in cosine) for the curved patch border and rounding of the cone.
const CONE_MARGIN: f64 = 1e-12;

/// Closed interval of heights above the reference radius (meters).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeightRange {
    pub min: f64,
    pub max: f64,
}

impl HeightRange {
    /// Range widened by `by` on both sides.
    #[must_use]
    pub fn widen(self, by: f64) -> HeightRange {
        HeightRange {
            min: self.min - by,
            max: self.max + by,
        }
    }

    /// Smallest range containing both.
    #[must_use]
    pub fn union(self, o: HeightRange) -> HeightRange {
        HeightRange {
            min: if o.min < self.min { o.min } else { self.min },
            max: if o.max > self.max { o.max } else { self.max },
        }
    }

    /// Intersection, falling back to `self` when the ranges are disjoint.
    #[must_use]
    pub fn clamp_to(self, o: HeightRange) -> HeightRange {
        let min = if self.min > o.min { self.min } else { o.min };
        let max = if self.max < o.max { self.max } else { o.max };
        if min <= max {
            HeightRange { min, max }
        } else {
            o
        }
    }
}

/// Cone containing a patch: unit `center` direction and the cosine/sine of its half angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub center: DVec3,
    pub cos_delta: f64,
    pub sin_delta: f64,
}

fn dot(a: DVec3, b: DVec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// Footprint of `id`: smallest cone around the center direction through the 4 corners and the
/// 4 edge midpoints (lattice points of level + 1), lowered by a tiny margin.
#[must_use]
pub fn patch_footprint(id: PatchId) -> Footprint {
    let c = patch_center_dir(id);
    let mut cos_delta = 1.0f64;
    for d in patch_corner_dirs(id) {
        let k = dot(d, c);
        if k < cos_delta {
            cos_delta = k;
        }
    }
    let (x, y) = id.xy();
    let (x, y) = (u64::from(x), u64::from(y));
    let g = 1u64 << (id.level() + 1);
    let f = id.face();
    let mids = [
        (2 * x + 1, 2 * y),
        (2 * x + 2, 2 * y + 1),
        (2 * x + 1, 2 * y + 2),
        (2 * x, 2 * y + 1),
    ];
    for (gu, gv) in mids {
        let k = dot(lattice_dir(f, gu, gv, g), c);
        if k < cos_delta {
            cos_delta = k;
        }
    }
    cos_delta -= CONE_MARGIN;
    let s2 = 1.0 - cos_delta * cos_delta;
    let sin_delta = if s2 > 0.0 { s2.sqrt() } else { 0.0 };
    Footprint {
        center: c,
        cos_delta,
        sin_delta,
    }
}

/// `(cos_t, cos_phi)` of the eye direction against the cone: `cos_t` is the cosine of the angle
/// between the eye direction and the cone axis, `cos_phi` the cosine of the angle between the eye
/// direction and the nearest cone boundary direction.
fn cone_angles(e: DVec3, fp: &Footprint) -> (f64, f64) {
    let cos_t = dot(e, fp.center);
    let s2 = 1.0 - cos_t * cos_t;
    let sin_t = if s2 > 0.0 { s2.sqrt() } else { 0.0 };
    (cos_t, cos_t * fp.cos_delta + sin_t * fp.sin_delta)
}

/// Whether the eye direction lies inside the footprint cone (the cell under the camera).
#[must_use]
pub fn eye_in_cone(eye: DVec3, fp: &Footprint) -> bool {
    let re2 = dot(eye, eye);
    if re2 <= 0.0 {
        return true;
    }
    let r_e = re2.sqrt();
    dot(DVec3::new(eye.x / r_e, eye.y / r_e, eye.z / r_e), fp.center) >= fp.cos_delta
}

/// Lower bound of the distance from `eye` to the footprint extruded between `r`.
///
/// Inside the cone the distance is the radial gap. Outside, the nearest points lie on the cone
/// boundary at angle `phi` from the eye direction, where the best radius is `r_e * cos(phi)`
/// clamped to the shell.
#[must_use]
pub fn cell_distance(radius: f64, eye: DVec3, fp: &Footprint, r: HeightRange) -> f64 {
    let r0 = radius + r.min;
    let r1 = radius + r.max;
    let re2 = dot(eye, eye);
    if re2 <= 0.0 {
        return r0;
    }
    let r_e = re2.sqrt();
    let e = DVec3::new(eye.x / r_e, eye.y / r_e, eye.z / r_e);
    let (cos_t, cos_phi) = cone_angles(e, fp);
    if cos_t >= fp.cos_delta {
        let mut d = 0.0;
        if r_e - r1 > d {
            d = r_e - r1;
        }
        if r0 - r_e > d {
            d = r0 - r_e;
        }
        return d;
    }
    let mut r_star = if cos_phi > 0.0 { r_e * cos_phi } else { r0 };
    if r_star < r0 {
        r_star = r0;
    }
    if r_star > r1 {
        r_star = r1;
    }
    let d2 = r_e * r_e + r_star * r_star - 2.0 * r_e * r_star * cos_phi;
    if d2 > 0.0 { d2.sqrt() } else { 0.0 }
}

/// Whether any point of the extruded footprint can be seen from `eye` over a spherical occluder
/// of `occluder_radius` (a sphere inside the terrain).
///
/// A point at radius `r` is visible over a sphere of radius `r_o` iff its angle from the eye
/// direction is at most `acos(r_o / r_e) + acos(r_o / r)`. The most visible point of the cell is
/// at the top radius `r1` and at the cone boundary nearest to the eye, so the cell is visible iff
/// `phi <= a_e + a_1`, i.e. `cos(phi) >= cos(a_e) cos(a_1) - sin(a_e) sin(a_1)`.
#[must_use]
pub fn horizon_visible(
    radius: f64,
    eye: DVec3,
    fp: &Footprint,
    r: HeightRange,
    occluder_radius: f64,
) -> bool {
    let r1 = radius + r.max;
    let re2 = dot(eye, eye);
    let r_e = re2.sqrt();
    if r_e <= occluder_radius || r1 <= occluder_radius {
        return true;
    }
    let e = DVec3::new(eye.x / r_e, eye.y / r_e, eye.z / r_e);
    let (cos_t, cos_phi) = cone_angles(e, fp);
    if cos_t >= fp.cos_delta {
        return true;
    }
    let cos_ae = occluder_radius / r_e;
    let sin_ae = (1.0 - cos_ae * cos_ae).sqrt();
    let cos_a1 = occluder_radius / r1;
    let sin_a1 = (1.0 - cos_a1 * cos_a1).sqrt();
    let cos_sum = cos_ae * cos_a1 - sin_ae * sin_a1;
    cos_phi >= cos_sum
}

/// Bounding sphere of the extruded footprint, centered at the mid shell radius.
#[must_use]
pub fn cell_bound(radius: f64, fp: &Footprint, r: HeightRange) -> Bound {
    let r0 = radius + r.min;
    let r1 = radius + r.max;
    let rm = radius + (r.min + r.max) / 2.0;
    let center = DVec3::new(fp.center.x * rm, fp.center.y * rm, fp.center.z * rm);
    let a = r0 * r0 + rm * rm - 2.0 * r0 * rm * fp.cos_delta;
    let b = r1 * r1 + rm * rm - 2.0 * r1 * rm * fp.cos_delta;
    let m = if a > b { a } else { b };
    Bound {
        center,
        radius: if m > 0.0 { m.sqrt() } else { 0.0 },
    }
}

/// False iff `b` lies entirely outside one of the view planes (`[a, b, c, d]`, inward normals).
/// Always true without a frustum.
#[must_use]
pub fn frustum_visible(view: &View, b: &Bound) -> bool {
    let Some(planes) = view.frustum else {
        return true;
    };
    for p in planes {
        let dist = p[0] * b.center.x + p[1] * b.center.y + p[2] * b.center.z + p[3];
        if dist < -b.radius {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------------------------
// Decision margins (07.1-27): how far the eye or the frustum planes may move before a test of the
// selection could change its answer. Every function returns a LOWER bound (never an estimate); an
// input it cannot bound yields 0, which forbids any skip. Exact ops only.

/// Largest change of the direction angle of an eye at distance `r_e` that moves by `de` is
/// `asin(de / r_e) <= FRAC_PI_2 * de / r_e` (`de <= r_e`), so an angle margin `a` is worth
/// `a * r_e / FRAC_PI_2` metres of eye translation.
fn angle_to_metres(a: f64, r_e: f64) -> f64 {
    if a > 0.0 && r_e > 0.0 {
        a * r_e / FRAC_PI_2
    } else {
        0.0
    }
}

/// Metres of eye translation that cannot change whether the eye direction is inside the cone.
/// `|cos| `-differences lower-bound angle differences (cos is 1-Lipschitz).
#[must_use]
pub fn cone_margin_m(eye: DVec3, fp: &Footprint) -> f64 {
    let re2 = dot(eye, eye);
    if re2 <= 0.0 || re2.is_nan() {
        return 0.0;
    }
    let r_e = re2.sqrt();
    let e = DVec3::new(eye.x / r_e, eye.y / r_e, eye.z / r_e);
    let m = dot(e, fp.center) - fp.cos_delta;
    angle_to_metres(if m < 0.0 { -m } else { m }, r_e)
}

/// Metres of eye translation that cannot change [`horizon_visible`] (including the cone test it
/// contains), valid while the eye moves by at most `(|eye| - occluder_radius) / 2`.
///
/// Outside the cone the test compares the angle `phi` (eye direction to the nearest cone boundary)
/// with `a_e + a_1`, `a_e = acos(occluder / r_e)`. A move of `de` changes `phi` by at most
/// `FRAC_PI_2 * de / r_e` and `a_e` by at most `de * occluder / (r_lo * sqrt(r_lo^2 - occluder^2))`
/// with `r_lo = (r_e + occluder) / 2` (the slope of `a_e` decreases with `r`); the angle gap is at
/// least the cosine gap.
#[must_use]
pub fn horizon_margin_m(
    radius: f64,
    eye: DVec3,
    fp: &Footprint,
    r: HeightRange,
    occluder_radius: f64,
) -> f64 {
    let r1 = radius + r.max;
    if r1 <= occluder_radius {
        return f64::INFINITY;
    }
    let re2 = dot(eye, eye);
    if re2 <= 0.0 || re2.is_nan() {
        return 0.0;
    }
    let r_e = re2.sqrt();
    if r_e <= occluder_radius {
        return 0.0;
    }
    let cone = cone_margin_m(eye, fp);
    let e = DVec3::new(eye.x / r_e, eye.y / r_e, eye.z / r_e);
    let (cos_t, cos_phi) = cone_angles(e, fp);
    if cos_t >= fp.cos_delta {
        return cone;
    }
    let cos_ae = occluder_radius / r_e;
    let sin_ae = (1.0 - cos_ae * cos_ae).sqrt();
    let cos_a1 = occluder_radius / r1;
    let sin_a1 = (1.0 - cos_a1 * cos_a1).sqrt();
    let cos_sum = cos_ae * cos_a1 - sin_ae * sin_a1;
    let gap = cos_phi - cos_sum;
    let gap = if gap < 0.0 { -gap } else { gap };
    let r_lo = (r_e + occluder_radius) / 2.0;
    let q = r_lo * r_lo - occluder_radius * occluder_radius;
    if q <= 0.0 || q.is_nan() {
        return 0.0;
    }
    let slope = FRAC_PI_2 / r_e + occluder_radius / (r_lo * q.sqrt());
    let h = gap / slope;
    if h < cone { h } else { cone }
}

/// Largest eye move for which the margins of [`horizon_margin_m`] hold (0 when the eye is not
/// above the occluder).
#[must_use]
pub fn horizon_eye_cap(eye: DVec3, occluder_radius: f64) -> f64 {
    let r_e = dot(eye, eye).sqrt();
    if r_e > occluder_radius {
        (r_e - occluder_radius) / 2.0
    } else {
        0.0
    }
}

/// Margin of the frustum decision of `b`: how much every plane value may change before the answer
/// of [`frustum_visible`] can change (plane-value units, metres for unit normals). Visible: the
/// smallest `dist + radius`; culled: the best excluding plane's `-radius - dist`. `None` without a
/// frustum.
#[must_use]
pub fn frustum_margin(view: &View, b: &Bound) -> Option<f64> {
    let planes = view.frustum?;
    let mut min_in = f64::INFINITY;
    let mut max_out = f64::NEG_INFINITY;
    for p in planes {
        let dist = p[0] * b.center.x + p[1] * b.center.y + p[2] * b.center.z + p[3];
        let inside = dist + b.radius;
        if inside < min_in {
            min_in = inside;
        }
        if -inside > max_out {
            max_out = -inside;
        }
    }
    let out = max_out > 0.0;
    Some(if out { max_out } else { min_in })
}

/// Margin (metres of distance) of the screen-space-error decisions `rho > tau` and
/// `rho > tau * (1 - hysteresis)` of a cell whose clamped distance is `d` and whose
/// `geometric_error * proj_scale` is `num`: `rho = num / d`, so each test is a threshold on `d`.
/// The distance to an extruded cone is exactly 1-Lipschitz in the eye, as is its clamp.
#[must_use]
pub fn sse_margin_m(num: f64, d: f64, tau: f64, low: f64) -> f64 {
    let mut m = f64::INFINITY;
    for x in [tau, low] {
        if x > 0.0 {
            let thr = num / x;
            if !thr.is_finite() {
                continue;
            }
            let gap = if d > thr { d - thr } else { thr - d };
            if gap < m {
                m = gap;
            }
        }
    }
    if m.is_nan() { 0.0 } else { m }
}
