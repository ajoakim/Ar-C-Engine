//! Streaming request shape.

use engine_assets::AssetClass;
use engine_jobs::{CancelToken, Priority};

/// A request to stream one key: priority, budget class and cancellation token.
#[derive(Clone, Debug)]
pub struct StreamRequest<K> {
    /// Requested key.
    pub key: K,
    /// Scheduling priority (higher starts first).
    pub priority: Priority,
    /// Residency budget class charged for this key.
    pub budget_class: AssetClass,
    /// Cancellation token; cancelling it frees the request's budget on the next update.
    pub cancel: CancelToken,
}

impl<K> StreamRequest<K> {
    /// New request with a fresh, uncancelled token.
    #[must_use]
    pub fn new(key: K, priority: Priority, budget_class: AssetClass) -> Self {
        Self {
            key,
            priority,
            budget_class,
            cancel: CancelToken::new(),
        }
    }

    /// Replace the cancellation token (clones share the flag).
    #[must_use]
    pub fn with_cancel(mut self, cancel: CancelToken) -> Self {
        self.cancel = cancel;
        self
    }
}
