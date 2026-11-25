//! Delegating raw-window-handle newtype over an engine render target.

use std::sync::Arc;

use engine_render::RenderTarget;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

/// Owned window target handed to wgpu (safe `create_surface` path).
pub(crate) struct Target(pub(crate) Arc<dyn RenderTarget>);

impl std::fmt::Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Target")
    }
}

impl HasWindowHandle for Target {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.0.window_handle()
    }
}

impl HasDisplayHandle for Target {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.0.display_handle()
    }
}
