//! App-shaped example: config -> modules -> full lifecycle with diagnostics.

use engine_core::{Engine, Module, ModuleConfig, ModuleError};

#[derive(serde::Deserialize, Default)]
struct CounterCfg {
    start: u64,
}

#[derive(Default)]
struct Counter {
    value: u64,
}

impl Module for Counter {
    fn name(&self) -> &'static str {
        "counter"
    }
    fn init(&mut self, config: &ModuleConfig<'_>) -> Result<(), ModuleError> {
        self.value = config.parse_or_default::<CounterCfg>()?.start;
        tracing::info!(start = self.value, "counter initialized");
        Ok(())
    }
    fn update(&mut self, _dt: f64) -> Result<(), ModuleError> {
        self.value += 1;
        tracing::debug!(value = self.value, "counter updated");
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = Engine::from_ron_str(
        r#"(engine: (app_name: "lifecycle-example", log_filter: "debug", frame_history: 8), modules: { "counter": (start: 10) })"#,
    )?;
    engine.install_diagnostics()?;
    engine.register_module(Box::new(Counter::default()))?;
    engine.initialize()?;
    engine.load()?;
    engine.start()?;
    for _ in 0..3 {
        let s = engine.tick(0.016)?;
        println!(
            "frame {}: {:?} spans={}",
            s.frame_index,
            s.frame_time,
            s.spans.len()
        );
    }
    engine.suspend()?;
    engine.resume()?;
    let s = engine.tick(0.016)?;
    println!(
        "frame {}: {:?} spans={}",
        s.frame_index,
        s.frame_time,
        s.spans.len()
    );
    engine.shutdown()?;
    Ok(())
}
