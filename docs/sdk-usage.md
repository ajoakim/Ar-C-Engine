# SDK Usage Guide

How to build, run and extend applications on the engine SDK (`engine-sdk`, tier 5). Everything here is copied from the source or an example header, or was run while writing this guide; where a snippet is an excerpt, its source file is named. Use `--release` for anything you fly: debug builds are several times slower. The repo pins Rust 1.99.0 (`rust-toolchain.toml`).

## Running the examples

There are three examples. Only the SDK crate is needed to build the first; the others live in extensions that use the SDK.

### `triangle` (SDK smoke test)

```sh
cargo run -p engine-sdk --example triangle
```

Opens a window with a rotating RGB triangle. WASD moves the camera, input events are logged, Esc or closing the window exits. It takes no flags and has no headless mode (source: `crates/engine-sdk/examples/triangle.rs`).

### `basic_scene` (conventional 3D reference app)

```sh
cargo run --release -p conventional-3d --example basic_scene
cargo run --release -p conventional-3d --example basic_scene -- --model path/to/model.glb
cargo run -p conventional-3d --example basic_scene -- --headless 30
```

A lit, textured glTF scene with a directional light and a fly camera. Usage from the example: `basic_scene [--model <path.glb>] [--headless <frames>]`. Controls: WASD/arrows move, E/Space up, Q/Ctrl down, Shift boost, hold the right mouse button to look, Esc quits. `--headless <n>` runs n frames on the null backend and exits non-zero if the last frame had no draws (it printed `30 frames, 10 draws in last frame, 30 submits` when this guide was written). If the model fails to import, a procedural cube is used.

### `planetary_flight` (planetary reference app)

```sh
# Interactive free flight
cargo run --release -p planetary-app --example planetary_flight

# Scripted orbit-to-surface descent with a per-frame CSV log
cargo run --release -p planetary-app --example planetary_flight -- --descent --perf-log flight.csv

# Headless smoke run on the null backend (deterministic)
cargo run -p planetary-app --example planetary_flight -- --headless 30

# Headless descent benchmark with per-altitude-band statistics
cargo run --release -p planetary-app --example planetary_flight -- --bench-descent
```

`--headless 30` printed `planetary_flight headless (hex): 30 frames, 53 draws in last frame, 120 meshes` when this guide was written. The complete flag list is the `USAGE` constant of the example; the planetary-app README documents each flag and the CSV columns.

| Flag | Meaning |
|------|---------|
| `--topology <hex\|cube>` | Planet topology (default `hex`); other values are a usage error |
| `--debug-cells` | Colour cells by level and id |
| `--headless <frames>` | Null backend, workers awaited every frame |
| `--descent`, `--ascent` | Scripted descent or ascent instead of the interactive camera |
| `--frames <n>` | Exit after n frames |
| `--perf-log <file.csv>` | One telemetry CSV row per frame |
| `--no-console` | Suppress the console telemetry summary |
| `--bench-descent`, `--budget-ms <ms>`, `--strict` | Headless descent benchmark; per-band p95 budget (default 16.7); exit non-zero on a miss |
| `--bench-stream`, `--fast`, `--settle`, `--no-adaptive`, `--frame-ms <ms>`, `--out <file.csv>` | Streaming harness and its controls |
| `--diagnostics` | Per-frame churn counters in the perf log, console line and `--out` |
| `--retain-frames <n>` | Frames a culled split cell stays split (default 8, 0 disables) |
| `--present-mode <fifo\|mailbox\|immediate\|auto>` | Surface present mode (default `fifo`) |
| `--frame-latency <1\|2\|3>` | Maximum frames in flight (default 2) |
| `--no-haze` | Turn the altitude haze and sky colour off |
| `--no-atmosphere` | Keep the plain haze but drop the sun-tinted sky and the limb ring (ADR-0019) |
| `--always-reselect` | Run the LOD selection every frame (same result, slower; for comparisons) |

Controls are the same as `basic_scene`. Immediate is a diagnostic present mode and can tear. A windowed run prints one startup line (adapter, requested and active present mode, supported modes, frame latency, monitor refresh) and shows `alt ... | ... km/s | hex L14` in the window title (ADR-0016).

## Writing an application

An application implements `engine_sdk::App` and is driven either by `run_app` (window and surface) or by a `Runtime` over any backend (headless).

### The `App` trait

`setup` creates GPU resources and fills the scene, `input` receives events, `update` advances the app and returns `AppControl::Continue` or `Exit`. The following skeleton is the head of `crates/engine-sdk/examples/triangle.rs`:

```rust
impl App for TriangleApp {
    fn setup(&mut self, res: &mut Resources<'_>, scene: &mut SceneState) -> Result<(), SdkError> {
        let mesh = res.create_mesh(
            "triangle",
            &[
                vertex([-0.5, -0.4, 0.0], [1.0, 0.0, 0.0]),
                vertex([0.5, -0.4, 0.0], [0.0, 1.0, 0.0]),
                vertex([0.0, 0.5, 0.0], [0.0, 0.0, 1.0]),
            ],
        )?;
        self.mesh = Some(mesh);
        let root = scene.frames.root();
        // -Z is forward (ADR-0001), so the triangle at z = 0 is in front of the camera at z = 2.
        scene.camera = Camera::new(WorldPos::from_xyz(root, 0.0, 6.4e6, 2.0)?);
        scene.items.push(RenderItem::new(
            mesh,
            WorldPos::from_xyz(root, 0.0, 6.4e6, 0.0)?,
        ));
        Ok(())
    }
```

and its `main`, which hands the app to the windowed loop:

```rust
fn main() -> Result<(), SdkError> {
    run_app(
        TriangleApp::default(),
        AppConfig {
            window: WindowConfig {
                title: "engine triangle".into(),
                ..Default::default()
            },
            ..Default::default()
        },
    )
}
```

`use engine_sdk::prelude::*;` brings in `App`, `AppConfig`, `AppControl`, `Camera`, `Resources`, `SceneState`, `SdkError`, `Transform`, `WorldPos`, `run_app` and the math types. Positions are f64 `WorldPos` values in a coordinate frame; rendering is camera-relative in f32 (ADR-0004). The coordinate conventions (Y up, -Z forward, SI units) are in ADR-0001.

### Per-frame GPU hook: `gpu_frame` (ADR-0009)

`App::gpu_frame(&mut self, resources, scene)` runs once per frame after `update` and the engine tick, before extraction, with access to `Resources`. Use it to upload or free resources produced at runtime, such as streamed terrain meshes (`create_lit_mesh`, `create_index_buffer`, `destroy_mesh`, `destroy_index_buffer`). The default does nothing. The app owns mesh lifetimes and must reconcile eviction against GPU residency; destroying a handle twice is a returned error, not a panic.

### Headless runs: `Runtime` and `frame_with`

`Runtime` drives frames against any `RenderBackend`. This is the headless branch of `extensions/conventional-3d/examples/basic_scene.rs`:

```rust
let mut rt = Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new())?;
rt.setup(&mut app)?;
for _ in 0..n {
    rt.frame(&mut app, 1.0 / 60.0)?;
}
let draws = rt
    .backend()
    .last_frame()
    .map_or(0, |f| f.passes.iter().map(|p| p.draws.len()).sum::<usize>());
```

One frame is: `update`, engine tick, `gpu_frame`, extract, build the frame packet, submit. `Runtime::frame_with(app, dt, before_submit)` is the same frame with a closure called with the backend and the app right before submit (not called if the app exits or the target is empty). `Runtime::frame` is `frame_with` with an empty hook. `run_app` uses the hook to stage the debug overlay; `crates/engine-sdk/tests/overlay_hook.rs` shows it headless:

```rust
rt.frame_with(&mut Idle, 0.016, |_, _| calls += 1)
    .expect("frame");
```

### Present mode

`AppConfig` has three fields: `window`, `engine` and `present`. `PresentConfig { mode: PresentModeSetting, frame_latency: FrameLatency }` defaults to `Fifo` with a latency of 2. `PresentModeSetting` is `Fifo`, `Mailbox`, `Immediate` or `Auto` (Mailbox when supported, otherwise Fifo, never Immediate). An unsupported request falls back to Fifo with a logged warning. `FrameLatency::new(n)` accepts 1 to 3 and returns an error otherwise. None of these types carries a GPU API type (ADR-0016). For example:

```rust
let config = AppConfig {
    present: PresentConfig {
        mode: PresentModeSetting::Immediate,
        ..PresentConfig::default()
    },
    ..AppConfig::default()
};
```

Before `setup`, the host fills `scene.startup` (`StartupInfo`: adapter string, `PresentInfo`, monitor refresh in millihertz); an app may set `scene.window_title` and the host applies it after the frame.

## Debug overlay (F3)

The overlay is read-only and off by default (ADR-0018). It needs two feature flags, both off in the default build so no egui code is compiled:

- `engine-sdk` feature `egui-overlay` adds `App::overlay_frame(&mut self) -> Option<DiagnosticsSnapshot>`, called once per frame after `update`, just before submit. Return `Some(snapshot)` to draw the overlay and `None` to draw nothing. The SDK owns the egui context and the painter in `engine-render-wgpu`; the application never names egui or GPU types. `DiagnosticsSnapshot`, `Budget` and `Histogram` are re-exported by the SDK.
- `planetary-app` feature `debug-overlay` turns that on for the planetary app: F3 toggles an overlay with LOD histograms, the streaming queue, memory and upload budgets and frame and prepare times. It is hidden at start.

```sh
cargo run --release -p planetary-app --features debug-overlay --example planetary_flight
```

Press F3 in the window to show it. Headless runs ignore the overlay.

## Tests, goldens and benchmarks

```sh
cargo test --workspace --locked                      # all unit and integration tests
cargo test -p xtask --locked --test docs_policy      # documentation policy tests
cargo xtask check-deps                               # layer and dependency rules
cargo xtask check-canary                             # conventional app must not reach planetary crates
cargo test -p engine-render-wgpu --locked --test image_diff -- --nocapture
```

The `image_diff` suite renders scenes on a software adapter (WARP on Windows, lavapipe on Linux) and compares them with goldens in `crates/engine-render-wgpu/tests/golden`. It reads these environment variables:

- `ARC_REQUIRE_ADAPTER=1`: a missing software adapter fails instead of skipping.
- `ARC_REQUIRE_GOLDENS=1`: a missing golden fails instead of skipping.
- `ARC_UPDATE_GOLDENS=1`: rewrite the goldens. Only do this deliberately and review the images; CI sets it only in the manual update workflow.

Benchmarks are criterion benches. Run them per target, because `--benches` also selects library test targets that reject criterion's flags:

```sh
cargo bench -p engine-spatial --bench spatial --locked -- --noplot --quick
cargo bench -p planetary-core --bench select --locked -- --noplot --quick
cargo bench -p planetary-core --bench mesh --locked -- --noplot --quick
cargo bench -p planetary-app --bench stream --locked -- --noplot --quick
cargo bench -p ecs-bench --bench ecs --locked -- --noplot --quick
```

Measured results are in `.planning/phases/08-hardening-decisions/08-BENCH-RESULTS.md`. See also [limitations.md](limitations.md) for what is and is not covered, and the [architecture overview](architecture-overview.md) for where each crate sits.
