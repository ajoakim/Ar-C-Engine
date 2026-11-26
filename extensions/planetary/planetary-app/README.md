# planetary-app

Planetary reference app: an orbit-to-surface flight over streamed cube-sphere terrain, built only on the public engine SDK. It lives in the planetary extension; engine crates never depend on it.

## Running

Use `--release` for flights. Debug builds are several times slower (mesh generation and selection are CPU work).

```
cargo run --release -p planetary-app --example planetary_flight -- --descent --perf-log flight.csv
```

A release build reuses `target/release` and needs about 3 GB of free disk. Keep the drive with `target/` from filling up.

## Controls

WASD/arrows move, E/Space up, Q/Ctrl down, Shift boost, hold right mouse button to look, Esc quits.

## Flags

| Flag | Meaning |
|---|---|
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
| `--out <file.csv>` | `--bench-stream` per-frame CSV (wanted/drawn level at the camera, caps, cost, uploads) |

Fly with `--release`. Example: `cargo run --release -p planetary-app --example planetary_flight -- --bench-stream --fast --out stream.csv`. Tests use the same harness through `planetary_app::stream_bench` (`tests/streaming_lag.rs`), paced with `PlanetaryApp::wait_for_workers` instead of a clock.

## Telemetry CSV columns

```
frame,sim_time_s,frame_ms,prepare_ms,gpu_frame_ms,altitude_m,nominal_altitude_m,center_distance_m,nodes_visited,leaves,wanted_leaves,drawn_leaves,held_meshes,pending,in_flight,awaiting_upload,started,completions,uploads,uploaded_bytes,evicted,cancelled,destroyed,upload_rejects,pending_requests,draw_items,culled_draws,tau_eff,forced_splits,trimmed_splits,culled_horizon,culled_frustum,node_budget_hit,leaf_budget_hit
```

## Reading the bench table

`--bench-descent` prints one row per altitude band: frames, avg / p95 / max frame ms, fps, stalls (frames over 100 ms), and maxima of nodes visited, wanted leaves, drawn leaves, held meshes and pending requests. The budget is p95 <= 16.7 ms in every band. The bench is headless and measures CPU time only (selection, streaming, draw building); GPU time is not included.
