//! Residency manager: budgets, per-frame upload cap, LRU candidates.

/// Asset class identifier (budgets are per class).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetClass(pub u16);

/// Memory pool a budget applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemoryPool {
    /// System memory.
    Cpu,
    /// Device memory.
    Gpu,
}
