//! Residency manager: budgets, per-frame upload cap, LRU candidates.
//!
//! Pure and deterministic: frame indices are passed in, there are no clocks
//! or threads. Every operation validates all preconditions before mutating
//! anything, so a failed call leaves state and byte counters untouched. The
//! only exception is the denial counters, which are observability data.

use std::collections::BTreeMap;

use crate::error::AssetError;
use crate::id::AssetId;
use crate::residency::ResidencyState;

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

/// CPU and GPU byte budgets of one asset class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassBudget {
    /// CPU byte budget.
    pub cpu_bytes: u64,
    /// GPU byte budget.
    pub gpu_bytes: u64,
}

struct ClassState {
    budget: ClassBudget,
    cpu_used: u64,
    gpu_used: u64,
}

struct Record {
    class: AssetClass,
    state: ResidencyState,
    cpu_bytes: u64,
    gpu_bytes: u64,
    last_used_frame: u64,
    request_gen: u64,
}

/// Per-class usage snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassStats {
    /// Class.
    pub class: AssetClass,
    /// CPU budget.
    pub cpu_budget: u64,
    /// GPU budget.
    pub gpu_budget: u64,
    /// CPU bytes in use.
    pub cpu_used: u64,
    /// GPU bytes in use.
    pub gpu_used: u64,
}

/// Observable manager statistics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResidencyStats {
    /// Current frame.
    pub frame: u64,
    /// Per-frame upload cap.
    pub upload_cap_bytes: u64,
    /// Bytes uploaded this frame.
    pub uploaded_this_frame: u64,
    /// Uploads denied by the frame cap.
    pub denied_uploads_cap: u64,
    /// Uploads denied by the GPU budget.
    pub denied_uploads_budget: u64,
    /// Per-class stats sorted by class.
    pub classes: Vec<ClassStats>,
    /// Asset counts indexed by `ResidencyState::index`.
    pub state_counts: [usize; 8],
}

/// Residency state machine plus budget and upload-cap accounting.
pub struct ResidencyManager {
    classes: BTreeMap<AssetClass, ClassState>,
    records: BTreeMap<AssetId, Record>,
    upload_cap_bytes: u64,
    uploaded_this_frame: u64,
    frame: u64,
    next_request_gen: u64,
    denied_uploads_cap: u64,
    denied_uploads_budget: u64,
}

impl ResidencyManager {
    /// Create a manager with a per-frame upload byte cap.
    pub fn new(upload_cap_bytes: u64) -> Self {
        Self {
            classes: BTreeMap::new(),
            records: BTreeMap::new(),
            upload_cap_bytes,
            uploaded_this_frame: 0,
            frame: 0,
            next_request_gen: 0,
            denied_uploads_cap: 0,
            denied_uploads_budget: 0,
        }
    }

    /// Create or replace a class budget. Lowering below current usage is
    /// allowed; only new reservations fail.
    pub fn set_class_budget(&mut self, class: AssetClass, budget: ClassBudget) {
        self.classes
            .entry(class)
            .and_modify(|c| c.budget = budget)
            .or_insert(ClassState {
                budget,
                cpu_used: 0,
                gpu_used: 0,
            });
    }

    /// Start tracking an asset in state `Unloaded`.
    pub fn track(&mut self, id: AssetId, class: AssetClass) -> Result<(), AssetError> {
        if !self.classes.contains_key(&class) {
            return Err(AssetError::UnknownClass(class));
        }
        if self.records.contains_key(&id) {
            return Err(AssetError::AlreadyTracked(id));
        }
        self.records.insert(
            id,
            Record {
                class,
                state: ResidencyState::Unloaded,
                cpu_bytes: 0,
                gpu_bytes: 0,
                last_used_frame: self.frame,
                request_gen: 0,
            },
        );
        Ok(())
    }

    /// Stop tracking an asset; only from `Unloaded` or `Failed`.
    pub fn untrack(&mut self, id: AssetId) -> Result<(), AssetError> {
        let state = self.rec(id)?.state;
        if !matches!(state, ResidencyState::Unloaded | ResidencyState::Failed) {
            return Err(AssetError::IllegalTransition {
                from: state,
                to: ResidencyState::Unloaded,
            });
        }
        self.records.remove(&id);
        Ok(())
    }

    /// Current state of an asset.
    pub fn state(&self, id: AssetId) -> Option<ResidencyState> {
        self.records.get(&id).map(|r| r.state)
    }

    /// Current request generation of an asset.
    pub fn request_generation(&self, id: AssetId) -> Option<u64> {
        self.records.get(&id).map(|r| r.request_gen)
    }

    /// Current frame index.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Start a frame: sets the frame index and resets the upload counter.
    pub fn begin_frame(&mut self, frame_index: u64) {
        self.frame = frame_index;
        self.uploaded_this_frame = 0;
    }

    /// `Unloaded | Failed -> Requested`; returns the new request generation.
    pub fn request(&mut self, id: AssetId) -> Result<u64, AssetError> {
        self.check(id, ResidencyState::Requested)?;
        self.next_request_gen = self
            .next_request_gen
            .checked_add(1)
            .ok_or(AssetError::AccountingOverflow)?;
        let generation = self.next_request_gen;
        let frame = self.frame;
        let r = self.rec_mut(id)?;
        r.state = ResidencyState::Requested;
        r.request_gen = generation;
        r.last_used_frame = frame;
        Ok(generation)
    }

    /// `Requested -> Generating`; reserves CPU bytes.
    pub fn begin_generating(&mut self, id: AssetId, cpu_bytes: u64) -> Result<(), AssetError> {
        self.check(id, ResidencyState::Generating)?;
        let class = self.rec(id)?.class;
        let c = self
            .classes
            .get(&class)
            .ok_or(AssetError::UnknownClass(class))?;
        let new_used = c
            .cpu_used
            .checked_add(cpu_bytes)
            .ok_or(AssetError::AccountingOverflow)?;
        if new_used > c.budget.cpu_bytes {
            return Err(AssetError::OverBudget {
                class,
                pool: MemoryPool::Cpu,
                needed: cpu_bytes,
                available: c.budget.cpu_bytes.saturating_sub(c.cpu_used),
            });
        }
        if let Some(c) = self.classes.get_mut(&class) {
            c.cpu_used = new_used;
        }
        let r = self.rec_mut(id)?;
        r.state = ResidencyState::Generating;
        r.cpu_bytes = cpu_bytes;
        Ok(())
    }

    /// `Generating -> CpuReady`; rejects completions of superseded requests.
    pub fn finish_generating(&mut self, id: AssetId, request_gen: u64) -> Result<(), AssetError> {
        self.check_from(id, ResidencyState::Generating, ResidencyState::CpuReady)?;
        if self.rec(id)?.request_gen != request_gen {
            return Err(AssetError::StaleRequest(id));
        }
        let frame = self.frame;
        let r = self.rec_mut(id)?;
        r.state = ResidencyState::CpuReady;
        r.last_used_frame = frame;
        Ok(())
    }

    /// `Requested | Generating -> Unloaded`; releases bytes, bumps generation.
    pub fn cancel(&mut self, id: AssetId) -> Result<(), AssetError> {
        let state = self.rec(id)?.state;
        if !matches!(
            state,
            ResidencyState::Requested | ResidencyState::Generating
        ) {
            return Err(AssetError::IllegalTransition {
                from: state,
                to: ResidencyState::Unloaded,
            });
        }
        let generation = self.bump_gen()?;
        self.release(id);
        let r = self.rec_mut(id)?;
        r.state = ResidencyState::Unloaded;
        r.request_gen = generation;
        Ok(())
    }

    /// Move to `Failed` where legal; releases bytes, bumps generation.
    pub fn fail(&mut self, id: AssetId) -> Result<(), AssetError> {
        self.check(id, ResidencyState::Failed)?;
        let generation = self.bump_gen()?;
        self.release(id);
        let r = self.rec_mut(id)?;
        r.state = ResidencyState::Failed;
        r.request_gen = generation;
        Ok(())
    }

    /// `CpuReady -> Uploading`; enforces the per-frame cap and GPU budget.
    ///
    /// An asset larger than the cap may upload only as the first upload of a
    /// frame, so it cannot starve.
    pub fn try_begin_upload(&mut self, id: AssetId, gpu_bytes: u64) -> Result<(), AssetError> {
        self.check(id, ResidencyState::Uploading)?;
        let class = self.rec(id)?.class;
        if self.uploaded_this_frame > 0 {
            let total = self
                .uploaded_this_frame
                .checked_add(gpu_bytes)
                .ok_or(AssetError::AccountingOverflow)?;
            if total > self.upload_cap_bytes {
                self.denied_uploads_cap += 1;
                return Err(AssetError::UploadCapExceeded {
                    needed: gpu_bytes,
                    remaining: self
                        .upload_cap_bytes
                        .saturating_sub(self.uploaded_this_frame),
                });
            }
        }
        let new_frame_total = self
            .uploaded_this_frame
            .checked_add(gpu_bytes)
            .ok_or(AssetError::AccountingOverflow)?;
        let c = self
            .classes
            .get(&class)
            .ok_or(AssetError::UnknownClass(class))?;
        let new_used = c
            .gpu_used
            .checked_add(gpu_bytes)
            .ok_or(AssetError::AccountingOverflow)?;
        if new_used > c.budget.gpu_bytes {
            let available = c.budget.gpu_bytes.saturating_sub(c.gpu_used);
            self.denied_uploads_budget += 1;
            return Err(AssetError::OverBudget {
                class,
                pool: MemoryPool::Gpu,
                needed: gpu_bytes,
                available,
            });
        }
        if let Some(c) = self.classes.get_mut(&class) {
            c.gpu_used = new_used;
        }
        self.uploaded_this_frame = new_frame_total;
        let r = self.rec_mut(id)?;
        r.state = ResidencyState::Uploading;
        r.gpu_bytes = gpu_bytes;
        Ok(())
    }

    /// `Uploading -> GpuResident`.
    pub fn complete_upload(&mut self, id: AssetId) -> Result<(), AssetError> {
        self.check(id, ResidencyState::GpuResident)?;
        let frame = self.frame;
        let r = self.rec_mut(id)?;
        r.state = ResidencyState::GpuResident;
        r.last_used_frame = frame;
        Ok(())
    }

    /// `Uploading -> CpuReady`; releases GPU bytes. The per-frame upload
    /// counter is not refunded.
    pub fn abort_upload(&mut self, id: AssetId) -> Result<(), AssetError> {
        self.check_from(id, ResidencyState::Uploading, ResidencyState::CpuReady)?;
        self.release_gpu(id);
        self.rec_mut(id)?.state = ResidencyState::CpuReady;
        Ok(())
    }

    /// Mark an asset as used this frame.
    pub fn touch(&mut self, id: AssetId) -> Result<(), AssetError> {
        let frame = self.frame;
        self.rec_mut(id)?.last_used_frame = frame;
        Ok(())
    }

    /// `CpuReady | GpuResident -> Evicting`.
    pub fn begin_evict(&mut self, id: AssetId) -> Result<(), AssetError> {
        self.check(id, ResidencyState::Evicting)?;
        self.rec_mut(id)?.state = ResidencyState::Evicting;
        Ok(())
    }

    /// `Evicting -> Unloaded`; releases CPU and GPU bytes.
    pub fn finish_evict(&mut self, id: AssetId) -> Result<(), AssetError> {
        self.check_from(id, ResidencyState::Evicting, ResidencyState::Unloaded)?;
        self.release(id);
        self.rec_mut(id)?.state = ResidencyState::Unloaded;
        Ok(())
    }

    /// LRU eviction candidates for `class` in `pool` until at least
    /// `bytes_needed` bytes are covered. Pure; the caller drives eviction.
    pub fn eviction_candidates(
        &self,
        class: AssetClass,
        pool: MemoryPool,
        bytes_needed: u64,
    ) -> Vec<AssetId> {
        if bytes_needed == 0 {
            return Vec::new();
        }
        let mut eligible: Vec<(u64, AssetId, u64)> = self
            .records
            .iter()
            .filter(|(_, r)| r.class == class)
            .filter(|(_, r)| match pool {
                MemoryPool::Gpu => r.state == ResidencyState::GpuResident,
                MemoryPool::Cpu => matches!(
                    r.state,
                    ResidencyState::CpuReady | ResidencyState::GpuResident
                ),
            })
            .map(|(id, r)| {
                let bytes = match pool {
                    MemoryPool::Cpu => r.cpu_bytes,
                    MemoryPool::Gpu => r.gpu_bytes,
                };
                (r.last_used_frame, *id, bytes)
            })
            .collect();
        eligible.sort_by_key(|&(frame, id, _)| (frame, id));
        let mut out = Vec::new();
        let mut sum: u64 = 0;
        for (_, id, bytes) in eligible {
            if sum >= bytes_needed {
                break;
            }
            out.push(id);
            sum = sum.saturating_add(bytes);
        }
        out
    }

    /// Snapshot of budgets, usage, upload counters and state counts.
    pub fn stats(&self) -> ResidencyStats {
        let mut state_counts = [0usize; 8];
        for r in self.records.values() {
            state_counts[r.state.index()] += 1;
        }
        ResidencyStats {
            frame: self.frame,
            upload_cap_bytes: self.upload_cap_bytes,
            uploaded_this_frame: self.uploaded_this_frame,
            denied_uploads_cap: self.denied_uploads_cap,
            denied_uploads_budget: self.denied_uploads_budget,
            classes: self
                .classes
                .iter()
                .map(|(class, c)| ClassStats {
                    class: *class,
                    cpu_budget: c.budget.cpu_bytes,
                    gpu_budget: c.budget.gpu_bytes,
                    cpu_used: c.cpu_used,
                    gpu_used: c.gpu_used,
                })
                .collect(),
            state_counts,
        }
    }

    /// Check that class usage equals the sum of record bytes and that
    /// records only hold bytes in states that allow it.
    pub fn verify_accounting(&self) -> bool {
        use ResidencyState::*;
        for r in self.records.values() {
            let cpu_ok = matches!(
                r.state,
                Generating | CpuReady | Uploading | GpuResident | Evicting
            ) || r.cpu_bytes == 0;
            let gpu_ok = matches!(r.state, Uploading | GpuResident | Evicting) || r.gpu_bytes == 0;
            if !cpu_ok || !gpu_ok {
                return false;
            }
        }
        self.classes.iter().all(|(class, c)| {
            let (mut cpu, mut gpu) = (0u64, 0u64);
            for r in self.records.values().filter(|r| r.class == *class) {
                match (cpu.checked_add(r.cpu_bytes), gpu.checked_add(r.gpu_bytes)) {
                    (Some(a), Some(b)) => {
                        cpu = a;
                        gpu = b;
                    }
                    _ => return false,
                }
            }
            cpu == c.cpu_used && gpu == c.gpu_used
        })
    }

    fn rec(&self, id: AssetId) -> Result<&Record, AssetError> {
        self.records.get(&id).ok_or(AssetError::UnknownAsset(id))
    }

    fn rec_mut(&mut self, id: AssetId) -> Result<&mut Record, AssetError> {
        self.records
            .get_mut(&id)
            .ok_or(AssetError::UnknownAsset(id))
    }

    /// Single transition-legality check used by every public operation.
    fn check(&self, id: AssetId, to: ResidencyState) -> Result<(), AssetError> {
        let from = self.rec(id)?.state;
        if from.can_transition(to) {
            Ok(())
        } else {
            Err(AssetError::IllegalTransition { from, to })
        }
    }

    /// Like [`Self::check`] but also requires the exact source state, for
    /// targets reachable from several states.
    fn check_from(
        &self,
        id: AssetId,
        from: ResidencyState,
        to: ResidencyState,
    ) -> Result<(), AssetError> {
        let actual = self.rec(id)?.state;
        if actual == from && actual.can_transition(to) {
            Ok(())
        } else {
            Err(AssetError::IllegalTransition { from: actual, to })
        }
    }

    fn bump_gen(&mut self) -> Result<u64, AssetError> {
        self.next_request_gen = self
            .next_request_gen
            .checked_add(1)
            .ok_or(AssetError::AccountingOverflow)?;
        Ok(self.next_request_gen)
    }

    /// Release all bytes held by a record.
    fn release(&mut self, id: AssetId) {
        self.release_gpu(id);
        if let Some(r) = self.records.get_mut(&id) {
            let bytes = std::mem::take(&mut r.cpu_bytes);
            if let Some(c) = self.classes.get_mut(&r.class) {
                c.cpu_used = c.cpu_used.saturating_sub(bytes);
            }
        }
    }

    fn release_gpu(&mut self, id: AssetId) {
        if let Some(r) = self.records.get_mut(&id) {
            let bytes = std::mem::take(&mut r.gpu_bytes);
            if let Some(c) = self.classes.get_mut(&r.class) {
                c.gpu_used = c.gpu_used.saturating_sub(bytes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ResidencyState::*;

    const C: AssetClass = AssetClass(1);

    fn mgr(cap: u64, cpu: u64, gpu: u64) -> ResidencyManager {
        let mut m = ResidencyManager::new(cap);
        m.set_class_budget(
            C,
            ClassBudget {
                cpu_bytes: cpu,
                gpu_bytes: gpu,
            },
        );
        m
    }

    fn id(k: &str) -> AssetId {
        AssetId::from_key(k).unwrap()
    }

    fn cpu_ready(m: &mut ResidencyManager, k: &str, cpu: u64) -> AssetId {
        let a = id(k);
        m.track(a, C).unwrap();
        let g = m.request(a).unwrap();
        m.begin_generating(a, cpu).unwrap();
        m.finish_generating(a, g).unwrap();
        a
    }

    fn used(m: &ResidencyManager) -> (u64, u64) {
        let c = &m.classes[&C];
        (c.cpu_used, c.gpu_used)
    }

    #[test]
    fn budget_full_lifecycle() {
        let mut m = mgr(1000, 1000, 1000);
        let a = id("a");
        m.track(a, C).unwrap();
        assert_eq!(m.state(a), Some(Unloaded));
        let g = m.request(a).unwrap();
        assert_eq!(m.state(a), Some(Requested));
        m.begin_generating(a, 100).unwrap();
        assert_eq!(m.state(a), Some(Generating));
        m.finish_generating(a, g).unwrap();
        assert_eq!(m.state(a), Some(CpuReady));
        m.try_begin_upload(a, 200).unwrap();
        assert_eq!(m.state(a), Some(Uploading));
        assert_eq!(used(&m), (100, 200));
        m.complete_upload(a).unwrap();
        assert_eq!(m.state(a), Some(GpuResident));
        m.begin_evict(a).unwrap();
        assert_eq!(m.state(a), Some(Evicting));
        m.finish_evict(a).unwrap();
        assert_eq!(m.state(a), Some(Unloaded));
        assert_eq!(used(&m), (0, 0));
    }

    #[test]
    fn budget_cpu_over_budget_rejected() {
        let mut m = mgr(1000, 150, 1000);
        let (a, b) = (id("a"), id("b"));
        for x in [a, b] {
            m.track(x, C).unwrap();
            m.request(x).unwrap();
        }
        m.begin_generating(a, 100).unwrap();
        assert_eq!(
            m.begin_generating(b, 100),
            Err(AssetError::OverBudget {
                class: C,
                pool: MemoryPool::Cpu,
                needed: 100,
                available: 50
            })
        );
        assert_eq!(m.state(b), Some(Requested));
        assert_eq!(used(&m).0, 100);
    }

    #[test]
    fn budget_gpu_over_budget_rejected() {
        let mut m = mgr(1000, 1000, 150);
        let a = cpu_ready(&mut m, "a", 10);
        let b = cpu_ready(&mut m, "b", 10);
        m.try_begin_upload(a, 100).unwrap();
        let before = (used(&m), m.stats_tuple());
        let err = m.try_begin_upload(b, 100).unwrap_err();
        assert!(matches!(
            err,
            AssetError::OverBudget {
                pool: MemoryPool::Gpu,
                ..
            }
        ));
        assert_eq!(m.state(b), Some(CpuReady));
        assert_eq!(used(&m), before.0);
        assert_eq!(m.uploaded_this_frame, 100);
        assert_eq!(m.denied_uploads_budget, 1);
    }

    impl ResidencyManager {
        fn stats_tuple(&self) -> (u64, u64) {
            (self.uploaded_this_frame, self.frame)
        }
    }

    #[test]
    fn budget_upload_cap_enforced_and_reset() {
        let mut m = mgr(100, 1000, 1000);
        let a = cpu_ready(&mut m, "a", 1);
        let b = cpu_ready(&mut m, "b", 1);
        m.try_begin_upload(a, 60).unwrap();
        assert_eq!(
            m.try_begin_upload(b, 60),
            Err(AssetError::UploadCapExceeded {
                needed: 60,
                remaining: 40
            })
        );
        assert_eq!(m.denied_uploads_cap, 1);
        m.begin_frame(1);
        m.try_begin_upload(b, 60).unwrap();
    }

    #[test]
    fn budget_oversize_asset_uploads_first_in_frame() {
        let mut m = mgr(100, 1000, 1000);
        let a = cpu_ready(&mut m, "a", 1);
        let b = cpu_ready(&mut m, "b", 1);
        m.try_begin_upload(a, 250).unwrap();
        assert!(matches!(
            m.try_begin_upload(b, 10),
            Err(AssetError::UploadCapExceeded { .. })
        ));
    }

    #[test]
    fn budget_illegal_operation_leaves_state() {
        let mut m = mgr(1000, 1000, 1000);
        let a = cpu_ready(&mut m, "a", 10);
        let before = used(&m);
        assert_eq!(
            m.complete_upload(a),
            Err(AssetError::IllegalTransition {
                from: CpuReady,
                to: GpuResident
            })
        );
        assert_eq!(m.state(a), Some(CpuReady));
        assert_eq!(used(&m), before);
    }

    #[test]
    fn budget_cancel_and_fail_release_bytes() {
        let mut m = mgr(1000, 1000, 1000);
        let a = id("a");
        m.track(a, C).unwrap();
        let g = m.request(a).unwrap();
        m.begin_generating(a, 100).unwrap();
        m.cancel(a).unwrap();
        assert_eq!(used(&m), (0, 0));
        assert_eq!(m.state(a), Some(Unloaded));
        assert_eq!(
            m.finish_generating(a, g),
            Err(AssetError::IllegalTransition {
                from: Unloaded,
                to: CpuReady
            })
        );
        // Stale generation on a re-requested asset.
        let g2 = m.request(a).unwrap();
        m.begin_generating(a, 100).unwrap();
        assert_ne!(g, g2);
        assert_eq!(m.finish_generating(a, g), Err(AssetError::StaleRequest(a)));
        m.finish_generating(a, g2).unwrap();
        m.try_begin_upload(a, 50).unwrap();
        assert_eq!(used(&m), (100, 50));
        m.fail(a).unwrap();
        assert_eq!(used(&m), (0, 0));
        assert_eq!(m.state(a), Some(Failed));
        m.untrack(a).unwrap();
        assert_eq!(m.state(a), None);
    }

    #[test]
    fn budget_unknown_class_and_asset() {
        let mut m = mgr(10, 10, 10);
        assert_eq!(
            m.track(id("a"), AssetClass(9)),
            Err(AssetError::UnknownClass(AssetClass(9)))
        );
        assert_eq!(m.request(id("x")), Err(AssetError::UnknownAsset(id("x"))));
        m.track(id("a"), C).unwrap();
        assert_eq!(
            m.track(id("a"), C),
            Err(AssetError::AlreadyTracked(id("a")))
        );
    }

    fn gpu_resident(m: &mut ResidencyManager, k: &str, frame: u64, gpu: u64) -> AssetId {
        let a = cpu_ready(m, k, 10);
        m.begin_frame(frame);
        m.try_begin_upload(a, gpu).unwrap();
        m.complete_upload(a).unwrap();
        a
    }

    fn snapshot(m: &ResidencyManager) -> (Vec<Option<ResidencyState>>, ResidencyStats) {
        (
            ["a", "b", "c", "d", "e"]
                .iter()
                .map(|k| m.state(id(k)))
                .collect(),
            m.stats(),
        )
    }

    #[test]
    fn eviction_lru_order() {
        let mut m = mgr(10_000, 10_000, 10_000);
        let a5 = gpu_resident(&mut m, "a", 5, 100);
        let a2 = gpu_resident(&mut m, "b", 2, 100);
        let _a9 = gpu_resident(&mut m, "c", 9, 100);
        assert_eq!(m.eviction_candidates(C, MemoryPool::Gpu, 150), vec![a2, a5]);
    }

    #[test]
    fn eviction_tie_broken_by_id() {
        let mut m = mgr(10_000, 10_000, 10_000);
        let x = gpu_resident(&mut m, "a", 3, 100);
        let y = gpu_resident(&mut m, "b", 3, 100);
        let mut expected = vec![x, y];
        expected.sort();
        assert_eq!(m.eviction_candidates(C, MemoryPool::Gpu, 200), expected);
    }

    #[test]
    fn eviction_excludes_in_flight() {
        let mut m = mgr(10_000, 10_000, 10_000);
        let a = id("a");
        m.track(a, C).unwrap();
        m.request(a).unwrap();
        let b = id("b");
        m.track(b, C).unwrap();
        let gb = m.request(b).unwrap();
        m.begin_generating(b, 10).unwrap();
        let _ = gb;
        let c = cpu_ready(&mut m, "c", 10);
        m.try_begin_upload(c, 10).unwrap(); // Uploading
        let d = cpu_ready(&mut m, "d", 10);
        m.begin_evict(d).unwrap(); // Evicting
        let e = id("e");
        m.track(e, C).unwrap();
        m.request(e).unwrap();
        m.fail(e).unwrap(); // Failed
        let f = cpu_ready(&mut m, "f", 10); // CpuReady
        let g = gpu_resident(&mut m, "g", 1, 10);
        assert_eq!(m.eviction_candidates(C, MemoryPool::Gpu, u64::MAX), vec![g]);
        let cpu = m.eviction_candidates(C, MemoryPool::Cpu, u64::MAX);
        assert_eq!(cpu.len(), 2);
        assert!(cpu.contains(&f) && cpu.contains(&g));
    }

    #[test]
    fn eviction_does_not_mutate() {
        let mut m = mgr(10_000, 10_000, 10_000);
        gpu_resident(&mut m, "a", 5, 100);
        gpu_resident(&mut m, "b", 2, 100);
        let before = snapshot(&m);
        let _ = m.eviction_candidates(C, MemoryPool::Gpu, 150);
        assert_eq!(snapshot(&m), before);
    }

    #[test]
    fn eviction_zero_needed_and_insufficient() {
        let mut m = mgr(10_000, 10_000, 10_000);
        let a = gpu_resident(&mut m, "a", 5, 100);
        let b = gpu_resident(&mut m, "b", 2, 100);
        assert!(m.eviction_candidates(C, MemoryPool::Gpu, 0).is_empty());
        assert_eq!(
            m.eviction_candidates(C, MemoryPool::Gpu, 10_000),
            vec![b, a]
        );
    }

    #[test]
    fn eviction_other_class_ignored() {
        let mut m = mgr(10_000, 10_000, 10_000);
        let other = AssetClass(2);
        m.set_class_budget(
            other,
            ClassBudget {
                cpu_bytes: 1000,
                gpu_bytes: 1000,
            },
        );
        let a = gpu_resident(&mut m, "a", 1, 100);
        let o = id("o");
        m.track(o, other).unwrap();
        let g = m.request(o).unwrap();
        m.begin_generating(o, 10).unwrap();
        m.finish_generating(o, g).unwrap();
        m.try_begin_upload(o, 10).unwrap();
        m.complete_upload(o).unwrap();
        assert_eq!(m.eviction_candidates(C, MemoryPool::Gpu, 1000), vec![a]);
        assert_eq!(m.eviction_candidates(other, MemoryPool::Gpu, 1000), vec![o]);
    }

    #[test]
    fn eviction_stats_reflect_state() {
        let mut m = mgr(100, 500, 300);
        let a = gpu_resident(&mut m, "a", 4, 80);
        let b = cpu_ready(&mut m, "b", 20);
        assert!(m.try_begin_upload(b, 80).is_err()); // cap
        let s = m.stats();
        assert_eq!(s.frame, 4);
        assert_eq!(s.upload_cap_bytes, 100);
        assert_eq!(s.uploaded_this_frame, 80);
        assert_eq!(s.denied_uploads_cap, 1);
        assert_eq!(s.denied_uploads_budget, 0);
        assert_eq!(
            s.classes,
            vec![ClassStats {
                class: C,
                cpu_budget: 500,
                gpu_budget: 300,
                cpu_used: 30,
                gpu_used: 80
            }]
        );
        assert_eq!(s.state_counts[GpuResident.index()], 1);
        assert_eq!(s.state_counts[CpuReady.index()], 1);
        assert_eq!(s.state_counts.iter().sum::<usize>(), 2);
        let _ = a;
    }

    fn next(s: &mut u64) -> u64 {
        let mut x = *s;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *s = x;
        x
    }

    #[test]
    fn eviction_random_walk_accounting() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut m = ResidencyManager::new(500);
        let classes = [AssetClass(0), AssetClass(1), AssetClass(2)];
        for c in classes {
            m.set_class_budget(
                c,
                ClassBudget {
                    cpu_bytes: 800,
                    gpu_bytes: 600,
                },
            );
        }
        let ids: Vec<AssetId> = (0..32).map(|i| id(&format!("asset{i}"))).collect();
        for (i, a) in ids.iter().enumerate() {
            m.track(*a, classes[i % 3]).unwrap();
        }
        for step in 0..10_000u64 {
            if step % 50 == 0 {
                m.begin_frame(step / 50);
            }
            let a = ids[(next(&mut seed) % 32) as usize];
            let bytes = next(&mut seed) % 400;
            let gen_arg = next(&mut seed) % 40;
            let op = next(&mut seed) % 14;
            let _ = match op {
                0 => m.request(a).map(|_| ()),
                1 => m.begin_generating(a, bytes),
                2 => m.finish_generating(a, gen_arg),
                3 => {
                    let g = m.request_generation(a).unwrap_or(0);
                    m.finish_generating(a, g)
                }
                4 => m.cancel(a),
                5 => m.fail(a),
                6 => m.try_begin_upload(a, bytes),
                7 => m.complete_upload(a),
                8 => m.abort_upload(a),
                9 => m.touch(a),
                10 => m.begin_evict(a),
                11 => m.finish_evict(a),
                12 => {
                    let c = classes[(bytes % 3) as usize];
                    let _ = m.eviction_candidates(c, MemoryPool::Gpu, bytes);
                    Ok(())
                }
                _ => m
                    .untrack(a)
                    .and_then(|()| m.track(a, classes[(bytes % 3) as usize])),
            };
            assert!(
                m.verify_accounting(),
                "accounting drift at step {step} op {op}"
            );
        }
    }

    #[test]
    fn budget_overflow_checked() {
        let mut m = mgr(u64::MAX, u64::MAX, u64::MAX);
        let a = cpu_ready(&mut m, "a", u64::MAX - 1);
        let b = id("b");
        m.track(b, C).unwrap();
        m.request(b).unwrap();
        assert_eq!(
            m.begin_generating(b, 10),
            Err(AssetError::AccountingOverflow)
        );
        assert_eq!(m.state(b), Some(Requested));
        m.try_begin_upload(a, u64::MAX - 1).unwrap();
        let c = cpu_ready(&mut m, "c", 0);
        assert!(m.try_begin_upload(c, 10).is_err());
        assert_eq!(m.state(c), Some(CpuReady));
    }
}
