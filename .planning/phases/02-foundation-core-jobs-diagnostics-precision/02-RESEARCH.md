# Phase 2: Foundation (Core, Jobs, Diagnostics, Precision) - Research

**Researched:** 2026-11-25
**Domain:** Rust engine foundation: lifecycle/module registry, priority job scheduler, tracing diagnostics, f64 world positions with frames and origin rebasing
**Confidence:** HIGH on stack and tier fit (versions verified via cargo search), MEDIUM on API-shape details (discretionary)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
**WorldPos / Precision**
- Opaque `WorldPos` wrapping f64 `DVec3` plus a `Frame` id; internals swappable later (i64 cell + offset remains a possible ADR spike).
- Coordinate spaces are explicit `Frame` handles with registered parent transforms; conversions go through frames, never implicit mixing.
- Origin rebasing is an explicit `rebase(new_origin)` that shifts registered positions; trigger policy is left to app/module.

**Lifecycle & Modules**
- `Module` trait with init/load/update/suspend/shutdown hooks; registry orders modules by declared dependencies.
- Typed `EngineConfig` loaded from RON (serde) at Initialize, with per-module sections.
- `Engine` state machine Initialize -> Load -> Running -> Suspend -> Shutdown; illegal transitions return errors (no panics).

**Jobs**
- Custom priority queue on std threads + crossbeam channels with priority classes and atomic cancel tokens.
- Cooperative cancellation: `CancelToken` checked by jobs; queued jobs dropped on cancel.
- Single-worker test mode for deterministic priority-order tests.

**Diagnostics**
- `tracing` with a thin `engine-diagnostics` wrapper that installs the subscriber.
- Per-frame `FrameStats` (frame time + named span timings) exposed via API for later overlays.

### Claude's Discretion
Exact type/method names, error types (thiserror in libraries), module file layout inside crates, test structure.

### Deferred Ideas (OUT OF SCOPE)
Automatic rebasing in the engine loop; i64-cell precision representation (ADR spike later).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CORE-01 | Lifecycle Initialize/Load/Running/Suspend/Shutdown | Explicit transition-table state machine returning `Result` (Pattern 1) |
| CORE-02 | Load config, register modules | serde+ron `EngineConfig`, dependency-ordered `ModuleRegistry` (Pattern 2) |
| CORE-04 | Prioritized, cancellable jobs | crossbeam-channel + BinaryHeap scheduler, `CancelToken` (Pattern 3) |
| CORE-05 | Logs, spans, frame timing | tracing + custom `Layer` collecting span timings into `FrameStats` (Pattern 4) |
| WORLD-02 | Opaque `WorldPos` across coordinate spaces | `WorldPos`/`Frame`/`FrameRegistry` in engine-math (Pattern 5) |
| WORLD-04 | Origin rebasing | `rebase` on a position set; round-trip test at 6.4e6 m (Pattern 5) |
</phase_requirements>

## Summary

All Phase 2 work fits the existing tier table without any change to crate-to-crate edges. `WorldPos`, `Frame`, and frame registry belong in `engine-math` (tier 0, "precision model" per its Cargo description), which only needs `glam`. `engine-diagnostics` (tier 0) needs `tracing` and `tracing-subscriber`. `engine-jobs` (tier 1) needs `crossbeam-channel` and `engine-diagnostics` (already declared). `engine-core` (tier 1) needs `serde`, `ron`, `thiserror`, plus existing `engine-math`/`engine-diagnostics`. The xtask checks (R1-R6) only police workspace-crate edges and wgpu/winit, so third-party additions need NO change to `xtask/src/layers.rs` or the dependency-direction doc. `deny.toml` licenses (MIT/Apache-2.0/BSD/ISC/Unicode-3.0/Zlib) cover all recommended crates [ASSUMED: transitive licenses; run `cargo deny check` in the phase to confirm].

Key design risk: precision. f64 gives ~1.2e-9 m resolution at 6.4e6 m (ulp = 2^-30 ~ 9.3e-10 m for values in [4.19e6, 8.39e6)), so rebase round-trips are not bit-exact in general but are within ~1e-9 m; tests must use tolerance (or pick binary-friendly values for exact asserts). Contrast: f32 at 6.4e6 has ulp 0.5 m, which is the motivating demonstration.

**Primary recommendation:** Put WorldPos/Frame in engine-math on glam `DVec3`; add third-party deps only via `[workspace.dependencies]`; build jobs with a `BinaryHeap` behind a `Mutex`+`Condvar` or channel-fed dispatcher; implement FrameStats as a tracing `Layer`.

## Architectural Responsibility Map

| Capability | Primary Tier (crate) | Secondary | Rationale |
|------------|---------------------|-----------|-----------|
| WorldPos, Frame, FrameRegistry, rebase | engine-math (0) | engine-core (re-use) | Pure math, no deps; lowest tier lets world/render/spatial all use it |
| Lifecycle state machine, Module trait, registry, EngineConfig | engine-core (1) | — | Spec'd in crate description |
| Job scheduler, CancelToken, priorities | engine-jobs (1) | engine-diagnostics (spans) | Already depends only on diagnostics |
| Subscriber install, FrameStats, span timing | engine-diagnostics (0) | — | Must not depend on core (tier 0) |
| Frame boundary calls (begin/end frame) | engine-core | engine-diagnostics | Engine loop drives `FrameStats` via diagnostics API |

## Standard Stack

### Core (versions verified with `cargo search` on 2026-11-25) [VERIFIED: cargo search]
| Library | Version | Crate | Purpose |
|---------|---------|-------|---------|
| glam | 0.34.3 | engine-math | `DVec3`, `DMat4`/`DQuat`, f64 math |
| tracing | 0.1.44 | engine-diagnostics (+ jobs/core for spans/macros) | spans/events |
| tracing-subscriber | 0.3.23 | engine-diagnostics | `Registry`, `fmt`, `EnvFilter`, custom `Layer` |
| crossbeam-channel | 0.5.17 | engine-jobs | MPMC channels between submitters and workers |
| serde | 1.0.229 (feature `derive`) | engine-core | config derive |
| ron | 0.12.3 | engine-core | RON config parse |
| thiserror | 2.0.21 | libs (core, jobs, math) | error enums per ADR-0001 / discretion |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom job queue | rayon | Locked decision says custom; rayon has no priority/cancel |
| Channel-only priority | `BinaryHeap` + `Mutex`/`Condvar` | crossbeam channels are FIFO; priority needs a heap (see Pitfall 1) |

**Installation:** add to root `[workspace.dependencies]`: `glam = "0.34.3"`, `tracing = "0.1.44"`, `tracing-subscriber = "0.3.23"` (features `registry`, `fmt`, `env-filter`), `crossbeam-channel = "0.5.17"`, `serde = { version = "1.0.229", features = ["derive"] }`, `ron = "0.12.3"`, `thiserror = "2.0.21"`; crates use `dep.workspace = true`. Note workspace `rust-version = "1.95"` vs toolchain 1.99.0; all crates support this [ASSUMED: MSRV of each; `cargo check` will confirm]. cargo-deny has `wildcards = "deny"`, so use explicit versions (done).

## Architecture Patterns

### System Architecture Diagram

```
App ──EngineConfig(RON)──> Engine::new ──> [Initialize]
                                  │ registry orders modules (topo sort by deps)
                          load()  v
                              [Load] ─ module.load() in order
                                  │ start()
                                  v
   frame loop: diagnostics.begin_frame ─> module.update(ctx) in order ─> end_frame ─> FrameStats
                                  │ suspend()/resume()      │ submit(job, priority)
                              [Suspend] <──────────> [Running]      v
                                  │ shutdown()        JobSystem: heap + workers ──CancelToken──> job
                              [Shutdown] (modules shut down in reverse order, workers joined)

Positions: WorldPos{frame, DVec3} ─FrameRegistry.convert(to)─> WorldPos in other frame
           rebase(new_origin): shifts all registered positions / frame origin; relative offsets preserved
```

### Recommended Project Structure
```
crates/engine-math/src/{lib.rs, frame.rs, world_pos.rs, rebase.rs}
crates/engine-diagnostics/src/{lib.rs, init.rs, frame_stats.rs, layer.rs}
crates/engine-jobs/src/{lib.rs, priority.rs, cancel.rs, scheduler.rs}
crates/engine-core/src/{lib.rs, lifecycle.rs, module.rs, registry.rs, config.rs, error.rs}
```

### Pattern 1: Lifecycle transition table
`enum EnginePhase {Initialize, Load, Running, Suspend, Shutdown}`; one `fn transition(&mut self, to) -> Result<(), LifecycleError>` with an explicit `matches!` allow-list: Initialize->Load, Load->Running, Running->Suspend, Suspend->Running (resume), Suspend->Shutdown, Running->Shutdown, Load->Shutdown, Initialize->Shutdown (abort). Everything else returns `LifecycleError::IllegalTransition{from,to}`. Resume (Suspend->Running) is a recommendation, since mobile/suspend implies it [ASSUMED]. Test exhaustively over all 25 pairs.

### Pattern 2: Module trait and registry
```rust
pub trait Module {
    fn name(&self) -> &'static str;
    fn dependencies(&self) -> &[&'static str] { &[] }
    fn init(&mut self, cfg: &ModuleConfig) -> Result<(), ModuleError> { Ok(()) }
    fn load(&mut self) -> Result<(), ModuleError> { Ok(()) }
    fn update(&mut self, dt: f64) -> Result<(), ModuleError> { Ok(()) }
    fn suspend(&mut self) {}
    fn shutdown(&mut self) {}
}
```
Registry: Kahn topological sort on `register` or at `Initialize`; errors `DuplicateModule`, `MissingDependency`, `DependencyCycle` (no panics). Shutdown in reverse order. Keep `Module` object-safe (`Box<dyn Module>`). Per-module config: `EngineConfig { engine: EngineSection, modules: BTreeMap<String, ron::Value> }` with `#[serde(default)]`; module deserializes its own section via `value.into_rust::<T>()` [ASSUMED: `ron::Value::into_rust` exists in 0.12; verify with docs before planning that task]. Use `ron::from_str` / `ron::de::SpannedError` mapped through thiserror.

### Pattern 3: Job scheduler
`enum Priority { Critical, High, Normal, Low, Background }` (ordered `Ord`). Queue: `Mutex<BinaryHeap<Entry{priority, seq, job}>>` + `Condvar`; `seq` (monotonic u64, reversed) gives FIFO within a class. `JobHandle` holds `CancelToken(Arc<AtomicBool>)` and optionally result via a crossbeam bounded(1) channel. `cancel()` sets flag; workers skip/drop queued jobs whose token is set at pop time; running jobs receive `&CancelToken`. Job closure type: `Box<dyn FnOnce(&CancelToken) + Send>`. Test mode: `JobSystem::with_workers(1)` plus submit-while-paused (`start_paused` / `resume`) so priority order is deterministic: enqueue many, then start worker, assert execution order. Shutdown: set flag, notify, join workers (no detached threads). Name worker threads and enter a tracing span per job.

### Pattern 4: Diagnostics
`init(DiagnosticsConfig) -> Result<DiagnosticsGuard, _>` uses `tracing_subscriber::registry().with(EnvFilter).with(fmt::layer()).with(FrameTimingLayer)` and `try_init` (returns error if already set; tests must tolerate). `FrameTimingLayer` implements `Layer<S: Subscriber + for<'a> LookupSpan<'a>>`: `on_new_span` stores `Instant` in span extensions; `on_close` accumulates `(name -> Duration, count)` into a shared `Arc<Mutex<..>>` accumulator. `FrameStats { frame_index, frame_time: Duration, spans: Vec<SpanTiming> }`; `Diagnostics::begin_frame()/end_frame() -> FrameStats` snapshots and resets accumulator, keeps a short ring of recent stats. Unit-test the layer using `tracing::subscriber::with_default` (scoped, avoids global-state clashes) rather than the global install.

### Pattern 5: WorldPos / Frames / rebase (engine-math)
```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameId(u32);
pub struct WorldPos { frame: FrameId, p: DVec3 }   // fields private
impl WorldPos {
    pub fn new(frame: FrameId, x: f64, y: f64, z: f64) -> Self;
    pub fn delta(&self, other: &WorldPos, reg: &FrameRegistry) -> Result<DVec3, FrameError>; // relative offset, same-frame fast path
    pub fn to_camera_relative_f32(&self, origin: &WorldPos, reg: &FrameRegistry) -> Result<Vec3, _>; // subtract in f64, then cast
}
pub struct FrameRegistry { /* parent: Option<FrameId>, transform to parent: DAffine3 */ }
```
Never expose raw `DVec3` as the public field or implement `Add`/`Sub` between WorldPos values (enforces "never implicit mixing"). Conversion composes parent transforms up to the common ancestor; cycle prevented since parents must pre-exist. Rebase: `FrameRegistry::rebase(frame, new_origin: DVec3)` shifts the frame's local origin, plus `WorldPos::rebase(&mut self, shift: DVec3)` / `rebase_all(&mut [WorldPos], ..)` for registered positions. Locked decision says rebase "shifts registered positions", so provide a `PositionSet`/handle-based store (e.g. `Vec<WorldPos>` owned by the registry with `PosId`) [discretion]. Required test: positions at x = 6.4e6 m (and 6.4e6 + small offsets such as 0.001 m), rebase to origin 6.4e6, then back; assert `delta` between pairs unchanged within 1e-8 m; also assert f32 at that magnitude loses sub-0.5 m precision (documents the why).

### Anti-Patterns to Avoid
- Public `DVec3` field or `From<DVec3>` on WorldPos (breaks opacity).
- `panic!`/`unwrap` for illegal transitions, cycles, bad config (workspace uses clippy `all` warn; planner should add `unwrap_used` only if desired).
- Global subscriber installs in multiple tests (`try_init` returns Err on second call).
- Sleeping in job tests; use channels/barriers/join.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| f64 vector/affine math | own Vec3d/matrix | glam `DVec3`, `DAffine3` | tested, SIMD |
| Log formatting / filtering | custom logger | tracing-subscriber fmt + EnvFilter | env filter, spans |
| Config parse | custom parser | serde + ron | spans/line errors |
| Error boilerplate | manual Display impls | thiserror | ADR-0001 style |
| Channels | Mutex<VecDeque> for results | crossbeam-channel | MPMC, timeouts |
| Topo sort | ad hoc ordering | small Kahn implementation (about 30 lines, OK) | no crate needed |

## Common Pitfalls

### Pitfall 1: Priority via channels
crossbeam channels are FIFO, so one channel per priority with `select` yields starvation/ordering ambiguity. Use heap under lock; reserve channels for results/notification. Warning sign: priority test flaky with multiple workers.

### Pitfall 2: Non-deterministic priority tests
Workers start popping before all jobs are enqueued. Provide a paused-start or manual `run_one()`/single-worker mode as locked decision says.

### Pitfall 3: Cancel race
Cancel after pop but before run: check token at the top of the job wrapper too. Dropping a queued cancelled job must still resolve its handle (state `Cancelled`) so waiters don't hang.

### Pitfall 4: f64 rebase round-trip exactness
`(p - o) + o` is not exact in floating point; use tolerance based on ulp (~1e-9 at 6.4e6). Rebase accumulation drifts over many rebases; document, and test N rebases.

### Pitfall 5: Global tracing state
`set_global_default` once per process; test binaries share it. Use scoped `with_default` in tests; `init` returns error on duplicate rather than panicking.

### Pitfall 6: Span timing semantic
`on_close` fires only when all handles dropped; for `#[instrument]` spans this is fine. Use `on_enter/on_exit` if cumulative busy time is wanted. Choose `on_close` total (simplest).

### Pitfall 7: Shutdown ordering and Drop
Joining threads in `Drop` can deadlock if a job blocks; provide explicit `shutdown()` and make Drop best-effort.

### Pitfall 8: Workspace plumbing
Crate Cargo.tomls need `dep.workspace = true`; new deps must be added to root `[workspace.dependencies]` or build fails. `unsafe_code = "forbid"` is workspace-wide, so no unsafe in job queue (fine with std/crossbeam).

## Code Examples

```rust
// Scoped subscriber test for span timing [CITED: docs.rs/tracing-subscriber Layer API]
let (layer, acc) = FrameTimingLayer::new();
let sub = tracing_subscriber::registry().with(layer);
tracing::subscriber::with_default(sub, || {
    let _s = tracing::info_span!("physics").entered();
});
assert!(acc.snapshot().iter().any(|s| s.name == "physics"));
```

```rust
// Priority order test, single worker
let jobs = JobSystem::builder().workers(1).start_paused(true).build();
let (tx, rx) = crossbeam_channel::unbounded();
for (p, n) in [(Priority::Low, 1), (Priority::High, 2), (Priority::Normal, 3)] {
    let tx = tx.clone();
    jobs.submit(p, move |_| tx.send(n).unwrap());
}
jobs.resume();
jobs.shutdown_and_drain();
assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![2, 3, 1]);
```

## State of the Art

| Old | Current | Impact |
|-----|---------|--------|
| thiserror 1.x | thiserror 2.0.21 | 2.x derive syntax; use 2 |
| tracing 0.1 | still 0.1.44 | stable line |
| Single-precision world | f64 world + camera-relative f32 | ADR-0001 convention |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Transitive licenses of new deps pass `cargo deny check licenses` | Stack | CI cargo-deny fails; add license to allow list |
| A2 | `ron::Value::into_rust` available/usable in 0.12.3 for per-module sections | Pattern 2 | Use `Box<RawValue>`-like alternative or each module gets its own typed struct field |
| A3 | Suspend->Running resume transition desired | Pattern 1 | Remove edge; tiny change |
| A4 | Crate MSRVs <= 1.95 | Stack | Raise workspace rust-version |
| A5 | f64 ulp arithmetic at 6.4e6 (2^-30 ~ 9.3e-10) | Summary | Loosen test tolerance |

## Open Questions (all RESOLVED)

1. **Where does WorldPos live given engine-world (tier 2) exists?** RESOLVED: engine-math (tier 0). engine-world later builds ECS/world on top and re-exports; avoids tier violation since core/jobs/spatial/render all can see it.
2. **Does engine-diagnostics need engine-core?** RESOLVED: No (tier 0). `FrameStats` driven by explicit `begin_frame/end_frame` calls from core.
3. **Do xtask layer table/docs change?** RESOLVED: No; no workspace edges change (existing declared edges suffice). Planner should still run `cargo xtask check-deps`.
4. **Per-module config representation?** RESOLVED: `BTreeMap<String, ron::Value>` in EngineConfig, typed on demand (fallback per A2).
5. **Rebase of "registered" positions?** RESOLVED: registry-owned position store with ids; also free-function slice rebase.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo/rustc | all | yes | 1.99.0 (2026-08-27) | — |
| crates.io access | new deps | yes (cargo search worked) | — | — |
| cargo-deny | license/ban check | not probed | — | CI job exists; run if installed |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | built-in `cargo test` (libtest), unit tests in-crate plus `tests/` integration per crate |
| Config file | none needed |
| Quick run command | `cargo test -p engine-math -p engine-diagnostics -p engine-jobs -p engine-core` |
| Full suite command | `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo xtask check-deps` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CORE-01 | all 25 transition pairs: legal ok, illegal Err | unit | `cargo test -p engine-core lifecycle` | Wave 0 |
| CORE-02 | RON config parse, bad RON errors, module dep order/cycle/missing | unit | `cargo test -p engine-core config registry` | Wave 0 |
| CORE-04 | priority order (1 worker), FIFO in class, cancel queued dropped, cancel running observed, shutdown joins | unit/integration | `cargo test -p engine-jobs` | Wave 0 |
| CORE-05 | span timing captured via scoped subscriber, FrameStats frame time > 0, duplicate init errors | unit | `cargo test -p engine-diagnostics` | Wave 0 |
| WORLD-02 | frame conversion round trip, cross-frame delta, no implicit mixing (compile-fail not required) | unit | `cargo test -p engine-math frame` | Wave 0 |
| WORLD-04 | 6.4e6 m rebase round trip preserves relative offsets within 1e-8 m, repeated rebases bounded drift | unit | `cargo test -p engine-math rebase` | Wave 0 |

### Sampling Rate
- Per task commit: quick run command for touched crate
- Per wave merge: full suite command
- Phase gate: full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] Add workspace deps to root Cargo.toml and per-crate manifests
- [ ] Test modules in each of the four crates (none exist; stubs only)
- [ ] Optional: a lifecycle integration test in engine-core driving a fake module through a full run with jobs and diagnostics

## Security Domain

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2/V3/V4 | no | N/A (library phase) |
| V5 Input Validation | yes | RON config parsed via serde into typed structs with `deny_unknown_fields` where feasible; errors not panics |
| V6 Cryptography | no | — |

### Known Threat Patterns
| Pattern | STRIDE | Mitigation |
|---------|--------|------------|
| Malformed/huge config file | DoS/Tampering | typed deserialize, size cap on read, return Err |
| Thread leak / deadlock at shutdown | DoS | explicit join, no detached threads |
| Untrusted job panics | DoS | `catch_unwind` around jobs so a worker survives; mark handle Failed |
| NaN/inf in WorldPos | Tampering | validate finite in constructor (return Result) |

## Sources

### Primary (HIGH)
- `cargo search` for glam, tracing, tracing-subscriber, crossbeam-channel, serde, ron, thiserror (2026-11-25)
- Repo: Cargo.toml, crates/*/Cargo.toml, xtask/src/layers.rs, docs/architecture/dependency-direction.md, deny.toml

### Secondary / Tertiary
- Training knowledge of tracing `Layer`, crossbeam, ron APIs (not re-verified this session; see A2) — LOW/MEDIUM

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH (versions verified)
- Architecture/tier fit: HIGH (read tier table and manifests)
- API details (ron Value, tracing Layer): MEDIUM (training knowledge)
- Pitfalls: MEDIUM

**Research date:** 2026-11-25
**Valid until:** 2026-12-25
