//! `run_app`: wires the platform window loop to the wgpu backend.

use std::sync::Arc;

use engine_core::{Engine, EngineConfig};
use engine_platform::{
    HandlerError, InputEvent, LoopControl, PlatformError, PlatformHandler, PlatformWindow,
    WindowConfig,
};
use engine_render::RenderTarget;
use engine_render_wgpu::WgpuBackend;
/// Present-mode types re-exported for [`AppConfig`] (they carry no GPU API types).
pub use engine_render_wgpu::{FrameLatency, PresentConfig, PresentInfo, PresentModeSetting};

use crate::app::{App, AppControl, StartupInfo};
use crate::error::SdkError;
use crate::runtime::Runtime;

/// Configuration for [`run_app`].
#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    /// Window configuration.
    pub window: WindowConfig,
    /// Engine configuration.
    pub engine: EngineConfig,
    /// Surface present mode and frame latency (default Fifo, latency 2).
    pub present: PresentConfig,
}

struct Host<A: App> {
    app: A,
    engine: Option<Engine>,
    runtime: Option<Runtime<WgpuBackend>>,
    error: Option<SdkError>,
    present: PresentConfig,
    window: Option<PlatformWindow>,
    #[cfg(feature = "egui-overlay")]
    overlay: crate::overlay::OverlayHost,
}

impl<A: App> Host<A> {
    fn start(&mut self, window: PlatformWindow) -> Result<(), SdkError> {
        let (w, h) = window.inner_size();
        let handle = window.clone();
        let refresh = handle.monitor_refresh_mhz();
        let target: Arc<dyn RenderTarget> = Arc::new(window);
        let backend = WgpuBackend::new_with(target, w, h, &self.present)?;
        engine_diagnostics::tracing::info!(adapter = %backend.adapter_summary(), "gpu adapter");
        let startup = StartupInfo {
            adapter: backend.adapter_summary(),
            present: Some(backend.present_info().clone()),
            monitor_refresh_mhz: refresh,
        };
        let engine = self
            .engine
            .take()
            .ok_or_else(|| SdkError::App("window_ready called twice".into()))?;
        let mut runtime = Runtime::new(engine, backend)?;
        runtime.set_startup_info(startup);
        runtime.setup(&mut self.app)?;
        self.runtime = Some(runtime);
        self.window = Some(handle);
        Ok(())
    }

    fn record(&mut self, e: SdkError) {
        if self.error.is_none() {
            self.error = Some(e);
        }
    }
}

impl<A: App> PlatformHandler for Host<A> {
    fn window_ready(&mut self, window: PlatformWindow) -> Result<(), HandlerError> {
        self.start(window).map_err(|e| Box::new(e) as HandlerError)
    }

    fn input(&mut self, event: InputEvent) {
        if let Some(rt) = self.runtime.as_mut() {
            rt.handle_input(&mut self.app, &event);
        }
    }

    fn redraw(&mut self, dt: f64) -> LoopControl {
        let Some(rt) = self.runtime.as_mut() else {
            return LoopControl::Continue;
        };
        #[cfg(feature = "egui-overlay")]
        let result = {
            let overlay = &mut self.overlay;
            let scale = self
                .window
                .as_ref()
                .map_or(1.0, |w| w.scale_factor() as f32);
            rt.frame_with(&mut self.app, dt, |backend, app| {
                overlay.stage(backend, app, scale, dt);
            })
        };
        #[cfg(not(feature = "egui-overlay"))]
        let result = rt.frame(&mut self.app, dt);
        match result {
            Ok(control) => {
                if let (Some(title), Some(window)) = (rt.take_window_title(), self.window.as_ref())
                {
                    window.set_title(&title);
                }
                match control {
                    AppControl::Continue => LoopControl::Continue,
                    AppControl::Exit => LoopControl::Exit,
                }
            }
            Err(e) => {
                engine_diagnostics::tracing::error!(error = %e, "frame failed");
                self.record(e);
                LoopControl::Exit
            }
        }
    }

    fn exiting(&mut self) {
        if let Some(rt) = self.runtime.as_mut()
            && let Err(e) = rt.shutdown()
        {
            self.record(e);
        }
    }
}

/// Open a window and run `app` on the wgpu backend until it exits.
///
/// # Errors
/// Returns the first error from startup, a frame, shutdown or the platform loop.
pub fn run_app<A: App>(app: A, config: AppConfig) -> Result<(), SdkError> {
    let mut engine = Engine::new(config.engine);
    if let Err(e) = engine.install_diagnostics() {
        engine_diagnostics::tracing::warn!(error = %e, "diagnostics already installed");
    }
    let mut host = Host {
        app,
        engine: Some(engine),
        runtime: None,
        error: None,
        present: config.present,
        window: None,
        #[cfg(feature = "egui-overlay")]
        overlay: crate::overlay::OverlayHost::default(),
    };
    match engine_platform::run(config.window, &mut host) {
        Ok(()) => {}
        Err(PlatformError::Handler(e)) => {
            return Err(match e.downcast::<SdkError>() {
                Ok(sdk) => *sdk,
                Err(other) => SdkError::Platform(PlatformError::Handler(other)),
            });
        }
        Err(e) => return Err(SdkError::Platform(e)),
    }
    match host.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_present_config_is_fifo_two() {
        assert_eq!(AppConfig::default().present, PresentConfig::default());
        assert_eq!(AppConfig::default().present.frame_latency.get(), 2);
    }
}
