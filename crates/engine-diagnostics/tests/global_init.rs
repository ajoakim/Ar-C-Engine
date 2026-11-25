//! Global subscriber install; single test so the global default is set once.

use engine_diagnostics::{init, DiagnosticsConfig, DiagnosticsError};

#[test]
fn global_init_once() {
    let handle = init(&DiagnosticsConfig::default()).expect("first init");
    tracing::info_span!("global_probe").in_scope(|| tracing::info!("hello"));
    assert!(handle.snapshot().iter().any(|t| t.name == "global_probe"));
    assert!(matches!(
        init(&DiagnosticsConfig::default()),
        Err(DiagnosticsError::AlreadyInitialized)
    ));
}
