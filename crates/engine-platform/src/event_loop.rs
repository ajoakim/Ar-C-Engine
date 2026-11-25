//! winit 0.30 event loop driving a [`PlatformHandler`].

use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::error::{HandlerError, PlatformError};
use crate::input::{
    InputEvent, translate_cursor, translate_key, translate_mouse_button, translate_resize,
    translate_wheel,
};
use crate::window::PlatformWindow;

/// Window creation parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "engine".to_owned(),
            width: 1280,
            height: 720,
        }
    }
}

impl WindowConfig {
    /// Maximum accepted width or height in pixels.
    pub const MAX_DIMENSION: u32 = 16384;
    /// Maximum accepted title length in characters.
    pub const MAX_TITLE_LEN: usize = 256;

    /// Validate bounds before any window is created.
    ///
    /// # Errors
    /// Returns [`PlatformError::InvalidConfig`] when size or title is out of range.
    pub fn validate(&self) -> Result<(), PlatformError> {
        let ok = 1..=Self::MAX_DIMENSION;
        if !ok.contains(&self.width) || !ok.contains(&self.height) {
            return Err(PlatformError::InvalidConfig("window size out of range"));
        }
        if self.title.is_empty() || self.title.chars().count() > Self::MAX_TITLE_LEN {
            return Err(PlatformError::InvalidConfig(
                "window title empty or too long",
            ));
        }
        Ok(())
    }
}

/// Whether the loop should keep running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopControl {
    Continue,
    Exit,
}

/// Application callbacks driven by [`run`].
pub trait PlatformHandler {
    /// Called once the window exists (inside winit's `resumed`).
    ///
    /// # Errors
    /// Any error aborts the loop and is returned from [`run`].
    fn window_ready(&mut self, window: PlatformWindow) -> Result<(), HandlerError>;
    /// Called for each translated input event.
    fn input(&mut self, event: InputEvent);
    /// Called per redraw with seconds since the previous redraw.
    fn redraw(&mut self, dt: f64) -> LoopControl;
    /// Called as the loop exits.
    fn exiting(&mut self) {}
}

struct Runner<'a> {
    config: WindowConfig,
    handler: &'a mut dyn PlatformHandler,
    window: Option<PlatformWindow>,
    last_redraw: Option<Instant>,
    error: Option<PlatformError>,
}

impl ApplicationHandler for Runner<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(PhysicalSize::new(self.config.width, self.config.height));
        match event_loop.create_window(attrs) {
            Ok(w) => {
                let window = PlatformWindow::new(w);
                self.window = Some(window.clone());
                if let Err(e) = self.handler.window_ready(window) {
                    self.error = Some(PlatformError::Handler(e));
                    event_loop.exit();
                }
            }
            Err(e) => {
                self.error = Some(PlatformError::CreateWindow(e.to_string()));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.handler.input(InputEvent::CloseRequested);
                event_loop.exit();
            }
            WindowEvent::Resized(s) => self.handler.input(translate_resize(s)),
            WindowEvent::Focused(f) => self.handler.input(InputEvent::Focused(f)),
            WindowEvent::KeyboardInput { event, .. } => {
                self.handler
                    .input(translate_key(event.physical_key, event.state, event.repeat));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.handler.input(translate_mouse_button(button, state));
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(e) = translate_cursor(position) {
                    self.handler.input(e);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(e) = translate_wheel(delta) {
                    self.handler.input(e);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = self
                    .last_redraw
                    .map_or(0.0, |t| now.duration_since(t).as_secs_f64())
                    .clamp(0.0, 0.25);
                self.last_redraw = Some(now);
                if self.handler.redraw(dt) == LoopControl::Exit {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.handler.exiting();
    }
}

/// Open a window and drive `handler` until exit.
///
/// # Errors
/// Returns [`PlatformError`] on invalid config, event-loop failure, window
/// creation failure, or handler error.
pub fn run(config: WindowConfig, handler: &mut dyn PlatformHandler) -> Result<(), PlatformError> {
    config.validate()?;
    let event_loop = EventLoop::new().map_err(|e| PlatformError::EventLoop(e.to_string()))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut runner = Runner {
        config,
        handler,
        window: None,
        last_redraw: None,
        error: None,
    };
    event_loop
        .run_app(&mut runner)
        .map_err(|e| PlatformError::EventLoop(e.to_string()))?;
    match runner.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_config_validate_default_ok() {
        assert!(WindowConfig::default().validate().is_ok());
    }

    #[test]
    fn window_config_validate_width_zero() {
        let c = WindowConfig {
            width: 0,
            ..WindowConfig::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn window_config_validate_width_too_large() {
        let c = WindowConfig {
            width: 16385,
            ..WindowConfig::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn window_config_validate_empty_title() {
        let c = WindowConfig {
            title: String::new(),
            ..WindowConfig::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn window_config_validate_long_title() {
        let c = WindowConfig {
            title: "x".repeat(257),
            ..WindowConfig::default()
        };
        assert!(c.validate().is_err());
    }
}
