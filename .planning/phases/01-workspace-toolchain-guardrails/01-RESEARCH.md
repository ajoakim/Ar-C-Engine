# Phase 1: Workspace, Toolchain & Guardrails - Research

**Researched:** 2026-11-25
**Domain:** Cargo workspace layout, toolchain pinning, dependency-direction enforcement (cargo-deny + cargo-metadata), GitHub Actions CI, ADR conventions
**Confidence:** MEDIUM-HIGH (local environment facts verified; cargo-deny semantics from official docs; CI details partly ASSUMED)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None. (CONTEXT.md has no locked decisions.)

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Guidance from research (.planning/research/SUMMARY.md, STACK.md, ARCHITECTURE.md): ~8-12 coarse crates, cargo-deny for dependency-direction/isolation, CI on Windows + Linux, decide separate `engine-render-backend` crate vs folded into `engine-render` here. Local rustc is 1.69; toolchain install via rustup is required to verify builds (flag to user before installing system toolchains).

### Deferred Ideas (OUT OF SCOPE)
None
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WS-01 | Build whole workspace on pinned Rust toolchain (>=1.95, edition 2024) via rust-toolchain.toml on Windows and Linux | Toolchain section; local 1.99.0 already installed; CI matrix |
| WS-02 | Coarse crates with documented dependency direction | Crate layout + dependency table; `docs/architecture/dependency-direction.md` |
| WS-03 | CI fails if core depends on extension, or wgpu/winit outside designated crates | Two-layer enforcement: xtask `check-deps` (cargo_metadata) + cargo-deny bans/wrappers; negative fixture test |
| WS-04 | ADR template + conventions ADR (meters, axes, handedness, depth range) | ADR section with recommended conventions |
</phase_requirements>

## Summary

This phase is greenfield scaffolding: no application code, only manifests, stub `lib.rs` files, config, CI, and docs. The central risk is not technology but proving the guardrails actually bite. Success criterion 3 requires a demonstrated failing negative test, so the enforcement tool must be runnable against a deliberately-violating fixture and be expected to fail.

Local environment finding: the default `stable` toolchain is 1.69, but **Rust 1.99.0 (2026-09-28) is already installed via rustup** with cargo, clippy, rustfmt. A `rust-toolchain.toml` pinning `channel = "1.99.0"` therefore needs no download locally; rustup proxies will select it in the project directory. No system install, so no user confirmation is needed for local verification. `cargo-deny` is NOT installed locally (needs `cargo install cargo-deny --locked` or a prebuilt binary; flag to user, it is a user-level cargo bin install, not a system toolchain).

Enforcement design: use two complementary layers. (1) A small in-repo Rust `xtask` using `cargo_metadata` that checks the workspace graph against an explicit layer table (core never depends on extension; wgpu/winit only in allowed crates), with unit tests against fixture manifests (this is the negative test, fast and hermetic, runs without network). (2) `cargo-deny` `[bans]` with `wrappers` for wgpu/winit plus licenses/advisories, run in CI as the standard ecosystem guard. Rationale: cargo-deny is excellent for third-party crate isolation but its handling of workspace-internal direction is less direct and unverified locally; the xtask is deterministic and testable.

**Primary recommendation:** Pin `channel = "1.99.0"` (MSRV `rust-version = "1.95"`), create 12 core + 2 extension stub crates with real dependency edges, split render into backend-agnostic `engine-render` and a wgpu-only `engine-render-wgpu`, enforce direction with `cargo xtask check-deps` (tested against fixtures) plus `cargo-deny`, and run fmt/clippy/test/check-deps/deny on an ubuntu+windows GitHub Actions matrix.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Toolchain pinning | Repo root config (rust-toolchain.toml) | CI | One source of truth for dev and CI |
| Dependency-direction enforcement | Dev tooling (xtask) | CI + cargo-deny | Must run locally and in CI identically |
| Third-party isolation (wgpu/winit) | cargo-deny bans | xtask allowlist | Defense in depth |
| Conventions (units/axes/depth) | docs/ADR | engine-math (Phase 2) doc comments | Decided now, encoded in types later |
| Crate boundaries | Cargo manifests | Layer table doc | Manifest edges are the truth; doc describes intent |

## Standard Stack

### Core
| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| Rust toolchain | 1.99.0 pinned (MSRV 1.95) | Language | Installed locally [VERIFIED: `rustup toolchain list`, `rustc 1.99.0 (b940084d7 2026-09-28)`]; STACK.md requires >=1.95 |
| Cargo workspace, edition 2024, resolver "3" | - | Crate boundaries | Edition 2024 needs >=1.85, resolver 3 >=1.84 [ASSUMED from training; trivially satisfied by 1.99] |
| cargo-deny | latest (`cargo install cargo-deny --locked`; CI via `taiki-e/install-action`) | bans/licenses/advisories | Official docs describe `bans.deny` with `wrappers` [CITED: embarkstudios.github.io/cargo-deny/checks/bans/cfg.html] |
| cargo_metadata (crate) | verify with `cargo add` / `cargo info` at implementation | xtask graph inspection | Standard way to read the workspace graph programmatically [ASSUMED] |

### Supporting
| Tool | Purpose | When |
|------|---------|------|
| cargo-nextest | faster tests | Optional; STACK.md lists it. Not needed in Phase 1; plain `cargo test` is enough |
| Swatinem/rust-cache | CI caching | CI speed [ASSUMED] |
| taiki-e/install-action | install cargo-deny on both OSes | Works on Windows and Linux runners [ASSUMED, verify when writing CI] |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| xtask + cargo-deny | cargo-deny only | cargo-deny action docs only show ubuntu examples and appear to be a Linux container action [CITED: github.com/EmbarkStudios/cargo-deny-action, Windows support not stated]; install the binary via install-action instead of using the action on Windows. Using only cargo-deny leaves workspace-internal direction weakly tested |
| `channel = "1.99.0"` | `channel = "stable"` | Floating stable is not reproducible; pinning satisfies "pinned toolchain" |

**Installation (local, user-level only):**
```bash
cargo install cargo-deny --locked   # not currently installed [VERIFIED: command not found]
```

## Architecture Patterns

### Crate layout decision: separate render backend crate

**Recommendation: split now.** Create `engine-render` (backend-agnostic: `RenderScene`, extraction contract, render graph skeleton, `RenderBackend` trait, engine-owned handle types; NO wgpu) and `engine-render-wgpu` (the only crate that depends on wgpu). Reasoning:
- WS-03 requires "wgpu/winit only in designated crates". With a fold, wgpu would sit in a crate that extensions (`planetary-render`, `conventional-3d`) must depend on for extraction types, so every consumer transitively compiles wgpu and the "direct dependents only" wrappers check cannot distinguish them. A dedicated crate makes the allowlist exactly one crate per ban.
- Headless/golden render-data tests (extraction output) run without a GPU stack and compile fast.
- Matches Anti-Pattern 7 in ARCHITECTURE.md (no wgpu in public render API).
- Cost: one extra tiny crate. This deviates from ARCHITECTURE.md's "fold until a second backend", which is a conscious override justified by enforceability. Naming: `engine-render-wgpu` is self-documenting (the ban list names it); `engine-render-backend` is acceptable if the roadmap prefers it. Pick one and keep it consistent in deny.toml and the layer table.
- winit only in `engine-platform`. The wgpu crate accepts `raw-window-handle` (`HasWindowHandle`/`HasDisplayHandle`) so it never needs winit. [ASSUMED from wgpu design; confirm in Phase 3]

### Recommended structure
```
Cargo.toml                 # [workspace] resolver="3", members, [workspace.package], [workspace.lints], [workspace.dependencies]
rust-toolchain.toml        # channel = "1.99.0", components = [rustfmt, clippy]
.cargo/config.toml         # [alias] xtask = "run --package xtask --"
deny.toml
rustfmt.toml  clippy.toml(optional)
crates/
  engine-math engine-diagnostics engine-core engine-jobs engine-platform
  engine-assets engine-world engine-spatial engine-render engine-render-wgpu engine-sdk
extensions/
  conventional-3d
  planetary/planetary-core
xtask/                     # check-deps command + tests (also a workspace member)
xtask/fixtures/            # violating mini-workspaces (NOT workspace members; excluded)
docs/architecture/dependency-direction.md
docs/architecture/adr/0000-template.md  0001-conventions.md  README.md
.github/workflows/ci.yml
```
Deferred to their own phases (no consumer yet, avoid fake boundaries): engine-streaming, engine-procedural, planetary-terrain/generation/render, examples/. Roadmap phases 2+ add them. Include `exclude = ["xtask/fixtures"]` in workspace so fixtures do not join the workspace. Keep stub crates minimal (`//! doc comment` + `#![forbid(unsafe_code)]` where sensible) but give them REAL path dependencies per the table so the graph check is meaningful.

### Layering (tiers) and allowed edges
Tier numbers, lower may not depend on higher:

| Tier | Crates | May depend on |
|------|--------|---------------|
| 0 foundation | engine-math, engine-diagnostics | external only (glam, tracing) |
| 1 | engine-core, engine-jobs | tier 0 |
| 2 services | engine-platform, engine-assets, engine-world, engine-spatial, engine-render | tier 0-1 (+ engine-assets/world/spatial by need; render may use math/core/assets/world types via extraction) |
| 2b backend | engine-render-wgpu | engine-render + tier 0-1; only crate allowed wgpu |
| 3 facade | engine-sdk | any `engine-*` |
| ext | conventional-3d, planetary-* | any `engine-*` (not engine-render-wgpu/engine-platform's third-party types); planetary-* never depended on by conventional-3d and vice versa |

Hard rules (checked): (R1) no `engine-*` package depends (including dev/build deps) on anything under `extensions/`; (R2) wgpu only as dependency of `engine-render-wgpu`; (R3) winit only of `engine-platform`; (R4) `conventional-3d` and `planetary-*` do not depend on each other; (R5) no cycles / tier rule. The layer table lives in a single data file (e.g. `xtask/src/layers.rs` or `layers.toml`) and is mirrored in `docs/architecture/dependency-direction.md`. Identify extension crates by manifest path prefix `extensions/` (robust vs naming) and core by `crates/`.

### Enforcement pattern
```rust
// xtask: check-deps (sketch)
// Source: cargo_metadata API (docs.rs/cargo_metadata) [ASSUMED API shape; verify at implementation]
let md = cargo_metadata::MetadataCommand::new().manifest_path(path).exec()?;
for pkg in md.workspace_packages() {
    let is_core = is_under(&pkg.manifest_path, "crates");
    for dep in &pkg.dependencies {           // includes dev/build kinds
        if is_core && ws_ext_names.contains(&dep.name) { violation(...) }
        if dep.name == "wgpu" && !WGPU_ALLOWED.contains(&pkg.name.as_str()) { violation(...) }
        if dep.name == "winit" && !WINIT_ALLOWED.contains(&pkg.name.as_str()) { violation(...) }
    }
}
```
Check `pkg.dependencies` (declared, direct) so violations are caught even before wgpu is in the lockfile, and also resolved transitive graph (`md.resolve`) for the third-party bans (wgpu reachable via a non-allowed crate). Make the checker a library function taking a manifest path; `main` calls it on the root; tests call it on fixtures.

### Negative test (success criterion 3)
- `xtask/fixtures/core-depends-on-ext/` : mini workspace with `crates/engine-x` depending on `extensions/ext-y` via path. `xtask/fixtures/wgpu-in-wrong-crate/`: `crates/engine-core` declaring `wgpu = "*"` (dependency declared; fixture resolution may need network, so rely on declared deps via `cargo metadata --no-deps`, which does not resolve/fetch) [ASSUMED: `--no-deps` works offline; verify].
- `#[test]` in xtask asserts `check_deps(fixture)` returns Err with the expected rule id, and the real workspace returns Ok. Also add a CI step running the real check. This runs in the normal `cargo test`, so the failing demonstration is itself a green test.
- Optionally a CI job proving cargo-deny fails against a fixture `deny.toml` + fixture workspace using `! cargo deny --manifest-path fixtures/... check bans`.

### cargo-deny configuration (recommended deny.toml)
```toml
[graph]
all-features = true

[bans]
multiple-versions = "warn"      # wgpu/winit stacks duplicate often; do not deny in P0
wildcards = "deny"              # NOTE: path deps in workspace need allow-wildcard-paths = true
allow-wildcard-paths = true
deny = [
  { crate = "wgpu",  wrappers = ["engine-render-wgpu"], reason = "GPU API confined to the backend crate (WS-03)" },
  { crate = "winit", wrappers = ["engine-platform"],    reason = "Windowing confined to platform crate (WS-03)" },
]

[licenses]
allow = ["MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-3-Clause", "BSD-2-Clause", "ISC", "Unicode-3.0", "Zlib"]
# confidence = default; add exceptions as the real dep tree appears in later phases

[advisories]
# defaults; requires network to fetch advisory db in CI
```
Semantics per official docs: `wrappers` lets listed crates depend directly on the banned crate while denying all other (transitive) dependencies on it [CITED: embarkstudios.github.io/cargo-deny/checks/bans/cfg.html]. Caveat: wgpu's own ecosystem may legitimately pull `winit`-adjacent crates (e.g. egui-winit later) which would need adding to wrappers; keep a reasons comment. In Phase 1 neither crate is in the graph, so these rules are dormant; do not rely on cargo-deny alone for the demonstrable negative test. cargo-deny `[licenses]` keys and `wildcards`/`allow-wildcard-paths` option names are from training and must be checked against `cargo deny init` output of the installed version [ASSUMED]. Do NOT use an `allow` allowlist in bans (denies everything unlisted).

Cargo-deny cannot easily be used as the sole workspace-direction guard: deny entries can name workspace crates, but behavior for workspace members (versus `allow-workspace`) is not verified here [LOW]; the xtask is the authoritative check.

### CI (GitHub Actions)
```yaml
name: CI
on: { push: { branches: [main, master] }, pull_request: {} }
env: { CARGO_TERM_COLOR: always, RUSTFLAGS: "-D warnings" }
jobs:
  build:
    strategy: { fail-fast: false, matrix: { os: [ubuntu-latest, windows-latest] } }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - name: Install pinned toolchain from rust-toolchain.toml
        run: rustup toolchain install   # newer rustup no longer auto-installs from the file [ASSUMED; verify], so be explicit
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
      - run: cargo xtask check-deps
  deny:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: taiki-e/install-action@cargo-deny
      - run: cargo deny check bans licenses advisories
```
Run cargo-deny on ubuntu only (graph is platform-independent mostly; use `[graph] targets` for both triples if needed). Check the repo's default branch: local branch is `master`, main is `main`; include both triggers. Pin action major versions; confirm current majors when writing the file.

### Workspace Cargo.toml essentials
```toml
[workspace]
resolver = "3"
members = ["crates/*", "extensions/*", "extensions/planetary/*", "xtask"]
exclude = ["xtask/fixtures"]
# NOTE: extensions/planetary is a directory of crates, not a crate: the glob "extensions/*" would
# try to treat it as a member and fail. List explicitly: "extensions/conventional-3d", "extensions/planetary/*".

[workspace.package]
edition = "2024"
rust-version = "1.95"
license = "MIT OR Apache-2.0"   # decision for user/ADR; confirm
publish = false

[workspace.lints.rust]
unsafe_code = "forbid"   # relax per-crate (platform/render-wgpu) via ADR if needed
[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
```
Each crate: `[lints] workspace = true`, `edition.workspace = true`, `rust-version.workspace = true`.

### Anti-Patterns to Avoid
- Empty crates with no declared edges (fake boundaries): give stubs real path deps.
- `channel = "stable"` (non-reproducible).
- Putting fixtures inside the workspace members (they would fail the real check). Exclude them.
- Relying on grep over Cargo.toml: use cargo_metadata.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Third-party crate bans, licenses, advisories | custom scripts | cargo-deny | Maintained, handles transitive graph |
| Parsing Cargo graph | regex over TOML | cargo_metadata | Handles renames, targets, dev/build deps |
| Toolchain pinning | version-check scripts | rust-toolchain.toml | rustup handles it |
| ADR tooling | custom generator | plain markdown template (MADR-style) | Zero deps; README index |

**Key insight:** the only custom code is the ~100-line layer-rule checker, because workspace-internal layering is project-specific.

## Runtime State Inventory
Omitted: greenfield, not a rename/refactor/migration phase.

## Common Pitfalls

### Pitfall 1: Glob member `extensions/*` swallowing the `planetary` directory
**What goes wrong:** cargo errors because `extensions/planetary` has no Cargo.toml. **Avoid:** list members explicitly. **Sign:** `failed to read .../extensions/planetary/Cargo.toml`.

### Pitfall 2: Negative fixtures joining the real workspace
**What goes wrong:** fixture manifests either break `cargo build` ("believes it's in a workspace") or trip the real check. **Avoid:** `exclude` in root, and give each fixture its own `[workspace]` table.

### Pitfall 3: Toolchain file not auto-installed on CI
**What goes wrong:** CI uses preinstalled older stable, ignoring pin, or errors on missing toolchain. **Avoid:** explicit `rustup toolchain install` step then `rustc --version` assertion. [ASSUMED rustup behavior]

### Pitfall 4: Local `cargo` still 1.69 outside the repo
**What goes wrong:** running cargo from another cwd or via a tool with wrong cwd uses 1.69, failing on edition 2024. **Avoid:** always run in repo root; verification commands can use `cargo +1.99.0`. The pin file handles in-repo usage. Windows note: this Git Bash shell resolves cargo via rustup proxies [VERIFIED: `cargo --version` gave 1.69 outside; `rustup run 1.99.0 cargo` gives 1.99.0].

### Pitfall 5: cargo-deny wrappers only dormant until deps exist
Rules for wgpu/winit pass vacuously now; the xtask fixture test is the proof they work. Re-verify when wgpu is added in Phase 3.

### Pitfall 6: Windows line endings / path separators in the checker
Compare manifest paths using `Path::components` or normalized forward slashes, not string prefixes with `/`, otherwise Windows CI diverges from Linux. Also set `.gitattributes` (`* text=auto eol=lf`) to keep fmt check stable across OSes.

### Pitfall 7: dev-dependencies sneaking violations
Core crates must not even dev-depend on extensions; check all dependency kinds (cargo_metadata `dep.kind`).

## Code Examples

### rust-toolchain.toml
```toml
[toolchain]
channel = "1.99.0"
components = ["rustfmt", "clippy"]
profile = "minimal"
```
Source: rustup docs [ASSUMED format; widely used].

### ADR template (docs/architecture/adr/0000-template.md, MADR-style)
```markdown
# ADR-NNNN: Title
- Status: proposed | accepted | superseded by ADR-XXXX
- Date: YYYY-MM-DD
- Requirement/Phase: e.g. WS-04
## Context
## Decision
## Alternatives considered
## Consequences (including limitations and assumptions)
## Evidence (benchmark / prototype link; required for WS-05 decisions)
```

### ADR-0001 Conventions (recommended content)
These are recommendations for the planner to write as an accepted ADR; they are design choices, not verified facts, and the user can adjust:
- **Units:** SI, meters for length, seconds for time, radians for angles internally (degrees only at API/config edges, named `_deg`). Planet radius in HLD is `radius_meters: 6371000` [VERIFIED: HLD.md], consistent with meters.
- **Handedness and axes:** right-handed, Y-up, -Z forward (glam `Mat4::look_to_rh` / `perspective_rh` convention, matches wgpu/glTF). glTF is also right-handed Y-up +Z-forward-of-asset; document import. Planetary extension may use internal Z-up (ECEF-like) frames but must convert through engine `Frame` transforms; engine world/render never see ECEF axes.
- **Depth:** reversed-Z, `Depth32Float`, clear depth 0.0, compare `Greater` (or GreaterEqual), infinite far plane; wgpu/WebGPU NDC depth range is [0, 1] [CITED in STACK.md: reversed-Z with Depth32Float; NDC range ASSUMED from WebGPU spec, verify in Phase 3].
- **Precision:** world positions f64; render transforms camera-relative f32 (STACK.md/ARCHITECTURE.md Pattern 5).
- **Matrix/layout:** column-major (glam); angles/winding: counter-clockwise front faces [ASSUMED convention; state explicitly].
- **Texture/UV origin:** top-left (WebGPU) [ASSUMED]; state it.
- **IDs/time:** document fixed timestep vs variable later; out of scope now.

## State of the Art

| Old | Current | Impact |
|-----|---------|--------|
| edition 2021 / resolver 2 | edition 2024 / resolver 3 | requested by roadmap; `workspace.package` + `workspace.lints` inheritance available |
| per-crate lint attributes | `[workspace.lints]` | single source for lints |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Newer rustup does not auto-install toolchain from rust-toolchain.toml; explicit `rustup toolchain install` is needed in CI | CI | CI step redundant but harmless |
| A2 | `cargo metadata --no-deps` works offline for fixtures with unresolvable registry deps | Negative test | Fixture needs path-only stand-in crates named `wgpu`/`winit` (fallback: local stub crates named wgpu via path) |
| A3 | cargo-deny option names (`allow-wildcard-paths`, license ids, `[graph]`) | deny.toml | Run `cargo deny init` and diff; deny will print config errors |
| A4 | cargo-deny behavior for denying workspace-internal crates | Enforcement | Mitigated by xtask being authoritative |
| A5 | taiki-e/install-action supports cargo-deny on both OSes | CI | Fall back to `cargo install cargo-deny --locked` |
| A6 | Conventions (Y-up, RH, -Z forward, reversed-Z, CCW, UV top-left) | ADR-0001 | User may prefer Z-up; confirm before ADR acceptance. Cheap now, costly later (CONFIRMED by orchestrator default) |
| A7 | License choice MIT OR Apache-2.0 | Cargo.toml | Project is open-source per PROJECT.md but license unspecified; ask user (CONFIRMED by orchestrator default) |
| A8 | Rust 1.99.0 is a suitable pin (1.99 is newer than the 1.95 floor; newer stable may exist by today's date) | Toolchain | None functionally; pinned version is installed locally already |

## Open Questions (RESOLVED)

1. **License** (A7): not specified anywhere; needed for `cargo-deny` license config and Cargo.toml. Recommend MIT OR Apache-2.0, confirm. RESOLVED: MIT OR Apache-2.0 (orchestrator default, ADR-0002; reversible).
2. **Y-up vs Z-up world convention** (A6): recommend Y-up RH; confirm in ADR review. RESOLVED: Y-up right-handed, -Z forward (ADR-0001).
3. **Render crate name:** `engine-render-wgpu` vs `engine-render-backend`; pick one in plan. RESOLVED: `engine-render-wgpu` (ADR-0002).
4. **Remote/CI host:** Is the repo hosted on GitHub? (Assumed; `.github/workflows` written regardless, cannot be exercised locally.) CI cannot be run in this environment; local equivalent is the same commands. RESOLVED: GitHub Actions (ADR-0003); hosted run is a manual-verification item.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rustup | toolchain | yes | present | - |
| Rust 1.99.0 (+clippy, rustfmt) | WS-01 | yes (installed) | 1.99.0 (2026-09-28) | `rustup toolchain install 1.99.0` (user-level, flag to user) |
| default `stable` | - | yes but 1.69 | 1.69.0 | irrelevant inside repo once pinned |
| cargo-deny | WS-03 | NO | - | `cargo install cargo-deny --locked` (user-level; ask) ; xtask covers rules meanwhile |
| network / crates.io | cargo_metadata crate, deny advisories | assumed yes | - | - |
| GitHub Actions | CI | cannot verify locally | - | run same commands locally |
| git | repo | yes | - | - |

**Missing, blocking:** none. **Missing with fallback:** cargo-deny.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Built-in `cargo test` (libtest); optional nextest later |
| Config file | none needed; `rust-toolchain.toml`, `deny.toml` |
| Quick run command | `cargo test -p xtask` |
| Full suite command | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo xtask check-deps && cargo deny check bans licenses` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| WS-01 | Pinned toolchain active; workspace builds | smoke | `cargo --version` matches pin in `rust-toolchain.toml`; `cargo build --workspace` (CI on ubuntu+windows) | Wave 0 |
| WS-01 | rust-version >= 1.95, edition 2024 on all members | unit (xtask) | `cargo test -p xtask workspace_metadata_policy` | Wave 0 |
| WS-02 | Real workspace passes layer rules; all members use workspace inheritance | unit/integration | `cargo test -p xtask real_workspace_ok` | Wave 0 |
| WS-02 | dependency-direction doc lists every crate | unit | `cargo test -p xtask docs_cover_all_crates` | Wave 0 |
| WS-03 | core->extension violation detected | negative unit | `cargo test -p xtask fixture_core_depends_on_extension_fails` | Wave 0 |
| WS-03 | wgpu/winit outside allowed crates detected | negative unit | `cargo test -p xtask fixture_wgpu_winit_misplaced_fails` | Wave 0 |
| WS-03 | cargo-deny passes on real workspace | CI | `cargo deny check bans licenses` | Wave 0 |
| WS-04 | ADR template and ADR-0001 exist with required sections (meters, axes, handedness, depth) | unit (file content grep) | `cargo test -p xtask adr_conventions_present` | Wave 0 |

### Sampling Rate
- Per task commit: `cargo test -p xtask` and `cargo check --workspace`
- Per wave merge: full suite command
- Phase gate: full suite green locally and CI green on both OSes before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `xtask/` crate with `check_deps` library fn and tests
- [ ] `xtask/fixtures/*` violating mini-workspaces
- [ ] `.cargo/config.toml` alias for `cargo xtask`
- [ ] cargo-deny install (user-level)

## Security Domain

Applicable ASVS categories: effectively none (no runtime code). Relevant supply-chain hygiene only.

| ASVS Category | Applies | Control |
|---------------|---------|---------|
| V2/V3/V4 | no | - |
| V5 Input Validation | no | - |
| V6 Cryptography | no | - |
| V14 Config / supply chain | yes | cargo-deny advisories + licenses; commit Cargo.lock; pin action versions |

| Threat | STRIDE | Mitigation |
|--------|--------|------------|
| Vulnerable/malicious dependency | Tampering | cargo-deny advisories, `Cargo.lock` committed, `--locked` in CI |
| Compromised CI action | Tampering | pin actions to major tag (SHA pin optional), least-privilege `permissions: contents: read` |

Add `--locked` to CI cargo commands (after Cargo.lock is committed). For a workspace of libraries and apps, commit Cargo.lock.

## Sources

### Primary (HIGH)
- Local probes: `rustc --version`, `rustup toolchain list`, `rustup run 1.99.0 cargo --version`, `cargo deny --version` (not installed), component list (clippy, rustfmt present).
- cargo-deny bans config docs: https://embarkstudios.github.io/cargo-deny/checks/bans/cfg.html (wrappers, skip, allow, multiple-versions)
- Project files: HLD.md (radius_meters), .planning/research/STACK.md, ARCHITECTURE.md, REQUIREMENTS.md

### Secondary (MEDIUM)
- https://github.com/EmbarkStudios/cargo-deny-action (ubuntu-only examples; Windows support not stated)

### Tertiary (LOW)
- Training-knowledge claims tagged [ASSUMED] above (rustup auto-install behavior, cargo_metadata API, edition/resolver MSRVs, conventions).

## Metadata
**Confidence breakdown:**
- Standard stack: HIGH for toolchain facts (verified locally); MEDIUM for cargo-deny details
- Architecture/crate layout: MEDIUM (reasoned; deviates from ARCHITECTURE.md on render split deliberately)
- Pitfalls: MEDIUM
- ADR conventions: recommendations needing user confirmation

**Research date:** 2026-11-25
**Valid until:** ~30 days
