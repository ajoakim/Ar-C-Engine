//! Streaming pipeline over [`ResidencyManager`] and [`AsyncLoader`].
//!
//! `update` never blocks: it sweeps cancellations, starts a counted number of
//! queued requests, polls finished work, applies a counted number of
//! completions and uploads within the residency upload cap.

use std::cmp::Reverse;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use engine_assets::{
    AssetClass, AssetError, AssetId, AsyncLoader, LoadEvent, MemoryPool, ResidencyManager,
    ResidencyState,
};
use engine_jobs::{CancelToken, JobSystem, Priority};

use crate::config::{FrameReport, StreamConfig};
use crate::error::StreamError;
use crate::key::StreamKey;
use crate::request::StreamRequest;
use crate::source::{StreamSource, Uploader};

type QueueKey = (Reverse<Priority>, u64);

struct InFlight {
    priority: Priority,
    seq: u64,
    #[allow(dead_code)]
    class: AssetClass,
    cancel: CancelToken,
}

/// Generic streaming manager: bounded priority queue, budgeted starts,
/// cancellation that frees budget immediately and counted per-frame caps.
pub struct StreamManager<K: StreamKey, S: StreamSource<K>> {
    config: StreamConfig,
    residency: ResidencyManager,
    loader: AsyncLoader,
    source: Arc<S>,
    keys: BTreeMap<AssetId, K>,
    pending: BTreeMap<QueueKey, StreamRequest<K>>,
    pending_index: BTreeMap<AssetId, QueueKey>,
    next_seq: u64,
    in_flight: BTreeMap<AssetId, InFlight>,
    completed: VecDeque<LoadEvent>,
    ready: BTreeMap<AssetId, Vec<u8>>,
    frame: u64,
}

impl<K: StreamKey, S: StreamSource<K>> StreamManager<K, S> {
    /// Create a manager; validates `config`.
    pub fn new(
        config: StreamConfig,
        residency: ResidencyManager,
        source: Arc<S>,
    ) -> Result<Self, StreamError> {
        config.validate()?;
        Ok(Self {
            config,
            residency,
            loader: AsyncLoader::new(),
            source,
            keys: BTreeMap::new(),
            pending: BTreeMap::new(),
            pending_index: BTreeMap::new(),
            next_seq: 0,
            in_flight: BTreeMap::new(),
            completed: VecDeque::new(),
            ready: BTreeMap::new(),
            frame: 0,
        })
    }

    /// Queue a request. Never blocks; rejects with `QueueFull` at capacity.
    pub fn request(&mut self, req: StreamRequest<K>) -> Result<(), StreamError> {
        let id = req.key.asset_id();
        let busy = matches!(
            self.residency.state(id),
            Some(
                ResidencyState::Requested
                    | ResidencyState::Generating
                    | ResidencyState::CpuReady
                    | ResidencyState::Uploading
                    | ResidencyState::GpuResident
                    | ResidencyState::Evicting
            )
        );
        if self.pending_index.contains_key(&id) || self.in_flight.contains_key(&id) || busy {
            return Err(StreamError::Duplicate(id));
        }
        if self.pending.len() >= self.config.max_pending {
            return Err(StreamError::QueueFull {
                max: self.config.max_pending,
            });
        }
        if self.residency.state(id).is_none() {
            self.residency.track(id, req.budget_class)?;
        }
        let qk = (Reverse(req.priority), self.next_seq);
        self.next_seq += 1;
        self.keys.insert(id, req.key.clone());
        self.pending_index.insert(id, qk);
        self.pending.insert(qk, req);
        Ok(())
    }

    /// Cancel a pending or in-flight key, freeing its budget immediately.
    /// Returns whether anything was cancelled.
    pub fn cancel(&mut self, key: &K) -> Result<bool, StreamError> {
        let id = key.asset_id();
        if let Some(qk) = self.pending_index.remove(&id) {
            self.pending.remove(&qk);
            return Ok(true);
        }
        if self.in_flight.contains_key(&id) {
            self.cancel_in_flight(id)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Advance the pipeline by one frame without waiting on any worker.
    pub fn update(
        &mut self,
        jobs: &JobSystem,
        frame: u64,
        uploader: &mut dyn Uploader<K>,
    ) -> Result<FrameReport, StreamError> {
        self.frame = frame;
        self.residency.begin_frame(frame);
        let mut report = FrameReport {
            frame,
            ..FrameReport::default()
        };

        // 2. Sweep cancellations.
        let cancelled_pending: Vec<AssetId> = self
            .pending
            .values()
            .filter(|r| r.cancel.is_cancelled())
            .map(|r| r.key.asset_id())
            .collect();
        for id in cancelled_pending {
            if let Some(qk) = self.pending_index.remove(&id) {
                self.pending.remove(&qk);
                report.cancelled += 1;
            }
        }
        let cancelled_flight: Vec<AssetId> = self
            .in_flight
            .iter()
            .filter(|(_, f)| f.cancel.is_cancelled())
            .map(|(id, _)| *id)
            .collect();
        for id in cancelled_flight {
            self.cancel_in_flight(id)?;
            report.cancelled += 1;
        }

        // 3. Start queued requests.
        while report.started < self.config.max_starts_per_frame {
            let Some((qk, req)) = self.pending.pop_first() else {
                break;
            };
            let id = req.key.asset_id();
            self.pending_index.remove(&id);
            self.residency.request(id)?;
            let est = self.source.estimate_bytes(&req.key);
            let src = Arc::clone(&self.source);
            let k = req.key.clone();
            let produce = move |tok: &CancelToken| src.produce(&k, tok);
            match self
                .loader
                .start(jobs, &mut self.residency, id, req.priority, est, produce)
            {
                Ok(()) => {
                    self.in_flight.insert(
                        id,
                        InFlight {
                            priority: req.priority,
                            seq: qk.1,
                            class: req.budget_class,
                            cancel: req.cancel.clone(),
                        },
                    );
                    report.started += 1;
                }
                Err(AssetError::OverBudget {
                    pool: MemoryPool::Cpu,
                    ..
                }) => {
                    self.residency.cancel(id)?;
                    self.pending_index.insert(id, qk);
                    self.pending.insert(qk, req);
                    break;
                }
                Err(_) => {
                    self.residency.fail(id)?;
                    report.failed += 1;
                }
            }
        }

        // 4. Poll (non-blocking).
        let events = self.loader.poll(&mut self.residency);
        self.completed.extend(events);

        // 5. Apply a counted number of completions.
        while report.completions_applied < self.config.max_completions_per_frame {
            let Some(ev) = self.completed.pop_front() else {
                break;
            };
            report.completions_applied += 1;
            match ev {
                LoadEvent::Loaded { id, data } => {
                    if self.in_flight.contains_key(&id) {
                        self.ready.insert(id, data);
                    }
                }
                LoadEvent::Failed { id } => {
                    if self.in_flight.contains_key(&id) {
                        self.retire(id);
                        report.failed += 1;
                    }
                }
                LoadEvent::Cancelled { id } => {
                    if self.in_flight.contains_key(&id) {
                        self.retire(id);
                        report.cancelled += 1;
                    }
                }
            }
        }

        // 6. Uploads in (priority desc, seq) order.
        let mut order: Vec<(Reverse<Priority>, u64, AssetId)> = self
            .ready
            .keys()
            .filter_map(|id| {
                self.in_flight
                    .get(id)
                    .map(|f| (Reverse(f.priority), f.seq, *id))
            })
            .collect();
        order.sort();
        for (_, _, id) in order {
            let Some(key) = self.keys.get(&id).cloned() else {
                continue;
            };
            let gpu = self.ready.get(&id).map_or(0, |d| d.len() as u64);
            match self.residency.try_begin_upload(id, gpu) {
                Ok(()) => {
                    let accepted = self
                        .ready
                        .get(&id)
                        .is_some_and(|data| uploader.upload(&key, data));
                    if accepted {
                        self.residency.complete_upload(id)?;
                        self.ready.remove(&id);
                        self.retire(id);
                        report.uploads += 1;
                        report.uploaded_bytes += gpu;
                    } else {
                        self.residency.abort_upload(id)?;
                    }
                }
                Err(AssetError::UploadCapExceeded { .. }) => break,
                Err(AssetError::OverBudget {
                    pool: MemoryPool::Gpu,
                    ..
                }) => {}
                Err(e) => return Err(e.into()),
            }
        }

        // 7. Report.
        report.pending = self.pending.len();
        report.in_flight = self.in_flight.len();
        report.deferred_completions = self.completed.len();
        report.awaiting_upload = self.ready.len();
        Ok(report)
    }

    /// BLOCKING: waits for every started job and buffers its completion.
    /// Test/shutdown only; never call on the frame path.
    pub fn settle(&mut self) {
        let ev = self.loader.wait_all(&mut self.residency);
        self.completed.extend(ev);
    }

    /// Queued (not started) request count.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Started, not yet resident request count.
    pub fn in_flight_len(&self) -> usize {
        self.in_flight.len()
    }

    /// Pending keys in start order.
    pub fn pending_keys(&self) -> Vec<K> {
        self.pending.values().map(|r| r.key.clone()).collect()
    }

    /// Whether `key` is queued and not started.
    pub fn is_pending(&self, key: &K) -> bool {
        self.pending_index.contains_key(&key.asset_id())
    }

    /// Residency state of `key`, if tracked.
    pub fn state(&self, key: &K) -> Option<ResidencyState> {
        self.residency.state(key.asset_id())
    }

    /// Read access to residency accounting.
    pub fn residency(&self) -> &ResidencyManager {
        &self.residency
    }

    /// Frame index of the last update.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Active configuration.
    pub fn config(&self) -> StreamConfig {
        self.config
    }

    fn retire(&mut self, id: AssetId) {
        self.in_flight.remove(&id);
    }

    fn cancel_in_flight(&mut self, id: AssetId) -> Result<(), StreamError> {
        match self.residency.state(id) {
            Some(ResidencyState::Requested | ResidencyState::Generating) => {
                self.loader.cancel(&mut self.residency, id)?;
            }
            Some(ResidencyState::CpuReady) => {
                self.residency.begin_evict(id)?;
                self.residency.finish_evict(id)?;
            }
            _ => {}
        }
        self.completed.retain(|ev| match ev {
            LoadEvent::Loaded { id: i, .. }
            | LoadEvent::Failed { id: i }
            | LoadEvent::Cancelled { id: i } => *i != id,
        });
        self.ready.remove(&id);
        self.retire(id);
        Ok(())
    }
}
