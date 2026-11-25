//! Windowing and input. The only crate allowed to depend on winit.
//!
//! Tier 2 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! Wraps winit 0.30; no winit type appears in the public API.

mod error;
mod input;

pub use error::{HandlerError, PlatformError};
pub use input::{InputEvent, KeyCode, MouseButton};
