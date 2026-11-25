//! The [`Engine`] driver: config, modules, guarded lifecycle and per-frame stats.

use std::path::Path;

use engine_diagnostics::{DiagnosticsConfig, FrameClock, FrameStats, FrameTimingHandle};

use crate::config::EngineConfig;
use crate::error::{EngineError, LifecycleError};
use crate::lifecycle::{EnginePhase, Lifecycle};
use crate::module::Module;
use crate::registry::ModuleRegistry;

/// Drives modules through the engine lifecycle.
pub struct Engine {
    config: EngineConfig,
    lifecycle: Lifecycle,
    registry: ModuleRegistry,
    clock: FrameClock,
    /// Names of modules whose `init` succeeded.
    initialized: Vec<&'static str>,
    init_attempted: bool,
}

impl Engine {
    /// Create an engine from a parsed configuration.
    pub fn new(config: EngineConfig) -> Self {
        let clock = FrameClock::new(None, config.engine.frame_history);
        Self {
            config,
            lifecycle: Lifecycle::new(),
            registry: ModuleRegistry::new(),
            clock,
            initialized: Vec::new(),
            init_attempted: false,
        }
    }

    /// Create an engine from RON text.
    pub fn from_ron_str(text: &str) -> Result<Self, EngineError> {
        Ok(Self::new(EngineConfig::from_ron_str(text)?))
    }

    /// Create an engine from a RON config file.
    pub fn from_config_file(path: &Path) -> Result<Self, EngineError> {
        Ok(Self::new(EngineConfig::load(path)?))
    }

    /// The engine configuration.
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Current lifecycle phase.
    pub fn phase(&self) -> EnginePhase {
        self.lifecycle.phase()
    }

    /// Register a module; only allowed before `initialize`.
    pub fn register_module(&mut self, module: Box<dyn Module>) -> Result<(), EngineError> {
        let phase = self.phase();
        if phase != EnginePhase::Initialize || self.init_attempted {
            return Err(EngineError::RegistrationClosed { phase });
        }
        self.registry.register(module)?;
        Ok(())
    }

    /// Install the global tracing subscriber and attach frame timing.
    pub fn install_diagnostics(&mut self) -> Result<(), EngineError> {
        let handle = engine_diagnostics::init(&DiagnosticsConfig {
            filter: self.config.engine.log_filter.clone(),
            ansi: true,
        })?;
        self.clock.set_timing(Some(handle));
        Ok(())
    }

    /// Attach a span timing source (for custom subscribers).
    pub fn attach_frame_timing(&mut self, handle: FrameTimingHandle) {
        self.clock.set_timing(Some(handle));
    }

    fn log_phase(&self) {
        tracing::info!(phase = ?self.phase(), "engine phase");
    }

    /// Initialize modules in dependency order, handing each its config section.
    pub fn initialize(&mut self) -> Result<(), EngineError> {
        if self.init_attempted {
            return Err(EngineError::AlreadyInitialized);
        }
        let phase = self.phase();
        if phase != EnginePhase::Initialize {
            return Err(LifecycleError::IllegalTransition {
                from: phase,
                to: EnginePhase::Initialize,
            }
            .into());
        }
        self.init_attempted = true;
        let Engine {
            config,
            registry,
            initialized,
            ..
        } = self;
        let order = registry.resolve_order()?;
        for section in config.module_section_names() {
            if !order.contains(&section) {
                tracing::warn!(section, "config section has no registered module");
            }
        }
        registry.for_each_in_order(|m| {
            let cfg = config.module_config(m.name());
            m.init(&cfg).map_err(|source| EngineError::Module {
                module: m.name(),
                source,
            })?;
            initialized.push(m.name());
            Ok::<(), EngineError>(())
        })?;
        self.log_phase();
        Ok(())
    }

    /// Enter the Load phase and call each module's `load`.
    pub fn load(&mut self) -> Result<(), EngineError> {
        if !self.init_attempted || self.initialized.len() != self.registry.len() {
            return Err(EngineError::NotInitialized);
        }
        self.lifecycle.transition(EnginePhase::Load)?;
        self.log_phase();
        self.registry.for_each_in_order(|m| {
            m.load().map_err(|source| EngineError::Module {
                module: m.name(),
                source,
            })
        })
    }

    /// Enter the Running phase.
    pub fn start(&mut self) -> Result<(), EngineError> {
        self.lifecycle.transition(EnginePhase::Running)?;
        self.log_phase();
        Ok(())
    }

    /// Run one frame, returning its stats.
    pub fn tick(&mut self, dt: f64) -> Result<FrameStats, EngineError> {
        let phase = self.phase();
        if phase != EnginePhase::Running {
            return Err(EngineError::NotRunning { phase });
        }
        self.clock.begin_frame();
        let frame_span = tracing::info_span!("engine_frame", dt).entered();
        let result = self.registry.for_each_in_order(|m| {
            let name = m.name();
            tracing::info_span!("module_update", module = name)
                .in_scope(|| m.update(dt))
                .map_err(|source| EngineError::Module {
                    module: name,
                    source,
                })
        });
        drop(frame_span);
        match result {
            Ok(()) => {
                let stats = self.clock.end_frame()?;
                tracing::trace!(frame = stats.frame_index, ?stats.frame_time, "frame complete");
                Ok(stats)
            }
            Err(e) => {
                let _ = self.clock.end_frame();
                Err(e)
            }
        }
    }

    /// Suspend; modules are suspended in reverse order.
    pub fn suspend(&mut self) -> Result<(), EngineError> {
        self.lifecycle.transition(EnginePhase::Suspend)?;
        self.log_phase();
        self.registry.for_each_in_reverse(|m| {
            m.suspend();
            Ok::<(), EngineError>(())
        })
    }

    /// Resume from Suspend; modules are resumed in order.
    pub fn resume(&mut self) -> Result<(), EngineError> {
        let phase = self.phase();
        if phase != EnginePhase::Suspend {
            return Err(LifecycleError::IllegalTransition {
                from: phase,
                to: EnginePhase::Running,
            }
            .into());
        }
        self.lifecycle.transition(EnginePhase::Running)?;
        self.log_phase();
        self.registry.for_each_in_order(|m| {
            m.resume();
            Ok::<(), EngineError>(())
        })
    }

    /// Shut down initialized modules in reverse order.
    pub fn shutdown(&mut self) -> Result<(), EngineError> {
        self.lifecycle.transition(EnginePhase::Shutdown)?;
        self.log_phase();
        let initialized = self.initialized.clone();
        if !initialized.is_empty() {
            self.registry.for_each_in_reverse(|m| {
                if initialized.contains(&m.name()) {
                    m.shutdown();
                }
                Ok::<(), EngineError>(())
            })?;
        }
        tracing::info!(app = %self.config.engine.app_name, "engine shut down");
        Ok(())
    }

    /// Most recent frame stats.
    pub fn last_frame_stats(&self) -> Option<&FrameStats> {
        self.clock.last()
    }

    /// Bounded frame history, oldest first.
    pub fn frame_history(&self) -> impl Iterator<Item = &FrameStats> + '_ {
        self.clock.history()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Noop;
    impl Module for Noop {
        fn name(&self) -> &'static str {
            "noop"
        }
    }

    #[test]
    fn engine_new_starts_in_initialize() {
        let e = Engine::new(EngineConfig::default());
        assert_eq!(e.phase(), EnginePhase::Initialize);
    }

    #[test]
    fn engine_tick_requires_running() {
        let mut e = Engine::new(EngineConfig::default());
        assert!(matches!(
            e.tick(0.016),
            Err(EngineError::NotRunning {
                phase: EnginePhase::Initialize
            })
        ));
    }

    #[test]
    fn engine_load_requires_initialize() {
        let mut e = Engine::new(EngineConfig::default());
        assert!(matches!(e.load(), Err(EngineError::NotInitialized)));
    }

    #[test]
    fn engine_register_after_initialize() {
        let mut e = Engine::new(EngineConfig::default());
        assert!(e.initialize().is_ok());
        assert!(matches!(
            e.register_module(Box::new(Noop)),
            Err(EngineError::RegistrationClosed { .. })
        ));
    }

    #[test]
    fn engine_initialize_twice() {
        let mut e = Engine::new(EngineConfig::default());
        assert!(e.initialize().is_ok());
        assert!(matches!(
            e.initialize(),
            Err(EngineError::AlreadyInitialized)
        ));
    }
}
