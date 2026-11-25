//! Origin rebasing (implemented in Task 2).

/// Handle to a position registered in a `PositionStore`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PosId(u32);

/// Placeholder until Task 2.
#[derive(Default)]
pub struct PositionStore;

/// Placeholder until Task 2.
pub fn rebase(
    _frames: &mut crate::frame::FrameRegistry,
    _store: &mut PositionStore,
    _frame: crate::frame::FrameId,
    _new_origin: glam::DVec3,
) -> Result<(), crate::error::MathError> {
    Ok(())
}
