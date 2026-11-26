//! Generational entity identity.

slotmap::new_key_type! {
    /// Stable generational handle to an entity in a [`World`](crate::World).
    ///
    /// A despawned id stays invalid even if its storage slot is reused. The
    /// default (null) id is never valid.
    pub struct EntityId;
}
