//! Windowing and input. The only crate allowed to depend on winit.
//!
//! Tier 2 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! winit itself is added in Phase 3; no other crate may depend on winit.
