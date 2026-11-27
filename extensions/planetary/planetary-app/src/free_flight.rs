//! Free-flight 6DOF camera controller (pure f64 quaternion maths, no GPU or window types).
//!
//! The controller lives in the planetary extension; the engine core only knows key codes.

use engine_sdk::{DQuat, DVec3, InputEvent, KeyCode, MouseButton};
use planetary_terrain::CameraParams;

/// Default roll rate in radians per second while a roll key is held.
pub const DEFAULT_ROLL_RATE: f64 = 1.2;

/// Applies body-axis increments to `q` and returns a normalised quaternion.
///
/// Yaw is about the local Y (camera up) axis, pitch about local X (camera right) and roll about
/// local Z (the view axis; a positive angle is counterclockwise as seen by the pilot looking
/// along -Z). The product is always renormalised, even for zero increments, so a non-unit input
/// is repaired and rounding drift cannot accumulate.
pub fn rotate_local(q: DQuat, pitch: f64, yaw: f64, roll: f64) -> DQuat {
    let dq = DQuat::from_axis_angle(DVec3::Y, yaw)
        * DQuat::from_axis_angle(DVec3::X, pitch)
        * DQuat::from_axis_angle(DVec3::Z, roll);
    (q * dq).normalize()
}

/// Levels the horizon: keeps the view direction, makes camera right perpendicular to
/// `planet_up` and camera up point toward it. Returns `q` unchanged when the view direction is
/// (nearly) parallel to `planet_up`.
pub fn level_orientation(q: DQuat, planet_up: DVec3) -> DQuat {
    let f = q * DVec3::NEG_Z;
    let side = f.cross(planet_up);
    if side.length() < 1e-9 {
        return q;
    }
    let r = side.normalize();
    let u = r.cross(f);
    // Roll about the (unchanged) view axis from the current camera up to the levelled up.
    let u0 = q * DVec3::Y;
    let angle = f.dot(u0.cross(u)).atan2(u0.dot(u));
    (DQuat::from_axis_angle(f, angle) * q).normalize()
}

const K_FWD: u16 = 1;
const K_BACK: u16 = 1 << 1;
const K_LEFT: u16 = 1 << 2;
const K_RIGHT: u16 = 1 << 3;
const K_UP: u16 = 1 << 4;
const K_DOWN: u16 = 1 << 5;
const K_BOOST: u16 = 1 << 6;
const K_ROLL_L: u16 = 1 << 7;
const K_ROLL_R: u16 = 1 << 8;

/// Camera flight mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FlightMode {
    /// Spacecraft style: all three rotation axes are local, thrust is camera-relative.
    #[default]
    Free,
    /// Horizon-locked: planet-up yaw, clamped pitch, radial vertical thrust, no roll.
    PlanetAligned,
}

impl FlightMode {
    /// Short display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::PlanetAligned => "planet-aligned",
        }
    }
}

fn key_bit(key: KeyCode) -> u16 {
    match key {
        KeyCode::W | KeyCode::Up => K_FWD,
        KeyCode::S | KeyCode::Down => K_BACK,
        KeyCode::A | KeyCode::Left => K_LEFT,
        KeyCode::D | KeyCode::Right => K_RIGHT,
        KeyCode::E | KeyCode::Space => K_UP,
        KeyCode::Q | KeyCode::ControlLeft => K_DOWN,
        KeyCode::ShiftLeft => K_BOOST,
        KeyCode::Z => K_ROLL_L,
        KeyCode::X => K_ROLL_R,
        _ => 0,
    }
}

fn finite3(v: DVec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

fn finite_q(q: DQuat) -> bool {
    q.x.is_finite() && q.y.is_finite() && q.z.is_finite() && q.w.is_finite()
}

fn valid_pose(pos: DVec3, orient: DQuat) -> bool {
    finite3(pos)
        && pos.length() > 0.0
        && finite_q(orient)
        && orient.length_squared() > 0.0
        && orient.length_squared().is_finite()
}

/// Six-degrees-of-freedom camera with f64 authoritative state and a unit orientation
/// quaternion that is renormalised on every update.
#[derive(Clone, Debug)]
pub struct FreeFlight {
    pos: DVec3,
    orient: DQuat,
    keys: u16,
    looking: bool,
    last_cursor: Option<(f64, f64)>,
    look_delta: (f64, f64),
    params: CameraParams,
    roll_rate: f64,
    mode: FlightMode,
}

impl FreeFlight {
    /// New controller in [`FlightMode::Free`]. `None` for a non-finite or zero position or a
    /// non-finite or zero-length orientation.
    pub fn new(pos: DVec3, orient: DQuat, params: CameraParams) -> Option<Self> {
        if !valid_pose(pos, orient) {
            return None;
        }
        let orient = orient.normalize();
        if !finite_q(orient) {
            return None;
        }
        Some(Self {
            pos,
            orient,
            keys: 0,
            looking: false,
            last_cursor: None,
            look_delta: (0.0, 0.0),
            params,
            roll_rate: DEFAULT_ROLL_RATE,
            mode: FlightMode::Free,
        })
    }

    /// Authoritative position (planet-centered metres).
    pub fn position(&self) -> DVec3 {
        self.pos
    }

    /// Authoritative unit orientation.
    pub fn orientation(&self) -> DQuat {
        self.orient
    }

    /// View direction (`orientation * -Z`).
    pub fn forward(&self) -> DVec3 {
        self.orient * DVec3::NEG_Z
    }

    /// Current flight mode.
    pub fn mode(&self) -> FlightMode {
        self.mode
    }

    /// Live roll rate (rad/s).
    pub fn roll_rate(&self) -> f64 {
        self.roll_rate
    }

    /// Sets the mode without touching the pose.
    pub fn set_mode(&mut self, mode: FlightMode) {
        self.mode = mode;
    }

    /// Toggles the mode and returns the new one. Entering planet-aligned levels the horizon
    /// (view direction kept); leaving it changes nothing.
    pub fn toggle_mode(&mut self) -> FlightMode {
        match self.mode {
            FlightMode::Free => {
                self.mode = FlightMode::PlanetAligned;
                let len = self.pos.length();
                if len > 0.0 {
                    let leveled = level_orientation(self.orient, self.pos / len);
                    if finite_q(leveled) {
                        self.orient = leveled;
                    }
                }
            }
            FlightMode::PlanetAligned => self.mode = FlightMode::Free,
        }
        self.mode
    }

    /// Overrides the pose. Returns false (pose unchanged) if invalid.
    pub fn set_pose(&mut self, pos: DVec3, orient: DQuat) -> bool {
        if !valid_pose(pos, orient) {
            return false;
        }
        self.pos = pos;
        self.orient = orient.normalize();
        true
    }

    /// Maps an SDK input event onto controller state. The mouse wheel is ignored; `F` (not a
    /// key repeat) toggles the flight mode.
    pub fn handle_input(&mut self, event: &InputEvent) {
        match *event {
            InputEvent::KeyPressed { key, repeat } => {
                if repeat {
                    return;
                }
                if key == KeyCode::F {
                    self.toggle_mode();
                }
                self.keys |= key_bit(key);
            }
            InputEvent::KeyReleased { key } => self.keys &= !key_bit(key),
            InputEvent::MouseButtonPressed(MouseButton::Right) => {
                self.looking = true;
                self.last_cursor = None;
            }
            InputEvent::MouseButtonReleased(MouseButton::Right) => {
                self.looking = false;
                self.last_cursor = None;
            }
            InputEvent::CursorMoved { x, y } => {
                if !(x.is_finite() && y.is_finite()) || !self.looking {
                    return;
                }
                if let Some((lx, ly)) = self.last_cursor {
                    self.look_delta.0 += x - lx;
                    self.look_delta.1 += y - ly;
                }
                self.last_cursor = Some((x, y));
            }
            InputEvent::Focused(false) => {
                self.keys = 0;
                self.looking = false;
                self.last_cursor = None;
                self.look_delta = (0.0, 0.0);
            }
            _ => {}
        }
    }

    fn axis(&self, pos: u16, neg: u16) -> f64 {
        f64::from(u8::from(self.keys & pos != 0)) - f64::from(u8::from(self.keys & neg != 0))
    }

    /// Advances the camera by `dt` seconds. Ignores non-positive or non-finite `dt`.
    ///
    /// In free mode Z rolls counterclockwise and X clockwise as seen by the pilot (about the
    /// view axis); thrust is relative to the camera axes. Planet-aligned mode reproduces
    /// `PlanetaryCamera::update`.
    pub fn update(
        &mut self,
        dt: f64,
        surface_radius_at: &dyn Fn(DVec3) -> f64,
        extra_clearance: f64,
    ) {
        if !(dt.is_finite() && dt > 0.0) {
            return;
        }
        let len = self.pos.length();
        if len <= 0.0 || !len.is_finite() {
            return;
        }
        let up_old = self.pos / len;
        let alt = len - surface_radius_at(up_old);
        let mut v = (alt * self.params.speed_per_altitude)
            .clamp(self.params.min_speed, self.params.max_speed);
        if self.keys & K_BOOST != 0 {
            v *= self.params.boost;
        }
        let fwd = self.orient * DVec3::NEG_Z;
        let right = self.orient * DVec3::X;
        let thrust_up = match self.mode {
            FlightMode::Free => self.orient * DVec3::Y,
            FlightMode::PlanetAligned => up_old,
        };
        let mv = fwd * self.axis(K_FWD, K_BACK)
            + right * self.axis(K_RIGHT, K_LEFT)
            + thrust_up * self.axis(K_UP, K_DOWN);
        let mv = if mv.length_squared() > 0.0 {
            mv.normalize()
        } else {
            mv
        };
        let mut new_pos = self.pos + mv * v * dt;
        let new_len = new_pos.length();
        if !finite3(new_pos) || new_len <= 0.0 {
            return;
        }
        let up_new = new_pos / new_len;
        let (dx, dy) = std::mem::take(&mut self.look_delta);
        let s = self.params.look_sensitivity;

        let orient = match self.mode {
            FlightMode::Free => {
                let roll = self.axis(K_ROLL_L, K_ROLL_R) * self.roll_rate * dt;
                rotate_local(self.orient, -dy * s, -dx * s, roll)
            }
            FlightMode::PlanetAligned => {
                // Parallel transport of the orientation along the great-circle step.
                let mut orient =
                    (DQuat::from_rotation_arc(up_old, up_new) * self.orient).normalize();
                if dx != 0.0 || dy != 0.0 {
                    let yawed = (DQuat::from_axis_angle(up_new, -dx * s) * orient).normalize();
                    let mut dpitch = -dy * s;
                    if alt < self.params.free_look_altitude {
                        let f = yawed * DVec3::NEG_Z;
                        let cur = f.dot(up_new).clamp(-1.0, 1.0).asin();
                        let m = self.params.max_pitch;
                        dpitch = (cur + dpitch).clamp(-m, m) - cur;
                    }
                    orient = (yawed * DQuat::from_axis_angle(DVec3::X, dpitch)).normalize();
                }
                orient
            }
        };

        // Surface clearance.
        let min_r = surface_radius_at(up_new) + self.params.min_clearance + extra_clearance;
        if new_len < min_r {
            new_pos = up_new * min_r;
        }

        if finite3(new_pos) && finite_q(orient) {
            self.pos = new_pos;
            self.orient = orient.normalize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DQuat {
        (DQuat::from_axis_angle(DVec3::Y, 0.7)
            * DQuat::from_axis_angle(DVec3::X, -0.4)
            * DQuat::from_axis_angle(DVec3::Z, 0.2))
        .normalize()
    }

    #[test]
    fn rotate_local_renormalises_non_unit_input() {
        let q = sample();
        let r = rotate_local(q * 1.001, 0.0, 0.0, 0.0);
        assert!((r.length() - 1.0).abs() < 1e-12);
        let r2 = rotate_local(q * 0.999, 0.01, -0.02, 0.03);
        assert!((r2.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn roll_keeps_forward_and_rotates_up_by_angle() {
        let q = sample();
        let a = 0.3;
        let r = rotate_local(q, 0.0, 0.0, a);
        let f0 = q * DVec3::NEG_Z;
        let f1 = r * DVec3::NEG_Z;
        assert!((f0 - f1).length() < 1e-12);
        let u0 = q * DVec3::Y;
        let u1 = r * DVec3::Y;
        assert!((u0.dot(u1).acos() - a).abs() < 1e-12);
        assert!(u0.cross(u1).dot(f0).abs() > 0.0);
    }

    #[test]
    fn roll_plus_minus_returns_to_start() {
        let q = sample();
        let back = rotate_local(rotate_local(q, 0.0, 0.0, 0.7), 0.0, 0.0, -0.7);
        assert!(q.dot(back).abs() > 1.0 - 1e-9);
    }

    #[test]
    fn level_orientation_levels_horizon_keeping_forward() {
        let q = sample();
        let up = DVec3::new(0.2, 0.9, -0.3).normalize();
        let l = level_orientation(q, up);
        assert!(((l * DVec3::NEG_Z) - (q * DVec3::NEG_Z)).length() < 1e-12);
        assert!((l * DVec3::X).dot(up).abs() < 1e-12);
        assert!((l * DVec3::Y).dot(up) > 0.0);
        assert!((l.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn level_orientation_parallel_forward_is_unchanged() {
        let q = DQuat::IDENTITY; // forward = -Z
        assert_eq!(level_orientation(q, DVec3::Z), q);
        assert_eq!(level_orientation(q, DVec3::NEG_Z), q);
    }

    const R: f64 = 6_371_000.0;

    fn flat(_: DVec3) -> f64 {
        R
    }

    fn flyer() -> FreeFlight {
        FreeFlight::new(
            DVec3::new(0.0, 0.0, R + 1.0e4),
            sample(),
            CameraParams::default(),
        )
        .unwrap()
    }

    fn press(f: &mut FreeFlight, key: KeyCode) {
        f.handle_input(&InputEvent::KeyPressed { key, repeat: false });
    }

    #[test]
    fn new_rejects_invalid() {
        let p = CameraParams::default();
        assert!(FreeFlight::new(DVec3::ZERO, DQuat::IDENTITY, p).is_none());
        assert!(FreeFlight::new(DVec3::new(f64::NAN, 0.0, 1.0), DQuat::IDENTITY, p).is_none());
        assert!(FreeFlight::new(DVec3::Z, DQuat::from_xyzw(0.0, 0.0, 0.0, 0.0), p).is_none());
        assert_eq!(flyer().mode(), FlightMode::Free);
        assert_eq!(FlightMode::Free.name(), "free");
        assert_eq!(FlightMode::PlanetAligned.name(), "planet-aligned");
        assert_eq!(flyer().roll_rate(), DEFAULT_ROLL_RATE);
    }

    #[test]
    fn free_thrust_is_camera_relative() {
        let mut f = flyer();
        let q = f.orientation();
        let p0 = f.position();
        press(&mut f, KeyCode::D);
        f.update(0.1, &flat, 0.0);
        let d = (f.position() - p0).normalize();
        assert!((d - q * DVec3::X).length() < 1e-9);

        let mut f = flyer();
        press(&mut f, KeyCode::E);
        f.update(0.1, &flat, 0.0);
        let d = (f.position() - p0).normalize();
        assert!((d - q * DVec3::Y).length() < 1e-9);
    }

    #[test]
    fn free_speed_scales_with_altitude_and_boost() {
        let step = |alt: f64, boost: bool| {
            let params = CameraParams {
                min_clearance: 0.0,
                ..CameraParams::default()
            };
            let tangent = DQuat::from_axis_angle(DVec3::Y, std::f64::consts::FRAC_PI_2);
            let mut f = FreeFlight::new(DVec3::new(0.0, 0.0, R + alt), tangent, params).unwrap();
            press(&mut f, KeyCode::W);
            if boost {
                press(&mut f, KeyCode::ShiftLeft);
            }
            let p0 = f.position();
            f.update(0.01, &flat, 0.0);
            (f.position() - p0).length() / 0.01
        };
        assert!((step(0.5, false) - 1.0).abs() < 1e-6);
        assert!((step(1.0e4, false) - 5.0e3).abs() < 1e-3);
        assert!((step(1.0e9, false) - 2.0e6).abs() < 1e-3);
        assert!((step(1.0e4, true) - 2.0e4).abs() < 1e-3);
    }

    #[test]
    fn free_roll_keeps_forward_and_rotates_up() {
        let mut f = flyer();
        let (f0, u0) = (f.forward(), f.orientation() * DVec3::Y);
        press(&mut f, KeyCode::Z);
        f.update(0.1, &flat, 0.0);
        let u1 = f.orientation() * DVec3::Y;
        assert!((f.forward() - f0).length() < 1e-12);
        assert!((u0.dot(u1).acos() - DEFAULT_ROLL_RATE * 0.1).abs() < 1e-9);
    }

    #[test]
    fn toggle_levels_and_back_keeps_pose() {
        let mut f = flyer();
        let (f0, up) = (f.forward(), f.position().normalize());
        assert_eq!(f.toggle_mode(), FlightMode::PlanetAligned);
        let q = f.orientation();
        assert!((q * DVec3::X).dot(up).abs() < 1e-12);
        assert!((q * DVec3::Y).dot(up) > 0.0);
        assert!((f.forward() - f0).length() < 1e-12);
        assert_eq!(f.toggle_mode(), FlightMode::Free);
        assert_eq!(f.orientation(), q);
    }

    #[test]
    fn f_key_toggles_but_repeat_does_not() {
        let mut f = flyer();
        f.handle_input(&InputEvent::KeyPressed {
            key: KeyCode::F,
            repeat: true,
        });
        assert_eq!(f.mode(), FlightMode::Free);
        press(&mut f, KeyCode::F);
        assert_eq!(f.mode(), FlightMode::PlanetAligned);
    }

    #[test]
    fn aligned_ignores_roll_and_uses_radial_vertical() {
        let mut f = flyer();
        f.toggle_mode();
        let q = f.orientation();
        let p0 = f.position();
        press(&mut f, KeyCode::Z);
        press(&mut f, KeyCode::E);
        f.update(0.1, &flat, 0.0);
        let up = p0.normalize();
        assert!(f.orientation().dot(q).abs() > 1.0 - 1e-12);
        assert!(((f.position() - p0).normalize() - up).length() < 1e-9);
    }

    #[test]
    fn bad_dt_and_cursor_keep_pose() {
        let mut f = flyer();
        let (p, q) = (f.position(), f.orientation());
        press(&mut f, KeyCode::W);
        f.update(f64::NAN, &flat, 0.0);
        f.update(-1.0, &flat, 0.0);
        f.handle_input(&InputEvent::MouseButtonPressed(MouseButton::Right));
        f.handle_input(&InputEvent::CursorMoved {
            x: f64::NAN,
            y: 0.0,
        });
        f.handle_input(&InputEvent::Focused(false));
        f.update(0.1, &flat, 0.0);
        assert_eq!(f.position(), p);
        assert_eq!(f.orientation(), q);
        assert!(!f.set_pose(DVec3::NAN, q));
        assert!(f.set_pose(p, q * 2.0));
        assert!((f.orientation().length() - 1.0).abs() < 1e-12);
    }
}
