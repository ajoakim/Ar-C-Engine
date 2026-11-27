# planetary-app

Planetary reference app: an orbit-to-surface flight over streamed terrain, built only on the public engine SDK. The planet is the hexagonal icosahedral hierarchy by default (`--topology hex`, ADR-0011); the cube-sphere stays as the reference and fallback (`--topology cube`). It lives in the planetary extension; engine crates never depend on it.

## Running

Use `--release` for flights. Debug builds are several times slower (mesh generation and selection are CPU work).

```
cargo run --release -p planetary-app --example planetary_flight -- --descent --perf-log flight.csv
```

A release build reuses `target/release` and needs about 3 GB of free disk. Keep the drive with `target/` from filling up.

## Controls

| Keys | Action |
| --- | --- |
| W/S, Up/Down | forward / back |
| A/D, Left/Right | strafe left / right (camera right) |
| E/Space, Q/Ctrl | up / down (camera up in free-flight, planet up when planet-aligned) |
| Z/X roll | roll left / right about the view axis (free-flight) |
| F toggles | free-flight / planet-aligned |
| Shift | boost |
| Right mouse + move | look |
| Esc | quit |

### Flight modes

- default = free-flight: full 6DOF with quaternion orientation. Pitch, yaw and roll are all about
  the camera axes; strafe and vertical thrust follow the camera right and up axes.
- F toggles to planet-aligned: the horizon is levelled (view direction kept), yaw is about planet
  up, vertical thrust follows planet up and pitch is clamped near the surface. F again returns to
  free-flight. The change is printed as `flight mode: <name>` and logged.
- The scripted `--descent` / `--ascent` flights stay planet-aligned and ignore these keys, so
  the descent behaviour is unchanged (CAM-05).
- The controller lives in `planetary-app` (`src/free_flight.rs`); no generic crate (engine-sdk,
  planetary-terrain camera, conventional-3d) was changed (CAM-06).

## Flags

| Flag | Meaning |
|---|---|
| `--topology <hex\|cube>` | Planet topology. `hex` is the default (16 cells per edge); `cube` is the reference / fallback (32 cells per edge). Other values are a usage error |
| `--debug-cells` | Colour every cell by level and id so hexes visibly give way to their parts (and the cube patches to their children) while descending |
| `--headless <frames>` | Run on the null backend (deterministic, workers awaited each frame) and print draw/mesh counts |
| `--descent` | Scripted orbit-to-surface descent (8 s simulated path) instead of the interactive camera |
| `--ascent` | Scripted surface-to-orbit ascent starting where the descent ends |
| `--frames <n>` | Exit by itself after n frames |
| `--perf-log <file.csv>` | Write one telemetry CSV row per frame |
| `--no-console` | Suppress the console telemetry summary |
| `--bench-descent` | Headless scripted descent at production defaults; prints a per-altitude-band table |
| `--budget-ms <ms>` | Per-band p95 budget for the bench (default 16.7) |
| `--strict` | Make a bench budget miss exit non-zero |
| `--bench-stream` | Windowed-realistic streaming harness (headless, free-running workers, simulated 1/30 s frames); prints lag at the camera, frames-to-detail and upload throughput |
| `--fast` | Fast free-flight descent (orbit to 10 m in 8/3 s); selects the script for `--bench-stream` and for `--descent` |
| `--settle` | `--bench-stream` control: workers awaited every frame (what the descent tests do) |
| `--no-adaptive` | Fixed per-frame streaming caps instead of the cost-adaptive ones (default adaptive for wall-clock runs) |
| `--frame-ms <ms>` | Real time spun per frame in `--bench-stream` so workers get a window-like slice (default 33) |
| `--present-mode <fifo\|mailbox\|immediate\|auto>` | Surface present mode (default `fifo`). An unsupported mode falls back to fifo with a warning; other values are a usage error. See ADR-0016 |
| `--frame-latency <1\|2\|3>` | Maximum frames in flight (default 2); other values are a usage error |
| `--no-haze` | Turn the altitude haze and sky colour off |
| `--no-atmosphere` | Keep the plain haze but drop the sun-tinted sky and the limb ring (ADR-0019) |
| `--always-reselect` | Run the LOD selection every frame. By default a selection is reused while the previous one was a fixed point of the hysteresis, hit no budget, no mesh was uploaded or destroyed since, and the eye and frustum moved less than the recorded margins (07.1-27); the result is identical either way (`tests/reselect_equality.rs`). The `reselect_skipped` CSV column and the per-scope `prep_*_ms` columns show the effect |
| `--out <file.csv>` | `--bench-stream` per-frame CSV (wanted/drawn level at the camera, caps, cost, uploads) |

Startup line and title. A windowed run prints one line at startup: `gpu: <adapter> | present: requested <mode>, active <mode>, supported [..], frame latency <n> | refresh: <x.xxx> Hz` (or `unknown`). The window title shows `alt 12.3 km | 1.2 km/s | hex L14` (altitude above the surface, speed, topology and the level of the drawn piece under the camera), refreshed every 15th frame. If the 31 ms frames appear, compare `frame_ms` with `prepare_ms + gpu_frame_ms` in the `--perf-log` CSV: a large gap means the frame waits for the present; try `--present-mode immediate` (may tear) or `mailbox` where supported.

Fly with `--release`. Example: `cargo run --release -p planetary-app --example planetary_flight -- --bench-stream --fast --out stream.csv`. Tests use the same harness through `planetary_app::stream_bench` (`tests/streaming_lag.rs`), paced with `PlanetaryApp::wait_for_workers` instead of a clock.

## Telemetry CSV columns

```
frame,sim_time_s,frame_ms,prepare_ms,gpu_frame_ms,altitude_m,nominal_altitude_m,center_distance_m,nodes_visited,leaves,wanted_leaves,drawn_leaves,held_meshes,pending,in_flight,awaiting_upload,started,completions,uploads,uploaded_bytes,evicted,cancelled,destroyed,upload_rejects,pending_requests,draw_items,culled_draws,tau_eff,forced_splits,trimmed_splits,culled_horizon,culled_frustum,node_budget_hit,leaf_budget_hit
```

## Reading the bench table

`--bench-descent` prints one row per altitude band: frames, avg / p95 / max frame ms, fps, stalls (frames over 100 ms), and maxima of nodes visited, wanted leaves, drawn leaves, held meshes and pending requests. The budget is p95 <= 16.7 ms in every band. The bench is headless and measures CPU time only (selection, streaming, draw building); GPU time is not included.
