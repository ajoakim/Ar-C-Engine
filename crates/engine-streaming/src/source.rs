//! Data source and uploader contracts.

use engine_jobs::CancelToken;

/// Produces data for keys on worker threads. Must be pure with respect to the key for determinism.
pub trait StreamSource<K>: Send + Sync + 'static {
    /// CPU bytes reserved against the class budget when the request starts.
    fn estimate_bytes(&self, key: &K) -> u64;
    /// Generate data; check `cancel` periodically; `None` means failure.
    fn produce(&self, key: &K, cancel: &CancelToken) -> Option<Vec<u8>>;
}

/// Frame-thread consumer of ready data (e.g. GPU upload).
/// Returning false keeps the item CPU-ready for retry next frame.
pub trait Uploader<K> {
    /// Consume `data` for `key`; return whether it was accepted.
    fn upload(&mut self, key: &K, data: &[u8]) -> bool;
}
