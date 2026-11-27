//! Engine-owned input events and winit translation.

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseScrollDelta};
use winit::keyboard::PhysicalKey;

/// Pixels that count as one scroll line for pixel-delta wheels.
const PIXELS_PER_LINE: f64 = 40.0;

/// Engine key codes (physical keys).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    W,
    A,
    S,
    D,
    Q,
    E,
    Up,
    Down,
    Left,
    Right,
    Space,
    Escape,
    Enter,
    F3,
    ShiftLeft,
    ControlLeft,
    Z,
    X,
    F,
    Other,
}

/// Engine mouse buttons.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

/// Engine input events.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEvent {
    KeyPressed { key: KeyCode, repeat: bool },
    KeyReleased { key: KeyCode },
    MouseButtonPressed(MouseButton),
    MouseButtonReleased(MouseButton),
    CursorMoved { x: f64, y: f64 },
    MouseWheel { lines_x: f32, lines_y: f32 },
    Resized { width: u32, height: u32 },
    Focused(bool),
    CloseRequested,
}

pub(crate) fn translate_physical_key(key: PhysicalKey) -> KeyCode {
    use winit::keyboard::KeyCode as K;
    match key {
        PhysicalKey::Code(code) => match code {
            K::KeyW => KeyCode::W,
            K::KeyA => KeyCode::A,
            K::KeyS => KeyCode::S,
            K::KeyD => KeyCode::D,
            K::KeyQ => KeyCode::Q,
            K::KeyE => KeyCode::E,
            K::ArrowUp => KeyCode::Up,
            K::ArrowDown => KeyCode::Down,
            K::ArrowLeft => KeyCode::Left,
            K::ArrowRight => KeyCode::Right,
            K::Space => KeyCode::Space,
            K::Escape => KeyCode::Escape,
            K::Enter => KeyCode::Enter,
            K::F3 => KeyCode::F3,
            K::ShiftLeft => KeyCode::ShiftLeft,
            K::ControlLeft => KeyCode::ControlLeft,
            K::KeyZ => KeyCode::Z,
            K::KeyX => KeyCode::X,
            K::KeyF => KeyCode::F,
            _ => KeyCode::Other,
        },
        PhysicalKey::Unidentified(_) => KeyCode::Other,
    }
}

pub(crate) fn translate_key(
    physical: PhysicalKey,
    state: ElementState,
    repeat: bool,
) -> InputEvent {
    let key = translate_physical_key(physical);
    match state {
        ElementState::Pressed => InputEvent::KeyPressed { key, repeat },
        ElementState::Released => InputEvent::KeyReleased { key },
    }
}

pub(crate) fn translate_mouse_button(
    button: winit::event::MouseButton,
    state: ElementState,
) -> InputEvent {
    use winit::event::MouseButton as M;
    let b = match button {
        M::Left => MouseButton::Left,
        M::Right => MouseButton::Right,
        M::Middle => MouseButton::Middle,
        M::Back => MouseButton::Back,
        M::Forward => MouseButton::Forward,
        M::Other(n) => MouseButton::Other(n),
    };
    match state {
        ElementState::Pressed => InputEvent::MouseButtonPressed(b),
        ElementState::Released => InputEvent::MouseButtonReleased(b),
    }
}

pub(crate) fn translate_cursor(pos: PhysicalPosition<f64>) -> Option<InputEvent> {
    if pos.x.is_finite() && pos.y.is_finite() {
        Some(InputEvent::CursorMoved { x: pos.x, y: pos.y })
    } else {
        None
    }
}

pub(crate) fn translate_wheel(delta: MouseScrollDelta) -> Option<InputEvent> {
    let (x, y) = match delta {
        MouseScrollDelta::LineDelta(x, y) => (f64::from(x), f64::from(y)),
        MouseScrollDelta::PixelDelta(p) => (p.x / PIXELS_PER_LINE, p.y / PIXELS_PER_LINE),
    };
    if x.is_finite() && y.is_finite() {
        #[allow(clippy::cast_possible_truncation)]
        Some(InputEvent::MouseWheel {
            lines_x: x as f32,
            lines_y: y as f32,
        })
    } else {
        None
    }
}

pub(crate) fn translate_resize(size: PhysicalSize<u32>) -> InputEvent {
    InputEvent::Resized {
        width: size.width,
        height: size.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{KeyCode as WK, NativeKeyCode};

    #[test]
    fn input_maps_f3() {
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::F3)),
            KeyCode::F3
        );
    }

    #[test]
    fn input_maps_wasd_and_escape() {
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::KeyW)),
            KeyCode::W
        );
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::KeyA)),
            KeyCode::A
        );
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::KeyS)),
            KeyCode::S
        );
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::KeyD)),
            KeyCode::D
        );
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::Escape)),
            KeyCode::Escape
        );
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::ArrowUp)),
            KeyCode::Up
        );
    }

    #[test]
    fn input_maps_z_x_f() {
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::KeyZ)),
            KeyCode::Z
        );
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::KeyX)),
            KeyCode::X
        );
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::KeyF)),
            KeyCode::F
        );
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::KeyC)),
            KeyCode::Other
        );
    }

    #[test]
    fn input_unmapped_key_is_other() {
        assert_eq!(
            translate_physical_key(PhysicalKey::Code(WK::F7)),
            KeyCode::Other
        );
        assert_eq!(
            translate_physical_key(PhysicalKey::Unidentified(NativeKeyCode::Unidentified)),
            KeyCode::Other
        );
    }

    #[test]
    fn input_key_pressed_and_released() {
        let k = PhysicalKey::Code(WK::KeyW);
        assert_eq!(
            translate_key(k, ElementState::Pressed, true),
            InputEvent::KeyPressed {
                key: KeyCode::W,
                repeat: true
            }
        );
        assert_eq!(
            translate_key(k, ElementState::Released, false),
            InputEvent::KeyReleased { key: KeyCode::W }
        );
    }

    #[test]
    fn input_mouse_buttons() {
        use winit::event::MouseButton as M;
        let p = ElementState::Pressed;
        assert_eq!(
            translate_mouse_button(M::Left, p),
            InputEvent::MouseButtonPressed(MouseButton::Left)
        );
        assert_eq!(
            translate_mouse_button(M::Right, p),
            InputEvent::MouseButtonPressed(MouseButton::Right)
        );
        assert_eq!(
            translate_mouse_button(M::Middle, p),
            InputEvent::MouseButtonPressed(MouseButton::Middle)
        );
        assert_eq!(
            translate_mouse_button(M::Back, p),
            InputEvent::MouseButtonPressed(MouseButton::Back)
        );
        assert_eq!(
            translate_mouse_button(M::Forward, p),
            InputEvent::MouseButtonPressed(MouseButton::Forward)
        );
        assert_eq!(
            translate_mouse_button(M::Other(7), p),
            InputEvent::MouseButtonPressed(MouseButton::Other(7))
        );
        assert_eq!(
            translate_mouse_button(M::Left, ElementState::Released),
            InputEvent::MouseButtonReleased(MouseButton::Left)
        );
    }

    #[test]
    fn input_cursor_rejects_non_finite() {
        assert_eq!(
            translate_cursor(PhysicalPosition::new(10.5, 20.0)),
            Some(InputEvent::CursorMoved { x: 10.5, y: 20.0 })
        );
        assert_eq!(translate_cursor(PhysicalPosition::new(f64::NAN, 0.0)), None);
    }

    #[test]
    fn input_wheel_line_and_pixel() {
        assert_eq!(
            translate_wheel(MouseScrollDelta::LineDelta(1.0, -2.0)),
            Some(InputEvent::MouseWheel {
                lines_x: 1.0,
                lines_y: -2.0
            })
        );
        assert_eq!(
            translate_wheel(MouseScrollDelta::PixelDelta(PhysicalPosition::new(
                80.0, 40.0
            ))),
            Some(InputEvent::MouseWheel {
                lines_x: 2.0,
                lines_y: 1.0
            })
        );
        assert_eq!(
            translate_wheel(MouseScrollDelta::PixelDelta(PhysicalPosition::new(
                f64::INFINITY,
                0.0
            ))),
            None
        );
    }

    #[test]
    fn input_resize() {
        assert_eq!(
            translate_resize(PhysicalSize::new(800, 600)),
            InputEvent::Resized {
                width: 800,
                height: 600
            }
        );
        assert_eq!(
            translate_resize(PhysicalSize::new(0, 0)),
            InputEvent::Resized {
                width: 0,
                height: 0
            }
        );
    }
}
