# Architecture Decision Records

To add an ADR, copy `0000-template.md` to the next free number (4-digit zero-padded) with a kebab-case title, for example `0004-my-decision.md`, and fill in every section. Never rewrite the decision of an accepted ADR: supersede it with a new ADR and set the old one's status to `superseded by ADR-XXXX`, so decision history stays auditable in git.

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-conventions.md) | Engine Conventions | accepted |
| [0002](0002-workspace-crate-layout.md) | Workspace Crate Layout, Render Split and License | accepted |
| [0003](0003-dependency-enforcement.md) | Dependency-Direction Enforcement and CI | accepted |
| [0004](0004-worldpos-frames-rebasing.md) | WorldPos, Coordinate Frames and Origin Rebasing | accepted |
| [0005](0005-rendering-api-wgpu-behind-trait.md) | Rendering API - wgpu Behind an Engine-Owned Backend Trait | accepted |
| [0006](0006-spatial-index-hash-grid-first.md) | Spatial Index - Uniform Hash Grid First, Benchmark Later | accepted |
