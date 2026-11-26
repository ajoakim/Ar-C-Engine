//! Planetary reference app: orbit-to-surface flight over streamed terrain, SDK only.
//!
//! Controls: WASD/arrows move, E/Space up, Q/Ctrl down, Shift boost,
//! hold right mouse button to look, Esc quits.
//!
//! Usage: `planetary_flight [--headless <frames>] [--descent]`
//!
//! `--headless` runs on the null backend with `settle_each_frame` enabled so the smoke run
//! is deterministic (workers are awaited every frame); the windowed run never blocks.
#![allow(clippy::result_large_err)]

use engine_sdk::{
    AppConfig, Engine, EngineConfig, NullBackend, Runtime, SdkError, WindowConfig, run_app,
};
use planetary_app::{AppSettings, DescentScript, PlanetaryApp};

const USAGE: &str = "usage: planetary_flight [--headless <frames>] [--descent]";

fn main() -> Result<(), SdkError> {
    let mut headless: Option<u32> = None;
    let mut descent = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" => {
                let n = args
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    .ok_or_else(|| SdkError::App(USAGE.into()))?;
                headless = Some(n);
            }
            "--descent" => descent = true,
            _ => return Err(SdkError::App(USAGE.into())),
        }
    }

    let mut settings = AppSettings::default();
    if descent {
        settings.script = Some(DescentScript::orbit_to_surface(
            settings.terrain.terrain.radius,
        ));
    }

    if let Some(n) = headless {
        settings.terrain.settle_each_frame = true;
        let mut app = PlanetaryApp::new(settings)?;
        let mut rt = Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new())?;
        rt.setup(&mut app)?;
        for _ in 0..n {
            rt.frame(&mut app, 1.0 / 60.0)?;
        }
        let draws = rt
            .backend()
            .last_frame()
            .map_or(0, |f| f.passes.iter().map(|p| p.draws.len()).sum::<usize>());
        let held = app.report().held_meshes;
        println!(
            "planetary_flight headless: {n} frames, {draws} draws in last frame, {held} meshes"
        );
        app.shutdown();
        rt.shutdown()?;
        if draws == 0 {
            return Err(SdkError::App("headless run produced zero draws".into()));
        }
        return Ok(());
    }

    println!(
        "Controls: WASD/arrows move, E/Space up, Q/Ctrl down, Shift boost, right mouse look, Esc exit"
    );
    let app = PlanetaryApp::new(settings)?;
    run_app(
        app,
        AppConfig {
            window: WindowConfig {
                title: "planetary_flight".into(),
                ..Default::default()
            },
            engine: EngineConfig::default(),
        },
    )
}
