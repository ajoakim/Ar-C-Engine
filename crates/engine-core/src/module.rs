//! The [`Module`] trait implemented by engine subsystems.

use crate::config::ModuleConfig;
use crate::error::ModuleError;

/// An engine subsystem with lifecycle hooks.
///
/// `resume` is an added default no-op hook: `Suspend -> Running` is a legal
/// lifecycle transition, so modules need a counterpart to `suspend`.
pub trait Module: Send {
    /// Unique module name (also the config section key).
    fn name(&self) -> &'static str;

    /// Names of modules that must be initialized before this one.
    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    /// Called during the Initialize phase with this module's config section.
    fn init(&mut self, _config: &ModuleConfig<'_>) -> Result<(), ModuleError> {
        Ok(())
    }

    /// Called during the Load phase.
    fn load(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    /// Called every frame while Running.
    fn update(&mut self, _dt: f64) -> Result<(), ModuleError> {
        Ok(())
    }

    /// Called when the engine suspends.
    fn suspend(&mut self) {}

    /// Called when the engine resumes from suspend.
    fn resume(&mut self) {}

    /// Called on shutdown (reverse dependency order).
    fn shutdown(&mut self) {}
}
