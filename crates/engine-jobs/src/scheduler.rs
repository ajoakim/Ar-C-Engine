//! Priority queue scheduler served by named std worker threads.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread::JoinHandle;

use crate::cancel::CancelToken;
use crate::error::JobError;
use crate::priority::Priority;

/// Final status of a job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobStatus {
    /// Ran to completion.
    Completed,
    /// Cancelled before or during execution.
    Cancelled,
    /// The job panicked; the worker survived.
    Panicked,
}

type JobFn = Box<dyn FnOnce(&CancelToken) + Send>;

struct Entry {
    priority: Priority,
    seq: u64,
    token: CancelToken,
    job: JobFn,
    done: crossbeam_channel::Sender<JobStatus>,
}

impl Entry {
    fn key(&self) -> (Priority, std::cmp::Reverse<u64>) {
        (self.priority, std::cmp::Reverse(self.seq))
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

struct State {
    heap: BinaryHeap<Entry>,
    paused: bool,
    shutting_down: bool,
    next_seq: u64,
}

struct Shared {
    queue: Mutex<State>,
    cv: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Handle to a submitted job.
pub struct JobHandle {
    token: CancelToken,
    done: crossbeam_channel::Receiver<JobStatus>,
    cached: OnceLock<JobStatus>,
}

impl JobHandle {
    /// Request cancellation.
    pub fn cancel(&self) {
        self.token.cancel();
    }

    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Clone of the job's cancel token.
    #[must_use]
    pub fn token(&self) -> CancelToken {
        self.token.clone()
    }

    /// Non-blocking status poll.
    #[must_use]
    pub fn try_status(&self) -> Option<JobStatus> {
        if let Some(s) = self.cached.get() {
            return Some(*s);
        }
        match self.done.try_recv() {
            Ok(s) => Some(*self.cached.get_or_init(|| s)),
            Err(_) => None,
        }
    }

    /// Block until the job resolves. A job dropped without reporting is `Cancelled`.
    #[must_use]
    pub fn wait(self) -> JobStatus {
        if let Some(s) = self.cached.get() {
            return *s;
        }
        self.done.recv().unwrap_or(JobStatus::Cancelled)
    }
}

/// Builder for [`JobSystem`].
pub struct JobSystemBuilder {
    workers: usize,
    start_paused: bool,
    thread_name: String,
}

impl JobSystemBuilder {
    /// Number of worker threads.
    #[must_use]
    pub fn workers(mut self, n: usize) -> Self {
        self.workers = n;
        self
    }

    /// Start with workers paused (deterministic tests).
    #[must_use]
    pub fn start_paused(mut self, paused: bool) -> Self {
        self.start_paused = paused;
        self
    }

    /// Worker thread name prefix.
    #[must_use]
    pub fn thread_name(mut self, prefix: &str) -> Self {
        self.thread_name = prefix.to_owned();
        self
    }

    /// Spawn the workers.
    ///
    /// # Errors
    /// `ZeroWorkers` if zero workers requested; `Spawn` if a thread fails to start.
    pub fn build(self) -> Result<JobSystem, JobError> {
        if self.workers == 0 {
            return Err(JobError::ZeroWorkers);
        }
        let shared = Arc::new(Shared {
            queue: Mutex::new(State {
                heap: BinaryHeap::new(),
                paused: self.start_paused,
                shutting_down: false,
                next_seq: 0,
            }),
            cv: Condvar::new(),
        });
        let mut system = JobSystem {
            shared: Arc::clone(&shared),
            workers: Vec::with_capacity(self.workers),
        };
        for i in 0..self.workers {
            let s = Arc::clone(&shared);
            let spawned = std::thread::Builder::new()
                .name(format!("{}-{i}", self.thread_name))
                .spawn(move || worker_loop(&s));
            match spawned {
                Ok(h) => system.workers.push(h),
                Err(e) => {
                    system.stop(true);
                    return Err(JobError::Spawn(e));
                }
            }
        }
        Ok(system)
    }
}

fn worker_loop(shared: &Shared) {
    loop {
        let entry = {
            let mut state = shared.lock();
            while (state.paused && !state.shutting_down)
                || (state.heap.is_empty() && !state.shutting_down)
            {
                state = shared
                    .cv
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            match state.heap.pop() {
                Some(e) => e,
                None => return,
            }
        };
        let Entry {
            priority,
            token,
            job,
            done,
            ..
        } = entry;
        if token.is_cancelled() {
            drop(job);
            let _ = done.send(JobStatus::Cancelled);
            continue;
        }
        let span = tracing::trace_span!("job", priority = ?priority);
        let _guard = span.enter();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&token)));
        let status = match result {
            Err(_) => JobStatus::Panicked,
            Ok(()) if token.is_cancelled() => JobStatus::Cancelled,
            Ok(()) => JobStatus::Completed,
        };
        let _ = done.send(status);
    }
}

/// Prioritized job system.
pub struct JobSystem {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}

impl JobSystem {
    /// Start building a job system.
    #[must_use]
    pub fn builder() -> JobSystemBuilder {
        let n = std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .saturating_sub(1)
            .max(1);
        JobSystemBuilder {
            workers: n,
            start_paused: false,
            thread_name: "engine-job".to_owned(),
        }
    }

    /// Submit a job at the given priority.
    pub fn submit<F>(&self, priority: Priority, job: F) -> JobHandle
    where
        F: FnOnce(&CancelToken) + Send + 'static,
    {
        let token = CancelToken::new();
        let (tx, rx) = crossbeam_channel::bounded(1);
        {
            let mut state = self.shared.lock();
            let seq = state.next_seq;
            state.next_seq += 1;
            state.heap.push(Entry {
                priority,
                seq,
                token: token.clone(),
                job: Box::new(job),
                done: tx,
            });
        }
        self.shared.cv.notify_one();
        JobHandle {
            token,
            done: rx,
            cached: OnceLock::new(),
        }
    }

    /// Pause workers (running jobs finish).
    pub fn pause(&self) {
        self.shared.lock().paused = true;
        self.shared.cv.notify_all();
    }

    /// Resume workers.
    pub fn resume(&self) {
        self.shared.lock().paused = false;
        self.shared.cv.notify_all();
    }

    /// Number of queued (not yet started) jobs.
    #[must_use]
    pub fn queued_len(&self) -> usize {
        self.shared.lock().heap.len()
    }

    /// Number of live worker threads.
    #[must_use]
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Drain the queue and join all workers.
    pub fn shutdown(mut self) {
        self.stop(false);
    }

    fn stop(&mut self, cancel_queued: bool) {
        {
            let mut state = self.shared.lock();
            if cancel_queued {
                for e in &state.heap {
                    e.token.cancel();
                }
            }
            state.paused = false;
            state.shutting_down = true;
        }
        self.shared.cv.notify_all();
        for h in std::mem::take(&mut self.workers) {
            let _ = h.join();
        }
    }
}

impl Drop for JobSystem {
    fn drop(&mut self) {
        self.stop(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(p: Priority, seq: u64) -> Entry {
        let (tx, _rx) = crossbeam_channel::bounded(1);
        Entry {
            priority: p,
            seq,
            token: CancelToken::new(),
            job: Box::new(|_| {}),
            done: tx,
        }
    }

    #[test]
    fn entry_ordering() {
        let mut h = BinaryHeap::new();
        h.push(entry(Priority::Low, 0));
        h.push(entry(Priority::High, 2));
        h.push(entry(Priority::High, 1));
        h.push(entry(Priority::Critical, 3));
        let order: Vec<_> = std::iter::from_fn(|| h.pop().map(|e| e.seq)).collect();
        assert_eq!(order, vec![3, 1, 2, 0]);
    }

    #[test]
    fn builder_zero_workers() {
        assert!(matches!(
            JobSystem::builder().workers(0).build(),
            Err(JobError::ZeroWorkers)
        ));
    }

    #[test]
    fn submit_and_wait() {
        let sys = JobSystem::builder().workers(1).build().unwrap();
        let (tx, rx) = crossbeam_channel::bounded(1);
        let h = sys.submit(Priority::Normal, move |_| {
            tx.send(42).unwrap();
        });
        assert_eq!(h.wait(), JobStatus::Completed);
        assert_eq!(rx.recv().unwrap(), 42);
        sys.shutdown();
    }
}
