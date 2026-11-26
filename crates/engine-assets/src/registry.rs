//! Registry mapping [`AssetId`] to typed [`Handle`]s.

use std::collections::HashMap;

use slotmap::{DefaultKey, SlotMap};

use crate::error::AssetError;
use crate::handle::Handle;
use crate::id::AssetId;

struct Entry<T> {
    id: AssetId,
    value: Option<T>,
}

/// Maps stable asset IDs to generational typed handles and owns CPU values.
pub struct AssetRegistry<T> {
    slots: SlotMap<DefaultKey, Entry<T>>,
    by_id: HashMap<AssetId, Handle<T>>,
}

impl<T> Default for AssetRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AssetRegistry<T> {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self { slots: SlotMap::new(), by_id: HashMap::new() }
    }

    /// Registers an ID (idempotent) and returns its handle.
    pub fn register(&mut self, id: AssetId) -> Handle<T> {
        if let Some(h) = self.by_id.get(&id) {
            return *h;
        }
        let key = self.slots.insert(Entry { id, value: None });
        let h = Handle::from_key(key);
        self.by_id.insert(id, h);
        h
    }

    /// Registers an ID and sets its value, replacing any previous value.
    pub fn insert(&mut self, id: AssetId, value: T) -> Handle<T> {
        let h = self.register(id);
        if let Some(e) = self.slots.get_mut(h.key()) {
            e.value = Some(value);
        }
        h
    }

    /// Sets the value for a handle, returning the previous value.
    ///
    /// # Errors
    /// [`AssetError::StaleHandle`] if the handle is stale.
    pub fn set(&mut self, h: Handle<T>, value: T) -> Result<Option<T>, AssetError> {
        let e = self.slots.get_mut(h.key()).ok_or(AssetError::StaleHandle)?;
        Ok(e.value.replace(value))
    }

    /// Returns the value for a handle.
    #[must_use]
    pub fn get(&self, h: Handle<T>) -> Option<&T> {
        self.slots.get(h.key()).and_then(|e| e.value.as_ref())
    }

    /// Returns the value mutably.
    pub fn get_mut(&mut self, h: Handle<T>) -> Option<&mut T> {
        self.slots.get_mut(h.key()).and_then(|e| e.value.as_mut())
    }

    /// Drops the CPU value but keeps the registration.
    pub fn take(&mut self, h: Handle<T>) -> Option<T> {
        self.slots.get_mut(h.key()).and_then(|e| e.value.take())
    }

    /// Returns the ID for a handle.
    #[must_use]
    pub fn id(&self, h: Handle<T>) -> Option<AssetId> {
        self.slots.get(h.key()).map(|e| e.id)
    }

    /// Returns the handle registered for an ID.
    #[must_use]
    pub fn handle(&self, id: AssetId) -> Option<Handle<T>> {
        self.by_id.get(&id).copied()
    }

    /// True if the handle is live.
    #[must_use]
    pub fn contains(&self, h: Handle<T>) -> bool {
        self.slots.contains_key(h.key())
    }

    /// Removes the registration; the handle becomes stale.
    ///
    /// # Errors
    /// [`AssetError::StaleHandle`] if the handle is already stale.
    pub fn unregister(&mut self, h: Handle<T>) -> Result<Option<T>, AssetError> {
        let e = self.slots.remove(h.key()).ok_or(AssetError::StaleHandle)?;
        self.by_id.remove(&e.id);
        Ok(e.value)
    }

    /// Number of registered assets.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// True if nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    struct Mesh;
    struct Texture;

    fn aid(k: &str) -> AssetId {
        AssetId::from_key(k).unwrap()
    }

    #[test]
    fn handle_register_is_idempotent() {
        let mut r = AssetRegistry::<u32>::new();
        let a = r.register(aid("a"));
        let b = r.register(aid("a"));
        assert_eq!(a, b);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn handle_insert_and_get() {
        let mut r = AssetRegistry::new();
        let h = r.insert(aid("a"), 7u32);
        assert_eq!(r.get(h), Some(&7));
        assert_eq!(r.id(h), Some(aid("a")));
        assert_eq!(r.handle(aid("a")), Some(h));
    }

    #[test]
    fn handle_stale_after_unregister() {
        let mut r = AssetRegistry::new();
        let h = r.insert(aid("a"), 7u32);
        assert_eq!(r.unregister(h), Ok(Some(7)));
        assert_eq!(r.get(h), None);
        assert_eq!(r.id(h), None);
        assert!(!r.contains(h));
        assert_eq!(r.unregister(h), Err(AssetError::StaleHandle));
        assert_eq!(r.set(h, 1), Err(AssetError::StaleHandle));
    }

    #[test]
    fn handle_reregister_gives_new_handle() {
        let mut r = AssetRegistry::<u32>::new();
        let h = r.register(aid("a"));
        r.unregister(h).unwrap();
        let h2 = r.register(aid("a"));
        assert_ne!(h, h2);
    }

    #[test]
    fn handle_types_are_distinct() {
        fn takes_mesh(_: Handle<Mesh>) {}
        let mut m = AssetRegistry::<Mesh>::new();
        let mut t = AssetRegistry::<Texture>::new();
        let hm = m.register(aid("m"));
        let _ht = t.register(aid("t"));
        takes_mesh(hm);
        let mut set = HashSet::new();
        set.insert(hm);
        assert!(set.contains(&hm));
    }

    #[test]
    fn handle_take_keeps_registration() {
        let mut r = AssetRegistry::new();
        let h = r.insert(aid("a"), 5u32);
        assert_eq!(r.take(h), Some(5));
        assert_eq!(r.get(h), None);
        assert!(r.contains(h));
    }
}
