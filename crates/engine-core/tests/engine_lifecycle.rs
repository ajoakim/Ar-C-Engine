//! End-to-end lifecycle tests through the public Engine API.

// EngineError embeds ron errors (cold path); large Err variant is acceptable.
#![allow(clippy::result_large_err)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use engine_core::{
    Engine, EngineConfig, EngineError, EnginePhase, Module, ModuleConfig, ModuleError,
    RegistryError,
};

type Log = Arc<Mutex<Vec<String>>>;

/// Serialises the tests that tick an engine. The tracing callsite interest
/// cache is process-global: a callsite first hit by another test thread while
/// no subscriber is set can be cached as "never" just as the span test installs
/// its thread-local subscriber, so its spans were occasionally missed
/// (`left: None, right: Some(2)`). Ticking one engine at a time removes the race.
static TICK_LOCK: Mutex<()> = Mutex::new(());

fn tick_guard() -> std::sync::MutexGuard<'static, ()> {
    TICK_LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

fn push(log: &Log, s: String) {
    if let Ok(mut g) = log.lock() {
        g.push(s);
    }
}

fn snapshot(log: &Log) -> Vec<String> {
    log.lock().map(|g| g.clone()).unwrap_or_default()
}

#[derive(serde::Deserialize, Default)]
struct AlphaCfg {
    greeting: String,
}

struct Recorder {
    name: &'static str,
    deps: &'static [&'static str],
    log: Log,
    fail_load: bool,
}

impl Recorder {
    fn boxed(
        name: &'static str,
        deps: &'static [&'static str],
        log: &Log,
        fail_load: bool,
    ) -> Box<dyn Module> {
        Box::new(Self {
            name,
            deps,
            log: Arc::clone(log),
            fail_load,
        })
    }
    fn rec(&self, hook: &str) {
        push(&self.log, format!("{}:{hook}", self.name));
    }
}

impl Module for Recorder {
    fn name(&self) -> &'static str {
        self.name
    }
    fn dependencies(&self) -> &[&'static str] {
        self.deps
    }
    fn init(&mut self, config: &ModuleConfig<'_>) -> Result<(), ModuleError> {
        if self.name == "alpha" {
            let cfg = config.parse_or_default::<AlphaCfg>()?;
            push(&self.log, format!("alpha:init:{}", cfg.greeting));
        } else {
            self.rec("init");
        }
        Ok(())
    }
    fn load(&mut self) -> Result<(), ModuleError> {
        if self.fail_load {
            return Err(ModuleError::Message("boom".into()));
        }
        self.rec("load");
        Ok(())
    }
    fn update(&mut self, _dt: f64) -> Result<(), ModuleError> {
        self.rec("update");
        Ok(())
    }
    fn suspend(&mut self) {
        self.rec("suspend");
    }
    fn resume(&mut self) {
        self.rec("resume");
    }
    fn shutdown(&mut self) {
        self.rec("shutdown");
    }
}

const CFG: &str =
    r#"(engine: (app_name: "it", frame_history: 4), modules: { "alpha": (greeting: "hi") })"#;

fn two_module_engine(log: &Log) -> Result<Engine, EngineError> {
    let mut e = Engine::from_ron_str(CFG)?;
    e.register_module(Recorder::boxed("beta", &["alpha"], log, false))?;
    e.register_module(Recorder::boxed("alpha", &[], log, false))?;
    Ok(e)
}

#[test]
fn engine_lifecycle_full_run() -> Result<(), EngineError> {
    let _tick = tick_guard();
    let log: Log = Arc::default();
    let mut e = two_module_engine(&log)?;
    e.initialize()?;
    e.load()?;
    e.start()?;
    let mut idx = Vec::new();
    idx.push(e.tick(0.016)?.frame_index);
    idx.push(e.tick(0.016)?.frame_index);
    e.suspend()?;
    e.resume()?;
    idx.push(e.tick(0.016)?.frame_index);
    e.shutdown()?;
    assert_eq!(
        snapshot(&log),
        [
            "alpha:init:hi",
            "beta:init",
            "alpha:load",
            "beta:load",
            "alpha:update",
            "beta:update",
            "alpha:update",
            "beta:update",
            "beta:suspend",
            "alpha:suspend",
            "alpha:resume",
            "beta:resume",
            "alpha:update",
            "beta:update",
            "beta:shutdown",
            "alpha:shutdown"
        ]
    );
    assert_eq!(idx, [0, 1, 2]);
    assert_eq!(e.phase(), EnginePhase::Shutdown);
    Ok(())
}

#[test]
fn engine_lifecycle_illegal_calls() -> Result<(), EngineError> {
    let log: Log = Arc::default();
    let mut e = two_module_engine(&log)?;
    e.initialize()?;
    assert!(matches!(e.start(), Err(EngineError::Lifecycle(_))));
    e.load()?;
    assert!(matches!(e.suspend(), Err(EngineError::Lifecycle(_))));
    e.start()?;
    assert!(matches!(e.resume(), Err(EngineError::Lifecycle(_))));
    e.shutdown()?;
    assert!(e.start().is_err());
    assert!(e.tick(0.016).is_err());
    assert!(e.suspend().is_err());
    assert!(e.resume().is_err());
    assert!(e.shutdown().is_err());
    Ok(())
}

#[test]
fn engine_lifecycle_module_error_propagates() -> Result<(), EngineError> {
    let log: Log = Arc::default();
    let mut e = Engine::new(EngineConfig::default());
    e.register_module(Recorder::boxed("failing", &[], &log, true))?;
    e.initialize()?;
    let err = e.load();
    assert!(matches!(
        err,
        Err(EngineError::Module {
            module: "failing",
            ..
        })
    ));
    e.shutdown()?;
    assert!(snapshot(&log).contains(&"failing:shutdown".to_owned()));
    Ok(())
}

#[test]
fn engine_lifecycle_missing_dependency() -> Result<(), EngineError> {
    let log: Log = Arc::default();
    let mut e = Engine::new(EngineConfig::default());
    e.register_module(Recorder::boxed("beta", &["alpha"], &log, false))?;
    assert!(matches!(
        e.initialize(),
        Err(EngineError::Registry(
            RegistryError::MissingDependency { .. }
        ))
    ));
    Ok(())
}

#[test]
fn engine_lifecycle_frame_stats_spans() -> Result<(), EngineError> {
    let _tick = tick_guard();
    let log: Log = Arc::default();
    let mut e = two_module_engine(&log)?;
    let (sub, handle) = engine_diagnostics::frame_timing_subscriber();
    e.attach_frame_timing(handle);
    e.initialize()?;
    e.load()?;
    e.start()?;
    let stats = tracing::subscriber::with_default(sub, || e.tick(0.016))?;
    let count = |n: &str| stats.spans.iter().find(|s| s.name == n).map(|s| s.count);
    assert_eq!(count("engine_frame"), Some(1));
    assert_eq!(count("module_update"), Some(2));
    assert!(stats.frame_time > Duration::ZERO || !stats.spans.is_empty());
    Ok(())
}

#[test]
fn engine_lifecycle_frame_history_from_config() -> Result<(), EngineError> {
    let _tick = tick_guard();
    let mut e = Engine::from_ron_str("(engine: (frame_history: 2))")?;
    e.initialize()?;
    e.load()?;
    e.start()?;
    for _ in 0..5 {
        e.tick(0.016)?;
    }
    assert_eq!(e.frame_history().count(), 2);
    assert_eq!(e.last_frame_stats().map(|s| s.frame_index), Some(4));
    Ok(())
}

#[test]
fn engine_lifecycle_bad_config() -> Result<(), EngineError> {
    assert!(matches!(
        Engine::from_ron_str("(engine: (oops: 1))"),
        Err(EngineError::Config(_))
    ));
    let mut e = Engine::from_ron_str(r#"(modules: { "ghost": (x: 1) })"#)?;
    e.initialize()?;
    Ok(())
}
