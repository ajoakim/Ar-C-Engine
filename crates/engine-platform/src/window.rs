//! Window handle wrapper exposing only raw-window-handle traits.

use std::sync::Arc;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use winit::window::Window;

/// A platform window. Carries the OS window across crate boundaries
/// exclusively through raw-window-handle traits.
#[derive(Clone)]
pub struct PlatformWindow {
    inner: Arc<Window>,
}

impl PlatformWindow {
    pub(crate) fn new(window: Window) -> Self {
        Self {
            inner: Arc::new(window),
        }
    }

    /// Inner (client area) size in physical pixels.
    #[must_use]
    pub fn inner_size(&self) -> (u32, u32) {
        let s = self.inner.inner_size();
        (s.width, s.height)
    }

    /// Window scale factor (physical pixels per logical point; 2.0 on a typical HiDPI display).
    #[must_use]
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    /// Request a redraw event.
    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }

    /// Refresh rate of the current monitor in millihertz, when the platform reports it.
    #[must_use]
    pub fn monitor_refresh_mhz(&self) -> Option<u32> {
        self.inner
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
    }

    /// Set the window title.
    pub fn set_title(&self, title: &str) {
        self.inner.set_title(title);
    }
}

impl HasWindowHandle for PlatformWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle()
    }
}

impl HasDisplayHandle for PlatformWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.inner.display_handle()
    }
}

impl std::fmt::Debug for PlatformWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PlatformWindow {{ size: {:?} }}", self.inner_size())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_target<T: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static>() {}

    #[test]
    fn window_satisfies_render_target_bounds() {
        assert_target::<PlatformWindow>();
    }

    #[test]
    fn monitor_refresh_signature() {
        let _f: fn(&PlatformWindow) -> Option<u32> = PlatformWindow::monitor_refresh_mhz;
    }
}
