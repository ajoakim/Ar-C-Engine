# ADR-0016: Present-Mode Setting and Window Readout

- Status: accepted
- Date: 2026-11-26
- Requirement/Phase: PERF-01, PERF-02 / Phase 7.1

## Context

Windowed flights showed frames of about 31 ms in some runs (Fifo, frame latency 2, monitor refresh unknown) against 16.7 ms elsewhere. Before the final human flight we need to know whether those frames are CPU-bound or wait on the present, and the user needs a way to choose the mode and to read altitude, speed and level while flying. The on-screen overlay stays Phase 8 (QA-02), so the readout goes into the window title.

## Decision

1. `PresentModeSetting { Fifo, Mailbox, Immediate, Auto }` and a validated `FrameLatency` (1 to 3) form `PresentConfig` in `engine-render-wgpu` (generic surface configuration, no planet wording). The default is `PresentModeSetting::Fifo` with latency 2, unchanged behaviour.
2. `resolve_present_mode(request, supported)` is a pure function over the modes the surface reports: Fifo is always Fifo; Mailbox and Immediate resolve to themselves when supported and otherwise to Fifo with `fell_back = true` (logged with `tracing::warn!`); Auto is Mailbox when supported else Fifo and never Immediate; an empty list resolves to Fifo.
3. `WgpuBackend::new_with(target, w, h, &PresentConfig)`; `new` delegates with the default; offscreen construction accepts and ignores the config. `PresentInfo` (strings only) reports requested mode, active mode, supported modes, latency and fallback, so the SDK needs no wgpu types.
4. Mechanism. `AppConfig.present` reaches `Host::start`, which clones the `PlatformWindow` before it is moved into the `Arc<dyn RenderTarget>`, reads `monitor_refresh_mhz()`, builds `StartupInfo { adapter, present, monitor_refresh_mhz }` and calls `Runtime::set_startup_info` before `setup`. `SceneState.startup` carries it to `App::setup` (the planetary app prints the startup line); `SceneState.window_title` is taken by the host after every successful frame and applied with `PlatformWindow::set_title`. Headless and null runs never have a host window, so the title is ignored there.
5. The title (`planetary-app/src/title.rs`) is `format_title(altitude, speed, topology, level)`, for example `alt 12.3 km | 1.2 km/s | hex L14`, throttled by frame count (every 15th frame, no clock). Speed is the camera displacement over the frame `dt`.
6. Flags `--present-mode fifo|mailbox|immediate|auto` and `--frame-latency 1|2|3` are strict (usage error, exit 1), plus `--no-haze` for `AppSettings.haze`.
7. Default rule: the default changes only if Mailbox is supported and its p95 frame time is at least 10 percent lower than Fifo with max not worse. Mailbox is not supported on the measured machine, so the default stays Fifo.

## Alternatives Considered

- Poll the monitor every frame from the app: rejected; the app has no window handle, and the refresh rate is stable enough to read once at startup.
- Global statics for startup info and title: rejected; hidden state, untestable.
- App-owned window: rejected; the platform crate owns the event loop and the SDK host is the one place that sees the window.
- A fixed present mode with no setting: rejected; the question is machine-dependent.

## Consequences

- The SDK exposes `PresentConfig`, `PresentModeSetting`, `FrameLatency`, `PresentInfo` and `StartupInfo`; none carries a GPU API type.
- Immediate can tear. It is a diagnostic and a user choice, never the default.
- The title costs one `set_title` call about four times per second.

## Evidence

- Pure selection, parsing and defaults: `present::tests` in `engine-render-wgpu`; title formatting and throttle: `title::tests`; throttle frames in a headless app: `window_title_is_set_only_on_throttle_frames`; startup info before `setup`: `startup_info_reaches_setup_before_the_first_frame`; flag parsing: the example's unit tests.
- Measured run (below) on a real window.

## Measurements (addendum)

Machine: Intel(R) Arc(TM) B580 Graphics (Vulkan), Windows 11, monitor refresh reported by winit 60.000 Hz, surface supported modes `[Immediate, Fifo]` (Mailbox is not supported). Command per row: `timeout 150 ./target/release/examples/planetary_flight.exe --descent --fast --frames 400 --present-mode <mode> [--frame-latency <n>] --perf-log target/diag/present-<name>.csv`; every run exited 0 (the program's own exit code). 400 frames per run, release build, all frames included. `cpu` is the average of prepare_ms plus gpu_frame_ms.

| Request | Active | Latency | avg ms | p95 ms | max ms | prepare ms | gpu_frame ms | cpu ms | frames over 25 ms |
|---|---|---|---|---|---|---|---|---|---|
| fifo | Fifo | 2 | 31.15 | 34.43 | 46.26 | 6.97 | 3.88 | 10.85 | 395 |
| fifo (repeat) | Fifo | 2 | 31.25 | 34.88 | 49.52 | 7.16 | 3.76 | 10.92 | 393 |
| fifo | Fifo | 1 | 31.21 | 34.86 | 51.22 | 7.11 | 3.71 | 10.81 | 396 |
| mailbox | Fifo (fell back, unsupported) | 2 | 31.02 | 34.00 | 48.68 | 7.05 | 3.73 | 10.77 | 394 |
| auto | Fifo (no Mailbox) | 2 | 31.12 | 34.06 | 45.74 | 7.06 | 3.78 | 10.84 | 395 |
| immediate | Immediate | 2 | 4.63 | 11.48 | 12.95 | 3.01 | 1.06 | 4.07 | 0 |

Reading. Under Fifo the frame time is about 31 ms while the CPU work (prepare plus gpu_frame) is about 11 ms, so roughly 20 ms per frame is spent waiting in the present, not computing: the 31 ms runs are present-bound, not CPU-bound. The 31 ms is about two 60 Hz intervals (33.3 ms) and frame latency 1 does not change it. Under Immediate the same flight runs at 4.6 ms average with 4.1 ms of CPU work, so the engine itself has large headroom on this machine. Cause of the two-interval cadence under Fifo was not isolated (compositor or driver pacing of a window started from a shell are candidates); it was not investigated further here.

Recommendation for the final flight: keep Fifo as the default (the default rule is not met: Mailbox is unsupported on this machine and Immediate is not Mailbox). Fly with `--present-mode immediate` to judge the engine without the present wait, and report the refresh rate and the preferred mode from the startup line and title. Whether Mailbox behaves differently on the user's machine is open (Pending, human).
