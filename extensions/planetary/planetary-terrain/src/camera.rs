//! Planetary camera: authoritative f64 pose, altitude-scaled speed, parallel-transported
//! orientation (no pole singularity), surface clearance and altitude-scaled near plane.
//!
//! Input arrives only as `engine_sdk::InputEvent`. The mouse wheel is ignored in Phase 7.

use engine_sdk::{
    Camera, DQuat, DVec3, FrameId, InputEvent, KeyCode, MouseButton, SdkError, WorldPos,
};

/// `tan(30 deg)`: tangent of the half vertical field of view for `Camera::DEFAULT_FOV_Y`.
pub const TAN_HALF_FOV_Y: f64 = 0.577_350_269_189_625_8;

const K_FWD: u16 = 1;
const K_BACK: u16 = 1 << 1;
const K_LEFT: u16 = 1 << 2;
const K_RIGHT: u16 = 1 << 3;
const K_UP: u16 = 1 << 4;
const K_DOWN: u16 = 1 << 5;
const K_BOOST: u16 = 1 << 6;

/// Tunable camera parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraParams {
    /// Speed in metres per second per metre of altitude.
    pub speed_per_altitude: f64,
    /// Lower speed clamp (m/s).
    pub min_speed: f64,
    /// Upper speed clamp (m/s).
    pub max_speed: f64,
    /// Speed multiplier while boost (left shift) is held.
    pub boost: f64,
    /// Radians per cursor pixel.
    pub look_sensitivity: f64,
    /// Minimum height above the surface (m).
    pub min_clearance: f64,
    /// Below this altitude the pitch is clamped to `max_pitch`.
    pub free_look_altitude: f64,
    /// Maximum pitch (radians) off the horizon near the surface.
    pub max_pitch: f64,
}

impl Default for CameraParams {
    fn default() -> Self {
        Self {
            speed_per_altitude: 0.5,
            min_speed: 1.0,
            max_speed: 2.0e6,
            boost: 4.0,
            look_sensitivity: 0.002,
            min_clearance: 2.0,
            free_look_altitude: 1.0e5,
            max_pitch: 1.4,
        }
    }
}

/// Orbit-to-surface fly camera with f64 authoritative state.
#[derive(Clone, Debug)]
pub struct PlanetaryCamera {
    pos: DVec3,
    orient: DQuat,
    keys: u16,
    looking: bool,
    last_cursor: Option<(f64, f64)>,
    look_delta: (f64, f64),
    exit: bool,
    params: CameraParams,
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
        _ => 0,
    }
}

fn finite3(v: DVec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

fn finite_q(q: DQuat) -> bool {
    q.x.is_finite() && q.y.is_finite() && q.z.is_finite() && q.w.is_finite()
}

impl PlanetaryCamera {
    /// Camera at `pos` looking at the planet center. `None` if `pos` is non-finite or zero.
    pub fn new(pos: DVec3, params: CameraParams) -> Option<Self> {
        if !finite3(pos) || pos.length() <= 0.0 {
            return None;
        }
        let f = -pos.normalize();
        let world_up = if f.y.abs() > 0.99 { DVec3::Z } else { DVec3::Y };
        let u = (world_up - f * world_up.dot(f)).normalize();
        let q1 = DQuat::from_rotation_arc(DVec3::NEG_Z, f);
        let up1 = q1 * DVec3::Y;
        let angle = f.dot(up1.cross(u)).atan2(up1.dot(u));
        let orient = (DQuat::from_axis_angle(f, angle) * q1).normalize();
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
            exit: false,
            params,
        })
    }

    /// Authoritative position (planet-centered metres).
    pub fn position(&self) -> DVec3 {
        self.pos
    }

    /// Authoritative orientation.
    pub fn orientation(&self) -> DQuat {
        self.orient
    }

    /// View direction (`orientation * -Z`).
    pub fn forward(&self) -> DVec3 {
        self.orient * DVec3::NEG_Z
    }

    /// True once Escape was pressed.
    pub fn exit_requested(&self) -> bool {
        self.exit
    }

    /// Overrides the pose. Returns false (pose unchanged) if any component is non-finite.
    pub fn set_pose(&mut self, pos: DVec3, orient: DQuat) -> bool {
        if !finite3(pos) || !finite_q(orient) || orient.length_squared() <= 0.0 {
            return false;
        }
        self.pos = pos;
        self.orient = orient.normalize();
        true
    }

    /// Maps an SDK input event onto camera state. The mouse wheel is ignored.
    pub fn handle_input(&mut self, event: &InputEvent) {
        match *event {
            InputEvent::KeyPressed { key, repeat } => {
                if repeat {
                    return;
                }
                if key == KeyCode::Escape {
                    self.exit = true;
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

    /// Height above the surface along the current radial direction.
    pub fn altitude(&self, surface_radius_at: &dyn Fn(DVec3) -> f64) -> f64 {
        let len = self.pos.length();
        if len <= 0.0 {
            return 0.0;
        }
        len - surface_radius_at(self.pos / len)
    }

    /// Near plane for an altitude: `clamp(alt * 0.01, 0.05, 1e4)`.
    pub fn near_plane(altitude: f64) -> f32 {
        let a = if altitude.is_finite() { altitude } else { 0.0 };
        (a * 0.01).clamp(0.05, 1.0e4) as f32
    }

    /// Pixels per unit tangent for a viewport height.
    pub fn proj_scale(viewport_height_px: u32) -> f64 {
        f64::from(viewport_height_px) / (2.0 * TAN_HALF_FOV_Y)
    }

    /// Advances the camera by `dt` seconds. Ignores non-positive or non-finite `dt`.
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
        let axis = |pos: u16, neg: u16| -> f64 {
            f64::from(u8::from(self.keys & pos != 0)) - f64::from(u8::from(self.keys & neg != 0))
        };
        let fwd = self.orient * DVec3::NEG_Z;
        let right = self.orient * DVec3::X;
        let mv =
            fwd * axis(K_FWD, K_BACK) + right * axis(K_RIGHT, K_LEFT) + up_old * axis(K_UP, K_DOWN);
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

        // Parallel transport of the orientation along the great-circle step.
        let mut orient = (DQuat::from_rotation_arc(up_old, up_new) * self.orient).normalize();

        // Mouse look.
        let (dx, dy) = std::mem::take(&mut self.look_delta);
        if dx != 0.0 || dy != 0.0 {
            let s = self.params.look_sensitivity;
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

        // Surface clearance.
        let min_r = surface_radius_at(up_new) + self.params.min_clearance + extra_clearance;
        if new_len < min_r {
            new_pos = up_new * min_r;
        }

        if finite3(new_pos) && finite_q(orient) {
            self.pos = new_pos;
            self.orient = orient;
        }
    }

    /// Writes the pose into an SDK camera (root frame).
    #[allow(clippy::result_large_err)] // SdkError is the SDK-wide error type.
    pub fn apply_to(
        &self,
        camera: &mut Camera,
        root: FrameId,
        altitude: f64,
    ) -> Result<(), SdkError> {
        camera.position = WorldPos::from_xyz(root, self.pos.x, self.pos.y, self.pos.z)?;
        camera.orientation = self.orient.as_quat().normalize();
        camera.z_near = Self::near_plane(altitude);
        camera.fov_y_radians = Camera::DEFAULT_FOV_Y;
        Ok(())
    }
}
