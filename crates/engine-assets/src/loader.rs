//! Synthetic asynchronous loader driving the residency state machine on the
//! Phase 2 job system. The real streaming scheduler is Phase 6.
//!
//! Workers send `(id, request_gen, result)` over a std mpsc channel; the
//! owning thread applies completions in [`AsyncLoader::poll`], accepting only
//! those whose generation matches the pending request.

use std::collections::BTreeMap;
use std::sync::mpsc;

use engine_jobs::{CancelToken, JobHandle, JobStatus, JobSystem, Priority};

use crate::error::AssetError;
use crate::id::AssetId;
use crate::manager::ResidencyManager;
use crate::residency::ResidencyState;

/// Outcome of a load, reported by [`AsyncLoader::poll`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadEvent {
    /// Load finished; the asset is `CpuReady` and `data` is handed over.
    Loaded {
        /// Asset.
        id: AssetId,
        /// Produced bytes.
        data: Vec<u8>,
    },
    /// Load failed (panic or producer returned nothing); asset is `Failed`.
    Failed {
        /// Asset.
        id: AssetId,
    },
    /// Job was cancelled externally; asset returned to `Unloaded`.
    Cancelled {
        /// Asset.
        id: AssetId,
    },
}

/// `None` means the producer returned nothing (treated as failure).
type Completion = (AssetId, u64, Option<Vec<u8>>);

/// Asynchronous loader over a [`JobSystem`].
pub struct AsyncLoader {
    tx: mpsc::Sender<Completion>,
    rx: mpsc::Receiver<Completion>,
    pending: BTreeMap<AssetId, (u64, JobHandle)>,
    abandoned: Vec<JobHandle>,
}

impl Default for AsyncLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncLoader {
    /// Create an idle loader.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            pending: BTreeMap::new(),
            abandoned: Vec::new(),
        }
    }

    /// Reserve `cpu_bytes`, move the asset to `Generating` and submit the
    /// producer. State and budget errors return before anything is submitted.
    pub fn start<F>(
        &mut self,
        jobs: &JobSystem,
        manager: &mut ResidencyManager,
        id: AssetId,
        priority: Priority,
        cpu_bytes: u64,
        produce: F,
    ) -> Result<(), AssetError>
    where
        F: FnOnce(&CancelToken) -> Option<Vec<u8>> + Send + 'static,
    {
        manager.begin_generating(id, cpu_bytes)?;
        let gen_id = manager
            .request_generation(id)
            .ok_or(AssetError::UnknownAsset(id))?;
        let tx = self.tx.clone();
        let handle = jobs.submit(priority, move |tok| {
            if tok.is_cancelled() {
                return;
            }
            let r = produce(tok);
            if tok.is_cancelled() {
                return;
            }
            let _ = tx.send((id, gen_id, r));
        });
        if let Some((_, old)) = self.pending.insert(id, (gen_id, handle)) {
            old.cancel();
            self.abandoned.push(old);
        }
        Ok(())
    }

    /// Cancel an in-flight load: asset returns to `Unloaded`, bytes are
    /// released and any late completion becomes stale.
    pub fn cancel(
        &mut self,
        manager: &mut ResidencyManager,
        id: AssetId,
    ) -> Result<(), AssetError> {
        if let Some((_, handle)) = self.pending.remove(&id) {
            handle.cancel();
            self.abandoned.push(handle);
        }
        manager.cancel(id)
    }

    /// Number of loads still in flight.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Apply finished work without blocking.
    ///
    /// Order matters: (1) snapshot job statuses, (2) drain the channel, (3)
    /// resolve remaining pending jobs from the snapshot. A job publishes its
    /// status after its closure returns, so a message sent by a job whose
    /// snapshot status is terminal is already in the channel at step 2.
    pub fn poll(&mut self, manager: &mut ResidencyManager) -> Vec<LoadEvent> {
        let snapshot: BTreeMap<AssetId, Option<JobStatus>> = self
            .pending
            .iter()
            .map(|(id, (_, h))| (*id, h.try_status()))
            .collect();
        self.abandoned.retain(|h| h.try_status().is_none());
        self.apply(manager, &snapshot)
    }

    /// Block until every pending (and abandoned) job has finished, then apply
    /// all completions. Deterministic; never sleeps.
    pub fn wait_all(&mut self, manager: &mut ResidencyManager) -> Vec<LoadEvent> {
        let mut snapshot: BTreeMap<AssetId, Option<JobStatus>> = BTreeMap::new();
        let taken = std::mem::take(&mut self.pending);
        let mut regs: Vec<(AssetId, u64)> = Vec::new();
        for (id, (g, handle)) in taken {
            snapshot.insert(id, Some(handle.wait()));
            regs.push((id, g));
        }
        for handle in std::mem::take(&mut self.abandoned) {
            let _ = handle.wait();
        }
        // Handles are consumed; re-register generations without handles by
        // applying directly.
        self.apply_waited(manager, &regs, &snapshot)
    }

    fn apply(
        &mut self,
        manager: &mut ResidencyManager,
        snapshot: &BTreeMap<AssetId, Option<JobStatus>>,
    ) -> Vec<LoadEvent> {
        let mut events: BTreeMap<AssetId, LoadEvent> = BTreeMap::new();
        while let Ok((id, gen_id, r)) = self.rx.try_recv() {
            let matches = self.pending.get(&id).is_some_and(|(g, _)| *g == gen_id);
            if !matches {
                continue;
            }
            self.pending.remove(&id);
            if let Some(ev) = Self::complete(manager, id, gen_id, r) {
                events.insert(id, ev);
            }
        }
        let ids: Vec<AssetId> = self.pending.keys().copied().collect();
        for id in ids {
            let Some(Some(status)) = snapshot.get(&id) else {
                continue;
            };
            self.pending.remove(&id);
            if let Some(ev) = Self::terminal(manager, id, *status) {
                events.insert(id, ev);
            }
        }
        events.into_values().collect()
    }

    fn apply_waited(
        &mut self,
        manager: &mut ResidencyManager,
        regs: &[(AssetId, u64)],
        snapshot: &BTreeMap<AssetId, Option<JobStatus>>,
    ) -> Vec<LoadEvent> {
        let mut events: BTreeMap<AssetId, LoadEvent> = BTreeMap::new();
        let mut done: BTreeMap<AssetId, u64> = regs.iter().copied().collect();
        while let Ok((id, gen_id, r)) = self.rx.try_recv() {
            if done.get(&id) != Some(&gen_id) {
                continue;
            }
            done.remove(&id);
            if let Some(ev) = Self::complete(manager, id, gen_id, r) {
                events.insert(id, ev);
            }
        }
        for id in done.keys() {
            if let Some(Some(status)) = snapshot.get(id)
                && let Some(ev) = Self::terminal(manager, *id, *status)
            {
                events.insert(*id, ev);
            }
        }
        events.into_values().collect()
    }

    fn complete(
        manager: &mut ResidencyManager,
        id: AssetId,
        gen_id: u64,
        result: Option<Vec<u8>>,
    ) -> Option<LoadEvent> {
        match result {
            Some(data) => manager
                .finish_generating(id, gen_id)
                .ok()
                .map(|()| LoadEvent::Loaded { id, data }),
            None => manager.fail(id).ok().map(|()| LoadEvent::Failed { id }),
        }
    }

    /// Resolve a job that ended without a (matching) message.
    fn terminal(
        manager: &mut ResidencyManager,
        id: AssetId,
        status: JobStatus,
    ) -> Option<LoadEvent> {
        match status {
            JobStatus::Cancelled => {
                if manager.state(id) == Some(ResidencyState::Generating) {
                    manager.cancel(id).ok()?;
                }
                Some(LoadEvent::Cancelled { id })
            }
            // Panicked, or Completed without a message (defensive).
            _ => manager.fail(id).ok().map(|()| LoadEvent::Failed { id }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::{AssetClass, ClassBudget};

    fn setup() -> (ResidencyManager, AssetId) {
        let mut m = ResidencyManager::new(1024);
        m.set_class_budget(
            AssetClass(1),
            ClassBudget {
                cpu_bytes: 1024,
                gpu_bytes: 1024,
            },
        );
        let id = AssetId::from_key("a").expect("key");
        m.track(id, AssetClass(1)).expect("track");
        (m, id)
    }

    #[test]
    fn loader_unit_poll_without_jobs_is_empty() {
        let (mut m, _) = setup();
        let mut l = AsyncLoader::new();
        assert!(l.poll(&mut m).is_empty());
        assert_eq!(l.pending_len(), 0);
    }

    #[test]
    fn loader_unit_start_requires_requested_state() {
        let (mut m, id) = setup();
        let jobs = JobSystem::builder()
            .workers(1)
            .start_paused(true)
            .build()
            .expect("jobs");
        let mut l = AsyncLoader::new();
        let before = jobs.queued_len();
        let r = l.start(&jobs, &mut m, id, Priority::Normal, 8, |_| Some(vec![1]));
        assert_eq!(
            r,
            Err(AssetError::IllegalTransition {
                from: ResidencyState::Unloaded,
                to: ResidencyState::Generating
            })
        );
        assert_eq!(jobs.queued_len(), before);
        assert_eq!(l.pending_len(), 0);
        jobs.shutdown();
    }
}
