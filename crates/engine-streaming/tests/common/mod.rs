#![allow(dead_code)]
//! Shared deterministic test helpers (spin-gated, no timing).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use engine_streaming::{
    AssetClass, AssetId, CancelToken, ClassBudget, ResidencyManager, StreamKey, StreamSource,
    Uploader,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TestKey {
    pub depth: u8,
    pub index: u64,
}

impl TestKey {
    pub fn new(depth: u8, index: u64) -> Self {
        Self { depth, index }
    }
}

impl StreamKey for TestKey {
    fn asset_id(&self) -> AssetId {
        AssetId::from_key(&format!("test/{}/{}", self.depth, self.index)).expect("asset id")
    }
    fn parent(&self) -> Option<Self> {
        if self.depth == 0 {
            None
        } else {
            Some(TestKey {
                depth: self.depth - 1,
                index: self.index / 4,
            })
        }
    }
}

pub struct GatedSource {
    pub released: Arc<AtomicU64>,
    pub release_at: BTreeMap<TestKey, u64>,
    pub fail: BTreeSet<TestKey>,
    pub bytes: u64,
}

impl GatedSource {
    /// Ungated source: every key is released from the start.
    pub fn new(bytes: u64) -> Self {
        Self {
            released: Arc::new(AtomicU64::new(0)),
            release_at: BTreeMap::new(),
            fail: BTreeSet::new(),
            bytes,
        }
    }

    /// Source whose listed keys block until `released` reaches `level`.
    pub fn gated_at(keys: &[TestKey], bytes: u64, level: u64) -> (Self, Arc<AtomicU64>) {
        let (mut s, r) = Self::gated(keys, bytes);
        for k in keys {
            s.release_at.insert(*k, level);
        }
        (s, r)
    }

    /// Source whose listed keys block until `released` reaches `u64::MAX`.
    pub fn gated(keys: &[TestKey], bytes: u64) -> (Self, Arc<AtomicU64>) {
        let mut s = Self::new(bytes);
        for k in keys {
            s.release_at.insert(*k, u64::MAX);
        }
        let r = Arc::clone(&s.released);
        (s, r)
    }
}

impl StreamSource<TestKey> for GatedSource {
    fn estimate_bytes(&self, _key: &TestKey) -> u64 {
        self.bytes
    }
    fn produce(&self, key: &TestKey, cancel: &CancelToken) -> Option<Vec<u8>> {
        let need = self.release_at.get(key).copied().unwrap_or(0);
        loop {
            if cancel.is_cancelled() {
                return None;
            }
            if self.released.load(Ordering::Acquire) >= need {
                break;
            }
            std::thread::yield_now();
        }
        if self.fail.contains(key) {
            return None;
        }
        Some(vec![key.depth; self.bytes as usize])
    }
}

#[derive(Default)]
pub struct RecordingUploader {
    pub uploaded: BTreeMap<TestKey, Vec<u8>>,
    pub refuse: bool,
}

impl Uploader<TestKey> for RecordingUploader {
    fn upload(&mut self, key: &TestKey, data: &[u8]) -> bool {
        if self.refuse {
            return false;
        }
        self.uploaded.insert(*key, data.to_vec());
        true
    }
}

pub const CLASS: AssetClass = AssetClass(1);

pub fn residency(cpu: u64, gpu: u64, upload_cap: u64) -> ResidencyManager {
    let mut m = ResidencyManager::new(upload_cap);
    m.set_class_budget(
        CLASS,
        ClassBudget {
            cpu_bytes: cpu,
            gpu_bytes: gpu,
        },
    );
    m
}

pub fn cpu_used(m: &ResidencyManager, class: AssetClass) -> u64 {
    m.stats()
        .classes
        .iter()
        .find(|c| c.class == class)
        .map_or(0, |c| c.cpu_used)
}
