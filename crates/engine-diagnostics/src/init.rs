//! Global subscriber installation.

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::error::DiagnosticsError;
use crate::layer::{FrameTimingHandle, FrameTimingLayer};

/// Configuration for [`init`].
#[derive(Clone, Debug)]
pub struct DiagnosticsConfig {
    /// `EnvFilter` directive string.
    pub filter: String,
    /// Whether to emit ANSI colors.
    pub ansi: bool,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            filter: "info".into(),
            ansi: true,
        }
    }
}

/// Installs the global subscriber (filter + fmt + frame timing).
///
/// # Errors
/// [`DiagnosticsError::InvalidFilter`] if the filter does not parse (nothing is
/// installed); [`DiagnosticsError::AlreadyInitialized`] if a global subscriber
/// already exists.
pub fn init(config: &DiagnosticsConfig) -> Result<FrameTimingHandle, DiagnosticsError> {
    let filter = tracing_subscriber::EnvFilter::try_new(&config.filter).map_err(|e| {
        DiagnosticsError::InvalidFilter {
            filter: config.filter.clone(),
            reason: e.to_string(),
        }
    })?;
    let (layer, handle) = FrameTimingLayer::new();
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_ansi(config.ansi))
        .with(layer)
        .try_init()
        .map_err(|_| DiagnosticsError::AlreadyInitialized)?;
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_invalid_filter() {
        let r = init(&DiagnosticsConfig {
            filter: "info,[".into(),
            ansi: false,
        });
        assert!(matches!(r, Err(DiagnosticsError::InvalidFilter { .. })));
    }
}
