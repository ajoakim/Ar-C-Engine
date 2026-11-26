//! Fly camera controller driven by SDK input events.

use engine_sdk::{Camera, DVec3, InputEvent, KeyCode, MouseButton, Quat, SdkError, Vec3};

const FORWARD: u16 = 1 << 0;
const BACK: u16 = 1 << 1;
const LEFT: u16 = 1 << 2;
const RIGHT: u16 = 1 << 3;
const UP: u16 = 1 << 4;
const DOWN: u16 = 1 << 5;
const BOOST: u16 = 1 << 6;

fn key_bit(key: KeyCode) -> u16 {
    match key {
        KeyCode::W | KeyCode::Up => FORWARD,
        KeyCode::S | KeyCode::Down => BACK,
        KeyCode::A | KeyCode::Left => LEFT,
        KeyCode::D | KeyCode::Right => RIGHT,
        KeyCode::E | KeyCode::Space => UP,
        KeyCode::Q | KeyCode::ControlLeft => DOWN,
        KeyCode::ShiftLeft => BOOST,
        _ => 0,
    }
}

/// Free-fly camera: WASD/arrows move, E/Space up, Q/Ctrl down, Shift boosts,
/// right-mouse-drag looks.
#[derive(Debug, Clone)]
pub struct FlyCamera {
    /// Yaw in radians (about +Y).
    pub yaw: f32,
    /// Pitch in radians, clamped to +-[`Self::MAX_PITCH`].
    pub pitch: f32,
    /// Base speed in metres per second.
    pub speed: f64,
    /// Speed multiplier while boosting.
    pub boost: f64,
    /// Look sensitivity in radians per pixel.
    pub sensitivity: f32,
    held: u16,
    looking: bool,
    last_cursor: Option<(f64, f64)>,
}

impl Default for FlyCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            speed: 5.0,
            boost: 10.0,
            sensitivity: 0.003,
            held: 0,
            looking: false,
            last_cursor: None,
        }
    }
}

impl FlyCamera {
    /// 89 degrees in radians.
    pub const MAX_PITCH: f32 = 1.553_343;

    /// Create a controller with default tuning.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one input event.
    pub fn handle_input(&mut self, event: &InputEvent) {
        match event {
            InputEvent::KeyPressed { key, .. } => self.held |= key_bit(*key),
            InputEvent::KeyReleased { key } => self.held &= !key_bit(*key),
            InputEvent::MouseButtonPressed(MouseButton::Right) => {
                self.looking = true;
                self.last_cursor = None;
            }
            InputEvent::MouseButtonReleased(MouseButton::Right) => {
                self.looking = false;
                self.last_cursor = None;
            }
            InputEvent::CursorMoved { x, y } => {
                if !x.is_finite() || !y.is_finite() {
                    return;
                }
                if self.looking {
                    if let Some((lx, ly)) = self.last_cursor {
                        self.yaw -= (x - lx) as f32 * self.sensitivity;
                        self.pitch = (self.pitch - (y - ly) as f32 * self.sensitivity)
                            .clamp(-Self::MAX_PITCH, Self::MAX_PITCH);
                    }
                    self.last_cursor = Some((*x, *y));
                }
            }
            InputEvent::Focused(false) => {
                self.held = 0;
                self.looking = false;
                self.last_cursor = None;
            }
            _ => {}
        }
    }

    /// Current look orientation.
    pub fn orientation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(self.pitch)
    }

    /// Advance the camera by `dt` seconds.
    pub fn update(&mut self, dt: f64, camera: &mut Camera) -> Result<(), SdkError> {
        let orientation = self.orientation();
        camera.orientation = orientation;
        if !(dt.is_finite() && dt > 0.0) {
            return Ok(());
        }
        let axis = |pos: u16, neg: u16| -> f32 {
            f32::from(u8::from(self.held & pos != 0)) - f32::from(u8::from(self.held & neg != 0))
        };
        // Rotate only the horizontal input; vertical input stays world +Y.
        let local = Vec3::new(axis(RIGHT, LEFT), 0.0, -axis(FORWARD, BACK));
        let rotated = orientation * local;
        let dir = rotated + Vec3::new(0.0, axis(UP, DOWN), 0.0);
        let len = dir.length();
        if len <= 0.0 || !len.is_finite() {
            return Ok(());
        }
        let dir = dir / len;
        let mult = if self.held & BOOST != 0 {
            self.boost
        } else {
            1.0
        };
        let v: DVec3 = dir.as_dvec3() * self.speed * mult * dt;
        camera.position = camera.position.translated(v)?;
        Ok(())
    }
}
