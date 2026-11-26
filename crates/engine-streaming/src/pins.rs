//! Refcounted pin table: a pinned resident ancestor cannot be evicted.

use std::collections::BTreeMap;

use engine_assets::AssetId;

#[derive(Default)]
pub(crate) struct Pins {
    counts: BTreeMap<AssetId, u32>,
}

impl Pins {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn pin(&mut self, id: AssetId) {
        *self.counts.entry(id).or_insert(0) += 1;
    }

    /// Decrement; returns false when `id` was not pinned.
    pub(crate) fn unpin(&mut self, id: AssetId) -> bool {
        match self.counts.get_mut(&id) {
            None => false,
            Some(c) => {
                *c -= 1;
                if *c == 0 {
                    self.counts.remove(&id);
                }
                true
            }
        }
    }

    pub(crate) fn count(&self, id: AssetId) -> u32 {
        self.counts.get(&id).copied().unwrap_or(0)
    }

    pub(crate) fn is_pinned(&self, id: AssetId) -> bool {
        self.counts.contains_key(&id)
    }

    #[allow(dead_code)]
    pub(crate) fn total(&self) -> u64 {
        self.counts.values().map(|c| u64::from(*c)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_refcount() {
        let id = AssetId::from_key("pins/a").unwrap();
        let other = AssetId::from_key("pins/b").unwrap();
        let mut p = Pins::new();
        p.pin(id);
        p.pin(id);
        assert!(p.unpin(id));
        assert_eq!(p.count(id), 1);
        assert!(p.is_pinned(id));
        assert_eq!(p.total(), 1);
        assert!(p.unpin(id));
        assert_eq!(p.count(id), 0);
        assert!(!p.is_pinned(id));
        assert!(!p.unpin(other));
    }
}
