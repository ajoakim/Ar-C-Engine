//! Deterministic integration tests for the job scheduler.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use engine_jobs::{JobStatus, JobSystem, Priority};

const T: Duration = Duration::from_secs(5);

fn paused(workers: usize) -> JobSystem {
    JobSystem::builder()
        .workers(workers)
        .start_paused(true)
        .build()
        .unwrap()
}

#[test]
fn priority_order_single_worker() {
    let sys = paused(1);
    let (tx, rx) = crossbeam_channel::unbounded();
    for (p, id) in [
        (Priority::Low, 1),
        (Priority::High, 2),
        (Priority::Normal, 3),
        (Priority::Critical, 4),
        (Priority::Background, 5),
        (Priority::High, 6),
    ] {
        let tx = tx.clone();
        sys.submit(p, move |_| tx.send(id).unwrap());
    }
    drop(tx);
    sys.resume();
    sys.shutdown();
    let order: Vec<i32> = rx.try_iter().collect();
    assert_eq!(order, vec![4, 2, 6, 3, 1, 5]);
}

#[test]
fn fifo_within_class() {
    let sys = paused(1);
    let (tx, rx) = crossbeam_channel::unbounded();
    for i in 0..10 {
        let tx = tx.clone();
        sys.submit(Priority::Normal, move |_| tx.send(i).unwrap());
    }
    drop(tx);
    sys.resume();
    sys.shutdown();
    let order: Vec<i32> = rx.try_iter().collect();
    assert_eq!(order, (0..10).collect::<Vec<_>>());
}

#[test]
fn cancel_queued_never_runs() {
    let sys = paused(1);
    let flag = Arc::new(AtomicBool::new(false));
    let probe = Arc::new(());
    let (f, p) = (Arc::clone(&flag), Arc::clone(&probe));
    let h = sys.submit(Priority::Normal, move |_| {
        let _keep = &p;
        f.store(true, Ordering::SeqCst);
    });
    h.cancel();
    sys.resume();
    assert_eq!(h.wait(), JobStatus::Cancelled);
    sys.shutdown();
    assert!(!flag.load(Ordering::SeqCst));
    assert_eq!(Arc::strong_count(&probe), 1);
}

#[test]
fn cancel_running_observed() {
    let sys = JobSystem::builder().workers(1).build().unwrap();
    let (tx, rx) = crossbeam_channel::bounded(1);
    let h = sys.submit(Priority::Normal, move |token| {
        tx.send("started").unwrap();
        while !token.is_cancelled() {
            std::thread::yield_now();
        }
    });
    assert_eq!(rx.recv_timeout(T).unwrap(), "started");
    h.cancel();
    assert_eq!(h.wait(), JobStatus::Cancelled);
    sys.shutdown();
}

#[test]
fn panic_isolated() {
    let sys = JobSystem::builder().workers(1).build().unwrap();
    let h1 = sys.submit(Priority::Normal, |_| panic!("boom"));
    assert_eq!(h1.wait(), JobStatus::Panicked);
    let h2 = sys.submit(Priority::Normal, |_| {});
    assert_eq!(h2.wait(), JobStatus::Completed);
    sys.shutdown();
}

#[test]
fn shutdown_drains_and_joins() {
    let sys = paused(2);
    let counter = Arc::new(AtomicUsize::new(0));
    for _ in 0..20 {
        let c = Arc::clone(&counter);
        sys.submit(Priority::Normal, move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
    }
    assert_eq!(sys.queued_len(), 20);
    sys.shutdown();
    assert_eq!(counter.load(Ordering::SeqCst), 20);
}

#[test]
fn drop_cancels_queued() {
    let sys = paused(1);
    let counter = Arc::new(AtomicUsize::new(0));
    let handles: Vec<_> = (0..5)
        .map(|_| {
            let c = Arc::clone(&counter);
            sys.submit(Priority::Normal, move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            })
        })
        .collect();
    drop(sys);
    for h in handles {
        assert_eq!(h.wait(), JobStatus::Cancelled);
    }
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

#[test]
fn multi_worker_completes() {
    let sys = JobSystem::builder().workers(4).build().unwrap();
    let prios = [
        Priority::Background,
        Priority::Low,
        Priority::Normal,
        Priority::High,
        Priority::Critical,
    ];
    let handles: Vec<_> = (0..200)
        .map(|i| sys.submit(prios[i % prios.len()], |_| {}))
        .collect();
    for h in handles {
        assert_eq!(h.wait(), JobStatus::Completed);
    }
    sys.shutdown();
}
