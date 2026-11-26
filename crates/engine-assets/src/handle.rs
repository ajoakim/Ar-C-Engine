//! Typed handles.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use slotmap::DefaultKey;

/// Typed handle (slot index + generation) into an `AssetRegistry<T>`.
///
/// Trait impls are written by hand so `T` needs no bounds.
pub struct Handle<T> {
    key: DefaultKey,
    _t: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    pub(crate) fn from_key(key: DefaultKey) -> Self {
        Self {
            key,
            _t: PhantomData,
        }
    }

    pub(crate) fn key(&self) -> DefaultKey {
        self.key
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl<T> PartialOrd for Handle<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Handle<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle<{}>({:?})", std::any::type_name::<T>(), self.key)
    }
}
